# Widget Row Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the floating widget's quote rows align into clean columns by switching `WidgetRow` from `flex justify-between` to a fixed-column CSS grid.

**Architecture:** A single presentational component changes. Each row becomes a CSS grid with the column template `minmax(0,1fr) 5rem 3.5rem` (name / price / change), so every row shares identical column widths and prices line up vertically. Plus a small padding cleanup.

**Tech Stack:** React + TypeScript, Tailwind CSS (arbitrary `grid-cols-[...]` value).

**Spec:** `docs/superpowers/specs/2026-05-17-widget-row-alignment-design.md`

---

## Task 1: Switch WidgetRow to a fixed-column grid

**Files:**
- Modify: `src/components/widget/WidgetRow.tsx`

This is a presentational component with no existing unit test (the codebase
unit-tests stores, not presentational components). There is no TDD red/green
step; verification is type-check + build + lint + a visual check.

- [ ] **Step 1: Replace the component**

Replace the entire contents of `src/components/widget/WidgetRow.tsx` with:

```tsx
import clsx from "clsx";
import { formatPrice } from "../../lib/format";
import type { QuoteDto } from "../../lib/ipc";

export function WidgetRow({ q }: { q: QuoteDto }) {
  const change = q.change_24h ? Number(q.change_24h) * 100 : null;
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_5rem_3.5rem] items-baseline gap-2 text-xs py-1">
      <span
        className="truncate text-slate-700 dark:text-slate-300"
        title={q.symbol.ticker}
      >
        {q.display_name ?? q.symbol.ticker}
      </span>
      <span className="text-right tabular-nums text-slate-900 dark:text-slate-100">
        {formatPrice(q.price)}
      </span>
      <span
        className={clsx(
          "text-right tabular-nums",
          change === null
            ? "text-slate-500 dark:text-slate-500"
            : change >= 0
              ? "text-emerald-600 dark:text-emerald-400"
              : "text-rose-600 dark:text-rose-400",
        )}
      >
        {change === null ? "" : `${change >= 0 ? "+" : ""}${change.toFixed(2)}%`}
      </span>
    </div>
  );
}
```

What changed from the previous version (and why):
- Root: `flex justify-between items-center text-xs px-2 py-1` →
  `grid grid-cols-[minmax(0,1fr)_5rem_3.5rem] items-baseline gap-2 text-xs py-1`.
  The three grid columns are name (`minmax(0,1fr)` — flexes, allows truncation),
  price (`5rem` fixed), change (`3.5rem` fixed). A shared template means every
  row's price and change line up vertically.
- The row's own `px-2` is dropped: the list container in `widget.tsx` already
  has `px-2`, so the row was double-indented 8px past the header. Removing it
  aligns row content with the header label.
- `items-center` → `items-baseline` (correct alignment for a row of text).
- Name span loses `max-w-[6rem]` — its width now comes from the `1fr` grid
  column. `truncate` and the `title` tooltip stay.
- Price span gains `text-right` (it is now a fixed-width column).
- Change span loses `w-14` — its width now comes from the `3.5rem` grid column.
  `text-right` and the conditional emerald/rose/slate colors stay.
- The change-sign / `toFixed(2)` / `%` formatting and the `clsx` usage are
  unchanged.

`widget.tsx` is NOT modified.

- [ ] **Step 2: Type-check**

Run: `npm run typecheck`
Expected: PASS (clean — `tsc -b --noEmit`).

- [ ] **Step 3: Build**

Run: `npm run build`
Expected: PASS — `tsc -b` and `vite build` both succeed. The Tailwind JIT
compiles the arbitrary `grid-cols-[minmax(0,1fr)_5rem_3.5rem]` value (underscores
become spaces: `grid-template-columns: minmax(0,1fr) 5rem 3.5rem`).

- [ ] **Step 4: Lint**

Run: `npm run lint`
Expected: 0 errors. The project has 4 pre-existing warnings in `e2e/` files —
those are not related to this change; confirm no NEW warnings appear.

- [ ] **Step 5: Frontend tests**

Run: `npm test`
Expected: PASS — 3 test files, 6 tests, unchanged (no test touches `WidgetRow`).

- [ ] **Step 6: Commit**

```bash
git add src/components/widget/WidgetRow.tsx
git commit -m "fix(web): align widget quote rows into fixed grid columns"
```

- [ ] **Step 7: Visual verification (manual — note for the user)**

This cannot be verified programmatically. The user should open the floating
widget (the 위젯 button in the header) with several quotes of differing name
and price lengths and confirm the prices form a clean right-aligned column and
rows align with the header. Record in the report that visual verification is
pending the user.

---

## Self-Review

**Spec coverage:**
- Switch row to a fixed-column CSS grid (`minmax(0,1fr) 5rem 3.5rem`) → Task 1
  Step 1. ✓
- Name `truncate` + `title`, no `max-w` → Step 1. ✓
- Price `5rem` right-aligned, `tabular-nums` → Step 1. ✓
- Change `3.5rem` right-aligned, conditional colors → Step 1. ✓
- Drop the row's `px-2` (double-padding cleanup) → Step 1. ✓
- `items-center` → `items-baseline` → Step 1. ✓
- `widget.tsx` untouched, header untouched → Step 1 explicitly states it. ✓
- Verification by typecheck + build + visual → Steps 2-7. ✓

**Placeholder scan:** No TBD/TODO. The full component code is given verbatim.

**Type consistency:** `WidgetRow` keeps its exact prop signature
(`{ q: QuoteDto }`), so `widget.tsx`'s `<WidgetRow q={q} />` call site is
unaffected. `formatPrice`, `clsx`, `QuoteDto` imports are unchanged.
