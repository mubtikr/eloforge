use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use std::time::Duration;

use crate::{error::AppResult, state::AppState};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_secs: u64,
    pub checks: HealthChecks,
    pub engine: EngineInfo,
}

#[derive(Serialize)]
pub struct HealthChecks {
    pub database: ComponentHealth,
    pub valkey: ComponentHealth,
    pub stockfish: ComponentHealth,
}

#[derive(Serialize)]
pub struct ComponentHealth {
    pub ok: bool,
    pub detail: String,
}

#[derive(Serialize)]
pub struct EngineInfo {
    pub workers: usize,
    pub queue_depth: usize,
    pub model: String,
    pub protocol: &'static str,
}

pub async fn healthz(
    State(state): State<AppState>,
) -> AppResult<(StatusCode, Json<HealthResponse>)> {
    let db = match tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.db),
    )
    .await
    {
        Ok(Ok(_)) => ComponentHealth {
            ok: true,
            detail: "primary reachable".into(),
        },
        Ok(Err(err)) => ComponentHealth {
            ok: false,
            detail: format!("query failed: {err}"),
        },
        Err(_) => ComponentHealth {
            ok: false,
            detail: "timed out".into(),
        },
    };

    let mut redis_conn = state.redis.clone();
    let valkey = match tokio::time::timeout(
        Duration::from_secs(3),
        redis::cmd("PING").query_async::<String>(&mut redis_conn),
    )
    .await
    {
        Ok(Ok(pong)) => ComponentHealth {
            ok: pong.eq_ignore_ascii_case("PONG"),
            detail: pong,
        },
        Ok(Err(err)) => ComponentHealth {
            ok: false,
            detail: format!("ping failed: {err}"),
        },
        Err(_) => ComponentHealth {
            ok: false,
            detail: "timed out".into(),
        },
    };

    let stockfish = match tokio::time::timeout(Duration::from_secs(4), state.engine.ping()).await {
        Ok(true) => ComponentHealth {
            ok: true,
            detail: format!("{} worker(s) responding", state.engine.worker_count()),
        },
        Ok(false) => ComponentHealth {
            ok: false,
            detail: "UCI handshake failed".into(),
        },
        Err(_) => ComponentHealth {
            ok: false,
            detail: "UCI handshake timed out".into(),
        },
    };

    let all_ok = db.ok && valkey.ok && stockfish.ok;
    let status = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    Ok((
        status,
        Json(HealthResponse {
            status: if all_ok { "ok" } else { "degraded" },
            version: env!("CARGO_PKG_VERSION"),
            uptime_secs: state.uptime_secs(),
            checks: HealthChecks {
                database: db,
                valkey,
                stockfish,
            },
            engine: EngineInfo {
                workers: state.engine.worker_count(),
                queue_depth: state.engine.queue_depth(),
                model: state.cfg.opencode_model.clone(),
                protocol: state.cfg.opencode_protocol.as_str(),
            },
        }),
    ))
}
