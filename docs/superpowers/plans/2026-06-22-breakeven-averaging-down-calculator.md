# Break-Even & Averaging-Down Calculator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a standalone "본전·물타기 계산기" opened from the Portfolio panel that, given an average cost, quantity, and live current price, computes the break-even gap and an averaging-down table (extra quantity + investment per target reduction, with a currency toggle), recomputing live as quotes tick.

**Architecture:** Pure Decimal math lives in a new `domain::averaging_down` module (unit-tested with `cargo test`). A thin async `application::breakeven::plan` snapshots the current `FxRates` and forwards to the domain (mirroring `indicator_service` free functions). One Tauri command `breakeven_plan` does string↔domain translation and DTO mapping. A new React modal `BreakevenCalc.tsx` reads the live price from `quotesStore`, debounce-invokes the command, and renders the results; the Portfolio panel header gains a button to open it.

**Tech Stack:** Rust (`rust_decimal`, `thiserror`, `serde`, `tokio`), Tauri v2 IPC, React + TypeScript + Tailwind, Zustand stores.

## Global Constraints

These apply to **every** task below:

- **Money math is Decimal-only, in the domain.** Use `rust_decimal::Decimal`, `Money`, `Quantity`, `Currency`, `FxRates` — never `f64`. In non-test domain/application code, `rust_decimal_macros::dec!` is **not** available (it is a dev-dependency only); use `Decimal::from(100)`, `Decimal::ONE`, `Decimal::ZERO`. `dec!` **is** available in `#[cfg(test)]` modules.
- **`A`, `P` share the asset's native currency**; `A`, `P`, `q` are all `> 0` (validated in the IPC command — the domain documents this as a precondition and must not be called with non-positive values, which would panic on Decimal division).
- **Decimal equality normalizes scale** in `rust_decimal` (`dec!(57.6) == dec!(57.60000)`), so assert against the mathematically simplest literal.
- **IPC error type is `String`.** Parse/convert errors use `.map_err(|e| e.to_string())` (or `format!("{e:?}")` for `MoneyError`/`QuantityError`), matching existing commands.
- **DTO convention:** `#[derive(Serialize, Clone)]`, every Decimal/Money amount is a `String`, optional money is `Option<String>`, currency codes are separate `String` fields. Input DTOs derive `Deserialize`. JS passes **snake_case** keys matching DTO field names exactly (see existing `HoldingDto`).
- **Frontend strings are hardcoded Korean**, matching every existing component (e.g. `AddHoldingDialog`). This codebase has **no active i18n machinery** — `src/i18n/{ko,en}.json` hold 3 vestigial keys consumed by nothing and there is no `t()` function. **Do not** add i18n keys or wire a translation layer; that supersedes the spec's i18n step, which assumed an i18n system that does not exist.
- **Commit after every task** with a Conventional Commit message.

## Deviations from the spec (deliberate)

1. **"불러오기" (prefill 평단+수량 from a saved Holding) is OUT OF SCOPE** for this plan (user decision, 2026-06-22). The current `HoldingValuationDto` exposes only `cost_basis` (total), not raw avg/quantity, and wiring that data would exceed the spec's stated "one IPC command" scope. 종목 selection still auto-fills **현재가** live from `quotesStore`; 평단 and 수량 are manual inputs. Prefill can be a fast follow-up.
2. **i18n** — hardcoded Korean (see Global Constraints).

## File Structure

| File | New/Modify | Responsibility |
|---|---|---|
| `crates/domain/src/averaging_down.rs` | **New** | Pure break-even + averaging-down formulas, types, unit tests. |
| `crates/domain/src/lib.rs` | Modify | Register `pub mod averaging_down;`. |
| `crates/application/src/breakeven.rs` | **New** | Thin async wrapper: snapshot `FxRates`, call domain. One `#[tokio::test]`. |
| `crates/application/src/lib.rs` | Modify | Register `pub mod breakeven;`. |
| `app/src/ipc.rs` | Modify | `breakeven_plan` command + input/output DTOs + mapping fn. |
| `app/src/main.rs` | Modify | Register `ipc::breakeven_plan` in `generate_handler!`. |
| `src/lib/ipc.ts` | Modify | `BreakevenPlanArgs` / `AveragingDownRowDto` / `BreakevenPlanDto` interfaces + `breakevenPlan` wrapper. |
| `src/components/BreakevenCalc.tsx` | **New** | Modal: inputs, live price, currency toggle, results table. |
| `src/components/PortfolioPanel.tsx` | Modify | Header "🧮 본전 계산" button + open/close state + render modal. |

