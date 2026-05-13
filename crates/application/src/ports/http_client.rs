use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout after {0:?}")]
    Timeout(std::time::Duration),
    #[error("invalid url: {0}")]
    InvalidUrl(String),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(
        &self,
        url: &str,
        headers: &[(&'static str, String)],
    ) -> Result<HttpResponse, HttpError>;
}
