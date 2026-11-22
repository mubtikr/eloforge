use std::{collections::HashMap, ops::ControlFlow};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use pgn_reader::{Outcome, RawComment, RawTag, Reader, SanPlus, Skip, Visitor};
use shakmaty::{
    fen::Fen, uci::UciMove, CastlingMode, Chess, Color, EnPassantMode, KnownOutcome, Move, Position,
};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    chesscom::CcGame,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportedMove {
    pub ply: u32,
    pub move_number: u32,
    pub color: String,
    pub san: String,
    pub uci: String,
    pub fen_before: String,
    pub fen_after: String,
    pub clock_remaining_sec: Option<f64>,
    pub time_spent_sec: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ParsedGame {
    pub headers: HashMap<String, String>,
    pub initial_fen: String,
    pub moves: Vec<ImportedMove>,
    pub user_color: Color,
    pub result: String,
    pub result_reason: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub white_username: String,
    pub black_username: String,
    pub white_rating: Option<i32>,
    pub black_rating: Option<i32>,
    pub eco: Option<String>,
    pub time_control: Option<String>,
    pub time_class: Option<String>,
    pub rated: bool,
    pub termination: Option<String>,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedMove {
    ply: u32,
    move_number: u32,
    color: Color,
    san: String,
    uci: String,
    fen_before: String,
    fen_after: String,
    clock_remaining_sec: Option<f64>,
    explicit_emt_sec: Option<f64>,
}

struct Movetext {
    headers: HashMap<String, String>,
    pos: Chess,
    initial_fen: String,
    moves: Vec<ParsedMove>,
    outcome: Option<Outcome>,
    error: Option<String>,
}

struct PgnVisitor<'a> {
    username: &'a str,
}

impl Visitor for PgnVisitor<'_> {
    type Tags = HashMap<String, String>;
    type Movetext = Movetext;
    type Output = Result<ParsedGame, String>;

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(HashMap::new())
    }

    fn tag(
        &mut self,
        tags: &mut Self::Tags,
        name: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        let name = String::from_utf8_lossy(name).to_string();
        let value = value.decode_utf8_lossy().to_string();
        tags.insert(name, value);
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, headers: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        let (pos, initial_fen) = match headers.get("FEN") {
            Some(fen) => match Fen::from_ascii(fen.as_bytes())
                .map_err(|e| format!("invalid FEN tag: {e}"))
                .and_then(|f| {
                    f.into_position::<Chess>(CastlingMode::Standard)
                        .map_err(|e| format!("illegal FEN tag: {e}"))
                }) {
                Ok(pos) => {
                    let normalized = Fen::from_position(&pos, EnPassantMode::Legal).to_string();
                    (pos, normalized)
                }
                Err(err) => {
                    return ControlFlow::Break(Err(err));
                }
            },
            None => (
                Chess::default(),
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            ),
        };

        ControlFlow::Continue(Movetext {
            headers,
            pos,
            initial_fen,
            moves: Vec::new(),
            outcome: None,
            error: None,
        })
    }

    fn san(
        &mut self,
        movetext: &mut Self::Movetext,
        san_plus: SanPlus,
    ) -> ControlFlow<Self::Output> {
        if movetext.error.is_some() {
            return ControlFlow::Continue(());
        }

        let color = movetext.pos.turn();
        let ply = movetext.moves.len() as u32 + 1;
        let move_number = ply.div_ceil(2);
        let fen_before = Fen::from_position(&movetext.pos, EnPassantMode::Legal).to_string();
        let san = san_plus.to_string();

        let mv: Move = match san_plus.san.to_move(&movetext.pos) {
            Ok(mv) => mv,
            Err(err) => {
                movetext.error = Some(format!("illegal move {san} at ply {ply}: {err}"));
                return ControlFlow::Continue(());
            }
        };

        let uci = UciMove::from_move(mv, CastlingMode::Standard).to_string();
        movetext.pos.play_unchecked(mv);
        let fen_after = Fen::from_position(&movetext.pos, EnPassantMode::Legal).to_string();

        movetext.moves.push(ParsedMove {
            ply,
            move_number,
            color,
            san,
            uci,
            fen_before,
            fen_after,
            clock_remaining_sec: None,
            explicit_emt_sec: None,
        });

        ControlFlow::Continue(())
    }

    fn comment(
        &mut self,
        movetext: &mut Self::Movetext,
        comment: RawComment<'_>,
    ) -> ControlFlow<Self::Output> {
        let text = String::from_utf8_lossy(comment.as_bytes());
        let clock = extract_percent_tag(&text, "clk").and_then(|v| parse_clock_seconds(&v));
        let emt = extract_percent_tag(&text, "emt").and_then(|v| parse_clock_seconds(&v));

        if let Some(last) = movetext.moves.last_mut() {
            if clock.is_some() {
                last.clock_remaining_sec = clock;
            }
            if emt.is_some() {
                last.explicit_emt_sec = emt;
            }
        }
        ControlFlow::Continue(())
    }

    fn begin_variation(
        &mut self,
        _movetext: &mut Self::Movetext,
    ) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }

    fn outcome(
        &mut self,
        movetext: &mut Self::Movetext,
        outcome: Outcome,
    ) -> ControlFlow<Self::Output> {
        movetext.outcome = Some(outcome);
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, movetext: Self::Movetext) -> Self::Output {
        if let Some(err) = movetext.error {
            return Err(err);
        }
        if movetext.moves.is_empty() {
            return Err("PGN contains no moves".to_string());
        }

        let headers = movetext.headers;
        let white_username = headers.get("White").cloned().unwrap_or_default();
        let black_username = headers.get("Black").cloned().unwrap_or_default();

        let user_color = if white_username.eq_ignore_ascii_case(self.username) {
            Color::White
        } else if black_username.eq_ignore_ascii_case(self.username) {
            Color::Black
        } else {
            return Err(format!(
                "user {0} is not a player in this game ({white_username} vs {black_username})",
                self.username
            ));
        };

        let result = determine_result(movetext.outcome, headers.get("Result"), user_color)?;
        let base_seconds = parse_time_control_base(headers.get("TimeControl"));
        let mut previous_clock: HashMap<Color, f64> = HashMap::new();
        let mut moves = Vec::with_capacity(movetext.moves.len());

        for mv in movetext.moves {
            let time_spent = if let Some(emt) = mv.explicit_emt_sec {
                Some(emt)
            } else if let Some(clock) = mv.clock_remaining_sec {
                let previous = previous_clock
                    .get(&mv.color)
                    .copied()
                    .or(base_seconds)
                    .unwrap_or(clock);
                Some((previous - clock).max(0.0))
            } else {
                None
            };

            if let Some(clock) = mv.clock_remaining_sec {
                previous_clock.insert(mv.color, clock);
            }

            moves.push(ImportedMove {
                ply: mv.ply,
                move_number: mv.move_number,
                color: color_name(mv.color).to_string(),
                san: mv.san,
                uci: mv.uci,
                fen_before: mv.fen_before,
                fen_after: mv.fen_after,
                clock_remaining_sec: mv.clock_remaining_sec,
                time_spent_sec: time_spent,
            });
        }

        Ok(ParsedGame {
            headers: headers.clone(),
            initial_fen: movetext.initial_fen,
            moves,
            user_color,
            result,
            result_reason: headers.get("Termination").cloned(),
            start_time: parse_pgn_datetime(
                headers.get("UTCDate").or_else(|| headers.get("Date")),
                headers.get("UTCTime"),
            ),
            end_time: parse_pgn_datetime(
                headers.get("EndDate").or_else(|| headers.get("UTCDate")),
                headers.get("EndTime"),
            ),
            white_username,
            black_username,
            white_rating: parse_rating(headers.get("WhiteElo")),
            black_rating: parse_rating(headers.get("BlackElo")),
            eco: headers.get("ECO").cloned().filter(|e| !e.is_empty()),
            time_control: headers.get("TimeControl").cloned(),
            time_class: headers.get("TimeClass").cloned(),
            rated: headers
                .get("Rated")
                .map(|r| !r.eq_ignore_ascii_case("false"))
                .unwrap_or(true),
            termination: headers.get("Termination").cloned(),
            outcome: headers.get("Result").cloned(),
        })
    }
}

