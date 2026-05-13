use application::{
    ai_service::AiService,
    alert_service::AlertService,
    market_service::MarketService, portfolio_service::PortfolioService,
    settings_service::SettingsService,
    ports::{ai_provider::AiProvider, asset_provider::AssetProvider, http_client::HttpClient, news_provider::NewsProvider},
};
use infrastructure::{
    clock::SystemClock, http::ReqwestHttpClient, keyring_secrets::KeyringSecretStore,
    news::{coindesk_rss::CoinDeskRss, yahoo_rss::YahooNewsRss},
    providers::{anthropic::AnthropicProvider, binance::BinanceProvider, coingecko::CoinGeckoProvider, finnhub::FinnhubProvider, gemini::GeminiProvider, naver_kr::NaverKrProvider, openai::OpenAiProvider, yahoo::YahooProvider},
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
    pub secrets: Arc<KeyringSecretStore>,
    pub ai: Arc<AiService>,
}

pub async fn assemble(app_handle: AppHandle, db_path: PathBuf, finnhub_key: Option<String>) -> AppState {
    let pool = open(&db_path).await.expect("open sqlite");
    let watchlist_repo = Arc::new(SqliteWatchlistRepo::new(pool.clone()));
    let portfolio_repo = Arc::new(SqlitePortfolioRepo::new(pool.clone()));
    let settings_repo = Arc::new(SqliteSettingsRepo::new(pool.clone()));
    let alert_repo = Arc::new(SqliteAlertRepo::new(pool.clone()));

    let notifier = Arc::new(TauriNotifier::new(app_handle.clone()));
    let clock_arc: Arc<dyn application::ports::clock::Clock> = Arc::new(SystemClock);

    let http: Arc<dyn HttpClient> = Arc::new(ReqwestHttpClient::new());

    let mut coingecko_ids = HashMap::new();
    for (t, id) in [("BTC", "bitcoin"), ("ETH", "ethereum"), ("SOL", "solana"), ("XRP", "ripple")] {
        coingecko_ids.insert(t.into(), id.into());
    }

    // Provider ordering matters: MarketService::refresh tries each supporting provider
    // in order and falls back on error. Put authenticated/working sources first to keep
    // hot path cheap and logs quiet.
    let mut providers: Vec<Arc<dyn AssetProvider>> = vec![
        Arc::new(BinanceProvider::new(http.clone())),
        Arc::new(CoinGeckoProvider::new(http.clone(), coingecko_ids)),
    ];
    if let Some(key) = finnhub_key {
        // Finnhub before Yahoo for US equities: Yahoo's public quote endpoint is
        // currently authentication-gated and returns 401 without a crumb/cookie.
        providers.push(Arc::new(FinnhubProvider::new(http.clone(), key)));
    }
    providers.push(Arc::new(YahooProvider::new(http.clone())));
    providers.push(Arc::new(NaverKrProvider::new(http.clone())));

    let market = Arc::new(MarketService::new(watchlist_repo, providers));
    let alerts = Arc::new(AlertService::new(alert_repo, notifier, clock_arc.clone(), market.clone()));
    let portfolio = Arc::new(PortfolioService::new(portfolio_repo, market.clone()));
    let settings = Arc::new(SettingsService::new(settings_repo.clone()));
    let secrets = Arc::new(KeyringSecretStore::new("dev.willowryu.aistock"));

    // NOTE: The periodic refresh loop is driven from `main.rs` rather than
    // `PollScheduler` here so the app event loop can capture per-symbol
    // provider errors from `RefreshOutcome` and emit them to the UI as
    // `provider-error` events. `PollScheduler` is still available as a
    // building block (and tested) but is no longer started from `assemble`.

    let news: Vec<Arc<dyn NewsProvider>> = vec![
        Arc::new(YahooNewsRss::new(http.clone())),
        Arc::new(CoinDeskRss::new(http.clone())),
    ];

    let provider_factory: application::ai_service::ProviderFactory =
        Arc::new(|kind: &str, key: &str| -> Option<Arc<dyn AiProvider>> {
            match kind {
                "openai" => Some(Arc::new(OpenAiProvider::new(key.into(), "gpt-4o-mini".into()))),
                "anthropic" => Some(Arc::new(AnthropicProvider::new(key.into(), "claude-haiku-4-5-20251001".into()))),
                "gemini" => Some(Arc::new(GeminiProvider::new(key.into(), "gemini-2.0-flash".into()))),
                _ => None,
            }
        });

    let secrets_dyn: Arc<dyn application::ports::secret_store::SecretStore> = secrets.clone();
    let ai = Arc::new(AiService::new(secrets_dyn, market.clone(), news, provider_factory));

    AppState { market, portfolio, settings, alerts, secrets, ai }
}
