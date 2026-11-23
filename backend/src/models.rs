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

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ReflectRequest {
    pub reason_tags: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ReflectMeta {
    pub reflection_id: Uuid,
    pub session_id: Uuid,
    pub model: String,
    pub ai_configured: bool,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct ReflectDone {
    pub reflection_id: Uuid,
    pub next_time_rule: String,
    pub ai_status: String,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct AnalyzeResponse {
    pub queued: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = EXPORT)]
pub struct PuzzleDto {
    pub id: Uuid,
    pub game_id: Option<Uuid>,
    pub move_id: Option<i64>,
    pub kind: String,
    pub fen: String,
    pub solution_uci: String,
    pub solution_san: Option<String>,
    #[ts(type = "unknown | null")]
    pub best_line: Option<serde_json::Value>,
    pub tags: Vec<String>,
    pub epl: Option<f64>,
    pub due_at: DateTime<Utc>,
    pub stability: f64,
    pub difficulty: f64,
    pub reps: i32,
    pub lapses: i32,
    pub state: String,
    pub mastered: bool,
    pub white_username: Option<String>,
    pub black_username: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct PuzzleAttemptRequest {
    pub uci: String,
    #[serde(default)]
    pub hints: i32,
    pub elapsed_ms: Option<i64>,
    pub rating: Option<u8>,
    #[serde(default)]
    pub revealed: bool,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct PuzzleAttemptResult {
    pub correct: bool,
    pub solution_uci: String,
    pub solution_san: Option<String>,
    pub rating: u8,
    pub interval_days: f64,
    pub due_at: DateTime<Utc>,
    pub stability: f64,
    pub state: String,
    pub mastered: bool,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct PuzzleStats {
    pub due: i32,
    pub total: i32,
    pub mastered: i32,
    pub blunder_fix: i32,
    pub endgame_conversion: i32,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct PuzzleList {
    pub items: Vec<PuzzleDto>,
    pub total: i32,
    pub limit: i32,
    pub offset: i32,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct PuzzleQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub kind: Option<String>,
    #[serde(default)]
    pub mastered: Option<bool>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct PuzzleExplanation {
    pub puzzle_id: Uuid,
    pub goal: String,
    pub motif: String,
    pub why: String,
    pub solution_san: String,
    pub opponent_reply_san: Option<String>,
    pub best_line_san: Vec<String>,
    pub next_time_rule: String,
    #[ts(type = "unknown | null")]
    pub best_line: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SparringMoveDto {
    pub move_number: i32,
    pub color: String,
    pub san: String,
    pub uci: String,
    pub fen_after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SparringSessionDto {
    pub id: Uuid,
    pub game_id: Option<Uuid>,
    pub puzzle_id: Option<Uuid>,
    pub fen: String,
    pub initial_fen: String,
    pub user_color: String,
    pub enemy_elo: Option<i32>,
    pub status: String,
    pub result: Option<String>,
    pub moves: Vec<SparringMoveDto>,
    pub puzzle_epl: Option<f64>,
    pub puzzle_tags: Vec<String>,
    pub white_username: Option<String>,
    pub black_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SparringHint {
    pub uci: String,
    pub san: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SparringStartRequest {
    pub puzzle_id: Option<Uuid>,
    pub game_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SparringMoveRequest {
    pub uci: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct SparringMoveResponse {
    pub session: SparringSessionDto,
    pub user_move: SparringMoveDto,
    pub engine_move: Option<SparringMoveDto>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = EXPORT)]
pub struct GoalDto {
    pub id: Uuid,
    pub kind: String,
    pub stage: Option<i32>,
    pub title: String,
    pub description: Option<String>,
    pub target_elo: Option<i32>,
    pub target_date: Option<chrono::NaiveDate>,
    #[ts(type = "unknown")]
    pub progress: serde_json::Value,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct GoalCreateRequest {
    pub title: String,
    pub description: Option<String>,
    pub target_elo: Option<i32>,
    pub target_date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct RoadmapStageView {
    pub stage: i32,
    pub target_elo: i32,
    pub title: String,
    pub description: String,
    pub criteria: Vec<String>,
    pub completed: bool,
    pub current: bool,
    pub rating_reached: bool,
    pub criteria_met: bool,
    #[ts(type = "unknown")]
    pub progress: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct RoadmapView {
    pub current_elo: Option<i32>,
    pub next_target: Option<i32>,
    pub progress_pct: f64,
    #[ts(type = "unknown")]
    pub metrics: serde_json::Value,
    pub stages: Vec<RoadmapStageView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct NotificationSettingsDto {
    pub user_id: Uuid,
    pub configured: bool,
    pub telegram_chat_id: Option<String>,
    pub email: Option<String>,
    #[ts(type = "unknown | null")]
    pub web_push_subscription: Option<serde_json::Value>,
    pub daily_reminder: bool,
    pub tilt_alerts: bool,
    pub reminder_hour: i32,
    pub timezone: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct NotificationSettingsUpdate {
    pub telegram_chat_id: Option<String>,
    pub email: Option<String>,
    #[ts(type = "unknown | null")]
    pub web_push_subscription: Option<serde_json::Value>,
    pub daily_reminder: bool,
    pub tilt_alerts: bool,
    pub reminder_hour: i32,
    pub timezone: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct VapidKeyResponse {
    pub public_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = EXPORT)]
pub struct NotificationDelivery {
    pub channel: String,
    pub ok: bool,
    pub detail: String,
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
    pub opening: Option<String>,
    pub time_class: Option<String>,
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
