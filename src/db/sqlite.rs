use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use super::{
    AdjustmentFilter, BalanceAdjustmentRecord, BlackjackDealRecord, BlackjackSettleRecord,
    BlackjackUpdateRecord, LoanRecord, PlayerFilter, SpinRecord, Store, StoreError,
    VideoPokerDealRecord, VideoPokerSettleRecord,
};
use crate::models::{
    BalanceAdjustment, BlackjackHand, Loan, Operator, OperatorRole, STARTING_BALANCE, User,
    VideoPokerHand,
};

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Opens (creating if needed) the SQLite database at `url`, e.g. `sqlite://data/casino.db`.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));

        if let Some(parent) = options
            .get_filename()
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }
}

#[async_trait]
impl Store for SqliteStore {
    fn backend(&self) -> &'static str {
        "sqlite"
    }

    async fn ping(&self) -> Result<(), StoreError> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }

    async fn user_count(&self) -> Result<i64, StoreError> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get("count")?)
    }

    async fn create_user(&self, username: &str, password_hash: &str) -> Result<User, StoreError> {
        let result = sqlx::query_as::<_, User>(
            "INSERT INTO users (username, password_hash, balance, created_at) \
             VALUES (?, ?, ?, ?) \
             RETURNING id, username, password_hash, balance, is_banned, created_at, last_login_at",
        )
        .bind(username)
        .bind(password_hash)
        .bind(STARTING_BALANCE)
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(user) => Ok(user),
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
                Err(StoreError::DuplicateUsername)
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn user_by_id(&self, id: i64) -> Result<Option<User>, StoreError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, username, password_hash, balance, is_banned, created_at, last_login_at \
             FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    async fn user_by_username(&self, username: &str) -> Result<Option<User>, StoreError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, username, password_hash, balance, is_banned, created_at, last_login_at \
             FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    async fn record_login(&self, id: i64, at: DateTime<Utc>) -> Result<(), StoreError> {
        sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
            .bind(at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn settle_spin(&self, spin: SpinRecord<'_>) -> Result<i64, StoreError> {
        let mut tx = self.pool.begin().await?;

        // The debit is guarded inside the UPDATE itself rather than by reading
        // the balance first, so two concurrent spins cannot both pass an
        // affordability check and overdraw the account.
        let balance_after: Option<i64> = sqlx::query_scalar(
            "UPDATE users SET balance = balance - ? + ? \
             WHERE id = ? AND balance >= ? \
             RETURNING balance",
        )
        .bind(spin.bet)
        .bind(spin.payout)
        .bind(spin.user_id)
        .bind(spin.bet)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(balance_after) = balance_after else {
            // No row changed: either the stake was too high or the account is
            // gone. Tell those apart so the caller can respond accurately.
            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
                .bind(spin.user_id)
                .fetch_optional(&mut *tx)
                .await?;

            return Err(if exists.is_some() {
                StoreError::InsufficientFunds
            } else {
                StoreError::UnknownUser
            });
        };

        sqlx::query(
            "INSERT INTO spins (user_id, bet, payout, reels, balance_after, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(spin.user_id)
        .bind(spin.bet)
        .bind(spin.payout)
        .bind(spin.reels)
        .bind(balance_after)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(balance_after)
    }

    async fn open_video_poker_hand(
        &self,
        user_id: i64,
    ) -> Result<Option<VideoPokerHand>, StoreError> {
        let hand = sqlx::query_as::<_, VideoPokerHand>(
            "SELECT id, user_id, bet, dealt, remaining, held, drawn, outcome, payout, \
             balance_after, created_at, settled_at \
             FROM video_poker_hands WHERE user_id = ? AND settled_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(hand)
    }

    async fn deal_video_poker(
        &self,
        deal: VideoPokerDealRecord<'_>,
    ) -> Result<(VideoPokerHand, i64), StoreError> {
        let mut tx = self.pool.begin().await?;

        // Debit first so two concurrent deals cannot both pass an affordability
        // check. If the insert then loses the unique race on an open hand, the
        // rollback puts the stake back.
        let balance: Option<i64> = sqlx::query_scalar(
            "UPDATE users SET balance = balance - ? \
             WHERE id = ? AND balance >= ? \
             RETURNING balance",
        )
        .bind(deal.bet)
        .bind(deal.user_id)
        .bind(deal.bet)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(balance) = balance else {
            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
                .bind(deal.user_id)
                .fetch_optional(&mut *tx)
                .await?;

            return Err(if exists.is_some() {
                StoreError::InsufficientFunds
            } else {
                StoreError::UnknownUser
            });
        };

        let inserted = sqlx::query_as::<_, VideoPokerHand>(
            "INSERT INTO video_poker_hands \
             (user_id, bet, dealt, remaining, balance_after, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             RETURNING id, user_id, bet, dealt, remaining, held, drawn, outcome, payout, \
             balance_after, created_at, settled_at",
        )
        .bind(deal.user_id)
        .bind(deal.bet)
        .bind(deal.dealt)
        .bind(deal.remaining)
        .bind(balance)
        .bind(Utc::now())
        .fetch_one(&mut *tx)
        .await;

        let hand = match inserted {
            Ok(hand) => hand,
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
                return Err(StoreError::HandInProgress);
            }
            Err(err) => return Err(err.into()),
        };

        tx.commit().await?;

        Ok((hand, balance))
    }

    async fn settle_video_poker(
        &self,
        settle: VideoPokerSettleRecord<'_>,
    ) -> Result<(VideoPokerHand, i64), StoreError> {
        let mut tx = self.pool.begin().await?;

        // Claim the open hand first so a second concurrent draw cannot credit
        // the same payout. If the player vanished, the transaction rolls back
        // and the hand stays open.
        let claimed: Option<VideoPokerHand> = sqlx::query_as::<_, VideoPokerHand>(
            "UPDATE video_poker_hands \
             SET held = ?, drawn = ?, outcome = ?, payout = ?, settled_at = ? \
             WHERE id = ? AND user_id = ? AND settled_at IS NULL \
             RETURNING id, user_id, bet, dealt, remaining, held, drawn, outcome, payout, \
             balance_after, created_at, settled_at",
        )
        .bind(settle.held)
        .bind(settle.drawn)
        .bind(settle.outcome)
        .bind(settle.payout)
        .bind(Utc::now())
        .bind(settle.hand_id)
        .bind(settle.user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(claimed) = claimed else {
            return Err(StoreError::NoOpenHand);
        };

        let balance: Option<i64> = sqlx::query_scalar(
            "UPDATE users SET balance = balance + ? WHERE id = ? RETURNING balance",
        )
        .bind(settle.payout)
        .bind(settle.user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(balance) = balance else {
            return Err(StoreError::UnknownUser);
        };

        sqlx::query("UPDATE video_poker_hands SET balance_after = ? WHERE id = ?")
            .bind(balance)
            .bind(claimed.id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        let mut claimed = claimed;
        claimed.balance_after = balance;
        claimed.payout = Some(settle.payout);

        Ok((claimed, balance))
    }

    async fn open_blackjack_hand(&self, user_id: i64) -> Result<Option<BlackjackHand>, StoreError> {
        let hand = sqlx::query_as::<_, BlackjackHand>(
            "SELECT id, user_id, bet, doubled, player, dealer, remaining, outcome, payout, \
             balance_after, version, created_at, settled_at \
             FROM blackjack_hands WHERE user_id = ? AND settled_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(hand)
    }

    async fn deal_blackjack(
        &self,
        deal: BlackjackDealRecord<'_>,
    ) -> Result<(BlackjackHand, i64), StoreError> {
        let mut tx = self.pool.begin().await?;
        let now = Utc::now();

        let balance: Option<i64> = sqlx::query_scalar(
            "UPDATE users SET balance = balance - ? \
             WHERE id = ? AND balance >= ? \
             RETURNING balance",
        )
        .bind(deal.bet)
        .bind(deal.user_id)
        .bind(deal.bet)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(mut balance) = balance else {
            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
                .bind(deal.user_id)
                .fetch_optional(&mut *tx)
                .await?;

            return Err(if exists.is_some() {
                StoreError::InsufficientFunds
            } else {
                StoreError::UnknownUser
            });
        };

        if deal.outcome.is_some() && deal.payout > 0 {
            balance = sqlx::query_scalar(
                "UPDATE users SET balance = balance + ? WHERE id = ? RETURNING balance",
            )
            .bind(deal.payout)
            .bind(deal.user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(StoreError::UnknownUser)?;
        }

        let settled_at = deal.outcome.map(|_| now);
        let inserted = sqlx::query_as::<_, BlackjackHand>(
            "INSERT INTO blackjack_hands \
             (user_id, bet, player, dealer, remaining, outcome, payout, balance_after, \
              created_at, settled_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             RETURNING id, user_id, bet, doubled, player, dealer, remaining, outcome, payout, \
             balance_after, version, created_at, settled_at",
        )
        .bind(deal.user_id)
        .bind(deal.bet)
        .bind(deal.player)
        .bind(deal.dealer)
        .bind(deal.remaining)
        .bind(deal.outcome)
        .bind(deal.outcome.map(|_| deal.payout))
        .bind(balance)
        .bind(now)
        .bind(settled_at)
        .fetch_one(&mut *tx)
        .await;

        let hand = match inserted {
            Ok(hand) => hand,
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
                return Err(StoreError::HandInProgress);
            }
            Err(err) => return Err(err.into()),
        };

        tx.commit().await?;

        Ok((hand, balance))
    }

    async fn update_blackjack(
        &self,
        update: BlackjackUpdateRecord<'_>,
    ) -> Result<(BlackjackHand, i64), StoreError> {
        let mut tx = self.pool.begin().await?;

        let balance: Option<i64> = sqlx::query_scalar("SELECT balance FROM users WHERE id = ?")
            .bind(update.user_id)
            .fetch_optional(&mut *tx)
            .await?;

        let Some(mut balance) = balance else {
            return Err(StoreError::UnknownUser);
        };

        if update.extra_bet > 0 {
            let debited: Option<i64> = sqlx::query_scalar(
                "UPDATE users SET balance = balance - ? \
                 WHERE id = ? AND balance >= ? \
                 RETURNING balance",
            )
            .bind(update.extra_bet)
            .bind(update.user_id)
            .bind(update.extra_bet)
            .fetch_optional(&mut *tx)
            .await?;

            let Some(next) = debited else {
                return Err(StoreError::InsufficientFunds);
            };
            balance = next;
        }

        let hand = sqlx::query_as::<_, BlackjackHand>(
            "UPDATE blackjack_hands \
             SET player = ?, remaining = ?, version = version + 1 \
             WHERE id = ? AND user_id = ? AND version = ? AND settled_at IS NULL \
             RETURNING id, user_id, bet, doubled, player, dealer, remaining, outcome, payout, \
             balance_after, version, created_at, settled_at",
        )
        .bind(update.player)
        .bind(update.remaining)
        .bind(update.hand_id)
        .bind(update.user_id)
        .bind(update.version)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(hand) = hand else {
            return Err(StoreError::NoOpenHand);
        };

        tx.commit().await?;
        Ok((hand, balance))
    }

    async fn settle_blackjack(
        &self,
        settle: BlackjackSettleRecord<'_>,
    ) -> Result<(BlackjackHand, i64), StoreError> {
        let mut tx = self.pool.begin().await?;

        if settle.extra_bet > 0 {
            let debited: Option<i64> = sqlx::query_scalar(
                "UPDATE users SET balance = balance - ? \
                 WHERE id = ? AND balance >= ? \
                 RETURNING balance",
            )
            .bind(settle.extra_bet)
            .bind(settle.user_id)
            .bind(settle.extra_bet)
            .fetch_optional(&mut *tx)
            .await?;

            if debited.is_none() {
                let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
                    .bind(settle.user_id)
                    .fetch_optional(&mut *tx)
                    .await?;

                return Err(if exists.is_some() {
                    StoreError::InsufficientFunds
                } else {
                    StoreError::UnknownUser
                });
            }
        }

        let claimed: Option<BlackjackHand> = sqlx::query_as::<_, BlackjackHand>(
            "UPDATE blackjack_hands \
             SET player = ?, dealer = ?, remaining = ?, doubled = ?, outcome = ?, payout = ?, \
                 settled_at = ?, version = version + 1 \
             WHERE id = ? AND user_id = ? AND version = ? AND settled_at IS NULL \
             RETURNING id, user_id, bet, doubled, player, dealer, remaining, outcome, payout, \
             balance_after, version, created_at, settled_at",
        )
        .bind(settle.player)
        .bind(settle.dealer)
        .bind(settle.remaining)
        .bind(i64::from(settle.doubled))
        .bind(settle.outcome)
        .bind(settle.payout)
        .bind(Utc::now())
        .bind(settle.hand_id)
        .bind(settle.user_id)
        .bind(settle.version)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(claimed) = claimed else {
            return Err(StoreError::NoOpenHand);
        };

        let balance: Option<i64> = sqlx::query_scalar(
            "UPDATE users SET balance = balance + ? WHERE id = ? RETURNING balance",
        )
        .bind(settle.payout)
        .bind(settle.user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(balance) = balance else {
            return Err(StoreError::UnknownUser);
        };

        sqlx::query("UPDATE blackjack_hands SET balance_after = ? WHERE id = ?")
            .bind(balance)
            .bind(claimed.id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        let mut claimed = claimed;
        claimed.balance_after = balance;
        claimed.payout = Some(settle.payout);

        Ok((claimed, balance))
    }

    async fn open_loan(&self, user_id: i64) -> Result<Option<Loan>, StoreError> {
        let loan = sqlx::query_as::<_, Loan>(
            "SELECT id, user_id, principal, interest, created_at, repaid_at \
             FROM loans WHERE user_id = ? AND repaid_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(loan)
    }

    async fn take_loan(&self, loan: LoanRecord) -> Result<(Loan, i64), StoreError> {
        let mut tx = self.pool.begin().await?;
        let now = Utc::now();

        // The partial unique index on unpaid loans is what stops two concurrent
        // borrows from both inserting. A unique violation here means they
        // already have a marker open.
        let inserted = sqlx::query_as::<_, Loan>(
            "INSERT INTO loans (user_id, principal, interest, created_at) \
             VALUES (?, ?, ?, ?) \
             RETURNING id, user_id, principal, interest, created_at, repaid_at",
        )
        .bind(loan.user_id)
        .bind(loan.principal)
        .bind(loan.interest)
        .bind(now)
        .fetch_one(&mut *tx)
        .await;

        let recorded = match inserted {
            Ok(recorded) => recorded,
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
                return Err(StoreError::LoanOutstanding);
            }
            Err(err) => return Err(err.into()),
        };

        let balance: Option<i64> = sqlx::query_scalar(
            "UPDATE users SET balance = balance + ? WHERE id = ? RETURNING balance",
        )
        .bind(loan.principal)
        .bind(loan.user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(balance) = balance else {
            return Err(StoreError::UnknownUser);
        };

        tx.commit().await?;

        Ok((recorded, balance))
    }

    async fn repay_loan(&self, user_id: i64) -> Result<(Loan, i64), StoreError> {
        let mut tx = self.pool.begin().await?;

        // Claim the marker first so a second concurrent repayment cannot
        // debit the same owed amount. If the player cannot cover it, the
        // transaction rolls back and the marker stays open.
        let claimed: Option<Loan> = sqlx::query_as::<_, Loan>(
            "UPDATE loans SET repaid_at = ? \
             WHERE user_id = ? AND repaid_at IS NULL \
             RETURNING id, user_id, principal, interest, created_at, repaid_at",
        )
        .bind(Utc::now())
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(claimed) = claimed else {
            return Err(StoreError::NoOpenLoan);
        };

        let owed = claimed.owed();

        let balance: Option<i64> = sqlx::query_scalar(
            "UPDATE users SET balance = balance - ? \
             WHERE id = ? AND balance >= ? \
             RETURNING balance",
        )
        .bind(owed)
        .bind(user_id)
        .bind(owed)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(balance) = balance else {
            return Err(StoreError::InsufficientFunds);
        };

        tx.commit().await?;

        Ok((claimed, balance))
    }

    async fn list_players(&self, filter: PlayerFilter<'_>) -> Result<Vec<User>, StoreError> {
        // A single statement with a nullable pattern keeps this one prepared
        // query rather than branching into two.
        let pattern = filter.search.map(|term| format!("%{term}%"));

        let players = sqlx::query_as::<_, User>(
            "SELECT id, username, password_hash, balance, is_banned, created_at, last_login_at \
             FROM users \
             WHERE ?1 IS NULL OR username LIKE ?1 \
             ORDER BY created_at DESC, id DESC \
             LIMIT ?2 OFFSET ?3",
        )
        .bind(pattern)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(players)
    }

    async fn set_player_banned(&self, id: i64, banned: bool) -> Result<bool, StoreError> {
        let result = sqlx::query("UPDATE users SET is_banned = ? WHERE id = ?")
            .bind(banned)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn adjust_player_balance(
        &self,
        adjustment: BalanceAdjustmentRecord<'_>,
    ) -> Result<(User, BalanceAdjustment), StoreError> {
        let mut tx = self.pool.begin().await?;

        // The floor of zero is guarded inside the UPDATE, the same way a spin
        // debit is, so two concurrent adjustments cannot both pass and leave
        // the account negative.
        let player: Option<User> = sqlx::query_as::<_, User>(
            "UPDATE users SET balance = balance + ? \
             WHERE id = ? AND balance + ? >= 0 \
             RETURNING id, username, password_hash, balance, is_banned, created_at, last_login_at",
        )
        .bind(adjustment.delta)
        .bind(adjustment.user_id)
        .bind(adjustment.delta)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(player) = player else {
            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
                .bind(adjustment.user_id)
                .fetch_optional(&mut *tx)
                .await?;

            return Err(if exists.is_some() {
                StoreError::InsufficientFunds
            } else {
                StoreError::UnknownUser
            });
        };

        let balance_after = player.balance;
        let balance_before = balance_after - adjustment.delta;

        let recorded = sqlx::query_as::<_, BalanceAdjustment>(
            "INSERT INTO balance_adjustments \
             (user_id, player_username, operator_id, operator_username, delta, \
              balance_before, balance_after, reason, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
             RETURNING id, user_id, player_username, operator_id, operator_username, \
             delta, balance_before, balance_after, reason, created_at",
        )
        .bind(player.id)
        .bind(&player.username)
        .bind(adjustment.operator_id)
        .bind(adjustment.operator_username)
        .bind(adjustment.delta)
        .bind(balance_before)
        .bind(balance_after)
        .bind(adjustment.reason)
        .bind(Utc::now())
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok((player, recorded))
    }

    async fn list_balance_adjustments(
        &self,
        filter: AdjustmentFilter,
    ) -> Result<Vec<BalanceAdjustment>, StoreError> {
        let rows = sqlx::query_as::<_, BalanceAdjustment>(
            "SELECT id, user_id, player_username, operator_id, operator_username, \
             delta, balance_before, balance_after, reason, created_at \
             FROM balance_adjustments \
             WHERE ?1 IS NULL OR user_id = ?1 \
             ORDER BY created_at DESC, id DESC \
             LIMIT ?2 OFFSET ?3",
        )
        .bind(filter.user_id)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn operator_count(&self) -> Result<i64, StoreError> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM operators")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get("count")?)
    }

    async fn list_operators(&self) -> Result<Vec<Operator>, StoreError> {
        let operators = sqlx::query_as::<_, Operator>(
            "SELECT id, username, password_hash, role, is_active, created_at, last_login_at, \
             password_changed_at \
             FROM operators ORDER BY username COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(operators)
    }

    async fn operator_by_id(&self, id: i64) -> Result<Option<Operator>, StoreError> {
        let operator = sqlx::query_as::<_, Operator>(
            "SELECT id, username, password_hash, role, is_active, created_at, last_login_at, \
             password_changed_at \
             FROM operators WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(operator)
    }

    async fn operator_by_username(&self, username: &str) -> Result<Option<Operator>, StoreError> {
        let operator = sqlx::query_as::<_, Operator>(
            "SELECT id, username, password_hash, role, is_active, created_at, last_login_at, \
             password_changed_at \
             FROM operators WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(operator)
    }

    async fn create_operator(
        &self,
        username: &str,
        password_hash: &str,
        role: OperatorRole,
    ) -> Result<Operator, StoreError> {
        let now = Utc::now();

        let result = sqlx::query_as::<_, Operator>(
            "INSERT INTO operators \
             (username, password_hash, role, is_active, created_at, password_changed_at) \
             VALUES (?, ?, ?, 1, ?, ?) \
             RETURNING id, username, password_hash, role, is_active, created_at, last_login_at, \
             password_changed_at",
        )
        .bind(username)
        .bind(password_hash)
        .bind(role)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(operator) => Ok(operator),
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
                Err(StoreError::DuplicateUsername)
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn update_operator_password(
        &self,
        id: i64,
        password_hash: &str,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE operators SET password_hash = ?, password_changed_at = ? WHERE id = ?",
        )
        .bind(password_hash)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_operator_active(&self, id: i64, is_active: bool) -> Result<bool, StoreError> {
        let result = sqlx::query("UPDATE operators SET is_active = ? WHERE id = ?")
            .bind(is_active)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_operator_role(&self, id: i64, role: OperatorRole) -> Result<bool, StoreError> {
        let result = sqlx::query("UPDATE operators SET role = ? WHERE id = ?")
            .bind(role)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_operator(&self, id: i64) -> Result<bool, StoreError> {
        let result = sqlx::query("DELETE FROM operators WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn record_operator_login(&self, id: i64, at: DateTime<Utc>) -> Result<(), StoreError> {
        sqlx::query("UPDATE operators SET last_login_at = ? WHERE id = ?")
            .bind(at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
