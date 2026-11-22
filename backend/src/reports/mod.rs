pub mod weekly;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use ts_rs::TS;
use uuid::Uuid;

use crate::{error::AppResult, models::ReflectionDto};

const EXPORT: &str = "../../frontend/src/api/bindings.ts";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct RatingLeak {
    pub tag: String,
    pub label: String,
    pub count: i32,
    pub avg_epl: f64,
    pub estimated_elo_cost: f64,
    pub trend: String,
    pub advice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct WeaknessReport {
    pub total_user_moves: i32,
    pub total_blunders: i32,
    pub tags: Vec<RatingLeak>,
    pub top_three: Vec<RatingLeak>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct OpeningStat {
    pub eco: String,
    pub name: String,
    pub color: String,
    pub games: i32,
    pub wins: i32,
    pub draws: i32,
    pub losses: i32,
    pub win_rate: f64,
    pub accuracy: f64,
    pub out_of_book_move: f64,
    pub most_punished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct OpeningReport {
    pub white: Vec<OpeningStat>,
    pub black: Vec<OpeningStat>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct GameRef {
    pub id: Uuid,
    pub white_username: String,
    pub black_username: String,
    pub result: String,
    pub start_time: DateTime<Utc>,
    pub max_advantage: Option<f64>,
    pub accuracy_user: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct EndgameTypeStat {
    pub kind: String,
    pub reached: i32,
    pub converted: i32,
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ConversionReport {
    pub threshold: f64,
    pub reached: i32,
    pub converted: i32,
    pub rate: f64,
    pub by_endgame_type: Vec<EndgameTypeStat>,
    pub thrown_games: Vec<GameRef>,
    pub biggest_win: Option<GameRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct TagCount {
    pub tag: String,
    pub label: String,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ReflectionReport {
    pub total: i32,
    pub top_reasons: Vec<TagCount>,
    pub shrinking: Vec<String>,
    pub persisting: Vec<String>,
    pub recent: Vec<ReflectionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct RoadmapStage {
    pub stage: i32,
    pub target_elo: i32,
    pub title: String,
    pub description: String,
    pub criteria: Vec<String>,
    pub completed: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct RoadmapReport {
    pub current_elo: Option<i32>,
    pub next_target: Option<i32>,
    pub progress_pct: f64,
    pub stages: Vec<RoadmapStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct WeeklyReport {
    pub id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub model: String,
    pub ai_status: String,
    pub summary: String,
    pub next_week_rule: String,
    pub top_leaks: Vec<RatingLeak>,
    pub action_plan: Vec<String>,
    #[ts(type = "unknown")]
    pub facts: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ReportSummary {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub model: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct CurriculumReport {
    pub focus: String,
    pub text: String,
    pub next_week_rule: String,
    pub ai_status: String,
    pub model: String,
    #[ts(type = "unknown")]
    pub facts: serde_json::Value,
}

pub fn tag_meta(tag: &str) -> (String, String) {
    match tag {
        "HANGING_PIECE" => (
            "Hanging pieces".to_string(),
            "Before each move, list the undefended pieces; do not leave one hanging.".to_string(),
        ),
        "MISSED_FREE_PIECE" => (
            "Missed free pieces".to_string(),
            "Scan every capture before moving. If the capture is safe, take it.".to_string(),
        ),
        "MISSED_MATE_1_2" => (
            "Missed mates in one or two".to_string(),
            "When a check appears, calculate checks first, then captures, then threats."
                .to_string(),
        ),
        "ALLOWED_MATE" => (
            "Allowed mate".to_string(),
            "After your candidate move, ask whether the opponent now has a forced mate."
                .to_string(),
        ),
        "TACTICAL_BLINDNESS" => (
            "Tactical blindness".to_string(),
            "Use the checks-captures-threats check for both players before every move.".to_string(),
        ),
        "WINNING_GAME_THROWN" => (
            "Winning games thrown".to_string(),
            "At +4 or better, simplify and trade pieces before looking for more.".to_string(),
        ),
        "STALEMATE_THROW" => (
            "Stalemates allowed".to_string(),
            "In a winning endgame, leave the opponent a legal move before every move.".to_string(),
        ),
        "ENDGAME_TECHNIQUE_FAIL" => (
            "Endgame technique".to_string(),
            "Practise king opposition, rook cuts and queen ladder mates.".to_string(),
        ),
        "IMPULSIVE_BLUNDER" => (
            "Impulsive blunders".to_string(),
            "If more than half your clock remains, spend at least four seconds per move."
                .to_string(),
        ),
        "TIME_SCRAMBLE_BLUNDER" => (
            "Time-scramble blunders".to_string(),
            "Protect 30 seconds of clock; simplify before the scramble starts.".to_string(),
        ),
        "OPENING_PRINCIPLE_BREAK" => (
            "Opening principles".to_string(),
            "Develop a new piece or castle for the first ten moves.".to_string(),
        ),
        other => (
            other.to_string(),
            "Review the tagged positions in Match Lab.".to_string(),
        ),
    }
}

pub async fn weakness_report(pool: &PgPool, user_id: Uuid) -> AppResult<WeaknessReport> {
    let total_user_moves = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM game_moves m JOIN games g ON g.id = m.game_id
         WHERE g.user_id = $1 AND g.analysis_status = 'done' AND m.color = g.user_color",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let total_blunders = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM game_moves m JOIN games g ON g.id = m.game_id
         WHERE g.user_id = $1 AND g.analysis_status = 'done' AND m.color = g.user_color
           AND m.classification IN ('blunder', 'missed_win')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query_as::<_, (String, i64, Option<f64>, i64, i64)>(
        "SELECT tag,
                COUNT(*) AS count,
                AVG(m.epl) AS avg_epl,
                COUNT(*) FILTER (WHERE g.start_time >= now() - interval '30 days') AS recent,
                COUNT(*) FILTER (
                    WHERE g.start_time < now() - interval '30 days'
                      AND g.start_time >= now() - interval '60 days'
                ) AS prior
         FROM games g
         JOIN game_moves m ON m.game_id = g.id
         CROSS JOIN LATERAL unnest(m.mistake_tags) AS tag
         WHERE g.user_id = $1 AND g.analysis_status = 'done' AND m.color = g.user_color
         GROUP BY tag
         ORDER BY count DESC, avg_epl DESC
         LIMIT 20",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let tags: Vec<RatingLeak> = rows
        .into_iter()
        .map(|(tag, count, avg_epl, recent, prior)| {
            let (label, advice) = tag_meta(&tag);
            let avg_epl = avg_epl.unwrap_or(0.0);
            let trend = if prior == 0 {
                "new".to_string()
            } else {
                let recent_rate = recent as f64 / prior as f64;
                if recent_rate < 0.7 {
                    "improving"
                } else if recent_rate > 1.2 {
                    "regressing"
                } else {
                    "flat"
                }
                .to_string()
            };
            RatingLeak {
                tag,
                label,
                count: count as i32,
                avg_epl,
                estimated_elo_cost: (count as f64 * avg_epl * 10.0).round(),
                trend,
                advice,
            }
        })
        .collect();

    let top_three = tags.iter().take(3).cloned().collect();

    Ok(WeaknessReport {
        total_user_moves: total_user_moves as i32,
        total_blunders: total_blunders as i32,
        tags,
        top_three,
    })
}

pub async fn opening_report(pool: &PgPool, user_id: Uuid) -> AppResult<OpeningReport> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            i64,
            Option<f64>,
            Option<f64>,
        ),
    >(
        "SELECT opening_eco, opening_name, user_color,
                COUNT(*) AS games,
                COUNT(*) FILTER (WHERE result = 'win') AS wins,
                COUNT(*) FILTER (WHERE result = 'draw') AS draws,
                COUNT(*) FILTER (WHERE result = 'loss') AS losses,
                AVG(accuracy_user) AS accuracy,
                AVG(COALESCE(last_book_ply, 0))::float8 AS out_of_book_ply
         FROM games
         WHERE user_id = $1 AND analysis_status = 'done' AND opening_eco IS NOT NULL
         GROUP BY opening_eco, opening_name, user_color
         ORDER BY games DESC, wins DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut white = Vec::new();
    let mut black = Vec::new();
    let mut recommendations = Vec::new();

    for (eco, name, color, games, wins, draws, losses, accuracy, out_ply) in rows {
        let win_rate = if games == 0 {
            0.0
        } else {
            wins as f64 / games as f64 * 100.0
        };
        let stat = OpeningStat {
            eco,
            name: name.unwrap_or_else(|| "Unnamed opening".into()),
            color: color.clone(),
            games: games as i32,
            wins: wins as i32,
            draws: draws as i32,
            losses: losses as i32,
            win_rate,
            accuracy: accuracy.unwrap_or(0.0),
            out_of_book_move: out_ply.unwrap_or(0.0) / 2.0,
            most_punished: games >= 3 && losses >= wins,
        };
        if color == "white" {
            white.push(stat);
        } else {
            black.push(stat);
        }
    }

    let weakest = white
        .iter()
        .chain(black.iter())
        .filter(|stat| stat.games >= 3)
        .min_by(|a, b| a.win_rate.total_cmp(&b.win_rate));

    if let Some(stat) = weakest {
        if stat.win_rate < 45.0 {
            recommendations.push(format!(
                "Trim {} ({}) first: {}% over {} games. Keep only the lines you can explain out loud.",
                stat.name, stat.eco, stat.win_rate.round(), stat.games
            ));
        }
    }
    if white.is_empty() {
        recommendations.push("No analysed White games yet. Import and analyse a few before changing your repertoire.".into());
    }
    if black.is_empty() {
        recommendations.push(
            "No analysed Black games yet. Pick one reply to 1.e4 and one to 1.d4, nothing more."
                .into(),
        );
    }
    let shallow = white
        .iter()
        .chain(black.iter())
        .filter(|stat| stat.games >= 3 && stat.out_of_book_move < 4.0)
        .count();
    if shallow > 0 {
        recommendations.push(
            "Several games leave the book before move 4. Learn a clear five-move setup rather than memorising long lines."
                .into(),
        );
    }
    if recommendations.is_empty() {
        recommendations.push(
            "Your repertoire is holding. Keep the same lines and spend the saved time on tactics."
                .into(),
        );
    }

    Ok(OpeningReport {
        white,
        black,
        recommendations,
    })
}

pub async fn conversion_report(pool: &PgPool, user_id: Uuid) -> AppResult<ConversionReport> {
    let threshold = 3.0;
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            DateTime<Utc>,
            Option<f64>,
            Option<f64>,
            Option<String>,
        ),
    >(
        "SELECT g.id, g.white_username, g.black_username, g.result, g.start_time,
                g.max_advantage, g.accuracy_user,
                (SELECT gm.fen_after FROM game_moves gm WHERE gm.game_id = g.id
                 ORDER BY gm.ply DESC LIMIT 1) AS final_fen
         FROM games g
         WHERE g.user_id = $1 AND g.analysis_status = 'done' AND g.max_advantage >= $2
         ORDER BY g.start_time DESC",
    )
    .bind(user_id)
    .bind(threshold)
    .fetch_all(pool)
    .await?;

    let reached = rows.len() as i32;
    let mut converted = 0i32;
    let mut by_kind: HashMap<String, (i32, i32)> = HashMap::new();
    let mut thrown = Vec::new();
    let mut biggest_win: Option<(f64, GameRef)> = None;

    for (id, white, black, result, start_time, max_advantage, accuracy, fen) in rows {
        let game_ref = GameRef {
            id,
            white_username: white,
            black_username: black,
            result: result.clone(),
            start_time,
            max_advantage,
            accuracy_user: accuracy,
        };
        if result == "win" {
            converted += 1;
            if let Some(adv) = max_advantage {
                if biggest_win
                    .as_ref()
                    .map(|(best, _)| adv > *best)
                    .unwrap_or(true)
                {
                    biggest_win = Some((adv, game_ref.clone()));
                }
            }
        } else if max_advantage.unwrap_or(0.0) >= 4.0 && thrown.len() < 10 {
            thrown.push(game_ref.clone());
        }
        let kind = fen
            .as_deref()
            .map(endgame_kind)
            .unwrap_or("unknown")
            .to_string();
        let entry = by_kind.entry(kind).or_insert((0, 0));
        entry.0 += 1;
        if result == "win" {
            entry.1 += 1;
        }
    }

    let by_endgame_type = by_kind
        .into_iter()
        .map(|(kind, (reached, converted))| EndgameTypeStat {
            kind,
            reached,
            converted,
            rate: if reached == 0 {
                0.0
            } else {
                converted as f64 / reached as f64 * 100.0
            },
        })
        .collect();

    Ok(ConversionReport {
        threshold,
        reached,
        converted,
        rate: if reached == 0 {
            0.0
        } else {
            converted as f64 / reached as f64 * 100.0
        },
        by_endgame_type,
        thrown_games: thrown,
        biggest_win: biggest_win.map(|(_, game)| game),
    })
}

fn endgame_kind(fen: &str) -> &'static str {
    let Ok(fen) = shakmaty::fen::Fen::from_ascii(fen.as_bytes()) else {
        return "unknown";
    };
    let board = fen.into_setup().board;
    let material = board.material();
    let queens = material.white.queen + material.black.queen;
    let rooks = material.white.rook + material.black.rook;
    let minors = material.white.bishop
        + material.white.knight
        + material.black.bishop
        + material.black.knight;
    if queens > 0 {
        "queen"
    } else if rooks > 0 {
        "rook"
    } else if minors > 0 {
        "minor"
    } else {
        "pawn"
    }
}

pub async fn reflection_report(pool: &PgPool, user_id: Uuid) -> AppResult<ReflectionReport> {
    let total =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM move_reflections WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    let rows = sqlx::query_as::<_, (Vec<String>, DateTime<Utc>)>(
        "SELECT reason_tags, created_at FROM move_reflections WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut counts: HashMap<String, i32> = HashMap::new();
    let mut recent: HashMap<String, i32> = HashMap::new();
    let mut prior: HashMap<String, i32> = HashMap::new();
    let now = Utc::now();
    for (tags, created_at) in &rows {
        let age = now.signed_duration_since(*created_at).num_days();
        for tag in tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
            if age < 30 {
                *recent.entry(tag.clone()).or_insert(0) += 1;
            } else if age < 60 {
                *prior.entry(tag.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut top_reasons: Vec<TagCount> = counts
        .into_iter()
        .map(|(tag, count)| TagCount {
            label: tag_meta(&tag).0,
            tag,
            count,
        })
        .collect();
    top_reasons.sort_by_key(|a| std::cmp::Reverse(a.count));
    top_reasons.truncate(6);

    let mut shrinking = Vec::new();
    let mut persisting = Vec::new();
    for (tag, recent_count) in &recent {
        let previous = prior.get(tag).copied().unwrap_or(0);
        if previous > 0 && *recent_count < previous {
            shrinking.push(tag_meta(tag).0);
        } else if previous > 0 && *recent_count >= previous {
            persisting.push(tag_meta(tag).0);
        }
    }

    let recent_rows = sqlx::query_as::<_, ReflectionDto>(
        "SELECT id, move_id, reason_tags, explanation, ai_feedback, next_time_rule,
                ai_status, created_at
         FROM move_reflections WHERE user_id = $1 ORDER BY created_at DESC LIMIT 8",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(ReflectionReport {
        total: total as i32,
        top_reasons,
        shrinking,
        persisting,
        recent: recent_rows,
    })
}

pub async fn roadmap_report(pool: &PgPool, user_id: Uuid) -> AppResult<RoadmapReport> {
    let current_elo: Option<i32> =
        sqlx::query_scalar("SELECT current_elo FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten();

    let rating = current_elo.unwrap_or(500);
    let next_target = tiers_next_target(rating);
    let stages = ladder_stages(rating);
    let lower = stages
        .iter()
        .filter(|stage| stage.target_elo <= rating)
        .map(|stage| stage.target_elo)
        .max()
        .unwrap_or(500);
    let upper = next_target.unwrap_or(lower);
    let progress_pct = if upper <= lower {
        100.0
    } else {
        let current = f64::from(current_elo.unwrap_or(lower));
        ((current - f64::from(lower)) / f64::from(upper - lower) * 100.0).clamp(0.0, 100.0)
    };

    Ok(RoadmapReport {
        current_elo,
        next_target,
        progress_pct,
        stages,
    })
}

fn tiers_next_target(current_elo: i32) -> Option<i32> {
    [500, 650, 800, 1000, 1200, 1500, 1800, 2200, 2500]
        .into_iter()
        .find(|target| *target > current_elo)
}

pub fn ladder_stages(current_elo: i32) -> Vec<RoadmapStage> {
    let tiers = [
        (
            1,
            500,
            "Foundation",
            "Stop giving away free pieces",
            vec![
                "Hanging-piece rate below 1.5 per game over the last 10 Rapid games",
                "90% accuracy on mate-in-one drills",
                "Convert 3 of 3 king-and-queen endgame drills",
            ],
        ),
        (
            2,
            650,
            "Tactical radar",
            "See the two-move tactic before it lands",
            vec![
                "Castle before move 12 in 80% of games",
                "Impulsive blunder rate below 15%",
                "Master 20 personal blunder puzzles",
            ],
        ),
        (
            3,
            800,
            "Conversion",
            "Finish what you start",
            vec![
                "Convert 75% of positions at +4 or better",
                "One White and two Black openings at 50%+ win rate",
                "Pass king-and-rook and king-and-pawn drills",
            ],
        ),
        (
            4,
            1000,
            "Structure",
            "Play with a plan, not a hope",
            vec![
                "Name the pawn structure in every review",
                "Accuracy above 80% in the middlegame",
                "No opening principle break in 8 of 10 games",
            ],
        ),
        (
            5,
            1200,
            "Calculation",
            "Calculate three moves, not one",
            vec![
                "Turning-point accuracy above 75%",
                "Solve 30 two-move tactics without hints",
                "Convert 4 of 5 rook endgames",
            ],
        ),
        (
            6,
            1500,
            "Repertoire",
            "A repeatable White and Black identity",
            vec![
                "Play the same main line in 80% of games",
                "Win rate above 55% in your main White opening",
                "No repeated piece opening errors for 20 games",
            ],
        ),
        (
            7,
            1800,
            "Prophylaxis",
            "See the opponent's idea first",
            vec![
                "Reduce tactical blindness tags by 50%",
                "Convert 80% of winning positions",
                "Complete the rook-and-pawn endgame module",
            ],
        ),
        (
            8,
            2200,
            "Mastery",
            "Deep calculation and tournament stamina",
            vec![
                "Hold 85%+ accuracy across a five-game session",
                "Prepare two sharp lines to move 15",
                "Win a 20-game Rapid block against 2100+ opposition",
            ],
        ),
        (
            9,
            2500,
            "Grandmaster ladder",
            "Sustain the whole system",
            vec![
                "Sustain sub-10% impulsivity across 50 games",
                "Convert every +4 position for a month",
                "Build and defend a full repertoire to move 20",
            ],
        ),
    ];

    let next_target = tiers
        .iter()
        .filter(|(_, target, _, _, _)| *target > current_elo)
        .map(|(_, target, _, _, _)| *target)
        .min();

    tiers
        .into_iter()
        .map(|(stage, target, title, description, criteria)| {
            let completed = target <= current_elo;
            let current = !completed && Some(target) == next_target;
            RoadmapStage {
                stage,
                target_elo: target,
                title: title.to_string(),
                description: description.to_string(),
                criteria: criteria.into_iter().map(str::to_string).collect(),
                completed,
                current,
            }
        })
        .collect()
}

pub async fn weekly_facts(pool: &PgPool, user_id: Uuid) -> AppResult<serde_json::Value> {
    let recent = sqlx::query_as::<_, (i64, i64, i64, i64, Option<f64>)>(
        "SELECT COUNT(*),
                COUNT(*) FILTER (WHERE result = 'win'),
                COUNT(*) FILTER (WHERE result = 'loss'),
                COUNT(*) FILTER (WHERE result = 'draw'),
                AVG(accuracy_user)
         FROM games
         WHERE user_id = $1 AND analysis_status = 'done'
           AND start_time >= now() - interval '7 days'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let prior = sqlx::query_as::<_, (i64, Option<f64>)>(
        "SELECT COUNT(*), AVG(accuracy_user)
         FROM games WHERE user_id = $1 AND analysis_status = 'done'
           AND start_time < now() - interval '7 days'
           AND start_time >= now() - interval '14 days'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let due_puzzles = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM puzzles WHERE user_id = $1 AND mastered = false AND due_at <= now()",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let current_elo: Option<i32> =
        sqlx::query_scalar("SELECT current_elo FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .flatten();

    let weakness = weakness_report(pool, user_id).await?;
    let top_leaks: Vec<serde_json::Value> = weakness
        .top_three
        .iter()
        .map(|leak| {
            serde_json::json!({
                "tag": leak.tag,
                "label": leak.label,
                "count": leak.count,
                "avg_epl": leak.avg_epl,
                "estimated_elo_cost": leak.estimated_elo_cost,
                "trend": leak.trend,
            })
        })
        .collect();

    let biggest_win = sqlx::query_as::<_, (Uuid, String, String, Option<f64>, DateTime<Utc>)>(
        "SELECT id, white_username, black_username, max_advantage, start_time
         FROM games
         WHERE user_id = $1 AND result = 'win' AND start_time >= now() - interval '7 days'
         ORDER BY COALESCE(max_advantage, 0) DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .map(|(id, white, black, advantage, start_time)| {
        serde_json::json!({
            "id": id,
            "white": white,
            "black": black,
            "advantage": advantage,
            "start_time": start_time,
        })
    });

    Ok(serde_json::json!({
        "window_days": 7,
        "games": recent.0,
        "wins": recent.1,
        "losses": recent.2,
        "draws": recent.3,
        "accuracy": recent.4,
        "prior_games": prior.0,
        "prior_accuracy": prior.1,
        "due_puzzles": due_puzzles,
        "current_elo": current_elo,
        "top_leaks": top_leaks,
        "biggest_win": biggest_win,
        "total_blunders": weakness.total_blunders,
        "ladder_stage": ladder_stage_number(current_elo.unwrap_or(500)),
    }))
}

pub fn ladder_stage_number(elo: i32) -> i32 {
    match elo {
        ..500 => 1,
        500..=649 => 1,
        650..=799 => 2,
        800..=999 => 3,
        1000..=1199 => 4,
        1200..=1499 => 5,
        1500..=1799 => 6,
        1800..=2199 => 7,
        _ => 8,
    }
}

pub fn fallback_weekly_summary(
    facts: &serde_json::Value,
    leaks: &[RatingLeak],
) -> (String, Vec<String>) {
    let games = facts
        .get("games")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let wins = facts
        .get("wins")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let accuracy = facts
        .get("accuracy")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    let due = facts
        .get("due_puzzles")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);

    let leak_lines = if leaks.is_empty() {
        "No tagged mistakes yet. Import a few analysed games first.".to_string()
    } else {
        leaks
            .iter()
            .map(|leak| {
                format!(
                    "- **{}**: {} occurrence(s), estimated {} Elo points.",
                    leak.label,
                    leak.count,
                    leak.estimated_elo_cost.round()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let primary = leaks
        .first()
        .map(|leak| leak.advice.clone())
        .unwrap_or_else(|| {
            "Start with the checks-captures-threats habit on every move.".to_string()
        });

    let summary = format!(
        "**This week**\n{games} analysed game(s), {wins} win(s), {accuracy:.0}% average accuracy. The trend is measured, not guessed.\n\n**Top leaks**\n{leak_lines}\n\n**The one thing**\n{primary}\n\n**Seven-day plan**\n1. Complete {due} due blunder puzzle(s).\n2. Review one lost game in Match Lab and write two reflections.\n3. Play three Rapid games applying the next-time rule before every move."
    );

    let action_plan = vec![
        format!("Complete {due} due blunder puzzle(s)."),
        primary,
        "Play three Rapid games with one pre-move habit check.".to_string(),
    ];
    (summary, action_plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_meta_covers_known_tags() {
        for tag in [
            "HANGING_PIECE",
            "MISSED_FREE_PIECE",
            "MISSED_MATE_1_2",
            "ALLOWED_MATE",
            "TACTICAL_BLINDNESS",
            "WINNING_GAME_THROWN",
            "STALEMATE_THROW",
            "ENDGAME_TECHNIQUE_FAIL",
            "IMPULSIVE_BLUNDER",
            "TIME_SCRAMBLE_BLUNDER",
            "OPENING_PRINCIPLE_BREAK",
        ] {
            let (label, advice) = tag_meta(tag);
            assert_ne!(label, tag);
            assert!(!advice.is_empty());
        }
    }

    #[test]
    fn endgame_kind_reads_material() {
        assert_eq!(endgame_kind("8/8/4k3/8/8/4K3/4P3/8 w - - 0 1"), "pawn");
        assert_eq!(endgame_kind("8/8/4k3/8/8/4K3/4R3/8 w - - 0 1"), "rook");
        assert_eq!(endgame_kind("8/8/4k3/8/8/4K3/4Q3/8 w - - 0 1"), "queen");
    }

    #[test]
    fn fallback_weekly_is_grounded() {
        let facts = serde_json::json!({"games": 4, "wins": 1, "accuracy": 72.0, "due_puzzles": 6});
        let leak = RatingLeak {
            tag: "HANGING_PIECE".into(),
            label: "Hanging pieces".into(),
            count: 3,
            avg_epl: 0.3,
            estimated_elo_cost: 9.0,
            trend: "new".into(),
            advice: "Check defenders.".into(),
        };
        let (summary, actions) = fallback_weekly_summary(&facts, &[leak]);
        assert!(summary.contains("4 analysed game(s)"));
        assert!(summary.contains("Hanging pieces"));
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn ladder_marks_current_stage() {
        let stages = ladder_stages(840);
        assert!(stages[0].completed);
        assert!(stages[2].completed);
        assert!(!stages[3].completed);
        assert!(stages[3].current);
    }
}
