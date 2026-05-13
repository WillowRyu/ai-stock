use domain::fx::FxRates;
use domain::money::{Currency, Money};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe wrapper around `FxRates`. The app owns one of these in `AppState`
/// and refreshes it from a background task; `PortfolioService` reads a snapshot
/// on each valuation call.
#[derive(Clone, Default)]
pub struct FxRateBook {
    inner: Arc<RwLock<FxRates>>,
}

impl FxRateBook {
    pub fn new() -> Self { Self::default() }

    pub async fn snapshot(&self) -> FxRates {
        self.inner.read().await.clone()
    }

    pub async fn set(&self, from: Currency, to: Currency, rate: rust_decimal::Decimal) {
        self.inner.write().await.set(from, to, rate);
    }

    /// Convenience helper using the current snapshot (for callers outside services).
    pub async fn convert(&self, money: Money, target: Currency) -> Option<Money> {
        self.inner.read().await.convert(money, target)
    }
}
