# ai-stock M1 (Core) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a working Tauri desktop app (macOS + Windows) that shows live crypto and US stock prices, supports a watchlist, and computes real-time portfolio P&L from user-entered holdings. No AI, no alerts, no KR stocks yet (those are M2/M3).

**Architecture:** Three Rust crates — `domain` (pure), `application` (traits + services), `infrastructure` (adapters: SQLite, keychain, HTTP, asset providers). One Tauri `app` binary wires everything. React/TS frontend with a main dashboard window and an always-on-top floating widget. CI enforces the layer boundary via `cargo-deny`.

**Tech Stack:** Rust (Tauri 2.x, tokio, sqlx, reqwest, keyring, thiserror, tracing, mockall, wiremock, proptest, insta), TypeScript + React 18, Vite, Tailwind, shadcn/ui, lightweight-charts, Zustand, vitest, @testing-library/react, tauri-driver + WebdriverIO.

**Reference spec:** `docs/superpowers/specs/2026-05-13-ai-stock-design.md`

---

## Conventions

- **TDD strictly.** Every behavior gets a failing test before the implementation. Red → Green → Refactor → Commit.
- **Commit after every passing test** unless the plan groups multiple steps under one commit. Small commits are good.
- **DDD layering enforced.** `domain/` imports `std` only. `application/` imports `domain/` plus `tokio`, `async-trait`, `thiserror`. `infrastructure/` is the only place `reqwest`, `sqlx`, `keyring`, `tauri::*` may appear.
- **Update `docs/progress.md` after each task block** (set of related tasks). Update `docs/CONTEXT.md` when the ubiquitous language gains or loses a term.
- **Write ADRs (`docs/adr/NNNN-title.md`) for non-trivial decisions** as they come up (e.g. "use canonical Symbol + per-provider translation").
- **Commit messages**: conventional commits (`feat:`, `test:`, `chore:`, `docs:`, `refactor:`).

---

## File structure (target end state for M1)

```
ai-stock/
├── Cargo.toml                       # workspace
├── crates/
│   ├── domain/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── money.rs
│   │       ├── symbol.rs
│   │       ├── quantity.rs
│   │       ├── percent.rs
│   │       ├── time_range.rs
│   │       ├── asset.rs
│   │       ├── quote.rs
│   │       ├── candle.rs
│   │       ├── holding.rs
│   │       ├── watchlist.rs
│   │       ├── portfolio.rs
│   │       ├── sanity.rs           # QuoteSanityCheck
│   │       └── portfolio_calc.rs
│   ├── application/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ports/
│   │       │   ├── mod.rs
│   │       │   ├── asset_provider.rs
│   │       │   ├── repos.rs
│   │       │   ├── secret_store.rs
│   │       │   ├── clock.rs
│   │       │   ├── http_client.rs
│   │       │   └── notifier.rs
│   │       ├── market_service.rs
│   │       ├── portfolio_service.rs
│   │       ├── settings_service.rs
│   │       └── poll_scheduler.rs
│   └── infrastructure/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── http.rs              # reqwest HttpClient impl
│           ├── clock.rs             # SystemClock
│           ├── sqlite/
│           │   ├── mod.rs
│           │   ├── migrations/
│           │   ├── watchlist_repo.rs
│           │   ├── portfolio_repo.rs
│           │   └── settings_repo.rs
│           ├── keyring_secrets.rs
│           └── providers/
│               ├── mod.rs
│               ├── binance.rs
│               ├── coingecko.rs
│               ├── yahoo.rs
│               └── finnhub.rs
├── app/                             # Tauri binary
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── src/
│   │   ├── main.rs
│   │   ├── ipc.rs                   # commands + event names
│   │   └── wiring.rs                # dependency assembly
│   └── icons/
├── src/                             # frontend (React)
│   ├── main.tsx
│   ├── index.html
│   ├── widget.html
│   ├── widget.tsx
│   ├── App.tsx
│   ├── lib/
│   │   ├── ipc.ts
│   │   └── state/
│   │       ├── watchlistStore.ts
│   │       ├── portfolioStore.ts
│   │       └── settingsStore.ts
│   ├── components/
│   │   ├── Watchlist.tsx
│   │   ├── DetailPane.tsx
│   │   ├── PortfolioPanel.tsx
│   │   ├── Settings.tsx
│   │   ├── PriceChart.tsx
│   │   └── widget/
│   │       └── WidgetRow.tsx
│   └── i18n/
│       ├── ko.json
│       └── en.json
├── e2e/
│   ├── wdio.conf.ts
│   └── specs/
│       └── golden-path.e2e.ts
├── docs/
│   ├── CONTEXT.md
│   ├── progress.md
│   ├── adr/
│   │   └── 0001-canonical-symbol-with-per-provider-translation.md
│   └── superpowers/
│       ├── specs/
│       └── plans/
├── deny.toml                        # cargo-deny config (layer rules)
├── .github/workflows/ci.yml
├── package.json
├── tsconfig.json
├── vite.config.ts
└── tailwind.config.ts
```

---

## Phase 0 — Scaffolding

### Task 0.1: Initialize Rust workspace and Tauri app

**Files:**
- Create: `Cargo.toml`
- Create: `app/Cargo.toml`
- Create: `app/tauri.conf.json`
- Create: `app/build.rs`
- Create: `app/src/main.rs`
- Create: `crates/domain/Cargo.toml`
- Create: `crates/domain/src/lib.rs`
- Create: `crates/application/Cargo.toml`
- Create: `crates/application/src/lib.rs`
- Create: `crates/infrastructure/Cargo.toml`
- Create: `crates/infrastructure/src/lib.rs`

- [ ] **Step 1: Install Rust + Tauri prerequisites (one-time)**

Run: `rustc --version` to confirm Rust ≥ 1.77. If missing, install via https://rustup.rs. Run `cargo install create-tauri-app --locked` (will be used for reference only — we set up by hand to fit the workspace shape).

- [ ] **Step 2: Create workspace `Cargo.toml`**

Create `Cargo.toml` at repo root:

```toml
[workspace]
resolver = "2"
members = ["crates/domain", "crates/application", "crates/infrastructure", "app"]

[workspace.package]
edition = "2021"
license = "MIT"
authors = ["WillowRyu"]

[workspace.dependencies]
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"], default-features = false }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "macros", "migrate", "chrono"] }
keyring = "2"
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = { version = "1", features = ["serde-with-str"] }
mockall = "0.12"
wiremock = "0.6"
proptest = "1"
insta = { version = "1", features = ["yaml"] }
tokio-test = "0.4"
tempfile = "3"
```

- [ ] **Step 3: Create the four crate `Cargo.toml`s**

`crates/domain/Cargo.toml`:

```toml
[package]
name = "domain"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
thiserror.workspace = true
serde.workspace = true
chrono.workspace = true
rust_decimal.workspace = true

[dev-dependencies]
proptest.workspace = true
insta.workspace = true
```

`crates/application/Cargo.toml`:

```toml
[package]
name = "application"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
domain = { path = "../domain" }
thiserror.workspace = true
async-trait.workspace = true
tokio.workspace = true
tracing.workspace = true
serde.workspace = true
chrono.workspace = true
rust_decimal.workspace = true
mockall = { workspace = true }

[dev-dependencies]
tokio-test.workspace = true
```

`crates/infrastructure/Cargo.toml`:

```toml
[package]
name = "infrastructure"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
domain = { path = "../domain" }
application = { path = "../application" }
thiserror.workspace = true
async-trait.workspace = true
tokio.workspace = true
tracing.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
rust_decimal.workspace = true
reqwest.workspace = true
sqlx.workspace = true
keyring.workspace = true

[dev-dependencies]
wiremock.workspace = true
tokio-test.workspace = true
tempfile.workspace = true
```

`app/Cargo.toml`:

```toml
[package]
name = "ai-stock-app"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
domain = { path = "../crates/domain" }
application = { path = "../crates/application" }
infrastructure = { path = "../crates/infrastructure" }
tauri = { version = "2", features = [] }
tauri-plugin-notification = "2"
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 4: Create lib.rs stubs for each crate**

`crates/domain/src/lib.rs`:

```rust
//! Pure domain layer. No IO, no async, no infra imports.
```

`crates/application/src/lib.rs`:

```rust
//! Application services and trait ports. Depends on domain only.
```

`crates/infrastructure/src/lib.rs`:

```rust
//! Adapters implementing application ports. Only place where reqwest/sqlx/keyring live.
```

- [ ] **Step 5: Create Tauri app skeleton**

`app/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

`app/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`app/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ai-stock",
  "version": "0.1.0",
  "identifier": "dev.willowryu.aistock",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "ai-stock",
        "url": "index.html",
        "width": 1100,
        "height": 720,
        "minWidth": 800,
        "minHeight": 500
      },
      {
        "label": "widget",
        "title": "ai-stock widget",
        "url": "widget.html",
        "width": 260,
        "height": 180,
        "alwaysOnTop": true,
        "decorations": false,
        "transparent": true,
        "skipTaskbar": true,
        "visible": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png"]
  }
}
```

Add a placeholder PNG: `app/icons/icon.png` (any 512×512 PNG — Tauri requires it for builds; for now use the default from `create-tauri-app` template or a transparent square).

- [ ] **Step 6: Verify the workspace builds**

Run: `cargo check --workspace`
Expected: builds cleanly (Tauri may warn about missing frontend `dist/`, that's fine — `cargo check` doesn't build the bundle).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/ app/
git commit -m "chore: scaffold tauri workspace with domain/application/infrastructure crates"
```

---

### Task 0.2: Frontend scaffolding (Vite + React + Tailwind)

**Files:**
- Create: `package.json`
- Create: `tsconfig.json`
- Create: `vite.config.ts`
- Create: `tailwind.config.ts`
- Create: `postcss.config.js`
- Create: `index.html`
- Create: `widget.html`
- Create: `src/main.tsx`
- Create: `src/widget.tsx`
- Create: `src/App.tsx`
- Create: `src/index.css`

- [ ] **Step 1: Create `package.json`**

```json
{
  "name": "ai-stock",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "typecheck": "tsc -b --noEmit",
    "test": "vitest run",
    "test:watch": "vitest",
    "lint": "eslint . --ext .ts,.tsx",
    "e2e": "wdio run e2e/wdio.conf.ts"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-notification": "^2",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "zustand": "^4.5.4",
    "lightweight-charts": "^4.2.0",
    "clsx": "^2.1.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.4.5",
    "vite": "^5.3.1",
    "tailwindcss": "^3.4.4",
    "postcss": "^8.4.39",
    "autoprefixer": "^10.4.19",
    "vitest": "^1.6.0",
    "@testing-library/react": "^16.0.0",
    "@testing-library/jest-dom": "^6.4.6",
    "jsdom": "^24.1.0",
    "eslint": "^8.57.0",
    "@typescript-eslint/parser": "^7.13.0",
    "@typescript-eslint/eslint-plugin": "^7.13.0",
    "@wdio/cli": "^9",
    "@wdio/local-runner": "^9",
    "@wdio/mocha-framework": "^9",
    "@wdio/spec-reporter": "^9",
    "ts-node": "^10.9.2"
  }
}
```

- [ ] **Step 2: Create config files**

`tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src", "e2e"]
}
```

`vite.config.ts`:

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        widget: "widget.html",
      },
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test-setup.ts",
  },
});
```

`tailwind.config.ts`:

```typescript
import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./widget.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: { extend: {} },
  plugins: [],
} satisfies Config;
```

`postcss.config.js`:

```javascript
export default {
  plugins: { tailwindcss: {}, autoprefixer: {} },
};
```

- [ ] **Step 3: Create HTML entry points**

`index.html`:

```html
<!doctype html>
<html lang="ko" class="dark">
  <head>
    <meta charset="UTF-8" />
    <title>ai-stock</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  </head>
  <body class="bg-slate-950 text-slate-100">
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`widget.html`:

```html
<!doctype html>
<html lang="ko" class="dark">
  <head>
    <meta charset="UTF-8" />
    <title>ai-stock widget</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  </head>
  <body style="background: transparent">
    <div id="root"></div>
    <script type="module" src="/src/widget.tsx"></script>
  </body>
</html>
```

- [ ] **Step 4: Create React entry stubs**

`src/main.tsx`:

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

`src/widget.tsx`:

```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";

function Widget() {
  return <div className="p-2 text-xs text-slate-200">widget</div>;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Widget />
  </React.StrictMode>,
);
```

`src/App.tsx`:

```typescript
export default function App() {
  return (
    <div className="p-6">
      <h1 className="text-2xl font-semibold">ai-stock</h1>
      <p className="text-sm text-slate-400">M1 — scaffolding</p>
    </div>
  );
}
```

`src/index.css`:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

`src/test-setup.ts`:

```typescript
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 5: Install dependencies**

Run: `npm install`
Expected: lockfile created, `node_modules/` populated.

- [ ] **Step 6: Smoke test the frontend**

Run: `npm run build`
Expected: `dist/index.html` and `dist/widget.html` produced. No TypeScript errors.

- [ ] **Step 7: Commit**

```bash
git add package.json package-lock.json tsconfig.json vite.config.ts tailwind.config.ts postcss.config.js index.html widget.html src/
git commit -m "chore: scaffold vite + react + tailwind frontend with main and widget entries"
```

---

### Task 0.3: Documentation skeleton + ADR template

**Files:**
- Create: `docs/CONTEXT.md`
- Create: `docs/progress.md`
- Create: `docs/adr/0001-canonical-symbol-with-per-provider-translation.md`

- [ ] **Step 1: Create `docs/CONTEXT.md`**

```markdown
# ai-stock — Context & Ubiquitous Language

> Last updated: 2026-05-13 (Task 0.3)

## Bounded Contexts

- **Market Data** — fetching, caching, and computing on quotes.
- **Portfolio** — holdings, valuation, P&L.
- **Alerts** — rule evaluation and notification (M2).
- **AI Assistance** — commentary/analysis (M3).

## Ubiquitous Language

| Term | Meaning |
|---|---|
| Symbol | Canonical identity of a tradable asset (kind + ticker + optional quote currency). |
| Quote | A point-in-time price observation for a Symbol. |
| Candle | OHLCV bar over a time interval. |
| Holding | A position the user owns: Symbol + Quantity + cost basis. |
| Watchlist | Aggregate of Symbols the user wants to track. |
| Portfolio | Aggregate of Holdings; can be evaluated against current Quotes. |
| Money | Decimal amount + currency. |
| Quantity | Non-negative decimal count of units. |
| Provider | External source for quotes (Binance, Yahoo, etc.) hidden behind a trait. |

## Current State

- M1 in progress — scaffolding complete (Task 0.1, 0.2).
```

- [ ] **Step 2: Create `docs/progress.md`**

```markdown
# Progress Log

## 2026-05-13

- Spec approved (`docs/superpowers/specs/2026-05-13-ai-stock-design.md`).
- M1 plan written (`docs/superpowers/plans/2026-05-13-ai-stock-m1-core.md`).

### Phase 0 — Scaffolding

- [ ] Task 0.1: Rust workspace + Tauri shell.
- [ ] Task 0.2: Vite + React + Tailwind frontend.
- [ ] Task 0.3: Docs skeleton + ADR 0001.
- [ ] Task 0.4: CI pipeline.
- [ ] Task 0.5: cargo-deny layer enforcement.
```

(Each task gets a checkbox flip when it is merged.)

- [ ] **Step 3: Create ADR 0001**

`docs/adr/0001-canonical-symbol-with-per-provider-translation.md`:

```markdown
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
```

- [ ] **Step 4: Commit**

```bash
git add docs/CONTEXT.md docs/progress.md docs/adr/
git commit -m "docs: add CONTEXT, progress log, and ADR 0001 (canonical Symbol)"
```

---

### Task 0.4: CI pipeline (GitHub Actions)

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create workflow**

`.github/workflows/ci.yml`:

```yaml
name: ci

on:
  pull_request:
  push:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  rust:
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - uses: Swatinem/rust-cache@v2
      - name: fmt
        run: cargo fmt --all -- --check
      - name: clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: test
        run: cargo test --workspace

  cargo-deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo install cargo-deny --locked
      - run: cargo deny check bans

  frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: npm }
      - run: npm ci
      - run: npm run typecheck
      - run: npm run lint
      - run: npm test
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add rust + frontend + cargo-deny pipeline"
```

(CI will fail until later tasks land the linting/tests it expects; that's fine — push happens at the end of Phase 0.)

---

### Task 0.5: `cargo-deny` enforcing the layer boundary

**Files:**
- Create: `deny.toml`

- [ ] **Step 1: Create `deny.toml`**

```toml
[bans]
multiple-versions = "warn"
wildcards = "deny"

# Domain must remain pure — no IO crates allowed.
[[bans.deny]]
name = "tokio"
[[bans.deny.wrappers]]
crate = "domain"

[[bans.deny]]
name = "reqwest"
[[bans.deny.wrappers]]
crate = "domain"

[[bans.deny]]
name = "sqlx"
[[bans.deny.wrappers]]
crate = "domain"

