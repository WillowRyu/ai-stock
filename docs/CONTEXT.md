# ai-stock — Context & Ubiquitous Language

> Last updated: 2026-05-13 (Task 0.3)

## Bounded Contexts

- **Market Data** — fetching, caching, and computing on quotes.
- **Portfolio** — holdings, valuation, P&L.
- **Alerts** — rule evaluation and notification (M2).
- **AI Assistance** — commentary/analysis (M3).

## Ubiquitous Language

| Term | Meaning |
|---|---|
| Symbol | Canonical identity of a tradable asset (kind + ticker + optional quote currency). |
| Quote | A point-in-time price observation for a Symbol. |
| Candle | OHLCV bar over a time interval. |
| Holding | A position the user owns: Symbol + Quantity + cost basis. |
| Watchlist | Aggregate of Symbols the user wants to track. |
| Portfolio | Aggregate of Holdings; can be evaluated against current Quotes. |
| Money | Decimal amount + Currency (3-5 uppercase ASCII letters, e.g. USD, KRW, USDT). |
| Quantity | Non-negative decimal count of units. |
| Provider | External source for quotes (Binance, Yahoo, etc.) hidden behind a trait. |

## Current State

- M1 in progress — scaffolding complete (Task 0.1, 0.2).
