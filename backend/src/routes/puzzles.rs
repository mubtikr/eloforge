use axum::{
    extract::{Path, Query, State},
    Json,
};
use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, Chess};
use sqlx::PgPool;
use uuid::Uuid;

use shakmaty::{san::SanPlus, Position};

use crate::{
    ai::coach::rule_for_tags,
    auth::AuthUser,
    engine::{EngineLine, EngineQuery},
    error::{AppError, AppResult},
    models::{
        PuzzleAttemptRequest, PuzzleAttemptResult, PuzzleDto, PuzzleExplanation, PuzzleList,
        PuzzleQuery, PuzzleStats,
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
           AND ($3::text IS NULL OR $3 = ANY(p.tags))
         ORDER BY p.due_at ASC LIMIT $2"
    );
    let items = sqlx::query_as::<_, PuzzleDto>(&sql)
        .bind(auth.id)
        .bind(i64::from(limit))
        .bind(query.tag.as_deref())
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
         AND ($3::bool IS NULL OR mastered = $3)
         AND ($4::text IS NULL OR $4 = ANY(tags))",
    )
    .bind(auth.id)
    .bind(query.kind.as_deref())
    .bind(query.mastered)
    .bind(query.tag.as_deref())
    .fetch_one(&state.db)
    .await?;

    let sql = format!(
        "{PUZZLE_SELECT}
         WHERE p.user_id = $1
           AND ($2::text IS NULL OR p.kind = $2)
           AND ($3::bool IS NULL OR p.mastered = $3)
           AND ($4::text IS NULL OR $4 = ANY(p.tags))
         ORDER BY p.due_at ASC LIMIT $5 OFFSET $6"
    );
    let items = sqlx::query_as::<_, PuzzleDto>(&sql)
        .bind(auth.id)
        .bind(query.kind.as_deref())
        .bind(query.mastered)
        .bind(query.tag.as_deref())
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

    let correct = !payload.revealed && submitted == puzzle.solution_uci.to_ascii_lowercase();
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

pub async fn explanation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(puzzle_id): Path<Uuid>,
) -> AppResult<Json<PuzzleExplanation>> {
    let row = sqlx::query_as::<_, PuzzleDetailRow>(
        "SELECT id, fen, solution_uci, best_line, tags, epl
         FROM puzzles WHERE id = $1 AND user_id = $2",
    )
    .bind(puzzle_id)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("Puzzle not found."))?;

    let pos = Fen::from_ascii(row.fen.as_bytes())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid puzzle FEN")))?
        .into_position::<Chess>(CastlingMode::Standard)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("illegal puzzle FEN")))?;
    let solution: UciMove = row
        .solution_uci
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid solution UCI")))?;
    let solution_move = solution
        .to_move(&pos)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("illegal solution move")))?;
    let solution_san = SanPlus::from_move(pos.clone(), solution_move).to_string();
    let after = pos
        .play(solution_move)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("illegal solution move")))?;
    let after_fen =
        shakmaty::fen::Fen::from_position(&after, shakmaty::EnPassantMode::Legal).to_string();

    let stored_line: Vec<EngineLine> = row
        .best_line
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let pv = stored_line
        .first()
        .map(|line| line.pv.clone())
        .unwrap_or_default();

    let reply_uci = pv.get(1).cloned().filter(|uci| {
        uci.parse::<UciMove>()
            .ok()
            .and_then(|parsed| parsed.to_move(&after).ok())
            .is_some()
    });

    let reply_uci = match reply_uci {
        Some(uci) => Some(uci),
        None => state
            .engine
            .evaluate(EngineQuery {
                fen: after_fen,
                depth: 12,
                multipv: 1,
                uci_elo: None,
                movetime_ms: Some(400),
            })
            .await
            .ok()
            .and_then(|eval| eval.best_move),
    };

    let mut line_sans = vec![solution_san.clone()];
    let mut reply_san = None;
    let mut line_pos = after.clone();
    if let Some(reply_uci) = reply_uci.clone() {
        if let Ok(parsed) = reply_uci.parse::<UciMove>() {
            if let Ok(reply_move) = parsed.to_move(&line_pos) {
                let san = SanPlus::from_move(line_pos.clone(), reply_move).to_string();
                reply_san = Some(san.clone());
                line_sans.push(san);
                if let Ok(next) = line_pos.clone().play(reply_move) {
                    line_pos = next;
                }
            }
        }
    }
    for uci in pv.iter().skip(2).take(3) {
        let Ok(parsed) = uci.parse::<UciMove>() else {
            break;
        };
        let Ok(mv) = parsed.to_move(&line_pos) else {
            break;
        };
        line_sans.push(SanPlus::from_move(line_pos.clone(), mv).to_string());
        if let Ok(next) = line_pos.clone().play(mv) {
            line_pos = next;
        } else {
            break;
        }
    }

    let (goal, motif) = puzzle_goal(&row.tags);
    let rule = rule_for_tags(&row.tags);
    let lost = row.epl.unwrap_or(0.0) * 100.0;
    let reply_text = reply_san
        .as_deref()
        .map(|san| format!(" The opponent's best reply is {san}."))
        .unwrap_or_default();
    let why = format!(
        "This position cost you {:.1} percentage points of winning chances. The engine move is {}. It keeps the tactics on your terms.{reply_text}",
        lost, solution_san
    );

    Ok(Json(PuzzleExplanation {
        puzzle_id: row.id,
        goal,
        motif,
        why,
        solution_san,
        opponent_reply_san: reply_san,
        best_line_san: line_sans,
        next_time_rule: rule,
        best_line: row.best_line,
    }))
}

#[derive(sqlx::FromRow)]
struct PuzzleDetailRow {
    id: Uuid,
    fen: String,
    solution_uci: String,
    best_line: Option<serde_json::Value>,
    tags: Vec<String>,
    epl: Option<f64>,
}

fn puzzle_goal(tags: &[String]) -> (String, String) {
    let has = |tag: &str| tags.iter().any(|value| value == tag);
    if has("MISSED_MATE_1_2") || has("ALLOWED_MATE") {
        (
            "Find the forced mate before the position slips.".into(),
            "Mating pattern".into(),
        )
    } else if has("HANGING_PIECE") || has("MISSED_FREE_PIECE") {
        (
            "Win the hanging material before it can be saved.".into(),
            "Hanging piece".into(),
        )
    } else if has("ENDGAME_TECHNIQUE_FAIL") || has("WINNING_GAME_THROWN") || has("STALEMATE_THROW")
    {
        (
            "Convert the winning endgame without allowing counterplay.".into(),
            "Endgame technique".into(),
        )
    } else if has("TACTICAL_BLINDNESS") {
        (
            "Find the short tactic before the opponent consolidates.".into(),
            "Tactical pattern".into(),
        )
    } else if has("IMPULSIVE_BLUNDER") || has("TIME_SCRAMBLE_BLUNDER") {
        (
            "Slow down and find the move the clock made you miss.".into(),
            "Clock discipline".into(),
        )
    } else if has("OPENING_PRINCIPLE_BREAK") {
        (
            "Find the developing move that keeps the opening sound.".into(),
            "Opening principle".into(),
        )
    } else {
        (
            "Find the move that changes the position in your favour.".into(),
            "Tactical chance".into(),
        )
    }
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
