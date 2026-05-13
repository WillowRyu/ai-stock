# ai-stock — Context & Ubiquitous Language

> Last updated: 2026-05-13 (M2 complete)

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

- **M2 complete.** Technical indicators (SMA/EMA/RSI/MACD/Bollinger), price-threshold alerts with desktop notifications, KR stocks via Naver scraping. CSP tightened to explicit provider origins. Poll interval driven by settings at startup.
- **M3 next** — BYOK AI (OpenAI/Anthropic/Gemini), news providers, commentary/analysis prompts.
- **Known M2 limitations:** Naver KR scraping is fragile (no API). KIS OpenAPI deferred. AlertService runs synchronously inside the poll loop (no separate worker). Poll-interval change requires app restart.
