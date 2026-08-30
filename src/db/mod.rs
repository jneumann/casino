pub mod sqlite;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::models::{BalanceAdjustment, Loan, Operator, OperatorRole, User};

/// The persistence seam for the whole application. Handlers depend on this
/// trait only, so moving off SQLite means adding one implementation.
#[async_trait]
pub trait Store: Send + Sync + 'static {
    /// Human-readable backend name, surfaced on the status page.
    fn backend(&self) -> &'static str;

    async fn ping(&self) -> Result<(), StoreError>;

    async fn user_count(&self) -> Result<i64, StoreError>;

    async fn create_user(&self, username: &str, password_hash: &str) -> Result<User, StoreError>;

    async fn user_by_id(&self, id: i64) -> Result<Option<User>, StoreError>;

    async fn user_by_username(&self, username: &str) -> Result<Option<User>, StoreError>;

    async fn record_login(&self, id: i64, at: DateTime<Utc>) -> Result<(), StoreError>;

    /// Debits `bet`, credits `payout`, and records the spin, all or nothing.
    /// Returns the balance afterwards, or [`StoreError::InsufficientFunds`] if
    /// the player could not cover the stake.
    async fn settle_spin(&self, spin: SpinRecord<'_>) -> Result<i64, StoreError>;

    /// The player's open house marker, if they have one.
    async fn open_loan(&self, user_id: i64) -> Result<Option<Loan>, StoreError>;

    /// Credits `principal` and records the marker. The unique index on an
    /// unpaid loan is what two concurrent borrows race against.
    async fn take_loan(&self, loan: LoanRecord) -> Result<(Loan, i64), StoreError>;

    /// Debits the outstanding owed amount and stamps `repaid_at`. Returns the
    /// closed loan and the balance afterwards.
    async fn repay_loan(&self, user_id: i64) -> Result<(Loan, i64), StoreError>;

    async fn list_players(&self, filter: PlayerFilter<'_>) -> Result<Vec<User>, StoreError>;

    /// Returns `false` when no player has that id.
    async fn set_player_banned(&self, id: i64, banned: bool) -> Result<bool, StoreError>;

    /// Credits or debits a player's coins and writes an audit row. Returns
    /// the updated player and the recorded adjustment, or
    /// [`StoreError::InsufficientFunds`] if a debit would take the balance
    /// below zero.
    async fn adjust_player_balance(
        &self,
        adjustment: BalanceAdjustmentRecord<'_>,
    ) -> Result<(User, BalanceAdjustment), StoreError>;

    async fn list_balance_adjustments(
        &self,
        filter: AdjustmentFilter,
    ) -> Result<Vec<BalanceAdjustment>, StoreError>;

    async fn operator_count(&self) -> Result<i64, StoreError>;

    async fn list_operators(&self) -> Result<Vec<Operator>, StoreError>;

    async fn operator_by_id(&self, id: i64) -> Result<Option<Operator>, StoreError>;

    async fn operator_by_username(&self, username: &str) -> Result<Option<Operator>, StoreError>;

    async fn create_operator(
        &self,
        username: &str,
        password_hash: &str,
        role: OperatorRole,
    ) -> Result<Operator, StoreError>;

    /// Also stamps `password_changed_at`, which invalidates that operator's
    /// existing tokens. Returns `false` when no operator has that id.
    async fn update_operator_password(
        &self,
        id: i64,
        password_hash: &str,
    ) -> Result<bool, StoreError>;

    async fn set_operator_active(&self, id: i64, is_active: bool) -> Result<bool, StoreError>;

    async fn set_operator_role(&self, id: i64, role: OperatorRole) -> Result<bool, StoreError>;

    async fn delete_operator(&self, id: i64) -> Result<bool, StoreError>;

    async fn record_operator_login(&self, id: i64, at: DateTime<Utc>) -> Result<(), StoreError>;
}

/// Narrows a player listing. `search` matches on username.
#[derive(Debug, Clone, Copy)]
pub struct PlayerFilter<'a> {
    pub search: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for PlayerFilter<'_> {
    fn default() -> Self {
        Self {
            search: None,
            limit: 50,
            offset: 0,
        }
    }
}

/// A spin that has been played and now needs paying for.
#[derive(Debug, Clone, Copy)]
pub struct SpinRecord<'a> {
    pub user_id: i64,
    pub bet: i64,
    pub payout: i64,
    /// The reels as the client will see them, already serialised.
    pub reels: &'a str,
}

/// A marker the cashier has agreed to write, waiting to be recorded.
#[derive(Debug, Clone, Copy)]
pub struct LoanRecord {
    pub user_id: i64,
    pub principal: i64,
    pub interest: i64,
}

/// An admin credit or debit waiting to be applied and logged.
#[derive(Debug, Clone, Copy)]
pub struct BalanceAdjustmentRecord<'a> {
    pub user_id: i64,
    pub operator_id: i64,
    pub operator_username: &'a str,
    pub delta: i64,
    pub reason: &'a str,
}

/// Narrows the balance-adjustment audit log. `user_id` limits the list to
/// one player when set.
#[derive(Debug, Clone, Copy)]
pub struct AdjustmentFilter {
    pub user_id: Option<i64>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for AdjustmentFilter {
    fn default() -> Self {
        Self {
            user_id: None,
            limit: 50,
            offset: 0,
        }
    }
}

pub type DynStore = Arc<dyn Store>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("username already exists")]
    DuplicateUsername,

    #[error("balance too low for that stake")]
    InsufficientFunds,

    #[error("an outstanding loan is already open")]
    LoanOutstanding,

    #[error("no outstanding loan to repay")]
    NoOpenLoan,

    #[error("account no longer exists")]
    UnknownUser,

    #[error(transparent)]
    Backend(#[from] sqlx::Error),

    #[error("migrations failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("could not prepare database storage: {0}")]
    Io(#[from] std::io::Error),
}
