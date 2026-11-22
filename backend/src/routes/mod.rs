pub mod auth;
pub mod events;
pub mod games;
pub mod health;
pub mod sync;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/healthz", get(health::healthz))
        .route("/api/auth/chesscom/start", post(auth::start))
        .route("/api/auth/chesscom/verify", post(auth::verify))
        .route("/api/auth/chesscom/preview/{username}", get(auth::preview))
        .route("/api/auth/chesscom/oauth/start", get(auth::oauth_start))
        .route("/api/auth/chesscom/callback", get(auth::oauth_callback))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/games", get(games::list))
        .route("/api/games/{id}", get(games::detail))
        .route("/api/games/{id}/pgn", get(games::pgn))
        .route("/api/sync/trigger", post(sync::trigger))
        .route("/api/sync/status", get(sync::status))
        .route("/api/events/stream", get(events::stream))
}
