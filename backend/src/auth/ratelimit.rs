use redis::AsyncCommands;

use crate::error::{AppError, AppResult};

pub async fn check_rate_limit(
    redis: &mut redis::aio::MultiplexedConnection,
    bucket: &str,
    limit: i64,
    window_secs: u64,
) -> AppResult<i64> {
    let key = format!("ef:rl:{bucket}");
    let count: i64 = redis
        .incr(&key, 1i64)
        .await
        .map_err(|_| AppError::RateLimited("rate limiter unavailable".into()))?;

    if count == 1 {
        let _: Result<bool, _> = redis.expire(&key, window_secs as i64).await;
    }

    if count > limit {
        return Err(AppError::RateLimited(
            "too many attempts. Please wait a minute and try again.".into(),
        ));
    }
    Ok(count)
}

pub fn client_key(headers: &axum::http::HeaderMap, fallback: &str) -> String {
    use sha2::{Digest, Sha256};

    let raw = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|v| !v.is_empty())
        })
        .unwrap_or(fallback);

    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(&hasher.finalize()[..8])
}
