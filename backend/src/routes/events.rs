use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::{stream, StreamExt};
use tokio_stream::wrappers::BroadcastStream;

use crate::{auth::AuthUser, metrics::Metrics, state::AppState};

struct SseGuard {
    metrics: Arc<Metrics>,
}

impl SseGuard {
    fn new(metrics: Arc<Metrics>) -> Self {
        metrics.sse_connections.inc();
        metrics.active_websockets.inc();
        Self { metrics }
    }
}

impl Drop for SseGuard {
    fn drop(&mut self) {
        self.metrics.sse_connections.dec();
        self.metrics.active_websockets.dec();
    }
}

pub async fn stream(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let user_id = auth.id;
    let guard = Arc::new(SseGuard::new(state.metrics.clone()));
    let receiver = state.events.subscribe();

    let filtered = BroadcastStream::new(receiver).filter_map(move |item| {
        let guard = guard.clone();
        async move {
            let _keep_alive = guard;
            match item {
                Ok(event) if event.user_id == user_id => {
                    let payload = match serde_json::to_string(&event) {
                        Ok(payload) => payload,
                        Err(_) => return None,
                    };
                    Some(Ok(Event::default().event(event.kind).data(payload)))
                }
                _ => None,
            }
        }
    });

    let ready = stream::once(async {
        Ok(Event::default()
            .event("ready")
            .data(r#"{"message":"stream connected"}"#))
    })
    .chain(filtered);

    Sse::new(ready).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
