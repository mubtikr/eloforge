use axum::{extract::State, Json};

use crate::{
    analyzer::focus_tilt::{compute_focus, FocusReport},
    auth::AuthUser,
    error::AppResult,
    state::AppState,
};

pub async fn report(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<FocusReport>> {
    Ok(Json(compute_focus(&state.db, auth.id).await?))
}
