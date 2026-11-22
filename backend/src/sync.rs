use std::sync::Arc;

use chrono::Utc;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    chess::pgn_importer::{import_game, ImportOutcome},
    chesscom::{CcGame, ChessComArchives, ChessComClient},
    config::Config,
    error::{AppError, AppResult},
    events::{AppEvent, EventBus},
    metrics::Metrics,
    models::{SyncStatus, UserDto},
};

#[derive(Clone)]
pub struct SyncService {
    db: PgPool,
    client: Arc<ChessComClient>,
    redis: redis::aio::MultiplexedConnection,
    events: Arc<EventBus>,
    cfg: Arc<Config>,
    metrics: Arc<Metrics>,
}

impl SyncService {
    pub fn new(
        db: PgPool,
        client: Arc<ChessComClient>,
        redis: redis::aio::MultiplexedConnection,
        events: Arc<EventBus>,
        cfg: Arc<Config>,
        metrics: Arc<Metrics>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            client,
            redis,
            events,
            cfg,
            metrics,
        })
    }

    pub async fn trigger(&self, user: &UserDto) -> AppResult<bool> {
        let mut redis = self.redis.clone();
        let lock_key = format!("ef:sync:lock:{}", user.id);
        let acquired: Option<String> = redis::cmd("SET")
            .arg(&lock_key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(1800)
            .query_async(&mut redis)
            .await?;

        if acquired.is_none() {
            return Ok(false);
        }

        self.metrics.sync_runs.inc();
        let service = self.clone();
        let user = user.clone();
        tokio::spawn(async move {
            let result = service.run(&user).await;
            if let Err(err) = &result {
                tracing::error!(error = %err, user_id = %user.id, "sync failed");
                service.metrics.sync_errors.inc();
                service
                    .events
                    .publish(AppEvent::new(
                        user.id,
                        "sync_error",
                        "Sync stopped before finishing.",
                        None,
                        serde_json::json!({ "error": err.to_string() }),
                    ))
                    .await;
            }
            let mut redis = service.redis.clone();
            let _: Result<i64, _> = redis.del(&lock_key).await;
        });

        Ok(true)
    }

    async fn run(&self, user: &UserDto) -> AppResult<()> {
        self.events
            .publish(AppEvent::new(
                user.id,
                "sync_started",
                format!("Opening {}'s Chess.com archive.", user.chesscom_username),
                Some(0),
                serde_json::json!({ "username": user.chesscom_username }),
            ))
            .await;

        self.refresh_rating(user).await;

        let archives_url = self.client.archives_url(&user.chesscom_username);
        let etag: Option<String> =
            sqlx::query_scalar("SELECT etag FROM sync_cursors WHERE user_id = $1 AND url = $2")
                .bind(user.id)
                .bind(&archives_url)
                .fetch_optional(&self.db)
                .await?
                .flatten();

        let archives = self
            .load_archives(user, &archives_url, etag.as_deref())
            .await?;

        let mut archives: Vec<String> = archives;
        archives.sort();
        if archives.len() > self.cfg.sync_max_archives {
            let start = archives.len() - self.cfg.sync_max_archives;
            archives = archives.split_off(start);
        }

        let mut imported = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;
        let total_months = archives.len().max(1);

        for (index, archive_url) in archives.iter().enumerate() {
            let cursor = sqlx::query_as::<_, (Option<String>, String)>(
                "SELECT etag, status FROM sync_cursors WHERE user_id = $1 AND url = $2",
            )
            .bind(user.id)
            .bind(archive_url)
            .fetch_optional(&self.db)
            .await?;
            let cursor_etag = cursor.as_ref().and_then(|(etag, _)| etag.as_deref());
            let was_synced = cursor
                .as_ref()
                .map(|(_, status)| status == "done")
                .unwrap_or(false);

            let fetched = match self.client.fetch_archive(archive_url, cursor_etag).await {
                Ok(fetched) => fetched,
                Err(err) => {
                    failed += 1;
                    self.metrics.sync_errors.inc();
                    self.mark_cursor_error(user.id, archive_url).await?;
                    tracing::warn!(error = %err, url = %archive_url, "archive fetch failed");
                    self.events
                        .publish(AppEvent::new(
                            user.id,
                            "sync_error",
                            format!("Could not read archive {}.", month_label(archive_url)),
                            Some(progress(index, total_months)),
                            serde_json::json!({ "error": err.to_string(), "url": archive_url }),
                        ))
                        .await;
                    continue;
                }
            };

            if fetched.not_modified {
                skipped += 1;
                self.events
                    .publish(AppEvent::new(
                        user.id,
                        "sync_progress",
                        format!("{} is unchanged.", month_label(archive_url)),
                        Some(progress(index + 1, total_months)),
                        serde_json::json!({
                            "month": month_label(archive_url),
                            "cached": true,
                            "imported": imported,
                            "skipped": skipped,
                        }),
                    ))
                    .await;
                continue;
            }

            let Some(month) = self.client.parse_month(fetched.clone())? else {
                continue;
            };

            let month_name = month_label(archive_url);
            let mut month_imported = 0usize;
            let mut month_skipped = 0usize;

            for game in &month.games {
                if game.pgn().is_none() {
                    month_skipped += 1;
                    continue;
                }
                match import_game(&self.db, user.id, &user.chesscom_username, game).await {
                    Ok(ImportOutcome::Imported(game_id)) => {
                        imported += 1;
                        month_imported += 1;
                        self.metrics.sync_games.inc();
                        if let Err(err) = crate::analyzer::enqueue_game(
                            &self.db,
                            &mut self.redis.clone(),
                            game_id,
                            user.id,
                        )
                        .await
                        {
                            tracing::warn!(error = %err, "could not queue analysis");
                        }
                        self.events
                            .publish(AppEvent::new(
                                user.id,
                                "sync_game",
                                game_message(game),
                                Some(progress(index, total_months)),
                                serde_json::json!({
                                    "month": month_name,
                                    "imported": imported,
                                    "skipped": skipped,
                                    "url": game.url,
                                }),
                            ))
                            .await;
                    }
                    Ok(ImportOutcome::Skipped) => {
                        skipped += 1;
                        month_skipped += 1;
                    }
                    Err(err) => {
                        failed += 1;
                        self.metrics.sync_errors.inc();
                        tracing::warn!(
                            error = %err,
                            url = ?game.url,
                            "game import failed"
                        );
                    }
                }
            }

            self.upsert_cursor(
                user.id,
                archive_url,
                fetched.etag.as_deref(),
                "done",
                month_imported as i32,
            )
            .await?;

            self.events
                .publish(AppEvent::new(
                    user.id,
                    "sync_progress",
                    format!("{month_name}: {month_imported} new, {month_skipped} already stored."),
                    Some(progress(index + 1, total_months)),
                    serde_json::json!({
                        "month": month_name,
                        "imported": imported,
                        "skipped": skipped,
                        "failed": failed,
                        "was_synced": was_synced,
                    }),
                ))
                .await;
        }

        let total_games =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games WHERE user_id = $1")
                .bind(user.id)
                .fetch_one(&self.db)
                .await?;

        self.events
            .publish(AppEvent::new(
                user.id,
                "sync_done",
                format!(
                    "Sync finished: {imported} new game(s), {total_games} total, {failed} error(s)."
                ),
                Some(100),
                serde_json::json!({
                    "imported": imported,
                    "skipped": skipped,
                    "failed": failed,
                    "total_games": total_games,
                }),
            ))
            .await;

        Ok(())
    }

    async fn load_archives(
        &self,
        user: &UserDto,
        archives_url: &str,
        etag: Option<&str>,
    ) -> AppResult<Vec<String>> {
        let fetched = self
            .client
            .fetch_archives(&user.chesscom_username, etag)
            .await?;

        if fetched.not_modified {
            let urls: Vec<String> = sqlx::query_scalar(
                "SELECT url FROM sync_cursors
                 WHERE user_id = $1 AND url LIKE $2 ORDER BY url",
            )
            .bind(user.id)
            .bind("%/games/%")
            .fetch_all(&self.db)
            .await?;
            if !urls.is_empty() {
                return Ok(urls);
            }
            let fetched = self
                .client
                .fetch_archives(&user.chesscom_username, None)
                .await?;
            return self
                .parse_and_store_archives(user.id, archives_url, fetched)
                .await;
        }

        self.parse_and_store_archives(user.id, archives_url, fetched)
            .await
    }

    async fn parse_and_store_archives(
        &self,
        user_id: Uuid,
        archives_url: &str,
        fetched: crate::chesscom::CachedFetch,
    ) -> AppResult<Vec<String>> {
        let parsed: ChessComArchives = self
            .client
            .parse_archives(fetched.clone())?
            .ok_or_else(|| AppError::Upstream("empty Chess.com archives response".into()))?;
        self.upsert_cursor(user_id, archives_url, fetched.etag.as_deref(), "done", 0)
            .await?;
        Ok(parsed.archives)
    }

    async fn refresh_rating(&self, user: &UserDto) {
        let Ok(stats) = self.client.fetch_stats(&user.chesscom_username).await else {
            return;
        };
        let public = crate::chesscom::PublicProfile::from_parts(
            &crate::chesscom::ChessComProfile {
                player_id: None,
                username: user.chesscom_username.clone(),
                name: user.display_name.clone(),
                location: None,
                country: user.country.clone(),
                avatar: user.avatar_url.clone(),
                title: user.title.clone(),
                status: None,
                url: None,
            },
            Some(&stats),
        );
        let Some(rating) = public.best_rating().and_then(|v| i32::try_from(v).ok()) else {
            return;
        };
        let _ = sqlx::query(
            "UPDATE users SET current_elo = $1,
             peak_elo = GREATEST(COALESCE(peak_elo, 0), $1) WHERE id = $2",
        )
        .bind(rating)
        .bind(user.id)
        .execute(&self.db)
        .await;
    }

    async fn upsert_cursor(
        &self,
        user_id: Uuid,
        url: &str,
        etag: Option<&str>,
        status: &str,
        games_imported: i32,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO sync_cursors (user_id, url, etag, status, games_imported, last_synced_at)
             VALUES ($1,$2,$3,$4,$5,now())
             ON CONFLICT (user_id, url) DO UPDATE SET
                etag = COALESCE(EXCLUDED.etag, sync_cursors.etag),
                status = EXCLUDED.status,
                games_imported = sync_cursors.games_imported + EXCLUDED.games_imported,
                last_synced_at = now()",
        )
        .bind(user_id)
        .bind(url)
        .bind(etag)
        .bind(status)
        .bind(games_imported)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn mark_cursor_error(&self, user_id: Uuid, url: &str) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO sync_cursors (user_id, url, status, last_synced_at)
             VALUES ($1,$2,'error',now())
             ON CONFLICT (user_id, url) DO UPDATE SET
                status = 'error', last_synced_at = now()",
        )
        .bind(user_id)
        .bind(url)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn status(&self, user_id: Uuid) -> AppResult<SyncStatus> {
        let mut redis = self.redis.clone();
        let running: bool = redis
            .exists(format!("ef:sync:lock:{user_id}"))
            .await
            .unwrap_or(false);

        let total_games =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM games WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&self.db)
                .await?;
        let analyzed_games = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM games WHERE user_id = $1 AND analysis_status = 'done'",
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await?;
        let pending_analysis = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM games WHERE user_id = $1
             AND analysis_status IN ('pending', 'queued', 'analyzing')",
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await?;
        let last_sync_at = sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT MAX(last_synced_at) FROM sync_cursors WHERE user_id = $1 AND status = 'done'",
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await?;
        let last_error = sqlx::query_scalar::<_, Option<String>>(
            "SELECT analysis_error FROM games
             WHERE user_id = $1 AND analysis_status = 'failed'
             ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?
        .flatten();

        Ok(SyncStatus {
            running,
            total_games: total_games as i32,
            analyzed_games: analyzed_games as i32,
            pending_analysis: pending_analysis as i32,
            last_sync_at,
            last_error,
        })
    }

    pub fn now() -> chrono::DateTime<chrono::Utc> {
        Utc::now()
    }
}

fn progress(done: usize, total: usize) -> u8 {
    if total == 0 {
        return 100;
    }
    (((done as f64 / total as f64) * 100.0).round() as u32).min(100) as u8
}

fn month_label(url: &str) -> String {
    let mut parts: Vec<&str> = url.trim_end_matches('/').rsplit('/').take(2).collect();
    parts.reverse();
    match parts.as_slice() {
        [year, month] => format!("{year}-{month}"),
        _ => url.to_string(),
    }
}

fn game_message(game: &CcGame) -> String {
    let white = game.white.username.as_deref().unwrap_or("White");
    let black = game.black.username.as_deref().unwrap_or("Black");
    format!("Stored {white} vs {black}.")
}