[[bans.deny]]
name = "tauri"
[[bans.deny.wrappers]]
crate = "domain"

[[bans.deny]]
name = "keyring"
[[bans.deny.wrappers]]
crate = "domain"

# Application must not import infrastructure crates directly.
[[bans.deny]]
name = "reqwest"
[[bans.deny.wrappers]]
crate = "application"

[[bans.deny]]
name = "sqlx"
[[bans.deny.wrappers]]
crate = "application"

[[bans.deny]]
name = "keyring"
[[bans.deny.wrappers]]
crate = "application"

[[bans.deny]]
name = "tauri"
[[bans.deny.wrappers]]
crate = "application"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Unicode-DFS-2016", "CC0-1.0", "MPL-2.0", "Zlib"]
confidence-threshold = 0.8

[advisories]
ignore = []
```

- [ ] **Step 2: Install cargo-deny locally**

Run: `cargo install cargo-deny --locked`

- [ ] **Step 3: Run check**

Run: `cargo deny check bans`
Expected: PASS (no domain/application crate imports forbidden ones yet).

- [ ] **Step 4: Commit**

```bash
git add deny.toml
git commit -m "ci: enforce ddd layer boundary via cargo-deny"
```

---

## Phase 1 — Domain layer (TDD throughout)

> Every value object/entity below follows the same loop: write the failing test, run to confirm RED, write the minimum code, run to confirm GREEN, refactor if needed, commit. Steps below show this loop explicitly for the first few; later ones compress repetitive boilerplate but keep all code.

### Task 1.1: `Money` value object (amount + currency)

**Files:**
- Create: `crates/domain/src/money.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/domain/src/money.rs`:

```rust
use rust_decimal::Decimal;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Currency([u8; 3]);

impl Currency {
    pub fn new(code: &str) -> Result<Self, MoneyError> {
        if code.len() != 3 || !code.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(MoneyError::InvalidCurrency(code.to_string()));
        }
        let bytes = code.as_bytes();
        Ok(Self([bytes[0], bytes[1], bytes[2]]))
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Money {
    amount: Decimal,
    currency: Currency,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoneyError {
    #[error("invalid currency code: {0}")]
    InvalidCurrency(String),
    #[error("currency mismatch: {0} vs {1}")]
    CurrencyMismatch(String, String),
    #[error("invalid amount: {0}")]
    InvalidAmount(String),
}

impl Money {
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }
    pub fn parse(amount: &str, currency: &str) -> Result<Self, MoneyError> {
        let amt = Decimal::from_str(amount).map_err(|_| MoneyError::InvalidAmount(amount.into()))?;
        Ok(Self { amount: amt, currency: Currency::new(currency)? })
    }
    pub fn amount(&self) -> Decimal { self.amount }
    pub fn currency(&self) -> Currency { self.currency }

    pub fn add(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(
                self.currency.as_str().into(),
                other.currency.as_str().into(),
            ));
        }
        Ok(Self { amount: self.amount + other.amount, currency: self.currency })
    }
    pub fn sub(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(
                self.currency.as_str().into(),
                other.currency.as_str().into(),
            ));
        }
        Ok(Self { amount: self.amount - other.amount, currency: self.currency })
    }
    pub fn mul_scalar(self, factor: Decimal) -> Self {
        Self { amount: self.amount * factor, currency: self.currency }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_valid_money() {
        let m = Money::parse("12.50", "USD").unwrap();
        assert_eq!(m.amount(), dec!(12.50));
        assert_eq!(m.currency().as_str(), "USD");
    }

    #[test]
    fn rejects_lowercase_currency() {
        assert!(matches!(
            Money::parse("1", "usd"),
            Err(MoneyError::InvalidCurrency(_))
        ));
    }

    #[test]
    fn rejects_invalid_amount() {
        assert!(matches!(
            Money::parse("twelve", "USD"),
            Err(MoneyError::InvalidAmount(_))
        ));
    }

    #[test]
    fn adds_same_currency() {
        let a = Money::parse("10", "USD").unwrap();
        let b = Money::parse("2.5", "USD").unwrap();
        assert_eq!(a.add(b).unwrap(), Money::parse("12.5", "USD").unwrap());
    }

    #[test]
    fn rejects_cross_currency_addition() {
        let a = Money::parse("10", "USD").unwrap();
        let b = Money::parse("10", "KRW").unwrap();
        assert!(matches!(a.add(b), Err(MoneyError::CurrencyMismatch(_, _))));
    }

    #[test]
    fn multiplies_by_scalar() {
        let a = Money::parse("3", "USD").unwrap();
        assert_eq!(a.mul_scalar(dec!(2)), Money::parse("6", "USD").unwrap());
    }
}
```

Add `rust_decimal_macros = "1"` to `crates/domain/Cargo.toml` `[dev-dependencies]` and `proptest`/`insta` were already added.

Modify `crates/domain/src/lib.rs`:

```rust
//! Pure domain layer. No IO, no async, no infra imports.
pub mod money;
```

- [ ] **Step 2: Run the tests to confirm RED**

Run: `cargo test -p domain money::`
Expected: FAIL with "unresolved import" the first time you stub out the file; iterate until tests compile, then watch them fail naturally if you remove implementations. (In TDD you'd write tests first with `unimplemented!()` stubs; the form above gives you the canonical end-state to grow into. For strict red-green, start with empty `Money` struct and add one method per test.)

- [ ] **Step 3: Verify GREEN**

Run: `cargo test -p domain money::`
Expected: all 6 tests pass.

- [ ] **Step 4: Add property-based invariant**

Append to the `tests` module:

```rust
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn addition_is_commutative(a in -1_000_000i64..1_000_000, b in -1_000_000i64..1_000_000) {
            let m1 = Money::new(Decimal::from(a), Currency::new("USD").unwrap());
            let m2 = Money::new(Decimal::from(b), Currency::new("USD").unwrap());
            prop_assert_eq!(m1.add(m2).unwrap(), m2.add(m1).unwrap());
        }
    }
