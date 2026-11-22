use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::{
    analyzer::focus_tilt::{compute_focus, FocusReport},
    auth::AuthUser,
    error::{AppError, AppResult},
    models::GameDetail,
    reports::{
        self,
        weekly::{generate_curriculum, generate_weekly, latest_weekly},
        ConversionReport, CurriculumReport, OpeningReport, ReflectionReport, ReportSummary,
        RoadmapReport, WeaknessReport, WeeklyReport,
    },
    routes::games::load_game_detail,
    state::AppState,
};

pub async fn openings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<OpeningReport>> {
    Ok(Json(reports::opening_report(&state.db, auth.id).await?))
}

pub async fn weaknesses(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<WeaknessReport>> {
    Ok(Json(reports::weakness_report(&state.db, auth.id).await?))
}

pub async fn conversion(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<ConversionReport>> {
    Ok(Json(reports::conversion_report(&state.db, auth.id).await?))
}

pub async fn reflections(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<ReflectionReport>> {
    Ok(Json(reports::reflection_report(&state.db, auth.id).await?))
}

pub async fn focus(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<FocusReport>> {
    Ok(Json(compute_focus(&state.db, auth.id).await?))
}

pub async fn roadmap(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<RoadmapReport>> {
    Ok(Json(reports::roadmap_report(&state.db, auth.id).await?))
}

pub async fn match_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<GameDetail>> {
    Ok(Json(load_game_detail(&state, auth.id, game_id).await?))
}

pub async fn weekly_generate(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<WeeklyReport>> {
    Ok(Json(generate_weekly(&state.db, &state.ai, auth.id).await?))
}

pub async fn weekly_latest(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<WeeklyReport>> {
    latest_weekly(&state.db, auth.id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found("No weekly briefing has been generated yet."))
}

pub async fn curriculum_generate(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<CurriculumReport>> {
    Ok(Json(
        generate_curriculum(&state.db, &state.ai, auth.id).await?,
    ))
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<ReportSummary>>> {
    let rows = sqlx::query_as::<_, ReportSummary>(
        "SELECT id, kind, title, model, created_at FROM coaching_reports
         WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}
