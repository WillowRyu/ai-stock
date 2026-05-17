# M5 — Local Release Build — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the app buildable as a runnable macOS application for personal use, and add an `#[ignore]`d contract test that catches the Naver scraper breaking.

**Architecture:** No new production code beyond one contract test and whatever concrete fixes the release build turns out to need. Three deliverables: a Naver contract test, a verified `npm run tauri build`, and build documentation. No CI, no code signing, version stays `0.1.0` — all deferred to a public release.

**Tech Stack:** Tauri 2 (`@tauri-apps/cli`, bundler), Rust workspace, `wiremock`-style infra tests (the contract test uses a real HTTP client, not wiremock).

**Spec:** `docs/superpowers/specs/2026-05-17-m5-local-release-design.md`

---

## Task 1: Naver contract test

**Files:**
- Modify: `crates/infrastructure/src/providers/naver_kr.rs` (the `#[cfg(test)] mod tests` block — add one test).

- [ ] **Step 1: Add the contract test**

In `crates/infrastructure/src/providers/naver_kr.rs`, inside the existing
`#[cfg(test)] mod tests { ... }` block, add this test after the existing
`unsupported_interval_errors` test (it is the last test in the module):

```rust
    /// Contract test: hits the real finance.naver.com to detect when Naver
    /// changes its HTML and the quote scraper's selectors silently break.
    /// `#[ignore]`d — it needs network access and is inherently fragile, so it
    /// must not run in the default `cargo test`. Run it on demand:
    ///   cargo test -p infrastructure -- --ignored contract_real_naver_quote
    #[tokio::test]
    #[ignore = "network contract test — run manually with --ignored"]
    async fn contract_real_naver_quote() {
        let provider = NaverKrProvider::new(Arc::new(ReqwestHttpClient::new()));
        // Samsung Electronics (KOSPI) — a stable, long-lived ticker.
        let s = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let quotes = provider
            .fetch_quotes(&[s])
            .await
            .expect("Naver quote fetch failed — the scrape selectors may have broken");
        assert_eq!(quotes.len(), 1, "expected exactly one quote for 005930");
        let q = &quotes[0];
        assert!(
            q.price.money().amount() > Decimal::ZERO,
            "scraped price should be positive, got {}",
            q.price.money().amount()
        );
        assert_eq!(q.price.money().currency().as_str(), "KRW");
        assert!(
            q.display_name.as_deref().map(|n| !n.is_empty()).unwrap_or(false),
            "expected a non-empty display_name, got {:?}",
            q.display_name
        );
    }
