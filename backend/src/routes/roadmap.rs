use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    goals::ladder::{roadmap_view, seed_roadmap},
    models::{GoalCreateRequest, GoalDto, RoadmapView},
    state::AppState,
};

pub async fn roadmap(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<RoadmapView>> {
    seed_roadmap(&state.db, auth.id).await?;
    Ok(Json(roadmap_view(&state.db, auth.id).await?))
}

pub async fn list_goals(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<GoalDto>>> {
    let rows = sqlx::query_as::<_, GoalDto>(
        "SELECT id, kind, stage, title, description, target_elo, target_date, progress,
                completed, created_at
         FROM milestones_and_goals WHERE user_id = $1 AND kind = 'goal'
         ORDER BY completed ASC, created_at DESC",
    )
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn create_goal(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<GoalCreateRequest>,
) -> AppResult<Json<GoalDto>> {
    let title = payload.title.trim();
    if title.len() < 3 || title.len() > 120 {
        return Err(AppError::bad_request(
            "Goal titles are between 3 and 120 characters.",
        ));
    }
    let row = sqlx::query_as::<_, GoalDto>(
        "INSERT INTO milestones_and_goals
            (user_id, kind, title, description, target_elo, target_date, progress)
         VALUES ($1,'goal',$2,$3,$4,$5,'{}'::jsonb)
         RETURNING id, kind, stage, title, description, target_elo, target_date, progress,
                   completed, created_at",
    )
    .bind(auth.id)
    .bind(title)
    .bind(payload.description.as_deref())
    .bind(payload.target_elo)
    .bind(payload.target_date)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

pub async fn complete_goal(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(goal_id): Path<Uuid>,
) -> AppResult<Json<GoalDto>> {
    let row = sqlx::query_as::<_, GoalDto>(
        "UPDATE milestones_and_goals SET completed = true
         WHERE id = $1 AND user_id = $2 AND kind = 'goal'
         RETURNING id, kind, stage, title, description, target_elo, target_date, progress,
                   completed, created_at",
    )
    .bind(goal_id)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Goal not found."))?;
    Ok(Json(row))
}
