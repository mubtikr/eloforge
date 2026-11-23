use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    analyzer::focus_tilt::{compute_focus, resolve_timezone},
    error::AppResult,
    reports,
};

pub async fn seed_roadmap(db: &PgPool, user_id: Uuid) -> AppResult<()> {
    let existing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM milestones_and_goals WHERE user_id = $1 AND kind = 'milestone'",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?;
    if existing > 0 {
        return Ok(());
    }

    for stage in reports::ladder_stages(500) {
        let progress = serde_json::json!({
            "criteria": stage.criteria,
            "metrics": {},
            "criteria_met": false,
        });
        sqlx::query(
            "INSERT INTO milestones_and_goals
                (user_id, kind, stage, title, description, target_elo, progress)
             VALUES ($1,'milestone',$2,$3,$4,$5,$6)",
        )
        .bind(user_id)
        .bind(stage.stage)
        .bind(format!("{} — {}", stage.target_elo, stage.title))
        .bind(&stage.description)
        .bind(stage.target_elo)
        .bind(progress)
        .execute(db)
        .await?;
    }
    Ok(())
}

pub async fn recompute_roadmap(db: &PgPool, user_id: Uuid) -> AppResult<()> {
    seed_roadmap(db, user_id).await?;

    let current_elo: Option<i32> =
        sqlx::query_scalar("SELECT current_elo FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await?
            .flatten();
    let elo = current_elo.unwrap_or(500);

    let timezone = resolve_timezone(db, user_id, None).await?;
    let focus = compute_focus(db, user_id, &timezone).await?;
    let weakness = reports::weakness_report(db, user_id).await?;
    let conversion = reports::conversion_report(db, user_id).await?;

    let done_games: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM games WHERE user_id = $1 AND analysis_status = 'done'",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?;
    let hanging = weakness
        .tags
        .iter()
        .find(|tag| tag.tag == "HANGING_PIECE")
        .map(|tag| tag.count)
        .unwrap_or(0);
    let hanging_rate = if done_games == 0 {
        0.0
    } else {
        f64::from(hanging) / done_games as f64 * 10.0
    };

    let uncastled: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM games
         WHERE user_id = $1 AND analysis_status = 'done'
           AND jsonb_typeof(analysis->'violations') = 'array'
           AND EXISTS (
             SELECT 1 FROM jsonb_array_elements(analysis->'violations') AS v
             WHERE v->>'code' = 'UNCASTLED_KING'
           )",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?;
    let castle_rate = if done_games == 0 {
        1.0
    } else {
        1.0 - (uncastled as f64 / done_games as f64)
    };

    let puzzles_mastered: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM puzzles WHERE user_id = $1 AND mastered = true")
            .bind(user_id)
            .fetch_one(db)
            .await?;

    let metrics = serde_json::json!({
        "current_elo": current_elo,
        "hanging_rate_per_10_games": hanging_rate,
        "castle_before_12_rate": castle_rate,
        "impulsivity_index": focus.impulsivity_index,
        "conversion_rate": conversion.rate,
        "puzzles_mastered": puzzles_mastered,
        "analysed_games": done_games,
    });

    let rows = sqlx::query_as::<_, (Uuid, i32, i32)>(
        "SELECT id, stage, target_elo FROM milestones_and_goals
         WHERE user_id = $1 AND kind = 'milestone' ORDER BY stage",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    for (id, stage, target_elo) in rows {
        let criteria_met = match stage {
            1 => hanging_rate < 1.5 && puzzles_mastered >= 3 && elo >= 500,
            2 => castle_rate >= 0.8 && focus.impulsivity_index < 0.15 && puzzles_mastered >= 20,
            3 => conversion.rate >= 75.0 && elo >= 800,
            _ => elo >= target_elo,
        };
        let completed = criteria_met || elo >= target_elo;
        let progress = serde_json::json!({
            "metrics": metrics,
            "criteria_met": criteria_met,
            "updated_at": chrono::Utc::now(),
        });
        sqlx::query(
            "UPDATE milestones_and_goals
             SET completed = $2, progress = $3 WHERE id = $1",
        )
        .bind(id)
        .bind(completed)
        .bind(progress)
        .execute(db)
        .await?;
    }

    Ok(())
}

pub async fn roadmap_view(db: &PgPool, user_id: Uuid) -> AppResult<crate::models::RoadmapView> {
    recompute_roadmap(db, user_id).await?;

    let current_elo: Option<i32> =
        sqlx::query_scalar("SELECT current_elo FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await?
            .flatten();
    let rating = current_elo.unwrap_or(500);

    let rows = sqlx::query_as::<_, (i32, String, String, i32, bool, serde_json::Value)>(
        "SELECT stage, title, COALESCE(description, ''), target_elo, completed, progress
         FROM milestones_and_goals
         WHERE user_id = $1 AND kind = 'milestone'
         ORDER BY stage",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    let next_target = rows
        .iter()
        .filter(|(_, _, _, target, completed, _)| !completed && *target > 0)
        .map(|(_, _, _, target, _, _)| *target)
        .min();
    let current_stage = rows
        .iter()
        .find(|(_, _, _, _, completed, _)| !completed)
        .map(|(stage, _, _, _, _, _)| *stage);

    let lower = rows
        .iter()
        .filter(|(_, _, _, target, _, _)| *target <= rating)
        .map(|(_, _, _, target, _, _)| *target)
        .max()
        .unwrap_or(500);
    let upper = next_target.unwrap_or(lower);
    let progress_pct = if upper <= lower {
        100.0
    } else {
        ((f64::from(rating) - f64::from(lower)) / f64::from(upper - lower) * 100.0)
            .clamp(0.0, 100.0)
    };

    let metrics = rows
        .first()
        .and_then(|(_, _, _, _, _, progress)| progress.get("metrics").cloned())
        .unwrap_or(serde_json::Value::Null);

    let stages = rows
        .into_iter()
        .map(
            |(stage, title, description, target_elo, completed, progress)| {
                let criteria: Vec<String> = progress
                    .get("criteria")
                    .and_then(|value| serde_json::from_value(value.clone()).ok())
                    .unwrap_or_default();
                let criteria_met = progress
                    .get("criteria_met")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                crate::models::RoadmapStageView {
                    stage,
                    target_elo,
                    title: title.clone(),
                    description: description.clone(),
                    criteria: if criteria.is_empty() {
                        crate::reports::ladder_stages(rating)
                            .into_iter()
                            .find(|candidate| candidate.stage == stage)
                            .map(|candidate| candidate.criteria)
                            .unwrap_or_default()
                    } else {
                        criteria
                    },
                    completed,
                    current: Some(stage) == current_stage,
                    rating_reached: target_elo <= rating,
                    criteria_met,
                    progress,
                }
            },
        )
        .collect();

    Ok(crate::models::RoadmapView {
        current_elo,
        next_target,
        progress_pct,
        metrics,
        stages,
    })
}
