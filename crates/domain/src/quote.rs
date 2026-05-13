use crate::{price::Price, symbol::Symbol};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Quote {
    pub symbol: Symbol,
    pub price: Price,
    pub change_24h: Option<rust_decimal::Decimal>, // ratio, e.g. 0.0124 = +1.24%
    pub volume_24h: Option<rust_decimal::Decimal>,
    pub observed_at: DateTime<Utc>,
}

impl Quote {
    pub fn new(symbol: Symbol, price: Price, observed_at: DateTime<Utc>) -> Self {
        Self { symbol, price, change_24h: None, volume_24h: None, observed_at }
    }
}
