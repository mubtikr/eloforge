use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, Chess, Move, Position, Role};

use crate::analyzer::winprob::EvalSnapshot;

#[derive(Debug, Clone, Copy, Default)]
pub struct MoveFeature {
    pub is_capture: bool,
    pub captured_value: i32,
    pub is_check: bool,
    pub is_mate: bool,
}

pub fn move_features(fen: &str, uci: &str) -> Option<MoveFeature> {
    let pos = Fen::from_ascii(fen.as_bytes())
        .ok()?
        .into_position::<Chess>(CastlingMode::Standard)
        .ok()?;
    let parsed: UciMove = uci.parse().ok()?;
    let mv = parsed.to_move(&pos).ok()?;

    let captured_value = match mv {
        Move::Normal { capture, .. } => capture.map(role_value).unwrap_or(0),
        Move::EnPassant { .. } => 100,
        _ => 0,
    };

    let after = pos.clone().play(mv).ok()?;
    Some(MoveFeature {
        is_capture: captured_value > 0,
        captured_value,
        is_check: after.is_check(),
        is_mate: after.is_checkmate(),
    })
}

fn role_value(role: Role) -> i32 {
    match role {
        Role::Pawn => 100,
        Role::Knight => 300,
        Role::Bishop => 310,
        Role::Rook => 500,
        Role::Queen => 900,
        Role::King => 0,
    }
}

fn pv_has_tactic(fen: &str, pv: &[String]) -> bool {
    let Ok(fen) = Fen::from_ascii(fen.as_bytes()) else {
        return false;
    };
    let Ok(mut pos) = fen.into_position::<Chess>(CastlingMode::Standard) else {
        return false;
    };

    for uci in pv.iter().take(4) {
        let Ok(parsed) = uci.parse::<UciMove>() else {
            break;
        };
        let Ok(mv) = parsed.to_move(&pos) else {
            break;
        };
        let capture = match mv {
            Move::Normal { capture, .. } => capture.is_some(),
            Move::EnPassant { .. } => true,
            _ => false,
        };
        let Ok(next) = pos.clone().play(mv) else {
            break;
        };
        if capture || next.is_check() {
            return true;
        }
        pos = next;
    }
    false
}

pub struct MoveContext<'a> {
    pub fen_before: &'a str,
    pub fen_after: &'a str,
    pub played_uci: &'a str,
    pub best_uci: Option<&'a str>,
    pub best_pv: &'a [String],
    pub before: &'a EvalSnapshot,
    pub after: &'a EvalSnapshot,
    pub mover_white: bool,
    pub epl: f64,
    pub classification: &'a str,
    pub phase: &'a str,
    pub time_spent_sec: Option<f64>,
    pub clock_remaining_sec: Option<f64>,
    pub initial_time_sec: Option<f64>,
    pub time_class: &'a str,
    pub opening_break: bool,
}

pub fn tag_move(ctx: &MoveContext<'_>) -> Vec<String> {
    let mut tags = Vec::new();
    if !matches!(
        ctx.classification,
        "inaccuracy" | "mistake" | "blunder" | "missed_win"
    ) {
        return tags;
    }

    let win_before = ctx.before.win_for(ctx.mover_white);
    let win_after = ctx.after.win_for(ctx.mover_white);
    let swing = win_before - win_after;

    if let Some(best_after) = ctx.after.best_uci.as_deref() {
        if let Some(feature) = move_features(ctx.fen_after, best_after) {
            if feature.is_capture && feature.captured_value >= 300 && swing >= 12.0 {
                tags.push("HANGING_PIECE".to_string());
            }
        }
    }

    if let Some(best_before) = ctx.best_uci {
        if best_before != ctx.played_uci {
            if let Some(feature) = move_features(ctx.fen_before, best_before) {
                if feature.is_capture
                    && feature.captured_value >= 300
                    && ctx.before.cp_for(ctx.mover_white).unwrap_or(0) >= 100
                {
                    tags.push("MISSED_FREE_PIECE".to_string());
                }
            }
        }
    }

    let mate_before = ctx.before.mate_for(ctx.mover_white);
    if let Some(mate) = mate_before {
        if (1..=2).contains(&mate) && ctx.best_uci != Some(ctx.played_uci) {
            tags.push("MISSED_MATE_1_2".to_string());
        }
    }

    if let Some(mate) = ctx.after.mate_for(!ctx.mover_white) {
        if (1..=2).contains(&mate) {
            tags.push("ALLOWED_MATE".to_string());
        }
    }

    let tactical = ctx
        .best_uci
        .and_then(|best| move_features(ctx.fen_before, best))
        .map(|feature| feature.is_capture || feature.is_check)
        .unwrap_or(false)
        || pv_has_tactic(ctx.fen_before, ctx.best_pv);
    if tactical && ctx.epl >= 0.10 {
        tags.push("TACTICAL_BLINDNESS".to_string());
    }

    if ctx.phase == "endgame" && ctx.epl >= 0.15 && win_before >= 65.0 {
        tags.push("ENDGAME_TECHNIQUE_FAIL".to_string());
    }

    if ctx.time_class.eq_ignore_ascii_case("rapid")
        && ctx.time_spent_sec.is_some_and(|seconds| seconds < 4.0)
        && ctx.epl >= 0.12
    {
        let fraction = match (ctx.clock_remaining_sec, ctx.initial_time_sec) {
            (Some(remaining), Some(initial)) if initial > 0.0 => remaining / initial,
            _ => 1.0,
        };
        if fraction > 0.5 {
            tags.push("IMPULSIVE_BLUNDER".to_string());
        }
    }

    if ctx
        .clock_remaining_sec
        .is_some_and(|remaining| remaining < 30.0)
        && ctx.epl >= 0.12
    {
        tags.push("TIME_SCRAMBLE_BLUNDER".to_string());
    }

    if ctx.opening_break && ctx.epl >= 0.06 {
        tags.push("OPENING_PRINCIPLE_BREAK".to_string());
    }

    tags.sort();
    tags.dedup();
    tags
}