pub fn parse_pgn(pgn: &str, username: &str) -> Result<ParsedGame, String> {
    let mut reader = Reader::new(pgn.as_bytes());
    let mut visitor = PgnVisitor { username };
    match reader.read_game(&mut visitor) {
        Ok(Some(result)) => result,
        Ok(None) => Err("PGN is empty".to_string()),
        Err(err) => Err(format!("failed to parse PGN: {err}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOutcome {
    Imported,
    Skipped,
}

pub async fn import_game(
    pool: &PgPool,
    user_id: Uuid,
    username: &str,
    game: &CcGame,
) -> AppResult<ImportOutcome> {
    let Some(pgn) = game.pgn() else {
        return Ok(ImportOutcome::Skipped);
    };

    if let Some(rules) = game.rules.as_deref() {
        if !rules.eq_ignore_ascii_case("chess") {
            tracing::debug!(rules, "skipping non-standard chess game");
            return Ok(ImportOutcome::Skipped);
        }
    }

    let parsed = match parse_pgn(pgn, username) {
        Ok(parsed) => parsed,
        Err(err) if err.contains("no moves") || err.contains("PGN is empty") => {
            return Ok(ImportOutcome::Skipped);
        }
        Err(err) => return Err(AppError::BadRequest(err)),
    };
    let chesscom_game_id = game
        .game_id()
        .unwrap_or_else(|| format!("pgn-{}", pgn_fingerprint(pgn)));

    let existing = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, analysis_status, analysis_error FROM games
         WHERE user_id = $1 AND chesscom_game_id = $2",
    )
    .bind(user_id)
    .bind(&chesscom_game_id)
    .fetch_optional(pool)
    .await?;

    if let Some((existing_id, status, _)) = existing {
        let move_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_moves WHERE game_id = $1")
                .bind(existing_id)
                .fetch_one(pool)
                .await?;
        if move_count as usize == parsed.moves.len() {
            return Ok(ImportOutcome::Skipped);
        }
        if status == "analyzing" {
            return Ok(ImportOutcome::Skipped);
        }
        sqlx::query("DELETE FROM game_moves WHERE game_id = $1")
            .bind(existing_id)
            .execute(pool)
            .await?;
        sqlx::query(
            "UPDATE games SET pgn = $1, analysis_status = 'pending', analysis = NULL,
             analyzed_at = NULL, accuracy_user = NULL, accuracy_white = NULL,
             accuracy_black = NULL, eval_peak = NULL, max_advantage = NULL WHERE id = $2",
        )
        .bind(pgn)
        .bind(existing_id)
        .execute(pool)
        .await?;
        insert_moves(pool, existing_id, &parsed.moves).await?;
        return Ok(ImportOutcome::Imported);
    }

    let start_time = parsed
        .start_time
        .or_else(|| {
            game.end_time
                .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        })
        .unwrap_or_else(Utc::now);
    let end_time = parsed.end_time.or_else(|| {
        game.end_time
            .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
    });

    let session_id = session_for(pool, user_id, start_time).await?;
    let time_class = game
        .time_class
        .clone()
        .or(parsed.time_class.clone())
        .unwrap_or_else(|| "rapid".to_string());

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO games (
            user_id, chesscom_game_id, url, pgn, initial_fen, rules, time_class, time_control,
            rated, white_username, black_username, white_rating, black_rating, user_color,
            result, result_reason, start_time, end_time, move_count, opening_eco, session_id
         ) VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21
         ) RETURNING id",
    )
    .bind(user_id)
    .bind(&chesscom_game_id)
    .bind(game.url.as_deref())
    .bind(pgn)
    .bind(&parsed.initial_fen)
    .bind(game.rules.as_deref().unwrap_or("chess"))
    .bind(&time_class)
    .bind(
        game.time_control
            .as_deref()
            .or(parsed.time_control.as_deref()),
    )
    .bind(game.rated.unwrap_or(parsed.rated))
    .bind(&parsed.white_username)
    .bind(&parsed.black_username)
    .bind(parsed.white_rating)
    .bind(parsed.black_rating)
    .bind(color_name(parsed.user_color))
    .bind(&parsed.result)
    .bind(parsed.result_reason.as_deref())
    .bind(start_time)
    .bind(end_time)
    .bind(parsed.moves.len() as i32)
    .bind(parsed.eco.as_deref())
    .bind(session_id)
    .fetch_one(pool)
    .await?;

    insert_moves(pool, id, &parsed.moves).await?;
    Ok(ImportOutcome::Imported)
}

