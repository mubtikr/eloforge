use std::collections::HashSet;

use chrono::Utc;
use chrono_tz::Tz;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    ai::provider::LlmRequest,
    ai::{
        coach::{extract_rule, strip_rule_line, validates_moves},
        prompts, stable_session, AiService,
    },
    error::AppResult,
    reports::{
        fallback_weekly_summary, ladder_stage_number, weakness_report, weekly_facts,
        CurriculumReport, RatingLeak, WeeklyReport,
    },
};

pub async fn generate_weekly(
    db: &PgPool,
    ai: &AiService,
    user_id: Uuid,
) -> AppResult<WeeklyReport> {
    let facts = weekly_facts(db, user_id).await?;
    let weakness = weakness_report(db, user_id).await?;
    let top_leaks = weakness.top_three.clone();
    let (fallback_summary, action_plan) = fallback_weekly_summary(&facts, &top_leaks);
    let fallback_rule = top_leaks
        .first()
        .map(|leak| leak.advice.clone())
        .unwrap_or_else(|| "Run the checks-captures-threats check before every move.".to_string());

    let mut summary = fallback_summary;
    let mut next_week_rule = fallback_rule.clone();
    let mut ai_status = if ai.is_configured() {
        "pending".to_string()
    } else {
        "unavailable".to_string()
    };
    let mut model = ai.model().to_string();

    if ai.is_configured() {
        let session = stable_session(&format!(
            "weekly:{}:{}",
            user_id,
            Utc::now().format("%G-W%V")
        ));
        let prompt =
            prompts::weekly_prompt(&serde_json::to_string_pretty(&facts).unwrap_or_default());
        let request = LlmRequest::new(session, "weekly_briefing")
            .system(prompt)
            .user("Write the weekly briefing now, following the system instructions.")
            .max_tokens(650);
        match ai.complete(user_id, request).await {
            Ok((text, _tokens)) if !text.trim().is_empty() => {
                if validates_moves(&text, &HashSet::new()) {
                    next_week_rule = extract_rule(&text, &fallback_rule);
                    summary = strip_rule_line(&text);
                    ai_status = "ok".to_string();
                } else {
                    ai_status = "rejected".to_string();
                }
            }
            Ok(_) => ai_status = "fallback".to_string(),
            Err(err) => {
                tracing::warn!(error = %err, "weekly briefing AI failed; using deterministic summary");
                ai_status = "fallback".to_string();
                ai.record_failure().await;
            }
        }
    }

    if ai_status != "ok" {
        model = "deterministic".to_string();
    }

    let content = json!({
        "summary": summary,
        "next_week_rule": next_week_rule,
        "top_leaks": top_leaks,
        "action_plan": action_plan,
        "facts": facts,
        "ai_status": ai_status,
        "model": model,
    });

    let timezone = sqlx::query_scalar::<_, Option<String>>(
        "SELECT timezone FROM notification_settings WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .flatten()
    .and_then(|value| value.parse::<Tz>().ok())
    .unwrap_or(chrono_tz::UTC);
    let title = format!(
        "Weekly briefing · {}",
        Utc::now().with_timezone(&timezone).format("%Y-%m-%d")
    );
    let (id, generated_at): (Uuid, chrono::DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO coaching_reports (user_id, kind, title, content, model)
         VALUES ($1, 'weekly', $2, $3, $4) RETURNING id, created_at",
    )
    .bind(user_id)
    .bind(&title)
    .bind(&content)
    .bind(&model)
    .fetch_one(db)
    .await?;

    Ok(WeeklyReport {
        id,
        generated_at,
        model,
        ai_status,
        summary,
        next_week_rule,
        top_leaks,
        action_plan,
        facts,
    })
}

pub async fn latest_weekly(db: &PgPool, user_id: Uuid) -> AppResult<Option<WeeklyReport>> {
    let row = sqlx::query_as::<_, (Uuid, chrono::DateTime<Utc>, serde_json::Value)>(
        "SELECT id, created_at, content FROM coaching_reports
         WHERE user_id = $1 AND kind = 'weekly'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    let Some((id, generated_at, content)) = row else {
        return Ok(None);
    };
    Ok(Some(weekly_from_content(id, generated_at, &content)))
}

fn weekly_from_content(
    id: Uuid,
    generated_at: chrono::DateTime<Utc>,
    content: &serde_json::Value,
) -> WeeklyReport {
    let text = |key: &str| {
        content
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let top_leaks: Vec<RatingLeak> = content
        .get("top_leaks")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    let action_plan: Vec<String> = content
        .get("action_plan")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    WeeklyReport {
        id,
        generated_at,
        model: text("model"),
        ai_status: text("ai_status"),
        summary: text("summary"),
        next_week_rule: text("next_week_rule"),
        top_leaks,
        action_plan,
        facts: content
            .get("facts")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    }
}

pub async fn generate_curriculum(
    db: &PgPool,
    ai: &AiService,
    user_id: Uuid,
) -> AppResult<CurriculumReport> {
    let facts = weekly_facts(db, user_id).await?;
    let weakness = weakness_report(db, user_id).await?;
    let primary = weakness.top_three.first();
    let focus = primary
        .map(|leak| leak.label.clone())
        .unwrap_or_else(|| "Checks, captures, threats".into());
    let drills = weakness
        .top_three
        .iter()
        .map(|leak| leak.label.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let stop = if ladder_stage_number(
        facts
            .get("current_elo")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(500) as i32,
    ) < 3
    {
        "No new opening theory this week.".to_string()
    } else {
        "Do not add a second opening system yet.".to_string()
    };
    let fallback_rule = primary
        .map(|leak| leak.advice.clone())
        .unwrap_or_else(|| "Use a pre-move habit check.".to_string());
    let fallback_text = format!(
        "FOCUS: {focus}\nWHY: {} tagged positions in the current sample. The leak is measured, not guessed.\nDRILLS: {drills}\nSTOP: {stop}\nNEXT_WEEK_RULE: {fallback_rule}",
        primary.map(|leak| leak.count).unwrap_or(0)
    );

    let (mut text, ai_status, model) = if ai.is_configured() {
        let session = stable_session(&format!(
            "curriculum:{}:{}",
            user_id,
            Utc::now().format("%G-W%V")
        ));
        let prompt =
            prompts::curriculum_prompt(&serde_json::to_string_pretty(&facts).unwrap_or_default());
        let request = LlmRequest::new(session, "curriculum")
            .system(prompt)
            .user("Prescribe the next micro-curriculum from the verified facts.")
            .max_tokens(450);
        match ai.complete(user_id, request).await {
            Ok((generated, _)) if !generated.trim().is_empty() => {
                if validates_moves(&generated, &HashSet::new()) {
                    (generated, "ok".to_string(), ai.model().to_string())
                } else {
                    (
                        fallback_text.clone(),
                        "rejected".to_string(),
                        "deterministic".into(),
                    )
                }
            }
            Ok(_) => (
                fallback_text.clone(),
                "fallback".to_string(),
                "deterministic".into(),
            ),
            Err(err) => {
                tracing::warn!(error = %err, "curriculum AI failed");
                ai.record_failure().await;
                (
                    fallback_text.clone(),
                    "fallback".to_string(),
                    "deterministic".into(),
                )
            }
        }
    } else {
        (
            fallback_text.clone(),
            "unavailable".to_string(),
            "deterministic".into(),
        )
    };

    let next_week_rule = extract_rule(&text, &fallback_rule);
    text = strip_rule_line(&text);

    let content = json!({
        "focus": focus,
        "text": text,
        "next_week_rule": next_week_rule,
        "ai_status": ai_status,
        "model": model,
        "facts": facts,
    });
    sqlx::query(
        "INSERT INTO coaching_reports (user_id, kind, title, content, model)
         VALUES ($1, 'curriculum', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(format!(
        "Next micro-curriculum · {}",
        Utc::now().format("%Y-%m-%d")
    ))
    .bind(&content)
    .bind(&model)
    .execute(db)
    .await?;

    Ok(CurriculumReport {
        focus,
        text,
        next_week_rule,
        ai_status,
        model,
        facts,
    })
}
