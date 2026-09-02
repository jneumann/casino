use chrono::{DateTime, Utc};

/// A Jacks or Better hand, either waiting on a draw or already settled.
///
/// `remaining` is the undealt stock and must never be sent to the client:
/// those cards are what the draw is taken from.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VideoPokerHand {
    pub id: i64,
    pub user_id: i64,
    pub bet: i64,
    pub dealt: String,
    pub remaining: String,
    pub held: Option<String>,
    pub drawn: Option<String>,
    pub outcome: Option<String>,
    pub payout: Option<i64>,
    pub balance_after: i64,
    pub created_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

impl VideoPokerHand {
    pub fn is_open(&self) -> bool {
        self.settled_at.is_none()
    }
}