---

## Task 1: Domain — `averaging_down` module

**Files:**
- Create: `crates/domain/src/averaging_down.rs`
- Modify: `crates/domain/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests` in `crates/domain/src/averaging_down.rs`

**Interfaces:**
- Consumes: `crate::fx::FxRates` (`new`, `set`, `convert(money, target) -> Option<Money>`); `crate::money::{Currency, Money}` (`Money::new`, `amount`, `currency`, `mul_scalar`); `crate::quantity::Quantity` (`new`, `value`, `zero`).
- Produces (later tasks rely on these exact names/types):
  ```rust
  pub struct AveragingDownRow {
      pub target_pct: Decimal,
      pub target_avg: Money,
      pub add_quantity: Quantity,
      pub add_invest_native: Money,
      pub add_invest_display: Option<Money>,
      pub new_breakeven_gap_pct: Decimal,
      pub feasible: bool,
  }
  pub struct BreakevenPlan {
      pub is_underwater: bool,
      pub breakeven_gap_pct: Option<Decimal>,
      pub current_return_pct: Option<Decimal>,
      pub max_reduction_pct: Decimal,
      pub rows: Vec<AveragingDownRow>,
  }
  pub fn plan(
      avg_cost: Money, quantity: Quantity, current_price: Money,
      targets_pct: &[Decimal], fx_rates: &FxRates, display_currency: Currency,
  ) -> BreakevenPlan;
  ```

- [ ] **Step 1: Create the module with types + `plan` signature (body `todo!()`), and register it**

Create `crates/domain/src/averaging_down.rs`:

```rust
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
    todo!()
}
```

Add to `crates/domain/src/lib.rs`, in alphabetical order (right after `pub mod asset;`):

```rust
pub mod averaging_down;
```

- [ ] **Step 2: Confirm it compiles**

Run: `cargo build -p domain`
Expected: builds (warnings about unused params are fine).

- [ ] **Step 3: Write the failing tests**

Append to `crates/domain/src/averaging_down.rs`:

```rust
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
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p domain averaging_down`
Expected: FAIL — all tests panic at `not yet implemented` (`todo!()`).

- [ ] **Step 5: Implement `plan`**

Replace the `todo!()` body in `crates/domain/src/averaging_down.rs` with:

