# Break-Even Calculator — Base-Currency (원화 기준) Input — Design

- **Date:** 2026-06-28
- **Status:** Approved (brainstorming complete)
- **Scope:** Evolve the existing break-even / averaging-down calculator so the
  whole computation can run in a chosen **base currency** (네이티브 or 원화),
  not only the asset's native currency. One domain simplification, one
  application/IPC change, one frontend change. No persistence, no schema change.
- **Amends:** `docs/superpowers/specs/2026-06-22-breakeven-averaging-down-calculator-design.md`.
  Replaces that design's output-only `display_currency` toggle (and the
  `add_invest_display` row field) with a single `base_currency` that drives
  input, calculation, and output.

## Summary

Today the calculator forces 평단(average cost) and 현재가(current price) to be
entered in the asset's **native** currency (USD for a US stock), and only the
*output* investment amount can be re-displayed in USD/KRW via a "표시 통화"
toggle. A Korean holder of a US stock thinks in **won**: "내 평단은 주당 28만원,
원화로 본전까지 얼마나 올라야 하나?". This feature lets the user pick a **기준
통화(base currency)** — 네이티브 or **원화(KRW)** — and runs the *entire*
calculation in it. The live USD quote is converted to KRW at the current FX rate,
so the break-even gap and every 물타기 figure are expressed in won and move with
the exchange rate as quotes tick and FX refreshes.

## Motivation

The break-even calculator (merged 2026-06-22) answers "본전까지 몇 %?" and "물타기
시나리오"—but only in the asset's native currency. For the app's primary user
(Korean retail, BYOK), foreign holdings are mentally tracked in KRW: the amount
that actually left a KRW account. "달러로는 본전이어도 원화로는 손해" is a real,
constant question. Because the app already refreshes `USDKRW=X` into the
`FxRateBook` for cross-currency portfolio valuation, the live USD→KRW rate is on
hand; this feature spends it on the calculator.

## Decisions (from brainstorming)

1. **원화 기준 본전 — FX-aware (chosen over a cosmetic input-convenience
   conversion).** The whole calc runs in the base currency: the live native
   price is converted to base, then compared to the base-entered average cost.
   The break-even gap therefore reflects the *current* exchange rate and changes
   as FX moves. (Purchase-time FX is out of scope — only the current rate is
   used.)
2. **Unified `base_currency` (chosen over keeping a separate output "표시 통화"
   toggle).** A single selector drives input, calculation, and output. The
   previous output-only `display_currency` / `add_invest_display` mechanism is
   removed.
3. **Implementation: domain simplification + conversion in the application layer
   (A안, chosen over an FX-aware domain).** All values are pre-converted to the
   base currency before the pure calculation. The domain `plan()` drops
   `fx_rates` and `display_currency` and becomes pure single-currency math; the
   application layer performs the single native→base conversion of the live
   price using the FX snapshot it already reads. `FxRates::convert` is itself a
   domain method, so Money math stays in the domain (the layer-boundary rule and
   ADR 0006 hold).

## Ubiquitous Language (additions)

| Term | Meaning |
|---|---|
| Base Currency (기준 통화) | The single currency the calculator runs in — inputs, computation, and outputs. Either the asset's native currency or KRW. |
| 원화 기준 본전 | Break-even evaluated in KRW: the price level (KRW-converted) equal to the KRW average cost, where the won value of the position is whole. Moves with FX. |

(`Break-Even`, `Break-Even Gap`, `Averaging Down`, `Averaging-Down Plan`,
`Target Reduction` carry over from the 2026-06-22 design, now evaluated in the
base currency.)

## Formulas

Let `B` = base currency, `A` = average cost (entered in `B`), `q` = quantity,
`P_in` = supplied price in its own currency, `N` = target reduction percent.

**Price normalization to base:**

```
P = convert(P_in, B)          # identity when P_in's currency == B
                              # native→base via FxRates when they differ
                              # → rate_missing when the cross rate is unknown
```

With `P` in `B`, the existing single-currency formulas apply unchanged, now all
in `B`:

```
is_underwater          = P < A
breakeven_gap_pct      = (A / P − 1) × 100          # when underwater
current_return_pct     = (P / A − 1) × 100          # when P ≥ A
max_reduction_pct      = (A − P) / A × 100          # N_max

# per target N:
T  = A × (1 − N/100)                                # target average (B)
feasible ⟺ is_underwater AND P < T < A
x  = q × (A − T) / (T − P)                          # additional quantity
add_invest = P × x                                  # additional money, in B
new_breakeven_gap_pct = (T / P − 1) × 100
```

No output FX conversion remains: every Money is already in `B`.

### Worked example (KRW base, US stock)

`A` = ₩280,000/share, `q` = 10, live price `$150`, rate ₩1,400/$ →
`P` = ₩210,000. Underwater (210k < 280k).

