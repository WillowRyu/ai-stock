use application::{
    alert_service::AlertService,
    market_service::MarketService, portfolio_service::PortfolioService,
    settings_service::SettingsService,
    ports::{asset_provider::AssetProvider, http_client::HttpClient},
    poll_scheduler::PollScheduler,
};
use infrastructure::{
    clock::SystemClock, http::ReqwestHttpClient, keyring_secrets::KeyringSecretStore,
    providers::{binance::BinanceProvider, coingecko::CoinGeckoProvider, finnhub::FinnhubProvider, yahoo::YahooProvider},
    sqlite::{open, alert_repo::SqliteAlertRepo, watchlist_repo::SqliteWatchlistRepo, portfolio_repo::SqlitePortfolioRepo, settings_repo::SqliteSettingsRepo},
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tauri::AppHandle;

use crate::tauri_notifier::TauriNotifier;

pub struct AppState {
    pub market: Arc<MarketService>,
    pub portfolio: Arc<PortfolioService>,
    pub settings: Arc<SettingsService>,
    pub alerts: Arc<AlertService>,
    // Held for future IPC commands (e.g. API-key management); not yet read.
    #[allow(dead_code)]
    pub secrets: Arc<KeyringSecretStore>,
}

pub async fn assemble(app_handle: AppHandle, db_path: PathBuf, finnhub_key: Option<String>) -> AppState {
    let pool = open(&db_path).await.expect("open sqlite");
    let watchlist_repo = Arc::new(SqliteWatchlistRepo::new(pool.clone()));
    let portfolio_repo = Arc::new(SqlitePortfolioRepo::new(pool.clone()));
    let settings_repo = Arc::new(SqliteSettingsRepo::new(pool.clone()));
    let alert_repo = Arc::new(SqliteAlertRepo::new(pool.clone()));

    let notifier = Arc::new(TauriNotifier::new(app_handle.clone()));
    let clock_arc: Arc<dyn application::ports::clock::Clock> = Arc::new(SystemClock);
    let alerts = Arc::new(AlertService::new(alert_repo, notifier, clock_arc.clone()));

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
    let (scheduler, _rx) = PollScheduler::new(market.clone(), clock_arc.clone());
    scheduler.start(std::time::Duration::from_secs(5));

    AppState { market, portfolio, settings, alerts, secrets }
}