```

Note: `NaverKrProvider`, `ReqwestHttpClient`, `Arc`, `Symbol`, `AssetKind`, and
`Decimal` are all already in scope in this test module (via `use super::*;` and
`use crate::http::ReqwestHttpClient;` at the top of the module) — do not add new
`use` lines.

- [ ] **Step 2: Confirm the test is ignored by the default suite**

Run: `cargo test -p infrastructure naver_kr`
Expected: the existing `naver_kr` tests pass, and `contract_real_naver_quote`
is listed as `ignored` (not run). The line reads roughly:
`test providers::naver_kr::tests::contract_real_naver_quote ... ignored`.

- [ ] **Step 3: Run the contract test against the real site**

Run: `cargo test -p infrastructure -- --ignored contract_real_naver_quote`
Expected: PASS — the scrape returns a positive KRW price and a non-empty name
for `005930`.

Interpreting the result:
- PASS → the scraper still works; done.
- FAIL with a network/DNS error → you are offline; this is not a code problem.
  Note it in your report and proceed (the test is still correctly in place).
- FAIL with a parse/assertion error → Naver may have changed their HTML. This
  is a real finding. STOP and report it — the scraper itself needs attention,
  which is beyond this task.

- [ ] **Step 4: Confirm the full suite is unaffected**

Run: `cargo test --workspace`
Expected: PASS — same counts as before (the new test is ignored, so the totals
are unchanged: app 0, application 15, domain 71, infrastructure 21).

- [ ] **Step 5: Commit**

```bash
git add crates/infrastructure/src/providers/naver_kr.rs
git commit -m "test(infra): ignored contract test for the Naver quote scraper"
```

---

## Task 2: Production build verification

This is a verification-and-fix task, not a TDD task — the spec
(`docs/superpowers/specs/2026-05-17-m5-local-release-design.md`, Deliverable 1)
explicitly sanctions this: the fixes a release build needs cannot be enumerated
in advance. Run the build, observe, fix concrete breakage, repeat.

**Files:**
- Modify: only whatever the build turns out to require (most likely
  `app/tauri.conf.json`, possibly nothing). Do not modify anything not needed
  to make the build succeed.

- [ ] **Step 1: Run the release build**

Run: `npm run tauri build`
This runs the frontend build (`npm run build`) then a Rust release compile and
the Tauri bundler. It is slow — allow up to ~10 minutes; use a long command
timeout. Capture the full output.

- [ ] **Step 2: Assess the outcome**

- If the build SUCCEEDED: proceed to Step 4.
- If the build FAILED: proceed to Step 3.

- [ ] **Step 3: Diagnose and fix (only if the build failed)**

Read the build output. Fix the concrete cause, then re-run `npm run tauri build`.
Repeat until it succeeds. Typical causes for a Tauri 2 macOS bundle: a missing
or mispathed bundle icon, a `tauri.conf.json` schema issue, or a frontend build
error.

Constraints:
- Make the **minimal** change that fixes the build. Do not restructure config
  or refactor.
- Do NOT add code-signing or notarization config — the spec defers signing.
- If the failure needs an architectural decision or a non-obvious change with
  multiple valid approaches, STOP and escalate (report `BLOCKED` with the build
  output) rather than guessing.

- [ ] **Step 4: Locate and launch the built app**

The Tauri crate is `app/` and the workspace `target/` is at the repo root, so
the bundle is under `target/release/bundle/`.

Run: `ls -R target/release/bundle/`
Expected: a `macos/` directory containing `ai-stock.app`, and a `dmg/`
directory containing a `.dmg`.

Launch the app:

Run: `open target/release/bundle/macos/ai-stock.app`
Expected: the app launches — the main window renders the watchlist UI. (It is
unsigned, so if macOS Gatekeeper blocks it, that confirms the unsigned-app
behavior the build doc will describe; you can still verify launch via
`open` from the terminal, which bypasses the first-run prompt, or right-click →
Open in Finder.)

- [ ] **Step 5: Commit (only if Step 3 changed files)**

If Step 3 made changes:

```bash
git add -A
git commit -m "fix(app): make the release build (npm run tauri build) succeed"
```

If the build succeeded with no changes, there is nothing to commit — record
that in your report (note that `target/` is git-ignored, so build artifacts are
correctly untracked; confirm with `git status` showing a clean tree).

---

## Task 3: Build documentation

**Files:**
- Create: `docs/RELEASE.md`
- Modify: `README.md` (the "빌드" subsection, currently lines 56–61).

- [ ] **Step 1: Create `docs/RELEASE.md`**

Create `docs/RELEASE.md` with this content (the outer fence below is 4
backticks; `docs/RELEASE.md` itself contains 3-backtick code blocks):

````markdown
# 로컬 릴리스 빌드

ai-stock를 개인용 데스크톱 앱으로 빌드하는 절차. CI·코드 서명은 아직 없으며,
빌드는 각 개발 머신에서 로컬로 수행한다.

## 사전 요구사항

- Rust ≥ 1.77 (`rustup`)
- Node.js ≥ 20
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- `npm install` 을 한 번 실행해 `@tauri-apps/cli` 등 의존성을 설치

## 빌드

```bash
npm run tauri build
```

`npm run build`(프론트엔드) → Rust 릴리스 컴파일 → Tauri 번들 순으로 진행된다.
산출물은 워크스페이스 루트의 `target/release/bundle/` 아래에 생성된다:

- `target/release/bundle/macos/ai-stock.app` — 실행 가능한 앱 번들
- `target/release/bundle/dmg/*.dmg` — 디스크 이미지

## 서명되지 않은 앱 (macOS)

이 빌드는 코드 서명이 되어 있지 않다. 처음 실행할 때 macOS Gatekeeper가
"확인되지 않은 개발자" 경고로 실행을 막을 수 있다. 우회 방법:

- Finder에서 `ai-stock.app`을 **우클릭 → 열기**, 그다음 대화상자에서 **열기**.
  한 번 허용하면 이후로는 일반 실행된다.

코드 서명·공증(notarization)은 공개 배포 시점에 추가할 예정이다.

## Windows / Linux

`tauri.conf.json`의 번들 타깃은 `all`로 설정되어 있어 Windows(`.msi`)·
Linux(`.AppImage`) 번들도 각 OS에서 빌드할 수 있다. 다만 현재 그 두 플랫폼의
빌드는 검증되지 않았다(개발 환경이 macOS).

## 네이버 스크래퍼 contract test

KR 종목 시세는 `finance.naver.com` HTML 스크래핑에 의존한다. 네이버가 마크업을
바꾸면 스크래퍼가 조용히 깨질 수 있다. 이를 감지하는 `#[ignore]` contract
test가 있다 — 네트워크가 필요하고 기본 `cargo test`에서는 제외된다:

```bash
cargo test -p infrastructure -- --ignored contract_real_naver_quote
```

통과하면 스크래퍼가 여전히 동작하는 것이다. 파싱/단언 실패면 네이버가 HTML을
바꿨을 가능성이 높다.
````

- [ ] **Step 2: Fix the README build section**

In `README.md`, the "빌드" subsection currently reads (4-backtick outer fence;
the content has a 3-backtick `bash` block):

````markdown
빌드:

```bash
npm run tauri build
# .dmg / .msi / .AppImage 가 src-tauri/target/release/bundle/ 에 생성
```
````

The path `src-tauri/target/release/bundle/` is wrong — this project's Tauri
crate is `app/`, not `src-tauri/`, and the workspace `target/` is at the repo
root. Replace that subsection with:

````markdown
빌드:

```bash
npm run tauri build
# 산출물: target/release/bundle/ (macOS: .app / .dmg)
```

로컬 릴리스 빌드 절차(사전 요구사항, 서명되지 않은 앱 실행법 등)는
[docs/RELEASE.md](docs/RELEASE.md) 참고.
````

- [ ] **Step 3: Verify the docs**

Run: `git diff --stat`
Expected: `docs/RELEASE.md` created, `README.md` modified.

Read `docs/RELEASE.md` once back and confirm it has no leftover placeholders and
that the build command and artifact paths match what Task 2 actually observed
(if Task 2 found the bundle somewhere other than `target/release/bundle/`,
correct `RELEASE.md` and the README to match reality).

- [ ] **Step 4: Commit**

```bash
git add docs/RELEASE.md README.md
git commit -m "docs: local release build guide; fix README bundle path"
```

---

## Task 4: M5 close-out

**Files:**
- Modify: `docs/progress.md`
- Modify: `docs/CONTEXT.md`

- [ ] **Step 1: Append the M5 entry to `docs/progress.md`**

Add at the very end of `docs/progress.md`:

```markdown
## 2026-05-17 (M5) — Local release build

Spec: `docs/superpowers/specs/2026-05-17-m5-local-release-design.md`.
Plan: `docs/superpowers/plans/2026-05-17-m5-local-release.md`.

- [x] Task 1: `#[ignore]`d Naver quote-scraper contract test.
- [x] Task 2: verified `npm run tauri build` produces a runnable macOS app.
- [x] Task 3: `docs/RELEASE.md` build guide; fixed README bundle path.
- [x] Task 4: progress/CONTEXT close-out.

### M5 complete

- The app builds into a runnable, unsigned macOS `.app` via `npm run tauri
  build`; the procedure is documented in `docs/RELEASE.md`.
- A network contract test (`cargo test -p infrastructure -- --ignored`) guards
  the Naver scraper against silent selector breakage.
- Deferred to a public release: CI, code signing / notarization, the `1.0.0`
  version bump, and Windows/Linux build verification.
- Next: M6 — persistent chat history (SQLite), deferred from M4.
```

- [ ] **Step 2: Update `docs/CONTEXT.md`**

In `docs/CONTEXT.md`:

(a) Change the `> Last updated:` line to:

```markdown
> Last updated: 2026-05-17 (M5 complete)
```

(b) In the `Current State` section, replace the first bullet — currently:

```markdown
- **M1 + M2 + M3 + M4 complete.** See `docs/progress.md`. 100+ backend test functions.
```

with:

```markdown
- **M1 + M2 + M3 + M4 + M5 complete.** See `docs/progress.md`. 100+ backend test
  functions. The app builds into a runnable unsigned macOS app via `npm run
  tauri build` (see `docs/RELEASE.md`).
```

- [ ] **Step 3: Verify the suite is still green**

Run: `cargo test --workspace && npm test`
Expected: PASS — backend and frontend unchanged and green (Task 1's contract
test is `#[ignore]`d, so totals are unchanged).

- [ ] **Step 4: Commit**

```bash
git add docs/progress.md docs/CONTEXT.md
git commit -m "docs: M5 close-out — progress + CONTEXT"
```

---

## Self-Review

**Spec coverage:**
- Deliverable 1 (production build verification) → Task 2. ✓
- Deliverable 2 (Naver contract test, `#[ignore]`d, `005930`, price>0 +
  non-empty name, run via `--ignored`) → Task 1. ✓
- Deliverable 3 (fix README path, add `docs/RELEASE.md` with prerequisites,
  build command, artifact path, Gatekeeper workaround, Windows/Linux untested
  note, contract-test command) → Task 3. ✓
- Version stays `0.1.0` — no task changes it. ✓
- Out of scope (CI, signing, version bump, chat persistence) — no task touches
  them. ✓
- Documentation impact (progress.md entry, CONTEXT.md one-liner, no new ADR) →
  Task 4. ✓

**Placeholder scan:** No TBD/TODO. Task 2 is intentionally a verify-and-fix task
without pre-written fix code — the spec sanctions this explicitly, and the task
gives concrete steps, constraints, and an escalation rule rather than a vague
"fix it" instruction.

**Type consistency:** Task 1's test uses `NaverKrProvider::new`,
`ReqwestHttpClient::new()`, `Symbol::new(AssetKind::KrEquity, "005930", None)`,
`fetch_quotes(&[s])`, `Quote.price.money().amount()`,
`Quote.price.money().currency().as_str()`, `Quote.display_name` — all verified
against the current `naver_kr.rs`, `asset_provider.rs`, and `quote.rs`. The
artifact path `target/release/bundle/` is used consistently in Tasks 2, 3, and
the README fix.
