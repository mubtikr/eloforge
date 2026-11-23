use axum::{
    extract::{Path, State},
    Json,
};
use shakmaty::{
    fen::Fen, san::SanPlus, uci::UciMove, CastlingMode, Chess, EnPassantMode, Move, Position, Role,
};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    engine::EngineQuery,
    error::{AppError, AppResult},
    models::{
        SparringHint, SparringMoveDto, SparringMoveRequest, SparringMoveResponse,
        SparringSessionDto, SparringStartRequest,
    },
    state::AppState,
};

#[derive(Clone, sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    game_id: Option<Uuid>,
    puzzle_id: Option<Uuid>,
    fen: String,
    initial_fen: String,
    moves: serde_json::Value,
    status: String,
    result: Option<String>,
    user_elo: Option<i32>,
    puzzle_epl: Option<f64>,
    puzzle_tags: Vec<String>,
    white_username: Option<String>,
    black_username: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl SessionRow {
    fn into_dto(self) -> AppResult<SparringSessionDto> {
        let moves: Vec<SparringMoveDto> = serde_json::from_value(self.moves)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid sparring moves: {e}")))?;
        let user_color = if self
            .initial_fen
            .split_whitespace()
            .nth(1)
            .map(|turn| turn == "w")
            .unwrap_or(true)
        {
            "white"
        } else {
            "black"
        };
        let enemy_elo = self.user_elo.map(|elo| (elo + 100).clamp(1320, 3190));
        Ok(SparringSessionDto {
            id: self.id,
            game_id: self.game_id,
            puzzle_id: self.puzzle_id,
            fen: self.fen,
            initial_fen: self.initial_fen,
            user_color: user_color.to_string(),
            enemy_elo,
            status: self.status,
            result: self.result,
            moves,
            puzzle_epl: self.puzzle_epl,
            puzzle_tags: self.puzzle_tags,
            white_username: self.white_username,
            black_username: self.black_username,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

const SESSION_SELECT: &str = "SELECT s.id, s.game_id, s.puzzle_id, s.fen, s.initial_fen, s.moves,
        s.status, s.result, s.user_elo,
        p.epl AS puzzle_epl, COALESCE(p.tags, '{}') AS puzzle_tags,
        g.white_username, g.black_username,
        s.created_at, s.updated_at
     FROM sparring_sessions s
     LEFT JOIN puzzles p ON p.id = s.puzzle_id
     LEFT JOIN games g ON g.id = s.game_id";

pub async fn start(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<SparringStartRequest>,
) -> AppResult<Json<SparringSessionDto>> {
    let puzzle = if let Some(puzzle_id) = payload.puzzle_id {
        sqlx::query_as::<_, (Uuid, Option<Uuid>, String)>(
            "SELECT id, game_id, fen FROM puzzles
             WHERE id = $1 AND user_id = $2 AND kind = 'endgame_conversion'",
        )
        .bind(puzzle_id)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
    } else if let Some(game_id) = payload.game_id {
        sqlx::query_as::<_, (Uuid, Option<Uuid>, String)>(
            "SELECT id, game_id, fen FROM puzzles
             WHERE game_id = $1 AND user_id = $2 AND kind = 'endgame_conversion'
             ORDER BY due_at ASC LIMIT 1",
        )
        .bind(game_id)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, (Uuid, Option<Uuid>, String)>(
            "SELECT id, game_id, fen FROM puzzles
             WHERE user_id = $1 AND kind = 'endgame_conversion' AND mastered = false
             ORDER BY due_at ASC LIMIT 1",
        )
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
    };

    let (puzzle_id, game_id, fen) = puzzle.ok_or_else(|| {
        AppError::bad_request(
            "No endgame drill is available yet. Analyse a game with a thrown winning endgame first.",
        )
    })?;

    let user_elo: Option<i32> = sqlx::query_scalar("SELECT current_elo FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .flatten();

    sqlx::query(
        "UPDATE sparring_sessions SET status = 'abandoned', result = 'abandoned'
         WHERE user_id = $1 AND status = 'active'",
    )
    .bind(auth.id)
    .execute(&state.db)
    .await?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO sparring_sessions
            (user_id, game_id, puzzle_id, fen, initial_fen, moves, status, user_elo)
         VALUES ($1,$2,$3,$4,$4,'[]'::jsonb,'active',$5) RETURNING id",
    )
    .bind(auth.id)
    .bind(game_id)
    .bind(puzzle_id)
    .bind(&fen)
    .bind(user_elo)
    .fetch_one(&state.db)
    .await?;

    let sql = format!("{SESSION_SELECT} WHERE s.id = $1 AND s.user_id = $2");
    let row = sqlx::query_as::<_, SessionRow>(&sql)
        .bind(id)
        .bind(auth.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(row.into_dto()?))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<SparringSessionDto>> {
    let sql = format!("{SESSION_SELECT} WHERE s.id = $1 AND s.user_id = $2");
    let row = sqlx::query_as::<_, SessionRow>(&sql)
        .bind(session_id)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Sparring session not found."))?;
    Ok(Json(row.into_dto()?))
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<SparringSessionDto>>> {
    let sql = format!("{SESSION_SELECT} WHERE s.user_id = $1 ORDER BY s.created_at DESC LIMIT 20");
    let rows = sqlx::query_as::<_, SessionRow>(&sql)
        .bind(auth.id)
        .fetch_all(&state.db)
        .await?;
    let items = rows
        .into_iter()
        .map(SessionRow::into_dto)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Json(items))
}

pub async fn hint(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<SparringHint>> {
    let sql = format!("{SESSION_SELECT} WHERE s.id = $1 AND s.user_id = $2");
    let row = sqlx::query_as::<_, SessionRow>(&sql)
        .bind(session_id)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Sparring session not found."))?;
    if row.status != "active" {
        return Err(AppError::bad_request("This drill is already finished."));
    }
    let dto = row.clone().into_dto()?;
    let user_white = dto.user_color == "white";
    let turn_white = row
        .fen
        .split_whitespace()
        .nth(1)
        .map(|turn| turn == "w")
        .unwrap_or(true);
    if turn_white != user_white {
        return Err(AppError::bad_request("Wait for the engine to move first."));
    }

    let eval = state
        .engine
        .evaluate(EngineQuery {
            fen: row.fen.clone(),
            depth: 12,
            multipv: 1,
            uci_elo: None,
            movetime_ms: Some(400),
        })
        .await
        .map_err(|err| AppError::Engine(format!("could not generate a hint: {err}")))?;
    let best = eval
        .best_move
        .clone()
        .ok_or_else(|| AppError::Engine("engine returned no hint".into()))?;
    let san = apply_uci(&row.fen, &best)
        .ok()
        .map(|played| played.san.clone());
    Ok(Json(SparringHint {
        uci: best,
        san,
        message: "A strong candidate is highlighted. Use it as a plan, then convert the drill."
            .into(),
    }))
}

pub async fn make_move(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<SparringMoveRequest>,
) -> AppResult<Json<SparringMoveResponse>> {
    let sql = format!("{SESSION_SELECT} WHERE s.id = $1 AND s.user_id = $2");
    let mut row = sqlx::query_as::<_, SessionRow>(&sql)
        .bind(session_id)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Sparring session not found."))?;

    if row.status != "active" {
        return Err(AppError::bad_request(
            "This sparring session is already finished.",
        ));
    }

    let dto = row.clone().into_dto()?;
    let user_color = dto.user_color.clone();
    let user_white = user_color == "white";
    let current_turn_white = row
        .fen
        .split_whitespace()
        .nth(1)
        .map(|turn| turn == "w")
        .unwrap_or(true);
    if current_turn_white != user_white {
        return Err(AppError::bad_request(
            "It is the engine's turn in this drill.",
        ));
    }

    let user_played = apply_uci(&row.fen, &payload.uci)?;
    let mut moves = dto.moves.clone();
    moves.push(SparringMoveDto {
        move_number: (moves.len() as i32) / 2 + 1,
        color: user_color.clone(),
        san: user_played.san.clone(),
        uci: user_played.uci.clone(),
        fen_after: user_played.fen_after.clone(),
    });
    row.fen = user_played.fen_after.clone();
    let user_move = moves
        .last()
        .cloned()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("missing user move")))?;

    let mut status = "active".to_string();
    let mut message = String::new();

    if user_played.checkmate {
        status = "won".into();
        message = "Checkmate. The drill is converted.".into();
    } else if user_played.stalemate || user_played.insufficient {
        status = "draw".into();
        message = "The position is drawn. The drill was not converted.".into();
    } else if user_played.decisive_promotion {
        let eval = state
            .engine
            .evaluate(EngineQuery {
                fen: row.fen.clone(),
                depth: 10,
                multipv: 1,
                uci_elo: None,
                movetime_ms: Some(500),
            })
            .await
            .ok();
        let user_advantage = eval
            .as_ref()
            .and_then(|eval| eval.top())
            .and_then(|line| line.score_cp)
            .map(|cp| -f64::from(cp) / 100.0)
            .unwrap_or(0.0);
        if user_advantage >= 2.0 {
            status = "won".into();
            message = "Decisive promotion. The drill is converted.".into();
        }
    }

    let mut engine_move = None;
    if status == "active" {
        let enemy_elo = dto.enemy_elo.unwrap_or(1320);
        let engine_eval = state
            .engine
            .evaluate(
                EngineQuery::new(row.fen.clone(), 10, 1)
                    .with_elo(enemy_elo as u32)
                    .with_movetime(500),
            )
            .await
            .map_err(|e| AppError::Engine(format!("engine reply failed: {e}")))?;
        let best = engine_eval
            .best_move
            .clone()
            .ok_or_else(|| AppError::Engine("engine returned no move".into()))?;
        let engine_played = apply_uci(&row.fen, &best)?;
        let engine_color = if user_white { "black" } else { "white" };
        let engine_dto = SparringMoveDto {
            move_number: (moves.len() as i32) / 2 + 1,
            color: engine_color.to_string(),
            san: engine_played.san.clone(),
            uci: engine_played.uci.clone(),
            fen_after: engine_played.fen_after.clone(),
        };
        moves.push(engine_dto.clone());
        row.fen = engine_played.fen_after.clone();
        engine_move = Some(engine_dto);

        if engine_played.checkmate {
            status = "lost".into();
            message = "The engine converted first. Replay the drill.".into();
        } else if engine_played.stalemate || engine_played.insufficient {
            status = "draw".into();
            message = "The drill ended in a draw.".into();
        } else {
            message = "Your move. Keep the winning plan clean.".into();
        }
    }

    row.moves = serde_json::to_value(&moves)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to encode moves: {e}")))?;
    row.status = status.clone();
    row.result = Some(status.clone());

    sqlx::query(
        "UPDATE sparring_sessions SET fen = $2, moves = $3, status = $4, result = $5
         WHERE id = $1",
    )
    .bind(row.id)
    .bind(&row.fen)
    .bind(&row.moves)
    .bind(&status)
    .bind(&status)
    .execute(&state.db)
    .await?;

    if status == "won" {
        if let Some(puzzle_id) = row.puzzle_id {
            let _ = sqlx::query(
                "UPDATE puzzles SET mastered = true, stability = GREATEST(stability, 30.0),
                        state = 'review', due_at = now() + interval '30 days',
                        reps = reps + 1, last_reviewed_at = now()
                 WHERE id = $1 AND user_id = $2",
            )
            .bind(puzzle_id)
            .bind(auth.id)
            .execute(&state.db)
            .await;
        }
    }

    let sql = format!("{SESSION_SELECT} WHERE s.id = $1 AND s.user_id = $2");
    let updated = sqlx::query_as::<_, SessionRow>(&sql)
        .bind(row.id)
        .bind(auth.id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(SparringMoveResponse {
        session: updated.into_dto()?,
        user_move,
        engine_move,
        status,
        message,
    }))
}

struct AppliedMove {
    san: String,
    uci: String,
    fen_after: String,
    checkmate: bool,
    stalemate: bool,
    insufficient: bool,
    decisive_promotion: bool,
}

fn apply_uci(fen: &str, uci: &str) -> AppResult<AppliedMove> {
    let pos = Fen::from_ascii(fen.as_bytes())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid stored FEN")))?
        .into_position::<Chess>(CastlingMode::Standard)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("illegal stored FEN")))?;
    let parsed: UciMove = uci
        .parse()
        .map_err(|_| AppError::bad_request("That is not a valid move."))?;
    let mv = parsed
        .to_move(&pos)
        .map_err(|_| AppError::bad_request("That move is not legal in this position."))?;
    let san = SanPlus::from_move(pos.clone(), mv).to_string();
    let decisive_promotion = matches!(
        mv,
        Move::Normal {
            promotion: Some(Role::Queen | Role::Rook),
            ..
        }
    );
    let after = pos
        .play(mv)
        .map_err(|_| AppError::bad_request("That move is not legal in this position."))?;
    let fen_after = Fen::from_position(&after, EnPassantMode::Legal).to_string();
    Ok(AppliedMove {
        san,
        uci: uci.to_ascii_lowercase(),
        fen_after,
        checkmate: after.is_checkmate(),
        stalemate: after.is_stalemate(),
        insufficient: after.is_insufficient_material(),
        decisive_promotion,
    })
}
