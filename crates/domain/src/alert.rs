use crate::{money::Money, symbol::Symbol};
use chrono::{DateTime, Duration, Utc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AlertCondition {
    Above(Money),
    Below(Money),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlertRule {
    pub id: i64,
    pub symbol: Symbol,
    pub condition: AlertCondition,
    pub enabled: bool,
    pub cooldown_secs: u32,
    pub last_fired_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlertError {
    #[error("currency mismatch: rule {0} vs price {1}")]
    CurrencyMismatch(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertOutcome {
    NotTriggered,
    OnCooldown,
    Fire,
}

pub fn evaluate(rule: &AlertRule, price: Money, now: DateTime<Utc>) -> Result<AlertOutcome, AlertError> {
    if !rule.enabled {
        return Ok(AlertOutcome::NotTriggered);
    }
    let threshold = match &rule.condition {
        AlertCondition::Above(m) => m,
        AlertCondition::Below(m) => m,
    };
    if threshold.currency() != price.currency() {
        return Err(AlertError::CurrencyMismatch(
            threshold.currency().as_str().into(),
            price.currency().as_str().into(),
        ));
    }
    let crossed = match &rule.condition {
        AlertCondition::Above(m) => price.amount() >= m.amount(),
        AlertCondition::Below(m) => price.amount() <= m.amount(),
    };
    if !crossed {
        return Ok(AlertOutcome::NotTriggered);
    }
    if let Some(last) = rule.last_fired_at {
        if now - last < Duration::seconds(rule.cooldown_secs as i64) {
            return Ok(AlertOutcome::OnCooldown);
        }
    }
    Ok(AlertOutcome::Fire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::Currency};
    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn rule(condition: AlertCondition, last: Option<DateTime<Utc>>) -> AlertRule {
        AlertRule {
            id: 1,
            symbol: Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap(),
            condition, enabled: true, cooldown_secs: 60,
            last_fired_at: last,
        }
    }

    #[test]
    fn above_threshold_fires() {
        let r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        assert_eq!(evaluate(&r, usd(dec!(71000)), Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn below_threshold_fires() {
        let r = rule(AlertCondition::Below(usd(dec!(70000))), None);
        assert_eq!(evaluate(&r, usd(dec!(69000)), Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn cooldown_suppresses_consecutive_fires() {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 30).unwrap();
        let prev = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap();
        let r = rule(AlertCondition::Above(usd(dec!(70000))), Some(prev));
        assert_eq!(evaluate(&r, usd(dec!(71000)), now).unwrap(), AlertOutcome::OnCooldown);
    }

    #[test]
    fn currency_mismatch_errors() {
        let r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        let krw_price = Money::new(dec!(100_000_000), Currency::new("KRW").unwrap());
        assert!(evaluate(&r, krw_price, Utc::now()).is_err());
    }

    #[test]
    fn disabled_never_fires() {
        let mut r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        r.enabled = false;
        assert_eq!(evaluate(&r, usd(dec!(71000)), Utc::now()).unwrap(), AlertOutcome::NotTriggered);
    }
}
