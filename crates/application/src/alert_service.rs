use crate::indicator_service::compute_series;
use crate::market_service::MarketService;
use crate::ports::{
    clock::Clock, notifier::{NotifyError, Notifier}, repos::{AlertRepo, RepoError},
};
use domain::{
    alert::{evaluate, AlertCondition, AlertError, AlertOutcome, AlertRule, EvalContext},
    quote::Quote,
};
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
    market: Arc<MarketService>,
}

impl AlertService {
    pub fn new(
        repo: Arc<dyn AlertRepo>,
        notifier: Arc<dyn Notifier>,
        clock: Arc<dyn Clock>,
        market: Arc<MarketService>,
    ) -> Self {
        Self { repo, notifier, clock, market }
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

    pub async fn evaluate_quote(&self, quote: &Quote) -> Result<(), AlertServiceError> {
        let rules = self.repo.list_for_symbol(&quote.symbol).await?;
        if rules.is_empty() {
            return Ok(());
        }
        let now = self.clock.now();

        // Build the eval context. Always include the price; only fetch
        // candles + compute indicators if at least one rule needs them.
        let needs_indicators = rules.iter().any(|r| matches!(
            r.condition,
            AlertCondition::RsiAbove(_) | AlertCondition::RsiBelow(_)
            | AlertCondition::MacdGoldenCross | AlertCondition::MacdDeathCross
        ));

        let (rsi_14, macd, macd_signal, prev_macd, prev_macd_signal) = if needs_indicators {
            let from = chrono::Utc::now() - chrono::Duration::days(90);
            let to = chrono::Utc::now();
            match self.market.fetch_candles(&quote.symbol, from, to).await {
                Ok(candles) if !candles.is_empty() => {
                    let series = compute_series(&candles);
                    let last = |v: &[Option<rust_decimal::Decimal>]| v.last().copied().flatten();
                    let prev = |v: &[Option<rust_decimal::Decimal>]| {
                        if v.len() < 2 { None } else { v[v.len() - 2] }
                    };
                    (
                        last(&series.rsi_14),
                        last(&series.macd),
                        last(&series.macd_signal),
                        prev(&series.macd),
                        prev(&series.macd_signal),
                    )
                }
                _ => (None, None, None, None, None),
            }
        } else {
            (None, None, None, None, None)
        };

        let ctx = EvalContext {
            price: Some(quote.price.money()),
            rsi_14, macd, macd_signal, prev_macd, prev_macd_signal,
        };

        for rule in rules {
            match evaluate(&rule, &ctx, now) {
                Ok(AlertOutcome::Fire) => {
                    let title = format!("Alert · {}", quote.symbol.ticker());
                    let body = describe_fire(&rule);
                    self.notifier.notify(&title, &body).await?;
                    self.repo.record_fire(rule.id, now).await?;
                }
                Ok(_) => {}
                Err(_) => { /* currency mismatch; surface in UI later */ }
            }
        }
        Ok(())
    }
}

fn describe_fire(rule: &AlertRule) -> String {
    let s = &rule.symbol;
    match &rule.condition {
        AlertCondition::Above(m) => format!("{} is at or above {} {}", s.ticker(), m.amount(), m.currency().as_str()),
        AlertCondition::Below(m) => format!("{} is at or below {} {}", s.ticker(), m.amount(), m.currency().as_str()),
        AlertCondition::RsiAbove(t) => format!("{} RSI(14) is above {}", s.ticker(), t),
        AlertCondition::RsiBelow(t) => format!("{} RSI(14) is below {}", s.ticker(), t),
        AlertCondition::MacdGoldenCross => format!("{} MACD golden cross", s.ticker()),
        AlertCondition::MacdDeathCross => format!("{} MACD death cross", s.ticker()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::MarketService;
    use crate::ports::asset_provider::MockAssetProvider;
    use crate::ports::clock::MockClock;
    use crate::ports::notifier::MockNotifier;
    use crate::ports::repos::{MockAlertRepo, MockWatchlistRepo};
    use chrono::TimeZone;
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol,
        watchlist::Watchlist,
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

        let mut wl_repo = MockWatchlistRepo::new();
        wl_repo.expect_load().returning(|| Ok(Watchlist::new()));
        let mut prov = MockAssetProvider::new();
        prov.expect_supports().return_const(false);
        let market = Arc::new(MarketService::new(Arc::new(wl_repo), vec![Arc::new(prov)]));

        let svc = AlertService::new(Arc::new(repo), Arc::new(notifier), Arc::new(clock), market);
        svc.evaluate_quote(&q(dec!(71000))).await.unwrap();
    }
}