async fn insert_moves(pool: &PgPool, game_id: Uuid, moves: &[ImportedMove]) -> AppResult<()> {
    if moves.is_empty() {
        return Ok(());
    }
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO game_moves (game_id, ply, move_number, color, san, uci, fen_before, fen_after, clock_remaining_sec, time_spent_sec) ",
    );
    builder.push_values(moves, |mut b, mv| {
        b.push_bind(game_id)
            .push_bind(mv.ply as i32)
            .push_bind(mv.move_number as i32)
            .push_bind(&mv.color)
            .push_bind(&mv.san)
            .push_bind(&mv.uci)
            .push_bind(&mv.fen_before)
            .push_bind(&mv.fen_after)
            .push_bind(mv.clock_remaining_sec)
            .push_bind(mv.time_spent_sec);
    });
    builder
        .build()
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}

async fn session_for(pool: &PgPool, user_id: Uuid, start_time: DateTime<Utc>) -> AppResult<Uuid> {
    let previous = sqlx::query_as::<_, (Option<Uuid>, Option<DateTime<Utc>>, DateTime<Utc>)>(
        "SELECT session_id, end_time, start_time FROM games
         WHERE user_id = $1 AND start_time <= $2
         ORDER BY start_time DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(start_time)
    .fetch_optional(pool)
    .await?;

    if let Some((Some(session_id), end_time, last_start)) = previous {
        let reference = end_time.unwrap_or(last_start);
        let gap = start_time.signed_duration_since(reference).num_minutes();
        if (0..=30).contains(&gap) {
            return Ok(session_id);
        }
    }
    Ok(Uuid::new_v4())
}

fn color_name(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Black => "black",
    }
}

