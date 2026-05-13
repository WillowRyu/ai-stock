use application::ports::repos::{RepoError, WatchlistRepo};
use async_trait::async_trait;
use domain::{asset::AssetKind, symbol::Symbol, watchlist::Watchlist};
use sqlx::SqlitePool;

pub struct SqliteWatchlistRepo {
    pool: SqlitePool,
}

impl SqliteWatchlistRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn kind_to_str(k: AssetKind) -> &'static str {
    match k {
        AssetKind::Crypto => "crypto",
        AssetKind::UsEquity => "us",
        AssetKind::KrEquity => "kr",
        AssetKind::Forex => "fx",
        AssetKind::Commodity => "com",
    }
}

fn str_to_kind(s: &str) -> Option<AssetKind> {
    Some(match s {
        "crypto" => AssetKind::Crypto,
        "us" => AssetKind::UsEquity,
        "kr" => AssetKind::KrEquity,
        "fx" => AssetKind::Forex,
        "com" => AssetKind::Commodity,
        _ => return None,
    })
}

#[async_trait]
impl WatchlistRepo for SqliteWatchlistRepo {
    async fn load(&self) -> Result<Watchlist, RepoError> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT kind, ticker, quote_currency FROM watchlist ORDER BY position ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Storage(e.to_string()))?;
        let mut wl = Watchlist::new();
        for (k, t, q) in rows {
            let Some(kind) = str_to_kind(&k) else {
                continue;
            };
            let symbol = Symbol::new(kind, &t, q.as_deref())
                .map_err(|e| RepoError::Storage(format!("invalid symbol: {e}")))?;
            wl.add(symbol);
        }
        Ok(wl)
    }

    async fn save(&self, watchlist: &Watchlist) -> Result<(), RepoError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Storage(e.to_string()))?;
        sqlx::query("DELETE FROM watchlist")
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Storage(e.to_string()))?;
        for (pos, s) in watchlist.symbols().iter().enumerate() {
            sqlx::query(
                "INSERT INTO watchlist (kind, ticker, quote_currency, position) VALUES (?, ?, ?, ?)",
            )
            .bind(kind_to_str(s.kind()))
            .bind(s.ticker())
            .bind(s.quote_currency())
            .bind(pos as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Storage(e.to_string()))?;
        }
        tx.commit()
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
    async fn round_trip_watchlist() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let pool = open(&path).await.unwrap();
        let repo = SqliteWatchlistRepo::new(pool);

        let mut wl = Watchlist::new();
        wl.add(Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap());
        wl.add(Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap());

        repo.save(&wl).await.unwrap();
        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.symbols(), wl.symbols());
    }
}
