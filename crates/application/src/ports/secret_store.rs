use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("backend error: {0}")]
    Backend(String),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<String, SecretError>;
    async fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    async fn delete(&self, key: &str) -> Result<(), SecretError>;
}
