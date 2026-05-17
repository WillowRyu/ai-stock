# Progress Log

## 2026-05-13

- Spec approved (`docs/superpowers/specs/2026-05-13-ai-stock-design.md`).
- M1 plan written (`docs/superpowers/plans/2026-05-13-ai-stock-m1-core.md`).

### Phase 0 — Scaffolding

- [x] Task 0.1: Rust workspace + Tauri shell.
- [x] Task 0.2: Vite + React + Tailwind frontend.
- [x] Task 0.3: Docs skeleton + ADR 0001.
- [x] Task 0.4: CI pipeline.
- [x] Task 0.5: cargo-deny layer enforcement.

### Phase 1 — Domain layer

- [x] Task 1.1: Money + currency-checked arithmetic.
- [x] Task 1.2: Symbol + AssetKind.
- [x] Task 1.3: Quantity, Percent, Price, TimeRange.
- [x] Task 1.4: Quote, Candle.
- [x] Task 1.5: Holding, Watchlist, Portfolio.
- [x] Task 1.6: QuoteSanityCheck, PortfolioCalc.

### Phase 2 — Application layer

- [x] Task 2.1: Clock, HttpClient, SecretStore, Notifier ports.
- [x] Task 2.2: AssetProvider, NewsProvider ports.
- [x] Task 2.3: Repo ports + AppSettings.
- [x] Task 2.4: MarketService.
- [x] Task 2.5: PortfolioService.
- [x] Task 2.6: SettingsService.
- [x] Task 2.7: PollScheduler.

### Phase 3 — Infrastructure adapters

- [x] Task 3.1: SystemClock + ReqwestHttpClient.
- [x] Task 3.2: KeyringSecretStore.
- [x] Task 3.3: SqliteWatchlistRepo + migrations.
- [x] Task 3.4: SqlitePortfolioRepo.
- [x] Task 3.5: SqliteSettingsRepo.
- [x] Task 3.6: BinanceProvider.
- [x] Task 3.7: CoinGeckoProvider.
- [x] Task 3.8: YahooProvider.
- [x] Task 3.9: FinnhubProvider.

### Phase 4 — Tauri wiring

- [x] Task 4.1: AppState wiring.
- [x] Task 4.2: IPC commands + quote-update broadcast.

### Phase 5 — Frontend

- [x] Task 5.1: Typed IPC bindings.
- [x] Task 5.2: Zustand stores.
- [x] Task 5.3: Watchlist + DetailPane + AddSymbol.
- [x] Task 5.4: PortfolioPanel + Settings + i18n stubs.
- [x] Task 5.5: Floating widget with transparency.

### Phase 6 — E2E + close-out

- [x] Task 6.1: tauri-driver E2E smoke.
- [x] Task 6.2: M1 close-out — ADR 0002, CONTEXT update.

## 2026-05-13 — M1 complete

