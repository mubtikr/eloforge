use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use shakmaty::{fen::Fen, san::SanPlus, uci::UciMove, CastlingMode, Chess, Position};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    ai::{prompts, stable_session},
    engine::EngineLine,
    error::{AppError, AppResult},
};

pub const EMBEDDING_DIMS: usize = 384;

#[derive(Debug, Clone)]
pub struct MoveCoachContext {
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub user_color: String,
    pub result: String,
    pub time_class: Option<String>,
    pub time_control: Option<String>,
    pub move_color: String,
    pub opening_name: Option<String>,
    pub accuracy_user: Option<f64>,
    pub ai_session_id: Uuid,
    pub user_elo: Option<i32>,
    pub move_id: i64,
    pub ply: i32,
    pub move_number: i32,
    pub san: String,
    pub uci: String,
    pub fen_before: String,
    pub eval_before_cp: Option<i32>,
    pub eval_after_cp: Option<i32>,
    pub epl: Option<f64>,
    pub classification: Option<String>,
    pub tags: Vec<String>,
    pub best_move_uci: Option<String>,
    pub best_move_san: Option<String>,
    pub pv: Option<serde_json::Value>,
    pub clock_remaining_sec: Option<f64>,
    pub time_spent_sec: Option<f64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MemoryRow {
    pub explanation: String,
    pub ai_feedback: Option<String>,
    pub next_time_rule: Option<String>,
    pub reason_tags: Vec<String>,
}

pub async fn load_move_context(
    db: &PgPool,
    user_id: Uuid,
    game_id: Uuid,
    move_id: i64,
) -> AppResult<MoveCoachContext> {
    let row = sqlx::query_as::<_, MoveContextRow>(
        "SELECT g.id AS game_id, g.user_id, g.user_color, g.result, g.time_class, g.time_control,
                m.color AS move_color, g.opening_name, g.accuracy_user, g.ai_session_id,
                u.current_elo AS user_elo,
                m.id AS move_id, m.ply, m.move_number, m.san, m.uci, m.fen_before,
                m.eval_before_cp, m.eval_after_cp, m.epl, m.classification, m.mistake_tags AS tags,
                m.best_move_uci, m.best_move_san, m.pv, m.clock_remaining_sec, m.time_spent_sec
         FROM games g
         JOIN game_moves m ON m.game_id = g.id
         JOIN users u ON u.id = g.user_id
         WHERE g.id = $1 AND g.user_id = $2 AND m.id = $3",
    )
    .bind(game_id)
    .bind(user_id)
    .bind(move_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::not_found("Move not found in this game."))?;

    if row.user_color != "white" && row.user_color != "black" {
        return Err(AppError::Internal(anyhow::anyhow!(
            "invalid game colour in database"
        )));
    }

    Ok(MoveCoachContext {
        game_id: row.game_id,
        user_id: row.user_id,
        user_color: row.user_color,
        result: row.result,
        time_class: row.time_class,
        time_control: row.time_control,
        move_color: row.move_color,
        opening_name: row.opening_name,
        accuracy_user: row.accuracy_user,
        ai_session_id: row.ai_session_id,
        user_elo: row.user_elo,
        move_id: row.move_id,
        ply: row.ply,
        move_number: row.move_number,
        san: row.san,
        uci: row.uci,
        fen_before: row.fen_before,
        eval_before_cp: row.eval_before_cp,
        eval_after_cp: row.eval_after_cp,
        epl: row.epl,
        classification: row.classification,
        tags: row.tags,
        best_move_uci: row.best_move_uci,
        best_move_san: row.best_move_san,
        pv: row.pv,
        clock_remaining_sec: row.clock_remaining_sec,
        time_spent_sec: row.time_spent_sec,
    })
}

#[derive(sqlx::FromRow)]
struct MoveContextRow {
    game_id: Uuid,
    user_id: Uuid,
    user_color: String,
    result: String,
    time_class: Option<String>,
    time_control: Option<String>,
    move_color: String,
    opening_name: Option<String>,
    accuracy_user: Option<f64>,
    ai_session_id: Uuid,
    user_elo: Option<i32>,
    move_id: i64,
    ply: i32,
    move_number: i32,
    san: String,
    uci: String,
    fen_before: String,
    eval_before_cp: Option<i32>,
    eval_after_cp: Option<i32>,
    epl: Option<f64>,
    classification: Option<String>,
    tags: Vec<String>,
    best_move_uci: Option<String>,
    best_move_san: Option<String>,
    pv: Option<serde_json::Value>,
    clock_remaining_sec: Option<f64>,
    time_spent_sec: Option<f64>,
}

pub fn elo_band(elo: Option<i32>) -> &'static str {
    match elo.unwrap_or(800) {
        ..=800 => "500-800: beginner. Short sentences, concrete board cues, no opening theory.",
        801..=1200 => "800-1200: developing club player. Name the pattern, keep variations short.",
        1201..=1600 => "1200-1600: intermediate. Precise terms are welcome; stay concrete.",
        _ => "1600+: advanced. Prioritise the decisive detail and the next-time rule.",
    }
}