```

Run: `cargo test -p domain money::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add Money value object with currency-checked arithmetic"
```

---

### Task 1.2: `Symbol` value object + `AssetKind`

**Files:**
- Create: `crates/domain/src/symbol.rs`
- Create: `crates/domain/src/asset.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/domain/src/asset.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AssetKind {
    Crypto,
    UsEquity,
    KrEquity,
    Forex,
    Commodity,
}
```

Create `crates/domain/src/symbol.rs`:

```rust
use crate::asset::AssetKind;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Symbol {
    kind: AssetKind,
    ticker: String,
    quote_currency: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SymbolError {
    #[error("ticker must be 1-20 ASCII alphanumeric/dot characters: {0}")]
    InvalidTicker(String),
    #[error("quote currency must be 3 uppercase ASCII: {0}")]
    InvalidQuoteCurrency(String),
}

impl Symbol {
    pub fn new(kind: AssetKind, ticker: &str, quote_currency: Option<&str>) -> Result<Self, SymbolError> {
        if ticker.is_empty()
            || ticker.len() > 20
            || !ticker.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
        {
            return Err(SymbolError::InvalidTicker(ticker.into()));
        }
        if let Some(qc) = quote_currency {
            if qc.len() != 3 || !qc.chars().all(|c| c.is_ascii_uppercase()) {
                return Err(SymbolError::InvalidQuoteCurrency(qc.into()));
            }
        }
        Ok(Self { kind, ticker: ticker.into(), quote_currency: quote_currency.map(|s| s.into()) })
    }
    pub fn kind(&self) -> AssetKind { self.kind }
    pub fn ticker(&self) -> &str { &self.ticker }
    pub fn quote_currency(&self) -> Option<&str> { self.quote_currency.as_deref() }

    /// Canonical string form: `kind:ticker[:quote]`
    pub fn to_canonical_string(&self) -> String {
        let prefix = match self.kind {
            AssetKind::Crypto => "crypto",
            AssetKind::UsEquity => "us",
            AssetKind::KrEquity => "kr",
            AssetKind::Forex => "fx",
            AssetKind::Commodity => "com",
        };
        match &self.quote_currency {
            Some(q) => format!("{}:{}:{}", prefix, self.ticker, q),
            None => format!("{}:{}", prefix, self.ticker),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_btc_usd() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        assert_eq!(s.ticker(), "BTC");
        assert_eq!(s.quote_currency(), Some("USD"));
        assert_eq!(s.to_canonical_string(), "crypto:BTC:USD");
    }

    #[test]
    fn creates_us_equity_without_quote() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        assert_eq!(s.to_canonical_string(), "us:AAPL");
    }

    #[test]
    fn rejects_empty_ticker() {
        assert!(Symbol::new(AssetKind::Crypto, "", Some("USD")).is_err());
    }

    #[test]
    fn rejects_lowercase_quote() {
        assert!(Symbol::new(AssetKind::Crypto, "BTC", Some("usd")).is_err());
    }

    #[test]
    fn allows_dot_in_ticker_for_kr_equity() {
        let s = Symbol::new(AssetKind::KrEquity, "005930.KS", None).unwrap();
        assert_eq!(s.ticker(), "005930.KS");
    }
}
```

Modify `crates/domain/src/lib.rs`:

```rust
//! Pure domain layer. No IO, no async, no infra imports.
pub mod asset;
pub mod money;
pub mod symbol;
```

- [ ] **Step 2: Confirm GREEN**

Run: `cargo test -p domain symbol:: asset::`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add Symbol value object and AssetKind enum"
```

---

### Task 1.3: `Quantity`, `Percent`, `Price`, `TimeRange`

**Files:**
- Create: `crates/domain/src/quantity.rs`
- Create: `crates/domain/src/percent.rs`
- Create: `crates/domain/src/price.rs`
- Create: `crates/domain/src/time_range.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: `Quantity`**

`crates/domain/src/quantity.rs`:

```rust
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Quantity(Decimal);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuantityError {
    #[error("quantity must be >= 0, got {0}")]
    Negative(Decimal),
}

impl Quantity {
    pub fn new(value: Decimal) -> Result<Self, QuantityError> {
        if value < Decimal::ZERO {
            return Err(QuantityError::Negative(value));
        }
        Ok(Self(value))
    }
    pub fn value(&self) -> Decimal { self.0 }
    pub fn zero() -> Self { Self(Decimal::ZERO) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn accepts_positive() {
        assert_eq!(Quantity::new(dec!(1.5)).unwrap().value(), dec!(1.5));
    }
    #[test]
    fn accepts_zero() {
        assert_eq!(Quantity::new(Decimal::ZERO).unwrap(), Quantity::zero());
    }
    #[test]
    fn rejects_negative() {
        assert!(matches!(Quantity::new(dec!(-0.1)), Err(QuantityError::Negative(_))));
    }
}
```

- [ ] **Step 2: `Percent`**

`crates/domain/src/percent.rs`:

```rust
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Percent(Decimal);

impl Percent {
    pub fn from_ratio(ratio: Decimal) -> Self {
        Self(ratio * Decimal::from(100))
    }
    pub fn from_value(v: Decimal) -> Self { Self(v) }
    pub fn value(&self) -> Decimal { self.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    #[test]
    fn ratio_to_percent() {
        assert_eq!(Percent::from_ratio(dec!(0.0124)).value(), dec!(1.2400));
    }
}
```

- [ ] **Step 3: `Price` (just a typed wrapper around `Money` for clarity)**

`crates/domain/src/price.rs`:

```rust
use crate::money::Money;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Price(Money);

impl Price {
    pub fn new(money: Money) -> Self { Self(money) }
    pub fn money(&self) -> Money { self.0 }
}
```

- [ ] **Step 4: `TimeRange`**

`crates/domain/src/time_range.rs`:

```rust
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimeRange { start: DateTime<Utc>, end: DateTime<Utc> }

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TimeRangeError {
    #[error("end must be >= start")]
    InvalidOrder,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self, TimeRangeError> {
        if end < start { return Err(TimeRangeError::InvalidOrder); }
        Ok(Self { start, end })
    }
    pub fn start(&self) -> DateTime<Utc> { self.start }
    pub fn end(&self) -> DateTime<Utc> { self.end }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    #[test]
    fn rejects_inverted_range() {
        let a = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap();
        let b = Utc.with_ymd_and_hms(2026, 5, 13, 9, 0, 0).unwrap();
        assert!(TimeRange::new(a, b).is_err());
    }
}
```

- [ ] **Step 5: Register modules**

`crates/domain/src/lib.rs`:

```rust
//! Pure domain layer. No IO, no async, no infra imports.
pub mod asset;
pub mod money;
pub mod percent;
pub mod price;
pub mod quantity;
pub mod symbol;
pub mod time_range;
```

- [ ] **Step 6: Run all domain tests**

Run: `cargo test -p domain`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add Quantity, Percent, Price, TimeRange value objects"
```

---

### Task 1.4: `Quote` and `Candle` entities

**Files:**
- Create: `crates/domain/src/quote.rs`
- Create: `crates/domain/src/candle.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: `Quote`**

`crates/domain/src/quote.rs`:

```rust
use crate::{price::Price, symbol::Symbol};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Quote {
    pub symbol: Symbol,
    pub price: Price,
    pub change_24h: Option<rust_decimal::Decimal>, // ratio, e.g. 0.0124 = +1.24%
    pub volume_24h: Option<rust_decimal::Decimal>,
    pub observed_at: DateTime<Utc>,
}

impl Quote {
    pub fn new(symbol: Symbol, price: Price, observed_at: DateTime<Utc>) -> Self {
        Self { symbol, price, change_24h: None, volume_24h: None, observed_at }
    }
}
```

- [ ] **Step 2: `Candle`**

`crates/domain/src/candle.rs`:

```rust
use crate::{price::Price, symbol::Symbol};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Candle {
    pub symbol: Symbol,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: Decimal,
    pub opened_at: DateTime<Utc>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CandleError {
    #[error("high {high} must be >= max(open, close, low)")]
    HighInvariantBroken { high: Decimal },
    #[error("low {low} must be <= min(open, close, high)")]
    LowInvariantBroken { low: Decimal },
}

impl Candle {
    pub fn validate(&self) -> Result<(), CandleError> {
        let o = self.open.money().amount();
        let h = self.high.money().amount();
        let l = self.low.money().amount();
        let c = self.close.money().amount();
        if h < o || h < c || h < l { return Err(CandleError::HighInvariantBroken { high: h }); }
        if l > o || l > c || l > h { return Err(CandleError::LowInvariantBroken { low: l }); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::{Currency, Money}};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn p(v: rust_decimal::Decimal) -> Price {
        Price::new(Money::new(v, Currency::new("USD").unwrap()))
    }

    #[test]
    fn rejects_high_below_close() {
        let c = Candle {
            symbol: Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            open: p(dec!(100)), high: p(dec!(101)), low: p(dec!(99)), close: p(dec!(102)),
            volume: dec!(0), opened_at: Utc::now(),
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn accepts_valid_candle() {
        let c = Candle {
            symbol: Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            open: p(dec!(100)), high: p(dec!(105)), low: p(dec!(99)), close: p(dec!(102)),
            volume: dec!(1000), opened_at: Utc::now(),
        };
        assert!(c.validate().is_ok());
    }
}
```

- [ ] **Step 3: Register and test**

`crates/domain/src/lib.rs` add:

```rust
pub mod candle;
pub mod quote;
```

Run: `cargo test -p domain`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/domain/
git commit -m "feat(domain): add Quote and Candle entities with OHLC invariants"
```

---

### Task 1.5: `Holding`, `Watchlist`, `Portfolio` aggregates

**Files:**
- Create: `crates/domain/src/holding.rs`
- Create: `crates/domain/src/watchlist.rs`
- Create: `crates/domain/src/portfolio.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: `Holding`**

`crates/domain/src/holding.rs`:

```rust
use crate::{money::Money, quantity::Quantity, symbol::Symbol};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Holding {
    pub symbol: Symbol,
    pub quantity: Quantity,
    pub avg_cost: Money, // per-unit cost basis in the holding's quote currency
}

impl Holding {
    pub fn new(symbol: Symbol, quantity: Quantity, avg_cost: Money) -> Self {
        Self { symbol, quantity, avg_cost }
    }
    pub fn cost_basis(&self) -> Money {
        self.avg_cost.mul_scalar(self.quantity.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::Currency};
    use rust_decimal_macros::dec;

    #[test]
    fn computes_cost_basis() {
        let h = Holding::new(
            Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            Quantity::new(dec!(10)).unwrap(),
            Money::new(dec!(150), Currency::new("USD").unwrap()),
        );
        assert_eq!(h.cost_basis(), Money::new(dec!(1500), Currency::new("USD").unwrap()));
    }
}
```

- [ ] **Step 2: `Watchlist`**

`crates/domain/src/watchlist.rs`:

```rust
use crate::symbol::Symbol;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Watchlist {
    symbols: Vec<Symbol>,
}

impl Watchlist {
    pub fn new() -> Self { Self::default() }
    pub fn symbols(&self) -> &[Symbol] { &self.symbols }
    pub fn add(&mut self, s: Symbol) -> bool {
        if self.symbols.contains(&s) { return false; }
        self.symbols.push(s);
        true
    }
    pub fn remove(&mut self, s: &Symbol) -> bool {
        let len = self.symbols.len();
        self.symbols.retain(|x| x != s);
        self.symbols.len() != len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::AssetKind;
    #[test]
    fn add_is_idempotent() {
        let mut w = Watchlist::new();
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        assert!(w.add(s.clone()));
        assert!(!w.add(s));
        assert_eq!(w.symbols().len(), 1);
    }
}
```

- [ ] **Step 3: `Portfolio`**

`crates/domain/src/portfolio.rs`:

```rust
use crate::holding::Holding;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Portfolio {
    holdings: Vec<Holding>,
}

impl Portfolio {
    pub fn new() -> Self { Self::default() }
    pub fn holdings(&self) -> &[Holding] { &self.holdings }
    pub fn upsert(&mut self, h: Holding) {
        if let Some(existing) = self.holdings.iter_mut().find(|x| x.symbol == h.symbol) {
            *existing = h;
        } else {
            self.holdings.push(h);
        }
    }
    pub fn remove(&mut self, symbol: &crate::symbol::Symbol) -> bool {
        let len = self.holdings.len();
        self.holdings.retain(|x| &x.symbol != symbol);
        self.holdings.len() != len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::{Currency, Money}, quantity::Quantity, symbol::Symbol};
    use rust_decimal_macros::dec;
    #[test]
    fn upsert_replaces_existing() {
        let mut p = Portfolio::new();
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let h1 = Holding::new(s.clone(), Quantity::new(dec!(10)).unwrap(), Money::new(dec!(100), Currency::new("USD").unwrap()));
        let h2 = Holding::new(s.clone(), Quantity::new(dec!(20)).unwrap(), Money::new(dec!(110), Currency::new("USD").unwrap()));
        p.upsert(h1);
        p.upsert(h2);
        assert_eq!(p.holdings().len(), 1);
        assert_eq!(p.holdings()[0].quantity.value(), dec!(20));
    }
}
```

- [ ] **Step 4: Register, test, commit**

`crates/domain/src/lib.rs` add:

```rust
pub mod holding;
pub mod portfolio;
pub mod watchlist;
```

Run: `cargo test -p domain`
Expected: PASS.

```bash
git add crates/domain/
git commit -m "feat(domain): add Holding, Watchlist, Portfolio aggregates"
```

---

### Task 1.6: `QuoteSanityCheck` and `PortfolioCalc` domain services

**Files:**
- Create: `crates/domain/src/sanity.rs`
- Create: `crates/domain/src/portfolio_calc.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: `QuoteSanityCheck`**

`crates/domain/src/sanity.rs`:

```rust
use crate::quote::Quote;
use rust_decimal::Decimal;

/// Pure outlier check. Returns true if the new quote should be accepted.
/// Reject if price moved more than `jump_threshold` ratio (e.g. 10 = 1000%)
/// vs. the previous accepted quote. Accept the first quote unconditionally.
pub fn is_sane(previous: Option<&Quote>, candidate: &Quote, jump_threshold: Decimal) -> bool {
    let Some(prev) = previous else { return true; };
    let p = prev.price.money().amount();
    let c = candidate.price.money().amount();
    if p == Decimal::ZERO { return true; }
    let ratio = (c - p).abs() / p;
    ratio < jump_threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn q(amount: rust_decimal::Decimal) -> Quote {
        Quote::new(
            Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap(),
            Price::new(Money::new(amount, Currency::new("USD").unwrap())),
            Utc::now(),
        )
    }

    #[test]
    fn accepts_first_quote() {
        assert!(is_sane(None, &q(dec!(100)), dec!(10)));
    }

    #[test]
    fn rejects_10x_jump() {
        let prev = q(dec!(100));
        let new = q(dec!(1100));
        assert!(!is_sane(Some(&prev), &new, dec!(10)));
    }

    #[test]
    fn accepts_5pct_move() {
        let prev = q(dec!(100));
        let new = q(dec!(105));
        assert!(is_sane(Some(&prev), &new, dec!(10)));
    }
}
```

- [ ] **Step 2: `PortfolioCalc`**

`crates/domain/src/portfolio_calc.rs`:

```rust
use crate::{holding::Holding, money::{Money, MoneyError}, portfolio::Portfolio, quote::Quote, symbol::Symbol};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioValuation {
    pub per_holding: Vec<HoldingValuation>,
    pub total_value: Option<Money>,         // None if quotes for any holding are missing
    pub total_cost: Option<Money>,
    pub total_pnl: Option<Money>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingValuation {
    pub symbol: Symbol,
    pub market_value: Option<Money>,
    pub cost_basis: Money,
    pub pnl_absolute: Option<Money>,
}

/// Pure: evaluate the portfolio against a quotes lookup table.
/// `display_currency` is informational; cross-currency aggregation returns `total_value = None`
/// if currencies mix and no `fx` table is given. (FX support is M2.)
pub fn evaluate(
    portfolio: &Portfolio,
    quotes_by_symbol: &HashMap<Symbol, Quote>,
) -> PortfolioValuation {
    let per_holding: Vec<HoldingValuation> = portfolio
        .holdings()
        .iter()
        .map(|h| value_holding(h, quotes_by_symbol.get(&h.symbol)))
        .collect();

    let total_value = sum_money(per_holding.iter().filter_map(|h| h.market_value));
    let total_cost = sum_money(per_holding.iter().map(|h| h.cost_basis));
    let total_pnl = match (total_value, total_cost) {
        (Some(v), Some(c)) => v.sub(c).ok(),
        _ => None,
    };

    PortfolioValuation { per_holding, total_value, total_cost, total_pnl }
}

fn value_holding(h: &Holding, quote: Option<&Quote>) -> HoldingValuation {
    let market_value = quote.map(|q| q.price.money().mul_scalar(h.quantity.value()));
    let cost_basis = h.cost_basis();
    let pnl_absolute = match market_value {
        Some(mv) => mv.sub(cost_basis).ok(),
        None => None,
    };
    HoldingValuation { symbol: h.symbol.clone(), market_value, cost_basis, pnl_absolute }
}

/// Returns `Some(sum)` if all items share a currency, `None` if they don't or the iterator is empty.
fn sum_money<I: IntoIterator<Item = Money>>(items: I) -> Option<Money> {
    let mut iter = items.into_iter();
    let first = iter.next()?;
    iter.try_fold(first, |acc, m| acc.add(m)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset::AssetKind, money::{Currency, Money}, price::Price, quantity::Quantity,
        symbol::Symbol,
    };
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn s_aapl() -> Symbol { Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap() }

    #[test]
    fn computes_pnl_for_single_holding() {
        let mut p = Portfolio::new();
        p.upsert(Holding::new(s_aapl(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));

        let mut quotes = HashMap::new();
        quotes.insert(s_aapl(), Quote::new(s_aapl(), Price::new(usd(dec!(180))), Utc::now()));

        let v = evaluate(&p, &quotes);
        assert_eq!(v.total_value, Some(usd(dec!(1800))));
        assert_eq!(v.total_cost, Some(usd(dec!(1500))));
        assert_eq!(v.total_pnl, Some(usd(dec!(300))));
    }

    #[test]
    fn missing_quote_yields_none_market_value() {
        let mut p = Portfolio::new();
        p.upsert(Holding::new(s_aapl(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));
        let quotes = HashMap::new();
        let v = evaluate(&p, &quotes);
        assert_eq!(v.per_holding[0].market_value, None);
        assert_eq!(v.total_value, None);
    }
}
```

> The `sum_money` helper above returns `None` whenever currencies mix. In M2 we'll pass an `Fx` table to convert; we leave that hook today.

- [ ] **Step 3: Register and test**

`crates/domain/src/lib.rs` add:

```rust
pub mod portfolio_calc;
pub mod sanity;
```

Run: `cargo test -p domain`
Expected: PASS.

- [ ] **Step 4: Update docs**

Append to `docs/progress.md`:

```markdown
### Phase 1 — Domain layer

- [x] Task 1.1: Money + currency-checked arithmetic.
- [x] Task 1.2: Symbol + AssetKind.
- [x] Task 1.3: Quantity, Percent, Price, TimeRange.
- [x] Task 1.4: Quote, Candle.
- [x] Task 1.5: Holding, Watchlist, Portfolio.
- [x] Task 1.6: QuoteSanityCheck, PortfolioCalc.
```

- [ ] **Step 5: Commit**

```bash
git add crates/domain/ docs/progress.md
git commit -m "feat(domain): add QuoteSanityCheck and PortfolioCalc"
```

---

## Phase 2 — Application layer (trait ports + services)

### Task 2.1: Foundational trait ports — `Clock`, `HttpClient`, `SecretStore`, `Notifier`

**Files:**
- Create: `crates/application/src/ports/mod.rs`
- Create: `crates/application/src/ports/clock.rs`
- Create: `crates/application/src/ports/http_client.rs`
- Create: `crates/application/src/ports/secret_store.rs`
- Create: `crates/application/src/ports/notifier.rs`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: Define `Clock`**

`crates/application/src/ports/clock.rs`:

```rust
use chrono::{DateTime, Utc};
use mockall::automock;

#[automock]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
```

- [ ] **Step 2: Define `HttpClient`**

`crates/application/src/ports/http_client.rs`:

```rust
use async_trait::async_trait;
use mockall::automock;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout after {0:?}")]
    Timeout(std::time::Duration),
    #[error("invalid url: {0}")]
    InvalidUrl(String),
}

#[automock]
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(
        &self,
        url: &str,
        headers: &[(&'static str, String)],
    ) -> Result<HttpResponse, HttpError>;
}
```

- [ ] **Step 3: Define `SecretStore`**

`crates/application/src/ports/secret_store.rs`:

```rust
use async_trait::async_trait;
use mockall::automock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("backend error: {0}")]
    Backend(String),
}

#[automock]
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<String, SecretError>;
    async fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    async fn delete(&self, key: &str) -> Result<(), SecretError>;
}
```

- [ ] **Step 4: Define `Notifier`**

`crates/application/src/ports/notifier.rs`:

```rust
use async_trait::async_trait;
use mockall::automock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("backend error: {0}")]
    Backend(String),
}

#[automock]
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, title: &str, body: &str) -> Result<(), NotifyError>;
}
```

- [ ] **Step 5: Register modules**

`crates/application/src/ports/mod.rs`:

```rust
pub mod clock;
pub mod http_client;
pub mod notifier;
pub mod secret_store;
```

`crates/application/src/lib.rs`:

```rust
//! Application services and trait ports. Depends on domain only.
pub mod ports;
```

- [ ] **Step 6: Verify**

Run: `cargo check -p application`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/application/
git commit -m "feat(application): add Clock, HttpClient, SecretStore, Notifier ports"
```

---

### Task 2.2: `AssetProvider` + `NewsProvider` trait ports

**Files:**
- Create: `crates/application/src/ports/asset_provider.rs`
- Create: `crates/application/src/ports/news_provider.rs`
- Modify: `crates/application/src/ports/mod.rs`

- [ ] **Step 1: `AssetProvider`**

`crates/application/src/ports/asset_provider.rs`:

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{candle::Candle, quote::Quote, symbol::Symbol};
use mockall::automock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("symbol not supported by this provider: {0}")]
    UnsupportedSymbol(String),
    #[error("rate limited; retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("network error: {0}")]
    Network(String),
}

#[automock]
#[async_trait]
pub trait AssetProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, symbol: &Symbol) -> bool;
    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError>;
    async fn fetch_candles(
        &self,
        symbol: &Symbol,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, ProviderError>;
}
```

- [ ] **Step 2: `NewsProvider`**

`crates/application/src/ports/news_provider.rs`:

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::symbol::Symbol;
use mockall::automock;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Headline {
    pub title: String,
    pub url: String,
    pub source: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum NewsError {
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("parse error: {0}")]
    Parse(String),
}

#[automock]
#[async_trait]
pub trait NewsProvider: Send + Sync {
    async fn fetch(&self, symbol: &Symbol, limit: usize) -> Result<Vec<Headline>, NewsError>;
}
```

- [ ] **Step 3: Register**

`crates/application/src/ports/mod.rs` (replace contents):

```rust
pub mod asset_provider;
pub mod clock;
pub mod http_client;
pub mod news_provider;
pub mod notifier;
pub mod secret_store;
```

- [ ] **Step 4: Verify**

Run: `cargo check -p application`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/application/
git commit -m "feat(application): add AssetProvider and NewsProvider ports"
```

---

### Task 2.3: Repository trait ports

**Files:**
- Create: `crates/application/src/ports/repos.rs`
- Modify: `crates/application/src/ports/mod.rs`

- [ ] **Step 1: Add repos**

`crates/application/src/ports/repos.rs`:

```rust
use async_trait::async_trait;
use domain::{holding::Holding, symbol::Symbol, watchlist::Watchlist, portfolio::Portfolio};
use mockall::automock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found")]
    NotFound,
}

#[automock]
#[async_trait]
pub trait WatchlistRepo: Send + Sync {
    async fn load(&self) -> Result<Watchlist, RepoError>;
    async fn save(&self, watchlist: &Watchlist) -> Result<(), RepoError>;
}

#[automock]
#[async_trait]
pub trait PortfolioRepo: Send + Sync {
    async fn load(&self) -> Result<Portfolio, RepoError>;
    async fn upsert_holding(&self, holding: &Holding) -> Result<(), RepoError>;
    async fn delete_holding(&self, symbol: &Symbol) -> Result<(), RepoError>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    pub poll_interval_secs: u32,
    pub display_currency: String,
    pub theme: String,
    pub widget_opacity: f32,
    pub widget_always_on_top: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            display_currency: "USD".into(),
            theme: "dark".into(),
            widget_opacity: 0.85,
            widget_always_on_top: true,
        }
    }
}

#[automock]
#[async_trait]
pub trait SettingsRepo: Send + Sync {
    async fn load(&self) -> Result<AppSettings, RepoError>;
    async fn save(&self, settings: &AppSettings) -> Result<(), RepoError>;
}
```

- [ ] **Step 2: Register**

`crates/application/src/ports/mod.rs` append `pub mod repos;`.

- [ ] **Step 3: Verify**

Run: `cargo check -p application`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/application/
git commit -m "feat(application): add Watchlist/Portfolio/Settings repo ports"
```

---

### Task 2.4: `MarketService` (TDD with mocks)

**Files:**
- Create: `crates/application/src/market_service.rs`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: Write the service skeleton + failing test**

`crates/application/src/market_service.rs`:

```rust
use crate::ports::asset_provider::{AssetProvider, ProviderError};
use crate::ports::repos::{RepoError, WatchlistRepo};
use domain::{quote::Quote, symbol::Symbol, watchlist::Watchlist};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Error)]
pub enum MarketError {
    #[error("provider: {0}")]
    Provider(#[from] ProviderError),
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("no provider supports symbol: {0}")]
    NoProvider(String),
}

pub struct MarketService {
    watchlist_repo: Arc<dyn WatchlistRepo>,
    providers: Vec<Arc<dyn AssetProvider>>,
    last_quotes: Arc<RwLock<HashMap<Symbol, Quote>>>,
}

impl MarketService {
    pub fn new(
        watchlist_repo: Arc<dyn WatchlistRepo>,
        providers: Vec<Arc<dyn AssetProvider>>,
    ) -> Self {
        Self { watchlist_repo, providers, last_quotes: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn load_watchlist(&self) -> Result<Watchlist, MarketError> {
        Ok(self.watchlist_repo.load().await?)
    }

    pub async fn add_to_watchlist(&self, symbol: Symbol) -> Result<(), MarketError> {
        let mut wl = self.watchlist_repo.load().await?;
        wl.add(symbol);
        self.watchlist_repo.save(&wl).await?;
        Ok(())
    }

    pub async fn remove_from_watchlist(&self, symbol: &Symbol) -> Result<(), MarketError> {
        let mut wl = self.watchlist_repo.load().await?;
        wl.remove(symbol);
        self.watchlist_repo.save(&wl).await?;
        Ok(())
    }

    pub async fn refresh(&self) -> Result<Vec<Quote>, MarketError> {
        let wl = self.watchlist_repo.load().await?;
        let mut all_quotes = Vec::new();

        for symbol in wl.symbols() {
            let provider = self
                .providers
                .iter()
                .find(|p| p.supports(symbol))
                .ok_or_else(|| MarketError::NoProvider(symbol.to_canonical_string()))?;
            let quotes = provider.fetch_quotes(std::slice::from_ref(symbol)).await?;
            for q in quotes {
                self.last_quotes.write().await.insert(q.symbol.clone(), q.clone());
                all_quotes.push(q);
            }
        }
        Ok(all_quotes)
    }

    pub async fn snapshot(&self) -> HashMap<Symbol, Quote> {
        self.last_quotes.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::asset_provider::MockAssetProvider;
    use crate::ports::repos::MockWatchlistRepo;
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol,
    };
    use mockall::predicate::*;
    use rust_decimal_macros::dec;

    fn s_btc() -> Symbol { Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap() }
    fn s_aapl() -> Symbol { Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap() }

    #[tokio::test]
    async fn refresh_routes_each_symbol_to_supporting_provider() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(s_btc());
        wl.add(s_aapl());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut crypto = MockAssetProvider::new();
        crypto.expect_name().return_const("crypto-mock");
        crypto.expect_supports().returning(|s| s.kind() == AssetKind::Crypto);
        crypto.expect_fetch_quotes().returning(|symbols| {
            Ok(symbols.iter().map(|s| {
                Quote::new(s.clone(), Price::new(Money::new(dec!(67000), Currency::new("USD").unwrap())), Utc::now())
            }).collect())
        });

        let mut stock = MockAssetProvider::new();
        stock.expect_name().return_const("stock-mock");
        stock.expect_supports().returning(|s| s.kind() == AssetKind::UsEquity);
        stock.expect_fetch_quotes().returning(|symbols| {
            Ok(symbols.iter().map(|s| {
                Quote::new(s.clone(), Price::new(Money::new(dec!(182), Currency::new("USD").unwrap())), Utc::now())
            }).collect())
        });

        let svc = MarketService::new(Arc::new(wl_repo), vec![Arc::new(crypto), Arc::new(stock)]);
        let quotes = svc.refresh().await.unwrap();
        assert_eq!(quotes.len(), 2);

        let snap = svc.snapshot().await;
        assert!(snap.contains_key(&s_btc()));
        assert!(snap.contains_key(&s_aapl()));
    }

    #[tokio::test]
    async fn refresh_errors_when_no_provider_supports_symbol() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(Symbol::new(AssetKind::Forex, "EURUSD", None).unwrap());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut crypto = MockAssetProvider::new();
        crypto.expect_supports().return_const(false);

        let svc = MarketService::new(Arc::new(wl_repo), vec![Arc::new(crypto)]);
        assert!(matches!(svc.refresh().await, Err(MarketError::NoProvider(_))));
    }
}
```

- [ ] **Step 2: Register module**

`crates/application/src/lib.rs`:

```rust
//! Application services and trait ports. Depends on domain only.
pub mod market_service;
pub mod ports;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p application market_service`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/application/
git commit -m "feat(application): add MarketService with provider routing and snapshot cache"
```

---

### Task 2.5: `PortfolioService` (TDD with mocks)

**Files:**
- Create: `crates/application/src/portfolio_service.rs`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: Write service + test**

`crates/application/src/portfolio_service.rs`:

```rust
use crate::market_service::MarketService;
use crate::ports::repos::{PortfolioRepo, RepoError};
use domain::{
    holding::Holding, portfolio_calc, symbol::Symbol,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PortfolioError {
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
}

pub struct PortfolioService {
    repo: Arc<dyn PortfolioRepo>,
    market: Arc<MarketService>,
}

impl PortfolioService {
    pub fn new(repo: Arc<dyn PortfolioRepo>, market: Arc<MarketService>) -> Self {
        Self { repo, market }
    }

    pub async fn upsert_holding(&self, holding: Holding) -> Result<(), PortfolioError> {
        self.repo.upsert_holding(&holding).await?;
        Ok(())
    }

    pub async fn delete_holding(&self, symbol: &Symbol) -> Result<(), PortfolioError> {
        self.repo.delete_holding(symbol).await?;
        Ok(())
    }

    pub async fn valuation(&self) -> Result<portfolio_calc::PortfolioValuation, PortfolioError> {
        let portfolio = self.repo.load().await?;
        let quotes = self.market.snapshot().await;
        Ok(portfolio_calc::evaluate(&portfolio, &quotes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::MarketService;
    use crate::ports::asset_provider::MockAssetProvider;
    use crate::ports::repos::{MockPortfolioRepo, MockWatchlistRepo};
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, portfolio::Portfolio, price::Price,
        quantity::Quantity, quote::Quote, symbol::Symbol, watchlist::Watchlist,
    };
    use rust_decimal_macros::dec;

    fn s_aapl() -> Symbol { Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap() }
    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }

    #[tokio::test]
    async fn valuation_uses_market_snapshot() {
        let mut wl_repo = MockWatchlistRepo::new();
        let mut wl = Watchlist::new();
        wl.add(s_aapl());
        let wl_clone = wl.clone();
        wl_repo.expect_load().returning(move || Ok(wl_clone.clone()));

        let mut prov = MockAssetProvider::new();
        prov.expect_supports().returning(|s| s.kind() == AssetKind::UsEquity);
        prov.expect_fetch_quotes().returning(|symbols| {
            Ok(symbols.iter().map(|s|
                Quote::new(s.clone(), Price::new(usd(dec!(180))), Utc::now())
            ).collect())
        });

        let market = Arc::new(MarketService::new(Arc::new(wl_repo), vec![Arc::new(prov)]));
        market.refresh().await.unwrap();

        let mut pf_repo = MockPortfolioRepo::new();
        let mut pf = Portfolio::new();
        pf.upsert(Holding::new(s_aapl(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));
        let pf_clone = pf.clone();
        pf_repo.expect_load().returning(move || Ok(pf_clone.clone()));

        let svc = PortfolioService::new(Arc::new(pf_repo), market);
        let v = svc.valuation().await.unwrap();
        assert_eq!(v.total_value, Some(usd(dec!(1800))));
        assert_eq!(v.total_pnl, Some(usd(dec!(300))));
    }
}
```

- [ ] **Step 2: Register**

`crates/application/src/lib.rs` add `pub mod portfolio_service;`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p application portfolio_service`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/application/
git commit -m "feat(application): add PortfolioService computing valuation from market snapshot"
```

---

### Task 2.6: `SettingsService`

**Files:**
- Create: `crates/application/src/settings_service.rs`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: Implement**

`crates/application/src/settings_service.rs`:

```rust
use crate::ports::repos::{AppSettings, RepoError, SettingsRepo};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("invalid poll interval: must be 1..=300 seconds, got {0}")]
    InvalidPollInterval(u32),
    #[error("invalid widget opacity: must be 0.1..=1.0, got {0}")]
    InvalidWidgetOpacity(f32),
}

pub struct SettingsService { repo: Arc<dyn SettingsRepo> }

impl SettingsService {
    pub fn new(repo: Arc<dyn SettingsRepo>) -> Self { Self { repo } }

    pub async fn get(&self) -> Result<AppSettings, SettingsError> {
        Ok(self.repo.load().await?)
    }

    pub async fn save(&self, settings: AppSettings) -> Result<(), SettingsError> {
        if !(1..=300).contains(&settings.poll_interval_secs) {
            return Err(SettingsError::InvalidPollInterval(settings.poll_interval_secs));
        }
        if !(0.1..=1.0).contains(&settings.widget_opacity) {
            return Err(SettingsError::InvalidWidgetOpacity(settings.widget_opacity));
        }
        self.repo.save(&settings).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::repos::MockSettingsRepo;

    #[tokio::test]
    async fn rejects_too_small_poll_interval() {
        let mut repo = MockSettingsRepo::new();
        repo.expect_save().never();
        let svc = SettingsService::new(Arc::new(repo));
        let mut s = AppSettings::default();
        s.poll_interval_secs = 0;
        assert!(matches!(svc.save(s).await, Err(SettingsError::InvalidPollInterval(0))));
    }

    #[tokio::test]
    async fn rejects_excessive_opacity() {
        let mut repo = MockSettingsRepo::new();
        repo.expect_save().never();
        let svc = SettingsService::new(Arc::new(repo));
        let mut s = AppSettings::default();
        s.widget_opacity = 1.5;
        assert!(matches!(svc.save(s).await, Err(SettingsError::InvalidWidgetOpacity(_))));
    }
}
```

- [ ] **Step 2: Register and test**

`crates/application/src/lib.rs` add `pub mod settings_service;`.

Run: `cargo test -p application settings_service`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/application/
git commit -m "feat(application): add SettingsService with poll interval and opacity validation"
```

---

### Task 2.7: `PollScheduler`

**Files:**
- Create: `crates/application/src/poll_scheduler.rs`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: Implement**

`crates/application/src/poll_scheduler.rs`:

```rust
use crate::{market_service::MarketService, ports::clock::Clock};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Driver that periodically calls `MarketService::refresh()`.
/// Emits a tick value (an incrementing counter) over a watch channel so consumers can wait on
/// "the most recent refresh finished". This is deliberately decoupled from IPC.
pub struct PollScheduler {
    market: Arc<MarketService>,
    clock: Arc<dyn Clock>,
    tick_tx: watch::Sender<u64>,
}

pub struct PollHandle { task: tokio::task::JoinHandle<()> }

impl PollHandle { pub fn abort(self) { self.task.abort(); } }

impl PollScheduler {
    pub fn new(market: Arc<MarketService>, clock: Arc<dyn Clock>) -> (Self, watch::Receiver<u64>) {
        let (tx, rx) = watch::channel(0);
        (Self { market, clock, tick_tx: tx }, rx)
    }

    pub fn start(self, interval: Duration) -> PollHandle {
        let task = tokio::spawn(async move {
            let mut counter: u64 = 0;
            loop {
                let _ = self.clock.now(); // forces dyn Clock to be live (and easy to mock-call in tests)
                if let Err(e) = self.market.refresh().await {
                    tracing::warn!(error = ?e, "poll refresh failed");
                }
                counter = counter.wrapping_add(1);
                let _ = self.tick_tx.send(counter);
                tokio::time::sleep(interval).await;
            }
        });
        PollHandle { task }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::MarketService;
    use crate::ports::asset_provider::MockAssetProvider;
    use crate::ports::clock::MockClock;
    use crate::ports::repos::MockWatchlistRepo;
    use chrono::Utc;
    use domain::watchlist::Watchlist;

    #[tokio::test(start_paused = true)]
    async fn ticks_at_least_twice_in_three_intervals() {
        let mut wl_repo = MockWatchlistRepo::new();
        wl_repo.expect_load().returning(|| Ok(Watchlist::new()));

        let mut prov = MockAssetProvider::new();
        prov.expect_supports().return_const(false);

        let mut clock = MockClock::new();
        clock.expect_now().returning(Utc::now);

        let market = Arc::new(MarketService::new(Arc::new(wl_repo), vec![Arc::new(prov)]));
        let (scheduler, mut rx) = PollScheduler::new(market, Arc::new(clock));
        let handle = scheduler.start(Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(160)).await;
        assert!(*rx.borrow_and_update() >= 2);
        handle.abort();
    }
}
```

- [ ] **Step 2: Register**

`crates/application/src/lib.rs` add `pub mod poll_scheduler;`.

- [ ] **Step 3: Run all application tests**

Run: `cargo test -p application`
Expected: PASS.

- [ ] **Step 4: Update progress**

Append to `docs/progress.md`:

```markdown
### Phase 2 — Application layer

- [x] Task 2.1: Clock, HttpClient, SecretStore, Notifier ports.
- [x] Task 2.2: AssetProvider, NewsProvider ports.
- [x] Task 2.3: Repo ports + AppSettings.
- [x] Task 2.4: MarketService.
- [x] Task 2.5: PortfolioService.
- [x] Task 2.6: SettingsService.
- [x] Task 2.7: PollScheduler.
```

- [ ] **Step 5: Commit**

```bash
git add crates/application/ docs/progress.md
git commit -m "feat(application): add PollScheduler driving MarketService.refresh"
```

---

## Phase 3 — Infrastructure adapters

### Task 3.1: `SystemClock` and `ReqwestHttpClient`

**Files:**
- Create: `crates/infrastructure/src/clock.rs`
- Create: `crates/infrastructure/src/http.rs`
- Modify: `crates/infrastructure/src/lib.rs`

- [ ] **Step 1: `SystemClock`**

`crates/infrastructure/src/clock.rs`:

```rust
use application::ports::clock::Clock;
use chrono::{DateTime, Utc};

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> { Utc::now() }
}
```

- [ ] **Step 2: `ReqwestHttpClient`**

`crates/infrastructure/src/http.rs`:

```rust
use application::ports::http_client::{HttpClient, HttpError, HttpResponse};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(concat!("ai-stock/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client");
        Self { client }
    }
}

impl Default for ReqwestHttpClient { fn default() -> Self { Self::new() } }

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(
        &self,
        url: &str,
        headers: &[(&'static str, String)],
    ) -> Result<HttpResponse, HttpError> {
        let mut req = self.client.get(url);
        for (k, v) in headers { req = req.header(*k, v); }
        let resp = req
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() { HttpError::Timeout(Duration::from_secs(5)) }
                else if e.is_builder() { HttpError::InvalidUrl(url.into()) }
                else { HttpError::Network(e.to_string()) }
            })?;
        let status = resp.status().as_u16();
        let headers_map: HashMap<String, String> = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
            .collect();
        let body = resp.bytes().await.map_err(|e| HttpError::Network(e.to_string()))?.to_vec();
        Ok(HttpResponse { status, headers: headers_map, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn round_trips_response_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/hello"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hi"))
            .mount(&server)
            .await;

        let client = ReqwestHttpClient::new();
        let resp = client.get(&format!("{}/hello", server.uri()), &[]).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"hi");
    }
}
```

- [ ] **Step 3: Register**

`crates/infrastructure/src/lib.rs`:

```rust
//! Adapters implementing application ports. Only place where reqwest/sqlx/keyring live.
pub mod clock;
pub mod http;
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p infrastructure`
Expected: PASS.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add SystemClock and ReqwestHttpClient with wiremock test"
```

---

### Task 3.2: `KeyringSecretStore`

**Files:**
- Create: `crates/infrastructure/src/keyring_secrets.rs`
- Modify: `crates/infrastructure/src/lib.rs`

- [ ] **Step 1: Implement**

`crates/infrastructure/src/keyring_secrets.rs`:

```rust
use application::ports::secret_store::{SecretError, SecretStore};
use async_trait::async_trait;

pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub fn new(service: impl Into<String>) -> Self { Self { service: service.into() } }
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn get(&self, key: &str) -> Result<String, SecretError> {
        let service = self.service.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|e| SecretError::Backend(e.to_string()))?;
            match entry.get_password() {
                Ok(v) => Ok(v),
                Err(keyring::Error::NoEntry) => Err(SecretError::NotFound(key)),
                Err(e) => Err(SecretError::Backend(e.to_string())),
            }
        })
        .await
        .map_err(|e| SecretError::Backend(e.to_string()))?
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        let service = self.service.clone();
        let key = key.to_string();
        let value = value.to_string();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|e| SecretError::Backend(e.to_string()))?;
            entry.set_password(&value).map_err(|e| SecretError::Backend(e.to_string()))
        })
        .await
        .map_err(|e| SecretError::Backend(e.to_string()))?
    }

    async fn delete(&self, key: &str) -> Result<(), SecretError> {
        let service = self.service.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|e| SecretError::Backend(e.to_string()))?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(SecretError::Backend(e.to_string())),
            }
        })
        .await
        .map_err(|e| SecretError::Backend(e.to_string()))?
    }
}
```

> The OS keychain isn't available in CI sandboxes, so this module has no integration test. Coverage comes from the IPC-level smoke test in Phase 4.

- [ ] **Step 2: Register and commit**

`crates/infrastructure/src/lib.rs` add `pub mod keyring_secrets;`.

Run: `cargo check -p infrastructure`
Expected: clean.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add KeyringSecretStore wrapping the keyring crate"
```

---

### Task 3.3: SQLite migrations + `SqliteWatchlistRepo`

**Files:**
- Create: `crates/infrastructure/migrations/20260513000001_init.sql`
- Create: `crates/infrastructure/src/sqlite/mod.rs`
- Create: `crates/infrastructure/src/sqlite/watchlist_repo.rs`
- Modify: `crates/infrastructure/src/lib.rs`

- [ ] **Step 1: Migration**

`crates/infrastructure/migrations/20260513000001_init.sql`:

```sql
CREATE TABLE IF NOT EXISTS watchlist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    ticker TEXT NOT NULL,
    quote_currency TEXT,
    position INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    UNIQUE(kind, ticker, quote_currency)
);

CREATE TABLE IF NOT EXISTS holdings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    ticker TEXT NOT NULL,
    quote_currency TEXT,
    quantity TEXT NOT NULL,            -- decimal as string
    avg_cost_amount TEXT NOT NULL,
    avg_cost_currency TEXT NOT NULL,
    UNIQUE(kind, ticker, quote_currency)
);

CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    poll_interval_secs INTEGER NOT NULL,
    display_currency TEXT NOT NULL,
    theme TEXT NOT NULL,
    widget_opacity REAL NOT NULL,
    widget_always_on_top INTEGER NOT NULL
);
```

- [ ] **Step 2: Pool init**

`crates/infrastructure/src/sqlite/mod.rs`:

```rust
pub mod watchlist_repo;
pub mod portfolio_repo;
pub mod settings_repo;

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;

pub async fn open(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

- [ ] **Step 3: `SqliteWatchlistRepo` with TDD**

`crates/infrastructure/src/sqlite/watchlist_repo.rs`:

```rust
use application::ports::repos::{RepoError, WatchlistRepo};
use async_trait::async_trait;
use domain::{asset::AssetKind, symbol::Symbol, watchlist::Watchlist};
use sqlx::SqlitePool;

pub struct SqliteWatchlistRepo { pool: SqlitePool }

impl SqliteWatchlistRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }
}

fn kind_to_str(k: AssetKind) -> &'static str {
    match k {
        AssetKind::Crypto => "crypto",
        AssetKind::UsEquity => "us",
        AssetKind::KrEquity => "kr",
        AssetKind::Forex => "fx",
        AssetKind::Commodity => "com",
    }
}
fn str_to_kind(s: &str) -> Option<AssetKind> {
    Some(match s {
        "crypto" => AssetKind::Crypto,
        "us" => AssetKind::UsEquity,
        "kr" => AssetKind::KrEquity,
        "fx" => AssetKind::Forex,
        "com" => AssetKind::Commodity,
        _ => return None,
    })
}

#[async_trait]
impl WatchlistRepo for SqliteWatchlistRepo {
    async fn load(&self) -> Result<Watchlist, RepoError> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT kind, ticker, quote_currency FROM watchlist ORDER BY position ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Storage(e.to_string()))?;
        let mut wl = Watchlist::new();
        for (k, t, q) in rows {
            let Some(kind) = str_to_kind(&k) else { continue; };
            let symbol = Symbol::new(kind, &t, q.as_deref())
                .map_err(|e| RepoError::Storage(format!("invalid symbol: {e}")))?;
            wl.add(symbol);
        }
        Ok(wl)
    }

    async fn save(&self, watchlist: &Watchlist) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(|e| RepoError::Storage(e.to_string()))?;
        sqlx::query("DELETE FROM watchlist").execute(&mut *tx).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        for (pos, s) in watchlist.symbols().iter().enumerate() {
            sqlx::query(
                "INSERT INTO watchlist (kind, ticker, quote_currency, position) VALUES (?, ?, ?, ?)",
            )
            .bind(kind_to_str(s.kind()))
            .bind(s.ticker())
            .bind(s.quote_currency())
            .bind(pos as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepoError::Storage(e.to_string()))?;
        }
        tx.commit().await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open;
    use tempfile::tempdir;

    #[tokio::test]
    async fn round_trip_watchlist() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let pool = open(&path).await.unwrap();
        let repo = SqliteWatchlistRepo::new(pool);

        let mut wl = Watchlist::new();
        wl.add(Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap());
        wl.add(Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap());

        repo.save(&wl).await.unwrap();
        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.symbols(), wl.symbols());
    }
}
```

- [ ] **Step 4: Run + commit**

Run: `cargo test -p infrastructure sqlite::watchlist_repo::`
Expected: PASS.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add sqlite pool + SqliteWatchlistRepo with migrations"
```

---

### Task 3.4: `SqlitePortfolioRepo`

**Files:**
- Create: `crates/infrastructure/src/sqlite/portfolio_repo.rs`

- [ ] **Step 1: Implement**

`crates/infrastructure/src/sqlite/portfolio_repo.rs`:

```rust
use application::ports::repos::{PortfolioRepo, RepoError};
use async_trait::async_trait;
use domain::{
    asset::AssetKind, holding::Holding, money::{Currency, Money}, portfolio::Portfolio,
    quantity::Quantity, symbol::Symbol,
};
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use std::str::FromStr;

pub struct SqlitePortfolioRepo { pool: SqlitePool }

impl SqlitePortfolioRepo { pub fn new(pool: SqlitePool) -> Self { Self { pool } } }

fn kind_to_str(k: AssetKind) -> &'static str {
    match k {
        AssetKind::Crypto => "crypto",
        AssetKind::UsEquity => "us",
        AssetKind::KrEquity => "kr",
        AssetKind::Forex => "fx",
        AssetKind::Commodity => "com",
    }
}
fn str_to_kind(s: &str) -> Option<AssetKind> {
    Some(match s {
        "crypto" => AssetKind::Crypto, "us" => AssetKind::UsEquity, "kr" => AssetKind::KrEquity,
        "fx" => AssetKind::Forex, "com" => AssetKind::Commodity, _ => return None,
    })
}

#[async_trait]
impl PortfolioRepo for SqlitePortfolioRepo {
    async fn load(&self) -> Result<Portfolio, RepoError> {
        let rows: Vec<(String, String, Option<String>, String, String, String)> = sqlx::query_as(
            "SELECT kind, ticker, quote_currency, quantity, avg_cost_amount, avg_cost_currency FROM holdings",
        )
        .fetch_all(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        let mut p = Portfolio::new();
        for (kind_s, ticker, qc, qty_s, amt_s, ccy_s) in rows {
            let kind = str_to_kind(&kind_s).ok_or_else(|| RepoError::Storage(format!("bad kind: {kind_s}")))?;
            let symbol = Symbol::new(kind, &ticker, qc.as_deref())
                .map_err(|e| RepoError::Storage(e.to_string()))?;
            let qty = Decimal::from_str(&qty_s).map_err(|e| RepoError::Storage(e.to_string()))?;
            let amt = Decimal::from_str(&amt_s).map_err(|e| RepoError::Storage(e.to_string()))?;
            let ccy = Currency::new(&ccy_s).map_err(|e| RepoError::Storage(e.to_string()))?;
            p.upsert(Holding::new(
                symbol,
                Quantity::new(qty).map_err(|e| RepoError::Storage(format!("{e:?}")))?,
                Money::new(amt, ccy),
            ));
        }
        Ok(p)
    }

    async fn upsert_holding(&self, h: &Holding) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO holdings (kind, ticker, quote_currency, quantity, avg_cost_amount, avg_cost_currency)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(kind, ticker, quote_currency) DO UPDATE SET
               quantity = excluded.quantity,
               avg_cost_amount = excluded.avg_cost_amount,
               avg_cost_currency = excluded.avg_cost_currency",
        )
        .bind(kind_to_str(h.symbol.kind()))
        .bind(h.symbol.ticker())
        .bind(h.symbol.quote_currency())
        .bind(h.quantity.value().to_string())
        .bind(h.avg_cost.amount().to_string())
        .bind(h.avg_cost.currency().as_str())
        .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete_holding(&self, symbol: &Symbol) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM holdings WHERE kind = ? AND ticker = ? AND coalesce(quote_currency,'') = coalesce(?,'')")
            .bind(kind_to_str(symbol.kind()))
            .bind(symbol.ticker())
            .bind(symbol.quote_currency())
            .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open;
    use rust_decimal_macros::dec;
    use tempfile::tempdir;

    #[tokio::test]
    async fn upsert_then_load() {
        let dir = tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        let repo = SqlitePortfolioRepo::new(pool);

        let h = Holding::new(
            Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            Quantity::new(dec!(10)).unwrap(),
            Money::new(dec!(150), Currency::new("USD").unwrap()),
        );
        repo.upsert_holding(&h).await.unwrap();

        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.holdings().len(), 1);
        assert_eq!(loaded.holdings()[0], h);

        repo.delete_holding(&h.symbol).await.unwrap();
        assert!(repo.load().await.unwrap().holdings().is_empty());
    }
}
```

- [ ] **Step 2: Run + commit**

Run: `cargo test -p infrastructure sqlite::portfolio_repo::`
Expected: PASS.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add SqlitePortfolioRepo with upsert/delete/load"
```

