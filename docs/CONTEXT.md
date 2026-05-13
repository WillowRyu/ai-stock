# ai-stock — Context & Ubiquitous Language

> Last updated: 2026-05-13 (M3 complete)

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

- **M3 complete.** BYOK AI with OpenAI / Anthropic / Gemini streaming, news context from Yahoo + CoinDesk, prompt template in domain. AI is toggleable; without a key set, the app works exactly as M2.
- **Known M3 limitations:** Only commentary prompt type — chart-analysis and news-summary variants are stubs. No chat history (each request is independent). RSS news is best-effort and unsymmetric (Yahoo per-symbol, CoinDesk filtered). Stream cancellation isn't wired — closing the AI panel during generation lets it finish to completion.
- **Next:** post-M3 polish (chart panel rendering historical candles with overlays, chat history, news summary prompts), then a 1.0 release.
