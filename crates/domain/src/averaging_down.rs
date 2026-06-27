//! Pure break-even and averaging-down ("물타기") calculations.
//!
//! Given an average cost `A`, quantity held `q`, and current price `P` — all in
//! a single **base currency** (`A` and `P` share it) — this answers:
//!   * Break-even gap — the percent `P` must rise to reach `A` (while underwater).
//!   * Averaging-down plan — for each target reduction `N%`, the extra quantity
//!     and money needed at `P` to pull the average down to `A·(1 − N/100)`.
//!
//! All figures are in the base currency. Any cross-currency conversion (e.g. a
//! live USD price into a KRW base) is the caller's responsibility — see
//! `application::breakeven::plan`. Preconditions (enforced by the caller):
//! `A > 0`, `P > 0`, `q > 0`, and `avg_cost.currency() == current_price.currency()`.

use crate::money::Money;
use crate::quantity::Quantity;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AveragingDownRow {
    pub target_pct: Decimal,
    pub target_avg: Money,
    pub add_quantity: Quantity,
    pub add_invest: Money,
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
) -> BreakevenPlan {
    let hundred = Decimal::from(100);
    let a = avg_cost.amount();
    let p = current_price.amount();
    let q = quantity.value();
    let base = avg_cost.currency();

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
            let target_avg = Money::new(t, base);
            // Feasible only when underwater and the target average `t` sits
            // strictly between the current price and the current average
            // (`p < t < a`). Requiring `t < a` routes non-positive targets
            // (`n <= 0`, where `t >= a`) to the infeasible branch instead of
            // producing a negative add-quantity.
            let feasible = is_underwater && t > p && t < a;
            if feasible {
                let x = q * (a - t) / (t - p);
                let add_quantity =
                    Quantity::new(x).expect("feasible row has positive add quantity");
                let add_invest = current_price.mul_scalar(x);
                let new_breakeven_gap_pct = (t / p - Decimal::ONE) * hundred;
                AveragingDownRow {
                    target_pct: n,
                    target_avg,
                    add_quantity,
                    add_invest,
                    new_breakeven_gap_pct,
                    feasible: true,
                }
            } else {
                AveragingDownRow {
                    target_pct: n,
                    target_avg,
                    add_quantity: Quantity::zero(),
                    add_invest: Money::new(Decimal::ZERO, base),
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
    use crate::money::Currency;
    use rust_decimal_macros::dec;

    fn krw(v: Decimal) -> Money { Money::new(v, Currency::new("KRW").unwrap()) }
    fn usd(v: Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn qty(v: Decimal) -> Quantity { Quantity::new(v).unwrap() }

    #[test]
    fn breakeven_gap_when_underwater() {
        let p = plan(krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)), &[]);
        assert!(p.is_underwater);
        assert_eq!(p.breakeven_gap_pct, Some(dec!(25)));
        assert_eq!(p.current_return_pct, None);
        assert_eq!(p.max_reduction_pct, dec!(20));
        assert!(p.rows.is_empty());
    }

    #[test]
    fn worked_example_n10_feasible() {
        let p = plan(krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)), &[dec!(10)]);
        let row = &p.rows[0];
        assert!(row.feasible);
        assert_eq!(row.target_pct, dec!(10));
        assert_eq!(row.target_avg, krw(dec!(90000)));
        assert_eq!(row.add_quantity, qty(dec!(1)));
        assert_eq!(row.add_invest, krw(dec!(80000)));
        assert_eq!(row.new_breakeven_gap_pct, dec!(12.5));
    }

    #[test]
    fn presets_feasible_until_n_max() {
        let p = plan(
            krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)),
            &[dec!(5), dec!(10), dec!(15), dec!(20), dec!(25)],
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
        let p = plan(usd(dec!(100)), qty(dec!(1)), usd(dec!(120)), &[dec!(5), dec!(10)]);
        assert!(!p.is_underwater);
        assert_eq!(p.breakeven_gap_pct, None);
        assert_eq!(p.current_return_pct, Some(dec!(20)));
        assert!(p.rows.iter().all(|r| !r.feasible));
    }

    #[test]
    fn non_positive_target_is_infeasible_not_panic() {
        // Underwater. n=0 => t==a; n<0 => t>a; neither lowers the average, and
        // neither must reach the `expect` panic in the feasible branch.
        let p = plan(
            krw(dec!(100000)), qty(dec!(1)), krw(dec!(80000)),
            &[dec!(0), dec!(-10)],
        );
        assert!(!p.rows[0].feasible); // n = 0
        assert!(!p.rows[1].feasible); // n = -10
    }
}
