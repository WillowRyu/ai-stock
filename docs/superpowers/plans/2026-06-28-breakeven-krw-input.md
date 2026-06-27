# Break-Even Calculator — Base-Currency (원화) Input — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the break-even / averaging-down calculator run in a chosen base currency (네이티브 or 원화), converting the live native price to base via the existing FxRateBook so a Korean holder of a USD asset sees break-even and 물타기 figures in won.

**Architecture:** The domain `averaging_down::plan` is simplified to pure single-currency math (all FX and the old output `display_currency` removed). The single native→base conversion of the live price moves to `application::breakeven::plan`, which reads the FX snapshot it already uses, returns the converted price + applied rate, and surfaces `RateMissing` when the cross rate is unknown. The IPC command and frontend swap the output-only display toggle for one `base_currency` that drives input, calculation, and output.

**Tech Stack:** Rust (workspace crates: `domain`, `application`, `app`), `rust_decimal`, Tauri IPC; React + TypeScript + Tailwind frontend; Zustand stores.

## Global Constraints

- All Money math uses `rust_decimal::Decimal`; the domain layer stays pure (no IO). (ADR 0006, layer-boundary script.)
- Layer boundary: `domain` ← `application` ← `app`. FX snapshotting lives in `application`; `FxRates::convert` is the domain primitive it calls.
- IPC passes all decimals **as strings**.
- `BreakevenCalc.tsx` uses **hardcoded Korean** strings (the component does not use the i18n system — only `AiPanel` does). Do **not** add i18n keys; match the existing component.
- Rust tests are colocated in `#[cfg(test)] mod tests` within each module.
- Commit after every task. Work stays on branch `breakeven-krw-input`.
- Only two callers exist for the changed functions (`application::breakeven::plan` ← `app/src/ipc.rs`; `domain::averaging_down::plan` ← `application::breakeven::plan`), so no other call sites need updating.

---

## File Structure

- `crates/domain/src/averaging_down.rs` — **modify.** Simplify `plan` to single-currency; rename row field `add_invest_native` → `add_invest`; drop `add_invest_display`, `fx_rates`, `display_currency`; rewrite tests.
- `crates/application/src/breakeven.rs` — **modify.** Add `BreakevenError::RateMissing` + `Outcome`; convert the price native→base via FX snapshot; rewrite tests.
- `app/src/ipc.rs` — **modify.** New args (`price_currency`, `base_currency`), new DTO fields (`rate_missing`, `base_currency`, `current_price_base`, `fx_rate_used`), row field rename, `RateMissing` → `rate_missing: true`.
- `src/lib/ipc.ts` — **modify.** Mirror the new arg/DTO types.
- `src/components/BreakevenCalc.tsx` — **modify.** 기준 통화 selector (hidden for KRW-native assets), base-aware labels, 원화 환산 echo + rate note, `rate_missing` notice; remove the display toggle.
- `docs/progress.md` — **modify.** Append the work entry.

---

## Task 1: Domain — simplify `averaging_down::plan` to single-currency

**Files:**
- Modify: `crates/domain/src/averaging_down.rs`
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `domain::averaging_down::plan(avg_cost: Money, quantity: Quantity, current_price: Money, targets_pct: &[Decimal]) -> BreakevenPlan` where `avg_cost.currency() == current_price.currency()` (the base currency). Row type:
  `AveragingDownRow { target_pct: Decimal, target_avg: Money, add_quantity: Quantity, add_invest: Money, new_breakeven_gap_pct: Decimal, feasible: bool }`. `BreakevenPlan` unchanged.

- [ ] **Step 1: Rewrite the test module to the new single-currency API**

Replace the entire `#[cfg(test)] mod tests { ... }` block at the bottom of `crates/domain/src/averaging_down.rs` with:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail (compile error)**

Run: `cargo test -p domain averaging_down`
Expected: FAIL to compile — the tests call `plan(..)` with 4 args and read `row.add_invest`, but the current `plan` takes 6 args and the row field is `add_invest_native` (errors like `this function takes 6 arguments but 4 arguments were supplied` and `no field add_invest`).

- [ ] **Step 3: Replace everything above the test module with the simplified implementation**

