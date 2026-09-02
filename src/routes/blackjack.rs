use actix_web::{HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::db::{BlackjackDealRecord, BlackjackSettleRecord, BlackjackUpdateRecord, StoreError};
use crate::error::ApiError;
use crate::games::blackjack::{self, Action, PublicTable, Settlement, Table};
use crate::games::cards::Card;
use crate::models::BlackjackHand;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct RulesResponse {
    pub hands: Vec<blackjack::RuleLine>,
    pub min_bet: i64,
    pub max_bet: i64,
    pub decks: usize,
    pub dealer_hits_soft_17: bool,
    pub blackjack_pays: &'static str,
    pub split: bool,
    pub insurance: bool,
    /// Long-run fraction of stakes returned with a stand-on-17 strategy.
    pub return_to_player: f64,
}

#[derive(Debug, Deserialize)]
pub struct DealRequest {
    pub bet: i64,
}

#[derive(Debug, Deserialize)]
pub struct ActRequest {
    pub action: Action,
}

#[derive(Debug, Serialize)]
pub struct OpenHandResponse {
    pub hand: Option<PublicTable>,
    pub balance: i64,
}

#[derive(Debug, Serialize)]
pub struct HandResponse {
    #[serde(flatten)]
    pub table: PublicTable,
    pub balance: i64,
}

/// Posted rules and payouts, served rather than duplicated in the client.
#[get("/blackjack/rules")]
pub async fn rules() -> HttpResponse {
    HttpResponse::Ok().json(RulesResponse {
        hands: blackjack::rules(),
        min_bet: blackjack::MIN_BET,
        max_bet: blackjack::MAX_BET,
        decks: 1,
        dealer_hits_soft_17: blackjack::DEALER_HITS_SOFT_17,
        blackjack_pays: "3:2",
        split: true,
        insurance: false,
        return_to_player: blackjack::theoretical_rtp(),
    })
}

/// The open table, if the player left mid-hand.
#[get("/blackjack/hand")]
pub async fn hand(
    state: web::Data<AppState>,
    identity: AuthUser,
) -> Result<HttpResponse, ApiError> {
    let user = state
        .store
        .user_by_id(identity.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    let open = state.store.open_blackjack_hand(identity.id).await?;

    match open {
        Some(row) => {
            let table = table_from_row(&row)?;
            Ok(HttpResponse::Ok().json(OpenHandResponse {
                hand: Some(blackjack::publish(&table, None, user.balance >= table.bet)),
                balance: user.balance,
            }))
        }
        None => Ok(HttpResponse::Ok().json(OpenHandResponse {
            hand: None,
            balance: user.balance,
        })),
    }
}

/// Deals two cards each. Naturals settle immediately; otherwise the player acts.
#[post("/blackjack/deal")]
pub async fn deal(
    state: web::Data<AppState>,
    identity: AuthUser,
    body: web::Json<DealRequest>,
) -> Result<HttpResponse, ApiError> {
    let bet = body.into_inner().bet;

    if !(blackjack::MIN_BET..=blackjack::MAX_BET).contains(&bet) {
        return Err(ApiError::BadRequest(format!(
            "bet must be between {} and {} coins",
            blackjack::MIN_BET,
            blackjack::MAX_BET
        )));
    }

    let table = Table::deal(&mut rand::thread_rng(), bet);
    let settlement = table
        .opening()
        .map(|outcome| Settlement::single(outcome, table.stake()));
    let payout = settlement.as_ref().map(Settlement::payout).unwrap_or(0);
    let outcome_json = match settlement.as_ref().and_then(Settlement::outcome) {
        Some(outcome) => Some(encode_json(&outcome)?),
        None => None,
    };

    let player = encode_json(&table.stored_player())?;
    let dealer = encode_cards(&table.dealer)?;
    let remaining = encode_cards(&table.remaining)?;

    let balance = match state
        .store
        .deal_blackjack(BlackjackDealRecord {
            user_id: identity.id,
            bet,
            player: &player,
            dealer: &dealer,
            remaining: &remaining,
            outcome: outcome_json.as_deref(),
            payout,
        })
        .await
    {
        Ok((_hand, balance)) => balance,
        Err(StoreError::InsufficientFunds) => return Err(ApiError::InsufficientFunds),
        Err(StoreError::HandInProgress) => return Err(ApiError::HandInProgress),
        Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
        Err(err) => return Err(err.into()),
    };

    tracing::info!(
        user_id = identity.id,
        bet,
        settled = settlement.is_some(),
        balance,
        "dealt blackjack"
    );

    Ok(HttpResponse::Ok().json(HandResponse {
        table: blackjack::publish(&table, settlement.as_ref(), balance >= bet),
        balance,
    }))
}

/// Hit, stand, double, or split. The hole card is revealed only when the table ends.
#[post("/blackjack/act")]
pub async fn act(
    state: web::Data<AppState>,
    identity: AuthUser,
    body: web::Json<ActRequest>,
) -> Result<HttpResponse, ApiError> {
    let action = body.into_inner().action;

    let open = state
        .store
        .open_blackjack_hand(identity.id)
        .await?
        .ok_or(ApiError::NoOpenHand)?;

    let user = state
        .store
        .user_by_id(identity.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;

    let mut table = table_from_row(&open)?;

    if matches!(action, Action::Double | Action::Split) && user.balance < table.bet {
        return Err(ApiError::InsufficientFunds);
    }

    let stake_before = table.stake();
    let settlement = match table.act(action) {
        Ok(settlement) => settlement,
        Err(blackjack::ActError::Illegal) => {
            return Err(ApiError::BadRequest(
                "that action is not legal on this hand".into(),
            ));
        }
    };
    let extra_bet = table.stake() - stake_before;

    let player = encode_json(&table.stored_player())?;
    let dealer = encode_cards(&table.dealer)?;
    let remaining = encode_cards(&table.remaining)?;

    let balance = if let Some(ref settlement) = settlement {
        let outcome_json = match settlement.outcome() {
            Some(outcome) => encode_json(&outcome)?,
            None => encode_json(
                &settlement
                    .hands
                    .iter()
                    .map(|result| result.outcome)
                    .collect::<Vec<_>>(),
            )?,
        };

        match state
            .store
            .settle_blackjack(BlackjackSettleRecord {
                user_id: identity.id,
                hand_id: open.id,
                version: open.version,
                extra_bet,
                player: &player,
                dealer: &dealer,
                remaining: &remaining,
                doubled: table.any_doubled(),
                outcome: &outcome_json,
                payout: settlement.payout(),
            })
            .await
        {
            Ok((_hand, balance)) => balance,
            Err(StoreError::NoOpenHand) => return Err(ApiError::NoOpenHand),
            Err(StoreError::InsufficientFunds) => return Err(ApiError::InsufficientFunds),
            Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
            Err(err) => return Err(err.into()),
        }
    } else {
        match state
            .store
            .update_blackjack(BlackjackUpdateRecord {
                user_id: identity.id,
                hand_id: open.id,
                version: open.version,
                extra_bet,
                player: &player,
                remaining: &remaining,
            })
            .await
        {
            Ok((_hand, balance)) => balance,
            Err(StoreError::NoOpenHand) => return Err(ApiError::NoOpenHand),
            Err(StoreError::InsufficientFunds) => return Err(ApiError::InsufficientFunds),
            Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
            Err(err) => return Err(err.into()),
        }
    };

    tracing::info!(
        user_id = identity.id,
        bet = table.bet,
        settled = settlement.is_some(),
        balance,
        "acted on blackjack"
    );

    Ok(HttpResponse::Ok().json(HandResponse {
        table: blackjack::publish(&table, settlement.as_ref(), balance >= table.bet),
        balance,
    }))
}

fn table_from_row(row: &BlackjackHand) -> Result<Table, ApiError> {
    let (hands, active) = blackjack::parse_player(&row.player)
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("could not decode hand: {err}")))?;

    Ok(Table {
        hands,
        active,
        dealer: decode_cards(&row.dealer)?,
        remaining: decode_cards(&row.remaining)?,
        bet: row.bet,
    })
}

fn encode_cards(cards: &[Card]) -> Result<String, ApiError> {
    encode_json(cards)
}

fn encode_json<T: Serialize + ?Sized>(value: &T) -> Result<String, ApiError> {
    serde_json::to_string(value)
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("could not encode hand: {err}")))
}

fn decode_cards(json: &str) -> Result<Vec<Card>, ApiError> {
    serde_json::from_str(json)
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("could not decode hand: {err}")))
}
