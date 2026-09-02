use actix_web::{HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::db::{SpinRecord, StoreError, VideoPokerDealRecord, VideoPokerSettleRecord};
use crate::error::ApiError;
use crate::games::slots::{self, Outcome, Symbol};
use crate::games::video_poker::{self, Card, Hand};
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
        min_bet: slots::MIN_BET,
        max_bet: slots::MAX_BET,
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

    if !(slots::MIN_BET..=slots::MAX_BET).contains(&bet) {
        return Err(ApiError::BadRequest(format!(
            "bet must be between {} and {} coins",
            slots::MIN_BET,
            slots::MAX_BET
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

#[derive(Debug, Serialize)]
pub struct VideoPokerPaytableResponse {
    pub hands: Vec<video_poker::PaytableEntry>,
    pub min_bet: i64,
    pub max_bet: i64,
    pub cards: usize,
    /// Long-run fraction of stakes returned with optimal holds, for an honest machine.
    pub return_to_player: f64,
}

#[derive(Debug, Deserialize)]
pub struct DealRequest {
    pub bet: i64,
}

#[derive(Debug, Deserialize)]
pub struct DrawRequest {
    pub held: [bool; video_poker::HAND_SIZE],
}

#[derive(Debug, Serialize)]
pub struct DealtHandResponse {
    pub cards: Hand,
    pub bet: i64,
    pub balance: i64,
    pub phase: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DrawnHandResponse {
    pub cards: Hand,
    pub held: [bool; video_poker::HAND_SIZE],
    pub outcome: video_poker::Outcome,
    pub bet: i64,
    /// Gross return, so Jacks or better on a stake of 10 pays back 10.
    pub payout: i64,
    /// What the hand actually did to the balance.
    pub net: i64,
    pub balance: i64,
    pub phase: &'static str,
}

#[derive(Debug, Serialize)]
pub struct OpenHandView {
    pub cards: Hand,
    pub bet: i64,
}

#[derive(Debug, Serialize)]
pub struct OpenHandResponse {
    pub hand: Option<OpenHandView>,
    pub balance: i64,
}

/// The odds and payouts, served rather than duplicated in the client so the
/// two can never disagree about what the machine pays.
#[get("/video-poker/paytable")]
pub async fn video_poker_paytable() -> HttpResponse {
    HttpResponse::Ok().json(VideoPokerPaytableResponse {
        hands: video_poker::paytable(),
        min_bet: video_poker::MIN_BET,
        max_bet: video_poker::MAX_BET,
        cards: video_poker::HAND_SIZE,
        return_to_player: video_poker::theoretical_rtp(),
    })
}

/// The dealt hand waiting on a draw, if the player left the table mid-hand.
#[get("/video-poker/hand")]
pub async fn video_poker_hand(
    state: web::Data<AppState>,
    identity: AuthUser,
) -> Result<HttpResponse, ApiError> {
    let user = state
        .store
        .user_by_id(identity.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    let open = state.store.open_video_poker_hand(identity.id).await?;

    let hand = match open {
        Some(row) => Some(OpenHandView {
            cards: parse_hand(&row.dealt)?,
            bet: row.bet,
        }),
        None => None,
    };

    Ok(HttpResponse::Ok().json(OpenHandResponse {
        hand,
        balance: user.balance,
    }))
}

/// Deals five cards and takes the stake. The remaining stock stays on the
/// server; the client only sees the hand, then posts which cards to hold.
#[post("/video-poker/deal")]
pub async fn video_poker_deal(
    state: web::Data<AppState>,
    identity: AuthUser,
    body: web::Json<DealRequest>,
) -> Result<HttpResponse, ApiError> {
    let bet = body.into_inner().bet;

    if !(video_poker::MIN_BET..=video_poker::MAX_BET).contains(&bet) {
        return Err(ApiError::BadRequest(format!(
            "bet must be between {} and {} coins",
            video_poker::MIN_BET,
            video_poker::MAX_BET
        )));
    }

    let (cards, remaining) = video_poker::deal(&mut rand::thread_rng());
    let dealt = encode_cards(&cards)?;
    let stock = encode_cards(&remaining)?;

    let (_hand, balance) = match state
        .store
        .deal_video_poker(VideoPokerDealRecord {
            user_id: identity.id,
            bet,
            dealt: &dealt,
            remaining: &stock,
        })
        .await
    {
        Ok(dealt) => dealt,
        Err(StoreError::InsufficientFunds) => return Err(ApiError::InsufficientFunds),
        Err(StoreError::HandInProgress) => return Err(ApiError::HandInProgress),
        Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
        Err(err) => return Err(err.into()),
    };

    tracing::info!(user_id = identity.id, bet, balance, "dealt video poker");

    Ok(HttpResponse::Ok().json(DealtHandResponse {
        cards,
        bet,
        balance,
        phase: "dealt",
    }))
}

/// Replaces the cards the player did not hold, then pays the resulting hand.
#[post("/video-poker/draw")]
pub async fn video_poker_draw(
    state: web::Data<AppState>,
    identity: AuthUser,
    body: web::Json<DrawRequest>,
) -> Result<HttpResponse, ApiError> {
    let held = body.into_inner().held;

    let open = state
        .store
        .open_video_poker_hand(identity.id)
        .await?
        .ok_or(ApiError::NoOpenHand)?;

    let dealt = parse_hand(&open.dealt)?;
    let remaining = parse_stock(&open.remaining)?;
    let cards = video_poker::draw(dealt, held, &remaining);
    let outcome = video_poker::evaluate(cards);
    let payout = video_poker::payout(outcome, open.bet);

    let held_json = encode_json(&held)?;
    let drawn_json = encode_cards(&cards)?;
    let outcome_json = encode_json(&outcome)?;

    let balance = match state
        .store
        .settle_video_poker(VideoPokerSettleRecord {
            user_id: identity.id,
            hand_id: open.id,
            held: &held_json,
            drawn: &drawn_json,
            outcome: &outcome_json,
            payout,
        })
        .await
    {
        Ok((_hand, balance)) => balance,
        Err(StoreError::NoOpenHand) => return Err(ApiError::NoOpenHand),
        Err(StoreError::UnknownUser) => return Err(ApiError::Unauthenticated),
        Err(err) => return Err(err.into()),
    };

    tracing::info!(
        user_id = identity.id,
        bet = open.bet,
        payout,
        balance,
        "settled video poker"
    );

    Ok(HttpResponse::Ok().json(DrawnHandResponse {
        cards,
        held,
        outcome,
        bet: open.bet,
        payout,
        net: payout - open.bet,
        balance,
        phase: "settled",
    }))
}

fn encode_cards(cards: &[Card]) -> Result<String, ApiError> {
    encode_json(cards)
}

fn encode_json<T: Serialize + ?Sized>(value: &T) -> Result<String, ApiError> {
    serde_json::to_string(value)
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("could not encode hand: {err}")))
}

fn parse_hand(json: &str) -> Result<Hand, ApiError> {
    let cards: Vec<Card> = parse_stock(json)?;
    cards
        .try_into()
        .map_err(|_| ApiError::Internal(anyhow::anyhow!("stored hand was not five cards")))
}

fn parse_stock(json: &str) -> Result<Vec<Card>, ApiError> {
    serde_json::from_str(json)
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("could not decode hand: {err}")))
}