Replace lines from the top of `crates/domain/src/averaging_down.rs` down to (but not including) `#[cfg(test)]` with:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p domain averaging_down`
Expected: PASS — 5 tests in `averaging_down::tests` pass.

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/averaging_down.rs
git commit -m "refactor(domain): averaging_down::plan is pure single-currency"
```

---

## Task 2: Application — `breakeven::plan` converts native→base + `RateMissing`

**Files:**
- Modify: `crates/application/src/breakeven.rs`
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `domain::averaging_down::plan(Money, Quantity, Money, &[Decimal]) -> BreakevenPlan` (Task 1); `FxRateBook::snapshot() -> FxRates`; `FxRates::convert(Money, Currency) -> Option<Money>`.
- Produces:
  - `application::breakeven::BreakevenError` (enum, `RateMissing` variant, derives `Debug, Clone, PartialEq, Eq`).
  - `application::breakeven::Outcome { plan: BreakevenPlan, current_price_base: Money, fx_rate_used: Option<Decimal> }`.
  - `async fn plan(fx: &FxRateBook, avg_cost: Money, quantity: Quantity, current_price: Money, base_currency: Currency, targets_pct: &[Decimal]) -> Result<Outcome, BreakevenError>`. `avg_cost` is in base; `current_price` is in its own currency (native for a live quote, base for a manual entry). When `current_price`'s currency differs from `base_currency`, it is converted via the FX snapshot; a missing rate yields `Err(RateMissing)`. `fx_rate_used` is `None` when no conversion happened.

- [ ] **Step 1: Replace the test module with FX-conversion cases**

Replace the entire `#[cfg(test)] mod tests { ... }` block in `crates/application/src/breakeven.rs` with:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail (compile error)**

Run: `cargo test -p application breakeven`
Expected: FAIL to compile — `Outcome`, `BreakevenError`, and the new 6-arg `plan` signature don't exist yet (the current `plan` takes `fx, avg_cost, quantity, current_price, targets_pct, display_currency` and returns `BreakevenPlan`).

- [ ] **Step 3: Replace everything above the test module with the new implementation**

Replace lines from the top of `crates/application/src/breakeven.rs` down to (but not including) `#[cfg(test)]` with:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p application breakeven`
Expected: PASS — `converts_native_price_to_base_and_runs_in_base`, `base_equals_price_currency_skips_conversion`, `missing_cross_rate_yields_rate_missing` pass.

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/breakeven.rs
git commit -m "feat(application): break-even runs in base currency, converts native price"
```

---

## Task 3: IPC — base-currency args/DTO + `rate_missing` mapping

**Files:**
- Modify: `app/src/ipc.rs` (import line 8; the breakeven args/DTO/mapping/command block, currently lines 124–213)

**Interfaces:**
- Consumes: `application::breakeven::{plan, Outcome, BreakevenError}` (Task 2).
- Produces (Tauri command `breakeven_plan`):
  - Args `BreakevenPlanArgs { avg_cost_amount, quantity, current_price_amount, price_currency, base_currency, targets_pct: Vec<String> }`.
  - DTO `BreakevenPlanDto { rate_missing: bool, base_currency: String, current_price_base: Option<String>, fx_rate_used: Option<String>, is_underwater: bool, breakeven_gap_pct: Option<String>, current_return_pct: Option<String>, max_reduction_pct: String, rows: Vec<AveragingDownRowDto> }`.
  - Row DTO `AveragingDownRowDto { target_pct, target_avg, add_quantity, add_invest, new_breakeven_gap_pct, feasible }`.

- [ ] **Step 1: Drop the now-unused `BreakevenPlan` import**

In `app/src/ipc.rs`, change the import line (line 8):

```rust
    asset::AssetKind, averaging_down::BreakevenPlan, holding::Holding,
```

to:

```rust
    asset::AssetKind, holding::Holding,
```

- [ ] **Step 2: Replace the breakeven args/DTO/mapping/command block**

Replace the block in `app/src/ipc.rs` that begins at `#[derive(Deserialize)]\npub struct BreakevenPlanArgs {` and ends at the closing `}` of the `breakeven_plan` command (currently lines 124–213) with:

