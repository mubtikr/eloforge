use std::{sync::Arc, time::Instant};

use sqlx::PgPool;

use crate::{config::Config, engine::EnginePool, events::EventBus, metrics::Metrics};

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub db: PgPool,
    pub redis: redis::aio::MultiplexedConnection,
    pub events: Arc<EventBus>,
    pub engine: Arc<EnginePool>,
    pub metrics: Arc<Metrics>,
    pub http: reqwest::Client,
    pub started_at: Instant,
}

impl AppState {
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}
