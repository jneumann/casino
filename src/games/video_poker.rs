//! Jacks or Better video poker.
//!
//! Five cards from a shuffled 52-card deck, then a single draw of anything
//! the player does not hold. The paytable is **6/5 Jacks or Better**: a full
//! house pays 6x and a flush pays 5x, which is the published ~95% return with
//! optimal hold strategy — close to the slot machine's house edge, so neither
//! game is the obvious place to stand.
//!
//! The draw is where the skill lives, so the return cannot be enumerated the
//! way the three-reel machine can. [`theoretical_rtp`] is the published figure
//! for this paytable; the tests instead pin the evaluator against the known
//! 5-card poker hand counts so a ranking bug cannot quietly change what the
//! machine pays.

use rand::Rng;
use rand::seq::SliceRandom;
use serde::Serialize;

pub use super::cards::{Card, DECK_SIZE, Rank, Suit, standard_deck};

pub const HAND_SIZE: usize = 5;

/// The lowest and highest a player may stake on one hand.
pub const MIN_BET: i64 = 1;
pub const MAX_BET: i64 = 500;

pub type Hand = [Card; HAND_SIZE];

/// What the five cards made, and what that pays as a multiple of the stake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Outcome {
    pub kind: HandKind,
    pub multiplier: i64,
}

