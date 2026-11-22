pub mod auth;
pub mod events;
pub mod focus;
pub mod games;
pub mod health;
pub mod metrics;
pub mod puzzles;
pub mod reflect;
pub mod reports;
pub mod roadmap;
pub mod sparring;
pub mod sync;

use axum::{
    routing::{get, post},
    Router,
};

use crate::{notifications, state::AppState};

pub fn router() -> Router<AppState> {
    let settings = notifications::settings::router();
    Router::new()
        .route("/api/healthz", get(health::healthz))
        .route("/metrics", get(metrics::metrics))
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
        .route("/api/games/{id}/analyze", post(games::analyze))
        .route(
            "/api/games/{game_id}/moves/{move_id}/reflect",
            post(reflect::reflect),
        )
        .route("/api/sync/trigger", post(sync::trigger))
        .route("/api/sync/status", get(sync::status))
        .route("/api/events/stream", get(events::stream))
        .route("/api/focus", get(focus::report))
        .route("/api/puzzles/due", get(puzzles::due))
        .route("/api/puzzles", get(puzzles::list))
        .route("/api/puzzles/stats", get(puzzles::stats))
        .route("/api/puzzles/{id}/attempt", post(puzzles::attempt))
        .route("/api/roadmap", get(roadmap::roadmap))
        .route("/api/goals", get(roadmap::list_goals))
        .route("/api/goals", post(roadmap::create_goal))
        .route("/api/goals/{id}/complete", post(roadmap::complete_goal))
        .route("/api/sparring/start", post(sparring::start))
        .route("/api/sparring", get(sparring::list))
        .route("/api/sparring/{id}", get(sparring::get))
        .route("/api/sparring/{id}/move", post(sparring::make_move))
        .route("/api/reports", get(reports::list))
        .route("/api/reports/openings", get(reports::openings))
        .route("/api/reports/weaknesses", get(reports::weaknesses))
        .route("/api/reports/conversion", get(reports::conversion))
        .route("/api/reports/reflections", get(reports::reflections))
        .route("/api/reports/focus", get(reports::focus))
        .route("/api/reports/roadmap", get(reports::roadmap))
        .route("/api/reports/match/{game_id}", get(reports::match_report))
        .route(
            "/api/reports/weekly/generate",
            post(reports::weekly_generate),
        )
        .route("/api/reports/weekly/latest", get(reports::weekly_latest))
        .route(
            "/api/reports/curriculum/generate",
            post(reports::curriculum_generate),
        )
        .merge(settings)
}
