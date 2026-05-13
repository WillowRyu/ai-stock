use crate::money::{Currency, Money};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// A small in-memory FX rate book. Rates are stored as "1 unit of `from`
/// equals `rate` units of `to`". Cross rates are NOT auto-derived; the
/// caller is expected to populate every (from, to) pair it needs.
///
/// Stablecoin pseudo-currencies (USDT, USDC, BUSD, DAI) are canonicalized
/// to USD before lookup, so `convert(BTC/USDT money, USD)` works without
/// an explicit USDT→USD rate.
#[derive(Debug, Clone, Default)]
pub struct FxRates {
    rates: HashMap<(Currency, Currency), Decimal>,
}

impl FxRates {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, from: Currency, to: Currency, rate: Decimal) {
        self.rates.insert((from, to), rate);
    }

    /// Convert `money` into `target` currency. Returns `None` if no rate
    /// is known and the source/target aren't equivalent canonical currencies.
    pub fn convert(&self, money: Money, target: Currency) -> Option<Money> {
        let from = canonicalize(money.currency());
        let to = canonicalize(target);
        if from == to {
            // Same canonical currency (e.g. USDT-denominated money asked for USD).
            return Some(Money::new(money.amount(), target));
        }
        let rate = self.rates.get(&(from, to))?;
        Some(Money::new(money.amount() * rate, target))
    }
}

fn canonicalize(c: Currency) -> Currency {
    match c.as_str() {
        "USDT" | "USDC" | "BUSD" | "DAI" => Currency::new("USD").unwrap(),
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn ccy(s: &str) -> Currency { Currency::new(s).unwrap() }

    #[test]
    fn same_currency_passes_through() {
        let fx = FxRates::new();
        let m = Money::new(dec!(100), ccy("USD"));
        assert_eq!(fx.convert(m, ccy("USD")), Some(Money::new(dec!(100), ccy("USD"))));
    }

    #[test]
    fn stablecoin_converts_to_usd_at_par() {
        let fx = FxRates::new();
        let m = Money::new(dec!(67000), ccy("USDT"));
        assert_eq!(fx.convert(m, ccy("USD")), Some(Money::new(dec!(67000), ccy("USD"))));
    }

    #[test]
    fn returns_none_when_rate_unknown() {
        let fx = FxRates::new();
        let m = Money::new(dec!(1), ccy("KRW"));
        assert_eq!(fx.convert(m, ccy("USD")), None);
    }

    #[test]
    fn applies_set_rate() {
        let mut fx = FxRates::new();
        fx.set(ccy("USD"), ccy("KRW"), dec!(1400));
        let m = Money::new(dec!(10), ccy("USD"));
        assert_eq!(fx.convert(m, ccy("KRW")), Some(Money::new(dec!(14000), ccy("KRW"))));
    }
}
