use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// What an operator is allowed to do once signed in to the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum OperatorRole {
    /// Reads the dashboard and manages operator and player accounts,
    /// including player balances.
    Admin,
    /// Reads the dashboard and changes their own password, nothing more.
    Viewer,
}

impl OperatorRole {
    pub fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
}

/// A member of staff with access to the admin dashboard.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Operator {
    pub id: i64,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub role: OperatorRole,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub password_changed_at: DateTime<Utc>,
}
