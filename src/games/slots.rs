//! A three-reel slot machine.
//!
//! All three reels share one weighted strip, so each position is an
//! independent draw. That keeps the maths simple enough to state the return to
//! player exactly rather than estimate it — see [`theoretical_rtp`] and the
//! tests below, which pin the house edge so an accidental paytable edit cannot
//! quietly make the game unprofitable (or unwinnable).

use rand::Rng;
use serde::Serialize;

pub const REELS: usize = 3;

/// The lowest and highest a player may stake on one spin.
pub const MIN_BET: i64 = 1;
pub const MAX_BET: i64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Symbol {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
    Star,
    Seven,
}

/// One entry per symbol: how often it appears on a reel, and what it pays for
/// three of a kind or exactly two of a kind, as a multiple of the stake.
struct Payline {
    symbol: Symbol,
    weight: u32,
    three: i64,
    pair: i64,
}

/// Weights sum to 100, so a weight reads directly as a percentage chance of
/// landing on any one reel.
#[rustfmt::skip]
const PAYLINES: [Payline; 6] = [
    //       symbol,            weight, three, pair
    Payline { symbol: Symbol::Clubs,    weight: 30, three:   4, pair: 1 },
    Payline { symbol: Symbol::Diamonds, weight: 25, three:   6, pair: 1 },
    Payline { symbol: Symbol::Hearts,   weight: 18, three:  12, pair: 1 },
    Payline { symbol: Symbol::Spades,   weight: 12, three:  25, pair: 2 },
    Payline { symbol: Symbol::Star,     weight:  9, three:  60, pair: 2 },
    Payline { symbol: Symbol::Seven,    weight:  6, three: 150, pair: 3 },
];

const TOTAL_WEIGHT: u32 = 100;

/// What a spin paid, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Outcome {
    /// Nothing matched.
    Nothing,
    /// Exactly two reels matched.
    Pair { symbol: Symbol, multiplier: i64 },
    /// All three reels matched.
    Three { symbol: Symbol, multiplier: i64 },
}

impl Outcome {
    pub fn multiplier(&self) -> i64 {
        match self {
            Self::Nothing => 0,
            Self::Pair { multiplier, .. } | Self::Three { multiplier, .. } => *multiplier,
        }
    }
}

/// Draws three independent symbols from the weighted strip.
pub fn spin<R: Rng + ?Sized>(rng: &mut R) -> [Symbol; REELS] {
    [draw(rng), draw(rng), draw(rng)]
}

fn draw<R: Rng + ?Sized>(rng: &mut R) -> Symbol {
    let mut roll = rng.gen_range(0..TOTAL_WEIGHT);

    for line in &PAYLINES {
        if roll < line.weight {
            return line.symbol;
        }
        roll -= line.weight;
    }

    // Unreachable while the weights sum to TOTAL_WEIGHT, which the tests check.
    PAYLINES[0].symbol
}

/// Classifies a set of reels. Three of a kind beats a pair; a pair only counts
/// when exactly two reels match.
pub fn evaluate(reels: [Symbol; REELS]) -> Outcome {
    let [a, b, c] = reels;

    if a == b && b == c {
        return Outcome::Three {
            symbol: a,
            multiplier: payline(a).three,
        };
    }

    let paired = if a == b || a == c {
        Some(a)
    } else if b == c {
        Some(b)
    } else {
        None
    };

    match paired {
        Some(symbol) => Outcome::Pair {
            symbol,
            multiplier: payline(symbol).pair,
        },
        None => Outcome::Nothing,
    }
}

/// What a stake of `bet` returns for this outcome. This is the gross return,
/// not the profit: a multiplier of 1 hands the stake back.
pub fn payout(outcome: Outcome, bet: i64) -> i64 {
    bet.saturating_mul(outcome.multiplier())
}

fn payline(symbol: Symbol) -> &'static Payline {
    PAYLINES
        .iter()
        .find(|line| line.symbol == symbol)
        .expect("every symbol has a payline")
}

/// The paytable as the client should display it, highest paying first.
#[derive(Debug, Serialize)]
pub struct PaytableEntry {
    pub symbol: Symbol,
    pub three_of_a_kind: i64,
    pub pair: i64,
}

