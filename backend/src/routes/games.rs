use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{
        AnalyzeResponse, GameDetail, GamePage, GameSummary, GamesQuery, MoveDto, ReflectionDto,
    },
    state::AppState,
};

const GAME_COLUMNS: &str = "id, url, white_username, black_username, white_rating, black_rating,
    user_color, result, result_reason, time_class, time_control, rated, start_time, end_time,
    move_count, opening_eco, opening_name, last_book_ply, analysis_status, accuracy_user,
    max_advantage, session_id";

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<GamesQuery>,
) -> AppResult<Json<GamePage>> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).clamp(0, 100_000);

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM games WHERE user_id = $1
         AND ($2::text IS NULL OR analysis_status = $2)
         AND ($3::text IS NULL OR result = $3)",
    )
    .bind(auth.id)
    .bind(query.status.as_deref())
    .bind(query.result.as_deref())
    .fetch_one(&state.db)
    .await?;

    let sql = format!(
        "SELECT {GAME_COLUMNS} FROM games WHERE user_id = $1
         AND ($2::text IS NULL OR analysis_status = $2)
         AND ($3::text IS NULL OR result = $3)
         ORDER BY start_time DESC LIMIT $4 OFFSET $5"
    );
    let items = sqlx::query_as::<_, GameSummary>(&sql)
        .bind(auth.id)
        .bind(query.status.as_deref())
        .bind(query.result.as_deref())
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(&state.db)
        .await?;

    Ok(Json(GamePage {
        items,
        total: total as i32,
        limit,
        offset,
    }))
}

pub async fn detail(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<GameDetail>> {
    let sql = format!("SELECT {GAME_COLUMNS} FROM games WHERE id = $1 AND user_id = $2");
    let game = sqlx::query_as::<_, GameSummary>(&sql)
        .bind(game_id)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Game not found."))?;

    let extras = sqlx::query_as::<_, GameExtras>(
        "SELECT initial_fen, accuracy_white, accuracy_black, eval_peak, analysis
         FROM games WHERE id = $1 AND user_id = $2",
    )
    .bind(game_id)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;

    let moves = sqlx::query_as::<_, MoveDto>(
        "SELECT id, ply, move_number, color, san, uci, fen_before, fen_after,
                clock_remaining_sec, time_spent_sec, eval_before_cp, eval_after_cp,
                eval_mate, win_prob_user, best_move_uci, best_move_san, epl,
                classification, mistake_tags, pv
         FROM game_moves WHERE game_id = $1 ORDER BY ply",
    )
    .bind(game_id)
    .fetch_all(&state.db)
    .await?;

    let reflections = sqlx::query_as::<_, ReflectionDto>(
        "SELECT id, move_id, reason_tags, explanation, ai_feedback, next_time_rule,
                ai_status, created_at
         FROM move_reflections WHERE game_id = $1 AND user_id = $2
         ORDER BY created_at DESC",
    )
    .bind(game_id)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(GameDetail {
        game,
        initial_fen: extras.initial_fen,
        accuracy_white: extras.accuracy_white,
        accuracy_black: extras.accuracy_black,
        eval_peak: extras.eval_peak,
        analysis: extras.analysis,
        moves,
        reflections,
    }))
}

#[derive(sqlx::FromRow)]
struct GameExtras {
    initial_fen: String,
    accuracy_white: Option<f64>,
    accuracy_black: Option<f64>,
    eval_peak: Option<f64>,
    analysis: Option<serde_json::Value>,
}

pub async fn analyze(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<AnalyzeResponse>> {
    let owned: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM games WHERE id = $1 AND user_id = $2")
            .bind(game_id)
            .bind(auth.id)
            .fetch_optional(&state.db)
            .await?;
    owned.ok_or_else(|| AppError::not_found("Game not found."))?;

    let mut redis = state.redis.clone();
    let queued = crate::analyzer::enqueue_game(&state.db, &mut redis, game_id, auth.id).await?;

    Ok(Json(AnalyzeResponse {
        queued,
        message: if queued {
            "Queued for Stockfish analysis.".into()
        } else {
            "This game is already analyzed or in progress.".into()
        },
    }))
}

pub async fn pgn(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Response> {
    let pgn: Option<String> =
        sqlx::query_scalar("SELECT pgn FROM games WHERE id = $1 AND user_id = $2")
            .bind(game_id)
            .bind(auth.id)
            .fetch_optional(&state.db)
            .await?;
    let pgn = pgn.ok_or_else(|| AppError::not_found("Game not found."))?;

    let mut response = pgn.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-chess-pgn; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"eloforge-game.pgn\""),
    );
    Ok(response)
}
