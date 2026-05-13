# ADR 0003 — Grep-based layer check, Naver scraping for KR stocks

- **Status:** Accepted
- **Date:** 2026-05-13 (M2)

## Context

Two M2 architectural calls worth recording:

### Layer enforcement
cargo-deny's `wrappers` field has inverse semantics from what M1's plan assumed. M1 pivoted to a shell script `scripts/check-layer-boundary.sh` that greps for forbidden direct deps. M2 leaves this in place — it's good enough, cheap, and runs on every CI build.

### KR stock data source
Real-time free KR stock data is scarce. The M2 implementation scrapes Naver Finance HTML via the `scraper` crate. This is fragile (Naver could change selectors) but unblocks KR coverage for v1 users without requiring a brokerage account. Production paths to mitigate fragility:
- Contract test (#[ignore], weekly CI) hitting the real Naver site to detect selector breakage.
- Add `KisOpenApi` for users who connect their 한국투자증권 account (deferred to M3 or post-1.0).
- Fall back to a "data source unavailable" UI state when the scraper fails.

## Decision

Adopt the grep-based layer check and Naver scraping in M2. Document both as known-fragile so future contributors don't mistake them for production-grade infrastructure.

## Consequences

- KR stock data is best-effort. M2 ships with a single source; outages are visible to the user.
- Layer enforcement is grep-based — adequate for our 4-crate workspace but does not catch indirect deps.
- Future M3+ work may revisit with cargo-modules or migrate to a brokerage API.
