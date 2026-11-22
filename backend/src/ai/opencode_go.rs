use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use futures::stream::StreamExt;
use rand::Rng;
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER, USER_AGENT},
    StatusCode,
};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    ai::provider::{ChatRole, LlmEvent, LlmProvider, LlmRequest, LlmStream},
    config::{Config, LlmProtocol},
    error::{AppError, AppResult},
    metrics::Metrics,
};

struct Breaker {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

pub struct OpenCodeGoProvider {
    cfg: Arc<Config>,
    http: reqwest::Client,
    metrics: Arc<Metrics>,
    breaker: Mutex<Breaker>,
}

impl std::fmt::Debug for OpenCodeGoProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenCodeGoProvider")
            .field("model", &self.cfg.opencode_model)
            .field("protocol", &self.cfg.opencode_protocol.as_str())
            .finish()
    }
}

impl OpenCodeGoProvider {
    pub fn new(cfg: &Config, metrics: Arc<Metrics>) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .user_agent("EloForge/1.0")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(cfg.opencode_timeout_sec.max(20)))
            .build()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to build AI client: {e}")))?;
        Ok(Self {
            cfg: Arc::new(cfg.clone()),
            http,
            metrics,
            breaker: Mutex::new(Breaker {
                consecutive_failures: 0,
                open_until: None,
            }),
        })
    }

    pub fn model_name(&self) -> &str {
        &self.cfg.opencode_model
    }

    async fn check_breaker(&self) -> AppResult<()> {
        let breaker = self.breaker.lock().await;
        if let Some(open_until) = breaker.open_until {
            if open_until > Instant::now() {
                let remaining = open_until.duration_since(Instant::now()).as_secs();
                return Err(AppError::AiUnavailable(format!(
                    "the coach is cooling down after repeated upstream failures ({remaining}s)"
                )));
            }
        }
        Ok(())
    }

    async fn note_failure(&self) {
        let mut breaker = self.breaker.lock().await;
        breaker.consecutive_failures += 1;
        if breaker.consecutive_failures >= 5 {
            breaker.open_until = Some(Instant::now() + Duration::from_secs(30));
            breaker.consecutive_failures = 0;
        }
    }

    async fn note_success(&self) {
        let mut breaker = self.breaker.lock().await;
        breaker.consecutive_failures = 0;
        breaker.open_until = None;
    }

    fn build_body(&self, request: &LlmRequest) -> Value {
        match self.cfg.opencode_protocol {
            LlmProtocol::Chat => {
                let messages: Vec<Value> = request
                    .messages
                    .iter()
                    .map(|message| {
                        json!({
                            "role": message.role.as_str(),
                            "content": message.content,
                        })
                    })
                    .collect();
                let mut body = json!({
                    "model": self.cfg.opencode_model,
                    "messages": messages,
                    "stream": true,
                    "max_tokens": request.max_tokens,
                });
                if let Some(temperature) = request.temperature {
                    body["temperature"] = json!(temperature);
                }
                body
            }
            LlmProtocol::Messages => {
                let system = join_system(&request.messages);
                let messages: Vec<Value> = request
                    .messages
                    .iter()
                    .filter(|m| m.role != ChatRole::System)
                    .map(|message| {
                        json!({
                            "role": message.role.as_str(),
                            "content": message.content,
                        })
                    })
                    .collect();
                let mut body = json!({
                    "model": self.cfg.opencode_model,
                    "messages": messages,
                    "max_tokens": request.max_tokens,
                    "stream": true,
                });
                if let Some(system) = system {
                    body["system"] = json!(system);
                }
                if let Some(temperature) = request.temperature {
                    body["temperature"] = json!(temperature);
                }
                body
            }
            LlmProtocol::Responses => {
                let system = join_system(&request.messages);
                let input: Vec<Value> = request
                    .messages
                    .iter()
                    .filter(|m| m.role != ChatRole::System)
                    .map(|message| {
                        json!({
                            "role": message.role.as_str(),
                            "content": message.content,
                        })
                    })
                    .collect();
                let mut body = json!({
                    "model": self.cfg.opencode_model,
                    "input": input,
                    "stream": true,
                    "max_output_tokens": request.max_tokens,
                });
                if let Some(system) = system {
                    body["instructions"] = json!(system);
                }
                if let Some(temperature) = request.temperature {
                    body["temperature"] = json!(temperature);
                }
                body
            }
        }
    }
}

fn join_system(messages: &[crate::ai::provider::ChatMessage]) -> Option<String> {
    let joined = messages
        .iter()
        .filter(|m| m.role == ChatRole::System)
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n\n");
    (!joined.is_empty()).then_some(joined)
}

