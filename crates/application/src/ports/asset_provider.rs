use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{candle::Candle, quote::Quote, symbol::Symbol};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("symbol not supported by this provider: {0}")]
    UnsupportedSymbol(String),
    #[error("rate limited; retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("network error: {0}")]
    Network(String),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AssetProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, symbol: &Symbol) -> bool;
    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError>;
    async fn fetch_candles(
        &self,
        symbol: &Symbol,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, ProviderError>;
}
