use crate::ports::repos::{AppSettings, RepoError, SettingsRepo};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("invalid poll interval: must be 1..=300 seconds, got {0}")]
    InvalidPollInterval(u32),
    #[error("invalid widget opacity: must be 0.1..=1.0, got {0}")]
    InvalidWidgetOpacity(f32),
}

pub struct SettingsService { repo: Arc<dyn SettingsRepo> }

impl SettingsService {
    pub fn new(repo: Arc<dyn SettingsRepo>) -> Self { Self { repo } }

    pub async fn get(&self) -> Result<AppSettings, SettingsError> {
        Ok(self.repo.load().await?)
    }

    pub async fn save(&self, settings: AppSettings) -> Result<(), SettingsError> {
        if !(1..=300).contains(&settings.poll_interval_secs) {
            return Err(SettingsError::InvalidPollInterval(settings.poll_interval_secs));
        }
        if !(0.1..=1.0).contains(&settings.widget_opacity) {
            return Err(SettingsError::InvalidWidgetOpacity(settings.widget_opacity));
        }
        self.repo.save(&settings).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::repos::MockSettingsRepo;

    #[tokio::test]
    async fn rejects_too_small_poll_interval() {
        let mut repo = MockSettingsRepo::new();
        repo.expect_save().never();
        let svc = SettingsService::new(Arc::new(repo));
        let s = AppSettings { poll_interval_secs: 0, ..AppSettings::default() };
        assert!(matches!(svc.save(s).await, Err(SettingsError::InvalidPollInterval(0))));
    }

    #[tokio::test]
    async fn rejects_excessive_opacity() {
        let mut repo = MockSettingsRepo::new();
        repo.expect_save().never();
        let svc = SettingsService::new(Arc::new(repo));
        let s = AppSettings { widget_opacity: 1.5, ..AppSettings::default() };
        assert!(matches!(svc.save(s).await, Err(SettingsError::InvalidWidgetOpacity(_))));
    }
}