- Working app: cross-platform Tauri shell, hybrid main + floating widget, 4 asset adapters (Binance, CoinGecko, Yahoo, Finnhub), portfolio P&L.
- Tests: ~45 backend unit tests (domain pure + application with mocks + infra via wiremock/sqlite temp), 1 frontend Zustand store test, 1 E2E golden-path smoke.
- DDD layer boundary enforced via scripts/check-layer-boundary.sh (cargo-deny `wrappers` semantics didn't match the plan; pivoted in commit 47b3058).
- Currency value object widened to 3-5 char ASCII to support USDT/USDC.
- Next: draft M2 plan (KR stocks via Naver, technical indicators, alerts + notifications, forex/commodities polish).

## 2026-05-13 (M2)

### Phase 7 — Technical indicators

- [x] Task 7.1: SMA (windowed sum).
- [x] Task 7.2: EMA (alpha smoothing from SMA seed).
- [x] Task 7.3: RSI (Wilder smoothing, proptest invariant).
- [x] Task 7.4: MACD(12,26,9) + Bollinger(20, 2).
- [x] Task 7.5: IndicatorService + indicators_for IPC.

### Phase 8 — Alerts

- [x] Task 8.1: AlertRule + AlertEvaluator (pure).
- [x] Task 8.2: alerts migration + AlertRepo + SqliteAlertRepo.
- [x] Task 8.3: AlertService.
- [x] Task 8.4: TauriNotifier (app crate).
- [x] Task 8.5: Wire into poll loop + IPC commands.
- [x] Task 8.6: Frontend AlertsPanel.

### Phase 9 — KR stocks

- [x] Task 9.1: NaverKrProvider scraping finance.naver.com.

### Phase 10 — Hygiene + close-out

- [x] Task 10.1: Poll interval from settings at startup.
- [x] Task 10.2: Tighten CSP to explicit provider origins.
- [x] Task 10.3: ADR 0003 + CONTEXT update + this progress entry.

## 2026-05-13 — M2 complete

- Indicators in domain, alerts bounded context, KR stocks via Naver, CSP tightened, settings-driven poll.
- Tests: ~64 backend unit tests (~46 domain + 8 application + 10 infrastructure), 1 frontend, E2E unchanged.
- Next: draft M3 plan (BYOK AI, news, commentary).

## 2026-05-13 (M3)

### Phase 11 — AI provider trait + adapters

- [x] Task 11.1: AiProvider port (streaming) + ports/mod.rs.
- [x] Task 11.2: OpenAiProvider.
- [x] Task 11.3: AnthropicProvider.
- [x] Task 11.4: GeminiProvider.

### Phase 12 — News providers

- [x] Task 12.1: YahooNewsRss.
- [x] Task 12.2: CoinDeskRss with symbol-alias filtering.

### Phase 13 — AI service + prompt templates

- [x] Task 13.1: PromptTemplate (domain) + AiService (application).

### Phase 14 — Wiring + IPC

- [x] Task 14.1: AiService wired in AppState; ai_* commands + ai-chunk/done/error events.

### Phase 15 — Frontend

- [x] Task 15.1: AiPanel + BYOK in Settings.

### Phase 16 — Close-out

- [x] Task 16.1: ADR 0004 + CONTEXT update + this entry.

## 2026-05-13 — M3 complete

- BYOK AI commentary streaming to the UI for any watchlist symbol.
- ~71 backend unit tests (including 3 AI streaming wiremock tests and 2 RSS parsing tests), 1 frontend test.
- Tauri app: M1 (core) + M2 (indicators/alerts/KR) + M3 (AI) all working together.
- Next: post-M3 polish.

## 2026-05-13 / 2026-05-14 — Post-M3 polish

Work done after the M3 close-out (`e34c90c`), grouped by theme. Logged retroactively
on 2026-05-17 — these commits were made but not recorded at the time.

### Charts

- `compute_series` (application) + `chart_data` IPC for chart overlays (`e9f0360`).
- ChartPanel: candlestick + SMA/RSI/MACD subpanes (`ec4ab1d`), embedded in DetailPane (`5de0c0b`).
- Volume bars, indicator visibility toggles, axis formatting (`33e16af`).
- Configurable candle interval — 1m / 5m / 30m / 1h / 1d / 1w (`8cbb186`).

### Indicator alerts

- Domain: `AlertCondition` extended with RSI/MACD conditions + `EvalContext` (`f41353a`).
- Wired RSI/MACD-cross alerts end-to-end (`20b3ddf`) with frontend UI (`ed44bc7`).

### Multi-currency portfolio

- Domain: `FxRates` value object + cross-currency portfolio aggregation (`84cf2dd`).
- `FxRateBook` wired into PortfolioService with periodic Yahoo refresh (`79cb2b9`).

### KR data sources

- `KisProvider` — 한국투자증권 OAuth tokenP + inquire-price endpoint (`f068dab`).
- KIS BYOK credential IPC commands (`1687bf3`) + Settings UI (`e961f11`).
- Naver candle endpoint + KR `display_name` scraping (`9c30aa2`); EUC-KR decoding fix (`c5a82e0`).

### Provider robustness

- Per-symbol `provider-error` events (`72e4aaa`); stale indicator + error toasts in UI (`5d581f3`).
- Yahoo quotes routed through v8/chart meta instead of 401-gated v7/quote (`e1e6f65`).
- Capabilities + provider fallback + candle provider fallback fixes (`0bc82b5`, `1760023`).
- Live poll-interval setting re-read each tick (`f601231`).

### UI polish + theming

- Cross-platform window vibrancy + transparent main window (`3422ad4`, `47f3dc6`).
- Theme store: light/dark/system with flash-free apply (`9cce420`); widget respects theme (`a1c5cdb`).
- Glass surfaces / dialogs / tinted toasts / chart chrome (`04a57c6`, `7912f8b`, `d300e47`).
- Custom Select component (`6f36fe8`); Settings dialog redesigned with sectioned layout (`6c6979a`, `18c2009`).
- Empty-state guides (`cb34719`), localized timestamps + timezone picker (`e04809a`),
  `toLocaleString` price/money formatting (`b123565`), notification-permission flow (`6ac5162`).

### Housekeeping

- App icon (candlestick design) + proper README (`6b26134`).
- `.eslintrc.cjs` so `npm run lint` runs (`c3864e0`); `.gitignore` expanded (`55756b0`).
- Removed GitHub Actions workflows (`1a456a0`).

### State after polish

- 92 backend test functions (`#[test]` / `#[tokio::test]`), 2 frontend test/spec files.
- Working tree clean; HEAD at `55756b0`.

## 2026-05-17 — Documentation backfill

- ADR 0005 written — KIS Open API provider for KR stocks (BYOK brokerage credentials).
- ADR 0006 written — FxRates value object + cross-currency portfolio aggregation.
- `docs/CONTEXT.md` updated to post-M3-polish state + ADR index added.
- Post-M3 polish work (above) logged retroactively from git history.

## 2026-05-17 — Test suite verification (post-polish green check)

First full-suite run recorded since the post-M3 polish pass.

- **Backend** (`cargo test --workspace`): 93 passed, 0 failed, 0 ignored
  (domain 62, application 10, infrastructure 21, app crate 0).
- **Frontend** (`vitest run`): 2 passed, 0 failed.
- **Typecheck** (`tsc -b --noEmit`): clean.
- **Lint** (`eslint`): 0 errors, 4 warnings (all pre-existing in `e2e/` files).
- **E2E**: NOT run — no execution path currently. `tauri-driver` is unsupported on
  macOS (WebDriver has no macOS WKWebView backend), and the CI workflows that ran
  E2E on Linux were removed in `1a456a0`. Restoring an E2E path is 1.0-release work
  (reinstate CI on Linux, or document E2E as Linux/Windows-only).
- Verdict: backend + frontend + typecheck green. Safe to proceed to M4.
