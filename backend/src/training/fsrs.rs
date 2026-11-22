use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

const W: [f64; 17] = [
    0.4072, 1.1829, 3.1262, 15.4722, 7.2102, 0.5316, 1.0651, 0.0234, 1.616, 0.1544, 1.0824, 1.9813,
    0.0953, 0.2975, 2.2042, 0.2407, 2.9466,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsrsState {
    pub stability: f64,
    pub difficulty: f64,
    pub reps: i32,
    pub lapses: i32,
    pub state: String,
    pub last_reviewed_at: Option<DateTime<Utc>>,
    pub mastered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsrsReview {
    pub stability: f64,
    pub difficulty: f64,
    pub reps: i32,
    pub lapses: i32,
    pub state: String,
    pub due_at: DateTime<Utc>,
    pub interval_days: f64,
    pub mastered: bool,
}

pub fn review(state: &FsrsState, rating: u8, now: DateTime<Utc>) -> FsrsReview {
    let rating = rating.clamp(1, 4);
    let elapsed = state
        .last_reviewed_at
        .map(|last| (now - last).num_seconds().max(0) as f64 / 86_400.0)
        .unwrap_or(0.0);

    let (stability, difficulty) = if state.reps == 0 {
        let stability = W[(rating - 1) as usize];
        let difficulty = (W[4] - (f64::from(rating) - 3.0) * W[5]).clamp(1.0, 10.0);
        (stability, difficulty)
    } else {
        let retrievability = (1.0 + elapsed / (9.0 * state.stability.max(0.1))).powf(-1.0);
        let mut difficulty = state.difficulty - W[6] * (f64::from(rating) - 3.0);
        let target = (W[4] - W[5]).clamp(1.0, 10.0);
        difficulty = (W[7] * target + (1.0 - W[7]) * difficulty).clamp(1.0, 10.0);

        let stability = if rating == 1 {
            W[11]
                * difficulty.powf(-W[12])
                * ((state.stability + 1.0).powf(W[13]) - 1.0)
                * (W[14] * (1.0 - retrievability)).exp()
        } else {
            let hard_penalty = if rating == 2 { W[15] } else { 1.0 };
            let easy_bonus = if rating == 4 { W[16] } else { 1.0 };
            state.stability
                * (1.0
                    + W[8].exp()
                        * (11.0 - difficulty)
                        * state.stability.max(0.1).powf(-W[9])
                        * ((W[10] * (1.0 - retrievability)).exp() - 1.0)
                        * hard_penalty
                        * easy_bonus)
        };
        (stability.max(0.1), difficulty)
    };

    let interval_days = stability.max(1.0).round();
    let reps = state.reps + 1;
    let lapses = state.lapses + i32::from(rating == 1);
    let state_name = if rating == 1 { "learning" } else { "review" };
    let due_at = now + Duration::seconds((interval_days * 86_400.0) as i64);
    let mastered = reps >= 4 && stability >= 21.0;

    FsrsReview {
        stability,
        difficulty,
        reps,
        lapses,
        state: state_name.to_string(),
        due_at,
        interval_days,
        mastered,
    }
}

pub fn rating_for_attempt(correct: bool, hints: u32, elapsed_ms: i64) -> u8 {
    if !correct {
        1
    } else if hints > 0 {
        2
    } else if elapsed_ms > 15_000 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> FsrsState {
        FsrsState {
            stability: 1.0,
            difficulty: 5.0,
            reps: 0,
            lapses: 0,
            state: "new".into(),
            last_reviewed_at: None,
            mastered: false,
        }
    }

    #[test]
    fn first_good_review_schedules_forward() {
        let now = Utc::now();
        let review = review(&fresh(), 3, now);
        assert!(review.due_at > now);
        assert_eq!(review.reps, 1);
        assert_eq!(review.state, "review");
    }

    #[test]
    fn failure_increments_lapses_and_returns_soon() {
        let now = Utc::now();
        let mut state = fresh();
        state.reps = 3;
        state.stability = 10.0;
        state.last_reviewed_at = Some(now - Duration::days(3));
        let review = review(&state, 1, now);
        assert_eq!(review.lapses, 1);
        assert_eq!(review.state, "learning");
        assert!(review.interval_days <= 10.0);
    }

    #[test]
    fn maps_attempt_quality() {
        assert_eq!(rating_for_attempt(false, 0, 1000), 1);
        assert_eq!(rating_for_attempt(true, 1, 1000), 2);
        assert_eq!(rating_for_attempt(true, 0, 30_000), 3);
        assert_eq!(rating_for_attempt(true, 0, 3000), 4);
    }
}
