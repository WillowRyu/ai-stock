# ai-stock — Design Spec

- **Date:** 2026-05-13
- **Status:** Approved (brainstorming phase)
- **Next step:** writing-plans → implementation

---

## 1. Goal

A cross-platform desktop app for real-time multi-asset price tracking with optional AI commentary. Inspired by Pluely's lightweight Tauri form factor but built as a public, non-stealth product.

The app shows live prices, statistics, and portfolio valuation across stocks, crypto, and FX, optionally enriched with AI explanations and chart analysis. It runs on macOS and Windows (Linux follows from the Tauri toolchain).

## 2. Scope decisions

| Item | Decision |
|---|---|
| Platforms | macOS + Windows (Linux falls out of Tauri) |
| Form factor | Hybrid — main dashboard window **plus** always-on-top floating mini widget. User-adjustable transparency on the widget. Not stealth. |
| Asset coverage | US stocks (NYSE/NASDAQ), KR stocks (KOSPI/KOSDAQ), Crypto, Forex/Commodities |
| Update model | Polling, centralized in the backend. User-configurable interval per watchlist with a default of 5s and a hard floor of 1s. |
| Tech stack | Tauri (Rust backend, React+TypeScript frontend), SQLite locally, OS keychain for secrets |
| Statistics | Basic price stats + technical indicators (MA, RSI, MACD, Bollinger) + portfolio P&L |
| AI assistance | BYOK (user pastes own OpenAI / Anthropic / Gemini key). Toggleable on/off. Roles: market commentary, news summary, chart/indicator analysis. **No portfolio coaching in v1.** |
| Alerts | Minimum viable — price threshold (above/below) only in v1. Expand later. |
| Architecture pattern | Layered with provider abstraction (chosen over monolithic and plugin-based) |
| Discipline | TDD (red-green-refactor) and DDD (pure domain, bounded contexts) throughout |
| Progress tracking | `docs/CONTEXT.md`, `docs/progress.md`, `docs/adr/NNNN-*.md` updated continuously |

## 3. Milestones

The full architecture below is designed up front; implementation proceeds in three milestones.

### M1 — Core (no AI)

- Tauri app skeleton (main window + floating widget + transparency control)
- `AssetProvider` trait + Crypto (Binance + CoinGecko) + US stocks (Yahoo + Finnhub) implementations
- Watchlist + basic price statistics display
- Local persistence (SQLite for watchlist/settings, OS keychain for secrets)
- Portfolio holdings input + real-time valuation and P&L

### M2 — Indicators, alerts, more sources

- Technical indicators: MA, RSI, MACD, Bollinger
- Price-threshold alerts + desktop notifications
- KR stocks integration (Naver Finance + optional 한국투자 OpenAPI)
- Forex / commodities

### M3 — AI

- `AiProvider` trait + OpenAI, Anthropic, Gemini implementations
- BYOK key entry + secure storage + AI on/off toggle
- Prompt templates for commentary, news summary, chart analysis
- News fetching (`NewsProvider` — Yahoo News RSS, CoinDesk RSS, extensible)

## 4. Architecture overview

Tauri = Rust main process (the backend) plus a webview rendering the React UI. They communicate over Tauri IPC (typed commands + event channels).

```
┌─────────────────────────────────────────────────────────┐
│ FRONTEND (Webview · React + TypeScript)                 │
│  ┌─────────────┐         ┌────────────────┐             │
│  │ Main Window │         │ Floating Widget│             │
│  └─────────────┘         └────────────────┘             │
│  State (Zustand/Jotai) · shadcn/ui · lightweight-charts │
└───────────────────────────▲─────────────────────────────┘
                            │ Tauri IPC (commands + events)
┌───────────────────────────▼─────────────────────────────┐
│ BACKEND (Rust · Tauri main process)                     │
│                                                         │
│  Orchestration (application/)                           │
│   MarketService · PortfolioService · AlertService · AiService │
│                                                         │
│  Domain (domain/) — pure, no IO                         │
│   Value Objects · Entities · Domain Services            │
│                                                         │
│  Ports (application/ traits)                            │
│   AssetProvider · AiProvider · NewsProvider · Repos     │
│   Clock · Notifier · SecretStore                        │
│                                                         │
│  Adapters (infrastructure/)                             │
│   BinanceProvider · YahooProvider · OpenAiProvider ...  │
│   SqliteRepos · KeyringSecretStore · TauriNotifier      │
│                                                         │
│  Infra: SQLite (sqlx) · keyring · tokio scheduler ·     │
│         reqwest · tracing                               │
└───────────────────────────▲─────────────────────────────┘
                            │ HTTPS
┌───────────────────────────▼─────────────────────────────┐
│ EXTERNAL                                                │
│  Binance · Upbit · CoinGecko · Yahoo · Finnhub          │
│  한국투자 OpenAPI / Naver 금융                            │
│  OpenAI · Anthropic · Gemini                            │
└─────────────────────────────────────────────────────────┘
```

