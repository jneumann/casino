//! The house bank: a short list of markers and a flat interest rate.
//!
//! Interest is charged when the loan is taken, not as it ages. The player
//! receives the principal immediately and owes principal plus interest in one
//! lump. That keeps repayment a single guarded debit, the same way a spin
//! settles, and lets the terms be stated exactly.

use serde::Serialize;

/// Interest as basis points of the principal. 2_000 = 20%.
pub const INTEREST_BPS: i64 = 2_000;

/// Amounts the cashier will actually hand over. Anything else is refused so
/// the client cannot invent a product the house never offered.
pub const OFFERS: [i64; 4] = [250, 500, 1_000, 2_500];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Offer {
    pub principal: i64,
    pub interest: i64,
    /// Principal plus interest: what clearing the marker costs.
    pub repay: i64,
}

/// Interest charged on `principal`, or `None` when that amount is not on the
/// board. Every offered principal divides the rate evenly, which the tests pin.
pub fn quote(principal: i64) -> Option<Offer> {
    if !OFFERS.contains(&principal) {
        return None;
    }

    let interest = principal.saturating_mul(INTEREST_BPS) / 10_000;
    Some(Offer {
        principal,
        interest,
        repay: principal.saturating_add(interest),
    })
}

pub fn offers() -> Vec<Offer> {
    OFFERS
        .iter()
        .filter_map(|principal| quote(*principal))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offer_divides_the_rate_evenly() {
        for principal in OFFERS {
            let interest = principal * INTEREST_BPS;
            assert_eq!(
                interest % 10_000,
                0,
                "{principal} does not produce a whole-coin interest charge"
            );
        }
    }

    #[test]
    fn quotes_only_the_posted_amounts() {
        assert!(quote(100).is_none());
        assert!(quote(499).is_none());
        assert!(quote(0).is_none());
        assert!(quote(-500).is_none());

        let five = quote(500).expect("500 is on the board");
        assert_eq!(five.principal, 500);
        assert_eq!(five.interest, 100);
        assert_eq!(five.repay, 600);
    }

    #[test]
    fn twenty_percent_on_every_offer() {
        for offer in offers() {
            assert_eq!(offer.interest, offer.principal / 5);
            assert_eq!(offer.repay, offer.principal + offer.interest);
        }
    }

    #[test]
    fn the_board_is_the_full_offer_list() {
        let principals: Vec<i64> = offers().iter().map(|offer| offer.principal).collect();
        assert_eq!(principals, OFFERS);
    }
}
