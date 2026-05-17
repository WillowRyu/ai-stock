# ADR 0005 — KIS Open API provider for KR stocks (BYOK brokerage credentials)

- **Status:** Accepted
- **Date:** 2026-05-13 (post-M3 polish; logged retroactively 2026-05-17)

## Context

ADR 0003 shipped KR stock coverage by scraping Naver Finance HTML and explicitly
flagged it as fragile. It listed `KisOpenApi` — letting users connect their own
한국투자증권 (Korea Investment & Securities) brokerage account — as a deferred
mitigation. Post-M3 polish implemented it as `KisProvider`
(`crates/infrastructure/src/providers/kis.rs`).

Two design questions had to be answered:

1. **Where do brokerage credentials live?** KIS Open API requires an app key + app
   secret per user. These are user-owned secrets, exactly like AI keys in ADR 0004.
2. **How to authenticate?** KIS uses OAuth2 `client_credentials` against
   `POST /oauth2/tokenP`, returning a bearer token with an expiry. The token must be
   cached and reused, not re-fetched per quote.

A wrinkle: the `HttpClient` application port only exposes `GET`. The token exchange
is a `POST`, and so are some KIS quote endpoints.

## Decision

- `KisProvider` is an `AssetProvider` adapter in `infrastructure::providers::kis`.
- **BYOK brokerage credentials.** App key/secret stored in the OS keychain under
  `kis_app_key` / `kis_app_secret`, mirroring the BYOK pattern of ADR 0004. Without
  them set, the provider is simply unavailable and the app falls back to Naver.
- **OAuth token caching.** `get_token` caches the access token with its `expires_at`
  and reuses it while more than 60s of life remains; otherwise it re-runs the
  `client_credentials` exchange.
- **Tactical HTTP exception.** Rather than widen the `HttpClient` port with a `post`
  method for one adapter's needs, `KisProvider` issues its `POST` requests via a
  one-off `reqwest::Client` built inside the adapter. This exception is scoped to
  this file only and documented inline.
- `with_base(...)` constructor lets tests point at a wiremock server.

## Consequences

- KR users with a brokerage account get a real API instead of fragile scraping.
  Naver remains the no-account fallback (ADR 0003 still stands).
- The `HttpClient` port stays GET-only. Cost: `KisProvider` is not fully testable
  through the mock `HttpClient` — its POST paths need wiremock. If a second adapter
  ever needs POST, revisit and add `HttpClient::post` properly.
- KIS credentials join AI keys as user-supplied secrets in the keychain; the
  Settings UI now has a section for them.
- KIS rate limits and account requirements are the user's concern, not ours —
  consistent with the BYOK philosophy.
