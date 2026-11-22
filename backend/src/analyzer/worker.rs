use std::time::Duration;

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    analyzer::game_analyzer::analyze_game,
    error::{AppError, AppResult},
    events::AppEvent,
    state::AppState,
};

const QUEUE_KEY: &str = "ef:analysis:jobs";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalysisJob {
    game_id: Uuid,
    user_id: Uuid,
}

pub async fn enqueue_game(
    db: &sqlx::PgPool,
    redis: &mut redis::aio::MultiplexedConnection,
    game_id: Uuid,
    user_id: Uuid,
) -> AppResult<bool> {
    let updated = sqlx::query(
        "UPDATE games SET analysis_status = 'queued', analysis_error = NULL
         WHERE id = $1 AND user_id = $2 AND analysis_status IN ('pending', 'failed')",
    )
    .bind(game_id)
    .bind(user_id)
    .execute(db)
    .await?;

    if updated.rows_affected() == 0 {
        return Ok(false);
    }

    let payload = serde_json::to_string(&AnalysisJob { game_id, user_id })
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to encode job: {e}")))?;
    let _: i64 = redis.lpush(QUEUE_KEY, payload).await?;
    Ok(true)
}

pub fn spawn_workers(state: AppState, count: usize) {
    for index in 0..count.max(1) {
        let state = state.clone();
        tokio::spawn(async move {
            tracing::info!(worker = index, "analysis worker online");
            worker_loop(state).await;
        });
    }

    let recovery = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if let Err(err) = recover_pending(&recovery).await {
                tracing::warn!(error = %err, "analysis recovery scan failed");
            }
        }
    });
}

async fn worker_loop(state: AppState) {
    let mut conn = match connect(&state).await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::error!(error = %err, "analysis worker could not connect to Valkey");
            return;
        }
    };

    loop {
        let popped: Option<(String, String)> = match redis::cmd("BRPOP")
            .arg(QUEUE_KEY)
            .arg(5)
            .query_async(&mut conn)
            .await
        {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(error = %err, "analysis queue read failed; reconnecting");
                tokio::time::sleep(Duration::from_secs(2)).await;
                if let Ok(next) = connect(&state).await {
                    conn = next;
                }
                continue;
            }
        };

        let Some((_, payload)) = popped else {
            continue;
        };
        let job: AnalysisJob = match serde_json::from_str(&payload) {
            Ok(job) => job,
            Err(err) => {
                tracing::warn!(error = %err, "discarding malformed analysis job");
                continue;
            }
        };

        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            run_job(&state_clone, job).await;
        });
        if let Err(err) = handle.await {
            tracing::error!(error = %err, "analysis job task panicked");
        }
    }
}

async fn run_job(state: &AppState, job: AnalysisJob) {
    state
        .events
        .publish(AppEvent::new(
            job.user_id,
            "analysis_started",
            "Stockfish is reviewing the game.",
            Some(5),
            serde_json::json!({ "game_id": job.game_id }),
        ))
        .await;

    match analyze_game(
        &state.db,
        &state.engine,
        &state.metrics,
        &state.events,
        &state.cfg,
        job.game_id,
        job.user_id,
    )
    .await
    {
        Ok(outcome) => {
            let created = crate::training::extract_puzzles_for_game(
                &state.db,
                &state.metrics,
                job.user_id,
                job.game_id,
            )
            .await
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "puzzle extraction failed");
                0
            });
            let _ = crate::goals::ladder::recompute_roadmap(&state.db, job.user_id).await;

            state
                .events
                .publish(AppEvent::new(
                    job.user_id,
                    "analysis_done",
                    format!("Review ready: {}% accuracy.", outcome.accuracy_user.round()),
                    Some(100),
                    serde_json::json!({
                        "game_id": job.game_id,
                        "accuracy_user": outcome.accuracy_user,
                        "blunders": outcome.blunders,
                        "mistakes": outcome.mistakes,
                        "puzzles_created": created,
                    }),
                ))
                .await;
        }
        Err(err) => {
            tracing::warn!(error = %err, game_id = %job.game_id, "analysis failed");
        }
    }
}

async fn recover_pending(state: &AppState) -> AppResult<usize> {
    let mut tx = state.db.begin().await?;
    let rows = sqlx::query_as::<_, (Uuid, Uuid)>(
        "UPDATE games SET analysis_status = 'queued'
         WHERE id IN (
            SELECT id FROM games
            WHERE analysis_status = 'queued'
               OR (analysis_status = 'pending' AND created_at < now() - interval '1 minute')
               OR (analysis_status = 'analyzing' AND updated_at < now() - interval '20 minutes')
            ORDER BY created_at ASC LIMIT 50
         )
         RETURNING id, user_id",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let mut requeued = 0;
    for (game_id, user_id) in rows {
        let payload = serde_json::to_string(&AnalysisJob { game_id, user_id })
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to encode job: {e}")))?;
        let mut redis = state.redis.clone();
        let _: i64 = redis.lpush(QUEUE_KEY, payload).await?;
        requeued += 1;
    }
    Ok(requeued)
}

async fn connect(state: &AppState) -> AppResult<redis::aio::MultiplexedConnection> {
    let client = redis::Client::open(state.cfg.valkey_url.clone())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid Valkey URL: {e}")))?;
    client
        .get_multiplexed_async_connection()
        .await
        .map_err(AppError::Cache)
}