pub fn legal_moves_san(fen: &str) -> Vec<String> {
    let Ok(fen) = Fen::from_ascii(fen.as_bytes()) else {
        return Vec::new();
    };
    let Ok(pos) = fen.into_position::<Chess>(CastlingMode::Standard) else {
        return Vec::new();
    };
    pos.legal_moves()
        .iter()
        .take(80)
        .map(|mv| SanPlus::from_move(pos.clone(), *mv).to_string())
        .collect()
}

pub fn pv_sans(fen: &str, pv: &[String]) -> Vec<String> {
    let Ok(fen) = Fen::from_ascii(fen.as_bytes()) else {
        return Vec::new();
    };
    let Ok(mut pos) = fen.into_position::<Chess>(CastlingMode::Standard) else {
        return Vec::new();
    };
    let mut sans = Vec::new();
    for uci in pv.iter().take(10) {
        let Ok(parsed) = uci.parse::<UciMove>() else {
            break;
        };
        let Ok(mv) = parsed.to_move(&pos) else {
            break;
        };
        sans.push(SanPlus::from_move(pos.clone(), mv).to_string());
        pos.play_unchecked(mv);
    }
    sans
}

fn best_pv(ctx: &MoveCoachContext) -> Vec<String> {
    ctx.pv
        .as_ref()
        .and_then(|value| serde_json::from_value::<Vec<EngineLine>>(value.clone()).ok())
        .and_then(|lines| lines.first().map(|line| line.pv.clone()))
        .unwrap_or_default()
}

pub fn allowed_moves(ctx: &MoveCoachContext) -> HashSet<String> {
    let mut allowed = HashSet::new();
    allowed.insert(normalize_san(&ctx.san));
    if let Some(best) = &ctx.best_move_san {
        allowed.insert(normalize_san(best));
    }
    for san in pv_sans(&ctx.fen_before, &best_pv(ctx)) {
        allowed.insert(normalize_san(&san));
    }
    allowed
}

pub fn build_facts(ctx: &MoveCoachContext) -> serde_json::Value {
    let best_pv = best_pv(ctx);
    json!({
        "game": {
            "result_for_player": ctx.result,
            "player_color": ctx.user_color,
            "time_class": ctx.time_class,
            "time_control": ctx.time_control,
            "opening": ctx.opening_name,
            "accuracy_user": ctx.accuracy_user,
            "player_elo": ctx.user_elo,
        },
        "move": {
            "move_number": ctx.move_number,
            "ply": ctx.ply,
            "played_move": ctx.san,
            "played_uci": ctx.uci,
            "classification": ctx.classification,
            "mistake_tags": ctx.tags,
            "eval_before_cp": ctx.eval_before_cp,
            "eval_after_cp": ctx.eval_after_cp,
            "expected_points_lost_fraction": ctx.epl,
            "win_probability_lost_points": ctx.epl.map(|epl| (epl * 100.0 * 10.0).round() / 10.0),
            "best_move_san": ctx.best_move_san,
            "best_move_uci": ctx.best_move_uci,
            "best_line_uci": best_pv,
            "clock_remaining_sec": ctx.clock_remaining_sec,
            "seconds_spent": ctx.time_spent_sec,
        },
        "legal_moves": legal_moves_san(&ctx.fen_before),
        "elo_band": elo_band(ctx.user_elo),
    })
}

