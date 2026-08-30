use actix_web::{HttpResponse, get, web};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::bank::LoanView;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub username: String,
    pub balance: i64,
    pub currency: &'static str,
    pub as_of: DateTime<Utc>,
    pub loan: Option<LoanView>,
}

/// Reads the signed-in player's coin balance straight from the store rather
/// than from the token, so it reflects any change made since sign-in.
#[get("/balance")]
pub async fn balance(
    state: web::Data<AppState>,
    identity: AuthUser,
) -> Result<HttpResponse, ApiError> {
    let user = state
        .store
        .user_by_id(identity.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    let loan = state.store.open_loan(identity.id).await?;

    Ok(HttpResponse::Ok().json(BalanceResponse {
        username: user.username,
        balance: user.balance,
        currency: "coins",
        as_of: Utc::now(),
        loan: loan.as_ref().map(LoanView::from),
    }))
}
