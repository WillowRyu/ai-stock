use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::symbol::Symbol;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Headline {
    pub title: String,
    pub url: String,
    pub source: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum NewsError {
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("parse error: {0}")]
    Parse(String),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait NewsProvider: Send + Sync {
    async fn fetch(&self, symbol: &Symbol, limit: usize) -> Result<Vec<Headline>, NewsError>;
}
