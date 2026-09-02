use chrono::{DateTime, Utc};

/// A blackjack table, either the player's turn or already settled.
///
/// `player` is the encoded hands (and which one is active). `dealer` includes
/// the hole card and `remaining` is the undealt stock. Neither the hole nor
/// the stock must be sent to the client until the table is over.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BlackjackHand {
    pub id: i64,
    pub user_id: i64,
    pub bet: i64,
    pub doubled: i64,
    pub player: String,
    pub dealer: String,
    pub remaining: String,
    pub outcome: Option<String>,
    pub payout: Option<i64>,
    pub balance_after: i64,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

impl BlackjackHand {
    pub fn is_open(&self) -> bool {
        self.settled_at.is_none()
    }
}
