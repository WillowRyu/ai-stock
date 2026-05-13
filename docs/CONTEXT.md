# ai-stock — Context & Ubiquitous Language

> Last updated: 2026-05-13 (M1 complete)

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

- **M1 complete.** Working cross-platform Tauri app with hybrid main window + always-on-top floating widget, Crypto (Binance + CoinGecko) + US/Forex/Commodity (Yahoo + optional Finnhub) coverage, watchlist + portfolio P&L + adjustable widget transparency. ~45 backend unit tests + 1 frontend store test + E2E smoke.
- **M2 next** — KR stocks (Naver / KIS), technical indicators (MA, RSI, MACD, Bollinger), price-threshold alerts + desktop notifications, forex/commodities polish.
- **M3 after** — BYOK AI (OpenAI/Anthropic/Gemini), news providers, commentary/analysis prompts.
- **Known M1 limitations:** Currency widened to 3-5 char ASCII to support stablecoins (USDT/USDC). Tauri CSP loose (default-src 'self' + https connect-src) — tighten in M2. cargo-deny limited to `bans` check (license/advisory subset blocked by transitive gtk-rs/MPL deps — revisit when tauri upstream migrates off gtk3).
