use crate::ports::asset_provider::{AssetProvider, ProviderError};
use crate::ports::repos::{RepoError, WatchlistRepo};
use domain::{quote::Quote, symbol::Symbol, watchlist::Watchlist};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Error)]
pub enum MarketError {
    #[error("provider: {0}")]
    Provider(#[from] ProviderError),
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("no provider supports symbol: {0}")]
    NoProvider(String),
}

pub struct MarketService {
    watchlist_repo: Arc<dyn WatchlistRepo>,
    providers: Vec<Arc<dyn AssetProvider>>,
    last_quotes: Arc<RwLock<HashMap<Symbol, Quote>>>,
}

impl MarketService {
    pub fn new(
        watchlist_repo: Arc<dyn WatchlistRepo>,
        providers: Vec<Arc<dyn AssetProvider>>,
    ) -> Self {
        Self { watchlist_repo, providers, last_quotes: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn load_watchlist(&self) -> Result<Watchlist, MarketError> {
        Ok(self.watchlist_repo.load().await?)
    }

    pub async fn add_to_watchlist(&self, symbol: Symbol) -> Result<(), MarketError> {
        let mut wl = self.watchlist_repo.load().await?;
        wl.add(symbol);
        self.watchlist_repo.save(&wl).await?;
        Ok(())
    }

    pub async fn remove_from_watchlist(&self, symbol: &Symbol) -> Result<(), MarketError> {
        let mut wl = self.watchlist_repo.load().await?;
        wl.remove(symbol);
        self.watchlist_repo.save(&wl).await?;
        Ok(())
    }

    pub async fn refresh(&self) -> Result<Vec<Quote>, MarketError> {
        let wl = self.watchlist_repo.load().await?;
        let mut all_quotes = Vec::new();

        for symbol in wl.symbols() {
            let provider = self
                .providers
                .iter()
                .find(|p| p.supports(symbol))
                .ok_or_else(|| MarketError::NoProvider(symbol.to_canonical_string()))?;
            let quotes = provider.fetch_quotes(std::slice::from_ref(symbol)).await?;
            for q in quotes {
                self.last_quotes.write().await.insert(q.symbol.clone(), q.clone());
                all_quotes.push(q);
            }
        }
        Ok(all_quotes)
    }

    pub async fn snapshot(&self) -> HashMap<Symbol, Quote> {
        self.last_quotes.read().await.clone()
    }

    pub async fn fetch_candles(
        &self,
        symbol: &Symbol,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<domain::candle::Candle>, MarketError> {
        let provider = self
            .providers
            .iter()
            .find(|p| p.supports(symbol))
            .ok_or_else(|| MarketError::NoProvider(symbol.to_canonical_string()))?;
        Ok(provider.fetch_candles(symbol, from, to).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::asset_provider::MockAssetProvider;
    use crate::ports::repos::MockWatchlistRepo;
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol,
    };
    use rust_decimal_macros::dec;

    fn s_btc() -> Symbol { Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap() }
    fn s_aapl() -> Symbol { Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap() }

    #[tokio::test]
    async fn refresh_routes_each_symbol_to_supporting_provider() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(s_btc());
        wl.add(s_aapl());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut crypto = MockAssetProvider::new();
        crypto.expect_name().return_const("crypto-mock");
        crypto.expect_supports().returning(|s| s.kind() == AssetKind::Crypto);
        crypto.expect_fetch_quotes().returning(|symbols| {
            Ok(symbols.iter().map(|s| {
                Quote::new(s.clone(), Price::new(Money::new(dec!(67000), Currency::new("USD").unwrap())), Utc::now())
            }).collect())
        });

        let mut stock = MockAssetProvider::new();
        stock.expect_name().return_const("stock-mock");
        stock.expect_supports().returning(|s| s.kind() == AssetKind::UsEquity);
        stock.expect_fetch_quotes().returning(|symbols| {
            Ok(symbols.iter().map(|s| {
                Quote::new(s.clone(), Price::new(Money::new(dec!(182), Currency::new("USD").unwrap())), Utc::now())
            }).collect())
        });

        let svc = MarketService::new(Arc::new(wl_repo), vec![Arc::new(crypto), Arc::new(stock)]);
        let quotes = svc.refresh().await.unwrap();
        assert_eq!(quotes.len(), 2);

        let snap = svc.snapshot().await;
        assert!(snap.contains_key(&s_btc()));
        assert!(snap.contains_key(&s_aapl()));
    }

    #[tokio::test]
    async fn refresh_errors_when_no_provider_supports_symbol() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(Symbol::new(AssetKind::Forex, "EURUSD", None).unwrap());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut crypto = MockAssetProvider::new();
        crypto.expect_supports().return_const(false);

        let svc = MarketService::new(Arc::new(wl_repo), vec![Arc::new(crypto)]);
        assert!(matches!(svc.refresh().await, Err(MarketError::NoProvider(_))));
    }
}
