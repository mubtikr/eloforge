use std::{collections::HashMap, sync::OnceLock};

use serde::{Deserialize, Serialize};

use crate::chess::pgn_importer::ImportedMove;

static BOOK: OnceLock<OpeningBook> = OnceLock::new();

#[derive(Debug, Clone)]
struct EcoEntry {
    eco: String,
    name: String,
}

#[derive(Debug)]
pub struct OpeningBook {
    by_sequence: HashMap<String, EcoEntry>,
    max_plies: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningMatch {
    pub eco: String,
    pub name: String,
    pub variation: Option<String>,
    pub last_book_ply: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipleViolation {
    pub code: String,
    pub label: String,
    pub detail: String,
    pub ply: u32,
}

pub fn book() -> &'static OpeningBook {
    BOOK.get_or_init(|| OpeningBook::load(include_str!("../../assets/eco.tsv")))
}

impl OpeningBook {
    fn load(tsv: &str) -> Self {
        let mut by_sequence = HashMap::with_capacity(4096);
        let mut max_plies = 0usize;

        for line in tsv.lines().skip(1) {
            let mut fields = line.split('\t');
            let (Some(eco), Some(name), Some(pgn)) = (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let moves = normalize_pgn(pgn);
            if moves.is_empty() {
                continue;
            }
            max_plies = max_plies.max(moves.len());
            by_sequence.insert(
                moves.join(" "),
                EcoEntry {
                    eco: eco.to_string(),
                    name: name.to_string(),
                },
            );
        }

        Self {
            by_sequence,
            max_plies,
        }
    }

    pub fn detect(&self, sans: &[String]) -> Option<OpeningMatch> {
        if sans.is_empty() {
            return None;
        }
        let normalized: Vec<String> = sans.iter().map(|san| normalize_san(san)).collect();
        let limit = normalized.len().min(self.max_plies);
        for plies in (1..=limit).rev() {
            let key = normalized[..plies].join(" ");
            if let Some(entry) = self.by_sequence.get(&key) {
                let variation = entry
                    .name
                    .split_once(": ")
                    .map(|(_, variation)| variation.to_string());
                return Some(OpeningMatch {
                    eco: entry.eco.clone(),
                    name: entry.name.clone(),
                    variation,
                    last_book_ply: plies,
                });
            }
        }
        None
    }
}

pub fn detect_opening(sans: &[String]) -> Option<OpeningMatch> {
    book().detect(sans)
}

fn normalize_san(san: &str) -> String {
    san.trim()
        .trim_end_matches(['+', '#', '!', '?'])
        .to_string()
}

fn normalize_pgn(pgn: &str) -> Vec<String> {
    pgn.split_whitespace()
        .filter(|token| {
            !token.is_empty()
                && !token.starts_with('$')
                && !token.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
        .map(normalize_san)
        .filter(|token| !token.is_empty() && token != "...")
        .collect()
}

pub fn find_violations(moves: &[ImportedMove], user_color: &str) -> Vec<PrincipleViolation> {
    let mut violations = Vec::new();
    let mut castled_ply: Option<u32> = None;
    let mut piece_counts: HashMap<String, (u32, u32)> = HashMap::new();
    let mut square_to_piece: HashMap<String, String> = HashMap::new();
    let mut early_queen = false;

    for mv in moves.iter().filter(|m| m.color == user_color) {
        let uci_from = mv.uci.get(0..2).unwrap_or("");
        let uci_to = mv.uci.get(2..4).unwrap_or("");
        let role = mv.san.chars().next().unwrap_or('P');

        if mv.san.starts_with("O-O") {
            castled_ply = Some(mv.ply);
            square_to_piece.insert(uci_to.to_string(), "K".to_string());
            continue;
        }

        if matches!(role, 'N' | 'B' | 'R' | 'Q' | 'K') {
            let id = square_to_piece
                .remove(uci_from)
                .unwrap_or_else(|| format!("{role}{uci_from}"));
            square_to_piece.insert(uci_to.to_string(), id.clone());

            if mv.ply <= 20 {
                let entry = piece_counts.entry(id).or_insert((0, mv.ply));
                entry.0 += 1;
                if entry.0 == 3 {
                    violations.push(PrincipleViolation {
                        code: "REPEATED_PIECE_MOVES".into(),
                        label: "Same piece moved three times".into(),
                        detail: format!(
                            "{} moved three times before move 10; each tempo is a gift.",
                            mv.san
                        ),
                        ply: entry.1,
                    });
                }
            }

            if role == 'Q' && mv.move_number <= 3 && !early_queen {
                early_queen = true;
                violations.push(PrincipleViolation {
                    code: "EARLY_QUEEN_SORTIE".into(),
                    label: "Early queen sortie".into(),
                    detail: format!(
                        "{} brings the queen out before move 4, where it can be hit by development.",
                        mv.san
                    ),
                    ply: mv.ply,
                });
            }
        }
    }

    let reached_midgame = moves.len() as u32 >= 24;
    if reached_midgame {
        match castled_ply {
            None => violations.push(PrincipleViolation {
                code: "UNCASTLED_KING".into(),
                label: "King still uncastled after move 12".into(),
                detail: "The king stayed in the centre past move 12; open files become tactics."
                    .into(),
                ply: 24,
            }),
            Some(ply) if ply > 24 => violations.push(PrincipleViolation {
                code: "UNCASTLED_KING".into(),
                label: "King castled late".into(),
                detail: format!("Castling arrived on ply {ply}, after the opening window."),
                ply,
            }),
            _ => {}
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_full_eco_range() {
        let book = book();
        assert!(book.by_sequence.len() > 3000);
        assert!(book.max_plies > 10);
    }

    #[test]
    fn detects_ruy_lopez() {
        let sans: Vec<String> = ["e4", "e5", "Nf3", "Nc6", "Bb5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let opening = detect_opening(&sans).expect("Ruy Lopez should be in the book");
        assert!(opening.eco.starts_with('C'));
        assert!(opening.name.contains("Ruy Lopez"));
        assert_eq!(opening.last_book_ply, 5);
    }

    #[test]
    fn detects_queens_gambit() {
        let sans: Vec<String> = ["d4", "d5", "c4"].iter().map(|s| s.to_string()).collect();
        let opening = detect_opening(&sans).expect("Queen's Gambit should be in the book");
        assert!(opening.eco.starts_with('D'));
    }

    #[test]
    fn normalizes_check_suffixes() {
        assert_eq!(normalize_san("Qxf7#"), "Qxf7");
        assert_eq!(normalize_san("Bb5+"), "Bb5");
    }

    #[test]
    fn no_opening_for_empty_moves() {
        assert!(detect_opening(&[]).is_none());
    }
}