```rust
#[derive(Deserialize)]
pub struct BreakevenPlanArgs {
    pub avg_cost_amount: String,
    pub quantity: String,
    pub current_price_amount: String,
    pub price_currency: String,
    pub base_currency: String,
    pub targets_pct: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct AveragingDownRowDto {
    pub target_pct: String,
    pub target_avg: String,
    pub add_quantity: String,
    pub add_invest: String,
    pub new_breakeven_gap_pct: String,
    pub feasible: bool,
}

#[derive(Serialize, Clone)]
pub struct BreakevenPlanDto {
    pub rate_missing: bool,
    pub base_currency: String,
    pub current_price_base: Option<String>,
    pub fx_rate_used: Option<String>,
    pub is_underwater: bool,
    pub breakeven_gap_pct: Option<String>,
    pub current_return_pct: Option<String>,
    pub max_reduction_pct: String,
    pub rows: Vec<AveragingDownRowDto>,
}

fn breakeven_outcome_to_dto(outcome: breakeven::Outcome, base: Currency) -> BreakevenPlanDto {
    let plan = outcome.plan;
    BreakevenPlanDto {
        rate_missing: false,
        base_currency: base.as_str().to_string(),
        current_price_base: Some(outcome.current_price_base.amount().to_string()),
        fx_rate_used: outcome.fx_rate_used.map(|d| d.to_string()),
        is_underwater: plan.is_underwater,
        breakeven_gap_pct: plan.breakeven_gap_pct.map(|d| d.to_string()),
        current_return_pct: plan.current_return_pct.map(|d| d.to_string()),
        max_reduction_pct: plan.max_reduction_pct.to_string(),
        rows: plan
            .rows
            .iter()
            .map(|r| AveragingDownRowDto {
                target_pct: r.target_pct.to_string(),
                target_avg: r.target_avg.amount().to_string(),
                add_quantity: r.add_quantity.value().to_string(),
                add_invest: r.add_invest.amount().to_string(),
                new_breakeven_gap_pct: r.new_breakeven_gap_pct.to_string(),
                feasible: r.feasible,
            })
            .collect(),
    }
}

#[tauri::command]
pub async fn breakeven_plan(
    state: State<'_, AppState>,
    args: BreakevenPlanArgs,
) -> Result<BreakevenPlanDto, String> {
    let base = Currency::new(&args.base_currency).map_err(|e| format!("{e:?}"))?;
    let price_ccy = Currency::new(&args.price_currency).map_err(|e| format!("{e:?}"))?;
    let a = Decimal::from_str(&args.avg_cost_amount).map_err(|e| e.to_string())?;
    let p = Decimal::from_str(&args.current_price_amount).map_err(|e| e.to_string())?;
    let q = Decimal::from_str(&args.quantity).map_err(|e| e.to_string())?;
    if a <= Decimal::ZERO || p <= Decimal::ZERO || q <= Decimal::ZERO {
        return Err("avg_cost, quantity, and current_price must all be greater than 0".into());
    }

    let mut targets = Vec::with_capacity(args.targets_pct.len());
    for t in &args.targets_pct {
        targets.push(Decimal::from_str(t).map_err(|e| e.to_string())?);
    }

    let avg_cost = Money::new(a, base);
    let current_price = Money::new(p, price_ccy);
    let quantity = Quantity::new(q).map_err(|e| format!("{e:?}"))?;

    match breakeven::plan(&state.fx, avg_cost, quantity, current_price, base, &targets).await {
        Ok(outcome) => Ok(breakeven_outcome_to_dto(outcome, base)),
        Err(breakeven::BreakevenError::RateMissing) => Ok(BreakevenPlanDto {
            rate_missing: true,
            base_currency: base.as_str().to_string(),
            current_price_base: None,
            fx_rate_used: None,
            is_underwater: false,
            breakeven_gap_pct: None,
            current_return_pct: None,
            max_reduction_pct: "0".to_string(),
            rows: vec![],
        }),
    }
}
```

- [ ] **Step 3: Build and run the whole backend suite**

Run: `cargo test --workspace`
Expected: PASS — all crates compile (including `app`) and every test passes, including the rewritten `domain::averaging_down` and `application::breakeven` tests.

- [ ] **Step 4: Commit**

