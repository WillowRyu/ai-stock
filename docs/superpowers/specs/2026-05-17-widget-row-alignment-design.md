# Widget Row Alignment — Design

- **Date:** 2026-05-17
- **Status:** Approved (brainstorming complete)
- **Scope:** Small UI polish — single component.

## Summary

The floating widget lists up to five quotes. Each `WidgetRow` lays its three
fields (symbol name, price, 24h change) out with `flex justify-between`, so the
columns are not vertically aligned across rows — names and prices have varying
intrinsic widths, leaving the price ragged. This redesign switches the row to a
fixed-column CSS grid so prices and changes line up cleanly, plus a light
padding/alignment cleanup.

## Problem

`WidgetRow` (`src/components/widget/WidgetRow.tsx`) uses
`flex justify-between items-center`. The three children:

- name — `truncate max-w-[6rem]`, width varies with content;
- price — no width constraint, width varies with content;
- change — `w-14` fixed.

With `justify-between` the inter-column gaps absorb whatever space is left after
the (varying) intrinsic widths, so each row's price sits at a different x. The
price column is ragged row-to-row — the "spacing doesn't line up" the user
reported.

A secondary issue: the row has `px-2` while its list container
(`widget.tsx`) also has `px-2`, double-indenting rows 8px past the header.

## Decision

Switch the row to a CSS grid with a shared, fixed column template so all rows
align. Approach chosen during brainstorming: **CSS Grid** (over flex-with-fixed-
widths, which is visually equivalent, and over an HTML `<table>`, which is
overkill and unprecedented in this codebase).

### Changes — `src/components/widget/WidgetRow.tsx` only

| Element | Before | After |
|---|---|---|
| row container | `flex justify-between items-center text-xs px-2 py-1` | `grid grid-cols-[minmax(0,1fr)_5rem_3.5rem] items-baseline gap-2 text-xs py-1` |
| name span | `truncate max-w-[6rem]` + colors | `truncate` + colors (width now from the `1fr` grid column) |
| price span | `tabular-nums` + colors | `text-right tabular-nums` + colors |
| change span | `tabular-nums w-14 text-right` + colors | `text-right tabular-nums` + colors (width now from the grid column) |

Colors (`text-slate-700 dark:text-slate-300` name, `text-slate-900
dark:text-slate-100` price, conditional emerald/rose/slate for change), the
`title` tooltip on the name, the `py-1` row spacing, and the change-sign /
percent formatting are all unchanged.

### Column widths

`formatPrice` emits a thousands-separated number with no currency symbol.

- **Price — `5rem` (80px), right-aligned.** At `text-xs` with `tabular-nums`
  this holds the realistic worst case (`1,234,567.00`, a million-won KR stock,
  ~12 chars). Common cases (`182.34`, `67,000.00`) sit comfortably inside.
- **Change — `3.5rem` (56px), right-aligned.** Unchanged from the current
  `w-14`; holds `+12.34%`.
- **Name — `minmax(0,1fr)`.** Fills the remaining width (~92px in the 260px
  window); `min-width: 0` lets `truncate` engage, with the existing `title`
  tooltip for the full name.

### Light polish (in scope)

- Drop `px-2` from the row. The list container's `px-2` already provides the
  horizontal inset; removing the row's own `px-2` ends the double-padding and
  aligns row content with the header label (also `px-2`).
- `items-center` → `items-baseline` — baseline alignment is the correct choice
  for a row of text fields.

## Out of Scope

- The widget header (`ai-stock` label, opacity slider, × button) — untouched.
- `widget.tsx` — no change.
- Row separators, hover states, column header labels — not added (the goal is a
  clean, minimal list).

## Testing

`WidgetRow` has no existing unit test, consistent with the codebase convention
(stores are unit-tested; presentational components are not). Verification is
`npm run typecheck` + `npm run build` + visual check in the running widget.
