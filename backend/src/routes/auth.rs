use axum::{
    extract::{Path, State},
    http::{header::SET_COOKIE, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::{
        jwt::{clear_cookie, session_cookie},
        ratelimit::{check_rate_limit, client_key},
        AuthUser,
    },
    error::{AppError, AppResult},
    models::{
        AuthResponse, ChallengeStart, ProfilePreview, StartChallengeRequest, VerifyChallengeRequest,
    },
    state::AppState,
};

pub async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<StartChallengeRequest>,
) -> AppResult<Json<ChallengeStart>> {
    let ip_key = client_key(&headers, "local");
    let challenge = state.auth.start(&payload.username, &ip_key).await?;
    Ok(Json(challenge))
}

pub async fn verify(
    State(state): State<AppState>,
    Json(payload): Json<VerifyChallengeRequest>,
) -> AppResult<Response> {
    let verified = state.auth.verify(payload.challenge_id).await?;
    let cookie = session_cookie(&verified.token, state.cfg.cookie_secure);
    Ok(with_cookie(
        Json(AuthResponse {
            user: verified.user,
        }),
        cookie,
    ))
}

pub async fn me(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<AuthResponse>> {
    let user = crate::auth::load_user(&state.db, auth.id).await?;
    Ok(Json(AuthResponse { user }))
}

pub async fn logout(State(state): State<AppState>) -> Response {
    with_cookie(
        StatusCode::NO_CONTENT,
        clear_cookie(state.cfg.cookie_secure),
    )
}

pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> AppResult<Json<ProfilePreview>> {
    let ip_key = client_key(&headers, "local");
    check_rate_limit(
        &mut state.redis.clone(),
        &format!("preview:{ip_key}"),
        20,
        60,
    )
    .await?;

    let profile = state.chesscom.fetch_profile(&username).await?;
    let stats = state.chesscom.fetch_stats(&profile.username).await.ok();
    let public = crate::chesscom::PublicProfile::from_parts(&profile, stats.as_ref());
    Ok(Json(ProfilePreview::from(&public)))
}

#[derive(Serialize)]
pub struct OAuthStartResponse {
    pub authorize_url: String,
}

pub async fn oauth_start(State(state): State<AppState>) -> AppResult<Json<OAuthStartResponse>> {
    let state_token = format!("ef-{}", Uuid::new_v4().simple());
    let authorize_url = state.oauth.authorize_url(&state_token)?;
    Ok(Json(OAuthStartResponse { authorize_url }))
}

pub async fn oauth_callback(State(_state): State<AppState>) -> AppError {
    AppError::BadRequest(
        "Chess.com OAuth callback is not active on this deployment. Use the profile challenge."
            .into(),
    )
}

pub fn with_cookie(body: impl IntoResponse, cookie: String) -> Response {
    let mut response = body.into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(SET_COOKIE, value);
    }
    response
}
