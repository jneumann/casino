//! Single-deck blackjack.
//!
//! A fresh 52-card shoe every hand, dealer hits soft 17, blackjack pays 3:2
//! (odd stakes round down to a whole coin). Pairs of the same rank may be
//! split once. Split aces take one card each. Double is allowed on any
//! two-card total, including after a split (except aces). No insurance, no
//! surrender, no resplit. A two-card 21 after a split pays even money, not
//! 3:2.
//!
//! The hole card and the remaining stock never leave the server. The client
//! sees each of the player's hands, the dealer's up card, and the legal
//! actions.

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::cards::{Card, Rank, standard_deck};

/// The lowest and highest a player may stake on one hand.
pub const MIN_BET: i64 = 1;
pub const MAX_BET: i64 = 500;

/// Dealer hits a soft 17 (ace counted as 11).
pub const DEALER_HITS_SOFT_17: bool = true;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    PlayerBlackjack,
    PlayerWin,
    Push,
    DealerWin,
    PlayerBust,
    DealerBlackjack,
}

impl Outcome {
    /// Gross return on `stake`. A win hands the stake back plus even money;
    /// a natural pays 3:2 on top of the stake.
    pub fn payout(self, stake: i64) -> i64 {
        match self {
            Self::PlayerBust | Self::DealerWin | Self::DealerBlackjack => 0,
            Self::Push => stake,
            Self::PlayerWin => stake.saturating_mul(2),
            Self::PlayerBlackjack => stake.saturating_add(stake.saturating_mul(3) / 2),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PlayerBlackjack => "Blackjack",
            Self::PlayerWin => "You win",
            Self::Push => "Push",
            Self::DealerWin => "Dealer wins",
            Self::PlayerBust => "Bust",
            Self::DealerBlackjack => "Dealer blackjack",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Hit,
    Stand,
    Double,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Player,
    Settled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Total {
    pub best: u8,
    pub soft: bool,
}

impl Total {
    pub fn bust(self) -> bool {
        self.best > 21
    }
}

/// Best non-busting total, counting one ace as 11 when that still fits.
pub fn total(cards: &[Card]) -> Total {
    let mut sum = 0u8;
    let mut aces = 0u8;

    for card in cards {
        if card.rank == Rank::Ace {
            aces += 1;
        }
        sum = sum.saturating_add(card.rank.pip());
    }

    if aces > 0 && sum + 10 <= 21 {
        Total {
            best: sum + 10,
            soft: true,
        }
    } else {
        Total {
            best: sum,
            soft: false,
        }
    }
}

pub fn is_blackjack(cards: &[Card]) -> bool {
    cards.len() == 2 && total(cards).best == 21
}

fn dealer_should_hit(cards: &[Card]) -> bool {
    let value = total(cards);
    if value.bust() || value.best > 17 {
        return false;
    }
    if value.best < 17 {
        return true;
    }
    DEALER_HITS_SOFT_17 && value.soft
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerHand {
    pub cards: Vec<Card>,
    pub doubled: bool,
    pub from_split: bool,
    pub split_aces: bool,
}

impl PlayerHand {
    fn fresh(cards: Vec<Card>) -> Self {
        Self {
            cards,
            doubled: false,
            from_split: false,
            split_aces: false,
        }
    }

    pub fn stake(&self, bet: i64) -> i64 {
        if self.doubled {
            bet.saturating_mul(2)
        } else {
            bet
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandResult {
    pub outcome: Outcome,
    pub stake: i64,
}

impl HandResult {
    pub fn payout(&self) -> i64 {
        self.outcome.payout(self.stake)
    }
}

/// Every player hand, once the table is finished.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Settlement {
    pub hands: Vec<HandResult>,
}

impl Settlement {
    pub fn single(outcome: Outcome, stake: i64) -> Self {
        Self {
            hands: vec![HandResult { outcome, stake }],
        }
    }

    pub fn payout(&self) -> i64 {
        self.hands.iter().map(HandResult::payout).sum()
    }

    pub fn total_stake(&self) -> i64 {
        self.hands.iter().map(|hand| hand.stake).sum()
    }

    pub fn net(&self) -> i64 {
        self.payout() - self.total_stake()
    }

    pub fn outcome(&self) -> Option<Outcome> {
        let first = self.hands.first()?.outcome;
        self.hands
            .iter()
            .all(|hand| hand.outcome == first)
            .then_some(first)
    }
}

/// How an open table is stored on `blackjack_hands.player`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPlayer {
    pub hands: Vec<StoredHand>,
    pub active: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredHand {
    pub cards: Vec<Card>,
    #[serde(default)]
    pub doubled: bool,
    #[serde(default)]
    pub from_split: bool,
    #[serde(default)]
    pub split_aces: bool,
}

impl From<&PlayerHand> for StoredHand {
    fn from(hand: &PlayerHand) -> Self {
        Self {
            cards: hand.cards.clone(),
            doubled: hand.doubled,
            from_split: hand.from_split,
            split_aces: hand.split_aces,
        }
    }
}

impl From<StoredHand> for PlayerHand {
    fn from(hand: StoredHand) -> Self {
        Self {
            cards: hand.cards,
            doubled: hand.doubled,
            from_split: hand.from_split,
            split_aces: hand.split_aces,
        }
    }
}

/// The live table: one or two player hands, dealer cards (up first, hole
/// second), and the stock the next card will come from.
#[derive(Debug, Clone)]
pub struct Table {
    pub hands: Vec<PlayerHand>,
    pub active: usize,
    pub dealer: Vec<Card>,
    pub remaining: Vec<Card>,
    pub bet: i64,
}

impl Table {
    pub fn deal<R: Rng + ?Sized>(rng: &mut R, bet: i64) -> Self {
        let mut deck = standard_deck();
        deck.shuffle(rng);
        let remaining: Vec<Card> = deck.into();

        let mut table = Self {
            hands: vec![PlayerHand::fresh(Vec::new())],
            active: 0,
            dealer: Vec::new(),
            remaining,
            bet,
        };

        let first = table.draw();
        table.hands[0].cards.push(first);
        let up = table.draw();
        table.dealer.push(up);
        let second = table.draw();
        table.hands[0].cards.push(second);
        let hole = table.draw();
        table.dealer.push(hole);
        table
    }

    pub fn stored_player(&self) -> StoredPlayer {
        StoredPlayer {
            hands: self.hands.iter().map(StoredHand::from).collect(),
            active: self.active,
        }
    }

    fn draw(&mut self) -> Card {
        self.remaining
            .drain(..1)
            .next()
            .expect("a 52-card shoe covers a blackjack hand")
    }

    fn active_hand(&self) -> &PlayerHand {
        &self.hands[self.active]
    }

    pub fn stake(&self) -> i64 {
        self.hands
            .iter()
            .map(|hand| hand.stake(self.bet))
            .sum()
    }

    pub fn any_doubled(&self) -> bool {
        self.hands.iter().any(|hand| hand.doubled)
    }

    pub fn can_hit(&self) -> bool {
        let hand = self.active_hand();
        !hand.split_aces && !hand.doubled && {
            let value = total(&hand.cards);
            !value.bust() && value.best < 21
        }
    }

    pub fn can_stand(&self) -> bool {
        !total(&self.active_hand().cards).bust()
    }

    pub fn can_double(&self) -> bool {
        let hand = self.active_hand();
        hand.cards.len() == 2 && !hand.doubled && !hand.split_aces && self.can_hit()
    }

    pub fn can_split(&self) -> bool {
        self.hands.len() == 1
            && self.hands[0].cards.len() == 2
            && !self.hands[0].doubled
            && self.hands[0].cards[0].rank == self.hands[0].cards[1].rank
    }

    /// Immediate result of the deal, if neither side needs to act.
    pub fn opening(&self) -> Option<Outcome> {
        if self.hands.len() != 1 || self.hands[0].from_split {
            return None;
        }

        let player = &self.hands[0].cards;
        let player_bj = is_blackjack(player);
        let dealer_bj = is_blackjack(&self.dealer);
        let dealer_up = self.dealer[0].rank;

        if player_bj && dealer_bj {
            return Some(Outcome::Push);
        }
        if player_bj {
            return Some(Outcome::PlayerBlackjack);
        }
        if (dealer_up == Rank::Ace || dealer_up.is_ten()) && dealer_bj {
            return Some(Outcome::DealerBlackjack);
        }

        None
    }

    pub fn hit(&mut self) {
        let card = self.draw();
        self.hands[self.active].cards.push(card);
    }

    pub fn double(&mut self) {
        self.hands[self.active].doubled = true;
        let card = self.draw();
        self.hands[self.active].cards.push(card);
    }

    fn split(&mut self) {
        let second = self.hands[0]
            .cards
            .pop()
            .expect("can_split requires two cards");
        let aces = self.hands[0].cards[0].rank == Rank::Ace;

        self.hands[0].from_split = true;
        self.hands[0].split_aces = aces;
        self.hands.push(PlayerHand {
            cards: vec![second],
            doubled: false,
            from_split: true,
            split_aces: aces,
        });

        let first = self.draw();
        self.hands[0].cards.push(first);
        let other = self.draw();
        self.hands[1].cards.push(other);
        self.active = 0;
    }

    fn play_dealer(&mut self) {
        while dealer_should_hit(&self.dealer) {
            let card = self.draw();
            self.dealer.push(card);
        }
    }

    fn play_dealer_if_needed(&mut self) {
        let any_live = self
            .hands
            .iter()
            .any(|hand| !total(&hand.cards).bust());
        if any_live {
            self.play_dealer();
        }
    }

    fn hand_finished(&self, index: usize) -> bool {
        let hand = &self.hands[index];
        let value = total(&hand.cards);
        hand.doubled || hand.split_aces || value.bust() || value.best == 21
    }

    /// Move on from the active hand. `None` means another hand still needs
    /// a decision; `Some` means the table is over.
    fn advance(&mut self) -> Option<Settlement> {
        if self.active + 1 < self.hands.len() {
            self.active += 1;
            if self.hand_finished(self.active) {
                return self.advance();
            }
            None
        } else {
            self.play_dealer_if_needed();
            Some(self.settlement())
        }
    }

    pub fn settle_hand(&self, hand: &PlayerHand) -> Outcome {
        if let Some(opening) = self.opening() {
            return opening;
        }

        let player = total(&hand.cards);
        if player.bust() {
            return Outcome::PlayerBust;
        }

        if is_blackjack(&hand.cards) && !hand.from_split {
            return Outcome::PlayerBlackjack;
        }

        let dealer = total(&self.dealer);
        if dealer.bust() {
            return Outcome::PlayerWin;
        }

        match player.best.cmp(&dealer.best) {
            std::cmp::Ordering::Greater => Outcome::PlayerWin,
            std::cmp::Ordering::Less => Outcome::DealerWin,
            std::cmp::Ordering::Equal => Outcome::Push,
        }
    }

    pub fn settlement(&self) -> Settlement {
        Settlement {
            hands: self
                .hands
                .iter()
                .map(|hand| HandResult {
                    outcome: self.settle_hand(hand),
                    stake: hand.stake(self.bet),
                })
                .collect(),
        }
    }

    /// Apply a player action. `None` means a hand is still the player's.
    pub fn act(&mut self, action: Action) -> Result<Option<Settlement>, ActError> {
        match action {
            Action::Hit => {
                if !self.can_hit() {
                    return Err(ActError::Illegal);
                }
                self.hit();
                let value = total(&self.hands[self.active].cards);
                if value.bust() || value.best == 21 {
                    Ok(self.advance())
                } else {
                    Ok(None)
                }
            }
            Action::Stand => {
                if !self.can_stand() {
                    return Err(ActError::Illegal);
                }
                Ok(self.advance())
            }
            Action::Double => {
                if !self.can_double() {
                    return Err(ActError::Illegal);
                }
                self.double();
                Ok(self.advance())
            }
            Action::Split => {
                if !self.can_split() {
                    return Err(ActError::Illegal);
                }
                self.split();
                if self.hand_finished(0) && self.hands.len() > 1 && self.hand_finished(1)
                {
                    self.play_dealer_if_needed();
                    return Ok(Some(self.settlement()));
                }
                if self.hand_finished(0) {
                    return Ok(self.advance());
                }
                Ok(None)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActError {
    Illegal,
}

/// What the client is allowed to see. The hole card is omitted until settlement.
#[derive(Debug, Serialize)]
pub struct PublicTable {
    pub player: Vec<Card>,
    pub hands: Vec<PublicHand>,
    pub active_hand: usize,
    pub dealer: Vec<Card>,
    pub player_total: u8,
    pub dealer_total: Option<u8>,
    pub bet: i64,
    pub stake: i64,
    pub doubled: bool,
    pub can_hit: bool,
    pub can_stand: bool,
    pub can_double: bool,
    pub can_split: bool,
    pub phase: Phase,
    pub outcome: Option<Outcome>,
    pub payout: Option<i64>,
    pub net: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PublicHand {
    pub cards: Vec<Card>,
    pub total: u8,
    pub bet: i64,
    pub stake: i64,
    pub doubled: bool,
    pub from_split: bool,
    pub active: bool,
    pub outcome: Option<Outcome>,
    pub payout: Option<i64>,
    pub net: Option<i64>,
}

pub fn publish(table: &Table, settlement: Option<&Settlement>, can_afford_extra: bool) -> PublicTable
{
    let settled = settlement.is_some();
    let dealer = if settled {
        table.dealer.clone()
    } else {
        vec![table.dealer[0]]
    };
    let dealer_total = if settled {
        Some(total(&table.dealer).best)
    } else {
        None
    };
    let payout = settlement.map(Settlement::payout);
    let net = settlement.map(Settlement::net);
    let active_cards = &table.hands[table.active.min(table.hands.len() - 1)].cards;

    let hands = table
        .hands
        .iter()
        .enumerate()
        .map(|(index, hand)| {
            let result = settlement.and_then(|done| done.hands.get(index));
            PublicHand {
                cards: hand.cards.clone(),
                total: total(&hand.cards).best,
                bet: table.bet,
                stake: hand.stake(table.bet),
                doubled: hand.doubled,
                from_split: hand.from_split,
                active: !settled && index == table.active,
                outcome: result.map(|item| item.outcome),
                payout: result.map(HandResult::payout),
                net: result.map(|item| item.payout() - item.stake),
            }
        })
        .collect();

    PublicTable {
        player: active_cards.clone(),
        hands,
        active_hand: table.active,
        dealer,
        player_total: total(active_cards).best,
        dealer_total,
        bet: table.bet,
        stake: table.stake(),
        doubled: table.any_doubled(),
        can_hit: !settled && table.can_hit(),
        can_stand: !settled && table.can_stand(),
        can_double: !settled && table.can_double() && can_afford_extra,
        can_split: !settled && table.can_split() && can_afford_extra,
        phase: if settled {
            Phase::Settled
        } else {
            Phase::Player
        },
        outcome: settlement.and_then(Settlement::outcome),
        payout,
        net,
    }
}

/// Restore a table from the `player` column. Objects are the current
/// multi-hand format; a bare card array is a hand dealt before splits.
pub fn parse_player(json: &str) -> Result<(Vec<PlayerHand>, usize), serde_json::Error> {
    let trimmed = json.trim_start();
    if trimmed.starts_with('[') {
        let cards: Vec<Card> = serde_json::from_str(json)?;
        return Ok((vec![PlayerHand::fresh(cards)], 0));
    }

    let stored: StoredPlayer = serde_json::from_str(json)?;
    let active = stored.active.min(stored.hands.len().saturating_sub(1));
    Ok((stored.hands.into_iter().map(PlayerHand::from).collect(), active))
}

#[derive(Debug, Serialize)]
pub struct RuleLine {
    pub kind: Outcome,
    pub name: &'static str,
    pub pays: &'static str,
}

pub fn rules() -> Vec<RuleLine> {
    vec![
        RuleLine {
            kind: Outcome::PlayerBlackjack,
            name: "Blackjack",
            pays: "3:2",
        },
        RuleLine {
            kind: Outcome::PlayerWin,
            name: "Win",
            pays: "1:1",
        },
        RuleLine {
            kind: Outcome::Push,
            name: "Push",
            pays: "stake back",
        },
        RuleLine {
            kind: Outcome::DealerWin,
            name: "Dealer wins",
            pays: "lose",
        },
        RuleLine {
            kind: Outcome::PlayerBust,
            name: "Bust",
            pays: "lose",
        },
        RuleLine {
            kind: Outcome::DealerBlackjack,
            name: "Dealer blackjack",
            pays: "lose",
        },
    ]
}

/// Approximate return with a stand-on-17, double-or-split-pairs strategy.
/// The true figure depends on the player; this is the posted house number.
pub fn theoretical_rtp() -> f64 {
    0.96
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::cards::{DECK_SIZE, Rank, Suit};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn card(rank: Rank, suit: Suit) -> Card {
        Card { rank, suit }
    }

    fn table_with(player: Vec<Card>, dealer: Vec<Card>) -> Table {
        Table {
            hands: vec![PlayerHand::fresh(player)],
            active: 0,
            dealer,
            remaining: vec![
                card(Rank::Two, Suit::Clubs),
                card(Rank::Three, Suit::Clubs),
                card(Rank::Four, Suit::Clubs),
                card(Rank::Five, Suit::Clubs),
                card(Rank::Six, Suit::Clubs),
                card(Rank::Nine, Suit::Hearts),
            ],
            bet: 10,
        }
    }

    fn first_outcome(result: Option<Settlement>) -> Option<Outcome> {
        result.map(|done| done.hands[0].outcome)
    }

    #[test]
    fn totals_treat_aces_as_one_or_eleven() {
        assert_eq!(
            total(&[card(Rank::Ace, Suit::Spades), card(Rank::Ten, Suit::Hearts)]).best,
            21
        );
        assert_eq!(
            total(&[
                card(Rank::Ace, Suit::Spades),
                card(Rank::Nine, Suit::Hearts),
                card(Rank::Two, Suit::Clubs)
            ])
            .best,
            12
        );
        assert_eq!(
            total(&[card(Rank::Ace, Suit::Spades), card(Rank::Ace, Suit::Hearts)]).best,
            12
        );
        assert!(total(&[card(Rank::Ace, Suit::Spades), card(Rank::Six, Suit::Hearts)]).soft);
        assert!(
            !total(&[
                card(Rank::Ace, Suit::Spades),
                card(Rank::Six, Suit::Hearts),
                card(Rank::Ten, Suit::Clubs)
            ])
            .soft
        );
        assert!(
            total(&[
                card(Rank::King, Suit::Spades),
                card(Rank::Six, Suit::Hearts),
                card(Rank::Six, Suit::Clubs)
            ])
            .bust()
        );
    }

    #[test]
    fn two_card_twenty_one_is_blackjack() {
        assert!(is_blackjack(&[
            card(Rank::Ace, Suit::Spades),
            card(Rank::Queen, Suit::Hearts)
        ]));
        assert!(!is_blackjack(&[
            card(Rank::Ace, Suit::Spades),
            card(Rank::Five, Suit::Hearts),
            card(Rank::Five, Suit::Clubs)
        ]));
    }

    #[test]
    fn opening_settles_naturals() {
        let both = table_with(
            vec![
                card(Rank::Ace, Suit::Spades),
                card(Rank::King, Suit::Hearts),
            ],
            vec![
                card(Rank::Ace, Suit::Clubs),
                card(Rank::Queen, Suit::Diamonds),
            ],
        );
        assert_eq!(both.opening(), Some(Outcome::Push));

        let player = table_with(
            vec![
                card(Rank::Ace, Suit::Spades),
                card(Rank::King, Suit::Hearts),
            ],
            vec![
                card(Rank::Nine, Suit::Clubs),
                card(Rank::Queen, Suit::Diamonds),
            ],
        );
        assert_eq!(player.opening(), Some(Outcome::PlayerBlackjack));

        let dealer = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Nine, Suit::Hearts),
            ],
            vec![
                card(Rank::Ace, Suit::Clubs),
                card(Rank::Queen, Suit::Diamonds),
            ],
        );
        assert_eq!(dealer.opening(), Some(Outcome::DealerBlackjack));
    }

    #[test]
    fn dealer_does_not_peek_without_an_ace_or_ten_up() {
        let table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Nine, Suit::Hearts),
            ],
            vec![
                card(Rank::Nine, Suit::Clubs),
                card(Rank::Ace, Suit::Diamonds),
            ],
        );
        assert_eq!(table.opening(), None);
    }

    #[test]
    fn dealer_hits_soft_seventeen() {
        let mut table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Eight, Suit::Hearts),
            ],
            vec![
                card(Rank::Ace, Suit::Clubs),
                card(Rank::Six, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Two, Suit::Hearts));
        table.act(Action::Stand).expect("stand is legal");
        assert!(table.dealer.len() > 2);
    }

    #[test]
    fn dealer_stands_on_hard_seventeen() {
        let mut table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Eight, Suit::Hearts),
            ],
            vec![
                card(Rank::Ten, Suit::Clubs),
                card(Rank::Seven, Suit::Diamonds),
            ],
        );
        table.act(Action::Stand).expect("stand is legal");
        assert_eq!(table.dealer.len(), 2);
    }