#[async_trait]
impl LlmProvider for OpenCodeGoProvider {
    fn name(&self) -> &'static str {
        "opencode_go"
    }

    fn model(&self) -> &str {
        &self.cfg.opencode_model
    }

    fn is_configured(&self) -> bool {
        self.cfg.opencode_api_key.is_some()
    }

    async fn stream(&self, request: LlmRequest) -> AppResult<LlmStream> {
        let api_key =
            self.cfg.opencode_api_key.as_deref().ok_or_else(|| {
                AppError::AiUnavailable("OPENCODE_API_KEY is not configured".into())
            })?;

        self.check_breaker().await?;
        self.metrics.ai_requests.inc();
        let started = Instant::now();
        let url = format!(
            "{}{}",
            self.cfg.opencode_base_url.trim_end_matches('/'),
            self.cfg.opencode_protocol.path()
        );
        let body = self.build_body(&request);
        let body_text = body.to_string();

        let mut attempt = 0u32;
        let response = loop {
            attempt += 1;
            let sent = self
                .http
                .post(&url)
                .header(AUTHORIZATION, format!("Bearer {api_key}"))
                .header(USER_AGENT, "EloForge/1.0")
                .header("x-opencode-session", request.session_id.to_string())
                .header(CONTENT_TYPE, "application/json")
                .header(ACCEPT, "text/event-stream")
                .body(body_text.clone())
                .send()
                .await;

            match sent {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        self.note_success().await;
                        self.metrics
                            .ai_latency
                            .observe(started.elapsed().as_secs_f64());
                        break response;
                    }

                    let retryable = matches!(
                        status,
                        StatusCode::BAD_GATEWAY
                            | StatusCode::SERVICE_UNAVAILABLE
                            | StatusCode::TOO_MANY_REQUESTS
                    ) || status.as_u16() == 529;

                    let retry_after = response
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(Duration::from_secs);

                    let detail = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "no response body".to_string());
                    let detail: String = detail.chars().take(400).collect();

                    if retryable && attempt < 4 {
                        self.note_failure().await;
                        let wait = retry_after.unwrap_or_else(|| backoff(attempt));
                        tracing::warn!(
                            status = %status,
                            attempt,
                            wait_ms = wait.as_millis(),
                            "OpenCode Go transient failure; backing off"
                        );
                        tokio::time::sleep(wait).await;
                        continue;
                    }

                    self.metrics.ai_errors.inc();
                    self.note_failure().await;
                    return Err(AppError::AiUnavailable(format!(
                        "OpenCode Go returned {status}: {detail}"
                    )));
                }
                Err(err) => {
                    if attempt < 4 {
                        self.note_failure().await;
                        let wait = backoff(attempt);
                        tracing::warn!(
                            attempt,
                            wait_ms = wait.as_millis(),
                            error = %err,
                            "OpenCode Go request failed; backing off"
                        );
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                    self.metrics.ai_errors.inc();
                    self.note_failure().await;
                    return Err(AppError::AiUnavailable(format!(
                        "OpenCode Go request failed: {err}"
                    )));
                }
            }
        };

        let protocol = self.cfg.opencode_protocol;
        let input_estimate = request
            .messages
            .iter()
            .map(|m| m.content.chars().count())
            .sum::<usize>();
        let (tx, rx) = mpsc::channel::<AppResult<LlmEvent>>(64);

        tokio::spawn(async move {
            let mut bytes = response.bytes_stream();
            let mut parser = SseParser::default();
            let mut output_chars = 0usize;
            let mut reported_tokens = 0u64;
            let sent_done = false;

            while let Some(chunk) = bytes.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        let _ = tx
                            .send(Err(AppError::AiUnavailable(format!(
                                "stream interrupted: {err}"
                            ))))
                            .await;
                        return;
                    }
                };

                for (event, data) in parser.push(&chunk) {
                    if data.trim() == "[DONE]" {
                        let tokens = token_estimate(input_estimate, output_chars, reported_tokens);
                        let _ = tx.send(Ok(LlmEvent::Done { tokens })).await;
                        return;
                    }
                    let parsed = match serde_json::from_str::<Value>(&data) {
                        Ok(parsed) => parsed,
                        Err(_) => continue,
                    };

                    if let Some(tokens) = usage_tokens(&parsed) {
                        reported_tokens = reported_tokens.max(tokens);
                    }

                    if let Some(delta) = extract_delta(protocol, &parsed) {
                        if !delta.is_empty() {
                            output_chars += delta.chars().count();
                            if tx.send(Ok(LlmEvent::Delta(delta))).await.is_err() {
                                return;
                            }
                        }
                    }

                    if stream_finished(protocol, &parsed) && !sent_done {
                        let tokens = token_estimate(input_estimate, output_chars, reported_tokens);
                        let _ = tx.send(Ok(LlmEvent::Done { tokens })).await;
                        return;
                    }
                    let _ = event;
                }
            }

            if !sent_done {
                let tokens = token_estimate(input_estimate, output_chars, reported_tokens);
                let _ = tx.send(Ok(LlmEvent::Done { tokens })).await;
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}

