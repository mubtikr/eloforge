use std::sync::Arc;

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use shakmaty::{fen::Fen, san::SanPlus, uci::UciMove, CastlingMode, Chess};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    analyzer::{
        mistake_tagger::{self, MoveContext},
        winprob::{
            accuracy_from_epl, classify, is_bad, phase_for_ply, win_prob_to_pawns, EvalSnapshot,
        },
    },
    chess::{
        openings::{detect_opening, find_violations},
        pgn_importer::ImportedMove,
    },
    config::Config,
    engine::{EngineEval, EnginePool, EngineQuery},
    error::{AppError, AppResult},
    events::{AppEvent, EventBus},
    metrics::Metrics,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisOutcome {
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub accuracy_user: f64,
    pub blunders: usize,
    pub mistakes: usize,
    pub puzzles_created: usize,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct GameRow {
    id: Uuid,
    user_id: Uuid,
    user_color: String,
    initial_fen: String,
    time_class: Option<String>,
    time_control: Option<String>,
    result: String,
    result_reason: Option<String>,
    analysis_status: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct MoveRow {
    id: i64,
    ply: i32,
    move_number: i32,
    color: String,
    san: String,
    uci: String,
    fen_before: String,
    fen_after: String,
    clock_remaining_sec: Option<f64>,
    time_spent_sec: Option<f64>,
}

struct EvaluatedMove {
    row: MoveRow,
    eval_before_cp: Option<i32>,
    eval_after_cp: Option<i32>,
    eval_mate: Option<i32>,
    win_prob_user: f64,
    best_move_uci: Option<String>,
    best_move_san: Option<String>,
    epl: f64,
    classification: String,
    tags: Vec<String>,
    pv: serde_json::Value,
    phase: String,
    accuracy: f64,
}

pub async fn analyze_game(
    db: &PgPool,
    engine: &Arc<EnginePool>,
    metrics: &Arc<Metrics>,
    events: &Arc<EventBus>,
    cfg: &Config,
    game_id: Uuid,
    requested_by: Uuid,
) -> AppResult<AnalysisOutcome> {
    let game = sqlx::query_as::<_, GameRow>(
        "SELECT id, user_id, user_color, initial_fen, time_class, time_control,
                result, result_reason, analysis_status
         FROM games WHERE id = $1 AND user_id = $2",
    )
    .bind(game_id)
    .bind(requested_by)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::not_found("Game not found."))?;

    if game.analysis_status == "done" {
        let accuracy: Option<f64> =
            sqlx::query_scalar("SELECT accuracy_user FROM games WHERE id = $1")
                .bind(game_id)
                .fetch_one(db)
                .await?;
        return Ok(AnalysisOutcome {
            game_id,
            user_id: game.user_id,
            accuracy_user: accuracy.unwrap_or(0.0),
            blunders: 0,
            mistakes: 0,
            puzzles_created: 0,
        });
    }

    sqlx::query(
        "UPDATE games SET analysis_status = 'analyzing', analysis_error = NULL WHERE id = $1",
    )
    .bind(game_id)
    .execute(db)
    .await?;

    let result = analyze_inner(db, engine, metrics, events, cfg, &game).await;

    match result {
        Ok(outcome) => Ok(outcome),
        Err(err) => {
            let message = err.to_string();
            sqlx::query(
                "UPDATE games SET analysis_status = 'failed', analysis_error = $2 WHERE id = $1",
            )
            .bind(game_id)
            .bind(&message)
            .execute(db)
            .await?;
            metrics.analysis_failures.inc();
            events
                .publish(AppEvent::new(
                    game.user_id,
                    "analysis_error",
                    "Analysis stopped before finishing.",
                    None,
                    serde_json::json!({ "game_id": game_id, "error": message }),
                ))
                .await;
            Err(err)
        }
    }
}

async fn analyze_inner(
    db: &PgPool,
    engine: &Arc<EnginePool>,
    metrics: &Arc<Metrics>,
    events: &Arc<EventBus>,
    cfg: &Config,
    game: &GameRow,
) -> AppResult<AnalysisOutcome> {
    let started = std::time::Instant::now();
    let moves = sqlx::query_as::<_, MoveRow>(
        "SELECT id, ply, move_number, color, san, uci, fen_before, fen_after,
                clock_remaining_sec, time_spent_sec
         FROM game_moves WHERE game_id = $1 ORDER BY ply",
    )
    .bind(game.id)
    .fetch_all(db)
    .await?;

    if moves.is_empty() {
        return Err(AppError::bad_request("This game has no moves to analyze."));
    }

    let user_white = game.user_color == "white";
    let time_class = game.time_class.as_deref().unwrap_or("rapid");
    let initial_time = parse_initial_time(game.time_control.as_deref());
    let san_moves: Vec<String> = moves.iter().map(|m| m.san.clone()).collect();
    let opening = detect_opening(&san_moves);

    let imported: Vec<ImportedMove> = moves
        .iter()
        .map(|m| ImportedMove {
            ply: m.ply as u32,
            move_number: m.move_number as u32,
            color: m.color.clone(),
            san: m.san.clone(),
            uci: m.uci.clone(),
            fen_before: m.fen_before.clone(),
            fen_after: m.fen_after.clone(),
            clock_remaining_sec: m.clock_remaining_sec,
            time_spent_sec: m.time_spent_sec,
        })
        .collect();
    let violations = find_violations(&imported, &game.user_color);
    let violation_plies: Vec<i32> = violations.iter().map(|v| v.ply as i32).collect();

    let opening_end = opening
        .as_ref()
        .map(|o| o.last_book_ply as i32)
        .unwrap_or(10)
        .clamp(10, 30);
    let endgame_start = compute_endgame_start(&moves, opening_end);

    let mut positions: Vec<String> = Vec::with_capacity(moves.len() + 1);
    positions.push(if game.initial_fen.trim().is_empty() {
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
    } else {
        game.initial_fen.clone()
    });
    positions.extend(moves.iter().map(|m| m.fen_after.clone()));

    let white_to_move: Vec<bool> = positions.iter().map(|fen| fen_turn_is_white(fen)).collect();

    let workers = engine.worker_count().clamp(1, 8);
    events
        .publish(AppEvent::new(
            game.user_id,
            "analysis_progress",
            format!("Evaluating {} positions.", positions.len()),
            Some(10),
            serde_json::json!({ "game_id": game.id }),
        ))
        .await;

    let full_depth = cfg.analysis_depth_full;
    let full_movetime =
        (cfg.analysis_movetime_full_ms > 0).then_some(cfg.analysis_movetime_full_ms);
    let critical_depth = cfg.analysis_depth_critical;
    let critical_movetime =
        (cfg.analysis_movetime_critical_ms > 0).then_some(cfg.analysis_movetime_critical_ms);
    let mut snapshots = evaluate_positions(
        engine,
        &positions,
        &white_to_move,
        full_depth,
        2,
        workers,
        full_movetime,
    )
    .await?;

    let first_pass: Vec<(usize, f64)> = moves
        .iter()
        .enumerate()
        .filter_map(|(index, mv)| {
            if mv.color != game.user_color {
                return None;
            }
            let mover_white = mv.color == "white";
            let before = snapshots[index].win_for(mover_white);
            let after = snapshots[index + 1].win_for(mover_white);
            let epl = ((before - after) / 100.0).max(0.0);
            (epl >= 0.10).then_some((index, epl))
        })
        .collect();

    if !first_pass.is_empty() {
        events
            .publish(AppEvent::new(
                game.user_id,
                "analysis_progress",
                format!(
                    "Re-checking {} turning point(s) at depth 20.",
                    first_pass.len()
                ),
                Some(45),
                serde_json::json!({ "game_id": game.id }),
            ))
            .await;
    }

    let mut critical_indices = Vec::new();
    for (index, _) in &first_pass {
        critical_indices.push(*index);
    }
    critical_indices.sort_unstable();
    critical_indices.dedup();

    if !critical_indices.is_empty() {
        let critical_fens: Vec<String> = critical_indices
            .iter()
            .map(|index| positions[*index].clone())
            .collect();
        let critical_turns: Vec<bool> = critical_indices
            .iter()
            .map(|index| white_to_move[*index])
            .collect();
        let refined = evaluate_positions(
            engine,
            &critical_fens,
            &critical_turns,
            critical_depth,
            3,
            workers,
            critical_movetime,
        )
        .await?;
        for (slot, index) in critical_indices.iter().enumerate() {
            if let Some(snapshot) = refined.get(slot) {
                snapshots[*index] = snapshot.clone();
            }
        }
    }

    let mut evaluated: Vec<EvaluatedMove> = Vec::with_capacity(moves.len());
    for (index, mv) in moves.iter().enumerate() {
        let mover_white = mv.color == "white";
        let before = &snapshots[index];
        let after = &snapshots[index + 1];
        let win_before = before.win_for(mover_white);
        let win_after = after.win_for(mover_white);
        let best_move_uci = before.best_uci.clone();
        let played_is_best = best_move_uci.as_deref() == Some(mv.uci.as_str());
        let raw_epl = ((win_before - win_after) / 100.0).max(0.0);
        let epl = if played_is_best { 0.0 } else { raw_epl };
        let classification = if played_is_best {
            "best"
        } else {
            classify(epl, win_before, win_after)
        };
        let accuracy = accuracy_from_epl(epl);
        let phase = phase_for_ply(mv.ply, opening_end, endgame_start).to_string();
        let best_move_san = best_move_uci
            .as_deref()
            .and_then(|uci| uci_to_san(&mv.fen_before, uci));
        let best_pv = before
            .lines
            .first()
            .map(|line| line.pv.clone())
            .unwrap_or_default();
        let is_user = mv.color == game.user_color;
        let opening_break = is_user && violation_plies.contains(&mv.ply);

        let mut tags = if is_user {
            mistake_tagger::tag_move(&MoveContext {
                fen_before: &mv.fen_before,
                fen_after: &mv.fen_after,
                played_uci: &mv.uci,
                best_uci: best_move_uci.as_deref(),
                best_pv: &best_pv,
                before,
                after,
                mover_white,
                epl,
                classification,
                phase: &phase,
                time_spent_sec: mv.time_spent_sec,
                clock_remaining_sec: mv.clock_remaining_sec,
                initial_time_sec: initial_time,
                time_class,
                opening_break,
            })
        } else {
            Vec::new()
        };

        if is_user
            && opening_break
            && is_bad(classification)
            && !tags.iter().any(|t| t == "OPENING_PRINCIPLE_BREAK")
        {
            tags.push("OPENING_PRINCIPLE_BREAK".to_string());
        }

        let pv = serde_json::to_value(before.lines.iter().take(3).collect::<Vec<_>>())
            .unwrap_or(serde_json::Value::Null);

        evaluated.push(EvaluatedMove {
            row: mv.clone(),
            eval_before_cp: before.cp_for(user_white),
            eval_after_cp: after.cp_for(user_white),
            eval_mate: before.mate_for(user_white),
            win_prob_user: after.win_for(user_white),
            best_move_uci,
            best_move_san,
            epl,
            classification: classification.to_string(),
            tags,
            pv,
            phase,
            accuracy,
        });
    }

    let max_advantage_user = snapshots
        .iter()
        .map(|snapshot| {
            if snapshot.mate_for(user_white).is_some_and(|m| m > 0) {
                20.0
            } else {
                f64::from(snapshot.cp_for(user_white).unwrap_or(0)) / 100.0
            }
        })
        .fold(0.0f64, f64::max)
        .max(0.0);

    let peak_win_prob_user = snapshots
        .iter()
        .map(|snapshot| snapshot.win_for(user_white))
        .fold(0.0f64, f64::max);

    let game_tag_list = mistake_tagger::game_tags(
        &game.result,
        game.result_reason.as_deref().unwrap_or(""),
        max_advantage_user,
    );

    apply_game_tags(&mut evaluated, &game_tag_list, user_white);

    let user_moves: Vec<&EvaluatedMove> = evaluated
        .iter()
        .filter(|m| m.row.color == game.user_color)
        .collect();
    let accuracy_user = mean(user_moves.iter().map(|m| m.accuracy));
    let accuracy_white = mean(
        evaluated
            .iter()
            .filter(|m| m.row.color == "white")
            .map(|m| m.accuracy),
    );
    let accuracy_black = mean(
        evaluated
            .iter()
            .filter(|m| m.row.color == "black")
            .map(|m| m.accuracy),
    );
    let accuracy_opening = mean(
        user_moves
            .iter()
            .filter(|m| m.phase == "opening")
            .map(|m| m.accuracy),
    );
    let accuracy_middlegame = mean(
        user_moves
            .iter()
            .filter(|m| m.phase == "middlegame")
            .map(|m| m.accuracy),
    );
    let accuracy_endgame = mean(
        user_moves
            .iter()
            .filter(|m| m.phase == "endgame")
            .map(|m| m.accuracy),
    );

    let blunders = user_moves
        .iter()
        .filter(|m| matches!(m.classification.as_str(), "blunder" | "missed_win"))
        .count();
    let mistakes = user_moves
        .iter()
        .filter(|m| m.classification == "mistake")
        .count();

    let turning_points: Vec<serde_json::Value> = user_moves
        .iter()
        .filter(|m| m.epl >= 0.10)
        .map(|m| {
            serde_json::json!({
                "ply": m.row.ply,
                "move_number": m.row.move_number,
                "san": m.row.san,
                "uci": m.row.uci,
                "classification": m.classification,
                "epl": m.epl,
                "eval_before_cp": m.eval_before_cp,
                "eval_after_cp": m.eval_after_cp,
                "best_move_uci": m.best_move_uci,
                "best_move_san": m.best_move_san,
                "tags": m.tags,
            })
        })
        .collect();

    let opening_json = opening.as_ref().map(|o| {
        serde_json::json!({
            "eco": o.eco,
            "name": o.name,
            "variation": o.variation,
            "last_book_ply": o.last_book_ply,
        })
    });

    let analysis = serde_json::json!({
        "version": 1,
        "engine": "Stockfish 17",
        "depth_full": cfg.analysis_depth_full,
        "depth_critical": cfg.analysis_depth_critical,
        "opening": opening_json,
        "violations": violations,
        "phases": {
            "opening_end_ply": opening_end,
            "endgame_start_ply": endgame_start,
        },
        "accuracy": {
            "user": accuracy_user,
            "white": accuracy_white,
            "black": accuracy_black,
            "opening": accuracy_opening,
            "middlegame": accuracy_middlegame,
            "endgame": accuracy_endgame,
        },
        "game_tags": game_tag_list,
        "turning_points": turning_points,
        "max_advantage_user": max_advantage_user,
        "peak_win_prob_user": peak_win_prob_user,
        "peak_win_prob_pawns": win_prob_to_pawns(peak_win_prob_user),
    });

    let mut tx = db.begin().await?;
    for mv in &evaluated {
        sqlx::query(
            "UPDATE game_moves SET eval_before_cp = $2, eval_after_cp = $3, eval_cp = $3,
                    eval_mate = $4, win_prob_user = $5, best_move_uci = $6,
                    best_move_san = $7, epl = $8, classification = $9,
                    mistake_tags = $10, pv = $11
             WHERE id = $1",
        )
        .bind(mv.row.id)
        .bind(mv.eval_before_cp)
        .bind(mv.eval_after_cp)
        .bind(mv.eval_mate)
        .bind(mv.win_prob_user)
        .bind(mv.best_move_uci.as_deref())
        .bind(mv.best_move_san.as_deref())
        .bind(mv.epl)
        .bind(&mv.classification)
        .bind(&mv.tags)
        .bind(&mv.pv)
        .execute(&mut *tx)
        .await?;
    }

    let (eco, name, last_book_ply): (Option<String>, Option<String>, Option<i32>) = match &opening {
        Some(opening) => (
            Some(opening.eco.clone()),
            Some(opening.name.clone()),
            Some(opening.last_book_ply as i32),
        ),
        None => (None, None, None),
    };

    sqlx::query(
        "UPDATE games SET opening_eco = COALESCE($2, opening_eco),
                opening_name = COALESCE($3, opening_name),
                opening_variation = $4,
                last_book_ply = COALESCE($5, last_book_ply),
                accuracy_user = $6, accuracy_white = $7, accuracy_black = $8,
                eval_peak = $9, max_advantage = $10,
                analysis = $11, analyzed_at = now(), analysis_status = 'done',
                analysis_error = NULL
         WHERE id = $1",
    )
    .bind(game.id)
    .bind(eco.as_deref())
    .bind(name.as_deref())
    .bind(opening.as_ref().and_then(|o| o.variation.as_deref()))
    .bind(last_book_ply)
    .bind(accuracy_user)
    .bind(accuracy_white)
    .bind(accuracy_black)
    .bind(peak_win_prob_user)
    .bind(max_advantage_user)
    .bind(&analysis)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    metrics.analysis_jobs.inc();
    metrics
        .analysis_latency
        .observe(started.elapsed().as_secs_f64());

    Ok(AnalysisOutcome {
        game_id: game.id,
        user_id: game.user_id,
        accuracy_user,
        blunders,
        mistakes,
        puzzles_created: 0,
    })
}

fn apply_game_tags(evaluated: &mut [EvaluatedMove], game_tags: &[String], user_white: bool) {
    if game_tags.is_empty() {
        return;
    }
    let worst_user = evaluated
        .iter()
        .enumerate()
        .filter(|(_, m)| (m.row.color == "white") == user_white)
        .max_by(|(_, a), (_, b)| a.epl.total_cmp(&b.epl))
        .map(|(index, _)| index);
    let last_user = evaluated
        .iter()
        .enumerate()
        .filter(|(_, m)| (m.row.color == "white") == user_white)
        .map(|(index, _)| index)
        .next_back();

    for tag in game_tags {
        let target = match tag.as_str() {
            "STALEMATE_THROW" => last_user,
            _ => worst_user,
        };
        if let Some(index) = target {
            if !evaluated[index].tags.iter().any(|existing| existing == tag) {
                evaluated[index].tags.push(tag.clone());
            }
        }
    }
}

async fn evaluate_positions(
    engine: &Arc<EnginePool>,
    fens: &[String],
    turns_white: &[bool],
    depth: u8,
    multipv: usize,
    concurrency: usize,
    movetime_ms: Option<u64>,
) -> AppResult<Vec<EvalSnapshot>> {
    let results: Vec<(usize, AppResult<EngineEval>)> =
        stream::iter(fens.iter().cloned().enumerate())
            .map(|(index, fen)| {
                let engine = engine.clone();
                async move {
                    let result = evaluate_one(&engine, &fen, depth, multipv, movetime_ms).await;
                    (index, result)
                }
            })
            .buffer_unordered(concurrency.max(1))
            .collect()
            .await;

    let mut ordered = results
        .into_iter()
        .map(|(index, result)| result.map(|eval| (index, eval)))
        .collect::<AppResult<Vec<_>>>()?;
    ordered.sort_by_key(|(index, _)| *index);

    Ok(ordered
        .into_iter()
        .map(|(index, eval)| {
            let white_to_move = turns_white.get(index).copied().unwrap_or(true);
            EvalSnapshot::from_eval(&eval, white_to_move)
        })
        .collect())
}

async fn evaluate_one(
    engine: &EnginePool,
    fen: &str,
    depth: u8,
    multipv: usize,
    movetime_ms: Option<u64>,
) -> AppResult<EngineEval> {
    let query = EngineQuery {
        fen: fen.to_string(),
        depth,
        multipv,
        uci_elo: None,
        movetime_ms,
    };
    match engine.evaluate(query).await {
        Ok(eval) => Ok(eval),
        Err(err) => {
            tracing::warn!(error = %err, depth, "engine query failed; retrying shallow");
            let fallback = EngineQuery {
                fen: fen.to_string(),
                depth: depth.min(10),
                multipv: multipv.min(2),
                uci_elo: None,
                movetime_ms: Some(900),
            };
            engine.evaluate(fallback).await.map_err(|err| {
                AppError::Engine(format!("Stockfish could not evaluate position: {err}"))
            })
        }
    }
}

fn compute_endgame_start(moves: &[MoveRow], opening_end: i32) -> i32 {
    for mv in moves {
        if crate::analyzer::winprob::is_endgame_fen(&mv.fen_after) {
            return mv.ply.max(opening_end + 1);
        }
    }
    i32::MAX / 2
}

fn parse_initial_time(time_control: Option<&str>) -> Option<f64> {
    let raw = time_control?;
    let base = raw.split('+').next()?.trim();
    base.parse::<f64>().ok().filter(|v| *v > 0.0)
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let mut total = 0.0;
    let mut count = 0usize;
    for value in values {
        total += value;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

fn fen_turn_is_white(fen: &str) -> bool {
    let mut parts = fen.split_whitespace();
    let _board = parts.next();
    parts.next().map(|turn| turn == "w").unwrap_or(true)
}

fn uci_to_san(fen: &str, uci: &str) -> Option<String> {
    let pos = Fen::from_ascii(fen.as_bytes())
        .ok()?
        .into_position::<Chess>(CastlingMode::Standard)
        .ok()?;
    let parsed: UciMove = uci.parse().ok()?;
    let mv = parsed.to_move(&pos).ok()?;
    Some(SanPlus::from_move(pos, mv).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_time_controls() {
        assert_eq!(parse_initial_time(Some("600")), Some(600.0));
        assert_eq!(parse_initial_time(Some("600+5")), Some(600.0));
        assert_eq!(parse_initial_time(Some("1/86400")), None);
        assert_eq!(parse_initial_time(None), None);
    }

    #[test]
    fn converts_uci_to_san() {
        let san = uci_to_san(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "e2e4",
        )
        .expect("legal move");
        assert_eq!(san, "e4");
    }

    #[test]
    fn detects_fen_turn() {
        assert!(fen_turn_is_white(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        ));
        assert!(!fen_turn_is_white("8/8/8/8/8/8/8/K6k b - - 0 1"));
    }
}
