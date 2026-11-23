use std::collections::HashMap;

use chrono::Timelike;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    analyzer::winprob::{accuracy_from_epl, is_bad},
    error::{AppError, AppResult},
};

const EXPORT: &str = "../../frontend/src/api/bindings.ts";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct HourAccuracy {
    pub hour: i32,
    pub games: i32,
    pub accuracy: f64,
    pub win_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct DepthAccuracy {
    pub label: String,
    pub moves: i32,
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct TimeBucket {
    pub label: String,
    pub midpoint_seconds: f64,
    pub moves: i32,
    pub blunder_rate: f64,
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SessionIndexRate {
    pub index: i32,
    pub games: i32,
    pub win_rate: f64,
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct FocusReport {
    pub timezone: String,
    pub total_user_moves: i32,
    pub impulsive_blunders: i32,
    pub impulsivity_index: f64,
    pub blunder_count: i32,
    pub cascade_instances: i32,
    pub post_blunder_cascade_rate: f64,
    pub sessions_analyzed: i32,
    pub game1_accuracy: f64,
    pub game3plus_accuracy: f64,
    pub session_tilt_drop: f64,
    pub hour_heatmap: Vec<HourAccuracy>,
    pub depth_fatigue: Vec<DepthAccuracy>,
    pub session_curve: Vec<SessionIndexRate>,
    pub time_buckets: Vec<TimeBucket>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct FocusMove {
    game_id: Uuid,
    time_class: Option<String>,
    time_control: Option<String>,
    ply: i32,
    time_spent_sec: Option<f64>,
    clock_remaining_sec: Option<f64>,
    classification: Option<String>,
    epl: Option<f64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct FocusGame {
    game_id: Uuid,
    session_id: Option<Uuid>,
    start_time: chrono::DateTime<chrono::Utc>,
    accuracy_user: Option<f64>,
    result: String,
}

pub async fn resolve_timezone(
    pool: &PgPool,
    user_id: Uuid,
    requested: Option<&str>,
) -> AppResult<String> {
    let timezone = match requested {
        Some(timezone) if !timezone.trim().is_empty() => timezone.trim().to_string(),
        _ => sqlx::query_scalar::<_, Option<String>>(
            "SELECT timezone FROM notification_settings WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .flatten()
        .unwrap_or_else(|| "UTC".to_string()),
    };
    timezone
        .parse::<Tz>()
        .map_err(|_| AppError::bad_request("Unknown IANA timezone."))?;
    Ok(timezone)
}

pub async fn compute_focus(pool: &PgPool, user_id: Uuid, timezone: &str) -> AppResult<FocusReport> {
    let tz: Tz = timezone
        .parse()
        .map_err(|_| AppError::bad_request("Unknown IANA timezone."))?;
    let moves = sqlx::query_as::<_, FocusMove>(
        "SELECT g.id AS game_id, g.time_class, g.time_control,
                m.ply, m.time_spent_sec, m.clock_remaining_sec,
                m.classification, m.epl
         FROM games g
         JOIN game_moves m ON m.game_id = g.id
         WHERE g.user_id = $1 AND g.analysis_status = 'done' AND m.color = g.user_color
         ORDER BY g.start_time ASC, m.ply ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let games = sqlx::query_as::<_, FocusGame>(
        "SELECT id AS game_id, session_id, start_time, accuracy_user, result
         FROM games WHERE user_id = $1 AND analysis_status = 'done'
         ORDER BY start_time ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let total_user_moves = moves.len() as i32;
    let mut impulsive = 0i32;
    let mut blunders = 0i32;
    let mut cascades = 0i32;
    let mut per_game_blunders: HashMap<Uuid, Vec<i32>> = HashMap::new();

    for mv in &moves {
        let initial = parse_initial_time(mv.time_control.as_deref());
        let rapid = mv
            .time_class
            .as_deref()
            .map(|class| class.eq_ignore_ascii_case("rapid"))
            .unwrap_or(false);
        let is_bad_move = mv.classification.as_deref().map(is_bad).unwrap_or(false);

        if rapid
            && is_bad_move
            && mv.time_spent_sec.is_some_and(|seconds| seconds < 4.0)
            && initial.is_some_and(|base| {
                mv.clock_remaining_sec
                    .map(|clock| base > 0.0 && clock / base > 0.5)
                    .unwrap_or(false)
            })
        {
            impulsive += 1;
        }

        if mv
            .classification
            .as_deref()
            .is_some_and(|class| matches!(class, "blunder" | "missed_win"))
        {
            blunders += 1;
            per_game_blunders
                .entry(mv.game_id)
                .or_default()
                .push(mv.ply);
        }
    }

    for plies in per_game_blunders.values_mut() {
        plies.sort_unstable();
        for window in plies.windows(2) {
            if let [first, second] = window {
                if second - first <= 6 {
                    cascades += 1;
                }
            }
        }
    }

    let impulsivity_index = if total_user_moves == 0 {
        0.0
    } else {
        f64::from(impulsive) / f64::from(total_user_moves)
    };
    let post_blunder_cascade_rate = if blunders == 0 {
        0.0
    } else {
        f64::from(cascades) / f64::from(blunders)
    };

    let mut hours: HashMap<i32, Vec<f64>> = HashMap::new();
    let mut hour_wins: HashMap<i32, (i32, i32)> = HashMap::new();
    for game in &games {
        let hour = game.start_time.with_timezone(&tz).hour() as i32;
        if let Some(accuracy) = game.accuracy_user {
            hours.entry(hour).or_default().push(accuracy);
        }
        let entry = hour_wins.entry(hour).or_insert((0, 0));
        entry.0 += 1;
        if game.result == "win" {
            entry.1 += 1;
        }
    }
    let hour_heatmap = (0..24)
        .map(|hour| {
            let accuracies = hours.get(&hour).cloned().unwrap_or_default();
            let accuracy = mean(&accuracies);
            let (games, wins) = hour_wins.get(&hour).copied().unwrap_or((0, 0));
            HourAccuracy {
                hour,
                games,
                accuracy,
                win_rate: if games == 0 {
                    0.0
                } else {
                    f64::from(wins) / f64::from(games) * 100.0
                },
            }
        })
        .collect();

    let depth_fatigue = vec![
        bucket("1–15", &moves, 1, 15),
        bucket("16–30", &moves, 16, 30),
        bucket("31+", &moves, 31, i32::MAX),
    ];

    let time_ranges = [
        ("<1s", 0.0, 1.0, 0.5),
        ("1–2s", 1.0, 2.0, 1.5),
        ("2–4s", 2.0, 4.0, 3.0),
        ("4–8s", 4.0, 8.0, 6.0),
        ("8–15s", 8.0, 15.0, 11.5),
        ("15s+", 15.0, f64::MAX, 22.0),
    ];
    let time_buckets = time_ranges
        .into_iter()
        .map(|(label, min, max, midpoint)| {
            let filtered: Vec<&FocusMove> = moves
                .iter()
                .filter(|mv| {
                    mv.time_spent_sec
                        .map(|seconds| seconds >= min && seconds < max)
                        .unwrap_or(false)
                })
                .collect();
            let bad = filtered
                .iter()
                .filter(|mv| mv.classification.as_deref().map(is_bad).unwrap_or(false))
                .count();
            let accuracy = mean(
                &filtered
                    .iter()
                    .filter_map(|mv| mv.epl)
                    .map(accuracy_from_epl)
                    .collect::<Vec<_>>(),
            );
            TimeBucket {
                label: label.to_string(),
                midpoint_seconds: midpoint,
                moves: filtered.len() as i32,
                blunder_rate: if filtered.is_empty() {
                    0.0
                } else {
                    bad as f64 / filtered.len() as f64 * 100.0
                },
                accuracy,
            }
        })
        .collect();

    let sessions = group_sessions(&games);
    let mut game1: Vec<f64> = Vec::new();
    let mut game3plus: Vec<f64> = Vec::new();
    let mut curve_acc: HashMap<i32, Vec<f64>> = HashMap::new();
    let mut curve_wins: HashMap<i32, (i32, i32)> = HashMap::new();

    for session in sessions.values() {
        for (index, game) in session.iter().enumerate() {
            let slot = (index + 1).min(3) as i32;
            if let Some(accuracy) = game.accuracy_user {
                curve_acc.entry(slot).or_default().push(accuracy);
                if index == 0 {
                    game1.push(accuracy);
                } else if index >= 2 {
                    game3plus.push(accuracy);
                }
            }
            let entry = curve_wins.entry(slot).or_insert((0, 0));
            entry.0 += 1;
            if game.result == "win" {
                entry.1 += 1;
            }
        }
    }

    let game1_accuracy = mean(&game1);
    let game3plus_accuracy = mean(&game3plus);
    let session_tilt_drop = if game3plus.is_empty() {
        0.0
    } else {
        game1_accuracy - game3plus_accuracy
    };
    let session_curve = (1..=3)
        .map(|index| {
            let accuracies = curve_acc.get(&index).cloned().unwrap_or_default();
            let (games, wins) = curve_wins.get(&index).copied().unwrap_or((0, 0));
            SessionIndexRate {
                index,
                games,
                accuracy: mean(&accuracies),
                win_rate: if games == 0 {
                    0.0
                } else {
                    f64::from(wins) / f64::from(games) * 100.0
                },
            }
        })
        .collect();

    Ok(FocusReport {
        timezone: timezone.to_string(),
        total_user_moves,
        impulsive_blunders: impulsive,
        impulsivity_index,
        blunder_count: blunders,
        cascade_instances: cascades,
        post_blunder_cascade_rate,
        sessions_analyzed: sessions.len() as i32,
        game1_accuracy,
        game3plus_accuracy,
        session_tilt_drop,
        hour_heatmap,
        depth_fatigue,
        session_curve,
        time_buckets,
    })
}

fn group_sessions(games: &[FocusGame]) -> HashMap<Uuid, Vec<FocusGame>> {
    let mut grouped: HashMap<Uuid, Vec<FocusGame>> = HashMap::new();
    for game in games {
        grouped
            .entry(game.session_id.unwrap_or(game.game_id))
            .or_default()
            .push(game.clone());
    }
    grouped
}

fn bucket(label: &str, moves: &[FocusMove], min: i32, max: i32) -> DepthAccuracy {
    let filtered: Vec<f64> = moves
        .iter()
        .filter(|m| m.ply >= min && m.ply <= max)
        .filter_map(|m| m.epl)
        .map(accuracy_from_epl)
        .collect();
    DepthAccuracy {
        label: label.to_string(),
        moves: filtered.len() as i32,
        accuracy: mean(&filtered),
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn parse_initial_time(time_control: Option<&str>) -> Option<f64> {
    let raw = time_control?;
    let base = raw.split('+').next()?.trim();
    base.parse::<f64>().ok().filter(|v| *v > 0.0)
}
