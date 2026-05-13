use crate::asset::AssetKind;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Symbol {
    kind: AssetKind,
    ticker: String,
    quote_currency: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SymbolError {
    #[error("ticker must be 1-20 ASCII alphanumeric/dot characters: {0}")]
    InvalidTicker(String),
    #[error("quote currency must be 3-5 uppercase ASCII: {0}")]
    InvalidQuoteCurrency(String),
}

impl Symbol {
    pub fn new(
        kind: AssetKind,
        ticker: &str,
        quote_currency: Option<&str>,
    ) -> Result<Self, SymbolError> {
        if ticker.is_empty()
            || ticker.len() > 20
            || !ticker
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.')
        {
            return Err(SymbolError::InvalidTicker(ticker.into()));
        }
        if let Some(qc) = quote_currency {
            if !(3..=5).contains(&qc.len()) || !qc.chars().all(|c| c.is_ascii_uppercase()) {
                return Err(SymbolError::InvalidQuoteCurrency(qc.into()));
            }
        }
        Ok(Self {
            kind,
            ticker: ticker.into(),
            quote_currency: quote_currency.map(|s| s.into()),
        })
    }
    pub fn kind(&self) -> AssetKind {
        self.kind
    }
    pub fn ticker(&self) -> &str {
        &self.ticker
    }
    pub fn quote_currency(&self) -> Option<&str> {
        self.quote_currency.as_deref()
    }

    /// Canonical string form: `kind:ticker[:quote]`
    pub fn to_canonical_string(&self) -> String {
        let prefix = match self.kind {
            AssetKind::Crypto => "crypto",
            AssetKind::UsEquity => "us",
            AssetKind::KrEquity => "kr",
            AssetKind::Forex => "fx",
            AssetKind::Commodity => "com",
        };
        match &self.quote_currency {
            Some(q) => format!("{}:{}:{}", prefix, self.ticker, q),
            None => format!("{}:{}", prefix, self.ticker),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_btc_usd() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        assert_eq!(s.ticker(), "BTC");
        assert_eq!(s.quote_currency(), Some("USD"));
        assert_eq!(s.to_canonical_string(), "crypto:BTC:USD");
    }

    #[test]
    fn creates_us_equity_without_quote() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        assert_eq!(s.to_canonical_string(), "us:AAPL");
    }

    #[test]
    fn rejects_empty_ticker() {
        assert!(Symbol::new(AssetKind::Crypto, "", Some("USD")).is_err());
    }

    #[test]
    fn rejects_lowercase_quote() {
        assert!(Symbol::new(AssetKind::Crypto, "BTC", Some("usd")).is_err());
    }

    #[test]
    fn allows_dot_in_ticker_for_kr_equity() {
        let s = Symbol::new(AssetKind::KrEquity, "005930.KS", None).unwrap();
        assert_eq!(s.ticker(), "005930.KS");
    }

    #[test]
    fn accepts_four_char_usdt_quote() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USDT")).unwrap();
        assert_eq!(s.quote_currency(), Some("USDT"));
        assert_eq!(s.to_canonical_string(), "crypto:BTC:USDT");
    }
}