Key principles:

- **Every external dependency hides behind a trait.** Mock at any seam during testing. Replace a broken adapter (e.g., a KR stock source) without touching domain or application code.
- **Domain logic is pure functions.** Deterministic, fast unit tests.
- **The orchestration layer is the only thing exposed over IPC.** The frontend calls service methods; everything else is internal.
- **Polling is centralized in the backend.** Both windows subscribe to the same event stream — no duplicate fetches.

## 5. DDD layers and bounded contexts

### Dependency rule

Onion / hexagonal style. Dependencies point inward. The CI enforces this via `cargo-deny` (forbidden imports per crate).

- `domain/` — depends on `std` only. No tokio, no reqwest, no sqlx, no tauri. Pure data + pure functions.
- `application/` — depends on `domain/`. Defines `trait` ports. May use async (tokio) but never HTTP / DB / OS directly.
- `infrastructure/` — depends on `application/` and `domain/`. **The only place** `reqwest`, `sqlx`, `keyring`, `tauri::*`, RSS parsers, etc. appear.

### Bounded contexts

1. **Market Data** — `Asset`, `Symbol`, `Quote`, `Candle`, `IndicatorEngine`. The core. Fetches, caches, computes indicators.
2. **Portfolio** — `Holding`, `CostBasis`, `PortfolioCalc`. Holdings → real-time valuation, P&L, drawdown. Reads `Quote` from Market Data.
3. **Alerts** — `AlertRule`, `AlertEvaluator`. Evaluates rules on each quote, emits notifications.
4. **AI Assistance** — `AiProvider`, `PromptTemplate`. Reads from the other contexts (read-only) and produces explanation text.

### Domain catalog

**Value Objects:** `Money` (amount + currency), `Symbol`, `Price`, `Quantity`, `Percent`, `TimeRange`.

`Symbol` is canonical (e.g., `Symbol::new("BTC", AssetKind::Crypto, Some("USD"))` → conceptual identity). Each `AssetProvider` adapter owns the translation between canonical symbols and its native format (`Binance: BTCUSDT`, `CoinGecko: bitcoin`, `Yahoo: AAPL`, `Naver: 005930.KS`). The domain never sees provider-specific strings.

**Entities:** `Asset`, `Holding`, `Quote`, `Candle`, `AlertRule`.

**Aggregates:** `Watchlist`, `Portfolio`.

**Domain Services (pure):** `IndicatorEngine` (MA, RSI, MACD, Bollinger), `PortfolioCalc` (valuation, P&L, drawdown), `AlertEvaluator` (predicate + cooldown), `QuoteSanityCheck` (outlier rejection), `PromptTemplate` (prompt construction).

### Application catalog

**Services (use cases, exposed via IPC):** `MarketService`, `PortfolioService`, `AlertService`, `AiService`, `SettingsService`.

**Ports (traits):**

- `AssetProvider` — `fetch_quotes(symbols) -> Vec<Quote>`, `fetch_candles(symbol, range) -> Vec<Candle>`
- `AiProvider` — `complete_stream(prompt) -> impl Stream<Chunk>`
- `NewsProvider` — `fetch(symbol) -> Vec<Headline>`
- `WatchlistRepo`, `PortfolioRepo`, `AlertRepo`, `SettingsRepo`
- `SecretStore` — `get(key)`, `set(key, value)`, `delete(key)`
- `Clock` — `now()`, `sleep(d)`
- `Notifier` — `notify(title, body)`
- `HttpClient` — `get(url, headers) -> Response` (lets infrastructure tests inject a fake)

## 6. Component breakdown

### Backend (Rust workspace)

```
crates/
  domain/           # pure
  application/      # services + traits
  infrastructure/   # adapters
  app/              # Tauri main binary, wires it all up
```

### Frontend (`src/`)

- `windows/main/` — dashboard (watchlist sidebar, detail pane, charts, stats, portfolio panel, settings)
- `windows/widget/` — floating mini-widget (compact rows, transparency slider, drag, pin)
- `lib/ipc.ts` — typed bindings to Tauri commands + event subscriptions
- `lib/state/` — Zustand stores per concern (watchlist, portfolio, alerts, ai, settings)
- `components/charts/` — `lightweight-charts` wrappers
- `components/ai/` — chat-style streaming response panel
- `i18n/` — KR + EN

