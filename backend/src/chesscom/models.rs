use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ChessComProfile {
    pub player_id: Option<i64>,
    #[serde(default)]
    pub username: String,
    pub name: Option<String>,
    pub location: Option<String>,
    pub country: Option<String>,
    pub avatar: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicProfile {
    pub username: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub title: Option<String>,
    pub country: Option<String>,
    pub url: Option<String>,
    pub rapid_rating: Option<i64>,
    pub blitz_rating: Option<i64>,
    pub bullet_rating: Option<i64>,
    pub rapid_games: Option<i64>,
}

impl PublicProfile {
    pub fn from_parts(profile: &ChessComProfile, stats: Option<&ChessComStats>) -> Self {
        let rapid = stats.and_then(|s| s.chess_rapid.as_ref());
        let blitz = stats.and_then(|s| s.chess_blitz.as_ref());
        let bullet = stats.and_then(|s| s.chess_bullet.as_ref());
        Self {
            username: profile.username.clone(),
            name: profile.name.clone(),
            avatar: profile.avatar.clone(),
            title: profile.title.clone(),
            country: profile.country.clone(),
            url: profile.url.clone(),
            rapid_rating: rapid.and_then(|r| r.last.as_ref().and_then(|l| l.rating)),
            blitz_rating: blitz.and_then(|r| r.last.as_ref().and_then(|l| l.rating)),
            bullet_rating: bullet.and_then(|r| r.last.as_ref().and_then(|l| l.rating)),
            rapid_games: rapid.and_then(|r| r.record.as_ref().and_then(|rec| rec.total())),
        }
    }

    pub fn best_rating(&self) -> Option<i64> {
        [self.rapid_rating, self.blitz_rating, self.bullet_rating]
            .into_iter()
            .flatten()
            .max()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChessComStats {
    pub chess_rapid: Option<RatingCategory>,
    pub chess_blitz: Option<RatingCategory>,
    pub chess_bullet: Option<RatingCategory>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RatingCategory {
    pub last: Option<RatingSnapshot>,
    pub best: Option<RatingSnapshot>,
    pub record: Option<RatingRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RatingSnapshot {
    pub rating: Option<i64>,
    pub date: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RatingRecord {
    pub win: Option<i64>,
    pub loss: Option<i64>,
    pub draw: Option<i64>,
}

impl RatingRecord {
    pub fn total(&self) -> Option<i64> {
        Some(self.win.unwrap_or(0) + self.loss.unwrap_or(0) + self.draw.unwrap_or(0))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChessComArchives {
    #[serde(default)]
    pub archives: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChessComMonth {
    #[serde(default)]
    pub games: Vec<CcGame>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CcGame {
    pub url: Option<String>,
    pub pgn: Option<String>,
    pub time_control: Option<String>,
    pub end_time: Option<i64>,
    pub rated: Option<bool>,
    pub time_class: Option<String>,
    pub rules: Option<String>,
    pub uuid: Option<String>,
    #[serde(default)]
    pub white: CcPlayer,
    #[serde(default)]
    pub black: CcPlayer,
    pub eco: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CcPlayer {
    pub username: Option<String>,
    pub rating: Option<i64>,
    pub result: Option<String>,
    #[serde(rename = "@id")]
    pub id: Option<String>,
}

impl CcGame {
    pub fn game_id(&self) -> Option<String> {
        if let Some(url) = &self.url {
            if let Some(last) = url.trim_end_matches('/').rsplit('/').next() {
                if !last.is_empty() {
                    return Some(last.to_string());
                }
            }
        }
        self.uuid.clone()
    }

    pub fn pgn(&self) -> Option<&str> {
        self.pgn.as_deref().filter(|p| !p.trim().is_empty())
    }
}