---

### Task 3.5: `SqliteSettingsRepo`

**Files:**
- Create: `crates/infrastructure/src/sqlite/settings_repo.rs`

- [ ] **Step 1: Implement**

```rust
use application::ports::repos::{AppSettings, RepoError, SettingsRepo};
use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct SqliteSettingsRepo { pool: SqlitePool }

impl SqliteSettingsRepo { pub fn new(pool: SqlitePool) -> Self { Self { pool } } }

#[async_trait]
impl SettingsRepo for SqliteSettingsRepo {
    async fn load(&self) -> Result<AppSettings, RepoError> {
        let row: Option<(i64, String, String, f64, i64)> = sqlx::query_as(
            "SELECT poll_interval_secs, display_currency, theme, widget_opacity, widget_always_on_top
             FROM settings WHERE id = 1",
        )
        .fetch_optional(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;

        match row {
            Some((iv, cur, theme, op, ot)) => Ok(AppSettings {
                poll_interval_secs: iv as u32,
                display_currency: cur,
                theme,
                widget_opacity: op as f32,
                widget_always_on_top: ot != 0,
            }),
            None => Ok(AppSettings::default()),
        }
    }

    async fn save(&self, s: &AppSettings) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO settings (id, poll_interval_secs, display_currency, theme, widget_opacity, widget_always_on_top)
             VALUES (1, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               poll_interval_secs = excluded.poll_interval_secs,
               display_currency = excluded.display_currency,
               theme = excluded.theme,
               widget_opacity = excluded.widget_opacity,
               widget_always_on_top = excluded.widget_always_on_top",
        )
        .bind(s.poll_interval_secs as i64)
        .bind(&s.display_currency)
        .bind(&s.theme)
        .bind(s.widget_opacity as f64)
        .bind(if s.widget_always_on_top { 1_i64 } else { 0_i64 })
        .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open;
    use tempfile::tempdir;

    #[tokio::test]
    async fn round_trip_settings() {
        let dir = tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        let repo = SqliteSettingsRepo::new(pool);
        let mut s = AppSettings::default();
        s.poll_interval_secs = 7;
        s.widget_opacity = 0.5;
        repo.save(&s).await.unwrap();
        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.poll_interval_secs, 7);
        assert!((loaded.widget_opacity - 0.5).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Register module + commit**

`crates/infrastructure/src/lib.rs` add `pub mod sqlite;` and `pub mod keyring_secrets;` (if not already).

Run: `cargo test -p infrastructure sqlite::`
Expected: PASS.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add SqliteSettingsRepo (single-row settings table)"
```