pub fn game_tags(
    result_for_user: &str,
    result_reason: &str,
    max_advantage_user: f64,
) -> Vec<String> {
    let mut tags = Vec::new();
    if max_advantage_user >= 4.0 && result_for_user != "win" {
        tags.push("WINNING_GAME_THROWN".to_string());
    }
    if result_for_user == "draw"
        && result_reason.to_ascii_lowercase().contains("stalemate")
        && max_advantage_user >= 4.0
    {
        tags.push("STALEMATE_THROW".to_string());
    }
    tags
}

pub fn is_puzzle_blunder(epl: f64) -> bool {
    epl >= 0.15
}

#[cfg(test)]
mod tests {
    use super::*;

    const BACK_RANK: &str = "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1";

    #[test]
    fn identifies_captures() {
        let fen = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
        let feature = move_features(fen, "e4d5").expect("capture should parse");
        assert!(feature.is_capture);
        assert_eq!(feature.captured_value, 100);
    }

    #[test]
    fn identifies_back_rank_checkmate() {
        let feature = move_features(BACK_RANK, "d1d8").expect("mate should parse");
        assert!(feature.is_mate);
        assert!(feature.is_check);
    }

    #[test]
    fn no_tags_for_best_move() {
        let before = crate::engine::EngineEval {
            best_move: Some("e2e4".into()),
            depth: 14,
            lines: vec![],
        };
        let snapshot = EvalSnapshot::from_eval(&before, true);
        let ctx = MoveContext {
            fen_before: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            fen_after: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
            played_uci: "e2e4",
            best_uci: Some("e2e4"),
            best_pv: &[],
            before: &snapshot,
            after: &snapshot,
            mover_white: true,
            epl: 0.0,
            classification: "best",
            phase: "opening",
            time_spent_sec: Some(5.0),
            clock_remaining_sec: Some(590.0),
            initial_time_sec: Some(600.0),
            time_class: "rapid",
            opening_break: false,
        };
        assert!(tag_move(&ctx).is_empty());
    }

    #[test]
    fn flags_impulsive_rapid_blunder() {
        let empty = crate::engine::EngineEval {
            best_move: None,
            depth: 14,
            lines: vec![],
        };
        let snapshot = EvalSnapshot::from_eval(&empty, true);
        let ctx = MoveContext {
            fen_before: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            fen_after: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
            played_uci: "e2e4",
            best_uci: Some("d2d4"),
            best_pv: &[],
            before: &snapshot,
            after: &snapshot,
            mover_white: true,
            epl: 0.2,
            classification: "blunder",
            phase: "opening",
            time_spent_sec: Some(1.2),
            clock_remaining_sec: Some(590.0),
            initial_time_sec: Some(600.0),
            time_class: "rapid",
            opening_break: false,
        };
        assert!(tag_move(&ctx).contains(&"IMPULSIVE_BLUNDER".to_string()));
    }

    #[test]
    fn identifies_winning_game_thrown() {
        let tags = game_tags("loss", "lost by resignation", 5.2);
        assert!(tags.contains(&"WINNING_GAME_THROWN".to_string()));
    }
}
