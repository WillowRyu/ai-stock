use application::ports::repos::{PortfolioRepo, RepoError};
use async_trait::async_trait;
use domain::{
    asset::AssetKind,
    holding::Holding,
    money::{Currency, Money},
    portfolio::Portfolio,
    quantity::Quantity,
    symbol::Symbol,
};
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use std::str::FromStr;

pub struct SqlitePortfolioRepo {
    pool: SqlitePool,
}

impl SqlitePortfolioRepo {
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
impl PortfolioRepo for SqlitePortfolioRepo {
    async fn load(&self) -> Result<Portfolio, RepoError> {
        let rows: Vec<(String, String, Option<String>, String, String, String)> = sqlx::query_as(
            "SELECT kind, ticker, quote_currency, quantity, avg_cost_amount, avg_cost_currency FROM holdings",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Storage(e.to_string()))?;
        let mut p = Portfolio::new();
        for (kind_s, ticker, qc, qty_s, amt_s, ccy_s) in rows {
            let kind = str_to_kind(&kind_s)
                .ok_or_else(|| RepoError::Storage(format!("bad kind: {kind_s}")))?;
            let symbol = Symbol::new(kind, &ticker, qc.as_deref())
                .map_err(|e| RepoError::Storage(e.to_string()))?;
            let qty = Decimal::from_str(&qty_s).map_err(|e| RepoError::Storage(e.to_string()))?;
            let amt = Decimal::from_str(&amt_s).map_err(|e| RepoError::Storage(e.to_string()))?;
            let ccy = Currency::new(&ccy_s).map_err(|e| RepoError::Storage(e.to_string()))?;
            p.upsert(Holding::new(
                symbol,
                Quantity::new(qty).map_err(|e| RepoError::Storage(format!("{e:?}")))?,
                Money::new(amt, ccy),
            ));
        }
        Ok(p)
    }

    async fn upsert_holding(&self, h: &Holding) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO holdings (kind, ticker, quote_currency, quantity, avg_cost_amount, avg_cost_currency)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(kind, ticker, quote_currency) DO UPDATE SET
               quantity = excluded.quantity,
               avg_cost_amount = excluded.avg_cost_amount,
               avg_cost_currency = excluded.avg_cost_currency",
        )
        .bind(kind_to_str(h.symbol.kind()))
        .bind(h.symbol.ticker())
        .bind(h.symbol.quote_currency())
        .bind(h.quantity.value().to_string())
        .bind(h.avg_cost.amount().to_string())
        .bind(h.avg_cost.currency().as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete_holding(&self, symbol: &Symbol) -> Result<(), RepoError> {
        sqlx::query(
            "DELETE FROM holdings WHERE kind = ? AND ticker = ? AND coalesce(quote_currency,'') = coalesce(?,'')",
        )
        .bind(kind_to_str(symbol.kind()))
        .bind(symbol.ticker())
        .bind(symbol.quote_currency())
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
    use rust_decimal_macros::dec;
    use tempfile::tempdir;

    #[tokio::test]
    async fn upsert_then_load() {
        let dir = tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        let repo = SqlitePortfolioRepo::new(pool);

        let h = Holding::new(
            Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            Quantity::new(dec!(10)).unwrap(),
            Money::new(dec!(150), Currency::new("USD").unwrap()),
        );
        repo.upsert_holding(&h).await.unwrap();

        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.holdings().len(), 1);
        assert_eq!(loaded.holdings()[0], h);

        repo.delete_holding(&h.symbol).await.unwrap();
        assert!(repo.load().await.unwrap().holdings().is_empty());
    }
}
