# M5 — Local Release Build — Design

- **Date:** 2026-05-17
- **Status:** Approved (brainstorming complete)
- **Milestone:** M5
- **Predecessors:** M1 (core), M2 (indicators/alerts/KR), M3 (BYOK AI), post-M3
  polish, M4 (multi-turn AI assistant).

## Summary

M5 makes the app buildable as a real, runnable desktop application for personal
use, and adds a safety net against the Naver scraper silently breaking. It is
deliberately a small, light milestone: no CI, no code signing, no version
promotion — those are deferred to an eventual public release.

## Decisions (from brainstorming)

1. **Scope** — M5 is release *prep* only. Persistent chat history (deferred from
   M4) is a separate later milestone (M6).
2. **No CI** — CI restoration is explicitly excluded by the user. Release builds
   are run locally with `npm run tauri build`.
3. **No code signing** — the app ships unsigned. Signing (Apple notarization,
   Windows Authenticode) requires paid certificates and is deferred to a public
   release. The build doc records the user-side workaround for the macOS
   Gatekeeper warning.
4. **Personal use** — the goal is "build it on my Mac and use it without
   `tauri dev`," not a public 1.0. The version stays `0.1.0`.
5. **Verify-first** — the build is actually run before the build doc is written,
   so the doc reflects reality. Build fixes cannot be fully enumerated in
   advance; see the note under Deliverable 1.

## Out of Scope

- CI restoration (GitHub Actions) — excluded by the user.
- Code signing / notarization (macOS, Windows) — deferred to a public release.
- Windows / Linux packaging verification — the developer is on macOS and cannot
  verify those locally. `docs/RELEASE.md` documents them as untested.
- Promoting the version to `1.0.0` — deferred to a public release.
- Persistent chat history (SQLite) — M6.

## Deliverables

### 1. Production build verification

Run `npm run tauri build` on macOS and make it produce a working application
bundle. Tauri config lives at `app/tauri.conf.json`; the workspace `target/` is
at the repo root, so bundles land in `target/release/bundle/` — a `.app` under
`macos/` and a `.dmg` under `dmg/`.

**Acceptance:** the built `.app` launches and the app starts normally (main
window renders; the floating widget can be toggled).

**Note on uncertainty:** the specific fixes this requires cannot be fully
specified in advance — they depend on what the first real build run surfaces
(bundle config, icons, frontend build, transparent-window/vibrancy interaction
with a release build, etc.). The implementation plan's first task is therefore
"run the build, capture the output, fix the concrete breakage, repeat until the
`.app` launches." If the build surfaces a problem large enough to need its own
design decision, that is an escalation point — stop and reassess rather than
guessing.

### 2. Naver contract test

ADR 0003 flagged the Naver Finance HTML scrape as fragile and recommended an
`#[ignore]`d contract test to detect selector breakage. M5 adds it.

- Location: the test module of `crates/infrastructure/src/providers/naver_kr.rs`
  (the `NaverKrProvider`).
- The test builds a real `NaverKrProvider` (real HTTP client, real
  `finance.naver.com` base URL) and fetches a quote for a stable, long-lived KR
  ticker — Samsung Electronics, `005930` (KOSPI).
- It asserts the scrape still yields a sane result: a price greater than zero
  and a non-empty `display_name`. It does not assert an exact price (that
  changes constantly) — only that the selectors still resolve to real data.
- Marked `#[ignore]` so it does not run in the default `cargo test` (it needs
  network access and is inherently fragile). Run on demand with
  `cargo test -p infrastructure -- --ignored`.

This is the M5 scope for the contract test: one quote test covering the
selector-dependent scrape. The fchart candle endpoint is not covered here.

### 3. Build documentation

- **Fix the README.** Its build section currently points bundles to
  `src-tauri/target/release/bundle/`. This project's Tauri crate is `app/`, not
  `src-tauri/`, and the workspace `target/` is at the repo root — the correct
  path is `target/release/bundle/`. Correct it and link to `docs/RELEASE.md`.
- **Add `docs/RELEASE.md`** — the local build/release procedure:
  - Prerequisites (Rust toolchain, Node, `@tauri-apps/cli` via `npm`, Xcode
    command-line tools on macOS).
  - The build command (`npm run tauri build`) and where artifacts land
    (`target/release/bundle/`).
  - The unsigned-app caveat: the macOS Gatekeeper warning and the user-side
    workaround (right-click → Open on first launch). A note that Windows/Linux
    bundles are configured but untested.
  - How to run the Naver contract test
    (`cargo test -p infrastructure -- --ignored`).

## Testing Strategy

- The Naver contract test is `#[ignore]`d — it does not run in `cargo test`, so
  the existing suite (107 backend + 6 frontend) stays green and unaffected.
- Build verification is manual: run the build, launch the `.app`, confirm the
  app starts.
- No other new automated tests — M5 adds no production code beyond the test
  itself and any concrete build fixes.

## Documentation Impact

- `docs/progress.md`: an M5 entry.
- `docs/CONTEXT.md`: a one-line "Current State" update noting M5 (local release
  build). No ubiquitous-language changes — M5 adds no domain concepts.
- No new ADR. Skipping CI and signing are "defer" decisions, not architectural
  ones; ADR 0003 already recorded the contract-test recommendation.
