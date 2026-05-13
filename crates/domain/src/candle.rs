use crate::{price::Price, symbol::Symbol};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Candle {
    pub symbol: Symbol,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Decimal,
    pub opened_at: DateTime<Utc>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CandleError {
    #[error("high {high} must be >= max(open, close, low)")]
    HighInvariantBroken { high: Decimal },
    #[error("low {low} must be <= min(open, close, high)")]
    LowInvariantBroken { low: Decimal },
}

impl Candle {
    pub fn validate(&self) -> Result<(), CandleError> {
        let o = self.open.money().amount();
        let h = self.high.money().amount();
        let l = self.low.money().amount();
        let c = self.close.money().amount();
        if h < o || h < c || h < l { return Err(CandleError::HighInvariantBroken { high: h }); }
        if l > o || l > c || l > h { return Err(CandleError::LowInvariantBroken { low: l }); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::{Currency, Money}};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn p(v: rust_decimal::Decimal) -> Price {
        Price::new(Money::new(v, Currency::new("USD").unwrap()))
    }

    #[test]
    fn rejects_high_below_close() {
        let c = Candle {
            symbol: Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            open: p(dec!(100)), high: p(dec!(101)), low: p(dec!(99)), close: p(dec!(102)),
            volume: dec!(0), opened_at: Utc::now(),
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn accepts_valid_candle() {
        let c = Candle {
            symbol: Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            open: p(dec!(100)), high: p(dec!(105)), low: p(dec!(99)), close: p(dec!(102)),
            volume: dec!(1000), opened_at: Utc::now(),
        };
        assert!(c.validate().is_ok());
    }
}