---

### Task 3.6: `BinanceProvider` (`AssetProvider` impl)

**Files:**
- Create: `crates/infrastructure/src/providers/mod.rs`
- Create: `crates/infrastructure/src/providers/binance.rs`
- Modify: `crates/infrastructure/src/lib.rs`

- [ ] **Step 1: Implement + wiremock test**

`crates/infrastructure/src/providers/mod.rs`:

```rust
pub mod binance;
pub mod coingecko;
pub mod yahoo;
pub mod finnhub;
```

`crates/infrastructure/src/providers/binance.rs`:

```rust
use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use domain::{
    asset::AssetKind, candle::Candle, money::{Currency, Money}, price::Price, quote::Quote,
    symbol::Symbol,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;

pub struct BinanceProvider {
    http: Arc<dyn HttpClient>,
    base: String,
}

impl BinanceProvider {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self { http, base: "https://api.binance.com".into() }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>) -> Self {
        Self { http, base: base.into() }
    }
    fn binance_symbol(s: &Symbol) -> Option<String> {
        let qc = s.quote_currency()?;
        if s.kind() != AssetKind::Crypto { return None; }
        Some(format!("{}{}", s.ticker(), qc))
    }
}

#[derive(Deserialize)]
struct Ticker24h {
    #[serde(rename = "lastPrice")] last_price: String,
    #[serde(rename = "priceChangePercent")] price_change_percent: String,
    #[serde(rename = "quoteVolume")] quote_volume: String,
}

#[async_trait]
impl AssetProvider for BinanceProvider {
    fn name(&self) -> &'static str { "binance" }
    fn supports(&self, s: &Symbol) -> bool {
        s.kind() == AssetKind::Crypto && s.quote_currency().is_some()
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::with_capacity(symbols.len());
        for s in symbols {
            let bs = Self::binance_symbol(s)
                .ok_or_else(|| ProviderError::UnsupportedSymbol(s.to_canonical_string()))?;
            let url = format!("{}/api/v3/ticker/24hr?symbol={}", self.base, bs);
            let resp = self.http.get(&url, &[]).await.map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status == 429 { return Err(ProviderError::RateLimited { retry_after_secs: 1 }); }
            if resp.status >= 500 { return Err(ProviderError::Upstream(format!("{} {}", resp.status, String::from_utf8_lossy(&resp.body)))); }
            let t: Ticker24h = serde_json::from_slice(&resp.body)
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            let amount = Decimal::from_str(&t.last_price).map_err(|e| ProviderError::Parse(e.to_string()))?;
            let ccy = Currency::new(s.quote_currency().unwrap()).map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
            let mut q = Quote::new(s.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = Decimal::from_str(&t.price_change_percent).ok().map(|d| d / Decimal::from(100));
            q.volume_24h = Decimal::from_str(&t.quote_volume).ok();
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(&self, s: &Symbol, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Candle>, ProviderError> {
        let bs = Self::binance_symbol(s)
            .ok_or_else(|| ProviderError::UnsupportedSymbol(s.to_canonical_string()))?;
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval=1h&startTime={}&endTime={}",
            self.base, bs, from.timestamp_millis(), to.timestamp_millis()
        );
        let resp = self.http.get(&url, &[]).await.map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status >= 500 || resp.status == 429 {
            return Err(ProviderError::Upstream(format!("{}", resp.status)));
        }
        let arr: Vec<Vec<serde_json::Value>> = serde_json::from_slice(&resp.body)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        let ccy = Currency::new(s.quote_currency().unwrap()).map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
        let mut out = Vec::with_capacity(arr.len());
        for k in arr {
            let open_ms = k.get(0).and_then(|v| v.as_i64()).ok_or_else(|| ProviderError::Parse("kline open time".into()))?;
            let o = k.get(1).and_then(|v| v.as_str()).ok_or_else(|| ProviderError::Parse("o".into()))?;
            let h = k.get(2).and_then(|v| v.as_str()).ok_or_else(|| ProviderError::Parse("h".into()))?;
            let l = k.get(3).and_then(|v| v.as_str()).ok_or_else(|| ProviderError::Parse("l".into()))?;
            let c = k.get(4).and_then(|v| v.as_str()).ok_or_else(|| ProviderError::Parse("c".into()))?;
            let v = k.get(5).and_then(|v| v.as_str()).ok_or_else(|| ProviderError::Parse("v".into()))?;
            let to_money = |s: &str| {
                Ok::<_, ProviderError>(Price::new(Money::new(
                    Decimal::from_str(s).map_err(|e| ProviderError::Parse(e.to_string()))?,
                    ccy,
                )))
            };
            let opened_at = Utc.timestamp_millis_opt(open_ms).single()
                .ok_or_else(|| ProviderError::Parse("bad timestamp".into()))?;
            out.push(Candle {
                symbol: s.clone(),
                open: to_money(o)?, high: to_money(h)?, low: to_money(l)?, close: to_money(c)?,
                volume: Decimal::from_str(v).map_err(|e| ProviderError::Parse(e.to_string()))?,
                opened_at,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_ticker_24h() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/v3/ticker/24hr"))
            .and(query_param("symbol", "BTCUSDT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "lastPrice": "67000.50",
                "priceChangePercent": "1.24",
                "quoteVolume": "1234567.0",
            })))
            .mount(&server).await;

        let provider = BinanceProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USDT")).unwrap();
        let quotes = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].price.money().amount(), Decimal::from_str("67000.50").unwrap());
    }
}
```

- [ ] **Step 2: Test + commit**

Run: `cargo test -p infrastructure providers::binance::`
Expected: PASS.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add BinanceProvider for crypto quotes/candles"
```

---

### Task 3.7: `CoinGeckoProvider` (fallback crypto)

**Files:**
- Create: `crates/infrastructure/src/providers/coingecko.rs`

- [ ] **Step 1: Implement**

```rust
use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    asset::AssetKind, candle::Candle, money::{Currency, Money}, price::Price, quote::Quote,
    symbol::Symbol,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use std::collections::HashMap;

pub struct CoinGeckoProvider {
    http: Arc<dyn HttpClient>,
    base: String,
    id_for_ticker: HashMap<String, String>,
}

impl CoinGeckoProvider {
    /// `id_for_ticker` maps ticker (e.g. "BTC") to CoinGecko id (e.g. "bitcoin").
    pub fn new(http: Arc<dyn HttpClient>, id_for_ticker: HashMap<String, String>) -> Self {
        Self { http, base: "https://api.coingecko.com".into(), id_for_ticker }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>, ids: HashMap<String, String>) -> Self {
        Self { http, base: base.into(), id_for_ticker: ids }
    }
}

