use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use ts_rs::TS;
use uuid::Uuid;

const EXPORT: &str = "../../frontend/src/api/bindings.ts";

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = EXPORT)]
pub struct UserDto {
    pub id: Uuid,
    pub chesscom_username: String,
    pub display_name: Option<String>,
    pub country: Option<String>,
    pub avatar_url: Option<String>,
    pub title: Option<String>,
    pub current_elo: Option<i32>,
    pub peak_elo: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ProfilePreview {
    pub username: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub title: Option<String>,
    pub country: Option<String>,
    pub url: Option<String>,
    pub rapid_rating: Option<i32>,
    pub blitz_rating: Option<i32>,
    pub bullet_rating: Option<i32>,
    pub rapid_games: Option<i32>,
}

impl From<&crate::chesscom::PublicProfile> for ProfilePreview {
    fn from(value: &crate::chesscom::PublicProfile) -> Self {
        Self {
            username: value.username.clone(),
            name: value.name.clone(),
            avatar: value.avatar.clone(),
            title: value.title.clone(),
            country: value.country.clone(),
            url: value.url.clone(),
            rapid_rating: value.rapid_rating.and_then(|v| i32::try_from(v).ok()),
            blitz_rating: value.blitz_rating.and_then(|v| i32::try_from(v).ok()),
            bullet_rating: value.bullet_rating.and_then(|v| i32::try_from(v).ok()),
            rapid_games: value.rapid_games.and_then(|v| i32::try_from(v).ok()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ChallengeStart {
    pub challenge_id: Uuid,
    pub username: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
    pub instructions: String,
    pub profile: Option<ProfilePreview>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct StartChallengeRequest {
    pub username: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct VerifyChallengeRequest {
    pub challenge_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct AuthResponse {
    pub user: UserDto,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = EXPORT)]
pub struct GameSummary {
    pub id: Uuid,
    pub url: Option<String>,
    pub white_username: String,
    pub black_username: String,
    pub white_rating: Option<i32>,
    pub black_rating: Option<i32>,
    pub user_color: String,
    pub result: String,
    pub result_reason: Option<String>,
    pub time_class: Option<String>,
    pub time_control: Option<String>,
    pub rated: bool,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub move_count: i32,
    pub opening_eco: Option<String>,
    pub opening_name: Option<String>,
    pub last_book_ply: Option<i32>,
    pub analysis_status: String,
    pub accuracy_user: Option<f64>,
    pub max_advantage: Option<f64>,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = EXPORT)]
pub struct MoveDto {
    #[ts(type = "number")]
    pub id: i64,
    pub ply: i32,
    pub move_number: i32,
    pub color: String,
    pub san: String,
    pub uci: String,
    pub fen_before: String,
    pub fen_after: String,
    pub clock_remaining_sec: Option<f64>,
    pub time_spent_sec: Option<f64>,
    pub eval_before_cp: Option<i32>,
    pub eval_after_cp: Option<i32>,
    pub eval_mate: Option<i32>,
    pub win_prob_user: Option<f64>,
    pub best_move_uci: Option<String>,
    pub best_move_san: Option<String>,
    pub epl: Option<f64>,
    pub classification: Option<String>,
    pub mistake_tags: Vec<String>,
    #[ts(type = "unknown | null")]
    pub pv: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ReflectionDto {
    pub id: Uuid,
    #[ts(type = "number")]
    pub move_id: i64,
    pub reason_tags: Vec<String>,
    pub explanation: String,
    pub ai_feedback: Option<String>,
    pub next_time_rule: Option<String>,
    pub ai_status: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct GameDetail {
    pub game: GameSummary,
    pub initial_fen: String,
    pub accuracy_white: Option<f64>,
    pub accuracy_black: Option<f64>,
    pub eval_peak: Option<f64>,
    #[ts(type = "unknown | null")]
    pub analysis: Option<serde_json::Value>,
    pub moves: Vec<MoveDto>,
    pub reflections: Vec<ReflectionDto>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct GamePage {
    pub items: Vec<GameSummary>,
    pub total: i32,
    pub limit: i32,
    pub offset: i32,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct AnalyzeResponse {
    pub queued: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SyncTriggerResponse {
    pub accepted: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SyncStatus {
    pub running: bool,
    pub total_games: i32,
    pub analyzed_games: i32,
    pub pending_analysis: i32,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct GamesQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub status: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct AppEventDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    pub message: String,
    pub progress: Option<u8>,
    #[ts(type = "unknown")]
    pub data: serde_json::Value,
    pub ts: DateTime<Utc>,
}