### Persistence schema (SQLite)

Tables: `watchlist`, `holdings`, `lots` (for tax-lot cost basis), `alerts`, `alert_history`, `settings`, `cache_quotes`, `cache_candles`. Migrations via `sqlx::migrate!`.

Secrets are **not** in SQLite. They live in the OS keychain via the `keyring` crate (`SecretStore` port).

## 7. Data flow

### Quote polling → UI update (most frequent)

1. `PollScheduler` (tokio interval) ticks at the user's configured interval (default 5s, floor 1s).
2. `MarketService.refresh_watchlist()` looks up which providers cover which symbols.
3. Per-provider `AssetProvider::fetch_quotes(symbols)` (parallel where possible).
4. Each `Quote` runs through `QuoteSanityCheck` (rejects 10×-style jumps).
5. `IndicatorEngine.compute(candles)` updates derived series.
6. `MarketService` emits a `quote-update` IPC event (per symbol or batched).
7. Both windows subscribe; React rerenders only affected components.

### Portfolio recalc

Triggered by `quote-update`. `PortfolioService.recalc()` reads holdings → calls `PortfolioCalc.evaluate(holdings, quotes)` (pure) → emits `portfolio-update` IPC event.

### Alert evaluation

Triggered by `quote-update`. `AlertService.evaluate(quote)` reads applicable rules → `AlertEvaluator.check(rule, quote, last_state)` (pure, includes cooldown) → on trigger calls `Notifier.send` and persists `alert_history`.

### AI explanation (on user request)

1. UI invokes `AiService.explain(symbol)`.
2. `AiService` collects context: snapshot from `MarketService`, headlines from `NewsProvider`.
3. `PromptTemplate.build(snapshot, headlines)` produces a prompt (pure).
4. `SecretStore.get("ai_provider_key")` fetches the API key (never appears in IPC payloads or logs).
5. `AiProvider.complete_stream(prompt)` streams chunks back; IPC forwards them to the UI which renders incrementally.

## 8. Error handling

Failure categories and responses:

### External quote APIs

- Network down → exponential backoff (1s → 30s cap), keep last successful quote, show "stale" indicator.
- 5xx → backoff + retry (max 3), then put that provider in a 5-minute circuit-breaker. If another provider covers the symbol, fall over.
- 429 → respect `Retry-After`, dynamically widen the poll interval.
- Schema change (deserialize fails) → skip that symbol only, structured log, one-time toast.
- Outlier (10×+ jump) → discard suspicious quote; accept if persists across two polls.
- Timeout (5s hard) → skip this tick, wait for the next.

### AI providers

- Key missing → AI toggle disabled, prompt to set key in settings.
- 401/403 → mark key invalid, force AI off, modal asking the user to re-enter.
- 429 → retry once with `Retry-After`, then surface "AI rate limit hit, try later".
- Stream interrupted → preserve partial response, show "[disconnected]" marker, expose a Retry.
- Token-limit overflow → auto-shrink prompt (drop oldest news first), retry once.

### Local storage

- SQLite busy/IO → retry 3×, fall back to read-only mode with a write queue, show "saving deferred" indicator.
- DB corruption → attempt restore from last backup; if it fails, modal with explicit choice (restore manual backup / start fresh).
- Keychain access denied → AI / secret-dependent features off. **No file fallback** — security trumps convenience.

### Domain validation

- Invalid symbol / negative quantity / negative money → rejected in the Value Object constructor (`Result<Money, MoneyError>` etc).
- Cross-currency arithmetic → `Money + Money` of different currencies returns `Err`; UI offers explicit conversion.

### System events

- Sleep / wake → pause polling, on wake do one immediate poll then resume.
- Network up / down (reachability) → reset backoff and poll immediately on recovery.
- App quit → abort in-flight polls, flush DB, persist window position + transparency setting.

### Cross-cutting

- Errors use `thiserror` per-domain enums. `anyhow` only at IPC boundary / `main`.
- Logs via `tracing` to a rotating JSON file (max 50 MB). **Never** log API keys or AI prompts containing secrets.
- All user-visible messages go through i18n keys; technical detail hides behind a "Details" expander.
- Each provider has a circuit breaker (N consecutive failures → K-minute cooldown).

## 9. Testing strategy

### Pyramid