fn extract_delta(protocol: LlmProtocol, value: &Value) -> Option<String> {
    match protocol {
        LlmProtocol::Chat => value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                value
                    .get("choices")
                    .and_then(|choices| choices.get(0))
                    .and_then(|choice| choice.get("text"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        LlmProtocol::Messages => {
            let kind = value.get("type").and_then(Value::as_str);
            if kind == Some("content_block_delta") {
                value
                    .get("delta")
                    .and_then(|delta| delta.get("text"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            } else {
                None
            }
        }
        LlmProtocol::Responses => {
            let kind = value.get("type").and_then(Value::as_str);
            if matches!(
                kind,
                Some("response.output_text.delta") | Some("response.refusal.delta")
            ) {
                value
                    .get("delta")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            } else {
                None
            }
        }
    }
}

fn stream_finished(protocol: LlmProtocol, value: &Value) -> bool {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match protocol {
        LlmProtocol::Chat => value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .is_some(),
        LlmProtocol::Messages => kind == "message_stop",
        LlmProtocol::Responses => matches!(kind, "response.completed" | "response.failed"),
    }
}

fn usage_tokens(value: &Value) -> Option<u64> {
    value
        .get("usage")
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("usage"))
                .and_then(|usage| usage.get("total_tokens"))
                .and_then(Value::as_u64)
        })
        .or_else(|| {
            value.get("usage").and_then(|usage| {
                let input = usage
                    .get("input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let output = usage
                    .get("output_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                (input + output > 0).then_some(input + output)
            })
        })
}

fn token_estimate(input_chars: usize, output_chars: usize, reported: u64) -> u64 {
    if reported > 0 {
        return reported;
    }
    ((input_chars + output_chars) / 4).max(1) as u64
}

fn backoff(attempt: u32) -> Duration {
    let base = 500u64.saturating_mul(1u64 << attempt.min(4));
    let jitter = rand::thread_rng().gen_range(0..250u64);
    Duration::from_millis((base + jitter).min(15_000))
}

#[derive(Default)]
struct SseParser {
    buffer: String,
    event: Option<String>,
    data: String,
}

impl SseParser {
    fn push(&mut self, chunk: &[u8]) -> Vec<(Option<String>, String)> {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
        let mut events = Vec::new();

        while let Some(index) = self.buffer.find('\n') {
            let line = self.buffer[..index].trim_end_matches('\r').to_string();
            self.buffer.drain(..=index);

            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push((self.event.take(), std::mem::take(&mut self.data)));
                } else {
                    self.event = None;
                }
                continue;
            }
            if let Some(value) = line.strip_prefix("event:") {
                self.event = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("data:") {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value.trim_start());
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sse_chunks_split_across_reads() {
        let mut parser = SseParser::default();
        let first = parser.push(b"event: delta\ndata: {\"a\":");
        assert!(first.is_empty());
        let second = parser.push(b"1}\n\n");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].0.as_deref(), Some("delta"));
        assert_eq!(second[0].1, "{\"a\":1}");
    }

    #[test]
    fn extracts_openai_chat_delta() {
        let value = serde_json::json!({
            "choices": [{"delta": {"content": "hello"}}]
        });
        assert_eq!(
            extract_delta(LlmProtocol::Chat, &value).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn extracts_anthropic_delta() {
        let value = serde_json::json!({
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "hello"}
        });
        assert_eq!(
            extract_delta(LlmProtocol::Messages, &value).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn extracts_responses_delta() {
        let value = serde_json::json!({
            "type": "response.output_text.delta",
            "delta": "hello"
        });
        assert_eq!(
            extract_delta(LlmProtocol::Responses, &value).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn estimates_tokens_when_upstream_omits_usage() {
        assert_eq!(token_estimate(40, 40, 0), 20);
        assert_eq!(token_estimate(40, 40, 99), 99);
    }
}
