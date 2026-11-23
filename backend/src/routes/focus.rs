use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    analyzer::focus_tilt::{compute_focus, resolve_timezone, FocusReport},
    auth::AuthUser,
    error::AppResult,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct FocusQuery {
    pub tz: Option<String>,
}

pub async fn report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<FocusQuery>,
) -> AppResult<Json<FocusReport>> {
    let timezone = resolve_timezone(&state.db, auth.id, query.tz.as_deref()).await?;
    Ok(Json(compute_focus(&state.db, auth.id, &timezone).await?))
}