pub fn build_reflection_prompt(
    ctx: &MoveCoachContext,
    explanation: &str,
    memories: &[MemoryRow],
) -> String {
    let facts = serde_json::to_string_pretty(&build_facts(ctx)).unwrap_or_else(|_| "{}".into());
    let memory_text = if memories.is_empty() {
        "No earlier reflections on record.".to_string()
    } else {
        memories
            .iter()
            .enumerate()
            .map(|(index, memory)| {
                format!(
                    "{}. tags={:?} • \"{}\" • rule: {}",
                    index + 1,
                    memory.reason_tags,
                    memory.explanation,
                    memory
                        .next_time_rule
                        .clone()
                        .unwrap_or_else(|| "none stored".into())
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    prompts::reflection_prompt(&facts, explanation, &memory_text)
}

pub fn fallback_reflection(ctx: &MoveCoachContext, explanation: &str) -> (String, String) {
    let pp = ctx.epl.unwrap_or(0.0) * 100.0;
    let best = ctx
        .best_move_san
        .clone()
        .unwrap_or_else(|| "the engine's top move".to_string());
    let tag_label = tag_label(&ctx.tags);
    let clock = clock_clause(ctx);
    let rule = rule_for_tags(&ctx.tags);

    let feedback = format!(
        "What happened\nYou played {}. The expected score fell by {:.1} percentage points{}. The better move was {}.\n\nWhy it happened\nYour note says \"{}\". The verified tags point to {}.\n\nNext-time rule\n{}",
        ctx.san, pp.max(0.0), clock, best, explanation.trim(), tag_label, rule
    );
    (feedback, rule)
}

fn clock_clause(ctx: &MoveCoachContext) -> String {
    let mut clauses = Vec::new();
    if let Some(seconds) = ctx.time_spent_sec.filter(|seconds| *seconds >= 0.2) {
        clauses.push(format!("after {seconds:.1}s"));
    }
    let live = ctx
        .time_class
        .as_deref()
        .map(|class| class.eq_ignore_ascii_case("rapid") || class.eq_ignore_ascii_case("blitz"))
        .unwrap_or(false);
    if live {
        if let Some(seconds) = ctx.clock_remaining_sec {
            clauses.push(format!("with {seconds:.0}s on the clock"));
        }
    }
    if clauses.is_empty() {
        String::new()
    } else {
        format!(" {}", clauses.join(" "))
    }
}

pub fn rule_for_tags(tags: &[String]) -> String {
    let has = |tag: &str| tags.iter().any(|value| value == tag);
    if has("HANGING_PIECE") {
        return "Before every move, name the undefended pieces on the board, then check whether your move leaves one of yours hanging.".into();
    }
    if has("MISSED_MATE_1_2") || has("ALLOWED_MATE") {
        return "When a check appears, calculate checks first: is there a forced mate in one or two?".into();
    }
    if has("MISSED_FREE_PIECE") {
        return "Before moving, list every free capture; if a safe one exists and no check is stronger, take it.".into();
    }
    if has("ENDGAME_TECHNIQUE_FAIL") || has("WINNING_GAME_THROWN") || has("STALEMATE_THROW") {
        return "In a winning endgame, improve your king or rook before touching a pawn, and confirm the opponent has a legal move.".into();
    }
    if has("IMPULSIVE_BLUNDER") || has("TIME_SCRAMBLE_BLUNDER") {
        return "If you have more than half your clock, spend at least four seconds on checks, captures and threats before moving.".into();
    }
    if has("OPENING_PRINCIPLE_BREAK") {
        return "For the first ten moves, develop a new piece or castle; do not move the same piece twice without a concrete reason.".into();
    }
    "Before moving, run the habit check: checks, captures, threats for both sides.".into()
}

pub fn tag_label(tags: &[String]) -> String {
    if tags.is_empty() {
        return "a one-move tactical slip".into();
    }
    tags.iter()
        .map(|tag| {
            match tag.as_str() {
                "HANGING_PIECE" => "a hanging piece",
                "MISSED_FREE_PIECE" => "a missed free piece",
                "MISSED_MATE_1_2" => "a missed mate in one or two",
                "ALLOWED_MATE" => "a mating net allowed",
                "TACTICAL_BLINDNESS" => "a missed tactic",
                "WINNING_GAME_THROWN" => "a winning position thrown",
                "STALEMATE_THROW" => "a stalemate resource allowed",
                "ENDGAME_TECHNIQUE_FAIL" => "an endgame technique failure",
                "IMPULSIVE_BLUNDER" => "an impulsive blunder",
                "TIME_SCRAMBLE_BLUNDER" => "a time-scramble blunder",
                "OPENING_PRINCIPLE_BREAK" => "an opening principle break",
                other => other,
            }
            .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn extract_rule(text: &str, fallback: &str) -> String {
    for line in text.lines() {
        let trimmed = line.trim().trim_start_matches(['*', '#', '-', ' ']);
        if let Some(value) = trimmed.strip_prefix("NEXT_TIME_RULE:") {
            let value = value.trim();
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    fallback.to_string()
}

pub fn strip_rule_line(text: &str) -> String {
    text.lines()
        .filter(|line| {
            !line
                .trim()
                .trim_start_matches(['*', '#', '-', ' '])
                .starts_with("NEXT_TIME_RULE:")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub fn validates_moves(text: &str, allowed: &HashSet<String>) -> bool {
    let mut candidate = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '=' | '+' | '#') {
            candidate.push(ch);
        } else {
            if is_san_candidate(&candidate) && !allowed.contains(&normalize_san(&candidate)) {
                return false;
            }
            candidate.clear();
        }
    }
    !(is_san_candidate(&candidate) && !allowed.contains(&normalize_san(&candidate)))
}

fn is_san_candidate(token: &str) -> bool {
    if token == "O-O" || token == "O-O-O" {
        return true;
    }
    if token.len() < 2 || token.len() > 8 {
        return false;
    }
    let core = token.trim_end_matches(['+', '#']);
    let has_file = core.chars().any(|c| ('a'..='h').contains(&c));
    let has_rank = core.chars().any(|c| ('1'..='8').contains(&c));
    let first = token.chars().next().unwrap_or(' ');
    let piece_letter = matches!(first, 'K' | 'Q' | 'R' | 'B' | 'N');
    let looks_numeric = core
        .chars()
        .all(|c| c.is_ascii_digit() || c == 'x' || c == '=');
    if looks_numeric || !(has_file && has_rank) {
        return false;
    }
    piece_letter
        || core
            .chars()
            .next()
            .is_some_and(|c| ('a'..='h').contains(&c))
        || core.starts_with('x')
}

fn normalize_san(san: &str) -> String {
    san.trim()
        .trim_end_matches(['+', '#', '!', '?'])
        .trim_start_matches(['*', '#', '-', ' '])
        .to_string()
}

pub fn hash_embedding(text: &str) -> Vec<f32> {
    let mut vector = vec![0f32; EMBEDDING_DIMS];
    let lower = text.to_lowercase();
    for token in lower.split(|c: char| !c.is_alphanumeric()) {
        if token.len() < 2 {
            continue;
        }
        let digest = Sha256::digest(token.as_bytes());
        let index = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as usize
            % EMBEDDING_DIMS;
        let sign = if digest[4] & 1 == 0 { 1.0 } else { -1.0 };
        vector[index] += sign;
    }
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    } else {
        vector[0] = 1.0;
    }
    vector
}

pub fn embedding_literal(vector: &[f32]) -> String {
    let body = vector
        .iter()
        .map(|value| format!("{value:.6}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

pub async fn find_memories(
    db: &PgPool,
    user_id: Uuid,
    exclude_id: Uuid,
    reason_tags: &[String],
    embedding: &[f32],
) -> AppResult<Vec<MemoryRow>> {
    let literal = embedding_literal(embedding);
    let rows = sqlx::query_as::<_, MemoryRow>(
        "SELECT explanation, ai_feedback, next_time_rule, reason_tags
         FROM move_reflections
         WHERE user_id = $1 AND id <> $2 AND ai_feedback IS NOT NULL
         ORDER BY (CASE WHEN reason_tags && $3::text[] THEN 0 ELSE 1 END),
                  embedding <=> $4::vector
         LIMIT 5",
    )
    .bind(user_id)
    .bind(exclude_id)
    .bind(reason_tags)
    .bind(literal)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn insert_reflection(
    db: &PgPool,
    ctx: &MoveCoachContext,
    reason_tags: &[String],
    explanation: &str,
) -> AppResult<Uuid> {
    let embedding = format!(
        "{} tags {} move {} {}",
        explanation,
        reason_tags.join(" "),
        ctx.san,
        ctx.tags.join(" ")
    );
    let literal = embedding_literal(&hash_embedding(&embedding));
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO move_reflections
            (user_id, game_id, move_id, reason_tags, explanation, ai_status, embedding)
         VALUES ($1,$2,$3,$4,$5,'pending',$6::vector)
         RETURNING id",
    )
    .bind(ctx.user_id)
    .bind(ctx.game_id)
    .bind(ctx.move_id)
    .bind(reason_tags)
    .bind(explanation)
    .bind(literal)
    .fetch_one(db)
    .await?;
    Ok(id)
}

pub async fn save_reflection_result(
    db: &PgPool,
    reflection_id: Uuid,
    feedback: &str,
    rule: &str,
    ai_status: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE move_reflections
         SET ai_feedback = $2, next_time_rule = $3, ai_status = $4
         WHERE id = $1",
    )
    .bind(reflection_id)
    .bind(feedback)
    .bind(rule)
    .bind(ai_status)
    .execute(db)
    .await?;
    Ok(())
}

pub fn session_for_game(game_id: Uuid) -> Uuid {
    stable_session(&format!("game:{game_id}"))
}

pub fn timestamp_label(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_is_normalised_and_deterministic() {
        let a = hash_embedding("I moved my queen too fast");
        let b = hash_embedding("I moved my queen too fast");
        assert_eq!(a, b);
        let norm = a.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[test]
    fn validation_accepts_supplied_moves_and_rejects_invented_ones() {
        let mut allowed = HashSet::new();
        allowed.insert("Nf3".to_string());
        allowed.insert("Qxf7".to_string());
        assert!(validates_moves("Better was Nf3 or Qxf7.", &allowed));
        assert!(!validates_moves(
            "Better was Bb5 in the Ruy Lopez.",
            &allowed
        ));
    }

    #[test]
    fn rule_extraction_strips_marker() {
        let rule = extract_rule("Body text\nNEXT_TIME_RULE: Check the defender.", "fallback");
        assert_eq!(rule, "Check the defender.");
        let body = strip_rule_line("Body text\nNEXT_TIME_RULE: Check the defender.");
        assert_eq!(body, "Body text");
    }

    #[test]
    fn fallback_is_engine_grounded() {
        let ctx = MoveCoachContext {
            game_id: Uuid::nil(),
            user_id: Uuid::nil(),
            user_color: "white".into(),
            result: "loss".into(),
            time_class: Some("rapid".into()),
            time_control: Some("600".into()),
            move_color: "white".into(),
            opening_name: None,
            accuracy_user: Some(70.0),
            ai_session_id: Uuid::nil(),
            user_elo: Some(600),
            move_id: 1,
            ply: 18,
            move_number: 9,
            san: "Qh5".into(),
            uci: "d1h5".into(),
            fen_before: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".into(),
            eval_before_cp: Some(30),
            eval_after_cp: Some(-250),
            epl: Some(0.28),
            classification: Some("blunder".into()),
            tags: vec!["HANGING_PIECE".into()],
            best_move_uci: Some("g1f3".into()),
            best_move_san: Some("Nf3".into()),
            pv: None,
            clock_remaining_sec: Some(420.0),
            time_spent_sec: Some(2.0),
        };
        let (feedback, rule) = fallback_reflection(&ctx, "I did not see the defender");
        assert!(feedback.contains("Qh5"));
        assert!(feedback.contains("28.0 percentage points"));
        assert!(rule.contains("undefended pieces"));
    }
}
