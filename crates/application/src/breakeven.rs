//! Thin orchestration for the break-even / averaging-down calculator: snapshot
//! the current FX rates and run the pure `domain::averaging_down` calculation.
//! All math lives in the domain; this only supplies the live `FxRates` snapshot
//! (mirroring how `PortfolioService::valuation` reads `FxRateBook::snapshot`).

use crate::fx_rate_book::FxRateBook;
use domain::averaging_down::{self, BreakevenPlan};
use domain::money::{Currency, Money};
use domain::quantity::Quantity;
use rust_decimal::Decimal;

pub async fn plan(
    fx: &FxRateBook,
    avg_cost: Money,
    quantity: Quantity,
    current_price: Money,
    targets_pct: &[Decimal],
    display_currency: Currency,
) -> BreakevenPlan {
    let rates = fx.snapshot().await;
    averaging_down::plan(
        avg_cost,
        quantity,
        current_price,
        targets_pct,
        &rates,
        display_currency,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn forwards_to_domain_using_fx_snapshot() {
        let krw = Currency::new("KRW").unwrap();
        let usd = Currency::new("USD").unwrap();
        let fx = FxRateBook::new();
        fx.set(krw, usd, dec!(0.00072)).await;

        let result = plan(
            &fx,
            Money::new(dec!(100000), krw),
            Quantity::new(dec!(1)).unwrap(),
            Money::new(dec!(80000), krw),
            &[dec!(10)],
            usd,
        )
        .await;

        assert!(result.is_underwater);
        assert_eq!(
            result.rows[0].add_invest_display,
            Some(Money::new(dec!(57.6), usd)) // snapshot rate applied: 80000 * 0.00072
        );
    }
}