```rust
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
            let feasible = is_underwater && t > p;
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
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p domain averaging_down`
Expected: PASS — 7 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/domain/src/averaging_down.rs crates/domain/src/lib.rs
git commit -m "feat(domain): break-even & averaging-down calculator"
```

---

## Task 2: Application — thin `breakeven::plan` wrapper

**Files:**
- Create: `crates/application/src/breakeven.rs`
- Modify: `crates/application/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests` in `crates/application/src/breakeven.rs`

**Interfaces:**
- Consumes: `crate::fx_rate_book::FxRateBook` (`snapshot(&self) -> FxRates`, async; `set(from, to, rate)`, async); `domain::averaging_down::{plan, BreakevenPlan}`.
- Produces:
  ```rust
  pub async fn plan(
      fx: &FxRateBook, avg_cost: Money, quantity: Quantity, current_price: Money,
      targets_pct: &[Decimal], display_currency: Currency,
  ) -> domain::averaging_down::BreakevenPlan;
  ```

- [ ] **Step 1: Create the wrapper and register the module**

Create `crates/application/src/breakeven.rs`:

```rust
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
```

Add to `crates/application/src/lib.rs`, in alphabetical order (right after `pub mod alert_service;`):

```rust
pub mod breakeven;
```

- [ ] **Step 2: Write the failing test**

Append to `crates/application/src/breakeven.rs`:

```rust
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
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p application breakeven`
Expected: PASS — `forwards_to_domain_using_fx_snapshot` passes (the wrapper compiles and the FX snapshot path works).

- [ ] **Step 4: Commit**

```bash
git add crates/application/src/breakeven.rs crates/application/src/lib.rs
git commit -m "feat(application): thin break-even plan wrapper over FxRates snapshot"
```

---

## Task 3: IPC command — `breakeven_plan`

**Files:**
- Modify: `app/src/ipc.rs` (add imports, DTOs, command, mapping fn)
- Modify: `app/src/main.rs` (register the command)

**Interfaces:**
- Consumes: `application::breakeven::plan`; `domain::averaging_down::BreakevenPlan`; `AppState.fx: FxRateBook`; existing `Currency`, `Money`, `Quantity`, `Decimal`, `FromStr`.
- Produces (frontend relies on these field names): `BreakevenPlanArgs` (input), `AveragingDownRowDto` + `BreakevenPlanDto` (output); command name `breakeven_plan` taking `{ args: BreakevenPlanArgs }`.

- [ ] **Step 1: Extend the domain import and add `application::breakeven`**

In `app/src/ipc.rs`, the existing domain import block is:

```rust
use domain::{
    alert::{AlertCondition, AlertRule},
    asset::AssetKind, holding::Holding, money::{Currency, Money}, quantity::Quantity, symbol::Symbol,
};
```

Change it to add `averaging_down::BreakevenPlan`:

```rust
use domain::{
    alert::{AlertCondition, AlertRule},
    asset::AssetKind, averaging_down::BreakevenPlan, holding::Holding,
    money::{Currency, Money}, quantity::Quantity, symbol::Symbol,
};
```

Add a `use` for the application wrapper near the other `use application::...;` lines at the top of the file:

```rust
use application::breakeven;
```

- [ ] **Step 2: Add the DTOs**

In `app/src/ipc.rs`, near the existing `PortfolioValuationDto` / `HoldingValuationDto` definitions, add:

```rust
#[derive(Deserialize)]
pub struct BreakevenPlanArgs {
    pub avg_cost_amount: String,
    pub quantity: String,
    pub current_price_amount: String,
    pub native_currency: String,
    pub targets_pct: Vec<String>,
    pub display_currency: String,
}

#[derive(Serialize, Clone)]
pub struct AveragingDownRowDto {
    pub target_pct: String,
    pub target_avg: String,
    pub add_quantity: String,
    pub add_invest_native: String,
    pub add_invest_native_currency: String,
    pub add_invest_display: Option<String>,
    pub display_currency: String,
    pub new_breakeven_gap_pct: String,
    pub feasible: bool,
}

#[derive(Serialize, Clone)]
pub struct BreakevenPlanDto {
    pub is_underwater: bool,
    pub breakeven_gap_pct: Option<String>,
    pub current_return_pct: Option<String>,
    pub max_reduction_pct: String,
    pub rows: Vec<AveragingDownRowDto>,
}
```

- [ ] **Step 3: Add the mapping fn and the command**

In `app/src/ipc.rs`, add the mapping helper:

```rust
fn breakeven_plan_to_dto(plan: BreakevenPlan, display: Currency) -> BreakevenPlanDto {
    BreakevenPlanDto {
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
                add_invest_native: r.add_invest_native.amount().to_string(),
                add_invest_native_currency: r.add_invest_native.currency().as_str().to_string(),
                add_invest_display: r.add_invest_display.map(|m| m.amount().to_string()),
                display_currency: display.as_str().to_string(),
                new_breakeven_gap_pct: r.new_breakeven_gap_pct.to_string(),
                feasible: r.feasible,
            })
            .collect(),
    }
}
```

And the command:

```rust
#[tauri::command]
pub async fn breakeven_plan(
    state: State<'_, AppState>,
    args: BreakevenPlanArgs,
) -> Result<BreakevenPlanDto, String> {
    let native = Currency::new(&args.native_currency).map_err(|e| format!("{e:?}"))?;
    let display = Currency::new(&args.display_currency).map_err(|e| format!("{e:?}"))?;
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

    let avg_cost = Money::new(a, native);
    let current_price = Money::new(p, native);
    let quantity = Quantity::new(q).map_err(|e| format!("{e:?}"))?;

    let plan = breakeven::plan(
        &state.fx,
        avg_cost,
        quantity,
        current_price,
        &targets,
        display,
    )
    .await;
    Ok(breakeven_plan_to_dto(plan, display))
}
```

- [ ] **Step 4: Register the command**

In `app/src/main.rs`, inside `tauri::generate_handler![ ... ]`, add `ipc::breakeven_plan,` on its own line right after the portfolio commands:

```rust
        ipc::portfolio_upsert, ipc::portfolio_delete, ipc::portfolio_valuation,
        ipc::breakeven_plan,
