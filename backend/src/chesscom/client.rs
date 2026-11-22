use std::time::Duration;

use rand::Rng;
use reqwest::header::{ACCEPT, ETAG, IF_NONE_MATCH, RETRY_AFTER, USER_AGENT};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::{
    chesscom::models::{ChessComArchives, ChessComMonth, ChessComProfile, ChessComStats},
    config::Config,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone)]
pub struct CachedFetch {
    pub not_modified: bool,
    pub etag: Option<String>,
    pub body: Option<String>,
}

struct SerialGate {
    last_request: Option<tokio::time::Instant>,
}

pub struct ChessComClient {
    http: reqwest::Client,
    base_url: String,
    user_agent: String,
    min_interval: Duration,
    gate: Mutex<SerialGate>,
}

impl std::fmt::Debug for ChessComClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChessComClient")
            .field("base_url", &self.base_url)
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl ChessComClient {
    pub fn new(cfg: &Config) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .user_agent(cfg.chesscom_user_agent.clone())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(2)
            .build()
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("failed to build Chess.com client: {e}"))
            })?;

        Ok(Self {
            http,
            base_url: cfg.chesscom_base_url.clone(),
            user_agent: cfg.chesscom_user_agent.clone(),
            min_interval: Duration::from_millis(cfg.chesscom_min_interval_ms),
            gate: Mutex::new(SerialGate { last_request: None }),
        })
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub async fn fetch_profile(&self, username: &str) -> AppResult<ChessComProfile> {
        let url = self.url(&format!("player/{}", urlencoding::encode(username)));
        let fetched = self.get(&url, None).await?;
        let body = fetched
            .body
            .ok_or_else(|| AppError::Upstream("empty Chess.com profile response".into()))?;
        serde_json::from_str(&body)
            .map_err(|e| AppError::Upstream(format!("invalid Chess.com profile JSON: {e}")))
    }

    pub async fn fetch_stats(&self, username: &str) -> AppResult<ChessComStats> {
        let url = self.url(&format!("player/{}/stats", urlencoding::encode(username)));
        let fetched = self.get(&url, None).await?;
        let body = fetched
            .body
            .ok_or_else(|| AppError::Upstream("empty Chess.com stats response".into()))?;
        serde_json::from_str(&body)
            .map_err(|e| AppError::Upstream(format!("invalid Chess.com stats JSON: {e}")))
    }

    pub fn archives_url(&self, username: &str) -> String {
        self.url(&format!(
            "player/{}/games/archives",
            urlencoding::encode(username)
        ))
    }

    pub async fn fetch_archives(
        &self,
        username: &str,
        etag: Option<&str>,
    ) -> AppResult<CachedFetch> {
        let url = self.archives_url(username);
        self.get(&url, etag).await
    }

    pub fn parse_archives(&self, fetch: CachedFetch) -> AppResult<Option<ChessComArchives>> {
        match fetch.body {
            Some(body) => serde_json::from_str(&body)
                .map(Some)
                .map_err(|e| AppError::Upstream(format!("invalid Chess.com archives JSON: {e}"))),
            None => Ok(None),
        }
    }

    pub async fn fetch_archive(&self, url: &str, etag: Option<&str>) -> AppResult<CachedFetch> {
        self.get(url, etag).await
    }

    pub fn parse_month(&self, fetch: CachedFetch) -> AppResult<Option<ChessComMonth>> {
        match fetch.body {
            Some(body) => serde_json::from_str(&body)
                .map(Some)
                .map_err(|e| AppError::Upstream(format!("invalid Chess.com month JSON: {e}"))),
            None => Ok(None),
        }
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        etag: Option<&str>,
    ) -> AppResult<T> {
        let fetched = self.get(url, etag).await?;
        let body = fetched
            .body
            .ok_or_else(|| AppError::Upstream("unexpected 304 without cached body".into()))?;
        serde_json::from_str(&body)
            .map_err(|e| AppError::Upstream(format!("invalid Chess.com JSON: {e}")))
    }

    pub async fn get(&self, url: &str, etag: Option<&str>) -> AppResult<CachedFetch> {
        let mut gate = self.gate.lock().await;

        if let Some(last) = gate.last_request {
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
        }

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let mut request = self
                .http
                .get(url)
                .header(USER_AGENT, &self.user_agent)
                .header(ACCEPT, "application/json");

            if let Some(tag) = etag {
                request = request.header(IF_NONE_MATCH, tag);
            }

            gate.last_request = Some(tokio::time::Instant::now());
            let response = request.send().await;

            let response = match response {
                Ok(response) => response,
                Err(err) => {
                    if attempt >= 4 {
                        return Err(AppError::Upstream(format!(
                            "Chess.com request failed: {err}"
                        )));
                    }
                    let wait = backoff(attempt);
                    tracing::warn!(
                        attempt,
                        wait_ms = wait.as_millis(),
                        error = %err,
                        "Chess.com request failed; backing off"
                    );
                    tokio::time::sleep(wait).await;
                    continue;
                }
            };

            let status = response.status();

            if status == reqwest::StatusCode::NOT_MODIFIED {
                return Ok(CachedFetch {
                    not_modified: true,
                    etag: etag.map(str::to_string),
                    body: None,
                });
            }

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::BAD_GATEWAY
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
                || status.as_u16() == 529
            {
                if attempt >= 6 {
                    return Err(AppError::RateLimited(format!(
                        "Chess.com returned {status} after {attempt} attempts"
                    )));
                }
                let retry_after = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|secs| Duration::from_secs(secs.clamp(1, 60)))
                    .unwrap_or_else(|| backoff(attempt));
                tracing::warn!(
                    attempt,
                    status = %status,
                    wait_ms = retry_after.as_millis(),
                    "Chess.com asked us to slow down"
                );
                tokio::time::sleep(retry_after).await;
                continue;
            }

            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(AppError::NotFound(
                    "Chess.com resource was not found. Check the username.".into(),
                ));
            }

            if status == reqwest::StatusCode::FORBIDDEN {
                return Err(AppError::Upstream(
                    "Chess.com refused the request. Check CHESSCOM_USER_AGENT.".into(),
                ));
            }

            if !status.is_success() {
                return Err(AppError::Upstream(format!(
                    "Chess.com returned HTTP {status}"
                )));
            }

            let etag = response
                .headers()
                .get(ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let body = response
                .text()
                .await
                .map_err(|e| AppError::Upstream(format!("failed to read Chess.com body: {e}")))?;

            return Ok(CachedFetch {
                not_modified: false,
                etag,
                body: Some(body),
            });
        }
    }
}

fn backoff(attempt: u32) -> Duration {
    let base = 400u64.saturating_mul(1u64 << attempt.min(5));
    let jitter = rand::thread_rng().gen_range(0..300u64);
    Duration::from_millis((base + jitter).min(30_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_but_is_capped() {
        for attempt in 1..10 {
            let wait = backoff(attempt);
            assert!(wait >= Duration::from_millis(400));
            assert!(wait <= Duration::from_millis(31_000));
        }
    }
}
