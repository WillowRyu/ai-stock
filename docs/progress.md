# Progress Log

## 2026-05-13

- Spec approved (`docs/superpowers/specs/2026-05-13-ai-stock-design.md`).
- M1 plan written (`docs/superpowers/plans/2026-05-13-ai-stock-m1-core.md`).

### Phase 0 — Scaffolding

- [x] Task 0.1: Rust workspace + Tauri shell.
- [x] Task 0.2: Vite + React + Tailwind frontend.
- [ ] Task 0.3: Docs skeleton + ADR 0001.
- [ ] Task 0.4: CI pipeline.
- [ ] Task 0.5: cargo-deny layer enforcement.

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
