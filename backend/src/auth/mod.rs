pub mod jwt;
pub mod oauth;
pub mod ratelimit;

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, HeaderMap},
};
use chrono::{DateTime, Duration, Utc};
use rand::seq::SliceRandom;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::{
        jwt::{claims_user_id, cookie_value, decode_token, issue_token, SESSION_COOKIE},
        ratelimit::check_rate_limit,
    },
    chesscom::ChessComClient,
    config::Config,
    error::{AppError, AppResult},
    models::{ChallengeStart, ProfilePreview, UserDto},
    state::AppState,
};

pub use oauth::ChessComOAuthProvider;

#[derive(Debug, Clone)]
pub struct VerifiedAuth {
    pub user: UserDto,
    pub token: String,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn start(&self, username: &str, ip_key: &str) -> AppResult<ChallengeStart>;
    async fn verify(&self, challenge_id: Uuid) -> AppResult<VerifiedAuth>;
}

pub struct ChessComChallengeProvider {
    db: PgPool,
    client: Arc<ChessComClient>,
    redis: redis::aio::MultiplexedConnection,
    jwt_secret: String,
}

impl ChessComChallengeProvider {
    pub fn new(
        db: PgPool,
        client: Arc<ChessComClient>,
        redis: redis::aio::MultiplexedConnection,
        cfg: &Config,
    ) -> Self {
        Self {
            db,
            client,
            redis,
            jwt_secret: cfg.jwt_secret.clone(),
        }
    }

    async fn upsert_user(
        &self,
        profile: &crate::chesscom::ChessComProfile,
        stats: Option<&crate::chesscom::ChessComStats>,
    ) -> AppResult<UserDto> {
        let public = crate::chesscom::PublicProfile::from_parts(profile, stats);
        let current_elo = public.best_rating().and_then(|v| i32::try_from(v).ok());

        let user = sqlx::query_as::<_, UserDto>(
            "INSERT INTO users (
                chesscom_username, chesscom_player_id, display_name, country, avatar_url, title,
                current_elo, peak_elo
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$7)
             ON CONFLICT (lower(chesscom_username)) DO UPDATE SET
                chesscom_username = EXCLUDED.chesscom_username,
                chesscom_player_id = COALESCE(EXCLUDED.chesscom_player_id, users.chesscom_player_id),
                display_name = COALESCE(EXCLUDED.display_name, users.display_name),
                country = COALESCE(EXCLUDED.country, users.country),
                avatar_url = COALESCE(EXCLUDED.avatar_url, users.avatar_url),
                title = COALESCE(EXCLUDED.title, users.title),
                current_elo = COALESCE(EXCLUDED.current_elo, users.current_elo),
                peak_elo = GREATEST(COALESCE(users.peak_elo, 0), COALESCE(EXCLUDED.peak_elo, 0))
             RETURNING id, chesscom_username, display_name, country, avatar_url, title,
                       current_elo, peak_elo, created_at",
        )
        .bind(&profile.username)
        .bind(profile.player_id.map(|v| v.to_string()))
        .bind(&profile.name)
        .bind(&profile.country)
        .bind(&profile.avatar)
        .bind(&profile.title)
        .bind(current_elo)
        .fetch_one(&self.db)
        .await?;

        Ok(user)
    }
}

