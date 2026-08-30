use chrono::{DateTime, Utc};
use serde::Serialize;

/// A house marker: the player received `principal` coins and still owes
/// `principal + interest` until `repaid_at` is set.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Loan {
    pub id: i64,
    pub user_id: i64,
    pub principal: i64,
    pub interest: i64,
    pub created_at: DateTime<Utc>,
    pub repaid_at: Option<DateTime<Utc>>,
}

impl Loan {
    /// What must be paid back to clear the marker.
    pub fn owed(&self) -> i64 {
        self.principal.saturating_add(self.interest)
    }

    pub fn is_open(&self) -> bool {
        self.repaid_at.is_none()
    }
}