```bash
git add app/src/ipc.rs
git commit -m "feat(app): breakeven_plan takes base_currency, returns rate_missing + converted price"
```

---

## Task 4: Frontend — `ipc.ts` types + `BreakevenCalc.tsx` base-currency UI

**Files:**
- Modify: `src/lib/ipc.ts` (the three breakeven interfaces, lines 42–69)
- Modify: `src/components/BreakevenCalc.tsx` (full replacement)

**Interfaces:**
- Consumes: the IPC contract from Task 3 (`breakeven_plan` args + `BreakevenPlanDto`).
- These two files must change together — updating `ipc.ts` alone leaves `BreakevenCalc.tsx` referencing removed fields, so typecheck only passes once both land.

- [ ] **Step 1: Update the breakeven types in `src/lib/ipc.ts`**

Replace the three interfaces `BreakevenPlanArgs`, `AveragingDownRowDto`, and `BreakevenPlanDto` (lines 42–69) with:

```ts
export interface BreakevenPlanArgs {
  avg_cost_amount: string;
  quantity: string;
  current_price_amount: string;
  price_currency: string;
  base_currency: string;
  targets_pct: string[];
}

export interface AveragingDownRowDto {
  target_pct: string;
  target_avg: string;
  add_quantity: string;
  add_invest: string;
  new_breakeven_gap_pct: string;
  feasible: boolean;
}

export interface BreakevenPlanDto {
  rate_missing: boolean;
  base_currency: string;
  current_price_base: string | null;
  fx_rate_used: string | null;
  is_underwater: boolean;
  breakeven_gap_pct: string | null;
  current_return_pct: string | null;
  max_reduction_pct: string;
  rows: AveragingDownRowDto[];
}
```

(The `ipc.breakevenPlan` wrapper on line ~89 is unchanged — it already forwards `args`.)

- [ ] **Step 2: Replace `src/components/BreakevenCalc.tsx` in full**

Overwrite `src/components/BreakevenCalc.tsx` with:

