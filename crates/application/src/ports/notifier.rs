use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("backend error: {0}")]
    Backend(String),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, title: &str, body: &str) -> Result<(), NotifyError>;
}
