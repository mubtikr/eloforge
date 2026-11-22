use std::{io::Cursor, time::Duration};

use chrono::{Timelike, Utc};
use chrono_tz::Tz;
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushMessageBuilder,
};

use crate::{
    error::{AppError, AppResult},
    models::{NotificationDelivery, NotificationSettingsDto},
    state::AppState,
};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SettingsRow {
    pub user_id: Uuid,
    pub telegram_chat_id: Option<String>,
    pub email: Option<String>,
    pub web_push_subscription: Option<serde_json::Value>,
    pub daily_reminder: bool,
    pub tilt_alerts: bool,
    pub reminder_hour: i32,
    pub timezone: String,
    pub username: String,
    pub current_elo: Option<i32>,
}

impl SettingsRow {
    pub fn to_dto(&self) -> NotificationSettingsDto {
        NotificationSettingsDto {
            user_id: self.user_id,
            telegram_chat_id: self.telegram_chat_id.clone(),
            email: self.email.clone(),
            web_push_subscription: self.web_push_subscription.clone(),
            daily_reminder: self.daily_reminder,
            tilt_alerts: self.tilt_alerts,
            reminder_hour: self.reminder_hour,
            timezone: self.timezone.clone(),
        }
    }
}

pub async fn load_settings(db: &PgPool, user_id: Uuid) -> AppResult<SettingsRow> {
    let row = sqlx::query_as::<_, SettingsRow>(
        "SELECT u.id AS user_id, ns.telegram_chat_id, ns.email, ns.web_push_subscription,
                COALESCE(ns.daily_reminder, true) AS daily_reminder,
                COALESCE(ns.tilt_alerts, true) AS tilt_alerts,
                COALESCE(ns.reminder_hour, 9) AS reminder_hour,
                COALESCE(ns.timezone, 'UTC') AS timezone,
                u.chesscom_username AS username, u.current_elo
         FROM users u
         LEFT JOIN notification_settings ns ON ns.user_id = u.id
         WHERE u.id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::not_found("User not found."))?;

    Ok(SettingsRow {
        daily_reminder: row.daily_reminder,
        tilt_alerts: row.tilt_alerts,
        reminder_hour: row.reminder_hour,
        timezone: row.timezone,
        ..row
    })
}

pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(45)).await;
        let mut ticker = tokio::time::interval(Duration::from_secs(900));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(err) = run_cycle(&state).await {
                tracing::warn!(error = %err, "notification cycle failed");
            }
        }
    });
}

