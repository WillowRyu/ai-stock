# Break-Even & Averaging-Down Calculator — Design

- **Date:** 2026-06-22
- **Status:** Approved (brainstorming complete)
- **Scope:** New Portfolio feature — one pure domain module, one IPC command, one
  frontend modal. No persistence, no schema change.

## Summary

A standalone "본전·물타기 계산기" (break-even / averaging-down calculator) opened
from the Portfolio panel. Given an average cost, a quantity, and a live current
price, it answers two questions a holder who is underwater asks:

1. **Break-even** — by what percent must the current price rise to reach the
   average cost (본전)?
2. **Averaging down (물타기)** — if I want to pull my average cost *down* by 5 /
   10 / 15 % (or a custom %), how much more must I buy at the current price, and
   how much money is that — shown in the asset's native currency and, via a
   toggle, converted to USD or KRW?

Everything recomputes live as the selected symbol's Quote ticks.

## Motivation

The Portfolio panel today shows market value and P&L per Holding, but a holder
sitting on a loss has no tool to reason about recovery. "How far underwater am
I?" and "what does it cost to average down to a tolerable break-even?" are the
two questions retail holders ask constantly, and they currently do the
arithmetic by hand. This feature puts both answers one click away, using the
average cost and live price the app already has.

## Ubiquitous Language (additions)

| Term | Meaning |
|---|---|
| Break-Even | The price level equal to a Holding's average cost, where P&L is zero (본전). |
| Break-Even Gap | The percent the current price must rise to reach Break-Even: `(avg / price − 1) × 100`. Defined only while underwater (`price < avg`). |
| Averaging Down | Buying additional units *below* the current average cost to lower it (물타기). |
| Averaging-Down Plan | For a chosen lower target average cost, the additional Quantity and investment (native + display currency) required at the current price. |
| Target Reduction | The chosen percent to lower the average cost by: target avg `T = avg × (1 − N/100)`. |

## Decision

### Semantics of the target percent — "lower the average cost by N%" (B안)

During brainstorming two readings were weighed:

- **A — target break-even gap:** buy until the new average sits at `price × (1 +
  N/100)` (so only +N% is needed to break even).
- **B — target reduction of average cost (chosen):** buy until the new average is
  `avg × (1 − N/100)`.

The user chose **B**: the percent lowers the average cost itself. The two differ
numerically; B is what this spec implements.

### Computation location — Rust domain + IPC (A안)

Two implementation sites were weighed:

- **A — Rust domain function + IPC command (chosen).** A pure
  `crates/domain/src/averaging_down.rs` holds the formulas (unit-tested with
  `cargo test`); an application/IPC layer exposes `breakeven_plan(...)`, reusing
  the existing `FxRates` for the currency toggle. Matches this codebase's rule
  that all Money math lives in the domain with `rust_decimal` precision (the
  layer-boundary script and ADR 0006 set this precedent).
- **B — frontend TypeScript calc + a small `fx_rate` IPC.** Snappier on
  keystrokes but moves Money math out of the domain, uses JS floats, and handles
  FX separately in the frontend.

**A** was chosen. The current price flows *into* the command from the frontend's
live Quote (the frontend owns the real-time price); the command is otherwise a
stateless compute that reads the current `FxRates` snapshot for conversion,
mirroring how `portfolio_calc::evaluate` takes `fx_rates` and a
`display_currency`. IPC calls are debounced; Quotes poll every 1–60 s, so the
round-trip cost is negligible.

## Formulas

