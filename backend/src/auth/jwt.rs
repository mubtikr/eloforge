use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::UserDto,
};

pub const SESSION_COOKIE: &str = "ef_session";
pub const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn issue_token(user: &UserDto, secret: &str) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user.id.to_string(),
        username: user.chesscom_username.clone(),
        iat: now.timestamp(),
        exp: (now + Duration::seconds(SESSION_TTL_SECS)).timestamp(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to issue session token: {e}")))
}

pub fn decode_token(token: &str, secret: &str) -> AppResult<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 30;
    validation.validate_exp = true;
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}

pub fn claims_user_id(claims: &Claims) -> AppResult<Uuid> {
    Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)
}

pub fn session_cookie(token: &str, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECS}{secure_flag}"
    )
}

pub fn clear_cookie(secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT{secure_flag}"
    )
}

pub fn cookie_value(cookie_header: &str, name: &str) -> Option<String> {
    cookie_header.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        if key == name {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn user() -> UserDto {
        UserDto {
            id: Uuid::new_v4(),
            chesscom_username: "testplayer".into(),
            display_name: None,
            country: None,
            avatar_url: None,
            title: None,
            current_elo: Some(520),
            peak_elo: Some(540),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn round_trips_claims() {
        let user = user();
        let token = issue_token(&user, "0123456789abcdef0123456789abcdef").expect("token");
        let claims = decode_token(&token, "0123456789abcdef0123456789abcdef").expect("claims");
        assert_eq!(claims.sub, user.id.to_string());
        assert_eq!(claims.username, "testplayer");
    }

    #[test]
    fn rejects_a_different_secret() {
        let user = user();
        let token = issue_token(&user, "0123456789abcdef0123456789abcdef").expect("token");
        assert!(decode_token(&token, "ffffffffffffffffffffffffffffffff").is_err());
    }

    #[test]
    fn parses_cookie_header() {
        assert_eq!(
            cookie_value("a=1; ef_session=abc.def; b=2", "ef_session"),
            Some("abc.def".to_string())
        );
        assert_eq!(cookie_value("a=1", "ef_session"), None);
    }
}
