use axum::{extract::State, Json};

use crate::{
    auth::{load_user, AuthUser},
    error::AppResult,
    models::{SyncStatus, SyncTriggerResponse},
    state::AppState,
};

pub async fn trigger(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<SyncTriggerResponse>> {
    let user = load_user(&state.db, auth.id).await?;
    let accepted = state.sync.trigger(&user).await?;
    Ok(Json(SyncTriggerResponse {
        accepted,
        message: if accepted {
            "Sync started. Progress is streaming live.".into()
        } else {
            "A sync is already running for this account.".into()
        },
    }))
}

pub async fn status(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<SyncStatus>> {
    Ok(Json(state.sync.status(auth.id).await?))
}
