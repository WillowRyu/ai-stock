# ADR 0002 — Polling only for M1 (no WebSocket streaming)

- **Status:** Accepted
- **Date:** 2026-05-13

## Context

Streaming quote feeds (Binance/Upbit WebSocket, IEX SIP, etc.) provide sub-second updates but require per-provider WS lifecycles, reconnect strategies, and additional native dependencies. Many providers (Yahoo, Finnhub free tier) do not offer streaming at all.

## Decision

M1 uses HTTP polling (default 5 s, user-configurable, floor 1 s) uniformly across all providers. A `PollScheduler` in the application layer drives a `MarketService::refresh()` call.

## Consequences

- Simpler implementation; one mental model.
- All four asset classes use the same path; no provider-specific WS layer.
- Real-time feel is "good enough" for casual viewing; active day-trading would want streaming (deferred to a future revisit).
