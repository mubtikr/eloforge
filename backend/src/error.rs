use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    NotFound(String),
    #[error("rate limit exceeded: {0}")]
    RateLimited(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("cache error")]
    Cache(#[from] redis::RedisError),
    #[error("engine error: {0}")]
    Engine(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("ai unavailable: {0}")]
    AiUnavailable(String),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: String,
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::RateLimited(_) => "rate_limited",
            Self::Database(_) => "database_error",
            Self::Cache(_) => "cache_error",
            Self::Engine(_) => "engine_error",
            Self::Upstream(_) => "upstream_error",
            Self::AiUnavailable(_) => "ai_unavailable",
            Self::Internal(_) => "internal_error",
        }
    }

    fn public_message(&self) -> String {
        match self {
            Self::Database(_) | Self::Cache(_) | Self::Internal(_) => {
                "An internal error occurred. Please retry shortly.".to_string()
            }
            other => other.to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Database(_) | Self::Cache(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Engine(_) | Self::Upstream(_) | Self::AiUnavailable(_) => StatusCode::BAD_GATEWAY,
        };

        if status.is_server_error() {
            tracing::error!(error = %self, code = self.code(), "request failed");
        } else {
            tracing::warn!(error = %self, code = self.code(), "request rejected");
        }

        (
            status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code(),
                    message: self.public_message(),
                },
            }),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
