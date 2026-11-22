use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppResult, metrics::Metrics};

pub async fn extract_puzzles_for_game(
    db: &PgPool,
    metrics: &Metrics,
    user_id: Uuid,
    game_id: Uuid,
) -> AppResult<usize> {
    let rows = sqlx::query_as::<_, PuzzleSource>(
        "SELECT m.id AS move_id, m.fen_before, m.best_move_uci, m.best_move_san,
                m.mistake_tags, m.epl, m.pv
         FROM game_moves m
         JOIN games g ON g.id = m.game_id
         WHERE m.game_id = $1 AND g.user_id = $2 AND g.analysis_status = 'done'
           AND m.color = g.user_color AND COALESCE(m.epl, 0) >= 0.15
           AND m.best_move_uci IS NOT NULL
         ORDER BY m.ply",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_all(db)
    .await?;

    let mut created = 0usize;
    for row in rows {
        let endgame = row.mistake_tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "ENDGAME_TECHNIQUE_FAIL" | "WINNING_GAME_THROWN" | "STALEMATE_THROW"
            )
        });
        let kind = if endgame {
            "endgame_conversion"
        } else {
            "blunder_fix"
        };
        let best_line = row.pv.clone().unwrap_or_else(|| json!([]));
        let inserted = sqlx::query(
            "INSERT INTO puzzles
                (user_id, game_id, move_id, kind, fen, solution_uci, solution_san,
                 best_line, tags, epl, due_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,now())
             ON CONFLICT (user_id, game_id, move_id, kind) DO NOTHING",
        )
        .bind(user_id)
        .bind(game_id)
        .bind(row.move_id)
        .bind(kind)
        .bind(&row.fen_before)
        .bind(row.best_move_uci.as_deref().unwrap_or_default())
        .bind(row.best_move_san.as_deref())
        .bind(&best_line)
        .bind(&row.mistake_tags)
        .bind(row.epl)
        .execute(db)
        .await?;
        if inserted.rows_affected() > 0 {
            created += 1;
            metrics.puzzles_created.inc();
        }
    }
    Ok(created)
}

#[derive(sqlx::FromRow)]
struct PuzzleSource {
    move_id: i64,
    fen_before: String,
    best_move_uci: Option<String>,
    best_move_san: Option<String>,
    mistake_tags: Vec<String>,
    epl: Option<f64>,
    pv: Option<serde_json::Value>,
}