#[async_trait]
impl AuthProvider for ChessComChallengeProvider {
    fn name(&self) -> &'static str {
        "chesscom_challenge"
    }

    async fn start(&self, username: &str, ip_key: &str) -> AppResult<ChallengeStart> {
        let username = username.trim();
        if username.len() < 2 || username.len() > 50 {
            return Err(AppError::BadRequest(
                "Chess.com usernames are between 2 and 50 characters.".into(),
            ));
        }
        if !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(AppError::BadRequest(
                "That does not look like a Chess.com username.".into(),
            ));
        }

        let bucket = format!("auth_start:{ip_key}");
        check_rate_limit(&mut self.redis.clone(), &bucket, 10, 60).await?;

        let profile = self.client.fetch_profile(username).await?;
        let stats = self.client.fetch_stats(username).await.ok();
        let preview = ProfilePreview::from(&crate::chesscom::PublicProfile::from_parts(
            &profile,
            stats.as_ref(),
        ));

        let nonce = generate_nonce();
        let expires_at = Utc::now() + Duration::minutes(10);

        sqlx::query("DELETE FROM auth_challenges WHERE expires_at < now() - interval '1 day'")
            .execute(&self.db)
            .await?;

        let challenge_id: Uuid = sqlx::query_scalar(
            "INSERT INTO auth_challenges (username, nonce, expires_at, ip_hash)
             VALUES ($1,$2,$3,$4) RETURNING id",
        )
        .bind(&profile.username)
        .bind(&nonce)
        .bind(expires_at)
        .bind(ip_key)
        .fetch_one(&self.db)
        .await?;

        Ok(ChallengeStart {
            challenge_id,
            username: profile.username.clone(),
            nonce: nonce.clone(),
            expires_at,
            instructions: format!(
                "Paste {nonce} into your Chess.com profile Location or Name field, save it, then press verify. You can remove it immediately afterwards."
            ),
            profile: Some(preview),
        })
    }

    async fn verify(&self, challenge_id: Uuid) -> AppResult<VerifiedAuth> {
        check_rate_limit(
            &mut self.redis.clone(),
            &format!("auth_verify:{challenge_id}"),
            20,
            60,
        )
        .await?;

        let row = sqlx::query_as::<_, (String, String, Option<DateTime<Utc>>)>(
            "SELECT username, nonce, verified_at FROM auth_challenges
             WHERE id = $1 AND expires_at > now()",
        )
        .bind(challenge_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(
                "This verification code has expired. Start again to get a fresh one.".into(),
            )
        })?;

        if row.2.is_some() {
            return Err(AppError::BadRequest(
                "This verification code has already been used. Start again.".into(),
            ));
        }

        let profile = self.client.fetch_profile(&row.0).await?;
        let haystacks = [profile.location.as_deref(), profile.name.as_deref()];
        let found = haystacks.into_iter().flatten().any(|field| {
            field
                .to_ascii_uppercase()
                .contains(&row.1.to_ascii_uppercase())
        });

        if !found {
            let current = profile
                .location
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    profile
                        .name
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                })
                .unwrap_or("empty");
            return Err(AppError::BadRequest(format!(
                "We could not find {} in your profile. Chess.com currently reports {:?} in Location/Name. Click Save after pasting the code from this tab, give Chess.com a few seconds, then press Verify again.",
                row.1, current
            )));
        }

        let stats = self.client.fetch_stats(&profile.username).await.ok();
        let user = self.upsert_user(&profile, stats.as_ref()).await?;
        if let Err(err) = crate::goals::ladder::seed_roadmap(&self.db, user.id).await {
            tracing::warn!(error = %err, "could not seed roadmap milestones");
        }
        let token = issue_token(&user, &self.jwt_secret)?;

        sqlx::query("UPDATE auth_challenges SET verified_at = now(), user_id = $2 WHERE id = $1")
            .bind(challenge_id)
            .bind(user.id)
            .execute(&self.db)
            .await?;

        Ok(VerifiedAuth { user, token })
    }
}

pub fn generate_nonce() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    let code: String = (0..5)
        .map(|_| ALPHABET.choose(&mut rng).copied().unwrap_or(b'A') as char)
        .collect();
    format!("EF-{code}")
}

pub async fn load_user(db: &PgPool, id: Uuid) -> AppResult<UserDto> {
    sqlx::query_as::<_, UserDto>(
        "SELECT id, chesscom_username, display_name, country, avatar_url, title,
                current_elo, peak_elo, created_at
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::Unauthorized)
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(&parts.headers).or_else(|| {
            parts
                .headers
                .get(axum::http::header::COOKIE)
                .and_then(|value| value.to_str().ok())
                .and_then(|header| cookie_value(header, SESSION_COOKIE))
        });

        let token = token.ok_or(AppError::Unauthorized)?;
        let claims = decode_token(&token, &state.cfg.jwt_secret)?;
        let id = claims_user_id(&claims)?;
        Ok(Self {
            id,
            username: claims.username,
        })
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_has_expected_shape() {
        let nonce = generate_nonce();
        assert!(nonce.starts_with("EF-"));
        assert_eq!(nonce.len(), 8);
        assert!(!nonce.contains(['I', 'O', '0', '1']));
    }

    #[test]
    fn nonces_are_not_constant() {
        let a = generate_nonce();
        let b = generate_nonce();
        assert_ne!(a, b);
    }
}
