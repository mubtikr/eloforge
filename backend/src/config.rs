use std::env;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProtocol {
    Chat,
    Messages,
    Responses,
}

impl LlmProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Messages => "messages",
            Self::Responses => "responses",
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Self::Chat => "/chat/completions",
            Self::Messages => "/messages",
            Self::Responses => "/responses",
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub valkey_url: String,
    pub jwt_secret: String,
    pub cookie_secure: bool,
    pub port: u16,
    pub frontend_origin: String,
    pub log_format: String,
    pub stockfish_path: String,
    pub stockfish_workers: usize,
    pub stockfish_hash_mb: u32,
    pub stockfish_threads: usize,
    pub stockfish_timeout_ms: u64,
    pub analysis_depth_full: u8,
    pub analysis_depth_critical: u8,
    pub analysis_movetime_full_ms: u64,
    pub analysis_movetime_critical_ms: u64,
    pub opencode_api_key: Option<String>,
    pub opencode_base_url: String,
    pub opencode_model: String,
    pub opencode_protocol: LlmProtocol,
    pub opencode_timeout_sec: u64,
    pub ai_daily_token_budget: i64,
    pub chesscom_base_url: String,
    pub chesscom_user_agent: String,
    pub chesscom_min_interval_ms: u64,
    pub chesscom_oauth_enabled: bool,
    pub chesscom_oauth_client_id: Option<String>,
    pub chesscom_oauth_client_secret: Option<String>,
    pub chesscom_oauth_redirect_uri: String,
    pub telegram_bot_token: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: String,
    pub vapid_public_key: Option<String>,
    pub vapid_private_key: Option<String>,
    pub vapid_subject: String,
    pub sync_interval_minutes: u64,
    pub sync_max_archives: usize,
}

fn var(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("missing required environment variable {key}"))
}

fn var_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> Result<T> {
    match env::var(key) {
        Ok(raw) if !raw.trim().is_empty() => raw
            .parse::<T>()
            .map_err(|_| anyhow::anyhow!("invalid value for {key}: {raw}")),
        _ => Ok(default),
    }
}

fn opt(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|v| {
        let v = v.trim().to_string();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    })
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let jwt_secret = var("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            anyhow::bail!("JWT_SECRET must be at least 32 characters");
        }

        let default_workers = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(1);

        let protocol_raw = var_or("OPENCODE_GO_PROTOCOL", "chat");
        let opencode_protocol = match protocol_raw.as_str() {
            "chat" | "openai" | "chat_completions" => LlmProtocol::Chat,
            "messages" | "anthropic" => LlmProtocol::Messages,
            "responses" | "openai_responses" => LlmProtocol::Responses,
            other => anyhow::bail!("unknown OPENCODE_GO_PROTOCOL: {other}"),
        };

        let cfg = Self {
            database_url: var("DATABASE_URL")?,
            valkey_url: var("VALKEY_URL")?,
            jwt_secret,
            cookie_secure: parse_or("COOKIE_SECURE", true)?,
            port: parse_or("PORT", 8080u16)?,
            frontend_origin: var_or("FRONTEND_ORIGIN", "http://localhost:5173"),
            log_format: var_or("LOG_FORMAT", "pretty"),
            stockfish_path: var_or("STOCKFISH_PATH", "stockfish"),
            stockfish_workers: parse_or("STOCKFISH_WORKERS", default_workers)?.max(1),
            stockfish_hash_mb: parse_or("STOCKFISH_HASH_MB", 128u32)?,
            stockfish_threads: {
                let workers = parse_or("STOCKFISH_WORKERS", default_workers)?.max(1);
                let cores = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(2);
                parse_or("STOCKFISH_THREADS", (cores / workers).clamp(1, 4))?
            },
            stockfish_timeout_ms: parse_or("STOCKFISH_TIMEOUT_MS", 5000u64)?,
            analysis_depth_full: parse_or("ANALYSIS_DEPTH_FULL", 14u8)?,
            analysis_depth_critical: parse_or("ANALYSIS_DEPTH_CRITICAL", 20u8)?,
            analysis_movetime_full_ms: parse_or("ANALYSIS_MOVETIME_FULL_MS", 160u64)?,
            analysis_movetime_critical_ms: parse_or("ANALYSIS_MOVETIME_CRITICAL_MS", 600u64)?,
            opencode_api_key: opt("OPENCODE_API_KEY"),
            opencode_base_url: var_or("OPENCODE_GO_BASE_URL", "https://opencode.ai/zen/go/v1")
                .trim_end_matches('/')
                .to_string(),
            opencode_model: var_or("OPENCODE_GO_MODEL", "deepseek-v4.1-flash"),
            opencode_protocol,
            opencode_timeout_sec: parse_or("OPENCODE_GO_TIMEOUT_SEC", 90u64)?,
            ai_daily_token_budget: parse_or("AI_DAILY_TOKEN_BUDGET", 250_000i64)?,
            chesscom_base_url: var_or("CHESSCOM_BASE_URL", "https://api.chess.com/pub")
                .trim_end_matches('/')
                .to_string(),
            chesscom_user_agent: var_or(
                "CHESSCOM_USER_AGENT",
                "EloForge/1.0 (contact: contact@eloforge.app)",
            ),
            chesscom_min_interval_ms: parse_or("CHESSCOM_MIN_INTERVAL_MS", 200u64)?,
            chesscom_oauth_enabled: parse_or("CHESSCOM_OAUTH_ENABLED", false)?,
            chesscom_oauth_client_id: opt("CHESSCOM_OAUTH_CLIENT_ID"),
            chesscom_oauth_client_secret: opt("CHESSCOM_OAUTH_CLIENT_SECRET"),
            chesscom_oauth_redirect_uri: var_or(
                "CHESSCOM_OAUTH_REDIRECT_URI",
                "http://localhost:8080/api/auth/chesscom/callback",
            ),
            telegram_bot_token: opt("TELEGRAM_BOT_TOKEN"),
            smtp_host: opt("SMTP_HOST"),
            smtp_port: parse_or("SMTP_PORT", 587u16)?,
            smtp_username: opt("SMTP_USERNAME"),
            smtp_password: opt("SMTP_PASSWORD"),
            smtp_from: var_or("SMTP_FROM", "coach@eloforge.app"),
            vapid_public_key: opt("VAPID_PUBLIC_KEY"),
            vapid_private_key: opt("VAPID_PRIVATE_KEY"),
            vapid_subject: var_or("VAPID_SUBJECT", "mailto:contact@eloforge.app"),
            sync_interval_minutes: parse_or("SYNC_INTERVAL_MINUTES", 15u64)?.max(1),
            sync_max_archives: parse_or("SYNC_MAX_ARCHIVES", 120usize)?.max(1),
        };

        if cfg.stockfish_workers > default_workers {
            tracing::warn!(
                requested = cfg.stockfish_workers,
                allowed = default_workers,
                "STOCKFISH_WORKERS exceeds available parallelism minus one"
            );
        }

        Ok(cfg)
    }
}
