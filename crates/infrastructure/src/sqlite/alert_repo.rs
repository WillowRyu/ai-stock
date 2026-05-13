use application::ports::repos::{AlertRepo, RepoError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    alert::{AlertCondition, AlertRule},
    asset::AssetKind, money::{Currency, Money}, symbol::Symbol,
};
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use std::str::FromStr;

pub struct SqliteAlertRepo { pool: SqlitePool }

impl SqliteAlertRepo { pub fn new(pool: SqlitePool) -> Self { Self { pool } } }

fn kind_to_str(k: AssetKind) -> &'static str {
    match k {
        AssetKind::Crypto => "crypto", AssetKind::UsEquity => "us", AssetKind::KrEquity => "kr",
        AssetKind::Forex => "fx", AssetKind::Commodity => "com",
    }
}
fn str_to_kind(s: &str) -> Option<AssetKind> {
    Some(match s {
        "crypto" => AssetKind::Crypto, "us" => AssetKind::UsEquity, "kr" => AssetKind::KrEquity,
        "fx" => AssetKind::Forex, "com" => AssetKind::Commodity, _ => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn row_to_rule(
    id: i64, kind: &str, ticker: &str, qc: Option<&str>,
    cond_kind: &str, thresh_amt: &str, thresh_ccy: &str,
    enabled: i64, cooldown: i64, last: Option<&str>,
) -> Result<AlertRule, RepoError> {
    let symbol = Symbol::new(
        str_to_kind(kind).ok_or_else(|| RepoError::Storage(format!("bad kind: {kind}")))?,
        ticker, qc,
    ).map_err(|e| RepoError::Storage(e.to_string()))?;
    let amount = Decimal::from_str(thresh_amt).map_err(|e| RepoError::Storage(e.to_string()))?;
    let ccy = Currency::new(thresh_ccy).map_err(|e| RepoError::Storage(format!("{e:?}")))?;
    let condition = match cond_kind {
        "above" => AlertCondition::Above(Money::new(amount, ccy)),
        "below" => AlertCondition::Below(Money::new(amount, ccy)),
        _ => return Err(RepoError::Storage(format!("bad condition: {cond_kind}"))),
    };
    let last_fired_at = last.and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc));
    Ok(AlertRule { id, symbol, condition, enabled: enabled != 0, cooldown_secs: cooldown as u32, last_fired_at })
}

#[async_trait]
impl AlertRepo for SqliteAlertRepo {
    async fn list(&self) -> Result<Vec<AlertRule>, RepoError> {
        let rows: Vec<(i64, String, String, Option<String>, String, String, String, i64, i64, Option<String>)> =
            sqlx::query_as("SELECT id, kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at FROM alerts")
            .fetch_all(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        rows.into_iter().map(|(id, k, t, qc, ck, ta, tc, en, cd, lf)| {
            row_to_rule(id, &k, &t, qc.as_deref(), &ck, &ta, &tc, en, cd, lf.as_deref())
        }).collect()
    }

    async fn list_for_symbol(&self, symbol: &Symbol) -> Result<Vec<AlertRule>, RepoError> {
        let rows: Vec<(i64, String, String, Option<String>, String, String, String, i64, i64, Option<String>)> =
            sqlx::query_as("SELECT id, kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at FROM alerts WHERE kind = ? AND ticker = ? AND coalesce(quote_currency,'') = coalesce(?,'')")
            .bind(kind_to_str(symbol.kind()))
            .bind(symbol.ticker())
            .bind(symbol.quote_currency())
            .fetch_all(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        rows.into_iter().map(|(id, k, t, qc, ck, ta, tc, en, cd, lf)| {
            row_to_rule(id, &k, &t, qc.as_deref(), &ck, &ta, &tc, en, cd, lf.as_deref())
        }).collect()
    }

    async fn insert(&self, rule: &AlertRule) -> Result<i64, RepoError> {
        let (cond_kind, thresh) = match &rule.condition {
            AlertCondition::Above(m) => ("above", m),
            AlertCondition::Below(m) => ("below", m),
        };
        let r = sqlx::query(
            "INSERT INTO alerts (kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(kind_to_str(rule.symbol.kind()))
        .bind(rule.symbol.ticker())
        .bind(rule.symbol.quote_currency())
        .bind(cond_kind)
        .bind(thresh.amount().to_string())
        .bind(thresh.currency().as_str())
        .bind(if rule.enabled { 1_i64 } else { 0_i64 })
        .bind(rule.cooldown_secs as i64)
        .bind(rule.last_fired_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(r.last_insert_rowid())
    }

    async fn update(&self, rule: &AlertRule) -> Result<(), RepoError> {
        let (cond_kind, thresh) = match &rule.condition {
            AlertCondition::Above(m) => ("above", m),
            AlertCondition::Below(m) => ("below", m),
        };
        sqlx::query(
            "UPDATE alerts SET kind=?, ticker=?, quote_currency=?, condition_kind=?, threshold_amount=?, threshold_currency=?, enabled=?, cooldown_secs=?, last_fired_at=? WHERE id = ?",
        )
        .bind(kind_to_str(rule.symbol.kind()))
        .bind(rule.symbol.ticker())
        .bind(rule.symbol.quote_currency())
        .bind(cond_kind)
        .bind(thresh.amount().to_string())
        .bind(thresh.currency().as_str())
        .bind(if rule.enabled { 1_i64 } else { 0_i64 })
        .bind(rule.cooldown_secs as i64)
        .bind(rule.last_fired_at.map(|t| t.to_rfc3339()))
        .bind(rule.id)
        .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM alerts WHERE id = ?")
            .bind(id)
            .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn record_fire(&self, id: i64, at: DateTime<Utc>) -> Result<(), RepoError> {
        sqlx::query("UPDATE alerts SET last_fired_at = ? WHERE id = ?")
            .bind(at.to_rfc3339())
            .bind(id)
            .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
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
    async fn insert_list_and_delete() {
        let dir = tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        let repo = SqliteAlertRepo::new(pool);
        let rule = AlertRule {
            id: 0,
            symbol: Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap(),
            condition: AlertCondition::Above(Money::new(dec!(70000), Currency::new("USD").unwrap())),
            enabled: true, cooldown_secs: 60, last_fired_at: None,
        };
        let id = repo.insert(&rule).await.unwrap();
        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        repo.delete(id).await.unwrap();
        assert!(repo.list().await.unwrap().is_empty());
    }
}
