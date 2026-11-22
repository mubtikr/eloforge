use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::{
    ai::{
        coach::{
            allowed_moves, build_reflection_prompt, extract_rule, fallback_reflection,
            find_memories, insert_reflection, load_move_context, save_reflection_result,
            strip_rule_line, validates_moves,
        },
        provider::{LlmEvent, LlmRequest},
    },
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{ReflectDone, ReflectMeta, ReflectRequest},
    state::AppState,
};

pub async fn reflect(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((game_id, move_id)): Path<(Uuid, i64)>,
    Json(payload): Json<ReflectRequest>,
) -> AppResult<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>> {
    let explanation = payload.explanation.trim().to_string();
    if explanation.len() < 3 {
        return Err(AppError::bad_request(
            "Write at least a short sentence about what you were thinking.",
        ));
    }
    if explanation.len() > 2000 {
        return Err(AppError::bad_request(
            "Keep the reflection under 2000 characters.",
        ));
    }
    let reason_tags: Vec<String> = payload
        .reason_tags
        .into_iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty() && tag.len() <= 40)
        .take(5)
        .collect();

    let ctx = load_move_context(&state.db, auth.id, game_id, move_id).await?;
    if ctx.move_color != ctx.user_color {
        return Err(AppError::Forbidden);
    }
    if !matches!(
        ctx.classification.as_deref(),
        Some("inaccuracy" | "mistake" | "blunder" | "missed_win")
    ) && ctx.tags.is_empty()
    {
        return Err(AppError::bad_request(
            "Pick a mistake, blunder or turning point to reflect on.",
        ));
    }

    let reflection_id = insert_reflection(&state.db, &ctx, &reason_tags, &explanation).await?;
    let memories = find_memories(
        &state.db,
        auth.id,
        reflection_id,
        &reason_tags,
        &crate::ai::coach::hash_embedding(&format!(
            "{} {} {}",
            explanation,
            reason_tags.join(" "),
            ctx.tags.join(" ")
        )),
    )
    .await
    .unwrap_or_default();

    let prompt = build_reflection_prompt(&ctx, &explanation, &memories);
    let (fallback_feedback, fallback_rule) = fallback_reflection(&ctx, &explanation);
    let allowed = allowed_moves(&ctx);
    let meta = ReflectMeta {
        reflection_id,
        session_id: ctx.ai_session_id,
        model: state.ai.model().to_string(),
        ai_configured: state.ai.is_configured(),
    };

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);
    let state_clone = state.clone();
    let fallback_for_task = (fallback_feedback, fallback_rule);

    tokio::spawn(async move {
        send_event(&tx, "meta", &meta).await;

        if !state_clone.ai.is_configured() {
            let _ = state_clone.ai.record_failure().await;
            let (feedback, rule) = fallback_for_task;
            send_fallback(
                &tx,
                &state_clone.db,
                reflection_id,
                &feedback,
                &rule,
                "unavailable",
            )
            .await;
            return;
        }

        let request = LlmRequest::new(ctx.ai_session_id, "reflection")
            .system(prompt.clone())
            .user("Write the reflection now, following the system instructions exactly.")
            .max_tokens(700);

        let mut stream = match state_clone.ai.stream(auth.id, request).await {
            Ok(stream) => stream,
            Err(err) => {
                tracing::warn!(error = %err, "reflection AI unavailable; using fallback");
                let (feedback, rule) = fallback_for_task;
                send_fallback(
                    &tx,
                    &state_clone.db,
                    reflection_id,
                    &feedback,
                    &rule,
                    "fallback",
                )
                .await;
                return;
            }
        };

        let mut text = String::new();
        let mut tokens = 0u64;
        let mut failed = false;
        while let Some(event) = stream.next().await {
            match event {
                Ok(LlmEvent::Delta(delta)) => {
                    text.push_str(&delta);
                    send_raw(&tx, "delta", json!({ "text": delta })).await;
                }
                Ok(LlmEvent::Done { tokens: reported }) => {
                    tokens = reported;
                    break;
                }
                Err(err) => {
                    tracing::warn!(error = %err, "reflection stream failed");
                    failed = true;
                    break;
                }
            }
        }

        if tokens > 0 {
            let _ = state_clone.ai.record_usage(auth.id, tokens).await;
        }

        let (feedback, rule, status) = if failed || text.trim().is_empty() {
            let (feedback, rule) = fallback_for_task;
            (feedback, rule, "fallback")
        } else if !validates_moves(&text, &allowed) {
            tracing::warn!("AI reflection mentioned a move outside the verified line");
            let (feedback, rule) = fallback_for_task;
            (feedback, rule, "rejected")
        } else {
            let rule = extract_rule(&text, &fallback_for_task.1);
            let feedback = strip_rule_line(&text);
            (feedback, rule, "ok")
        };

        if status != "ok" {
            send_raw(&tx, "notice", json!({ "message": "The coach reply was replaced with deterministic engine notes because it failed validation." })).await;
        }

        if let Err(err) =
            save_reflection_result(&state_clone.db, reflection_id, &feedback, &rule, status).await
        {
            tracing::error!(error = %err, "failed to save reflection result");
        }

        send_raw(&tx, "delta", json!({ "text": feedback })).await;
        let done = ReflectDone {
            reflection_id,
            next_time_rule: rule,
            ai_status: status.to_string(),
            model: state_clone.ai.model().to_string(),
        };
        send_event(&tx, "done", &done).await;
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    ))
}

async fn send_fallback(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    db: &sqlx::PgPool,
    reflection_id: Uuid,
    feedback: &str,
    rule: &str,
    status: &str,
) {
    if let Err(err) = save_reflection_result(db, reflection_id, feedback, rule, status).await {
        tracing::error!(error = %err, "failed to save fallback reflection");
    }
    send_raw(tx, "delta", json!({ "text": feedback })).await;
    let done = ReflectDone {
        reflection_id,
        next_time_rule: rule.to_string(),
        ai_status: status.to_string(),
        model: "deterministic".to_string(),
    };
    send_event(tx, "done", &done).await;
}

async fn send_event<T: serde::Serialize>(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    event: &str,
    data: &T,
) {
    let payload = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    let _ = tx
        .send(Ok(Event::default().event(event).data(payload)))
        .await;
}

async fn send_raw(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    event: &str,
    data: serde_json::Value,
) {
    send_event(tx, event, &data).await;
}