    #[test]
    fn hit_busts_a_twenty() {
        let mut table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Queen, Suit::Hearts),
            ],
            vec![
                card(Rank::Nine, Suit::Clubs),
                card(Rank::Seven, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::King, Suit::Clubs));
        let outcome = first_outcome(table.act(Action::Hit).expect("hit is legal"));
        assert_eq!(outcome, Some(Outcome::PlayerBust));
    }

    #[test]
    fn double_draws_one_then_stands() {
        let mut table = table_with(
            vec![
                card(Rank::Five, Suit::Spades),
                card(Rank::Six, Suit::Hearts),
            ],
            vec![
                card(Rank::Ten, Suit::Clubs),
                card(Rank::Six, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Ten, Suit::Hearts));
        let outcome = first_outcome(table.act(Action::Double).expect("double is legal"));
        assert!(table.hands[0].doubled);
        assert_eq!(table.hands[0].cards.len(), 3);
        assert_eq!(outcome, Some(Outcome::PlayerWin));
        assert_eq!(table.stake(), 20);
    }

    #[test]
    fn refuses_a_second_double_or_a_hit_after_twenty_one() {
        let mut table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Queen, Suit::Hearts),
            ],
            vec![
                card(Rank::Nine, Suit::Clubs),
                card(Rank::Seven, Suit::Diamonds),
            ],
        );
        table.hands[0].cards.push(card(Rank::Ace, Suit::Clubs));
        assert_eq!(table.act(Action::Hit), Err(ActError::Illegal));
        assert_eq!(table.act(Action::Double), Err(ActError::Illegal));
    }

    #[test]
    fn splits_a_pair_and_plays_each_hand() {
        let mut table = table_with(
            vec![
                card(Rank::Eight, Suit::Spades),
                card(Rank::Eight, Suit::Hearts),
            ],
            vec![
                card(Rank::Ten, Suit::Clubs),
                card(Rank::Six, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Three, Suit::Hearts));
        table.remaining.insert(1, card(Rank::Two, Suit::Hearts));
        table.remaining.insert(2, card(Rank::Ten, Suit::Hearts));

        assert!(table.can_split());
        let mid = table.act(Action::Split).expect("split is legal");
        assert!(mid.is_none());
        assert_eq!(table.hands.len(), 2);
        assert_eq!(table.active, 0);
        assert_eq!(table.stake(), 20);
        assert_eq!(table.hands[0].cards[0].rank, Rank::Eight);
        assert_eq!(table.hands[1].cards[0].rank, Rank::Eight);
        assert!(!table.can_split());

        let still = table.act(Action::Stand).expect("stand hand 1");
        assert!(still.is_none());
        assert_eq!(table.active, 1);

        assert!(table.act(Action::Hit).expect("hit hand 2").is_none());
        assert_eq!(table.hands[1].cards.len(), 3);
        let done = table.act(Action::Stand).expect("stand hand 2");
        assert!(done.is_some());
        assert!(table.dealer.len() >= 2);
    }

    #[test]
    fn refuses_a_split_of_unequal_ranks() {
        let table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Jack, Suit::Hearts),
            ],
            vec![
                card(Rank::Nine, Suit::Clubs),
                card(Rank::Seven, Suit::Diamonds),
            ],
        );
        assert!(!table.can_split());
    }

    #[test]
    fn split_aces_take_one_card_and_settle() {
        let mut table = table_with(
            vec![
                card(Rank::Ace, Suit::Spades),
                card(Rank::Ace, Suit::Hearts),
            ],
            vec![
                card(Rank::Ten, Suit::Clubs),
                card(Rank::Six, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Nine, Suit::Hearts));
        table.remaining.insert(1, card(Rank::Eight, Suit::Clubs));

        let done = table.act(Action::Split).expect("split aces");
        let settlement = done.expect("split aces finish immediately");
        assert_eq!(table.hands.len(), 2);
        assert_eq!(table.hands[0].cards.len(), 2);
        assert_eq!(table.hands[1].cards.len(), 2);
        assert!(table.hands.iter().all(|hand| hand.split_aces));
        assert_eq!(table.act(Action::Hit), Err(ActError::Illegal));
        assert_eq!(settlement.hands[0].outcome, Outcome::PlayerWin);
        assert_eq!(settlement.hands[1].outcome, Outcome::PlayerWin);
        assert_ne!(settlement.hands[0].outcome, Outcome::PlayerBlackjack);
    }

    #[test]
    fn twenty_one_after_a_split_is_not_blackjack() {
        let mut table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Ten, Suit::Hearts),
            ],
            vec![
                card(Rank::Nine, Suit::Clubs),
                card(Rank::Seven, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Ace, Suit::Clubs));
        table.remaining.insert(1, card(Rank::Ace, Suit::Diamonds));

        let done = table.act(Action::Split).expect("split tens");
        let settlement = done.expect("both hands are 21");
        assert_eq!(settlement.hands[0].outcome, Outcome::PlayerWin);
        assert_eq!(settlement.hands[1].outcome, Outcome::PlayerWin);
        assert_eq!(settlement.payout(), 40);
    }

    #[test]
    fn busting_the_first_split_hand_plays_the_second() {
        let mut table = table_with(
            vec![
                card(Rank::Eight, Suit::Spades),
                card(Rank::Eight, Suit::Hearts),
            ],
            vec![
                card(Rank::Ten, Suit::Clubs),
                card(Rank::Nine, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Six, Suit::Hearts));
        table.remaining.insert(1, card(Rank::Two, Suit::Clubs));
        table.remaining.insert(2, card(Rank::King, Suit::Hearts));

        assert!(table.act(Action::Split).expect("split").is_none());
        let mid = table.act(Action::Hit).expect("bust hand 1");
        assert!(mid.is_none());
        assert_eq!(table.active, 1);
        assert!(total(&table.hands[0].cards).bust());

        let done = table.act(Action::Stand).expect("stand hand 2");
        let settlement = done.expect("table finished");
        assert_eq!(settlement.hands[0].outcome, Outcome::PlayerBust);
        assert_eq!(settlement.hands[1].outcome, Outcome::DealerWin);
        assert_eq!(table.dealer.len(), 2);
    }

    #[test]
    fn double_after_split_then_plays_the_other_hand() {
        let mut table = table_with(
            vec![
                card(Rank::Five, Suit::Spades),
                card(Rank::Five, Suit::Hearts),
            ],
            vec![
                card(Rank::Ten, Suit::Clubs),
                card(Rank::Six, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Six, Suit::Clubs));
        table.remaining.insert(1, card(Rank::Four, Suit::Clubs));
        table.remaining.insert(2, card(Rank::Ten, Suit::Hearts));

        assert!(table.act(Action::Split).expect("split").is_none());
        assert!(table.can_double());
        let mid = table.act(Action::Double).expect("DAS");
        assert!(mid.is_none());
        assert!(table.hands[0].doubled);
        assert_eq!(table.active, 1);
        assert_eq!(table.stake(), 30);

        let done = table.act(Action::Stand).expect("stand hand 2");
        assert!(done.is_some());
        assert!(table.hands[0].cards.len() == 3);
    }

    #[test]
    fn dealer_does_not_draw_when_every_split_hand_busts() {
        let mut table = table_with(
            vec![
                card(Rank::Eight, Suit::Spades),
                card(Rank::Eight, Suit::Hearts),
            ],
            vec![
                card(Rank::Five, Suit::Clubs),
                card(Rank::Five, Suit::Diamonds),
            ],
        );
        table.remaining.insert(0, card(Rank::Six, Suit::Hearts));
        table.remaining.insert(1, card(Rank::Six, Suit::Clubs));
        table.remaining.insert(2, card(Rank::King, Suit::Hearts));
        table.remaining.insert(3, card(Rank::King, Suit::Clubs));

        assert!(table.act(Action::Split).expect("split").is_none());
        assert!(table.act(Action::Hit).expect("bust 1").is_none());
        let done = table.act(Action::Hit).expect("bust 2");
        let settlement = done.expect("both bust");
        assert!(settlement.hands.iter().all(|hand| hand.outcome == Outcome::PlayerBust));
        assert_eq!(table.dealer.len(), 2);
    }

    #[test]
    fn pays_three_to_two_and_rounds_odd_stakes_down() {
        assert_eq!(Outcome::PlayerBlackjack.payout(10), 25);
        assert_eq!(Outcome::PlayerBlackjack.payout(5), 12);
        assert_eq!(Outcome::PlayerWin.payout(10), 20);
        assert_eq!(Outcome::Push.payout(10), 10);
        assert_eq!(Outcome::PlayerBust.payout(10), 0);
    }

    #[test]
    fn hole_card_stays_hidden_until_settlement() {
        let table = table_with(
            vec![
                card(Rank::Ten, Suit::Spades),
                card(Rank::Nine, Suit::Hearts),
            ],
            vec![
                card(Rank::Seven, Suit::Clubs),
                card(Rank::Queen, Suit::Diamonds),
            ],
        );
        let live = publish(&table, None, true);
        assert_eq!(live.dealer.len(), 1);
        assert_eq!(live.dealer[0], table.dealer[0]);
        assert!(live.dealer_total.is_none());
        assert_eq!(live.phase, Phase::Player);
        assert_eq!(live.hands.len(), 1);
        assert!(!live.can_split);

        let done = publish(
            &table,
            Some(&Settlement::single(Outcome::PlayerWin, 10)),
            false,
        );
        assert_eq!(done.dealer.len(), 2);
        assert_eq!(done.dealer_total, Some(17));
        assert_eq!(done.phase, Phase::Settled);
    }

    #[test]
    fn a_deal_is_four_cards_from_a_full_unique_deck() {
        let mut rng = StdRng::seed_from_u64(0xB1A6);
        let table = Table::deal(&mut rng, 10);

        assert_eq!(table.hands[0].cards.len(), 2);
        assert_eq!(table.dealer.len(), 2);
        assert_eq!(table.remaining.len(), DECK_SIZE - 4);

        let mut seen = std::collections::HashSet::new();
        for card in table.hands[0]
            .cards
            .iter()
            .chain(table.dealer.iter())
            .chain(table.remaining.iter())
        {
            assert!(seen.insert(*card), "dealt a duplicate {card:?}");
        }
        assert_eq!(seen.len(), DECK_SIZE);
    }

    #[test]
    fn stored_player_round_trips_and_reads_a_legacy_array() {
        let table = table_with(
            vec![
                card(Rank::Eight, Suit::Spades),
                card(Rank::Eight, Suit::Hearts),
            ],
            vec![
                card(Rank::Ten, Suit::Clubs),
                card(Rank::Six, Suit::Diamonds),
            ],
        );
        let json = serde_json::to_string(&table.stored_player()).unwrap();
        let (hands, active) = parse_player(&json).unwrap();
        assert_eq!(hands.len(), 1);
        assert_eq!(active, 0);
        assert_eq!(hands[0].cards.len(), 2);

        let legacy = r#"[{"rank":"ace","suit":"spades"},{"rank":"king","suit":"hearts"}]"#;
        let (old, _) = parse_player(legacy).unwrap();
        assert!(is_blackjack(&old[0].cards));
    }

    #[test]
    fn the_house_keeps_a_small_edge() {
        let rtp = theoretical_rtp();
        assert!(
            (0.90..0.97).contains(&rtp),
            "return to player drifted out of band: {rtp}"
        );
    }

    #[test]
    fn standing_on_seventeen_does_not_break_the_bank() {
        let mut rng = StdRng::seed_from_u64(0x17);
        let hands = 40_000;
        let bet = 10i64;
        let mut returned = 0i64;

        for _ in 0..hands {
            let mut table = Table::deal(&mut rng, bet);
            if let Some(outcome) = table.opening() {
                returned += outcome.payout(table.stake());
                continue;
            }

            while total(&table.hands[0].cards).best < 17 {
                table.hit();
                if total(&table.hands[0].cards).bust() {
                    break;
                }
            }

            let outcome = if total(&table.hands[0].cards).bust() {
                Outcome::PlayerBust
            } else {
                table
                    .act(Action::Stand)
                    .expect("stand")
                    .expect("finished")
                    .hands[0]
                    .outcome
            };
            returned += outcome.payout(table.stake());
        }

        let sampled = returned as f64 / (hands as f64 * bet as f64);
        assert!(
            (0.85..1.05).contains(&sampled),
            "sampled RTP {sampled} drifted out of a playable band"
        );
    }
}