fn parse_rating(value: Option<&String>) -> Option<i32> {
    value.and_then(|v| v.trim().parse::<i32>().ok())
}

fn determine_result(
    outcome: Option<Outcome>,
    result_header: Option<&String>,
    user_color: Color,
) -> Result<String, String> {
    let known = match outcome {
        Some(Outcome::Known(known)) => Some(known),
        Some(Outcome::Unknown) | None => None,
    };

    if let Some(known) = known {
        return Ok(match known {
            KnownOutcome::Draw => "draw".to_string(),
            KnownOutcome::Decisive { winner } if winner == user_color => "win".to_string(),
            KnownOutcome::Decisive { .. } => "loss".to_string(),
        });
    }

    match result_header.map(|s| s.as_str()) {
        Some("1-0") => Ok(if user_color == Color::White {
            "win"
        } else {
            "loss"
        }
        .to_string()),
        Some("0-1") => Ok(if user_color == Color::Black {
            "win"
        } else {
            "loss"
        }
        .to_string()),
        Some("1/2-1/2") => Ok("draw".to_string()),
        Some(other) => Err(format!("unsupported PGN result: {other}")),
        None => Err("PGN has no result".to_string()),
    }
}

fn parse_time_control_base(value: Option<&String>) -> Option<f64> {
    let value = value?;
    let base = value.split('+').next()?.trim();
    base.parse::<f64>().ok().filter(|v| *v > 0.0)
}

fn extract_percent_tag(comment: &str, tag: &str) -> Option<String> {
    let needle = format!("[%{tag} ");
    let start = comment.find(&needle)? + needle.len();
    let rest = &comment[start..];
    let end = rest.find(']')?;
    Some(rest[..end].trim().to_string())
}