- Break-even gap = `(280,000/210,000 − 1) × 100` = **+33.3%**.
- `N = 10%` → `T` = ₩252,000 → `x = 10 × (280,000 − 252,000)/(252,000 − 210,000)` =
  **6.667 shares** → `add_invest` = ₩1,400,000 → new break-even gap =
  `(252,000/210,000 − 1) × 100` = **+20%**.
- `N_max` = `(280,000 − 210,000)/280,000 × 100` = **25%**; `N ≥ 25%` → 불가능.

If the same inputs are computed with base = 네이티브(USD) — `A` would instead be
entered as `$200` — the math is identical in USD and no FX is touched (current
behavior preserved).

## Architecture & Data Flow

```
BreakevenCalc.tsx
  ├─ base_currency selector: 네이티브 / 원화   (hidden when native == KRW)
  ├─ live quote? → price sent in NATIVE (price_currency = native)
  │  no quote?   → user types price in BASE   (price_currency = base)
  ├─ avg_cost, quantity entered in BASE
  │  debounced invoke
  ▼
ipc.breakevenPlan(args) ──► Tauri `breakeven_plan`
                              │ parse strings → Money/Quantity/Currency
                              ▼
              application::breakeven::plan(fx, A, q, P_in, base, targets)
                              │ P = (P_in.ccy == base) ? P_in
                              │     : fx.snapshot().convert(P_in, base)? → RateMissing
                              ▼
              domain::averaging_down::plan(A, q, P, targets)   # pure, single-ccy
                              │
                              ▼
              Outcome { plan, current_price_base, fx_rate_used }
                              ▼
                       BreakevenPlanDto (rate_missing on RateMissing)
```

### Domain — `crates/domain/src/averaging_down.rs` (simplify)

```rust
pub struct AveragingDownRow {
    pub target_pct: Decimal,
    pub target_avg: Money,        // base
    pub add_quantity: Quantity,
    pub add_invest: Money,        // base   (was: add_invest_native)
    pub new_breakeven_gap_pct: Decimal,
    pub feasible: bool,
    // REMOVED: add_invest_display
}

pub struct BreakevenPlan {        // unchanged
    pub is_underwater: bool,
    pub breakeven_gap_pct: Option<Decimal>,
    pub current_return_pct: Option<Decimal>,
    pub max_reduction_pct: Decimal,
    pub rows: Vec<AveragingDownRow>,
}

pub fn plan(
    avg_cost: Money,        // base
    quantity: Quantity,
    current_price: Money,   // base (caller pre-converts)
    targets_pct: &[Decimal],
) -> BreakevenPlan;
// REMOVED params: fx_rates, display_currency
```

Precondition unchanged in spirit: `avg_cost.currency() == current_price.currency()`
(both are the base currency). The feasibility guard (`is_underwater && p < t < a`)
and the non-positive-target handling from `d0d13ae` are retained verbatim.

### Application + IPC — `crates/application/src/breakeven.rs`, `app/`

```rust
pub enum BreakevenError { RateMissing }

pub struct Outcome {
    pub plan: BreakevenPlan,
    pub current_price_base: Money,     // P after conversion (for UI echo)
    pub fx_rate_used: Option<Decimal>, // None when no conversion was needed
}

pub async fn plan(
    fx: &FxRateBook,
    avg_cost: Money,           // base
    quantity: Quantity,
    current_price: Money,      // native (live) or base (manual)
    base_currency: Currency,
    targets_pct: &[Decimal],
) -> Result<Outcome, BreakevenError>;
// converts current_price → base via snapshot when currencies differ;
// rate absent → Err(RateMissing). fx_rate_used = converted/native amount ratio,
// i.e. the applied (native→base) rate, or None when identity.
```

Tauri `breakeven_plan` IPC args (snake_case, decimals as strings):

| field | meaning |
|---|---|
| `avg_cost_amount` | 평단, in base |
| `quantity` | 수량 |
| `current_price_amount` | 가격 |
| `price_currency` | currency of the price — native (live) or base (manual) |
| `base_currency` | 기준 통화 (replaces `display_currency`) |
| `targets_pct` | `string[]` |

The handler parses inputs, calls `application::breakeven::plan`, and maps
`Ok(Outcome)` → populated DTO, `Err(RateMissing)` → DTO with `rate_missing: true`
and empty/null computed fields. Genuine input-parse failures still return
`Err(String)` (thrown to the frontend `.catch`), as today.

### Frontend DTO — `src/lib/ipc.ts`

```ts
export interface AveragingDownRowDto {
  target_pct: string;
  target_avg: string;
  add_quantity: string;
  add_invest: string;                 // base   (was add_invest_native + currency + display)
  new_breakeven_gap_pct: string;
  feasible: boolean;
}
export interface BreakevenPlanDto {
  rate_missing: boolean;              // NEW
  base_currency: string;              // NEW (replaces display_currency)
  current_price_base: string | null;  // NEW — converted price for UI echo
  fx_rate_used: string | null;        // NEW — applied native→base rate
  is_underwater: boolean;
  breakeven_gap_pct: string | null;
  current_return_pct: string | null;
  max_reduction_pct: string;
  rows: AveragingDownRowDto[];
}
// ipc.breakevenPlan(args) => invoke<BreakevenPlanDto>("breakeven_plan", { ... })
```

