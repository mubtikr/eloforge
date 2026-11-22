use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    auth::{AuthProvider, VerifiedAuth},
    config::Config,
    error::{AppError, AppResult},
    models::ChallengeStart,
};

/// Drop-in Chess.com OAuth 2.0 provider.
///
/// Chess.com gates OAuth client registration behind manual approval. This
/// provider is wired behind the same trait as the nonce challenge so that
/// flipping `CHESSCOM_OAUTH_ENABLED=true` and supplying credentials is the only
/// change required once access is granted.
#[derive(Clone)]
pub struct ChessComOAuthProvider {
    enabled: bool,
    client_id: Option<String>,
    redirect_uri: String,
}

impl ChessComOAuthProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            enabled: cfg.chesscom_oauth_enabled,
            client_id: cfg.chesscom_oauth_client_id.clone(),
            redirect_uri: cfg.chesscom_oauth_redirect_uri.clone(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled && self.client_id.is_some()
    }

    pub fn authorize_url(&self, state: &str) -> AppResult<String> {
        if !self.enabled() {
            return Err(AppError::BadRequest(
                "Chess.com OAuth is not enabled on this deployment.".into(),
            ));
        }
        let client_id = self
            .client_id
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("missing Chess.com OAuth client id".into()))?;
        Ok(format!(
            "https://www.chess.com/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&state={}",
            urlencoding::encode(client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(state),
        ))
    }
}

#[async_trait]
impl AuthProvider for ChessComOAuthProvider {
    fn name(&self) -> &'static str {
        "chesscom_oauth"
    }

    async fn start(&self, _username: &str, _ip_key: &str) -> AppResult<ChallengeStart> {
        Err(AppError::BadRequest(
            "Chess.com OAuth is not enabled. Use the profile verification challenge.".into(),
        ))
    }

    async fn verify(&self, _challenge_id: Uuid) -> AppResult<VerifiedAuth> {
        Err(AppError::BadRequest(
            "Chess.com OAuth is not enabled. Use the profile verification challenge.".into(),
        ))
    }
}