```

- [ ] **Step 5: Build the workspace**

Run: `cargo build`
Expected: builds with no errors (the whole workspace, including the `app` crate, compiles).

- [ ] **Step 6: Commit**

```bash
git add app/src/ipc.rs app/src/main.rs
git commit -m "feat(app): breakeven_plan IPC command + DTOs"
```

---

## Task 4: Frontend IPC wrapper

**Files:**
- Modify: `src/lib/ipc.ts`

**Interfaces:**
- Consumes: existing `invoke` import, `SymbolDto`.
- Produces (the modal relies on these): `BreakevenPlanArgs`, `AveragingDownRowDto`, `BreakevenPlanDto`, and `ipc.breakevenPlan(args) => Promise<BreakevenPlanDto>`.

- [ ] **Step 1: Add the interfaces**

In `src/lib/ipc.ts`, after the `PortfolioValuationDto` interface (around line 40), add:

```typescript
export interface BreakevenPlanArgs {
  avg_cost_amount: string;
  quantity: string;
  current_price_amount: string;
  native_currency: string;
  targets_pct: string[];
  display_currency: string;
}

export interface AveragingDownRowDto {
  target_pct: string;
  target_avg: string;
  add_quantity: string;
  add_invest_native: string;
  add_invest_native_currency: string;
  add_invest_display: string | null;
  display_currency: string;
  new_breakeven_gap_pct: string;
  feasible: boolean;
}

export interface BreakevenPlanDto {
  is_underwater: boolean;
  breakeven_gap_pct: string | null;
  current_return_pct: string | null;
  max_reduction_pct: string;
  rows: AveragingDownRowDto[];
}
```

- [ ] **Step 2: Add the wrapper to the `ipc` object**

In `src/lib/ipc.ts`, inside the `export const ipc = { ... }` object, after the `portfolioValuation` line, add:

```typescript
  breakevenPlan: (args: BreakevenPlanArgs) => invoke<BreakevenPlanDto>("breakeven_plan", { args }),
```

- [ ] **Step 3: Typecheck**

Run: `npm run typecheck`
Expected: PASS — no type errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/ipc.ts
git commit -m "feat(web): breakevenPlan ipc wrapper + DTO types"
```

---

## Task 5: Frontend — `BreakevenCalc` modal

**Files:**
- Create: `src/components/BreakevenCalc.tsx`

**Interfaces:**
- Consumes: `useWatchlistStore`, `useQuotesStore` + `quoteKey`, `ipc.breakevenPlan`, `BreakevenPlanDto`, `SymbolDto`, `formatMoney`, `Select`.
- Produces: `export function BreakevenCalc({ onClose }: { onClose(): void })` — used by Task 6.

- [ ] **Step 1: Create the component**

