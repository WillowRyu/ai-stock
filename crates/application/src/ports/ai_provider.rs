use async_trait::async_trait;
use futures::stream::BoxStream;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AiPrompt {
    pub system: String,
    pub user: String,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum AiChunk {
    Text(String),
    Done,
}

#[derive(Debug, Error)]
pub enum AiError {
    #[error("not configured (no api key)")]
    NotConfigured,
    #[error("unauthorized — invalid api key")]
    Unauthorized,
    #[error("rate limited; retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("network error: {0}")]
    Network(String),
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn stream(&self, prompt: AiPrompt) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError>;
}