#[async_trait]
impl AssetProvider for CoinGeckoProvider {
    fn name(&self) -> &'static str { "coingecko" }
    fn supports(&self, s: &Symbol) -> bool {
        s.kind() == AssetKind::Crypto && self.id_for_ticker.contains_key(s.ticker())
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let ids: Vec<&str> = symbols.iter().filter_map(|s| self.id_for_ticker.get(s.ticker()).map(|x| x.as_str())).collect();
        if ids.is_empty() { return Ok(vec![]); }
        let url = format!(
            "{}/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true",
            self.base, ids.join(",")
        );
        let resp = self.http.get(&url, &[]).await.map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status == 429 { return Err(ProviderError::RateLimited { retry_after_secs: 60 }); }
        if resp.status >= 500 { return Err(ProviderError::Upstream(resp.status.to_string())); }
        let map: HashMap<String, HashMap<String, f64>> = serde_json::from_slice(&resp.body)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        let ccy = Currency::new("USD").unwrap();
        let mut out = Vec::new();
        for s in symbols {
            let Some(id) = self.id_for_ticker.get(s.ticker()) else { continue; };
            let Some(entry) = map.get(id) else { continue; };
            let Some(usd) = entry.get("usd") else { continue; };
            let amount = Decimal::from_f64_retain(*usd).ok_or_else(|| ProviderError::Parse("price not decimal".into()))?;
            let mut q = Quote::new(s.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = entry.get("usd_24h_change").and_then(|f| Decimal::from_f64_retain(*f / 100.0));
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(&self, _s: &Symbol, _from: DateTime<Utc>, _to: DateTime<Utc>) -> Result<Vec<Candle>, ProviderError> {
        // CoinGecko free-tier candle endpoint is paginated/limited — left for M2.
        Err(ProviderError::Upstream("candles not implemented for CoinGecko in M1".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_simple_price() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/v3/simple/price"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bitcoin": { "usd": 67000.5, "usd_24h_change": 1.24 }
            })))
            .mount(&server).await;
        let mut map = HashMap::new();
        map.insert("BTC".into(), "bitcoin".into());
        let p = CoinGeckoProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri(), map);
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let q = p.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].price.money().currency().as_str(), "USD");
    }
}
```

- [ ] **Step 2: Test + commit**

Run: `cargo test -p infrastructure providers::coingecko::`
Expected: PASS.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add CoinGeckoProvider fallback for crypto spot quotes"
```

---

### Task 3.8: `YahooProvider` (US equities, primary)

**Files:**
- Create: `crates/infrastructure/src/providers/yahoo.rs`

- [ ] **Step 1: Implement**

```rust
use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use domain::{
    asset::AssetKind, candle::Candle, money::{Currency, Money}, price::Price, quote::Quote,
    symbol::Symbol,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;

pub struct YahooProvider { http: Arc<dyn HttpClient>, base: String }

impl YahooProvider {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self { http, base: "https://query1.finance.yahoo.com".into() }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>) -> Self {
        Self { http, base: base.into() }
    }
}

#[async_trait]
impl AssetProvider for YahooProvider {
    fn name(&self) -> &'static str { "yahoo" }
    fn supports(&self, s: &Symbol) -> bool {
        matches!(s.kind(), AssetKind::UsEquity | AssetKind::Forex | AssetKind::Commodity)
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        if symbols.is_empty() { return Ok(vec![]); }
        let tickers: Vec<&str> = symbols.iter().map(|s| s.ticker()).collect();
        let url = format!("{}/v7/finance/quote?symbols={}", self.base, tickers.join(","));
        let resp = self.http.get(&url, &[]).await.map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status == 429 { return Err(ProviderError::RateLimited { retry_after_secs: 5 }); }
        if resp.status >= 500 { return Err(ProviderError::Upstream(resp.status.to_string())); }

        let v: serde_json::Value = serde_json::from_slice(&resp.body)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        let arr = v.pointer("/quoteResponse/result").and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("missing quoteResponse.result".into()))?;
        let mut out = Vec::new();
        for item in arr {
            let ticker = item.get("symbol").and_then(|x| x.as_str()).unwrap_or("");
            let Some(symbol) = symbols.iter().find(|s| s.ticker() == ticker) else { continue; };
            let price_f = item.get("regularMarketPrice").and_then(|x| x.as_f64())
                .ok_or_else(|| ProviderError::Parse("missing regularMarketPrice".into()))?;
            let ccy_s = item.get("currency").and_then(|x| x.as_str()).unwrap_or("USD");
            let ccy = Currency::new(ccy_s).map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
            let amount = Decimal::from_f64_retain(price_f)
                .ok_or_else(|| ProviderError::Parse("price not decimal".into()))?;
            let mut q = Quote::new(symbol.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = item.get("regularMarketChangePercent").and_then(|x| x.as_f64())
                .and_then(|f| Decimal::from_f64_retain(f / 100.0));
            q.volume_24h = item.get("regularMarketVolume").and_then(|x| x.as_f64())
                .and_then(Decimal::from_f64_retain);
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(&self, s: &Symbol, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Candle>, ProviderError> {
        let url = format!(
            "{}/v8/finance/chart/{}?period1={}&period2={}&interval=1d",
            self.base, s.ticker(), from.timestamp(), to.timestamp()
        );
        let resp = self.http.get(&url, &[]).await.map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status >= 500 { return Err(ProviderError::Upstream(resp.status.to_string())); }
        let v: serde_json::Value = serde_json::from_slice(&resp.body)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        let result = v.pointer("/chart/result/0").ok_or_else(|| ProviderError::Parse("no result".into()))?;
        let timestamps = result.pointer("/timestamp").and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("no timestamps".into()))?;
        let q = result.pointer("/indicators/quote/0").ok_or_else(|| ProviderError::Parse("no quote".into()))?;
        let opens = q.get("open").and_then(|x| x.as_array()).ok_or_else(|| ProviderError::Parse("opens".into()))?;
        let highs = q.get("high").and_then(|x| x.as_array()).ok_or_else(|| ProviderError::Parse("highs".into()))?;
        let lows = q.get("low").and_then(|x| x.as_array()).ok_or_else(|| ProviderError::Parse("lows".into()))?;
        let closes = q.get("close").and_then(|x| x.as_array()).ok_or_else(|| ProviderError::Parse("closes".into()))?;
        let volumes = q.get("volume").and_then(|x| x.as_array()).ok_or_else(|| ProviderError::Parse("volumes".into()))?;

        let ccy_s = result.pointer("/meta/currency").and_then(|x| x.as_str()).unwrap_or("USD");
        let ccy = Currency::new(ccy_s).map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
        let to_money = |f: f64| -> Result<Price, ProviderError> {
            Ok(Price::new(Money::new(
                Decimal::from_f64_retain(f).ok_or_else(|| ProviderError::Parse("nan".into()))?,
                ccy,
            )))
        };

        let mut out = Vec::new();
        for i in 0..timestamps.len() {
            let ts = timestamps[i].as_i64().ok_or_else(|| ProviderError::Parse("ts".into()))?;
            let opened_at = Utc.timestamp_opt(ts, 0).single().ok_or_else(|| ProviderError::Parse("ts".into()))?;
            let (Some(o), Some(h), Some(l), Some(c), Some(v)) = (
                opens.get(i).and_then(|x| x.as_f64()),
                highs.get(i).and_then(|x| x.as_f64()),
                lows.get(i).and_then(|x| x.as_f64()),
                closes.get(i).and_then(|x| x.as_f64()),
                volumes.get(i).and_then(|x| x.as_f64()),
            ) else { continue; };
            out.push(Candle {
                symbol: s.clone(),
                open: to_money(o)?, high: to_money(h)?, low: to_money(l)?, close: to_money(c)?,
                volume: Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO),
                opened_at,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_quote() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/v7/finance/quote"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "quoteResponse": { "result": [
                    { "symbol": "AAPL", "regularMarketPrice": 182.45, "currency": "USD",
                      "regularMarketChangePercent": 1.24, "regularMarketVolume": 52_000_000 }
                ]}
            })))
            .mount(&server).await;
        let provider = YahooProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert!(q[0].change_24h.is_some());
    }
}
```

- [ ] **Step 2: Test + commit**

Run: `cargo test -p infrastructure providers::yahoo::`
Expected: PASS.

```bash
git add crates/infrastructure/
git commit -m "feat(infra): add YahooProvider for US equities, forex, commodities"
```

---

### Task 3.9: `FinnhubProvider` (US equities, fallback for Yahoo)

**Files:**
- Create: `crates/infrastructure/src/providers/finnhub.rs`

- [ ] **Step 1: Implement**

```rust
use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    asset::AssetKind, candle::Candle, money::{Currency, Money}, price::Price, quote::Quote,
    symbol::Symbol,
};
use rust_decimal::Decimal;
use std::sync::Arc;

pub struct FinnhubProvider {
    http: Arc<dyn HttpClient>,
    base: String,
    api_key: String,
}

impl FinnhubProvider {
    pub fn new(http: Arc<dyn HttpClient>, api_key: impl Into<String>) -> Self {
        Self { http, base: "https://finnhub.io".into(), api_key: api_key.into() }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, api_key: impl Into<String>, base: impl Into<String>) -> Self {
        Self { http, base: base.into(), api_key: api_key.into() }
    }
}

#[async_trait]
impl AssetProvider for FinnhubProvider {
    fn name(&self) -> &'static str { "finnhub" }
    fn supports(&self, s: &Symbol) -> bool { s.kind() == AssetKind::UsEquity }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::with_capacity(symbols.len());
        for s in symbols {
            let url = format!("{}/api/v1/quote?symbol={}&token={}", self.base, s.ticker(), self.api_key);
            let resp = self.http.get(&url, &[]).await.map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status == 429 { return Err(ProviderError::RateLimited { retry_after_secs: 60 }); }
            if resp.status >= 500 { return Err(ProviderError::Upstream(resp.status.to_string())); }
            let v: serde_json::Value = serde_json::from_slice(&resp.body)
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            let c = v.get("c").and_then(|x| x.as_f64())
                .ok_or_else(|| ProviderError::Parse("missing c (current)".into()))?;
            let dp = v.get("dp").and_then(|x| x.as_f64());
            let ccy = Currency::new("USD").unwrap();
            let amount = Decimal::from_f64_retain(c).ok_or_else(|| ProviderError::Parse("nan".into()))?;
            let mut q = Quote::new(s.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = dp.and_then(|f| Decimal::from_f64_retain(f / 100.0));
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(&self, _s: &Symbol, _from: DateTime<Utc>, _to: DateTime<Utc>) -> Result<Vec<Candle>, ProviderError> {
        Err(ProviderError::Upstream("Finnhub candle endpoint behind paid tier; deferred".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_quote_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/v1/quote"))
            .and(query_param("symbol", "AAPL"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "c": 182.45, "dp": 1.24
            })))
            .mount(&server).await;
        let provider = FinnhubProvider::with_base(Arc::new(ReqwestHttpClient::new()), "test-key", server.uri());
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
    }
}
```

- [ ] **Step 2: Register providers**

`crates/infrastructure/src/lib.rs`:

```rust
//! Adapters implementing application ports. Only place where reqwest/sqlx/keyring live.
pub mod clock;
pub mod http;
pub mod keyring_secrets;
pub mod providers;
pub mod sqlite;
```

- [ ] **Step 3: Run all infra tests + commit**

Run: `cargo test -p infrastructure`
Expected: PASS.

Append to `docs/progress.md`:

```markdown
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
```

```bash
git add crates/infrastructure/ docs/progress.md
git commit -m "feat(infra): add FinnhubProvider and register all adapter modules"
```

---

## Phase 4 — Tauri app wiring + IPC

### Task 4.1: Wiring module (dependency assembly)

**Files:**
- Create: `app/src/wiring.rs`
- Modify: `app/src/main.rs`

- [ ] **Step 1: Wiring**

`app/src/wiring.rs`:

```rust
use application::{
    market_service::MarketService, portfolio_service::PortfolioService,
    settings_service::SettingsService,
    ports::{asset_provider::AssetProvider, http_client::HttpClient},
    poll_scheduler::PollScheduler,
};
use infrastructure::{
    clock::SystemClock, http::ReqwestHttpClient, keyring_secrets::KeyringSecretStore,
    providers::{binance::BinanceProvider, coingecko::CoinGeckoProvider, finnhub::FinnhubProvider, yahoo::YahooProvider},
    sqlite::{open, watchlist_repo::SqliteWatchlistRepo, portfolio_repo::SqlitePortfolioRepo, settings_repo::SqliteSettingsRepo},
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub struct AppState {
    pub market: Arc<MarketService>,
    pub portfolio: Arc<PortfolioService>,
    pub settings: Arc<SettingsService>,
    pub secrets: Arc<KeyringSecretStore>,
}

pub async fn assemble(db_path: PathBuf, finnhub_key: Option<String>) -> AppState {
    let pool = open(&db_path).await.expect("open sqlite");
    let watchlist_repo = Arc::new(SqliteWatchlistRepo::new(pool.clone()));
    let portfolio_repo = Arc::new(SqlitePortfolioRepo::new(pool.clone()));
    let settings_repo = Arc::new(SqliteSettingsRepo::new(pool.clone()));

    let http: Arc<dyn HttpClient> = Arc::new(ReqwestHttpClient::new());

    let mut coingecko_ids = HashMap::new();
    for (t, id) in [("BTC", "bitcoin"), ("ETH", "ethereum"), ("SOL", "solana"), ("XRP", "ripple")] {
        coingecko_ids.insert(t.into(), id.into());
    }

    let mut providers: Vec<Arc<dyn AssetProvider>> = vec![
        Arc::new(BinanceProvider::new(http.clone())),
        Arc::new(CoinGeckoProvider::new(http.clone(), coingecko_ids)),
        Arc::new(YahooProvider::new(http.clone())),
    ];
    if let Some(key) = finnhub_key {
        providers.push(Arc::new(FinnhubProvider::new(http.clone(), key)));
    }

    let market = Arc::new(MarketService::new(watchlist_repo, providers));
    let portfolio = Arc::new(PortfolioService::new(portfolio_repo, market.clone()));
    let settings = Arc::new(SettingsService::new(settings_repo));
    let secrets = Arc::new(KeyringSecretStore::new("dev.willowryu.aistock"));

    // Kick off poller (5s default). Future: read from settings.
    let clock = Arc::new(SystemClock);
    let (scheduler, _rx) = PollScheduler::new(market.clone(), clock);
    scheduler.start(std::time::Duration::from_secs(5));

    AppState { market, portfolio, settings, secrets }
}
```

- [ ] **Step 2: Test compile**

Run: `cargo check -p ai-stock-app`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add app/src/wiring.rs
git commit -m "feat(app): wiring module assembling services and adapters"
```

---

### Task 4.2: IPC commands + events

**Files:**
- Create: `app/src/ipc.rs`
- Modify: `app/src/main.rs`

- [ ] **Step 1: IPC commands**

`app/src/ipc.rs`:

```rust
use crate::wiring::AppState;
use application::ports::repos::AppSettings;
use domain::{
    asset::AssetKind, holding::Holding, money::{Currency, Money}, quantity::Quantity, symbol::Symbol,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct SymbolDto { pub kind: String, pub ticker: String, pub quote_currency: Option<String> }

#[derive(Serialize, Deserialize, Clone)]
pub struct QuoteDto {
    pub symbol: SymbolDto,
    pub price: String,
    pub currency: String,
    pub change_24h: Option<String>,
    pub observed_at: String,
}

fn kind_to_str(k: AssetKind) -> &'static str {
    match k {
        AssetKind::Crypto => "crypto", AssetKind::UsEquity => "us", AssetKind::KrEquity => "kr",
        AssetKind::Forex => "fx", AssetKind::Commodity => "com",
    }
}
fn str_to_kind(s: &str) -> Option<AssetKind> {
    Some(match s {
        "crypto" => AssetKind::Crypto, "us" => AssetKind::UsEquity, "kr" => AssetKind::KrEquity,
        "fx" => AssetKind::Forex, "com" => AssetKind::Commodity, _ => return None,
    })
}
fn dto_to_symbol(d: &SymbolDto) -> Result<Symbol, String> {
    let k = str_to_kind(&d.kind).ok_or_else(|| format!("bad kind: {}", d.kind))?;
    Symbol::new(k, &d.ticker, d.quote_currency.as_deref()).map_err(|e| format!("{e:?}"))
}
fn symbol_to_dto(s: &Symbol) -> SymbolDto {
    SymbolDto { kind: kind_to_str(s.kind()).into(), ticker: s.ticker().into(), quote_currency: s.quote_currency().map(|x| x.into()) }
}

#[tauri::command]
pub async fn watchlist_get(state: State<'_, AppState>) -> Result<Vec<SymbolDto>, String> {
    let wl = state.market.load_watchlist().await.map_err(|e| e.to_string())?;
    Ok(wl.symbols().iter().map(symbol_to_dto).collect())
}

