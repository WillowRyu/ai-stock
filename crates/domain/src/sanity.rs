use crate::quote::Quote;
use rust_decimal::Decimal;

/// Pure outlier check. Returns true if the new quote should be accepted.
/// Reject if price moved more than `jump_threshold` ratio (e.g. 10 = 1000%)
/// vs. the previous accepted quote. Accept the first quote unconditionally.
pub fn is_sane(previous: Option<&Quote>, candidate: &Quote, jump_threshold: Decimal) -> bool {
    let Some(prev) = previous else { return true; };
    let p = prev.price.money().amount();
    let c = candidate.price.money().amount();
    if p == Decimal::ZERO { return true; }
    let ratio = (c - p).abs() / p;
    ratio < jump_threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn q(amount: rust_decimal::Decimal) -> Quote {
        Quote::new(
            Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap(),
            Price::new(Money::new(amount, Currency::new("USD").unwrap())),
            Utc::now(),
        )
    }

    #[test]
    fn accepts_first_quote() {
        assert!(is_sane(None, &q(dec!(100)), dec!(10)));
    }

    #[test]
    fn rejects_10x_jump() {
        let prev = q(dec!(100));
        let new = q(dec!(1100));
        assert!(!is_sane(Some(&prev), &new, dec!(10)));
    }

    #[test]
    fn accepts_5pct_move() {
        let prev = q(dec!(100));
        let new = q(dec!(105));
        assert!(is_sane(Some(&prev), &new, dec!(10)));
    }
}
