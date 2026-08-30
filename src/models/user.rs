use chrono::{DateTime, Utc};
use serde::Serialize;

/// Coins credited to every new account.
pub const STARTING_BALANCE: i64 = 1000;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub balance: i64,
    pub is_banned: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}