- **Unit (~75%)** — domain pure functions + application services with mocked traits. Run on every save. Sub-millisecond per test.
- **Integration (~20%)** — adapters against `wiremock` for HTTP, real SQLite via `tempfile` for repos.
- **E2E (~5%)** — `tauri-driver` (WebDriver) + WebdriverIO against a dev build wired to wiremock. Golden paths only. (Playwright doesn't drive Tauri's native window; we use the official WebDriver bridge instead.)

### Per layer

| Layer | Approach | Tools |
|---|---|---|
| `domain/` | Pure function tests + property tests | `cargo test`, `proptest`, `insta` (snapshots) |
| `application/` | Services with mock traits | `mockall::automock` |
| `infrastructure/` (HTTP) | Real adapter against a local mock server | `wiremock` |
| `infrastructure/` (DB) | Real sqlite via temp files | `sqlx::test`, `tempfile` |
| Frontend components | Render + interaction tests | `vitest`, `@testing-library/react` |
| Frontend ↔ Tauri | Mock the IPC bridge | `@tauri-apps/api/mocks` |
| E2E | Built app + mocked external network | `tauri-driver` + WebdriverIO |

### TDD discipline

Every feature, every bug:

1. **RED** — write a failing test that names the behavior. Starting from the domain when possible.
2. **GREEN** — minimum code to pass. Fake implementations are fine at this step.
3. **REFACTOR** — pull out Value Objects, dedupe, rename. Tests stay green and stay the same.
4. Repeat with the next behavior.

### Structures that make TDD natural

- Domain has zero external deps → tests are deterministic.
- Every trait is a mock seam — `mockall::automock` on `AssetProvider`, `AiProvider`, `Clock`, `Notifier`, every Repo.
- Time only flows through the `Clock` trait — cooldowns, poll intervals, "stale" timing all deterministic in tests.
- HTTP only flows through `HttpClient` (or providers using it) — unit tests stub, integration tests use wiremock.

### Special techniques

- **Property-based (`proptest`):** RSI ∈ [0, 100] for any candle series; P&L sign consistency under price flips; `Money` arithmetic invariants.
- **Snapshot (`insta`):** prompt template output, IPC payload serialization.
- **Contract tests:** real-API tests per provider, marked `#[ignore]`, run weekly in CI to catch upstream schema changes early.
- **Mutation testing (`cargo-mutants`):** scheduled run against `domain/` only, to check test quality.

### CI gates

- `cargo fmt --check` and `cargo clippy -D warnings`
- `cargo test --workspace`
- `cargo-deny` — forbidden-import rules enforce the layer boundary
- `npm run typecheck`, `npm run test`, ESLint
- `tauri-driver` E2E on PRs targeting `main` and on release branches
- Coverage report (`cargo-tarpaulin`) — informational, no enforced threshold

## 10. Security & privacy

- BYOK API keys live only in the OS keychain. They never appear in SQLite, JSON logs, or IPC payloads.
- The app makes outbound HTTPS requests only — no inbound ports.
- AI requests include only the data the user implicitly asked about (the symbol's snapshot + recent headlines). No telemetry, no analytics in v1.
- News feeds are public RSS — no auth, no personal data sent.

## 11. Progress documentation (required, ongoing)

Three documents live under `docs/` and must stay current:

- **`docs/CONTEXT.md`** — ubiquitous language (domain vocabulary), current architectural state, active bounded contexts. Refreshed at each milestone boundary or after any vocabulary change.
- **`docs/progress.md`** — chronological work log. One entry per merged task: date, what shipped, which tests cover it, what's next.
- **`docs/adr/NNNN-title.md`** — one ADR per non-trivial architectural decision (e.g., "use polling instead of WebSocket in v1", "choose Yahoo as default US provider").

Every PR / task completion includes a checklist item: which of these three was updated?

## 12. Open risks

- **Korean stocks free real-time data is genuinely scarce.** Plan: scrape Naver Finance (best-effort, isolated behind `NaverKrProvider`); optional 한국투자 OpenAPI integration for users with brokerage accounts; treat any single KR adapter as replaceable. Surface "delayed" badging when source is delayed.
- **Yahoo Finance has no official public API and may rate-limit / change.** Treat as best-effort; layered behind the trait so we can switch to Finnhub free tier or another provider if Yahoo breaks.
- **AI provider streaming APIs differ in shape.** Adapter layer normalizes to a single `Chunk` type to keep `AiService` clean.
- **Floating-widget transparency on Windows** has historical quirks (compositor differences). Plan to validate early in M1.

## 13. Non-goals (for v1)

- Order placement / brokerage trading
- Portfolio AI coaching
- Mobile clients
- Cloud sync between devices
- Hosted AI (BYOK only)
- Backtesting

These may be revisited after M3.
