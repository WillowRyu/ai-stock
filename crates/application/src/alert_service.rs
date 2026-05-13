use crate::ports::{
    clock::Clock, notifier::{NotifyError, Notifier}, repos::{AlertRepo, RepoError},
};
use domain::{alert::{evaluate, AlertCondition, AlertError, AlertOutcome, AlertRule}, quote::Quote};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AlertServiceError {
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("alert eval: {0}")]
    Alert(#[from] AlertError),
    #[error("notify: {0}")]
    Notify(#[from] NotifyError),
}

pub struct AlertService {
    repo: Arc<dyn AlertRepo>,
    notifier: Arc<dyn Notifier>,
    clock: Arc<dyn Clock>,
}

impl AlertService {
    pub fn new(repo: Arc<dyn AlertRepo>, notifier: Arc<dyn Notifier>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, notifier, clock }
    }

    pub async fn list(&self) -> Result<Vec<AlertRule>, AlertServiceError> {
        Ok(self.repo.list().await?)
    }

    pub async fn create(&self, rule: AlertRule) -> Result<i64, AlertServiceError> {
        Ok(self.repo.insert(&rule).await?)
    }

    pub async fn delete(&self, id: i64) -> Result<(), AlertServiceError> {
        Ok(self.repo.delete(id).await?)
    }

    /// Called by the poll loop on every quote-update. Skips silently on currency mismatch
    /// (misconfigured rules shouldn't crash the polling loop).
    pub async fn evaluate_quote(&self, quote: &Quote) -> Result<(), AlertServiceError> {
        let rules = self.repo.list_for_symbol(&quote.symbol).await?;
        let now = self.clock.now();
        for rule in rules {
            match evaluate(&rule, quote.price.money(), now) {
                Ok(AlertOutcome::Fire) => {
                    let threshold_str = match &rule.condition {
                        AlertCondition::Above(m) | AlertCondition::Below(m) =>
                            format!("{} {}", m.amount(), m.currency().as_str()),
                    };
                    let cond_str = match &rule.condition {
                        AlertCondition::Above(_) => "above",
                        AlertCondition::Below(_) => "below",
                    };
                    let body = format!("{} is {} {}", quote.symbol.ticker(), cond_str, threshold_str);
                    self.notifier.notify(&format!("Alert · {}", quote.symbol.ticker()), &body).await?;
                    self.repo.record_fire(rule.id, now).await?;
                }
                Ok(_) => {}
                Err(_) => { /* currency mismatch — surface in UI later; don't crash polling */ }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{clock::MockClock, notifier::MockNotifier, repos::MockAlertRepo};
    use chrono::TimeZone;
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol,
    };
    use mockall::predicate::*;
    use rust_decimal_macros::dec;

    fn s_btc() -> Symbol { Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap() }
    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn q(amount: rust_decimal::Decimal) -> Quote {
        Quote::new(s_btc(), Price::new(usd(amount)), Utc::now())
    }

    #[tokio::test]
    async fn fires_when_above_threshold_and_records() {
        let mut repo = MockAlertRepo::new();
        repo.expect_list_for_symbol().returning(|_| Ok(vec![AlertRule {
            id: 1, symbol: s_btc(),
            condition: AlertCondition::Above(usd(dec!(70000))),
            enabled: true, cooldown_secs: 0, last_fired_at: None,
        }]));
        repo.expect_record_fire().with(eq(1), always()).times(1).returning(|_, _| Ok(()));

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().times(1).returning(|_, _| Ok(()));

        let mut clock = MockClock::new();
        clock.expect_now().returning(|| Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap());

        let svc = AlertService::new(Arc::new(repo), Arc::new(notifier), Arc::new(clock));
        svc.evaluate_quote(&q(dec!(71000))).await.unwrap();
    }
}
