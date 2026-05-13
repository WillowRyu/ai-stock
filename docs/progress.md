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
