//! A standard 52-card French deck, shared by the table games.
//!
//! Serialised as `{ "rank": "ace", "suit": "spades" }` so every client draws
//! the same card the same way.

use serde::{Deserialize, Serialize};

pub const DECK_SIZE: usize = 52;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Self::Clubs, Self::Diamonds, Self::Hearts, Self::Spades];
}

/// Ace-high ordering. Ace-low straights and blackjack pip values are special
/// cases in the games that need them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Rank {
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
    Ace = 14,
}

impl Rank {
    pub const ALL: [Rank; 13] = [
        Self::Two,
        Self::Three,
        Self::Four,
        Self::Five,
        Self::Six,
        Self::Seven,
        Self::Eight,
        Self::Nine,
        Self::Ten,
        Self::Jack,
        Self::Queen,
        Self::King,
        Self::Ace,
    ];

    pub fn value(self) -> u8 {
        self as u8
    }

    /// Jack, Queen, King, or Ace — the pairs that pay in Jacks or Better.
    pub fn is_high(self) -> bool {
        self >= Self::Jack
    }

    /// Blackjack pip value: ace is 1 (soft 11 is applied later), faces are 10.
    pub fn pip(self) -> u8 {
        match self {
            Self::Ace => 1,
            Self::Ten | Self::Jack | Self::Queen | Self::King => 10,
            other => other.value(),
        }
    }

    pub fn is_ten(self) -> bool {
        self.pip() == 10
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

/// A 52-card deck in rank-within-suit order, unshuffled.
pub fn standard_deck() -> [Card; DECK_SIZE] {
    let mut deck = [Card {
        rank: Rank::Two,
        suit: Suit::Clubs,
    }; DECK_SIZE];
    let mut index = 0;

    for suit in Suit::ALL {
        for rank in Rank::ALL {
            deck[index] = Card { rank, suit };
            index += 1;
        }
    }

    deck
}
