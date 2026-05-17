# ai-stock — Context & Ubiquitous Language

> Last updated: 2026-05-17 (M4 complete)

## Bounded Contexts

- **Market Data** — fetching, caching, and computing on quotes.
- **Portfolio** — holdings, valuation, P&L, cross-currency aggregation.
- **Alerts** — rule evaluation and notification (M2).
- **AI Assistance** — commentary/analysis (M3).

## Ubiquitous Language

| Term | Meaning |
|---|---|
| Symbol | Canonical identity of a tradable asset (kind + ticker + optional quote currency). |
| Quote | A point-in-time price observation for a Symbol. |
| Candle | OHLCV bar over a CandleInterval (1m / 5m / 30m / 1h / 1d / 1w). |
| Holding | A position the user owns: Symbol + Quantity + cost basis. |
| Watchlist | Aggregate of Symbols the user wants to track. |
| Portfolio | Aggregate of Holdings; evaluated against current Quotes in a base currency. |
| Money | Decimal amount + Currency (3-5 uppercase ASCII letters, e.g. USD, KRW, USDT). |
| Quantity | Non-negative decimal count of units. |
| FxRates | Pure value object — a table of directional currency conversion rates. |
| Conversation | An ordered, per-symbol list of user/assistant Messages — a multi-turn AI chat. |
| Message | One turn in a Conversation: a Role (User/Assistant) plus text content. |
| PromptKind | Which preset analysis a turn is: Commentary, ChartAnalysis, or NewsSummary. |
| Provider | External source for quotes (Binance, Yahoo, KIS, Naver, etc.) hidden behind a trait. |

## Current State

- **M1 + M2 + M3 + M4 complete.** See `docs/progress.md`. 100+ backend test functions.
- **AI assistant (M4).** The AI panel is a per-symbol multi-turn chat: free-form
  follow-up messages with three preset quick-start buttons (commentary,
  chart-analysis, news-summary). History is session memory; streaming turns can
  be cancelled. See ADR 0007.
- **Charts.** `ChartPanel` renders historical candles with SMA/RSI/MACD subpanes and
  volume bars inside the DetailPane; candle interval is user-configurable.
- **Indicator alerts.** `AlertCondition` covers RSI thresholds and MACD crosses in
  addition to price thresholds, evaluated via an `EvalContext`.
- **Cross-currency portfolio.** Holdings in different currencies are converted into a
  base currency via `FxRates` (domain) / `FxRateBook` (application, Yahoo-refreshed).
  See ADR 0006.
- **KR stocks.** Two sources: Naver Finance scraping (no account needed, fragile —
  ADR 0003) and `KisProvider` (한국투자증권 BYOK brokerage credentials — ADR 0005).
- **Theming + UI.** Light/dark/system theme store, cross-platform window vibrancy,
  glass surfaces, custom Select, sectioned Settings dialog.
- **Known limitations:** AI still has only the commentary prompt type (chart-analysis
  and news-summary are stubs); no chat history; AI stream cancellation not wired.
  FX cross rates are not auto-derived — the refresher must populate each pair used.
- **Next:** chart-analysis / news-summary prompts, chat history, then a 1.0 release.

## Architecture Decisions

See `docs/adr/`:

- 0001 — Canonical Symbol with per-provider translation.
- 0002 — Polling-only for M1.
- 0003 — Grep-based layer check, Naver scraping for KR stocks.
- 0004 — BYOK AI with streaming over Tauri events.
- 0005 — KIS Open API provider for KR stocks (BYOK brokerage credentials).
- 0006 — FxRates value object + cross-currency portfolio aggregation.
- 0007 — Multi-turn AI conversation.