impl Outcome {
    fn of(kind: HandKind) -> Self {
        Self {
            kind,
            multiplier: kind.multiplier(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandKind {
    Nothing,
    JacksOrBetter,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
    RoyalFlush,
}

impl HandKind {
    /// 6/5 Jacks or Better. The royal is the five-coin rate (800x) at every
    /// stake, so the published return does not depend on how much is bet.
    pub fn multiplier(self) -> i64 {
        match self {
            Self::Nothing => 0,
            Self::JacksOrBetter => 1,
            Self::TwoPair => 2,
            Self::ThreeOfAKind => 3,
            Self::Straight => 4,
            Self::Flush => 5,
            Self::FullHouse => 6,
            Self::FourOfAKind => 25,
            Self::StraightFlush => 50,
            Self::RoyalFlush => 800,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Nothing => "Nothing",
            Self::JacksOrBetter => "Jacks or better",
            Self::TwoPair => "Two pair",
            Self::ThreeOfAKind => "Three of a kind",
            Self::Straight => "Straight",
            Self::Flush => "Flush",
            Self::FullHouse => "Full house",
            Self::FourOfAKind => "Four of a kind",
            Self::StraightFlush => "Straight flush",
            Self::RoyalFlush => "Royal flush",
        }
    }

    const PAYTABLE: [HandKind; 9] = [
        Self::RoyalFlush,
        Self::StraightFlush,
        Self::FourOfAKind,
        Self::FullHouse,
        Self::Flush,
        Self::Straight,
        Self::ThreeOfAKind,
        Self::TwoPair,
        Self::JacksOrBetter,
    ];
}

/// Shuffles a fresh deck and deals five cards. The remaining 47 stay on the
/// server: they are the stock the draw is taken from, and they are never sent
/// to the client.
pub fn deal<R: Rng + ?Sized>(rng: &mut R) -> (Hand, Vec<Card>) {
    let mut deck = standard_deck();
    deck.shuffle(rng);

    let hand: Hand = deck[..HAND_SIZE]
        .try_into()
        .expect("the deck is longer than a hand");
    let remaining = deck[HAND_SIZE..].to_vec();

    (hand, remaining)
}

/// Replaces every card that is not held, in left-to-right order, from the
/// front of the remaining stock.
pub fn draw(hand: Hand, held: [bool; HAND_SIZE], remaining: &[Card]) -> Hand {
    let mut stock = remaining.iter().copied();
    let mut next = hand;

    for (index, keep) in held.iter().enumerate() {
        if !*keep {
            next[index] = stock
                .next()
                .expect("a 47-card stock covers discarding all five");
        }
    }

    next
}

/// Classifies a five-card hand under Jacks or Better ranking.
pub fn evaluate(hand: Hand) -> Outcome {
    let flush = hand.iter().all(|card| card.suit == hand[0].suit);
    let straight = is_straight(hand);
    let royal = flush && is_royal(hand);

    let mut freq = [0u8; 15];
    for card in hand {
        freq[card.rank.value() as usize] += 1;
    }

    let mut pairs = 0;
    let mut high_pair = false;
    let mut trips = false;
    let mut quads = false;

    for rank in Rank::ALL {
        match freq[rank.value() as usize] {
            2 => {
                pairs += 1;
                if rank.is_high() {
                    high_pair = true;
                }
            }
            3 => trips = true,
            4 => quads = true,
            _ => {}
        }
    }

    let kind = if royal {
        HandKind::RoyalFlush
    } else if flush && straight {
        HandKind::StraightFlush
    } else if quads {
        HandKind::FourOfAKind
    } else if trips && pairs == 1 {
        HandKind::FullHouse
    } else if flush {
        HandKind::Flush
    } else if straight {
        HandKind::Straight
    } else if trips {
        HandKind::ThreeOfAKind
    } else if pairs == 2 {
        HandKind::TwoPair
    } else if pairs == 1 && high_pair {
        HandKind::JacksOrBetter
    } else {
        HandKind::Nothing
    };

    Outcome::of(kind)
}

/// What a stake of `bet` returns for this outcome. Gross return, not profit:
/// Jacks or better hands the stake back.
pub fn payout(outcome: Outcome, bet: i64) -> i64 {
    bet.saturating_mul(outcome.multiplier)
}

fn is_straight(hand: Hand) -> bool {
    let mut values = [0u8; HAND_SIZE];
    for (index, card) in hand.iter().enumerate() {
        values[index] = card.rank.value();
    }
    values.sort_unstable();

    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return false;
    }

    if values[HAND_SIZE - 1] - values[0] == 4 {
        return true;
    }

    // The wheel: A-2-3-4-5. Sorted that is 2, 3, 4, 5, 14.
    values == [2, 3, 4, 5, 14]
}

fn is_royal(hand: Hand) -> bool {
    let mut values = [0u8; HAND_SIZE];
    for (index, card) in hand.iter().enumerate() {
        values[index] = card.rank.value();
    }
    values.sort_unstable();
    values == [10, 11, 12, 13, 14]
}

/// The paytable as the client should display it, highest paying first.
#[derive(Debug, Serialize)]
pub struct PaytableEntry {
    pub kind: HandKind,
    pub name: &'static str,
    pub multiplier: i64,
}

pub fn paytable() -> Vec<PaytableEntry> {
    HandKind::PAYTABLE
        .iter()
        .copied()
        .map(|kind| PaytableEntry {
            kind,
            name: kind.label(),
            multiplier: kind.multiplier(),
        })
        .collect()
}

/// Published return to player for 6/5 Jacks or Better with optimal holds.
/// A value below 1.0 is the house edge.
pub fn theoretical_rtp() -> f64 {
    0.95
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn card(rank: Rank, suit: Suit) -> Card {
        Card { rank, suit }
    }

    fn hand(cards: [(Rank, Suit); 5]) -> Hand {
        cards.map(|(rank, suit)| card(rank, suit))
    }

    fn kind_of(cards: [(Rank, Suit); 5]) -> HandKind {
        evaluate(hand(cards)).kind
    }

    #[test]
    fn paytable_kinds_are_snake_case_strings() {
        let royal = serde_json::to_value(&paytable()[0]).expect("paytable serialises");
        assert_eq!(royal["kind"], "royal_flush");
        assert_eq!(royal["multiplier"], 800);

        let outcome = serde_json::to_value(Outcome::of(HandKind::JacksOrBetter)).expect("outcome");
        assert_eq!(outcome["kind"], "jacks_or_better");
        assert_eq!(outcome["multiplier"], 1);
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
    fn rarer_hands_pay_more() {
        let table = HandKind::PAYTABLE;
        for pair in table.windows(2) {
            assert!(
                pair[0].multiplier() > pair[1].multiplier(),
                "{} should pay more than {}",
                pair[0].label(),
                pair[1].label()
            );
        }
    }

    #[test]
    fn classifies_the_paid_hands() {
        use HandKind::*;
        use Rank::*;
        use Suit::*;

        assert_eq!(
            kind_of([
                (Ace, Spades),
                (King, Spades),
                (Queen, Spades),
                (Jack, Spades),
                (Ten, Spades)
            ]),
            RoyalFlush
        );
        assert_eq!(
            kind_of([
                (Nine, Hearts),
                (Eight, Hearts),
                (Seven, Hearts),
                (Six, Hearts),
                (Five, Hearts)
            ]),
            StraightFlush
        );
        // The steel wheel is a straight flush, not a royal.
        assert_eq!(
            kind_of([
                (Ace, Clubs),
                (Two, Clubs),
                (Three, Clubs),
                (Four, Clubs),
                (Five, Clubs)
            ]),
            StraightFlush
        );
        assert_eq!(
            kind_of([
                (Jack, Clubs),
                (Jack, Diamonds),
                (Jack, Hearts),
                (Jack, Spades),
                (Three, Clubs)
            ]),
            FourOfAKind
        );
        assert_eq!(
            kind_of([
                (King, Clubs),
                (King, Diamonds),
                (King, Hearts),
                (Nine, Clubs),
                (Nine, Spades)
            ]),
            FullHouse
        );
        assert_eq!(
            kind_of([
                (Two, Spades),
                (Five, Spades),
                (Nine, Spades),
                (Jack, Spades),
                (King, Spades)
            ]),
            Flush
        );
        assert_eq!(
            kind_of([
                (Ten, Clubs),
                (Jack, Diamonds),
                (Queen, Hearts),
                (King, Spades),
                (Ace, Clubs)
            ]),
            Straight
        );
        assert_eq!(
            kind_of([
                (Ace, Hearts),
                (Two, Clubs),
                (Three, Diamonds),
                (Four, Spades),
                (Five, Hearts)
            ]),
            Straight
        );
        assert_eq!(
            kind_of([
                (Seven, Clubs),
                (Seven, Diamonds),
                (Seven, Hearts),
                (Two, Clubs),
                (Nine, Spades)
            ]),
            ThreeOfAKind
        );
        assert_eq!(
            kind_of([
                (Jack, Clubs),
                (Jack, Diamonds),
                (Four, Hearts),
                (Four, Spades),
                (Ace, Clubs)
            ]),
            TwoPair
        );
        assert_eq!(
            kind_of([
                (Jack, Clubs),
                (Jack, Diamonds),
                (Two, Hearts),
                (Five, Spades),
                (Nine, Clubs)
            ]),
            JacksOrBetter
        );
        assert_eq!(
            kind_of([
                (Ace, Clubs),
                (Ace, Diamonds),
                (Three, Hearts),
                (Seven, Spades),
                (Nine, Clubs)
            ]),
            JacksOrBetter
        );
    }

    #[test]
    fn does_not_pay_a_low_pair_or_a_wraparound() {
        use HandKind::*;
        use Rank::*;
        use Suit::*;

        assert_eq!(
            kind_of([
                (Ten, Clubs),
                (Ten, Diamonds),
                (Two, Hearts),
                (Five, Spades),
                (Nine, Clubs)
            ]),
            Nothing
        );
        // K-A-2-3-4 is not a straight.
        assert_eq!(
            kind_of([
                (King, Clubs),
                (Ace, Diamonds),
                (Two, Hearts),
                (Three, Spades),
                (Four, Clubs)
            ]),
            Nothing
        );
        assert_eq!(
            kind_of([
                (Two, Clubs),
                (Five, Diamonds),
                (Nine, Hearts),
                (Jack, Spades),
                (King, Clubs)
            ]),
            Nothing
        );
    }

    #[test]
    fn pays_a_multiple_of_the_stake() {
        let full_house = Outcome::of(HandKind::FullHouse);

        assert_eq!(payout(full_house, 10), 60);
        assert_eq!(payout(Outcome::of(HandKind::Nothing), 10), 0);
        assert_eq!(payout(Outcome::of(HandKind::RoyalFlush), 5), 4_000);
    }

    #[test]
    fn draw_keeps_held_cards_and_replaces_the_rest() {
        use Rank::*;
        use Suit::*;

        let dealt = hand([
            (Ace, Spades),
            (Ace, Hearts),
            (Two, Clubs),
            (Five, Diamonds),
            (Nine, Clubs),
        ]);
        let remaining = vec![
            card(King, Clubs),
            card(King, Diamonds),
            card(Three, Hearts),
            card(Four, Spades),
            card(Six, Hearts),
        ];

        let drawn = draw(dealt, [true, true, false, false, false], &remaining);

        assert_eq!(drawn[0], dealt[0]);
        assert_eq!(drawn[1], dealt[1]);
        assert_eq!(drawn[2], remaining[0]);
        assert_eq!(drawn[3], remaining[1]);
        assert_eq!(drawn[4], remaining[2]);
        assert_eq!(evaluate(drawn).kind, HandKind::TwoPair);
    }

    #[test]
    fn a_deal_is_five_from_a_full_unique_deck() {
        let mut rng = StdRng::seed_from_u64(0xC15A);
        let (dealt, remaining) = deal(&mut rng);

        assert_eq!(dealt.len(), HAND_SIZE);
        assert_eq!(remaining.len(), DECK_SIZE - HAND_SIZE);

        let mut seen = std::collections::HashSet::new();
        for card in dealt.iter().chain(remaining.iter()) {
            assert!(seen.insert(*card), "dealt a duplicate {card:?}");
        }
        assert_eq!(seen.len(), DECK_SIZE);
    }

    #[test]
    fn only_ever_deals_real_cards() {
        let mut rng = StdRng::seed_from_u64(7);
        let legal: std::collections::HashSet<Card> = standard_deck().into_iter().collect();

        for _ in 0..200 {
            let (dealt, remaining) = deal(&mut rng);
            for card in dealt.iter().chain(remaining.iter()) {
                assert!(legal.contains(card));
            }
        }
    }

    /// Standard 5-card poker frequencies, with one pair split into jacks-or-
    /// better versus a low pair (which pays nothing, like high card).
    #[test]
    fn five_card_hands_match_known_poker_counts() {
        use HandKind::*;

        let deck = standard_deck();
        let mut counts = std::collections::HashMap::new();
        let mut total = 0u64;

        for a in 0..DECK_SIZE {
            for b in (a + 1)..DECK_SIZE {
                for c in (b + 1)..DECK_SIZE {
                    for d in (c + 1)..DECK_SIZE {
                        for e in (d + 1)..DECK_SIZE {
                            let kind = evaluate([deck[a], deck[b], deck[c], deck[d], deck[e]]).kind;
                            *counts.entry(kind).or_insert(0u64) += 1;
                            total += 1;
                        }
                    }
                }
            }
        }

        assert_eq!(total, 2_598_960);
        assert_eq!(counts.get(&RoyalFlush).copied().unwrap_or(0), 4);
        assert_eq!(counts.get(&StraightFlush).copied().unwrap_or(0), 36);
        assert_eq!(counts.get(&FourOfAKind).copied().unwrap_or(0), 624);
        assert_eq!(counts.get(&FullHouse).copied().unwrap_or(0), 3_744);
        assert_eq!(counts.get(&Flush).copied().unwrap_or(0), 5_108);
        assert_eq!(counts.get(&Straight).copied().unwrap_or(0), 10_200);
        assert_eq!(counts.get(&ThreeOfAKind).copied().unwrap_or(0), 54_912);
        assert_eq!(counts.get(&TwoPair).copied().unwrap_or(0), 123_552);
        // 4 of the 13 pair ranks pay (J, Q, K, A); the rest are nothing.
        assert_eq!(counts.get(&JacksOrBetter).copied().unwrap_or(0), 337_920);
        assert_eq!(counts.get(&Nothing).copied().unwrap_or(0), 2_062_860);
    }

    /// Expected value of standing pat — never drawing. A terrible strategy,
    /// but it is exact (every 5-card deal, no hold tree), so a paytable edit
    /// or a ranking bug moves this number.
    #[test]
    fn standing_pat_is_a_losing_strategy() {
        let deck = standard_deck();
        let mut returned = 0u64;
        let mut total = 0u64;

        for a in 0..DECK_SIZE {
            for b in (a + 1)..DECK_SIZE {
                for c in (b + 1)..DECK_SIZE {
                    for d in (c + 1)..DECK_SIZE {
                        for e in (d + 1)..DECK_SIZE {
                            returned += evaluate([deck[a], deck[b], deck[c], deck[d], deck[e]])
                                .multiplier as u64;
                            total += 1;
                        }
                    }
                }
            }
        }

        let ev = returned as f64 / total as f64;
        assert!(
            (0.330..0.332).contains(&ev),
            "stand-pat expected value drifted: {ev}"
        );
    }
}
