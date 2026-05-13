use async_trait::async_trait;
use domain::{holding::Holding, portfolio::Portfolio, symbol::Symbol, watchlist::Watchlist};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found")]
    NotFound,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WatchlistRepo: Send + Sync {
    async fn load(&self) -> Result<Watchlist, RepoError>;
    async fn save(&self, watchlist: &Watchlist) -> Result<(), RepoError>;
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PortfolioRepo: Send + Sync {
    async fn load(&self) -> Result<Portfolio, RepoError>;
    async fn upsert_holding(&self, holding: &Holding) -> Result<(), RepoError>;
    async fn delete_holding(&self, symbol: &Symbol) -> Result<(), RepoError>;
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    pub poll_interval_secs: u32,
    pub display_currency: String,
    pub theme: String,
    pub widget_opacity: f32,
    pub widget_always_on_top: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            display_currency: "USD".into(),
            theme: "dark".into(),
            widget_opacity: 0.85,
            widget_always_on_top: true,
        }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait SettingsRepo: Send + Sync {
    async fn load(&self) -> Result<AppSettings, RepoError>;
    async fn save(&self, settings: &AppSettings) -> Result<(), RepoError>;
}

use domain::alert::AlertRule;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AlertRepo: Send + Sync {
    async fn list(&self) -> Result<Vec<AlertRule>, RepoError>;
    async fn list_for_symbol(&self, symbol: &Symbol) -> Result<Vec<AlertRule>, RepoError>;
    async fn insert(&self, rule: &AlertRule) -> Result<i64, RepoError>;
    async fn update(&self, rule: &AlertRule) -> Result<(), RepoError>;
    async fn delete(&self, id: i64) -> Result<(), RepoError>;
    async fn record_fire(&self, id: i64, at: chrono::DateTime<chrono::Utc>) -> Result<(), RepoError>;
}
