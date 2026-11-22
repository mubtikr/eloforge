use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::StreamExt;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast;
use uuid::Uuid;

pub const CHANNEL_PREFIX: &str = "ef:events:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEvent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    pub message: String,
    pub progress: Option<u8>,
    pub data: serde_json::Value,
    pub ts: DateTime<Utc>,
}

impl AppEvent {
    pub fn new(
        user_id: Uuid,
        kind: impl Into<String>,
        message: impl Into<String>,
        progress: Option<u8>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            kind: kind.into(),
            message: message.into(),
            progress,
            data,
            ts: Utc::now(),
        }
    }

    pub fn simple(user_id: Uuid, kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(user_id, kind, message, None, json!({}))
    }

    pub fn channel(&self) -> String {
        format!("{CHANNEL_PREFIX}{}", self.user_id)
    }
}

pub struct EventBus {
    local: broadcast::Sender<AppEvent>,
    redis: redis::aio::MultiplexedConnection,
    client: redis::Client,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("subscribers", &self.local.receiver_count())
            .finish()
    }
}

impl EventBus {
    pub async fn new(client: redis::Client, redis: redis::aio::MultiplexedConnection) -> Arc<Self> {
        let (tx, _) = broadcast::channel(512);
        let bus = Arc::new(Self {
            local: tx,
            redis: redis.clone(),
            client,
        });

        let bridge_bus = bus.clone();
        tokio::spawn(async move {
            bridge_bus.run_valkey_bridge().await;
        });

        bus
    }

    async fn run_valkey_bridge(self: Arc<Self>) {
        let mut backoff = std::time::Duration::from_secs(1);
        loop {
            match self.subscribe_loop().await {
                Ok(()) => {
                    tracing::warn!("valkey event bridge stopped; reconnecting");
                }
                Err(err) => {
                    tracing::warn!(error = %err, "valkey event bridge failed; reconnecting");
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
        }
    }

    async fn subscribe_loop(&self) -> anyhow::Result<()> {
        let mut pubsub = self.client.get_async_pubsub().await?;
        pubsub.psubscribe(format!("{CHANNEL_PREFIX}*")).await?;
        let mut stream = pubsub.on_message();
        while let Some(msg) = stream.next().await {
            let payload: String = msg.get_payload()?;
            match serde_json::from_str::<AppEvent>(&payload) {
                Ok(event) => {
                    let _ = self.local.send(event);
                }
                Err(err) => tracing::debug!(error = %err, "ignored malformed event payload"),
            }
        }
        anyhow::bail!("valkey pubsub stream ended")
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.local.subscribe()
    }

    pub async fn publish(&self, event: AppEvent) {
        let payload = match serde_json::to_string(&event) {
            Ok(payload) => payload,
            Err(err) => {
                tracing::error!(error = %err, "failed to serialize event");
                return;
            }
        };
        let mut conn = self.redis.clone();
        let result: redis::RedisResult<i64> = conn.publish(event.channel(), payload).await;
        if let Err(err) = result {
            tracing::warn!(error = %err, "valkey publish failed; delivering locally");
            let _ = self.local.send(event);
        }
    }
}