```tsx
import { useEffect, useMemo, useState } from "react";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
import { ipc, type BreakevenPlanDto, type SymbolDto } from "../lib/ipc";
import { formatMoney } from "../lib/format";
import { Select } from "./Select";

// Native currency of an asset (mirrors PortfolioPanel's private helper; kept
// local to avoid restructuring an unrelated module).
function defaultCostCurrency(s: SymbolDto): string {
  if (s.quote_currency) return s.quote_currency;
  switch (s.kind) {
    case "us": return "USD";
    case "kr": return "KRW";
    case "fx":
    case "com":
    default:
      return "USD";
  }
}

function symbolLabel(s: SymbolDto): string {
  return s.quote_currency ? `${s.ticker} / ${s.quote_currency}` : s.ticker;
}

function fmtPct(s: string): string {
  const n = Number(s);
  if (!Number.isFinite(n)) return s;
  return n.toLocaleString(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

function fmtQty(s: string): string {
  const n = Number(s);
  if (!Number.isFinite(n)) return s;
  return n.toLocaleString(undefined, { maximumFractionDigits: 6 });
}

function fmtRate(s: string): string {
  const n = Number(s);
  if (!Number.isFinite(n)) return s;
  return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

const PRESET_TARGETS = ["5", "10", "15"];

export function BreakevenCalc({ onClose }: { onClose(): void }) {
  const watchlist = useWatchlistStore((s) => s.symbols);
  const loadWatchlist = useWatchlistStore((s) => s.load);
  const quotes = useQuotesStore((s) => s.bySymbol);

  useEffect(() => {
    if (watchlist.length === 0) loadWatchlist();
  }, [watchlist.length, loadWatchlist]);

  const [selectedKey, setSelectedKey] = useState<string>("");
  const [avgInput, setAvgInput] = useState("");
  const [qtyInput, setQtyInput] = useState("");
  const [manualPrice, setManualPrice] = useState("");
  const [customPct, setCustomPct] = useState("");
  const [baseMode, setBaseMode] = useState<"native" | "KRW">("native");
  const [plan, setPlan] = useState<BreakevenPlanDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selectedSymbol = useMemo<SymbolDto | undefined>(
    () => watchlist.find((s) => quoteKey(s) === selectedKey),
    [watchlist, selectedKey],
  );
  const liveQuote = selectedSymbol ? quotes[quoteKey(selectedSymbol)] : undefined;
  const nativeCcy = selectedSymbol ? defaultCostCurrency(selectedSymbol) : "USD";
  // When the asset is already KRW-denominated there is no base-currency choice.
  const baseChoiceAvailable = nativeCcy !== "KRW";
  const baseCcy = baseMode === "KRW" && baseChoiceAvailable ? "KRW" : nativeCcy;
  // A live quote drives the price field (in native ccy); otherwise the user types
  // it directly in the base currency.
  const effectivePrice = liveQuote?.price ?? manualPrice;
  const priceCcy = liveQuote ? nativeCcy : baseCcy;

  const targets = useMemo(() => {
    const list = [...PRESET_TARGETS];
    if (customPct && Number(customPct) > 0 && !list.includes(customPct)) list.push(customPct);
    return list;
  }, [customPct]);

  // Debounced recompute; also re-fires whenever the live price ticks.
  useEffect(() => {
    const a = avgInput.trim();
    const q = qtyInput.trim();
    const p = effectivePrice.trim();
    if (!a || !q || !p || Number(a) <= 0 || Number(q) <= 0 || Number(p) <= 0) {
      setPlan(null);
      setError(null);
      return;
    }
    const handle = setTimeout(() => {
      ipc
        .breakevenPlan({
          avg_cost_amount: a,
          quantity: q,
          current_price_amount: p,
          price_currency: priceCcy,
          base_currency: baseCcy,
          targets_pct: targets,
        })
        .then((result) => {
          setPlan(result);
          setError(null);
        })
        .catch((e) => {
          setError(String(e));
          setPlan(null);
        });
    }, 150);
    return () => clearTimeout(handle);
  }, [avgInput, qtyInput, effectivePrice, priceCcy, baseCcy, targets]);

  const showKrwEcho =
    baseCcy === "KRW" &&
    !!liveQuote &&
    !!plan &&
    !plan.rate_missing &&
    plan.current_price_base != null &&
    plan.fx_rate_used != null;

  return (
    <div className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="glass-panel rounded-lg p-5 w-[34rem] max-h-[90vh] overflow-y-auto space-y-4"
      >
        <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">🧮 본전·물타기 계산기</h3>

        <div className="block text-sm">
          <span className="text-slate-700 dark:text-slate-300">종목 (선택)</span>
          <Select
            value={selectedKey}
            options={[
              { value: "", label: "직접 입력" },
              ...watchlist.map((s) => ({ value: quoteKey(s), label: symbolLabel(s) })),
            ]}
            onChange={setSelectedKey}
            className="mt-1"
          />
        </div>

        <div className="grid grid-cols-3 gap-3">
          <label className="block text-sm">
            <span className="text-slate-700 dark:text-slate-300">평단 ({baseCcy})</span>
            <input
              value={avgInput}
              onChange={(e) => setAvgInput(e.target.value)}
              inputMode="decimal"
              placeholder="평균 매입가"
              className="mt-1 w-full glass-inset rounded px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
            />
          </label>
          <label className="block text-sm">
            <span className="text-slate-700 dark:text-slate-300">보유수량</span>
            <input
              value={qtyInput}
              onChange={(e) => setQtyInput(e.target.value)}
              inputMode="decimal"
              placeholder="예: 0.5"
              className="mt-1 w-full glass-inset rounded px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
            />
          </label>
          <label className="block text-sm">
            <span className="text-slate-700 dark:text-slate-300">현재가 ({priceCcy})</span>
            <input
              value={effectivePrice}
              onChange={(e) => setManualPrice(e.target.value)}
              readOnly={!!liveQuote}
              inputMode="decimal"
              placeholder="현재가"
              className={
                "mt-1 w-full glass-inset rounded px-3 py-2.5 text-base text-slate-900 dark:text-slate-100 " +
                (liveQuote ? "opacity-70" : "")
              }
            />
          </label>
        </div>
        {liveQuote && (
          <div className="text-xs text-emerald-600 dark:text-emerald-400">● 실시간 현재가 사용 중</div>
        )}
        {showKrwEcho && plan && (
          <div className="text-xs text-slate-500 dark:text-slate-400">
            원화 환산 현재가 ≈ {formatMoney(plan.current_price_base ?? "")} KRW · 환율 1 {nativeCcy} = {fmtRate(plan.fx_rate_used ?? "")} KRW
          </div>
        )}

        <div className="flex items-center gap-2 text-sm">
          {baseChoiceAvailable && (
            <>
              <span className="text-slate-700 dark:text-slate-300">기준 통화</span>
              {(["native", "KRW"] as const).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => setBaseMode(m)}
                  className={
                    "px-2 py-1 rounded text-xs " +
                    (baseMode === m
                      ? "bg-emerald-600/15 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400"
                      : "btn-secondary")
                  }
                >
                  {m === "native" ? `네이티브 (${nativeCcy})` : "원화 (KRW)"}
                </button>
              ))}
            </>
          )}
          <label className="ml-auto flex items-center gap-1">
            <span className="text-slate-500 dark:text-slate-400 text-xs">직접 %</span>
            <input
              value={customPct}
              onChange={(e) => setCustomPct(e.target.value)}
              inputMode="decimal"
              placeholder="예: 8"
              className="w-16 glass-inset rounded px-2 py-1 text-xs text-slate-900 dark:text-slate-100"
            />
          </label>
        </div>

        {error && <div className="text-rose-600 dark:text-rose-400 text-xs">{error}</div>}

        {plan?.rate_missing ? (
          <p className="text-sm text-amber-600 dark:text-amber-400">
            원화 환율을 아직 불러오지 못했습니다. 네이티브 통화로 보거나 잠시 후 다시 시도하세요.
          </p>
        ) : !plan ? (
          <p className="text-sm text-slate-500 dark:text-slate-400">
            평단·보유수량·현재가를 입력하면 본전까지의 상승률과 물타기 시나리오가 계산됩니다.
          </p>
        ) : (
          <>
            <div className="glass-inset rounded p-3">
              <div className="text-xs text-slate-500 dark:text-slate-400">본전까지</div>
              {plan.is_underwater && plan.breakeven_gap_pct ? (
                <div className="text-base text-slate-900 dark:text-slate-100">
                  {baseCcy === "KRW" ? "원화 기준 " : ""}현재가가{" "}
                  <span className="font-semibold text-rose-600 dark:text-rose-400">
                    +{fmtPct(plan.breakeven_gap_pct)}%
                  </span>{" "}
                  오르면 본전
                </div>
              ) : (
                <div className="text-base text-emerald-700 dark:text-emerald-400">
                  이미 본전 이상 (+{fmtPct(plan.current_return_pct ?? "0")}%)
                </div>
              )}
            </div>

            {!plan.is_underwater ? (
              <p className="text-sm text-slate-500 dark:text-slate-400">
                현재가가 평단 이상이라 추가 매수로 평단을 낮출 수 없습니다.
              </p>
            ) : (
              <div className="space-y-1">
                <div className="text-xs text-slate-500 dark:text-slate-400">
                  물타기 시나리오 (최대 −{fmtPct(plan.max_reduction_pct)}%까지 가능)
                </div>
                <table className="w-full text-xs">
                  <thead className="text-slate-500 dark:text-slate-400">
                    <tr className="text-left">
                      <th className="py-1">평단 낮춤</th>
                      <th>목표 평단</th>
                      <th>추가 매수</th>
                      <th>추가 투자금</th>
                      <th>새 본전까지</th>
                    </tr>
                  </thead>
                  <tbody>
                    {plan.rows.map((r, i) => (
                      <tr key={i} className="border-t border-slate-300/40 dark:border-white/10">
                        <td className="py-1 tabular-nums">−{fmtPct(r.target_pct)}%</td>
                        {r.feasible ? (
                          <>
                            <td className="tabular-nums">{formatMoney(r.target_avg)}</td>
                            <td className="tabular-nums">{fmtQty(r.add_quantity)}</td>
                            <td className="tabular-nums">
                              {formatMoney(r.add_invest)} {baseCcy}
                            </td>
                            <td className="tabular-nums text-rose-600 dark:text-rose-400">
                              +{fmtPct(r.new_breakeven_gap_pct)}%
                            </td>
                          </>
                        ) : (
                          <td colSpan={4} className="text-slate-400 dark:text-slate-500">
                            불가능 — 현재가가 더 낮아야 함
                          </td>
                        )}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )}

        <div className="flex justify-end">
          <button type="button" onClick={onClose} className="btn-secondary text-sm">닫기</button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Typecheck**

Run: `npm run typecheck`
Expected: PASS (exit 0, no errors).

- [ ] **Step 4: Build**

Run: `npm run build`
Expected: PASS (`tsc -b` then `vite build` both succeed).

- [ ] **Step 5: Lint**

Run: `npm run lint`
Expected: 0 errors. (Pre-existing warnings in `e2e/` are acceptable; introduce no new errors in `src/`.)

- [ ] **Step 6: Commit**

```bash
git add src/lib/ipc.ts src/components/BreakevenCalc.tsx
git commit -m "feat(web): base-currency (원화) selector in break-even calculator"
```

---

## Task 5: Progress log + full-suite verification

**Files:**
- Modify: `docs/progress.md`

- [ ] **Step 1: Append the progress entry**

Add to the end of `docs/progress.md`:

```markdown
## 2026-06-28 — Break-even calculator: 원화 기준(base-currency) input