pub fn paytable() -> Vec<PaytableEntry> {
    let mut entries: Vec<PaytableEntry> = PAYLINES
        .iter()
        .map(|line| PaytableEntry {
            symbol: line.symbol,
            three_of_a_kind: line.three,
            pair: line.pair,
        })
        .collect();

    entries.sort_by_key(|entry| std::cmp::Reverse(entry.three_of_a_kind));
    entries
}

/// Exact return to player, by enumerating all 216 reel combinations weighted
/// by their probability. A value below 1.0 is the house edge.
pub fn theoretical_rtp() -> f64 {
    let total = f64::from(TOTAL_WEIGHT);
    let mut rtp = 0.0;

    for a in &PAYLINES {
        for b in &PAYLINES {
            for c in &PAYLINES {
                let probability = (f64::from(a.weight) / total)
                    * (f64::from(b.weight) / total)
                    * (f64::from(c.weight) / total);
                let outcome = evaluate([a.symbol, b.symbol, c.symbol]);

                rtp += probability * outcome.multiplier() as f64;
            }
        }
    }

    rtp
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn weights_sum_to_the_declared_total() {
        let sum: u32 = PAYLINES.iter().map(|line| line.weight).sum();

        assert_eq!(
            sum, TOTAL_WEIGHT,
            "draw() relies on the weights summing exactly"
        );
    }

    #[test]
    fn the_house_keeps_a_small_edge() {
        let rtp = theoretical_rtp();

        // Generous enough to be worth playing, still profitable for the house.
        assert!(
            (0.90..0.97).contains(&rtp),
            "return to player drifted out of band: {rtp}"
        );
    }

    #[test]
    fn rarer_symbols_pay_more() {
        for pair in PAYLINES.windows(2) {
            assert!(
                pair[0].weight > pair[1].weight,
                "paylines must be ordered from most to least common"
            );
            assert!(
                pair[0].three < pair[1].three,
                "a rarer symbol must pay more for three of a kind"
            );
        }
    }

    #[test]
    fn classifies_reels() {
        use Symbol::*;

        assert_eq!(
            evaluate([Seven, Seven, Seven]),
            Outcome::Three {
                symbol: Seven,
                multiplier: 150
            }
        );
        assert_eq!(
            evaluate([Clubs, Clubs, Seven]),
            Outcome::Pair {
                symbol: Clubs,
                multiplier: 1
            }
        );
        // A pair split by a different symbol still counts.
        assert_eq!(
            evaluate([Star, Hearts, Star]),
            Outcome::Pair {
                symbol: Star,
                multiplier: 2
            }
        );
        assert_eq!(evaluate([Clubs, Hearts, Seven]), Outcome::Nothing);
    }

    #[test]
    fn pays_a_multiple_of_the_stake() {
        let outcome = Outcome::Three {
            symbol: Symbol::Seven,
            multiplier: 150,
        };

        assert_eq!(payout(outcome, 10), 1_500);
        assert_eq!(payout(Outcome::Nothing, 10), 0);
    }

    #[test]
    fn sampling_matches_the_theoretical_rtp() {
        let mut rng = StdRng::seed_from_u64(0x000C_A510);
        let spins = 400_000;
        let bet = 1;
        let mut returned = 0i64;

        for _ in 0..spins {
            returned += payout(evaluate(spin(&mut rng)), bet);
        }

        let sampled = returned as f64 / f64::from(spins);
        let expected = theoretical_rtp();

        // Seeded, so this is deterministic; the tolerance covers sampling noise
        // at this many spins, not flakiness.
        assert!(
            (sampled - expected).abs() < 0.02,
            "sampled RTP {sampled} strayed from the expected {expected}"
        );
    }

    #[test]
    fn only_ever_draws_known_symbols() {
        let mut rng = StdRng::seed_from_u64(7);

        for _ in 0..10_000 {
            for symbol in spin(&mut rng) {
                assert!(PAYLINES.iter().any(|line| line.symbol == symbol));
            }
        }
    }
}
