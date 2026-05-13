use crate::{money::Money, symbol::Symbol};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AlertCondition {
    Above(Money),
    Below(Money),
    /// RSI(14) above the given threshold (e.g. 70 → overbought).
    RsiAbove(Decimal),
    /// RSI(14) below the given threshold (e.g. 30 → oversold).
    RsiBelow(Decimal),
    /// MACD line crosses above the signal line on the most recent tick.
    MacdGoldenCross,
    /// MACD line crosses below the signal line on the most recent tick.
    MacdDeathCross,
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

/// Snapshot of everything an `AlertCondition` might consult, for the current
/// tick and the immediately preceding tick (for cross detection). All
/// indicator fields are `Option` because indicators warm up gradually.
#[derive(Debug, Clone, Default)]
pub struct EvalContext {
    pub price: Option<Money>,
    pub rsi_14: Option<Decimal>,
    pub macd: Option<Decimal>,
    pub macd_signal: Option<Decimal>,
    pub prev_macd: Option<Decimal>,
    pub prev_macd_signal: Option<Decimal>,
}

pub fn evaluate(
    rule: &AlertRule,
    ctx: &EvalContext,
    now: DateTime<Utc>,
) -> Result<AlertOutcome, AlertError> {
    if !rule.enabled {
        return Ok(AlertOutcome::NotTriggered);
    }

    let crossed = match &rule.condition {
        AlertCondition::Above(m) | AlertCondition::Below(m) => {
            let Some(price) = ctx.price else { return Ok(AlertOutcome::NotTriggered); };
            if m.currency() != price.currency() {
                return Err(AlertError::CurrencyMismatch(
                    m.currency().as_str().into(),
                    price.currency().as_str().into(),
                ));
            }
            match &rule.condition {
                AlertCondition::Above(_) => price.amount() >= m.amount(),
                AlertCondition::Below(_) => price.amount() <= m.amount(),
                _ => unreachable!(),
            }
        }
        AlertCondition::RsiAbove(threshold) => {
            ctx.rsi_14.map_or(false, |v| v >= *threshold)
        }
        AlertCondition::RsiBelow(threshold) => {
            ctx.rsi_14.map_or(false, |v| v <= *threshold)
        }
        AlertCondition::MacdGoldenCross => {
            match (ctx.prev_macd, ctx.prev_macd_signal, ctx.macd, ctx.macd_signal) {
                (Some(pm), Some(ps), Some(cm), Some(cs)) => pm < ps && cm >= cs,
                _ => false,
            }
        }
        AlertCondition::MacdDeathCross => {
            match (ctx.prev_macd, ctx.prev_macd_signal, ctx.macd, ctx.macd_signal) {
                (Some(pm), Some(ps), Some(cm), Some(cs)) => pm > ps && cm <= cs,
                _ => false,
            }
        }
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

    fn usd(v: Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn rule(condition: AlertCondition, last: Option<DateTime<Utc>>) -> AlertRule {
        AlertRule {
            id: 1,
            symbol: Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap(),
            condition, enabled: true, cooldown_secs: 60,
            last_fired_at: last,
        }
    }
    fn price_ctx(amount: Decimal) -> EvalContext {
        EvalContext { price: Some(usd(amount)), ..EvalContext::default() }
    }

    #[test]
    fn above_threshold_fires() {
        let r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        assert_eq!(evaluate(&r, &price_ctx(dec!(71000)), Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn below_threshold_fires() {
        let r = rule(AlertCondition::Below(usd(dec!(70000))), None);
        assert_eq!(evaluate(&r, &price_ctx(dec!(69000)), Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn cooldown_suppresses_consecutive_fires() {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 30).unwrap();
        let prev = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap();
        let r = rule(AlertCondition::Above(usd(dec!(70000))), Some(prev));
        assert_eq!(evaluate(&r, &price_ctx(dec!(71000)), now).unwrap(), AlertOutcome::OnCooldown);
    }

    #[test]
    fn currency_mismatch_errors() {
        let r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        let ctx = EvalContext {
            price: Some(Money::new(dec!(100_000_000), Currency::new("KRW").unwrap())),
            ..EvalContext::default()
        };
        assert!(evaluate(&r, &ctx, Utc::now()).is_err());
    }

    #[test]
    fn disabled_never_fires() {
        let mut r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        r.enabled = false;
        assert_eq!(evaluate(&r, &price_ctx(dec!(71000)), Utc::now()).unwrap(), AlertOutcome::NotTriggered);
    }

    #[test]
    fn rsi_above_fires_when_overbought() {
        let r = rule(AlertCondition::RsiAbove(dec!(70)), None);
        let ctx = EvalContext { rsi_14: Some(dec!(75)), ..EvalContext::default() };
        assert_eq!(evaluate(&r, &ctx, Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn rsi_above_does_not_fire_when_below_threshold() {
        let r = rule(AlertCondition::RsiAbove(dec!(70)), None);
        let ctx = EvalContext { rsi_14: Some(dec!(60)), ..EvalContext::default() };
        assert_eq!(evaluate(&r, &ctx, Utc::now()).unwrap(), AlertOutcome::NotTriggered);
    }

    #[test]
    fn rsi_below_fires_when_oversold() {
        let r = rule(AlertCondition::RsiBelow(dec!(30)), None);
        let ctx = EvalContext { rsi_14: Some(dec!(25)), ..EvalContext::default() };
        assert_eq!(evaluate(&r, &ctx, Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn rsi_warmup_does_not_fire() {
        let r = rule(AlertCondition::RsiAbove(dec!(70)), None);
        let ctx = EvalContext { rsi_14: None, ..EvalContext::default() };
        assert_eq!(evaluate(&r, &ctx, Utc::now()).unwrap(), AlertOutcome::NotTriggered);
    }

    #[test]
    fn macd_golden_cross_detected_on_transition() {
        let r = rule(AlertCondition::MacdGoldenCross, None);
        let ctx = EvalContext {
            prev_macd: Some(dec!(-0.5)),
            prev_macd_signal: Some(dec!(-0.2)),
            macd: Some(dec!(0.1)),
            macd_signal: Some(dec!(0.0)),
            ..EvalContext::default()
        };
        assert_eq!(evaluate(&r, &ctx, Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn macd_golden_cross_does_not_fire_without_transition() {
        let r = rule(AlertCondition::MacdGoldenCross, None);
        let ctx = EvalContext {
            prev_macd: Some(dec!(0.5)),
            prev_macd_signal: Some(dec!(0.3)),
            macd: Some(dec!(0.6)),
            macd_signal: Some(dec!(0.4)),
            ..EvalContext::default()
        };
        assert_eq!(evaluate(&r, &ctx, Utc::now()).unwrap(), AlertOutcome::NotTriggered);
    }

    #[test]
    fn macd_death_cross_detected_on_transition() {
        let r = rule(AlertCondition::MacdDeathCross, None);
        let ctx = EvalContext {
            prev_macd: Some(dec!(0.5)),
            prev_macd_signal: Some(dec!(0.2)),
            macd: Some(dec!(-0.1)),
            macd_signal: Some(dec!(0.0)),
            ..EvalContext::default()
        };
        assert_eq!(evaluate(&r, &ctx, Utc::now()).unwrap(), AlertOutcome::Fire);
    }
}
