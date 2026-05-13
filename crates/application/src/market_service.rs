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

#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolError {
    pub symbol_canonical: String,
    pub provider: String, // "" if no provider supported
    pub error: String,
}

#[derive(Debug, Default)]
pub struct RefreshOutcome {
    pub quotes: Vec<Quote>,
    pub errors: Vec<SymbolError>,
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

    /// Refresh quotes for every watchlist symbol. Per-symbol failures do not
    /// fail the whole refresh; if a supporting provider returns an error or
    /// empty result, the next supporting provider is tried in order. Failures
    /// after exhausting all supporting providers (and "no provider supports
    /// this symbol" cases) are returned as `SymbolError`s on the outcome so
    /// callers (e.g. the app's event emit loop) can surface them to the UI.
    pub async fn refresh(&self) -> Result<RefreshOutcome, MarketError> {
        let wl = self.watchlist_repo.load().await?;
        let mut outcome = RefreshOutcome::default();

        for symbol in wl.symbols() {
            let supporting: Vec<&Arc<dyn AssetProvider>> = self
                .providers
                .iter()
                .filter(|p| p.supports(symbol))
                .collect();
            if supporting.is_empty() {
                tracing::warn!(
                    symbol = %symbol.to_canonical_string(),
                    "no provider supports symbol; skipping",
                );
                outcome.errors.push(SymbolError {
                    symbol_canonical: symbol.to_canonical_string(),
                    provider: String::new(),
                    error: "no provider supports this symbol".into(),
                });
                continue;
            }
            let mut got = false;
            let mut last_err: Option<SymbolError> = None;
            for provider in &supporting {
                match provider.fetch_quotes(std::slice::from_ref(symbol)).await {
                    Ok(qs) if !qs.is_empty() => {
                        for q in qs {
                            self.last_quotes.write().await.insert(q.symbol.clone(), q.clone());
                            outcome.quotes.push(q);
                        }
                        got = true;
                        break;
                    }
                    Ok(_) => {
                        tracing::debug!(
                            provider = provider.name(),
                            symbol = %symbol.to_canonical_string(),
                            "provider returned empty result; trying next",
                        );
                        last_err = Some(SymbolError {
                            symbol_canonical: symbol.to_canonical_string(),
                            provider: provider.name().into(),
                            error: "empty result".into(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            provider = provider.name(),
                            symbol = %symbol.to_canonical_string(),
                            error = ?e,
                            "provider failed; trying next",
                        );
                        last_err = Some(SymbolError {
                            symbol_canonical: symbol.to_canonical_string(),
                            provider: provider.name().into(),
                            error: e.to_string(),
                        });
                    }
                }
            }
            if !got {
                tracing::warn!(
                    symbol = %symbol.to_canonical_string(),
                    "all providers failed for symbol",
                );
                if let Some(err) = last_err {
                    outcome.errors.push(err);
                }
            }
        }
        Ok(outcome)
    }

    pub async fn snapshot(&self) -> HashMap<Symbol, Quote> {
        self.last_quotes.read().await.clone()
    }

    /// Try each supporting provider in order until one returns candles. Some
    /// providers (Finnhub free, CoinGecko free) don't expose candle data even
    /// though they support quote lookups for the same symbol, so a single
    /// successful provider is enough.
    pub async fn fetch_candles(
        &self,
        symbol: &Symbol,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<domain::candle::Candle>, MarketError> {
        let supporting: Vec<&Arc<dyn AssetProvider>> = self
            .providers
            .iter()
            .filter(|p| p.supports(symbol))
            .collect();
        if supporting.is_empty() {
            return Err(MarketError::NoProvider(symbol.to_canonical_string()));
        }
        let mut last_err: Option<ProviderError> = None;
        for provider in supporting {
            match provider.fetch_candles(symbol, from, to).await {
                Ok(candles) if !candles.is_empty() => return Ok(candles),
                Ok(_) => {
                    tracing::debug!(
                        provider = provider.name(),
                        symbol = %symbol.to_canonical_string(),
                        "provider returned empty candles; trying next",
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        provider = provider.name(),
                        symbol = %symbol.to_canonical_string(),
                        error = ?e,
                        "provider candle fetch failed; trying next",
                    );
                    last_err = Some(e);
                }
            }
        }
        Err(MarketError::Provider(
            last_err.unwrap_or_else(|| ProviderError::Upstream("no provider returned candles".into())),
        ))
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
        let outcome = svc.refresh().await.unwrap();
        assert_eq!(outcome.quotes.len(), 2);
        assert!(outcome.errors.is_empty());

        let snap = svc.snapshot().await;
        assert!(snap.contains_key(&s_btc()));
        assert!(snap.contains_key(&s_aapl()));
    }

    #[tokio::test]
    async fn refresh_skips_symbol_with_no_supporting_provider() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(Symbol::new(AssetKind::Forex, "EURUSD", None).unwrap());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut crypto = MockAssetProvider::new();
        crypto.expect_supports().return_const(false);

        let svc = MarketService::new(Arc::new(wl_repo), vec![Arc::new(crypto)]);
        let outcome = svc.refresh().await.unwrap();
        assert!(outcome.quotes.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert!(outcome.errors[0].provider.is_empty());
    }

    #[tokio::test]
    async fn refresh_falls_back_to_next_provider_on_failure() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(s_aapl());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut failing = MockAssetProvider::new();
        failing.expect_name().return_const("failing");
        failing.expect_supports().returning(|s| s.kind() == AssetKind::UsEquity);
        failing.expect_fetch_quotes().returning(|_| Err(ProviderError::Upstream("blocked".into())));

        let mut ok = MockAssetProvider::new();
        ok.expect_name().return_const("ok");
        ok.expect_supports().returning(|s| s.kind() == AssetKind::UsEquity);
        ok.expect_fetch_quotes().returning(|symbols| {
            Ok(symbols.iter().map(|s|
                Quote::new(s.clone(), Price::new(Money::new(dec!(295), Currency::new("USD").unwrap())), Utc::now())
            ).collect())
        });

        let svc = MarketService::new(
            Arc::new(wl_repo),
            vec![Arc::new(failing), Arc::new(ok)],
        );
        let outcome = svc.refresh().await.unwrap();
        assert_eq!(outcome.quotes.len(), 1);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.quotes[0].price.money().amount(), dec!(295));
    }
}