Let `A` = average cost, `q` = quantity held, `P` = current price (all in the
asset's native currency, `A` and `P` sharing it), `N` = target reduction percent.

**Break-even gap** (underwater, `P < A`):

```
breakeven_gap_pct = (A / P − 1) × 100
```

If `P ≥ A` the holder is already at or above break-even; report the current
return `(P / A − 1) × 100` instead and suppress the averaging-down table (see
feasibility).

**Averaging-down plan** for target reduction `N`:

```
T  = A × (1 − N/100)              # target average cost
x  = q × (A − T) / (T − P)        # additional quantity to buy at P
invest_native  = P × x            # money required, native currency
new_breakeven_gap_pct = (T / P − 1) × 100
new_quantity   = q + x
new_cost_basis = A·q + P·x
```

Derivation of `x`: solving `(A·q + P·x) / (q + x) = T` for `x`.

**Currency toggle:**

```
invest_display = FxRates.convert(invest_native, display_currency)   # may be None
```

When `display_currency` equals the native currency, no conversion is applied.
When the required cross rate is absent (CONTEXT.md notes cross rates are not
auto-derived), `invest_display` is `None` and the UI shows the native amount only
with a "환율 없음" note — the same graceful degradation the portfolio totals use.

### Feasibility

Averaging down can only *lower* the average when buying *below* it, and the
lowest reachable average (buying infinitely at `P`) is `P` itself. Therefore:

```
feasible(N)  ⟺  P < A  AND  T > P
             ⟺  N < N_max,  where  N_max = 100 × (A − P) / A
```

`N_max` is exactly the current loss expressed as a fraction of the average cost.

- A row with `N ≥ N_max` is shown as **"불가능 — 현재가가 더 낮아야 함"** (as
  `N → N_max`, `x → ∞`).
- If `P ≥ A` (not underwater), the whole table is replaced by a note: buying at or
  above the average cannot lower it. The break-even section then shows the
  current return instead of a gap.

### Worked example

`A` = ₩100,000, `P` = ₩80,000, `q` = 1. `N_max = 100 × (100,000 − 80,000)/100,000
= 20%`.

- Break-even gap = `(100,000/80,000 − 1) × 100` = **+25%**.
- `N = 10%` → `T` = ₩90,000 → `x = 1 × (100,000 − 90,000)/(90,000 − 80,000)` = **1
  share** → `invest_native` = ₩80,000 → new break-even gap = `(90,000/80,000 − 1)
  × 100` = **+12.5%**.
- `N = 20%` and above → **불가능** (`T ≤ P`).

## Architecture & Data Flow

```
BreakevenCalc.tsx ── reads live P from quotesStore (quote-update stream)
        │            reads A, q from inputs (or prefilled from a saved Holding)
        │  debounced invoke
        ▼
ipc.breakevenPlan(args)  ──►  Tauri command `breakeven_plan`
                                  │ reads current FxRates snapshot from app state
                                  ▼
                          domain::averaging_down::plan(A, q, P, targets, fx, display_ccy)
                                  │ pure, rust_decimal
                                  ▼
                          BreakevenPlan  ──► BreakevenPlanDto  ──► rendered
```

### Domain — `crates/domain/src/averaging_down.rs` (new)

Pure, no IO. Public API (sketch):

```rust
pub struct AveragingDownRow {
    pub target_pct: Decimal,        // N
    pub target_avg: Money,          // T
    pub add_quantity: Quantity,     // x
    pub add_invest_native: Money,   // P·x
    pub add_invest_display: Option<Money>,  // converted, None if no rate / same ccy
    pub new_breakeven_gap_pct: Decimal,
    pub feasible: bool,
}

pub struct BreakevenPlan {
    pub is_underwater: bool,
    pub breakeven_gap_pct: Option<Decimal>,   // Some when underwater
    pub current_return_pct: Option<Decimal>,  // Some when at/above break-even
    pub max_reduction_pct: Decimal,           // N_max
    pub rows: Vec<AveragingDownRow>,          // one per requested target
}

pub fn plan(
    avg_cost: Money,
    quantity: Quantity,
    current_price: Money,        // native ccy == avg_cost ccy
    targets_pct: &[Decimal],
    fx_rates: &FxRates,
    display_currency: Currency,
) -> BreakevenPlan;
```

Reuses `Money`, `Quantity`, `Currency`, `FxRates`. Registered in
`crates/domain/src/lib.rs`.

### Application + IPC

- A thin application path supplies the current `FxRates` snapshot (the same
  `FxRateBook` the portfolio valuation already uses) and calls `plan`.
- New Tauri command `breakeven_plan` in `app/` accepts the frontend args
  (decimal-as-string), maps to domain types, returns `BreakevenPlanDto`.

### Frontend — `src/lib/ipc.ts`

```ts
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
// ipc.breakevenPlan(args) => invoke<BreakevenPlanDto>("breakeven_plan", { ... })
```

## UI / UX — `src/components/BreakevenCalc.tsx` (new)

Entry point: a **"🧮 본전 계산"** button in the Portfolio panel header next to
`+ Add`, opening a modal in the same style as `AddHoldingDialog`.

**Inputs**

| Field | Behaviour |
|---|---|
| 종목 (optional) | `Select` over the watchlist. On pick, `현재가` auto-fills live from `quotesStore` and keeps ticking. If the symbol is also a saved Holding, a "불러오기" action prefills 평단 + 수량. |
| 현재가 `P` | Auto from the Quote; editable / manual when no Quote. |
| 평단 `A` | Manual (or prefilled). |
| 보유수량 `q` | Manual (or prefilled). |
| 표시 통화 | Toggle: 네이티브 / USD / KRW — drives `display_currency`. |

**Outputs**

1. **본전까지** — underwater: "현재가가 **+X.X%** 오르면 본전"; otherwise "이미 본전
   이상 (+Y%)".
2. **물타기 표** — preset rows **5 / 10 / 15 %** plus a custom-percent input. Each
   row: 목표 평단 `T` · 추가 매수 수량 `x` · 추가 투자금 (native, plus converted when
   the toggle ≠ native) · 매수 후 새 본전까지. Infeasible rows render the "불가능"
   note; when `P ≥ A` the table is replaced by the not-underwater note.

Recompute is debounced (~150 ms) on input change and fires on each
`quote-update` for the selected symbol.

**i18n:** all literals added to `src/i18n/ko.json` and `src/i18n/en.json`.

## Edge Cases & Validation

- `A`, `q`, `P` must parse as decimals `> 0`; otherwise the outputs show a
  prompt rather than computing.
- No Quote for the selected symbol → require manual `현재가`.
- `T − P → 0` (`N → N_max`) → row infeasible (avoids divide-by-zero / ∞).
- `P ≥ A` → not-underwater branch (no negative-quantity rows).
- Missing FX cross rate → `add_invest_display = null`, native shown with "환율
  없음".
- `add_quantity` is reported as an exact decimal. Whole-share rounding for
  equities is **not** forced (crypto is fractional); see Out of Scope.

## Changes (file-level)

- `crates/domain/src/averaging_down.rs` — **new**, with unit tests.
- `crates/domain/src/lib.rs` — register module.
- `crates/application/` — supply `FxRates` snapshot + call `plan` (thin).
- `app/` — new `breakeven_plan` IPC command + DTO mapping + command registration.
- `src/lib/ipc.ts` — `BreakevenPlanDto` / `AveragingDownRowDto` + `breakevenPlan`.
- `src/components/BreakevenCalc.tsx` — **new** modal.
- `src/components/PortfolioPanel.tsx` — header button (one line + state).
- `src/i18n/ko.json`, `src/i18n/en.json` — strings.

## Out of Scope

- Persisting calculator inputs or results (it is a transient what-if).
- Whole-share / lot-size rounding and per-market trading rules.
- Fees, taxes, and slippage in the break-even / investment figures.
- Multi-currency *holdings* in one calc (one asset, one native currency at a time).
- Auto-deriving FX cross rates — the existing refresher's coverage is reused as-is.
- An averaging-*up* / target-price mode (the rejected A semantics).

## Testing

- **Domain (`cargo test`)** — `averaging_down`: break-even gap; the worked
  example (`x`, invest, new gap); `N_max` boundary and infeasible `N ≥ N_max`;
  not-underwater (`P ≥ A`) branch; FX conversion present vs. missing-rate `None`;
  decimal precision. Consistent with the domain's existing test density.
- **Frontend** — `npm run typecheck` + `npm run build`; manual verification in the
  running app (live tick updates, currency toggle, infeasible rows). Per codebase
  convention, presentational components are not unit-tested; the math is covered
  in the domain.
