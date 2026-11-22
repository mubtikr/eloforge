use serde::{Deserialize, Serialize};
use shakmaty::fen::Fen;

use crate::engine::{EngineEval, EngineLine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSnapshot {
    pub cp: Option<i32>,
    pub mate: Option<i32>,
    pub win_white: f64,
    pub best_uci: Option<String>,
    pub lines: Vec<EngineLine>,
}

impl EvalSnapshot {
    pub fn from_eval(eval: &EngineEval, white_to_move: bool) -> Self {
        let top = eval.top();
        let score_cp = top.and_then(|line| line.score_cp);
        let score_mate = top.and_then(|line| line.score_mate);

        let (cp, mate) = match (score_cp, score_mate) {
            (_, Some(mate)) => (None, Some(if white_to_move { mate } else { -mate })),
            (Some(cp), None) => (Some(if white_to_move { cp } else { -cp }), None),
            (None, None) => (Some(0), None),
        };

        let win_white = match mate {
            Some(m) if m > 0 => 100.0,
            Some(m) if m < 0 => 0.0,
            Some(_) => 50.0,
            None => {
                let side_to_move_win = cp_to_win_prob(f64::from(cp.unwrap_or(0)));
                if white_to_move {
                    side_to_move_win
                } else {
                    100.0 - side_to_move_win
                }
            }
        };

        Self {
            cp,
            mate,
            win_white: win_white.clamp(0.0, 100.0),
            best_uci: eval
                .best_move
                .clone()
                .or_else(|| eval.top().and_then(|line| line.pv.first()).cloned()),
            lines: eval.lines.clone(),
        }
    }

    pub fn win_for(&self, color_white: bool) -> f64 {
        if color_white {
            self.win_white
        } else {
            100.0 - self.win_white
        }
    }

    pub fn mate_for(&self, color_white: bool) -> Option<i32> {
        self.mate.map(|m| if color_white { m } else { -m })
    }

    pub fn cp_for(&self, color_white: bool) -> Option<i32> {
        self.cp.map(|cp| if color_white { cp } else { -cp })
    }
}

/// Lichess-calibrated logistic mapping from centipawns to win probability.
pub fn cp_to_win_prob(cp: f64) -> f64 {
    50.0 + 50.0 * (2.0 / (1.0 + (-0.00368208 * cp).exp()) - 1.0)
}

pub fn win_prob_to_pawns(win: f64) -> f64 {
    if win <= 0.0 || win >= 100.0 {
        return 0.0;
    }
    let p = (win - 50.0) / 50.0;
    let inner = (2.0 / (p + 1.0)) - 1.0;
    if inner <= 0.0 {
        0.0
    } else {
        -inner.ln() / 0.00368208 / 100.0
    }
}

pub fn classify(epl: f64, win_before: f64, win_after: f64) -> &'static str {
    if win_before >= 85.0 && win_after < 55.0 {
        return "missed_win";
    }
    if epl <= 0.01 {
        "best"
    } else if epl <= 0.03 {
        "strong"
    } else if epl <= 0.06 {
        "good"
    } else if epl <= 0.12 {
        "inaccuracy"
    } else if epl <= 0.22 {
        "mistake"
    } else {
        "blunder"
    }
}

pub fn accuracy_from_epl(epl: f64) -> f64 {
    let raw = 103.1668 * (-0.04354 * (epl * 100.0)).exp() - 3.1669;
    raw.clamp(0.0, 100.0)
}

pub fn is_bad(classification: &str) -> bool {
    matches!(
        classification,
        "inaccuracy" | "mistake" | "blunder" | "missed_win"
    )
}

pub fn phase_for_ply(ply: i32, opening_end: i32, endgame_start: i32) -> &'static str {
    if ply <= opening_end {
        "opening"
    } else if ply >= endgame_start {
        "endgame"
    } else {
        "middlegame"
    }
}

pub fn endgame_start_ply(moves: &[crate::models::MoveDto], opening_end: i32) -> i32 {
    for mv in moves {
        if is_endgame_fen(&mv.fen_after) {
            return mv.ply.max(opening_end);
        }
    }
    i32::MAX / 2
}

pub fn is_endgame_fen(fen: &str) -> bool {
    let Ok(fen) = Fen::from_ascii(fen.as_bytes()) else {
        return false;
    };
    let board = fen.into_setup().board;
    let material = board.material();
    let white = non_pawn_value(&material.white);
    let black = non_pawn_value(&material.black);
    let queens = i32::from(material.white.queen) + i32::from(material.black.queen);

    (white <= 13 && black <= 13) || (queens == 0 && white <= 6 && black <= 6)
}

fn non_pawn_value(role: &shakmaty::ByRole<u8>) -> i32 {
    i32::from(role.knight) * 3
        + i32::from(role.bishop) * 3
        + i32::from(role.rook) * 5
        + i32::from(role.queen) * 9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_probability_is_symmetric() {
        assert!((cp_to_win_prob(0.0) - 50.0).abs() < 1e-9);
        assert!((cp_to_win_prob(100.0) - (100.0 - cp_to_win_prob(-100.0))).abs() < 1e-9);
        assert!(cp_to_win_prob(1000.0) > 95.0);
    }

    #[test]
    fn classification_matches_prd_thresholds() {
        assert_eq!(classify(0.0, 50.0, 50.0), "best");
        assert_eq!(classify(0.02, 50.0, 48.0), "strong");
        assert_eq!(classify(0.05, 50.0, 45.0), "good");
        assert_eq!(classify(0.10, 50.0, 40.0), "inaccuracy");
        assert_eq!(classify(0.20, 50.0, 30.0), "mistake");
        assert_eq!(classify(0.30, 50.0, 20.0), "blunder");
        assert_eq!(classify(0.05, 90.0, 50.0), "missed_win");
    }

    #[test]
    fn accuracy_is_high_for_perfect_moves() {
        assert!(accuracy_from_epl(0.0) > 99.0);
        assert!(accuracy_from_epl(0.5) < 80.0);
    }

    #[test]
    fn detects_endgame_material() {
        assert!(is_endgame_fen("8/8/4k3/8/8/4K3/4P3/8 w - - 0 1"));
        assert!(!is_endgame_fen(
            "r1bqkbnr/pppppppp/2n5/8/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 0 1"
        ));
    }

    #[test]
    fn win_prob_round_trips_to_pawns() {
        let win = cp_to_win_prob(150.0);
        let pawns = win_prob_to_pawns(win);
        assert!((pawns - 1.5).abs() < 0.05);
    }
}
