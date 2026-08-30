use actix_web::{HttpResponse, get, post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::db::{LoanRecord, StoreError};
use crate::error::ApiError;
use crate::games::bank::{self, Offer};
use crate::models::Loan;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct LoanView {
    pub id: i64,
    pub principal: i64,
    pub interest: i64,
    pub owed: i64,
    pub created_at: DateTime<Utc>,
}

impl From<&Loan> for LoanView {
    fn from(loan: &Loan) -> Self {
        Self {
            id: loan.id,
            principal: loan.principal,
            interest: loan.interest,
            owed: loan.owed(),
            created_at: loan.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BankResponse {
    /// Interest as basis points of the principal. 2_000 is 20%.
    pub interest_bps: i64,
    pub offers: Vec<Offer>,
    pub loan: Option<LoanView>,
    pub balance: i64,
}

#[derive(Debug, Deserialize)]
pub struct BorrowRequest {
    pub amount: i64,
}

#[derive(Debug, Serialize)]
pub struct LoanSettlement {
    pub loan: Option<LoanView>,
    pub balance: i64,
}

/// Posted terms and the signed-in player's marker, if they have one.
#[get("/bank")]
pub async fn terms(
    state: web::Data<AppState>,
    identity: AuthUser,
) -> Result<HttpResponse, ApiError> {
    let (user, loan) = load_wallet(&state, identity.id).await?;

    Ok(HttpResponse::Ok().json(BankResponse {
        interest_bps: bank::INTEREST_BPS,
        offers: bank::offers(),
        loan: loan.as_ref().map(LoanView::from),
        balance: user.balance,
    }))
}

/// Writes a marker and credits the principal. The cashier will not open a
/// second one while the first is still unpaid.
#[post("/bank/borrow")]
pub async fn borrow(
    state: web::Data<AppState>,
    identity: AuthUser,
    body: web::Json<BorrowRequest>,
) -> Result<HttpResponse, ApiError> {
    let amount = body.into_inner().amount;
    let Some(offer) = bank::quote(amount) else {
        return Err(ApiError::BadRequest(
            "the bank does not offer a loan of that size".into(),
        ));
    };

    let (loan, balance) = match state
        .store
        .take_loan(LoanRecord {
            user_id: identity.id,
            principal: offer.principal,
            interest: offer.interest,
        })
        .await
    {
        Ok(settled) => settled,
        Err(StoreError::LoanOutstanding) => return Err(ApiError::LoanOutstanding),
        Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
        Err(err) => return Err(err.into()),
    };

    tracing::info!(
        user_id = identity.id,
        principal = loan.principal,
        interest = loan.interest,
        balance,
        "opened a loan"
    );

    Ok(HttpResponse::Ok().json(LoanSettlement {
        loan: Some(LoanView::from(&loan)),
        balance,
    }))
}

/// Pays the outstanding marker in full. Partial payments are refused so the
/// books stay a single debit, the same way a spin settles.
#[post("/bank/repay")]
pub async fn repay(
    state: web::Data<AppState>,
    identity: AuthUser,
) -> Result<HttpResponse, ApiError> {
    let (_loan, balance) = match state.store.repay_loan(identity.id).await {
        Ok(settled) => settled,
        Err(StoreError::NoOpenLoan) => return Err(ApiError::NoOpenLoan),
        Err(StoreError::InsufficientFunds) => return Err(ApiError::InsufficientFunds),
        Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
        Err(err) => return Err(err.into()),
    };

    tracing::info!(user_id = identity.id, balance, "repaid a loan");

    Ok(HttpResponse::Ok().json(LoanSettlement {
        loan: None,
        balance,
    }))
}

async fn load_wallet(
    state: &AppState,
    user_id: i64,
) -> Result<(crate::models::User, Option<Loan>), ApiError> {
    let user = state
        .store
        .user_by_id(user_id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    let loan = state.store.open_loan(user_id).await?;
    Ok((user, loan))
}
