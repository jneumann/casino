use actix_web::{HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::db::{SpinRecord, StoreError};
use crate::error::ApiError;
use crate::games::slots::{self, MAX_BET, MIN_BET, Outcome, Symbol};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SpinRequest {
    pub bet: i64,
}

#[derive(Debug, Serialize)]
pub struct SpinResponse {
    pub reels: [Symbol; slots::REELS],
    pub outcome: Outcome,
    pub bet: i64,
    /// Gross return, so a winning stake of 10 at 1x pays back 10.
    pub payout: i64,
    /// What the spin actually did to the balance.
    pub net: i64,
    pub balance: i64,
}

#[derive(Debug, Serialize)]
pub struct PaytableResponse {
    pub symbols: Vec<slots::PaytableEntry>,
    pub min_bet: i64,
    pub max_bet: i64,
    pub reels: usize,
    /// Long-run fraction of stakes returned to players, for an honest machine.
    pub return_to_player: f64,
}

/// The odds and payouts, served rather than duplicated in the client so the
/// two can never disagree about what the machine pays.
#[get("/slots/paytable")]
pub async fn paytable() -> HttpResponse {
    HttpResponse::Ok().json(PaytableResponse {
        symbols: slots::paytable(),
        min_bet: MIN_BET,
        max_bet: MAX_BET,
        reels: slots::REELS,
        return_to_player: slots::theoretical_rtp(),
    })
}

/// Spins the reels. The result is decided here, never by the client, and the
/// stake and winnings move in a single transaction.
#[post("/slots/spin")]
pub async fn spin(
    state: web::Data<AppState>,
    identity: AuthUser,
    body: web::Json<SpinRequest>,
) -> Result<HttpResponse, ApiError> {
    let bet = body.into_inner().bet;

    if !(MIN_BET..=MAX_BET).contains(&bet) {
        return Err(ApiError::BadRequest(format!(
            "bet must be between {MIN_BET} and {MAX_BET} coins"
        )));
    }

    let reels = slots::spin(&mut rand::thread_rng());
    let outcome = slots::evaluate(reels);
    let payout = slots::payout(outcome, bet);

    let encoded = serde_json::to_string(&reels)
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("could not encode reels: {err}")))?;

    let balance = match state
        .store
        .settle_spin(SpinRecord {
            user_id: identity.id,
            bet,
            payout,
            reels: &encoded,
        })
        .await
    {
        Ok(balance) => balance,
        Err(StoreError::InsufficientFunds) => return Err(ApiError::InsufficientFunds),
        Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
        Err(err) => return Err(err.into()),
    };

    tracing::info!(
        user_id = identity.id,
        bet,
        payout,
        balance,
        "settled a spin"
    );

    Ok(HttpResponse::Ok().json(SpinResponse {
        reels,
        outcome,
        bet,
        payout,
        net: payout - bet,
        balance,
    }))
}