#[tauri::command]
pub async fn watchlist_add(state: State<'_, AppState>, symbol: SymbolDto) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    state.market.add_to_watchlist(s).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn watchlist_remove(state: State<'_, AppState>, symbol: SymbolDto) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    state.market.remove_from_watchlist(&s).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn quotes_snapshot(state: State<'_, AppState>) -> Result<Vec<QuoteDto>, String> {
    let snap = state.market.snapshot().await;
    Ok(snap.values().map(|q| QuoteDto {
        symbol: symbol_to_dto(&q.symbol),
        price: q.price.money().amount().to_string(),
        currency: q.price.money().currency().as_str().to_string(),
        change_24h: q.change_24h.map(|d| d.to_string()),
        observed_at: q.observed_at.to_rfc3339(),
    }).collect())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HoldingDto {
    pub symbol: SymbolDto,
    pub quantity: String,
    pub avg_cost_amount: String,
    pub avg_cost_currency: String,
}

#[tauri::command]
pub async fn portfolio_upsert(state: State<'_, AppState>, holding: HoldingDto) -> Result<(), String> {
    let symbol = dto_to_symbol(&holding.symbol)?;
    let qty = Quantity::new(Decimal::from_str(&holding.quantity).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{e:?}"))?;
    let ccy = Currency::new(&holding.avg_cost_currency).map_err(|e| format!("{e:?}"))?;
    let amt = Decimal::from_str(&holding.avg_cost_amount).map_err(|e| e.to_string())?;
    state.portfolio
        .upsert_holding(Holding::new(symbol, qty, Money::new(amt, ccy)))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn portfolio_delete(state: State<'_, AppState>, symbol: SymbolDto) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    state.portfolio.delete_holding(&s).await.map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
pub struct PortfolioValuationDto {
    pub total_value: Option<String>,
    pub total_value_currency: Option<String>,
    pub total_pnl: Option<String>,
    pub holdings: Vec<HoldingValuationDto>,
}

#[derive(Serialize, Clone)]
pub struct HoldingValuationDto {
    pub symbol: SymbolDto,
    pub market_value: Option<String>,
    pub cost_basis: String,
    pub pnl: Option<String>,
}

#[tauri::command]
pub async fn portfolio_valuation(state: State<'_, AppState>) -> Result<PortfolioValuationDto, String> {
    let v = state.portfolio.valuation().await.map_err(|e| e.to_string())?;
    Ok(PortfolioValuationDto {
        total_value: v.total_value.map(|m| m.amount().to_string()),
        total_value_currency: v.total_value.map(|m| m.currency().as_str().to_string()),
        total_pnl: v.total_pnl.map(|m| m.amount().to_string()),
        holdings: v.per_holding.iter().map(|h| HoldingValuationDto {
            symbol: symbol_to_dto(&h.symbol),
            market_value: h.market_value.map(|m| m.amount().to_string()),
            cost_basis: h.cost_basis.amount().to_string(),
            pnl: h.pnl_absolute.map(|m| m.amount().to_string()),
        }).collect(),
    })
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.settings.get().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn settings_save(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    state.settings.save(settings).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Wire commands and emit events**

Replace `app/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc;
mod wiring;

use std::time::Duration;
use tauri::{Emitter, Manager};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            ipc::watchlist_get, ipc::watchlist_add, ipc::watchlist_remove,
            ipc::quotes_snapshot,
            ipc::portfolio_upsert, ipc::portfolio_delete, ipc::portfolio_valuation,
            ipc::settings_get, ipc::settings_save,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let db_path = handle
                    .path()
                    .app_data_dir()
                    .expect("app data dir")
                    .join("ai-stock.db");
                std::fs::create_dir_all(db_path.parent().unwrap()).ok();
                let state = wiring::assemble(db_path, std::env::var("FINNHUB_API_KEY").ok()).await;
                handle.manage(state);

                // Periodic event emit loop (every 1s): broadcast snapshot to UI.
                let market = handle.state::<wiring::AppState>().market.clone();
                loop {
                    let snap = market.snapshot().await;
                    let dto: Vec<ipc::QuoteDto> = snap.values().map(|q| ipc::QuoteDto {
                        symbol: ipc::SymbolDto {
                            kind: match q.symbol.kind() {
                                domain::asset::AssetKind::Crypto => "crypto".into(),
                                domain::asset::AssetKind::UsEquity => "us".into(),
                                domain::asset::AssetKind::KrEquity => "kr".into(),
                                domain::asset::AssetKind::Forex => "fx".into(),
                                domain::asset::AssetKind::Commodity => "com".into(),
                            },
                            ticker: q.symbol.ticker().into(),
                            quote_currency: q.symbol.quote_currency().map(|x| x.into()),
                        },
                        price: q.price.money().amount().to_string(),
                        currency: q.price.money().currency().as_str().to_string(),
                        change_24h: q.change_24h.map(|d| d.to_string()),
                        observed_at: q.observed_at.to_rfc3339(),
                    }).collect();
                    let _ = handle.emit("quote-update", dto);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Build smoke**

Run: `cargo build -p ai-stock-app`
Expected: builds (Tauri needs a `dist/` for full build — but `cargo build` only compiles the binary).

- [ ] **Step 4: Commit**

```bash
git add app/src/
git commit -m "feat(app): wire IPC commands and quote-update broadcast loop"
```

---

## Phase 5 — Frontend (React)

### Task 5.1: Typed IPC bindings

**Files:**
- Create: `src/lib/ipc.ts`

- [ ] **Step 1: Bindings**

`src/lib/ipc.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AssetKind = "crypto" | "us" | "kr" | "fx" | "com";

export interface SymbolDto {
  kind: AssetKind;
  ticker: string;
  quote_currency?: string | null;
}

export interface QuoteDto {
  symbol: SymbolDto;
  price: string;       // decimal-as-string
  currency: string;
  change_24h: string | null;
  observed_at: string; // RFC3339
}

export interface HoldingDto {
  symbol: SymbolDto;
  quantity: string;
  avg_cost_amount: string;
  avg_cost_currency: string;
}

export interface HoldingValuationDto {
  symbol: SymbolDto;
  market_value: string | null;
  cost_basis: string;
  pnl: string | null;
}

export interface PortfolioValuationDto {
  total_value: string | null;
  total_value_currency: string | null;
  total_pnl: string | null;
  holdings: HoldingValuationDto[];
}

export interface AppSettingsDto {
  poll_interval_secs: number;
  display_currency: string;
  theme: string;
  widget_opacity: number;
  widget_always_on_top: boolean;
}

export const ipc = {
  watchlistGet: () => invoke<SymbolDto[]>("watchlist_get"),
  watchlistAdd: (symbol: SymbolDto) => invoke<void>("watchlist_add", { symbol }),
  watchlistRemove: (symbol: SymbolDto) => invoke<void>("watchlist_remove", { symbol }),

  quotesSnapshot: () => invoke<QuoteDto[]>("quotes_snapshot"),

  portfolioUpsert: (holding: HoldingDto) => invoke<void>("portfolio_upsert", { holding }),
  portfolioDelete: (symbol: SymbolDto) => invoke<void>("portfolio_delete", { symbol }),
  portfolioValuation: () => invoke<PortfolioValuationDto>("portfolio_valuation"),

  settingsGet: () => invoke<AppSettingsDto>("settings_get"),
  settingsSave: (settings: AppSettingsDto) => invoke<void>("settings_save", { settings }),
};

export function onQuoteUpdate(cb: (quotes: QuoteDto[]) => void): Promise<UnlistenFn> {
  return listen<QuoteDto[]>("quote-update", (e) => cb(e.payload));
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/ipc.ts
git commit -m "feat(web): add typed IPC bindings for tauri commands and events"
```

---

### Task 5.2: Zustand stores

**Files:**
- Create: `src/lib/state/watchlistStore.ts`
- Create: `src/lib/state/portfolioStore.ts`
- Create: `src/lib/state/settingsStore.ts`
- Create: `src/lib/state/quotesStore.ts`

- [ ] **Step 1: Quotes store (events → memory)**

`src/lib/state/quotesStore.ts`:

```typescript
import { create } from "zustand";
import type { QuoteDto, SymbolDto } from "../ipc";

function key(s: SymbolDto): string {
  return s.quote_currency ? `${s.kind}:${s.ticker}:${s.quote_currency}` : `${s.kind}:${s.ticker}`;
}

interface QuotesState {
  bySymbol: Record<string, QuoteDto>;
  apply(updates: QuoteDto[]): void;
}

export const useQuotesStore = create<QuotesState>((set) => ({
  bySymbol: {},
  apply(updates) {
    set((prev) => {
      const next = { ...prev.bySymbol };
      for (const q of updates) next[key(q.symbol)] = q;
      return { bySymbol: next };
    });
  },
}));

export function quoteKey(s: SymbolDto): string { return key(s); }
```

- [ ] **Step 2: Watchlist store**

`src/lib/state/watchlistStore.ts`:

```typescript
import { create } from "zustand";
import { ipc, type SymbolDto } from "../ipc";

interface WatchlistState {
  symbols: SymbolDto[];
  loading: boolean;
  load(): Promise<void>;
  add(s: SymbolDto): Promise<void>;
  remove(s: SymbolDto): Promise<void>;
}

export const useWatchlistStore = create<WatchlistState>((set) => ({
  symbols: [],
  loading: false,
  async load() {
    set({ loading: true });
    try { set({ symbols: await ipc.watchlistGet() }); } finally { set({ loading: false }); }
  },
  async add(s) { await ipc.watchlistAdd(s); set((p) => ({ symbols: [...p.symbols.filter(x => !sameSymbol(x, s)), s] })); },
  async remove(s) { await ipc.watchlistRemove(s); set((p) => ({ symbols: p.symbols.filter(x => !sameSymbol(x, s)) })); },
}));

function sameSymbol(a: SymbolDto, b: SymbolDto) {
  return a.kind === b.kind && a.ticker === b.ticker && (a.quote_currency ?? null) === (b.quote_currency ?? null);
}
```

- [ ] **Step 3: Portfolio store**

`src/lib/state/portfolioStore.ts`:

```typescript
import { create } from "zustand";
import { ipc, type HoldingDto, type PortfolioValuationDto, type SymbolDto } from "../ipc";

interface PortfolioState {
  valuation: PortfolioValuationDto | null;
  refresh(): Promise<void>;
  upsert(h: HoldingDto): Promise<void>;
  remove(s: SymbolDto): Promise<void>;
}

export const usePortfolioStore = create<PortfolioState>((set) => ({
  valuation: null,
  async refresh() { set({ valuation: await ipc.portfolioValuation() }); },
  async upsert(h) { await ipc.portfolioUpsert(h); set({ valuation: await ipc.portfolioValuation() }); },
  async remove(s) { await ipc.portfolioDelete(s); set({ valuation: await ipc.portfolioValuation() }); },
}));
```

- [ ] **Step 4: Settings store**

`src/lib/state/settingsStore.ts`:

```typescript
import { create } from "zustand";
import { ipc, type AppSettingsDto } from "../ipc";

interface SettingsState {
  settings: AppSettingsDto | null;
  load(): Promise<void>;
  save(s: AppSettingsDto): Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  async load() { set({ settings: await ipc.settingsGet() }); },
  async save(s) { await ipc.settingsSave(s); set({ settings: s }); },
}));
```

- [ ] **Step 5: Frontend tests**

`src/lib/state/quotesStore.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { useQuotesStore } from "./quotesStore";

describe("quotesStore", () => {
  it("apply merges by symbol key", () => {
    const { apply } = useQuotesStore.getState();
    apply([{
      symbol: { kind: "crypto", ticker: "BTC", quote_currency: "USD" },
      price: "67000", currency: "USD", change_24h: "0.0124",
      observed_at: new Date().toISOString(),
    }]);
    const snap = useQuotesStore.getState().bySymbol;
    expect(snap["crypto:BTC:USD"].price).toBe("67000");
  });
});
```

Run: `npm test`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/lib/state/
git commit -m "feat(web): add zustand stores for watchlist, quotes, portfolio, settings"
```

---

### Task 5.3: Main window — layout + Watchlist + DetailPane

**Files:**
- Create: `src/components/Watchlist.tsx`
- Create: `src/components/DetailPane.tsx`
- Create: `src/components/AddSymbolDialog.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: `Watchlist.tsx`**

```typescript
import { useEffect } from "react";
import clsx from "clsx";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
import type { SymbolDto } from "../lib/ipc";

interface Props {
  selected: SymbolDto | null;
  onSelect(s: SymbolDto): void;
  onAdd(): void;
}

export function Watchlist({ selected, onSelect, onAdd }: Props) {
  const { symbols, load, remove } = useWatchlistStore();
  const quotes = useQuotesStore((s) => s.bySymbol);

  useEffect(() => { load(); }, [load]);

  return (
    <aside className="w-64 border-r border-slate-800 flex flex-col">
      <div className="p-3 flex justify-between items-center border-b border-slate-800">
        <span className="text-xs uppercase text-slate-400">Watchlist</span>
        <button onClick={onAdd} className="text-xs px-2 py-1 rounded bg-slate-800 hover:bg-slate-700">+ Add</button>
      </div>
      <ul className="flex-1 overflow-y-auto">
        {symbols.map((s) => {
          const q = quotes[quoteKey(s)];
          const isSelected = selected && s.kind === selected.kind && s.ticker === selected.ticker;
          const changePct = q?.change_24h ? Number(q.change_24h) * 100 : null;
          return (
            <li key={quoteKey(s)}
                onClick={() => onSelect(s)}
                className={clsx("px-3 py-2 cursor-pointer flex justify-between items-center", isSelected && "bg-slate-800")}>
              <div>
                <div className="text-sm">{s.ticker}</div>
                <div className="text-[10px] text-slate-500 uppercase">{s.kind}</div>
              </div>
              <div className="text-right">
                <div className="text-sm tabular-nums">{q?.price ?? "—"}</div>
                <div className={clsx("text-[10px] tabular-nums",
                  changePct === null ? "text-slate-500" : changePct >= 0 ? "text-emerald-400" : "text-rose-400")}>
                  {changePct === null ? "" : `${changePct >= 0 ? "+" : ""}${changePct.toFixed(2)}%`}
                </div>
              </div>
              <button onClick={(e) => { e.stopPropagation(); remove(s); }}
                className="ml-2 text-slate-600 hover:text-rose-400 text-xs">×</button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
```

- [ ] **Step 2: `DetailPane.tsx`**

```typescript
import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
import type { SymbolDto } from "../lib/ipc";

export function DetailPane({ symbol }: { symbol: SymbolDto | null }) {
  const quotes = useQuotesStore((s) => s.bySymbol);
  if (!symbol) {
    return <div className="flex-1 flex items-center justify-center text-slate-500 text-sm">워치리스트에서 종목을 선택하세요</div>;
  }
  const q = quotes[quoteKey(symbol)];
  const changePct = q?.change_24h ? Number(q.change_24h) * 100 : null;
  return (
    <main className="flex-1 p-6">
      <div className="text-xs uppercase text-slate-500">{symbol.kind}</div>
      <h2 className="text-2xl font-semibold">{symbol.ticker}{symbol.quote_currency ? `/${symbol.quote_currency}` : ""}</h2>
      <div className="mt-4 flex items-baseline gap-3">
        <div className="text-4xl tabular-nums">{q?.price ?? "—"}</div>
        <div className="text-slate-400">{q?.currency ?? ""}</div>
        {changePct !== null && (
          <div className={changePct >= 0 ? "text-emerald-400" : "text-rose-400"}>
            {changePct >= 0 ? "+" : ""}{changePct.toFixed(2)}%
          </div>
        )}
      </div>
      <p className="mt-2 text-xs text-slate-500">Last observed: {q?.observed_at ?? "—"}</p>
    </main>
  );
}
```

- [ ] **Step 3: `AddSymbolDialog.tsx`**

```typescript
import { useState } from "react";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import type { AssetKind, SymbolDto } from "../lib/ipc";

export function AddSymbolDialog({ onClose }: { onClose(): void }) {
  const add = useWatchlistStore((s) => s.add);
  const [kind, setKind] = useState<AssetKind>("crypto");
  const [ticker, setTicker] = useState("BTC");
  const [quote, setQuote] = useState("USD");
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const symbol: SymbolDto = { kind, ticker: ticker.toUpperCase(), quote_currency: kind === "crypto" ? quote.toUpperCase() : null };
    try { await add(symbol); onClose(); } catch (err) { setError(String(err)); }
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <form onClick={(e) => e.stopPropagation()} onSubmit={submit}
            className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-80 space-y-3">
        <h3 className="text-lg font-semibold">종목 추가</h3>
        <label className="block text-xs">자산 유형
          <select value={kind} onChange={(e) => setKind(e.target.value as AssetKind)}
                  className="mt-1 w-full bg-slate-800 rounded p-1.5">
            <option value="crypto">Crypto</option>
            <option value="us">US Equity</option>
            <option value="kr">KR Equity (M2)</option>
            <option value="fx">Forex</option>
            <option value="com">Commodity</option>
          </select>
        </label>
        <label className="block text-xs">티커
          <input value={ticker} onChange={(e) => setTicker(e.target.value)}
                 className="mt-1 w-full bg-slate-800 rounded p-1.5" />
        </label>
        {kind === "crypto" && (
          <label className="block text-xs">호가 통화
            <input value={quote} onChange={(e) => setQuote(e.target.value)}
                   className="mt-1 w-full bg-slate-800 rounded p-1.5" />
          </label>
        )}
        {error && <div className="text-rose-400 text-xs">{error}</div>}
        <div className="flex gap-2 justify-end">
          <button type="button" onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button type="submit" className="px-3 py-1 text-sm rounded bg-emerald-600">추가</button>
        </div>
      </form>
    </div>
  );
}
```

- [ ] **Step 4: `App.tsx`**

```typescript
import { useEffect, useState } from "react";
import { Watchlist } from "./components/Watchlist";
import { DetailPane } from "./components/DetailPane";
import { AddSymbolDialog } from "./components/AddSymbolDialog";
import { PortfolioPanel } from "./components/PortfolioPanel";
import { useQuotesStore } from "./lib/state/quotesStore";
import { onQuoteUpdate, ipc, type SymbolDto } from "./lib/ipc";
import { usePortfolioStore } from "./lib/state/portfolioStore";

export default function App() {
  const [selected, setSelected] = useState<SymbolDto | null>(null);
  const [adding, setAdding] = useState(false);
  const apply = useQuotesStore((s) => s.apply);
  const refreshPortfolio = usePortfolioStore((s) => s.refresh);

  useEffect(() => {
    ipc.quotesSnapshot().then(apply);
    refreshPortfolio();
    const unsub = onQuoteUpdate((updates) => {
      apply(updates);
      refreshPortfolio();
    });
    return () => { unsub.then((fn) => fn()); };
  }, [apply, refreshPortfolio]);

  return (
    <div className="h-screen flex flex-col">
      <header className="h-10 border-b border-slate-800 px-4 flex items-center text-sm">
        <span className="font-semibold">ai-stock</span>
      </header>
      <div className="flex flex-1 min-h-0">
        <Watchlist selected={selected} onSelect={setSelected} onAdd={() => setAdding(true)} />
        <DetailPane symbol={selected} />
        <PortfolioPanel />
      </div>
      {adding && <AddSymbolDialog onClose={() => setAdding(false)} />}
    </div>
  );
}
```

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat(web): main window with watchlist, detail pane, add-symbol dialog"
```

---

### Task 5.4: PortfolioPanel + Settings + i18n

**Files:**
- Create: `src/components/PortfolioPanel.tsx`
- Create: `src/components/Settings.tsx`
- Create: `src/i18n/ko.json`
- Create: `src/i18n/en.json`

- [ ] **Step 1: `PortfolioPanel.tsx`**

```typescript
import { useEffect, useState } from "react";
import { usePortfolioStore } from "../lib/state/portfolioStore";
import type { AssetKind, HoldingDto } from "../lib/ipc";

export function PortfolioPanel() {
  const { valuation, refresh, upsert, remove } = usePortfolioStore();
  const [open, setOpen] = useState(false);

  useEffect(() => { refresh(); }, [refresh]);

  return (
    <aside className="w-80 border-l border-slate-800 flex flex-col">
      <div className="p-3 border-b border-slate-800 flex justify-between items-center">
        <span className="text-xs uppercase text-slate-400">Portfolio</span>
        <button onClick={() => setOpen(true)} className="text-xs px-2 py-1 rounded bg-slate-800 hover:bg-slate-700">+ Add</button>
      </div>

      <div className="p-3 border-b border-slate-800">
        <div className="text-xs text-slate-500">총 평가액</div>
        <div className="text-xl tabular-nums">
          {valuation?.total_value ?? "—"} {valuation?.total_value_currency ?? ""}
        </div>
        <div className={"text-xs " + ((Number(valuation?.total_pnl ?? "0") >= 0) ? "text-emerald-400" : "text-rose-400")}>
          P&L: {valuation?.total_pnl ?? "—"}
        </div>
      </div>

      <ul className="flex-1 overflow-y-auto text-xs">
        {valuation?.holdings.map((h, i) => (
          <li key={i} className="p-2 border-b border-slate-900 flex justify-between">
            <div>
              <div>{h.symbol.ticker}</div>
              <div className="text-slate-500">cost: {h.cost_basis}</div>
            </div>
            <div className="text-right">
              <div>{h.market_value ?? "—"}</div>
              <div className={(Number(h.pnl ?? "0") >= 0) ? "text-emerald-400" : "text-rose-400"}>
                {h.pnl ?? "—"}
              </div>
            </div>
            <button onClick={() => remove(h.symbol)} className="ml-2 text-slate-600 hover:text-rose-400">×</button>
          </li>
        ))}
      </ul>

      {open && <AddHoldingDialog onClose={() => setOpen(false)} onSubmit={upsert} />}
    </aside>
  );
}

function AddHoldingDialog({ onClose, onSubmit }: { onClose(): void; onSubmit(h: HoldingDto): Promise<void> }) {
  const [kind, setKind] = useState<AssetKind>("crypto");
  const [ticker, setTicker] = useState("BTC");
  const [quote, setQuote] = useState("USD");
  const [qty, setQty] = useState("0");
  const [cost, setCost] = useState("0");
  const [ccy, setCcy] = useState("USD");
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      await onSubmit({
        symbol: { kind, ticker: ticker.toUpperCase(), quote_currency: kind === "crypto" ? quote.toUpperCase() : null },
        quantity: qty, avg_cost_amount: cost, avg_cost_currency: ccy.toUpperCase(),
      });
      onClose();
    } catch (err) { setError(String(err)); }
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <form onClick={(e) => e.stopPropagation()} onSubmit={submit}
            className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-96 space-y-3">
        <h3 className="text-lg font-semibold">보유 자산 추가</h3>
        <select value={kind} onChange={(e) => setKind(e.target.value as AssetKind)} className="w-full bg-slate-800 rounded p-1.5">
          <option value="crypto">Crypto</option>
          <option value="us">US Equity</option>
        </select>
        <input value={ticker} onChange={(e) => setTicker(e.target.value)} placeholder="ticker" className="w-full bg-slate-800 rounded p-1.5" />
        {kind === "crypto" && (
          <input value={quote} onChange={(e) => setQuote(e.target.value)} placeholder="quote currency" className="w-full bg-slate-800 rounded p-1.5" />
        )}
        <input value={qty} onChange={(e) => setQty(e.target.value)} placeholder="수량" className="w-full bg-slate-800 rounded p-1.5" />
        <input value={cost} onChange={(e) => setCost(e.target.value)} placeholder="평단가" className="w-full bg-slate-800 rounded p-1.5" />
        <input value={ccy} onChange={(e) => setCcy(e.target.value)} placeholder="통화" className="w-full bg-slate-800 rounded p-1.5" />
        {error && <div className="text-rose-400 text-xs">{error}</div>}
        <div className="flex gap-2 justify-end">
          <button type="button" onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button type="submit" className="px-3 py-1 text-sm rounded bg-emerald-600">저장</button>
        </div>
      </form>
    </div>
  );
}
```

- [ ] **Step 2: `Settings.tsx`** (minimal; opened via menu/keystroke later)

```typescript
import { useEffect, useState } from "react";
import { useSettingsStore } from "../lib/state/settingsStore";
import type { AppSettingsDto } from "../lib/ipc";

export function Settings({ onClose }: { onClose(): void }) {
  const { settings, load, save } = useSettingsStore();
  const [draft, setDraft] = useState<AppSettingsDto | null>(null);
  useEffect(() => { load().then(() => setDraft(useSettingsStore.getState().settings)); }, [load]);
  if (!draft) return null;

  async function commit() {
    await save(draft);
    onClose();
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-96 space-y-3">
        <h3 className="text-lg font-semibold">설정</h3>
        <label className="block text-xs">폴링 주기 (초)
          <input type="number" min={1} max={300} value={draft.poll_interval_secs}
                 onChange={(e) => setDraft({ ...draft, poll_interval_secs: Number(e.target.value) })}
                 className="mt-1 w-full bg-slate-800 rounded p-1.5" />
        </label>
        <label className="block text-xs">표시 통화
          <input value={draft.display_currency}
                 onChange={(e) => setDraft({ ...draft, display_currency: e.target.value.toUpperCase() })}
                 className="mt-1 w-full bg-slate-800 rounded p-1.5" />
        </label>
        <label className="block text-xs">위젯 투명도 ({draft.widget_opacity.toFixed(2)})
          <input type="range" min={0.1} max={1} step={0.05} value={draft.widget_opacity}
                 onChange={(e) => setDraft({ ...draft, widget_opacity: Number(e.target.value) })}
                 className="mt-1 w-full" />
        </label>
        <div className="flex gap-2 justify-end">
          <button onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button onClick={commit} className="px-3 py-1 text-sm rounded bg-emerald-600">저장</button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: i18n stub**

`src/i18n/ko.json`:

```json
{
  "watchlist.title": "관심 종목",
  "portfolio.title": "포트폴리오",
  "settings.title": "설정"
}
```

`src/i18n/en.json`:

```json
{
  "watchlist.title": "Watchlist",
  "portfolio.title": "Portfolio",
  "settings.title": "Settings"
}
```

> Full i18n wiring (`react-i18next`) is deferred to M2; for M1 we keep strings inline but the JSON files exist for future use.

- [ ] **Step 4: Commit**

```bash
git add src/
git commit -m "feat(web): portfolio panel with holdings dialog and settings modal"
```

---

### Task 5.5: Floating widget with transparency slider

**Files:**
- Modify: `src/widget.tsx`
- Create: `src/components/widget/WidgetRow.tsx`

- [ ] **Step 1: `WidgetRow.tsx`**

```typescript
import clsx from "clsx";
import type { QuoteDto } from "../../lib/ipc";

export function WidgetRow({ q }: { q: QuoteDto }) {
  const change = q.change_24h ? Number(q.change_24h) * 100 : null;
  return (
    <div className="flex justify-between items-center text-xs px-2 py-1">
      <span className="opacity-90">{q.symbol.ticker}</span>
      <span className="tabular-nums">{q.price}</span>
      <span className={clsx("tabular-nums w-14 text-right",
        change === null ? "text-slate-400" : change >= 0 ? "text-emerald-400" : "text-rose-400")}>
        {change === null ? "" : `${change >= 0 ? "+" : ""}${change.toFixed(2)}%`}
      </span>
    </div>
  );
}
```

- [ ] **Step 2: `widget.tsx`**

```typescript
import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { onQuoteUpdate, ipc } from "./lib/ipc";
import { useQuotesStore } from "./lib/state/quotesStore";
import { useSettingsStore } from "./lib/state/settingsStore";
import { WidgetRow } from "./components/widget/WidgetRow";
import "./index.css";

function Widget() {
  const quotes = useQuotesStore((s) => Object.values(s.bySymbol));
  const apply = useQuotesStore((s) => s.apply);
  const { settings, load, save } = useSettingsStore();
  const [opacity, setOpacity] = useState(0.85);

  useEffect(() => {
    ipc.quotesSnapshot().then(apply);
    const unsub = onQuoteUpdate(apply);
    load().then(() => {
      const s = useSettingsStore.getState().settings;
      if (s) setOpacity(s.widget_opacity);
    });
    return () => { unsub.then((fn) => fn()); };
  }, [apply, load]);

  async function changeOpacity(v: number) {
    setOpacity(v);
    if (settings) await save({ ...settings, widget_opacity: v });
  }

  return (
    <div
      className="rounded-lg p-2 select-none"
      style={{ backgroundColor: `rgba(15,23,42,${opacity})`, color: "#e2e8f0", height: "100vh" }}
      data-tauri-drag-region
    >
      <div className="flex justify-between items-center text-[10px] text-slate-400 px-2 mb-1">
        <span>ai-stock</span>
        <input type="range" min={0.1} max={1} step={0.05}
               value={opacity}
               onChange={(e) => changeOpacity(Number(e.target.value))}
               className="w-20" />
        <button onClick={() => getCurrentWebviewWindow().hide()} className="hover:text-slate-200">×</button>
      </div>
      <div>
        {quotes.slice(0, 5).map((q) => <WidgetRow key={`${q.symbol.kind}:${q.symbol.ticker}`} q={q} />)}
        {quotes.length === 0 && <div className="text-center text-[11px] text-slate-500 py-2">no quotes yet</div>}
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode><Widget /></React.StrictMode>,
);
```

- [ ] **Step 3: Add toggle command for the widget window**

Append to `app/src/ipc.rs`:

```rust
#[tauri::command]
pub async fn widget_toggle(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = tauri::Manager::get_webview_window(&app, "widget") {
        if win.is_visible().unwrap_or(false) {
            win.hide().map_err(|e| e.to_string())?;
        } else {
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
```

Register in `app/src/main.rs` invoke_handler list (add `ipc::widget_toggle` to `tauri::generate_handler![...]`), and add a button on the header in `App.tsx`. At the top of `App.tsx` add:

```typescript
import { invoke } from "@tauri-apps/api/core";
```

Then inside the `<header>` element (right side):

```typescript
<button onClick={() => invoke("widget_toggle")} className="ml-auto text-xs px-2 py-1 rounded bg-slate-800">위젯</button>
```

- [ ] **Step 4: Manual test**

Run: `npm run tauri dev`
Expected: main window appears, widget hidden. Click 위젯 to show; transparency slider works; quote rows show after a few seconds.

- [ ] **Step 5: Commit + progress**

Append to `docs/progress.md`:

```markdown
### Phase 5 — Frontend

- [x] Task 5.1: Typed IPC bindings.
- [x] Task 5.2: Zustand stores.
- [x] Task 5.3: Watchlist + DetailPane + AddSymbol.
- [x] Task 5.4: PortfolioPanel + Settings + i18n stubs.
- [x] Task 5.5: Floating widget with transparency.
```

```bash
git add src/ app/src/ docs/progress.md
git commit -m "feat(web): floating widget with transparency slider and toggle command"
```

---

## Phase 6 — E2E + close-out

### Task 6.1: `tauri-driver` + WebdriverIO golden path

**Files:**
- Create: `e2e/wdio.conf.ts`
- Create: `e2e/specs/golden-path.e2e.ts`

- [ ] **Step 1: Install `tauri-driver`**

Run: `cargo install tauri-driver --locked` (uses platform-specific WebDriver — `WebKitWebDriver` on macOS, `msedgedriver` on Windows). Document this in `docs/CONTEXT.md`.

- [ ] **Step 2: `wdio.conf.ts`**

```typescript
export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [{
    "tauri:options": { application: "../target/debug/ai-stock-app" },
  }] as unknown as WebdriverIO.Capabilities[],
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 60000 },
  hostname: "127.0.0.1",
  port: 4444,
  services: [],
  beforeSession: () => {
    require("child_process").spawn("tauri-driver", [], { stdio: "inherit" });
  },
};
```

- [ ] **Step 3: Golden path test**

`e2e/specs/golden-path.e2e.ts`:

```typescript
import { browser, $ } from "@wdio/globals";

describe("ai-stock golden path", () => {
  it("starts and shows watchlist heading", async () => {
    await browser.pause(2000);
    const heading = await $("text=ai-stock");
    await heading.waitForDisplayed({ timeout: 10000 });
  });
});
```

- [ ] **Step 4: Build the app once for the test target**

Run: `cargo build -p ai-stock-app && npm run build`
Expected: produces `target/debug/ai-stock-app` and frontend `dist/`.

- [ ] **Step 5: Run e2e**

Run: `npm run e2e`
Expected: PASS. (If `tauri-driver` not installed: install it and retry.)

- [ ] **Step 6: Commit**

```bash
git add e2e/
git commit -m "test(e2e): tauri-driver golden-path smoke test"
```

---

### Task 6.2: Final M1 close-out

**Files:**
- Modify: `docs/CONTEXT.md`
- Modify: `docs/progress.md`
- Create: `docs/adr/0002-polling-only-for-m1.md`

- [ ] **Step 1: ADR 0002**

`docs/adr/0002-polling-only-for-m1.md`:

```markdown
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
```

- [ ] **Step 2: Update `CONTEXT.md`**

Append a "Current state" entry: "M1 complete. Watchlist + portfolio + floating widget operational against Binance/CoinGecko/Yahoo/Finnhub. Streaming and KR stocks deferred to M2."

- [ ] **Step 3: Update `progress.md`**

```markdown
### Phase 6 — E2E + close-out

- [x] Task 6.1: tauri-driver E2E smoke.
- [x] Task 6.2: M1 close-out — ADR 0002, CONTEXT update.

## 2026-05-?? — M1 complete

- Working app: cross-platform Tauri shell, hybrid main + floating widget, 4 asset adapters, portfolio P&L.
- Tests: domain ~75% via cargo test + proptest; infra via wiremock + temp sqlite; E2E golden path via tauri-driver.
- Next: draft M2 plan (KR stocks via Naver, technical indicators, alerts + notifications, forex/commodities polish).
```

- [ ] **Step 4: Commit**

```bash
git add docs/
git commit -m "docs: ADR 0002 (polling-only M1) and close-out M1 progress log"
```

---

## Done

M1 produces a working, testable desktop app. Open questions and next plans:

- **M2 plan:** technical indicators, alerts, KR stocks (Naver/KIS), forex polish.
- **M3 plan:** BYOK AI integration (OpenAI/Anthropic/Gemini), news providers, prompt templates, streaming chat UI.

Each subsequent milestone should get its own plan document written when M1 has shipped and any architectural surprises have been folded back into the design spec.
