# ADR 0001 — Canonical Symbol with per-provider translation

- **Status:** Accepted
- **Date:** 2026-05-13

## Context

External providers each use different symbol conventions: `BTCUSDT` (Binance), `bitcoin` (CoinGecko), `AAPL` (Yahoo), `005930.KS` (Naver). If provider-specific strings leak into the domain or storage, switching providers becomes a schema migration.

## Decision

The domain owns a canonical `Symbol` value object: kind + ticker + optional quote currency. Each `AssetProvider` adapter is responsible for translating canonical `Symbol`s into and from its native format. The domain and persistence layer see only canonical Symbols.

## Consequences

- Switching providers is a one-file change.
- Storage is stable across provider swaps.
- Adapters have non-trivial mapping logic — covered by adapter unit tests.
- Adding a new asset class may require extending `AssetKind` (a breaking change to stored data — track via migrations).
