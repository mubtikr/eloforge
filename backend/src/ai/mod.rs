pub mod coach;
pub mod opencode_go;
pub mod prompts;
pub mod provider;

use std::sync::Arc;

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    ai::{
        opencode_go::OpenCodeGoProvider,
        provider::{collect_text, LlmProvider, LlmRequest, LlmStream},
    },
    config::Config,
    error::{AppError, AppResult},
    metrics::Metrics,
};

pub struct AiService {
    provider: Arc<dyn LlmProvider>,
    db: PgPool,
    cfg: Arc<Config>,
    metrics: Arc<Metrics>,
}

impl AiService {
    pub fn new(db: PgPool, cfg: &Config, metrics: Arc<Metrics>) -> AppResult<Arc<Self>> {
        let provider = Arc::new(OpenCodeGoProvider::new(cfg, metrics.clone())?);
        Ok(Arc::new(Self {
            provider,
            db,
            cfg: Arc::new(cfg.clone()),
            metrics,
        }))
    }

    pub fn is_configured(&self) -> bool {
        self.provider.is_configured()
    }

    pub fn model(&self) -> &str {
        self.provider.model()
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    pub fn daily_budget(&self) -> i64 {
        self.cfg.ai_daily_token_budget
    }

    pub async fn used_today(&self, user_id: Uuid) -> AppResult<i64> {
        let used = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT tokens FROM ai_token_usage WHERE user_id = $1 AND day = CURRENT_DATE",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?
        .flatten()
        .unwrap_or(0);
        Ok(used)
    }

    pub async fn ensure_budget(&self, user_id: Uuid) -> AppResult<()> {
        let used = self.used_today(user_id).await?;
        if used >= self.cfg.ai_daily_token_budget {
            return Err(AppError::RateLimited(
                "the daily AI token budget is used up. The deterministic coach notes are still available."
                    .into(),
            ));
        }
        Ok(())
    }

    pub async fn record_usage(&self, user_id: Uuid, tokens: u64) -> AppResult<()> {
        if tokens == 0 {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO ai_token_usage (user_id, day, tokens, requests)
             VALUES ($1, CURRENT_DATE, $2, 1)
             ON CONFLICT (user_id, day) DO UPDATE SET
                tokens = ai_token_usage.tokens + EXCLUDED.tokens,
                requests = ai_token_usage.requests + 1",
        )
        .bind(user_id)
        .bind(tokens as i64)
        .execute(&self.db)
        .await?;
        self.metrics.ai_tokens.inc_by(tokens);
        Ok(())
    }

    pub async fn stream(&self, user_id: Uuid, request: LlmRequest) -> AppResult<LlmStream> {
        if !self.provider.is_configured() {
            return Err(AppError::AiUnavailable(
                "OPENCODE_API_KEY is not configured; using deterministic coach notes.".into(),
            ));
        }
        self.ensure_budget(user_id).await?;
        self.provider.stream(request).await
    }

    pub async fn complete(&self, user_id: Uuid, request: LlmRequest) -> AppResult<(String, u64)> {
        let stream = self.stream(user_id, request).await?;
        let (text, tokens) = collect_text(stream).await?;
        self.record_usage(user_id, tokens).await?;
        Ok((text, tokens))
    }

    pub async fn record_failure(&self) {
        self.metrics.ai_errors.inc();
    }
}

pub fn stable_session(namespace: &str) -> Uuid {
    let digest = Sha256::digest(namespace.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