async fn run_cycle(state: &AppState) -> AppResult<()> {
    let rows = sqlx::query_as::<_, SettingsRow>(
        "SELECT ns.user_id, ns.telegram_chat_id, ns.email, ns.web_push_subscription,
                ns.daily_reminder, ns.tilt_alerts, ns.reminder_hour, ns.timezone,
                u.chesscom_username AS username, u.current_elo
         FROM notification_settings ns
         JOIN users u ON u.id = ns.user_id
         LIMIT 500",
    )
    .fetch_all(&state.db)
    .await?;

    for settings in rows {
        let tz = settings.timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC);
        let local_now = Utc::now().with_timezone(&tz);
        let hour = local_now.hour() as i32;

        if settings.daily_reminder
            && hour == settings.reminder_hour
            && !sent_recently(&state.db, settings.user_id, "daily_nudge", 20).await?
        {
            let due = due_puzzles(&state.db, settings.user_id).await?;
            let rule = latest_rule(&state.db, settings.user_id).await?;
            let body = if let Some(rule) = rule {
                format!("{due} blunder puzzle(s) are due. Your next-time rule: {rule}")
            } else {
                format!("{due} blunder puzzle(s) are due. Ten focused minutes is enough.")
            };
            send_and_log(
                state,
                &settings,
                "daily_nudge",
                "Today's EloForge session",
                &body,
            )
            .await?;
        }

        if settings.tilt_alerts
            && !sent_recently(&state.db, settings.user_id, "tilt_alert", 6).await?
        {
            let last_two = sqlx::query_as::<_, (String,)>(
                "SELECT result FROM games WHERE user_id = $1
                 ORDER BY start_time DESC LIMIT 2",
            )
            .bind(settings.user_id)
            .fetch_all(&state.db)
            .await?;
            let losses = last_two.iter().filter(|(result,)| result == "loss").count();
            if losses >= 2 {
                send_and_log(
                    state,
                    &settings,
                    "tilt_alert",
                    "Two losses in a row",
                    "Stop for today. Review one game, write one reflection, then close the board.",
                )
                .await?;
            }
        }

        if !sent_recently(&state.db, settings.user_id, "streak_alert", 20).await? {
            let days: Vec<(chrono::NaiveDate,)> = sqlx::query_as(
                "SELECT DISTINCT start_time::date FROM games
                 WHERE user_id = $1 ORDER BY 1 DESC LIMIT 7",
            )
            .bind(settings.user_id)
            .fetch_all(&state.db)
            .await?;
            let today = Utc::now().date_naive();
            let yesterday = today - chrono::Duration::days(1);
            let mut streak = 0i64;
            let mut cursor = if days.iter().any(|(day,)| *day == today) {
                today
            } else if days.iter().any(|(day,)| *day == yesterday) {
                yesterday
            } else {
                today - chrono::Duration::days(2)
            };
            for (day,) in &days {
                if *day == cursor {
                    streak += 1;
                    cursor -= chrono::Duration::days(1);
                } else if *day < cursor {
                    break;
                }
            }
            if streak >= 3 {
                send_and_log(
                    state,
                    &settings,
                    "streak_alert",
                    "Study streak",
                    &format!("{streak} days in a row. One short review keeps the chain alive."),
                )
                .await?;
            }
        }

        if !sent_recently(&state.db, settings.user_id, "milestone_alert", 20).await? {
            if let Some((target, title)) = next_milestone(&state.db, settings.user_id).await? {
                let distance = target - settings.current_elo.unwrap_or(0);
                if (0..=25).contains(&distance) {
                    send_and_log(
                        state,
                        &settings,
                        "milestone_alert",
                        "Milestone within reach",
                        &format!(
                            "You are {distance} Elo from {} ({}). One clean session could close it.",
                            target, title
                        ),
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

async fn due_puzzles(db: &PgPool, user_id: Uuid) -> AppResult<i64> {
    let due = sqlx::query_scalar(
        "SELECT COUNT(*) FROM puzzles WHERE user_id = $1 AND mastered = false AND due_at <= now()",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?;
    Ok(due)
}

async fn latest_rule(db: &PgPool, user_id: Uuid) -> AppResult<Option<String>> {
    let rule = sqlx::query_scalar::<_, Option<String>>(
        "SELECT next_week_rule FROM coaching_reports
         WHERE user_id = $1 AND kind = 'weekly'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .flatten();
    Ok(rule)
}

async fn next_milestone(db: &PgPool, user_id: Uuid) -> AppResult<Option<(i32, String)>> {
    let row = sqlx::query_as::<_, (i32, String)>(
        "SELECT target_elo, title FROM milestones_and_goals
         WHERE user_id = $1 AND kind = 'milestone' AND completed = false
         ORDER BY target_elo ASC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

async fn sent_recently(db: &PgPool, user_id: Uuid, kind: &str, hours: i64) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications_log
         WHERE user_id = $1 AND kind = $2 AND created_at > now() - ($3 || ' hours')::interval",
    )
    .bind(user_id)
    .bind(kind)
    .bind(hours.to_string())
    .fetch_one(db)
    .await?;
    Ok(count > 0)
}

async fn send_and_log(
    state: &AppState,
    settings: &SettingsRow,
    kind: &str,
    title: &str,
    body: &str,
) -> AppResult<()> {
    let deliveries = deliver(state, settings, kind, title, body).await;
    for delivery in deliveries {
        sqlx::query(
            "INSERT INTO notifications_log (user_id, channel, kind, payload, sent_at, error)
             VALUES ($1,$2,$3,$4,now(),$5)",
        )
        .bind(settings.user_id)
        .bind(&delivery.channel)
        .bind(kind)
        .bind(serde_json::json!({"title": title, "body": body}))
        .bind((!delivery.ok).then_some(delivery.detail.clone()))
        .execute(&state.db)
        .await?;
    }
    Ok(())
}

pub async fn deliver(
    state: &AppState,
    settings: &SettingsRow,
    kind: &str,
    title: &str,
    body: &str,
) -> Vec<NotificationDelivery> {
    let mut deliveries = Vec::new();

    if let (Some(token), Some(chat_id)) = (
        state.cfg.telegram_bot_token.as_deref(),
        settings.telegram_chat_id.as_deref(),
    ) {
        deliveries.push(send_telegram(&state.http, token, chat_id, kind, title, body).await);
    }

    if let (Some(host), Some(email)) = (state.cfg.smtp_host.as_deref(), settings.email.as_deref()) {
        deliveries.push(send_email(&state.cfg, host, email, kind, title, body).await);
    }

    if let (Some(private_key), Some(subscription)) = (
        state.cfg.vapid_private_key.as_deref(),
        settings.web_push_subscription.as_ref(),
    ) {
        deliveries.push(send_web_push(private_key, subscription, title, body).await);
    }

    deliveries
}

async fn send_telegram(
    http: &reqwest::Client,
    token: &str,
    chat_id: &str,
    kind: &str,
    title: &str,
    body: &str,
) -> NotificationDelivery {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let text = format!("EloForge · {title}\n{body}");
    let result = http
        .post(url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "disable_web_page_preview": true,
        }))
        .send()
        .await;
    match result {
        Ok(response) if response.status().is_success() => NotificationDelivery {
            channel: "telegram".into(),
            ok: true,
            detail: format!("{kind} delivered"),
        },
        Ok(response) => NotificationDelivery {
            channel: "telegram".into(),
            ok: false,
            detail: format!("Telegram returned HTTP {}", response.status()),
        },
        Err(err) => NotificationDelivery {
            channel: "telegram".into(),
            ok: false,
            detail: err.to_string(),
        },
    }
}

async fn send_email(
    cfg: &crate::config::Config,
    host: &str,
    email: &str,
    kind: &str,
    title: &str,
    body: &str,
) -> NotificationDelivery {
    let from: Mailbox = match cfg.smtp_from.parse() {
        Ok(from) => from,
        Err(err) => {
            return NotificationDelivery {
                channel: "email".into(),
                ok: false,
                detail: format!("invalid SMTP_FROM: {err}"),
            }
        }
    };
    let to: Mailbox = match email.parse() {
        Ok(to) => to,
        Err(err) => {
            return NotificationDelivery {
                channel: "email".into(),
                ok: false,
                detail: format!("invalid recipient: {err}"),
            }
        }
    };
    let message = match Message::builder()
        .from(from)
        .to(to)
        .subject(format!("EloForge · {title}"))
        .body(format!("{body}\n\n— EloForge"))
    {
        Ok(message) => message,
        Err(err) => {
            return NotificationDelivery {
                channel: "email".into(),
                ok: false,
                detail: err.to_string(),
            }
        }
    };

    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(host)
        .map(|builder| builder.port(cfg.smtp_port));
    if let (Some(username), Some(password)) = (&cfg.smtp_username, &cfg.smtp_password) {
        builder = builder.map(|builder| {
            builder.credentials(Credentials::new(username.clone(), password.clone()))
        });
    }
    let transport = match builder {
        Ok(builder) => builder.build(),
        Err(err) => {
            return NotificationDelivery {
                channel: "email".into(),
                ok: false,
                detail: err.to_string(),
            }
        }
    };
    match transport.send(message).await {
        Ok(_) => NotificationDelivery {
            channel: "email".into(),
            ok: true,
            detail: format!("{kind} delivered"),
        },
        Err(err) => NotificationDelivery {
            channel: "email".into(),
            ok: false,
            detail: err.to_string(),
        },
    }
}

#[derive(Debug, Deserialize)]
struct BrowserSubscription {
    endpoint: String,
    keys: BrowserKeys,
}

#[derive(Debug, Deserialize)]
struct BrowserKeys {
    p256dh: String,
    auth: String,
}

async fn send_web_push(
    private_key_pem: &str,
    subscription: &serde_json::Value,
    title: &str,
    body: &str,
) -> NotificationDelivery {
    let subscription: BrowserSubscription = match serde_json::from_value(subscription.clone()) {
        Ok(subscription) => subscription,
        Err(err) => {
            return NotificationDelivery {
                channel: "web_push".into(),
                ok: false,
                detail: format!("invalid subscription: {err}"),
            }
        }
    };
    let info = SubscriptionInfo::new(
        subscription.endpoint,
        subscription.keys.p256dh,
        subscription.keys.auth,
    );
    let signature = VapidSignatureBuilder::from_pem(Cursor::new(private_key_pem.as_bytes()), &info)
        .and_then(|builder| builder.build());
    let signature = match signature {
        Ok(signature) => signature,
        Err(err) => {
            return NotificationDelivery {
                channel: "web_push".into(),
                ok: false,
                detail: format!("VAPID signing failed: {err}"),
            }
        }
    };
    let payload = format!("{}", serde_json::json!({"title": title, "body": body}));
    let mut builder = WebPushMessageBuilder::new(&info);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
    builder.set_vapid_signature(signature);
    let message = match builder.build() {
        Ok(message) => message,
        Err(err) => {
            return NotificationDelivery {
                channel: "web_push".into(),
                ok: false,
                detail: err.to_string(),
            }
        }
    };
    let client = match IsahcWebPushClient::new() {
        Ok(client) => client,
        Err(err) => {
            return NotificationDelivery {
                channel: "web_push".into(),
                ok: false,
                detail: err.to_string(),
            }
        }
    };
    match tokio::time::timeout(Duration::from_secs(10), client.send(message)).await {
        Ok(Ok(())) => NotificationDelivery {
            channel: "web_push".into(),
            ok: true,
            detail: "delivered".into(),
        },
        Ok(Err(err)) => NotificationDelivery {
            channel: "web_push".into(),
            ok: false,
            detail: err.to_string(),
        },
        Err(_) => NotificationDelivery {
            channel: "web_push".into(),
            ok: false,
            detail: "timed out".into(),
        },
    }
}