Spec: `docs/superpowers/specs/2026-06-28-breakeven-krw-input-design.md`.
Plan: `docs/superpowers/plans/2026-06-28-breakeven-krw-input.md`.

- The break-even / averaging-down calculator now runs in a chosen **base currency**
  (네이티브 or 원화). A Korean holder of a USD asset enters 평단 in KRW; the live
  USD price is converted to KRW via the existing FxRateBook, so the break-even gap
  and every 물타기 figure are in won and move with the exchange rate.
- `domain::averaging_down::plan` simplified to pure single-currency math (FX and
  the output `display_currency` removed). The native→base price conversion moved to
  `application::breakeven::plan`, which returns the converted price + applied rate
  and surfaces `BreakevenError::RateMissing` when the cross rate is unknown.
- IPC `breakeven_plan`: args take `price_currency` + `base_currency` (replacing
  `native_currency` + `display_currency`); DTO gains `rate_missing`,
  `base_currency`, `current_price_base`, `fx_rate_used` and drops the per-row
  display fields (`add_invest_native` → `add_invest`).
- Frontend: 기준 통화 selector (hidden for KRW-native assets) replaces the old
  표시 통화 toggle; shows 원화 환산 현재가 + 환율, and a graceful "환율 없음" notice.
- New tests: domain `breakeven_gap_when_underwater` / `worked_example_n10_feasible`
  / `presets_feasible_until_n_max` / `not_underwater_reports_current_return` /
  `non_positive_target_is_infeasible_not_panic` (single-currency); application
  `converts_native_price_to_base_and_runs_in_base` /
  `base_equals_price_currency_skips_conversion` / `missing_cross_rate_yields_rate_missing`.
