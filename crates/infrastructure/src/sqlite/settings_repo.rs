use application::ports::repos::{AppSettings, RepoError, SettingsRepo};
use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct SqliteSettingsRepo {
    pool: SqlitePool,
}

impl SqliteSettingsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SettingsRepo for SqliteSettingsRepo {
    async fn load(&self) -> Result<AppSettings, RepoError> {
        let row: Option<(i64, String, String, f64, i64)> = sqlx::query_as(
            "SELECT poll_interval_secs, display_currency, theme, widget_opacity, widget_always_on_top
             FROM settings WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Storage(e.to_string()))?;

        match row {
            Some((iv, cur, theme, op, ot)) => Ok(AppSettings {
                poll_interval_secs: iv as u32,
                display_currency: cur,
                theme,
                widget_opacity: op as f32,
                widget_always_on_top: ot != 0,
            }),
            None => Ok(AppSettings::default()),
        }
    }

    async fn save(&self, s: &AppSettings) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO settings (id, poll_interval_secs, display_currency, theme, widget_opacity, widget_always_on_top)
             VALUES (1, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               poll_interval_secs = excluded.poll_interval_secs,
               display_currency = excluded.display_currency,
               theme = excluded.theme,
               widget_opacity = excluded.widget_opacity,
               widget_always_on_top = excluded.widget_always_on_top",
        )
        .bind(s.poll_interval_secs as i64)
        .bind(&s.display_currency)
        .bind(&s.theme)
        .bind(s.widget_opacity as f64)
        .bind(if s.widget_always_on_top { 1_i64 } else { 0_i64 })
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open;
    use tempfile::tempdir;

    #[tokio::test]
    async fn round_trip_settings() {
        let dir = tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        let repo = SqliteSettingsRepo::new(pool);
        let s = AppSettings {
            poll_interval_secs: 7,
            widget_opacity: 0.5,
            ..AppSettings::default()
        };
        repo.save(&s).await.unwrap();
        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.poll_interval_secs, 7);
        assert!((loaded.widget_opacity - 0.5).abs() < 1e-6);
    }
}