Create `src/components/BreakevenCalc.tsx`:

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
  const [displayMode, setDisplayMode] = useState<"native" | "USD" | "KRW">("native");
  const [plan, setPlan] = useState<BreakevenPlanDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selectedSymbol = useMemo<SymbolDto | undefined>(
    () => watchlist.find((s) => quoteKey(s) === selectedKey),
    [watchlist, selectedKey],
  );
  const liveQuote = selectedSymbol ? quotes[quoteKey(selectedSymbol)] : undefined;
  const nativeCcy = selectedSymbol ? defaultCostCurrency(selectedSymbol) : "USD";
  // The live quote drives the price field when present; otherwise the user types it.
  const effectivePrice = liveQuote?.price ?? manualPrice;
  const displayCcy = displayMode === "native" ? nativeCcy : displayMode;

  const targets = useMemo(() => {
    const list = [...PRESET_TARGETS];
    if (customPct && Number(customPct) > 0 && !list.includes(customPct)) list.push(customPct);
    return list;
  }, [customPct]);

  // Debounced recompute; also re-fires whenever the live price ticks (effectivePrice changes).
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
          native_currency: nativeCcy,
          targets_pct: targets,
          display_currency: displayCcy,
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
  }, [avgInput, qtyInput, effectivePrice, nativeCcy, displayCcy, targets]);

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
            <span className="text-slate-700 dark:text-slate-300">평단 ({nativeCcy})</span>
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
            <span className="text-slate-700 dark:text-slate-300">현재가 ({nativeCcy})</span>
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

        <div className="flex items-center gap-2 text-sm">
          <span className="text-slate-700 dark:text-slate-300">표시 통화</span>
          {(["native", "USD", "KRW"] as const).map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => setDisplayMode(m)}
              className={
                "px-2 py-1 rounded text-xs " +
                (displayMode === m
                  ? "bg-emerald-600/15 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400"
                  : "btn-secondary")
              }
            >
              {m === "native" ? `네이티브 (${nativeCcy})` : m}
            </button>
          ))}
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

        {!plan ? (
          <p className="text-sm text-slate-500 dark:text-slate-400">
            평단·보유수량·현재가를 입력하면 본전까지의 상승률과 물타기 시나리오가 계산됩니다.
          </p>
        ) : (
          <>
            <div className="glass-inset rounded p-3">
              <div className="text-xs text-slate-500 dark:text-slate-400">본전까지</div>
              {plan.is_underwater && plan.breakeven_gap_pct ? (
                <div className="text-base text-slate-900 dark:text-slate-100">
                  현재가가{" "}
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
                              {formatMoney(r.add_invest_native)} {r.add_invest_native_currency}
                              {r.add_invest_display && (
                                <span className="text-slate-500 dark:text-slate-400">
                                  {" "}≈ {formatMoney(r.add_invest_display)} {r.display_currency}
                                </span>
                              )}
                              {!r.add_invest_display && displayCcy !== r.add_invest_native_currency && (
                                <span className="text-slate-400 dark:text-slate-500"> (환율 없음)</span>
                              )}
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

- [ ] **Step 2: Typecheck**

Run: `npm run typecheck`
Expected: PASS — no type errors. (`BreakevenCalc` is unused until Task 6; that is not a type error.)

- [ ] **Step 3: Commit**

```bash
git add src/components/BreakevenCalc.tsx
git commit -m "feat(web): break-even & averaging-down calculator modal"
```

---

## Task 6: Wire the entry button into `PortfolioPanel`

**Files:**
- Modify: `src/components/PortfolioPanel.tsx`

**Interfaces:**
- Consumes: `BreakevenCalc` from Task 5.

- [ ] **Step 1: Import the modal**

In `src/components/PortfolioPanel.tsx`, after the existing `import { Select } from "./Select";` line, add:

```tsx
import { BreakevenCalc } from "./BreakevenCalc";
```

- [ ] **Step 2: Add open/close state**

In `PortfolioPanel`, right after `const [open, setOpen] = useState(false);`, add:

```tsx
  const [calcOpen, setCalcOpen] = useState(false);
```

- [ ] **Step 3: Add the header button**

In `src/components/PortfolioPanel.tsx`, replace the header's single button:

```tsx
        <button onClick={() => setOpen(true)} className="btn-secondary text-xs px-2 py-1">+ Add</button>
```

with a two-button group:

```tsx
        <div className="flex gap-1">
          <button onClick={() => setCalcOpen(true)} className="btn-secondary text-xs px-2 py-1">🧮 본전 계산</button>
          <button onClick={() => setOpen(true)} className="btn-secondary text-xs px-2 py-1">+ Add</button>
        </div>
```

- [ ] **Step 4: Render the modal**

In `src/components/PortfolioPanel.tsx`, replace the existing dialog render line:

```tsx
      {open && <AddHoldingDialog onClose={() => setOpen(false)} onSubmit={upsert} />}
```

with both modals:

```tsx
      {open && <AddHoldingDialog onClose={() => setOpen(false)} onSubmit={upsert} />}
      {calcOpen && <BreakevenCalc onClose={() => setCalcOpen(false)} />}
```

- [ ] **Step 5: Typecheck and build**

Run: `npm run typecheck`
Expected: PASS.

Run: `npm run build`
Expected: PASS — `tsc -b && vite build` completes with no errors.

- [ ] **Step 6: Commit**

```bash
git add src/components/PortfolioPanel.tsx
git commit -m "feat(web): open break-even calculator from Portfolio header"
```

---

## Task 7: Full verification

**Files:** none (verification only).

- [ ] **Step 1: Backend tests**

Run: `cargo test`
Expected: PASS — all existing tests plus the 7 `averaging_down` tests and the 1 `breakeven` application test.

- [ ] **Step 2: Backend build**

Run: `cargo build`
Expected: PASS — whole workspace, no errors/warnings introduced.

- [ ] **Step 3: Frontend checks**

Run: `npm run typecheck && npm run build && npm run lint`
Expected: PASS — no type errors, build succeeds, no new lint errors.

- [ ] **Step 4: Manual verification in the running app**

Run: `npm run tauri dev` (or the project's usual dev command), then verify:
- Portfolio header shows **🧮 본전 계산**; clicking opens the modal.
- With a watchlist symbol selected and a live quote present: **현재가** auto-fills, shows "● 실시간 현재가 사용 중", and the results update as the quote ticks (without retyping).
- Enter 평단 / 보유수량 with 평단 > 현재가 (underwater): **본전까지** shows "+X.X% 오르면 본전"; the 물타기 table shows 5 / 10 / 15 % rows; a custom % adds a row; rows at/above N_max show **불가능 — 현재가가 더 낮아야 함**.
- Toggle **표시 통화** between 네이티브 / USD / KRW: 추가 투자금 shows the converted value (≈ …) when a rate exists, or "(환율 없음)" when the cross rate is missing.
- Enter 평단 ≤ 현재가 (not underwater): **본전까지** shows "이미 본전 이상 (+Y%)" and the table is replaced by the not-underwater note.
- "직접 입력" (no symbol) keeps 현재가 editable and computes from manual inputs.

- [ ] **Step 5: Confirm the branch is clean**

Run: `git status`
Expected: clean working tree; all changes committed.

---

## Self-Review (completed by plan author)

**Spec coverage:**
- Break-even gap (underwater) + current-return (not underwater) → Task 1 (`breakeven_gap_when_underwater`, `not_underwater_reports_current_return`) + Task 5 본전까지 section. ✅
- Averaging-down plan (T, x, invest, new gap) per target, B-semantics `T = A·(1 − N/100)` → Task 1 (`worked_example_n10_feasible`). ✅
- N_max boundary + infeasible rows → Task 1 (`presets_feasible_until_n_max`) + Task 5 "불가능" rendering. ✅
- Currency toggle (native/USD/KRW), convert + missing-rate `None` + same-ccy `None` → Task 1 (`fx_*`, `display_equals_native_*`), Task 3 DTO, Task 5 toggle + "(환율 없음)". ✅
- Computation in Rust domain + thin application + one IPC command → Tasks 1–3. ✅
- Live recompute on quote tick + debounced inputs → Task 5 effect. ✅
- Entry button in Portfolio header, modal styled like `AddHoldingDialog` → Tasks 5–6. ✅
- Edge cases (inputs > 0, no quote → manual price, divide-by-zero avoided via `feasible`) → Task 3 validation + Task 1 `feasible` guard + Task 5 input guards. ✅
- **"불러오기" prefill** → intentionally deferred (see Deviations). ⚠️ Documented.
- **i18n** → intentionally hardcoded Korean (see Deviations); no active i18n exists. ⚠️ Documented.

**Type consistency:** `BreakevenPlan` / `AveragingDownRow` field names are identical across domain (Task 1), application return (Task 2), DTO mapping (Task 3), and TS interfaces (Task 4). `breakevenPlan({ args })` ↔ command param `args: BreakevenPlanArgs`; snake_case DTO fields match on both sides. `quoteKey` is reused for both Select values and quote lookup (equal to PortfolioPanel's private `symbolKey`).

**Placeholder scan:** none — every code step contains complete code; every run step has an exact command + expected outcome.
