use crate::fx_rate_book::FxRateBook;
use crate::market_service::MarketService;
use crate::ports::repos::{PortfolioRepo, RepoError, SettingsRepo};
use domain::{holding::Holding, money::Currency, portfolio_calc, symbol::Symbol};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PortfolioError {
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("invalid display currency: {0}")]
    InvalidCurrency(String),
}

pub struct PortfolioService {
    repo: Arc<dyn PortfolioRepo>,
    market: Arc<MarketService>,
    settings_repo: Arc<dyn SettingsRepo>,
    fx: FxRateBook,
}

impl PortfolioService {
    pub fn new(
        repo: Arc<dyn PortfolioRepo>,
        market: Arc<MarketService>,
        settings_repo: Arc<dyn SettingsRepo>,
        fx: FxRateBook,
    ) -> Self {
        Self { repo, market, settings_repo, fx }
    }

    pub async fn upsert_holding(&self, holding: Holding) -> Result<(), PortfolioError> {
        self.repo.upsert_holding(&holding).await?;
        Ok(())
    }

    pub async fn delete_holding(&self, symbol: &Symbol) -> Result<(), PortfolioError> {
        self.repo.delete_holding(symbol).await?;
        Ok(())
    }

    pub async fn valuation(&self) -> Result<portfolio_calc::PortfolioValuation, PortfolioError> {
        let portfolio = self.repo.load().await?;
        let quotes = self.market.snapshot().await;
        let settings = self.settings_repo.load().await?;
        let display = Currency::new(&settings.display_currency)
            .map_err(|_| PortfolioError::InvalidCurrency(settings.display_currency.clone()))?;
        let fx = self.fx.snapshot().await;
        Ok(portfolio_calc::evaluate(&portfolio, &quotes, &fx, display))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::MarketService;
    use crate::ports::asset_provider::MockAssetProvider;
    use crate::ports::repos::{AppSettings, MockPortfolioRepo, MockSettingsRepo, MockWatchlistRepo};
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, portfolio::Portfolio, price::Price,
        quantity::Quantity, quote::Quote, symbol::Symbol, watchlist::Watchlist,
    };
    use rust_decimal_macros::dec;

    fn s_aapl() -> Symbol { Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap() }
    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }

    #[tokio::test]
    async fn valuation_uses_market_snapshot() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(s_aapl());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut prov = MockAssetProvider::new();
        prov.expect_supports().returning(|s| s.kind() == AssetKind::UsEquity);
        prov.expect_fetch_quotes().returning(|symbols| {
            Ok(symbols.iter().map(|s|
                Quote::new(s.clone(), Price::new(usd(dec!(180))), Utc::now())
            ).collect())
        });

        let market = Arc::new(MarketService::new(Arc::new(wl_repo), vec![Arc::new(prov)]));
        market.refresh().await.unwrap();

        let mut pf_repo = MockPortfolioRepo::new();
        let mut pf = Portfolio::new();
        pf.upsert(Holding::new(s_aapl(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));
        let pf_clone = pf.clone();
        pf_repo.expect_load().returning(move || Ok(pf_clone.clone()));

        let mut settings_repo = MockSettingsRepo::new();
        settings_repo.expect_load().returning(|| Ok(AppSettings::default()));

        let fx = FxRateBook::default();
        let svc = PortfolioService::new(Arc::new(pf_repo), market, Arc::new(settings_repo), fx);
        let v = svc.valuation().await.unwrap();
        assert_eq!(v.total_value, Some(usd(dec!(1800))));
        assert_eq!(v.total_pnl, Some(usd(dec!(300))));
    }
}
