//! Pure break-even and averaging-down ("물타기") calculations.
//!
//! Given an average cost `A`, quantity held `q`, and current price `P` (with
//! `A` and `P` sharing the asset's native currency), this answers:
//!   * Break-even gap — the percent `P` must rise to reach `A` (while underwater).
//!   * Averaging-down plan — for each target reduction `N%`, the extra quantity
//!     and money needed at `P` to pull the average down to `A·(1 − N/100)`.
//!
//! Preconditions (enforced by the caller): `A > 0`, `P > 0`, `q > 0`, and
//! `avg_cost.currency() == current_price.currency()`.

use crate::fx::FxRates;
use crate::money::{Currency, Money};
use crate::quantity::Quantity;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AveragingDownRow {
    pub target_pct: Decimal,
    pub target_avg: Money,
    pub add_quantity: Quantity,
    pub add_invest_native: Money,
    pub add_invest_display: Option<Money>,
    pub new_breakeven_gap_pct: Decimal,
    pub feasible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakevenPlan {
    pub is_underwater: bool,
    pub breakeven_gap_pct: Option<Decimal>,
    pub current_return_pct: Option<Decimal>,
    pub max_reduction_pct: Decimal,
    pub rows: Vec<AveragingDownRow>,
}

pub fn plan(
    avg_cost: Money,
    quantity: Quantity,
    current_price: Money,
    targets_pct: &[Decimal],
    fx_rates: &FxRates,
    display_currency: Currency,
) -> BreakevenPlan {
    let hundred = Decimal::from(100);
    let a = avg_cost.amount();
    let p = current_price.amount();
    let q = quantity.value();
    let native = avg_cost.currency();

    let is_underwater = p < a;
    let breakeven_gap_pct = if is_underwater {
        Some((a / p - Decimal::ONE) * hundred)
    } else {
        None
    };
    let current_return_pct = if is_underwater {
        None
    } else {
        Some((p / a - Decimal::ONE) * hundred)
    };
    let max_reduction_pct = (a - p) / a * hundred;

    let rows = targets_pct
        .iter()
        .map(|&n| {
            let t = a * (Decimal::ONE - n / hundred);
            let target_avg = Money::new(t, native);
            // Feasible only when underwater and the target average `t` sits
            // strictly between the current price and the current average
            // (`p < t < a`). Requiring `t < a` routes non-positive targets
            // (`n <= 0`, where `t >= a` and averaging down can't lower the
            // average) to the infeasible branch instead of producing a
            // negative add-quantity.
            let feasible = is_underwater && t > p && t < a;
            if feasible {
                let x = q * (a - t) / (t - p);
                let add_quantity =
                    Quantity::new(x).expect("feasible row has positive add quantity");
                let add_invest_native = current_price.mul_scalar(x);
                let add_invest_display = if display_currency == native {
                    None
                } else {
                    fx_rates.convert(add_invest_native, display_currency)
                };
                let new_breakeven_gap_pct = (t / p - Decimal::ONE) * hundred;
                AveragingDownRow {
                    target_pct: n,
                    target_avg,
                    add_quantity,
                    add_invest_native,
                    add_invest_display,
                    new_breakeven_gap_pct,
                    feasible: true,
                }
            } else {
                AveragingDownRow {
                    target_pct: n,
                    target_avg,
                    add_quantity: Quantity::zero(),
                    add_invest_native: Money::new(Decimal::ZERO, native),
                    add_invest_display: None,
                    new_breakeven_gap_pct: Decimal::ZERO,
                    feasible: false,
                }
            }
        })
        .collect();

    BreakevenPlan {
        is_underwater,
        breakeven_gap_pct,
        current_return_pct,
        max_reduction_pct,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn krw(v: Decimal) -> Money { Money::new(v, Currency::new("KRW").unwrap()) }
    fn usd(v: Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn ccy(s: &str) -> Currency { Currency::new(s).unwrap() }
    fn qty(v: Decimal) -> Quantity { Quantity::new(v).unwrap() }

    #[test]
    fn breakeven_gap_when_underwater() {
        let p = plan(krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)), &[], &FxRates::new(), ccy("KRW"));
        assert!(p.is_underwater);
        assert_eq!(p.breakeven_gap_pct, Some(dec!(25)));
        assert_eq!(p.current_return_pct, None);
        assert_eq!(p.max_reduction_pct, dec!(20));
        assert!(p.rows.is_empty());
    }

    #[test]
    fn worked_example_n10_feasible() {
        let p = plan(krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)), &[dec!(10)], &FxRates::new(), ccy("KRW"));
        let row = &p.rows[0];
        assert!(row.feasible);
        assert_eq!(row.target_pct, dec!(10));
        assert_eq!(row.target_avg, krw(dec!(90000)));
        assert_eq!(row.add_quantity, qty(dec!(1)));
        assert_eq!(row.add_invest_native, krw(dec!(80000)));
        assert_eq!(row.new_breakeven_gap_pct, dec!(12.5));
        assert_eq!(row.add_invest_display, None); // display == native → no conversion
    }

    #[test]
    fn presets_feasible_until_n_max() {
        let p = plan(
            krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)),
            &[dec!(5), dec!(10), dec!(15), dec!(20), dec!(25)],
            &FxRates::new(), ccy("KRW"),
        );
        assert_eq!(p.rows.len(), 5);
        assert!(p.rows[0].feasible);   // 5%
        assert!(p.rows[1].feasible);   // 10%
        assert!(p.rows[2].feasible);   // 15%
        assert!(!p.rows[3].feasible);  // 20% == N_max → T == P
        assert!(!p.rows[4].feasible);  // 25% > N_max
    }

    #[test]
    fn not_underwater_reports_current_return() {
        let p = plan(usd(dec!(100)), qty(dec!(1)), usd(dec!(120)), &[dec!(5), dec!(10)], &FxRates::new(), ccy("USD"));
        assert!(!p.is_underwater);
        assert_eq!(p.breakeven_gap_pct, None);
        assert_eq!(p.current_return_pct, Some(dec!(20)));
        assert!(p.rows.iter().all(|r| !r.feasible));
    }

    #[test]
    fn fx_conversion_present_when_rate_known() {
        let mut fx = FxRates::new();
        fx.set(ccy("KRW"), ccy("USD"), dec!(0.00072));
        let p = plan(krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)), &[dec!(10)], &fx, ccy("USD"));
        let row = &p.rows[0];
        assert_eq!(row.add_invest_native, krw(dec!(80000)));
        assert_eq!(row.add_invest_display, Some(usd(dec!(57.6)))); // 80000 * 0.00072
    }

    #[test]
    fn fx_missing_rate_yields_none() {
        let p = plan(krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)), &[dec!(10)], &FxRates::new(), ccy("USD"));
        assert_eq!(p.rows[0].add_invest_display, None);
    }

    #[test]
    fn display_equals_native_skips_conversion() {
        let p = plan(usd(dec!(100)), qty(dec!(1)), usd(dec!(80)), &[dec!(10)], &FxRates::new(), ccy("USD"));
        assert!(p.rows[0].feasible);
        assert_eq!(p.rows[0].add_invest_display, None);
    }

    #[test]
    fn non_positive_target_is_infeasible_not_panic() {
        // Underwater. A non-positive target must NOT reach the `expect` panic and
        // must be reported infeasible (n=0 => t==a; n<0 => t>a; neither lowers the avg).
        let p = plan(
            krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)),
            &[dec!(0), dec!(-10)],
            &FxRates::new(), ccy("KRW"),
        );
        assert!(!p.rows[0].feasible); // n = 0
        assert!(!p.rows[1].feasible); // n = -10 (previously panicked)
    }
}
