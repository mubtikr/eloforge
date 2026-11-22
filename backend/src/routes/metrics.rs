use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};

use crate::{error::AppResult, state::AppState};

pub async fn metrics(State(state): State<AppState>) -> AppResult<Response> {
    state
        .metrics
        .stockfish_queue_depth
        .set(state.engine.queue_depth() as i64);
    let (content_type, body) = state.metrics.render()?;
    let mut response = body.into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("text/plain; version=0.0.4")),
    );
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}
