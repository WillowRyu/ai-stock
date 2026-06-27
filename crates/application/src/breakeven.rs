//! Thin orchestration for the break-even / averaging-down calculator. The whole
//! calculation runs in a chosen **base currency**: the supplied price (native for
//! a live quote) is converted into the base via the current FX snapshot, then the
//! pure `domain::averaging_down` calculation runs entirely in base. All Money math
//! lives in the domain (`FxRates::convert` included); this only snapshots the live
//! rates and reports the conversion it applied.

use crate::fx_rate_book::FxRateBook;
use domain::averaging_down::{self, BreakevenPlan};
use domain::money::{Currency, Money};
use domain::quantity::Quantity;
use rust_decimal::Decimal;

/// Why a plan could not be produced. `RateMissing` means the price had to be
/// converted into the base currency but no cross rate was known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakevenError {
    RateMissing,
}

/// A computed plan plus the conversion that fed it, for the UI to echo.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub plan: BreakevenPlan,
    pub current_price_base: Money,
    pub fx_rate_used: Option<Decimal>,
}

pub async fn plan(
    fx: &FxRateBook,
    avg_cost: Money,
    quantity: Quantity,
    current_price: Money,
    base_currency: Currency,
    targets_pct: &[Decimal],
) -> Result<Outcome, BreakevenError> {
    let (price_base, fx_rate_used) = if current_price.currency() == base_currency {
        (current_price, None)
    } else {
        let rates = fx.snapshot().await;
        let converted = rates
            .convert(current_price, base_currency)
            .ok_or(BreakevenError::RateMissing)?;
        // amount() > 0 is guaranteed by the caller, so this ratio is the applied rate.
        let rate = converted.amount() / current_price.amount();
        (converted, Some(rate))
    };
    let plan = averaging_down::plan(avg_cost, quantity, price_base, targets_pct);
    Ok(Outcome {
        plan,
        current_price_base: price_base,
        fx_rate_used,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn converts_native_price_to_base_and_runs_in_base() {
        // US stock: avg entered in KRW, live price in USD, base = KRW, 1 USD = 1400 KRW.
        let krw = Currency::new("KRW").unwrap();
        let usd = Currency::new("USD").unwrap();
        let fx = FxRateBook::new();
        fx.set(usd, krw, dec!(1400)).await;

        let outcome = plan(
            &fx,
            Money::new(dec!(280000), krw), // avg cost in KRW
            Quantity::new(dec!(10)).unwrap(),
            Money::new(dec!(160), usd),    // live price in USD → 224000 KRW
            krw,                            // base = KRW
            &[dec!(10)],
        )
        .await
        .unwrap();

        assert_eq!(outcome.current_price_base, Money::new(dec!(224000), krw));
        assert_eq!(outcome.fx_rate_used, Some(dec!(1400)));
        assert!(outcome.plan.is_underwater);
        assert_eq!(outcome.plan.breakeven_gap_pct, Some(dec!(25))); // 280000/224000 = 1.25
        let row = &outcome.plan.rows[0];
        assert!(row.feasible);
        assert_eq!(row.target_avg, Money::new(dec!(252000), krw)); // 280000 * 0.9
        assert_eq!(row.add_invest, Money::new(dec!(2240000), krw)); // 224000 * 10
        assert_eq!(row.new_breakeven_gap_pct, dec!(12.5));          // 252000/224000 = 1.125
    }

    #[tokio::test]
    async fn base_equals_price_currency_skips_conversion() {
        let usd = Currency::new("USD").unwrap();
        let fx = FxRateBook::new(); // no rates set
        let outcome = plan(
            &fx,
            Money::new(dec!(100), usd),
            Quantity::new(dec!(1)).unwrap(),
            Money::new(dec!(80), usd),
            usd,
            &[dec!(10)],
        )
        .await
        .unwrap();
        assert_eq!(outcome.fx_rate_used, None);
        assert_eq!(outcome.current_price_base, Money::new(dec!(80), usd));
        assert!(outcome.plan.rows[0].feasible);
    }

    #[tokio::test]
    async fn missing_cross_rate_yields_rate_missing() {
        let krw = Currency::new("KRW").unwrap();
        let usd = Currency::new("USD").unwrap();
        let fx = FxRateBook::new(); // (USD, KRW) absent
        let err = plan(
            &fx,
            Money::new(dec!(280000), krw),
            Quantity::new(dec!(10)).unwrap(),
            Money::new(dec!(160), usd),
            krw,
            &[dec!(10)],
        )
        .await
        .unwrap_err();
        assert_eq!(err, BreakevenError::RateMissing);
    }
}
