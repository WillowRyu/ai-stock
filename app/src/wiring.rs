use application::{
    market_service::MarketService, portfolio_service::PortfolioService,
    settings_service::SettingsService,
    ports::{asset_provider::AssetProvider, http_client::HttpClient},
    poll_scheduler::PollScheduler,
};
use infrastructure::{
    clock::SystemClock, http::ReqwestHttpClient, keyring_secrets::KeyringSecretStore,
    providers::{binance::BinanceProvider, coingecko::CoinGeckoProvider, finnhub::FinnhubProvider, yahoo::YahooProvider},
    sqlite::{open, watchlist_repo::SqliteWatchlistRepo, portfolio_repo::SqlitePortfolioRepo, settings_repo::SqliteSettingsRepo},
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub struct AppState {
    pub market: Arc<MarketService>,
    pub portfolio: Arc<PortfolioService>,
    pub settings: Arc<SettingsService>,
    pub secrets: Arc<KeyringSecretStore>,
}

pub async fn assemble(db_path: PathBuf, finnhub_key: Option<String>) -> AppState {
    let pool = open(&db_path).await.expect("open sqlite");
    let watchlist_repo = Arc::new(SqliteWatchlistRepo::new(pool.clone()));
    let portfolio_repo = Arc::new(SqlitePortfolioRepo::new(pool.clone()));
    let settings_repo = Arc::new(SqliteSettingsRepo::new(pool.clone()));

    let http: Arc<dyn HttpClient> = Arc::new(ReqwestHttpClient::new());

    let mut coingecko_ids = HashMap::new();
    for (t, id) in [("BTC", "bitcoin"), ("ETH", "ethereum"), ("SOL", "solana"), ("XRP", "ripple")] {
        coingecko_ids.insert(t.into(), id.into());
    }

    let mut providers: Vec<Arc<dyn AssetProvider>> = vec![
        Arc::new(BinanceProvider::new(http.clone())),
        Arc::new(CoinGeckoProvider::new(http.clone(), coingecko_ids)),
        Arc::new(YahooProvider::new(http.clone())),
    ];
    if let Some(key) = finnhub_key {
        providers.push(Arc::new(FinnhubProvider::new(http.clone(), key)));
    }

    let market = Arc::new(MarketService::new(watchlist_repo, providers));
    let portfolio = Arc::new(PortfolioService::new(portfolio_repo, market.clone()));
    let settings = Arc::new(SettingsService::new(settings_repo));
    let secrets = Arc::new(KeyringSecretStore::new("dev.willowryu.aistock"));

    // Kick off poller (5s default). Future: read from settings.
    let clock = Arc::new(SystemClock);
    let (scheduler, _rx) = PollScheduler::new(market.clone(), clock);
    scheduler.start(std::time::Duration::from_secs(5));

    AppState { market, portfolio, settings, secrets }
}