pub fn parse_clock_seconds(raw: &str) -> Option<f64> {
    let mut parts = raw.split(':').rev();
    let seconds = parts.next()?.parse::<f64>().ok()?;
    let minutes = parts.next().and_then(|v| v.parse::<f64>().ok());
    let hours = parts.next().and_then(|v| v.parse::<f64>().ok());
    let total = seconds + minutes.unwrap_or(0.0) * 60.0 + hours.unwrap_or(0.0) * 3600.0;
    if total.is_finite() && total >= 0.0 {
        Some(total)
    } else {
        None
    }
}

fn parse_pgn_datetime(date: Option<&String>, time: Option<&String>) -> Option<DateTime<Utc>> {
    let date = date?;
    let naive_date = NaiveDate::parse_from_str(date, "%Y.%m.%d")
        .or_else(|_| NaiveDate::parse_from_str(date, "%Y-%m-%d"))
        .ok()?;
    let naive_time = time
        .and_then(|t| NaiveTime::parse_from_str(t, "%H:%M:%S").ok())
        .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap_or_default());
    let naive = NaiveDateTime::new(naive_date, naive_time);
    Some(Utc.from_utc_datetime(&naive))
}

fn pgn_fingerprint(pgn: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(pgn.as_bytes());
    hex::encode(&hasher.finalize()[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[Event "Live Chess"]
[Site "Chess.com"]
[Date "2025.01.02"]
[UTCDate "2025.01.02"]
[UTCTime "18:00:00"]
[EndTime "18:02:00"]
[White "TestPlayer"]
[Black "Opponent"]
[Result "1-0"]
[WhiteElo "520"]
[BlackElo "505"]
[TimeControl "600"]
[TimeClass "rapid"]
[Termination "TestPlayer won by checkmate"]

1. e4 {[%clk 0:09:58.4]} e5 {[%clk 0:09:55.1]} 2. Bc4 {[%clk 0:09:52.0]} Nc6 {[%clk 0:09:49.7]} 3. Qh5 {[%clk 0:09:47.2]} Nf6 {[%clk 0:09:44.6]} 4. Qxf7# {[%clk 0:09:40.0]} 1-0
"#;

    #[test]
    fn parses_moves_fens_and_clocks() {
        let parsed = parse_pgn(SAMPLE, "testplayer").expect("sample should parse");
        assert_eq!(parsed.moves.len(), 7);
        assert_eq!(parsed.result, "win");
        assert_eq!(parsed.user_color, Color::White);
        assert_eq!(parsed.white_rating, Some(520));
        assert_eq!(parsed.moves[0].san, "e4");
        assert!((parsed.moves[0].time_spent_sec.unwrap_or_default() - 1.6).abs() < 0.01);
        assert!((parsed.moves[1].time_spent_sec.unwrap_or_default() - 4.9).abs() < 0.01);
        assert!((parsed.moves[2].time_spent_sec.unwrap_or_default() - 6.4).abs() < 0.01);
        assert_eq!(parsed.moves[6].san, "Qxf7#");
        assert!(parsed.moves[6].fen_after.contains(' '));
        assert_eq!(parsed.moves[0].ply, 1);
        assert_eq!(parsed.moves[1].move_number, 1);
    }

    #[test]
    fn parses_clock_formats() {
        assert_eq!(parse_clock_seconds("0:09:58.4"), Some(598.4));
        assert_eq!(parse_clock_seconds("1:02:03"), Some(3723.0));
        assert_eq!(parse_clock_seconds("0:05"), Some(5.0));
        assert_eq!(parse_clock_seconds("bad"), None);
    }

    #[test]
    fn rejects_game_where_user_is_missing() {
        let error = parse_pgn(SAMPLE, "someone-else").unwrap_err();
        assert!(error.contains("not a player"));
    }

    #[test]
    fn computes_black_clock() {
        let parsed = parse_pgn(SAMPLE, "opponent").expect("sample should parse");
        assert_eq!(parsed.result, "loss");
        assert_eq!(parsed.user_color, Color::Black);
        assert!((parsed.moves[1].time_spent_sec.unwrap_or_default() - 4.9).abs() < 0.01);
    }
}
