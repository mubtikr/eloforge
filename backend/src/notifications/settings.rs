use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use chrono_tz::Tz;
use std::str::FromStr;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{
        NotificationDelivery, NotificationSettingsDto, NotificationSettingsUpdate, VapidKeyResponse,
    },
    notifications::worker::{deliver, load_settings},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/settings/notifications",
            get(get_settings).put(put_settings),
        )
        .route("/api/settings/vapid", get(vapid))
        .route("/api/settings/notifications/test", post(send_test))
}

pub async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<NotificationSettingsDto>> {
    let row = load_settings(&state.db, auth.id).await?;
    Ok(Json(row.to_dto()))
}

pub async fn put_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<NotificationSettingsUpdate>,
) -> AppResult<Json<NotificationSettingsDto>> {
    if !(0..=23).contains(&payload.reminder_hour) {
        return Err(AppError::bad_request(
            "Reminder hour must be between 0 and 23.",
        ));
    }
    if Tz::from_str(&payload.timezone).is_err() {
        return Err(AppError::bad_request(
            "Use an IANA timezone such as Europe/Berlin or America/New_York.",
        ));
    }
    if let Some(email) = payload.email.as_deref() {
        if !email.is_empty() && (!email.contains('@') || email.len() > 254) {
            return Err(AppError::bad_request("That email address is not valid."));
        }
    }
    if let Some(subscription) = &payload.web_push_subscription {
        if !subscription.is_null()
            && (subscription.get("endpoint").is_none() || subscription.get("keys").is_none())
        {
            return Err(AppError::bad_request(
                "The web push subscription is missing endpoint or keys.",
            ));
        }
    }

    sqlx::query(
        "INSERT INTO notification_settings
            (user_id, telegram_chat_id, email, web_push_subscription,
             daily_reminder, tilt_alerts, reminder_hour, timezone)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         ON CONFLICT (user_id) DO UPDATE SET
            telegram_chat_id = EXCLUDED.telegram_chat_id,
            email = EXCLUDED.email,
            web_push_subscription = EXCLUDED.web_push_subscription,
            daily_reminder = EXCLUDED.daily_reminder,
            tilt_alerts = EXCLUDED.tilt_alerts,
            reminder_hour = EXCLUDED.reminder_hour,
            timezone = EXCLUDED.timezone",
    )
    .bind(auth.id)
    .bind(
        payload
            .telegram_chat_id
            .as_deref()
            .filter(|v| !v.trim().is_empty()),
    )
    .bind(payload.email.as_deref().filter(|v| !v.trim().is_empty()))
    .bind(&payload.web_push_subscription)
    .bind(payload.daily_reminder)
    .bind(payload.tilt_alerts)
    .bind(payload.reminder_hour)
    .bind(&payload.timezone)
    .execute(&state.db)
    .await?;

    let row = load_settings(&state.db, auth.id).await?;
    Ok(Json(row.to_dto()))
}

pub async fn vapid(State(state): State<AppState>, _auth: AuthUser) -> Json<VapidKeyResponse> {
    Json(VapidKeyResponse {
        public_key: state.cfg.vapid_public_key.clone(),
    })
}

pub async fn send_test(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<NotificationDelivery>>> {
    let settings = load_settings(&state.db, auth.id).await?;
    let mut deliveries = Vec::new();
    let message = "EloForge test: your study-room notifications are wired correctly.";
    for delivery in deliver(&state, &settings, "test", "EloForge test", message).await {
        sqlx::query(
            "INSERT INTO notifications_log (user_id, channel, kind, payload, sent_at, error)
             VALUES ($1,$2,'test',$3,now(),$4)",
        )
        .bind(auth.id)
        .bind(&delivery.channel)
        .bind(serde_json::json!({"title": "EloForge test", "body": message}))
        .bind((!delivery.ok).then_some(delivery.detail.clone()))
        .execute(&state.db)
        .await?;
        deliveries.push(delivery);
    }
    if deliveries.is_empty() {
        return Err(AppError::bad_request(
            "No notification channel is configured yet.",
        ));
    }
    Ok(Json(deliveries))
}