```

- [ ] **Step 2: Run the full backend suite**

Run: `cargo test --workspace`
Expected: PASS — 0 failed.

- [ ] **Step 3: Run frontend typecheck + build**

Run: `npm run typecheck && npm run build`
Expected: both PASS.

- [ ] **Step 4: Commit**

```bash
git add docs/progress.md
git commit -m "docs(progress): break-even base-currency (원화) input"
```

---

## Verification Checklist (after all tasks)

- [ ] `cargo test --workspace` — green.
- [ ] `npm run typecheck` — clean.
- [ ] `npm run build` — succeeds.
- [ ] `npm run lint` — no new `src/` errors.
- [ ] Manual (running app, `npm run tauri dev`): pick a US watchlist symbol; toggle 기준 통화 → 원화; confirm 평단 label shows `(KRW)`, the 원화 환산 현재가 + 환율 line appears, and the break-even gap / 물타기 figures are in won and re-tick. Toggle back to 네이티브 → behaves as before. Pick a KR symbol → 기준 통화 selector is hidden. With FX not yet loaded (or an EUR asset + 원화) → "환율 없음" notice shows.

## Follow-up (not part of this plan)

Merging `breakeven-krw-input` → `main` is handled separately via the
superpowers:finishing-a-development-branch skill once the checklist passes.
