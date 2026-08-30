use chrono::{DateTime, Utc};
use serde::Serialize;

/// An admin-issued credit or debit against a player's coin balance.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct BalanceAdjustment {
    pub id: i64,
    pub user_id: i64,
    pub player_username: String,
    pub operator_id: Option<i64>,
    pub operator_username: String,
    pub delta: i64,
    pub balance_before: i64,
    pub balance_after: i64,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}
