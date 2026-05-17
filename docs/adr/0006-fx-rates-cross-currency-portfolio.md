# ADR 0006 — FxRates value object + cross-currency portfolio aggregation

- **Status:** Accepted
- **Date:** 2026-05-13 (post-M3 polish; logged retroactively 2026-05-17)

## Context

A portfolio can hold assets denominated in different currencies: KR stocks in KRW,
US stocks in USD, crypto priced in USDT/USDC. Through M3, `PortfolioCalc` assumed
every `Holding` and `Quote` shared one currency — currency-checked `Money`
arithmetic (ADR-era Task 1.1) would simply reject a mixed-currency sum.

To report a single portfolio value and P&L, holdings must be converted into one
base currency, which means the domain needs an FX rate concept. Two constraints:

- The domain layer must stay pure (no IO) — see [[feedback-tdd-ddd-progress-log]].
  An FX *rate table* is a pure value object; *fetching* rates is infrastructure.
- Stablecoins (USDT, USDC, BUSD, DAI) trade ~1:1 with USD. Requiring an explicit
  USDT→USD rate for every crypto holding would be noise.

## Decision

- **`FxRates` — pure domain value object** (`crates/domain/src/fx.rs`). A
  `HashMap<(Currency, Currency), Decimal>` of directional rates. Cross rates are
  **not** auto-derived; the caller populates every pair it needs. `convert(money,
  target)` canonicalizes stablecoins to USD first, so a USDT-denominated `Money`
  converts to USD at par with no explicit rate. Unknown rate → `None`.
- **`FxRateBook` — application-layer wrapper** (`crates/application/src/fx_rate_book.rs`).
  An `Arc<RwLock<FxRates>>`. `AppState` owns one; a background task refreshes it
  from Yahoo FX quotes; `PortfolioService` reads a `snapshot()` per valuation call.
- `PortfolioService` converts each holding's value into the portfolio base
  currency via the snapshot before aggregating.

## Consequences

- The domain stays pure: `FxRates` is fully unit-testable with no mocks.
- Conversion is explicit and fails loudly — a missing rate yields `None` rather
  than a silently wrong total. Callers must handle the partial-data case.
- No auto-derived cross rates keeps the value object trivial, but the refresher
  task must populate each pair the portfolio actually uses (e.g. KRW→USD).
- Stablecoin canonicalization is a hard-coded list in `fx.rs`; a new stablecoin
  needs a one-line addition.
- FX refresh is best-effort (Yahoo) — same fragility class as quote providers.
  Stale or missing rates degrade the portfolio total, not the rest of the app.
