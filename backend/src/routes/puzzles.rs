use axum::{
    extract::{Path, Query, State},
    Json,
};
use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, Chess};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{
        PuzzleAttemptRequest, PuzzleAttemptResult, PuzzleDto, PuzzleList, PuzzleQuery, PuzzleStats,
    },
    state::AppState,
    training::{rating_for_attempt, review, FsrsState},
};

const PUZZLE_SELECT: &str = "SELECT p.id, p.game_id, p.move_id, p.kind, p.fen, p.solution_uci,
        p.solution_san, p.best_line, p.tags, p.epl, p.due_at, p.stability, p.difficulty,
        p.reps, p.lapses, p.state, p.mastered,
        g.white_username, g.black_username, g.result
     FROM puzzles p LEFT JOIN games g ON g.id = p.game_id";

pub async fn due(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<PuzzleQuery>,
) -> AppResult<Json<Vec<PuzzleDto>>> {
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let sql = format!(
        "{PUZZLE_SELECT}
         WHERE p.user_id = $1 AND p.mastered = false AND p.due_at <= now()
         ORDER BY p.due_at ASC LIMIT $2"
    );
    let items = sqlx::query_as::<_, PuzzleDto>(&sql)
        .bind(auth.id)
        .bind(i64::from(limit))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(items))
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<PuzzleQuery>,
) -> AppResult<Json<PuzzleList>> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).clamp(0, 100_000);

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM puzzles WHERE user_id = $1
         AND ($2::text IS NULL OR kind = $2)
         AND ($3::bool IS NULL OR mastered = $3)",
    )
    .bind(auth.id)
    .bind(query.kind.as_deref())
    .bind(query.mastered)
    .fetch_one(&state.db)
    .await?;

    let sql = format!(
        "{PUZZLE_SELECT}
         WHERE p.user_id = $1
           AND ($2::text IS NULL OR p.kind = $2)
           AND ($3::bool IS NULL OR p.mastered = $3)
         ORDER BY p.due_at ASC LIMIT $4 OFFSET $5"
    );
    let items = sqlx::query_as::<_, PuzzleDto>(&sql)
        .bind(auth.id)
        .bind(query.kind.as_deref())
        .bind(query.mastered)
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(&state.db)
        .await?;

    Ok(Json(PuzzleList {
        items,
        total: total as i32,
        limit,
        offset,
    }))
}

pub async fn stats(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<PuzzleStats>> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
        "SELECT
            COUNT(*) FILTER (WHERE mastered = false AND due_at <= now()),
            COUNT(*),
            COUNT(*) FILTER (WHERE mastered = true),
            COUNT(*) FILTER (WHERE kind = 'blunder_fix'),
            COUNT(*) FILTER (WHERE kind = 'endgame_conversion')
         FROM puzzles WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(PuzzleStats {
        due: row.0 as i32,
        total: row.1 as i32,
        mastered: row.2 as i32,
        blunder_fix: row.3 as i32,
        endgame_conversion: row.4 as i32,
    }))
}

#[derive(sqlx::FromRow)]
struct PuzzleRow {
    id: Uuid,
    solution_uci: String,
    solution_san: Option<String>,
    stability: f64,
    difficulty: f64,
    reps: i32,
    lapses: i32,
    state: String,
    mastered: bool,
    last_reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn attempt(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(puzzle_id): Path<Uuid>,
    Json(payload): Json<PuzzleAttemptRequest>,
) -> AppResult<Json<PuzzleAttemptResult>> {
    let puzzle = sqlx::query_as::<_, PuzzleRow>(
        "SELECT id, solution_uci, solution_san, stability, difficulty, reps, lapses,
                state, mastered, last_reviewed_at
         FROM puzzles WHERE id = $1 AND user_id = $2",
    )
    .bind(puzzle_id)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Puzzle not found."))?;

    validate_uci(&puzzle.solution_uci)?;
    let submitted = payload.uci.trim().to_ascii_lowercase();
    let submitted_move = UciMove::from_ascii(submitted.as_bytes())
        .map_err(|_| AppError::bad_request("That is not a valid UCI move."))?;
    let _ = submitted_move;

    let correct = submitted == puzzle.solution_uci.to_ascii_lowercase();
    let hints = payload.hints.clamp(0, 5) as u32;
    let elapsed_ms = payload.elapsed_ms.unwrap_or(0).max(0);
    let rating = payload
        .rating
        .filter(|rating| (1..=4).contains(rating))
        .unwrap_or_else(|| rating_for_attempt(correct, hints, elapsed_ms));

    let now = chrono::Utc::now();
    let fsrs_state = FsrsState {
        stability: puzzle.stability,
        difficulty: puzzle.difficulty,
        reps: puzzle.reps,
        lapses: puzzle.lapses,
        state: puzzle.state.clone(),
        last_reviewed_at: puzzle.last_reviewed_at,
        mastered: puzzle.mastered,
    };
    let next = review(&fsrs_state, rating, now);

    sqlx::query(
        "UPDATE puzzles SET stability = $2, difficulty = $3, reps = $4, lapses = $5,
                state = $6, due_at = $7, mastered = $8, last_rating = $9,
                last_reviewed_at = now()
         WHERE id = $1",
    )
    .bind(puzzle.id)
    .bind(next.stability)
    .bind(next.difficulty)
    .bind(next.reps)
    .bind(next.lapses)
    .bind(&next.state)
    .bind(next.due_at)
    .bind(next.mastered)
    .bind(i32::from(rating))
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO puzzle_attempts (puzzle_id, user_id, correct, hints, elapsed_ms, rating)
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(puzzle.id)
    .bind(auth.id)
    .bind(correct)
    .bind(i32::try_from(hints).unwrap_or(5))
    .bind(elapsed_ms as i32)
    .bind(i32::from(rating))
    .execute(&state.db)
    .await?;

    Ok(Json(PuzzleAttemptResult {
        correct,
        solution_uci: puzzle.solution_uci,
        solution_san: puzzle.solution_san,
        rating,
        interval_days: next.interval_days,
        due_at: next.due_at,
        stability: next.stability,
        state: next.state,
        mastered: next.mastered,
    }))
}

fn validate_uci(uci: &str) -> AppResult<()> {
    UciMove::from_ascii(uci.as_bytes())
        .map(|_| ())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid solution stored for puzzle")))
}

#[allow(dead_code)]
fn _validate_fen(fen: &str) -> bool {
    Fen::from_ascii(fen.as_bytes())
        .and_then(|fen| {
            fen.into_position::<Chess>(CastlingMode::Standard)
                .map(|_| ())
                .map_err(|_| shakmaty::fen::ParseFenError::InvalidBoard)
        })
        .is_ok()
}

#[allow(dead_code)]
fn _pool(_: &PgPool) {}