## UI / UX — `src/components/BreakevenCalc.tsx`

- The "표시 통화" toggle is **replaced** by a **기준 통화** selector:
  `네이티브 (USD)` / `원화 (KRW)`. When the asset's native currency is already KRW,
  the selector is **hidden** (no meaningful choice).
- Input labels reflect the base currency: `평단 (KRW)`, `현재가 (KRW)`.
- **Price field behaviour:**
  - Live quote present and base ≠ native → send the native price with
    `price_currency = native`; display the **base-converted** value
    (`current_price_base`, read-only) plus a "환율 ₩1,400/$ 적용"
    note from `fx_rate_used`.
  - No live quote → the user types 현재가 directly in the base currency
    (`price_currency = base`); no conversion, no FX needed.
  - base == native → unchanged from today.
- **본전 문구** in base terms: underwater → "원화 기준 현재가가 **+X.X%** 오르면
  본전"; otherwise "이미 본전 이상 (+Y%)". Recompute stays debounced (~150 ms) and
  re-fires on each `quote-update` and on FX refresh.
- **rate_missing** → replace outputs with a notice: "원화 환율을 아직 불러오지
  못했습니다. 네이티브 통화로 보거나 잠시 후 다시 시도하세요."
- **i18n:** all new literals added to `src/i18n/ko.json` and `src/i18n/en.json`.

## Edge Cases & Validation

- `A`, `q`, `P` must parse as decimals `> 0`; otherwise outputs show the input
  prompt rather than computing (unchanged).
- base = 원화, native = USD/USDT(→USD) → `(USD, KRW)` rate present → converts.
- base = 원화, native = EUR/JPY/other → no direct cross rate (not auto-derived) →
  `rate_missing` notice (acceptable; US stocks + USD crypto are the target case).
- base = 원화 but FX not yet refreshed / refresh failed → `rate_missing`.
- base = native → FX untouched; identical to current behaviour (regression-guarded).
- `T − P → 0` (`N → N_max`) → row infeasible (no divide-by-zero), per existing guard.
- `P ≥ A` (not underwater in base terms) → current-return branch, table suppressed.
- `add_quantity` reported as an exact decimal; no whole-share rounding (crypto is
  fractional) — unchanged.

## Changes (file-level)

- `crates/domain/src/averaging_down.rs` — simplify `plan` signature; drop
  `fx_rates`/`display_currency`; rename row `add_invest_native` → `add_invest`;
  remove `add_invest_display`; rewrite tests as single-currency.
- `crates/application/src/breakeven.rs` — convert native→base via snapshot;
  add `BreakevenError::RateMissing` + `Outcome`; tests for conversion and
  missing-rate.
- `app/src/ipc.rs` (+ DTO mapping) — new args (`price_currency`, `base_currency`),
  new DTO fields (`rate_missing`, `base_currency`, `current_price_base`,
  `fx_rate_used`), `RateMissing` → `rate_missing: true`.
- `src/lib/ipc.ts` — DTO/arg type updates per above.
- `src/components/BreakevenCalc.tsx` — base-currency selector, base-aware labels,
  converted-price echo + rate note, `rate_missing` notice; remove display toggle.
- `src/i18n/ko.json`, `src/i18n/en.json` — strings.

## Out of Scope

- Purchase-time FX (only the current rate is used).
- 원화 총투자금 → 평단 역산 (a different input model, not chosen).
- A third base-currency option (USD) — only 네이티브 / 원화.
- A separate output display-currency toggle (subsumed by `base_currency`).
- Auto-deriving FX cross rates; persistence; fees/taxes/slippage; whole-share
  rounding — all unchanged from the 2026-06-22 design.

## Testing

- **Domain (`cargo test`)** — single-currency rewrite of: break-even gap; the
  worked example (`x`, `add_invest`, new gap); `N_max` boundary and infeasible
  `N ≥ N_max`; not-underwater (`P ≥ A`) branch; non-positive target → infeasible
  (no panic). FX assertions removed (FX no longer in the domain).
- **Application (`cargo test`)** — native≠base converts the live price via the
  snapshot and runs the calc in base (KRW worked example); base==native skips
  conversion; missing cross rate → `Err(RateMissing)`.
- **Frontend** — `npm run typecheck` + `npm run build`; manual verification in the
  running app: 기준 통화 toggle, live-price KRW echo + rate note, `rate_missing`
  notice, FX-tick recompute. Presentational components are not unit-tested
  (codebase convention); the math is covered in the domain + application.
