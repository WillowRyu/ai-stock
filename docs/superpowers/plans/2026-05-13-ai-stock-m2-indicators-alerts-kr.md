# ai-stock M2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) tracking.

**Goal:** Add technical indicators (MA, RSI, MACD, Bollinger), price-threshold alerts with OS-native notifications, KR stocks coverage (Naver scraping primary, KIS OpenAPI optional), forex/commodity polish, and a few hygiene fixes that were deferred from M1.

**Architecture:** Same DDD layering as M1. Indicators live in `domain/` as pure functions on `Vec<Candle>`. Alerts get a new bounded context (`AlertRule` entity, `AlertEvaluator` domain service, `AlertRepo` port, `AlertService` orchestration, `SqliteAlertRepo` adapter, `TauriNotifier` adapter). KR providers slot into the existing `AssetProvider` trait.

**Tech Stack:** Same as M1 (Rust workspace + Tauri 2 + React/TS + Tailwind). New crates introduced: none. New runtime deps: `scraper = "0.20"` (Naver HTML parsing), `tauri-plugin-notification` (already in M1 manifest).

**Reference:** `docs/superpowers/specs/2026-05-13-ai-stock-design.md`, `docs/superpowers/plans/2026-05-13-ai-stock-m1-core.md` (final commit `3e0683a`).

---

## Conventions

- Same TDD discipline as M1.
- DDD layering enforced — `domain/` stays pure, `application/` defines ports, `infrastructure/` is the only place that touches IO.
- After each task block, append to `docs/progress.md` and bump `docs/CONTEXT.md` if ubiquitous-language changed.
- Conventional commits (`feat:`, `test:`, `fix:`, `docs:`, `chore:`, `refactor:`).

---

## Phase 7 — Technical indicators (domain)

All indicators are pure functions in `crates/domain/src/indicators/*.rs`. They take a `&[Candle]` or `&[Decimal]` (close prices) and return `Vec<Option<Decimal>>` (None for points before enough history exists).

### Task 7.1: Module skeleton + Simple Moving Average (SMA)

**Files:**
- Create: `crates/domain/src/indicators/mod.rs`
- Create: `crates/domain/src/indicators/sma.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: `crates/domain/src/indicators/sma.rs`**

```rust
use rust_decimal::Decimal;

/// Simple Moving Average over `period` samples.
/// Returns a vector of the same length as `closes`; the first `period - 1` entries are `None`.
pub fn sma(closes: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 {
        return vec![None; closes.len()];
    }
    let mut out = Vec::with_capacity(closes.len());
    let mut sum = Decimal::ZERO;
    for (i, &c) in closes.iter().enumerate() {
        sum += c;
        if i >= period {
            sum -= closes[i - period];
        }
        if i + 1 >= period {
            out.push(Some(sum / Decimal::from(period as u64)));
        } else {
            out.push(None);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn period_three_matches_hand_computed() {
        let closes = vec![dec!(1), dec!(2), dec!(3), dec!(4), dec!(5)];
        let r = sma(&closes, 3);
        assert_eq!(r, vec![None, None, Some(dec!(2)), Some(dec!(3)), Some(dec!(4))]);
    }

    #[test]
    fn zero_period_returns_all_none() {
        let r = sma(&[dec!(1), dec!(2)], 0);
        assert_eq!(r, vec![None, None]);
    }

    #[test]
    fn period_longer_than_data_returns_all_none() {
        let r = sma(&[dec!(1), dec!(2)], 5);
        assert_eq!(r, vec![None, None]);
    }
}
```

- [ ] **Step 2: `crates/domain/src/indicators/mod.rs`**

```rust
pub mod sma;
```

- [ ] **Step 3: register in `crates/domain/src/lib.rs`** — add `pub mod indicators;` in alphabetical order.

- [ ] **Step 4: verify + commit**

```bash
cargo test -p domain indicators::
cargo clippy --workspace --all-targets -- -D warnings
git add crates/domain/
git commit -m "feat(domain): add SMA indicator with windowed sum and edge handling"
```

---

### Task 7.2: Exponential Moving Average (EMA)

**Files:**
- Create: `crates/domain/src/indicators/ema.rs`
- Modify: `crates/domain/src/indicators/mod.rs`

- [ ] **Step 1: `ema.rs`**

```rust
use rust_decimal::Decimal;

/// Exponential Moving Average with smoothing factor α = 2 / (period + 1).
/// Returns a vector of the same length as `closes`; the first `period - 1` entries are `None`.
/// The first EMA value at index `period - 1` is computed as the SMA of the first `period` closes,
/// then each subsequent value is `α * close + (1 - α) * prev`.
pub fn ema(closes: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 || closes.len() < period {
        return vec![None; closes.len()];
    }
    let alpha = Decimal::from(2) / Decimal::from((period as u64) + 1);
    let one_minus_alpha = Decimal::ONE - alpha;
    let mut out: Vec<Option<Decimal>> = vec![None; period - 1];

    let seed: Decimal = closes[..period].iter().sum::<Decimal>() / Decimal::from(period as u64);
    out.push(Some(seed));
    let mut prev = seed;
    for &c in &closes[period..] {
        let next = alpha * c + one_minus_alpha * prev;
        out.push(Some(next));
        prev = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn constant_series_is_flat() {
        let closes = vec![dec!(10); 10];
        let r = ema(&closes, 3);
        assert_eq!(r[2], Some(dec!(10)));
        assert_eq!(r[9], Some(dec!(10)));
    }

    #[test]
    fn short_series_returns_none() {
        assert_eq!(ema(&[dec!(1), dec!(2)], 5), vec![None, None]);
    }

    #[test]
    fn rising_series_ema_is_below_latest_close() {
        let closes: Vec<Decimal> = (1..=10).map(|n| Decimal::from(n)).collect();
        let r = ema(&closes, 3);
        let last = r.last().unwrap().unwrap();
        assert!(last < dec!(10));
        assert!(last > dec!(5));
    }
}
```

- [ ] **Step 2: `mod.rs`** — add `pub mod ema;`.

- [ ] **Step 3: verify + commit**

```bash
cargo test -p domain indicators::ema::
git add crates/domain/
git commit -m "feat(domain): add EMA indicator using SMA seed + alpha smoothing"
```

---

### Task 7.3: Relative Strength Index (RSI)

**Files:**
- Create: `crates/domain/src/indicators/rsi.rs`
- Modify: `crates/domain/src/indicators/mod.rs`

- [ ] **Step 1: `rsi.rs`**

```rust
use rust_decimal::Decimal;

/// 14-period RSI by default (`period = 14`). Uses Wilder's smoothing.
/// Returns same-length vector; first `period` entries are `None`.
pub fn rsi(closes: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 || closes.len() <= period {
        return vec![None; closes.len()];
    }
    let mut out: Vec<Option<Decimal>> = vec![None; period];

    // Seed: average gain/loss over first `period` deltas.
    let (mut sum_gain, mut sum_loss) = (Decimal::ZERO, Decimal::ZERO);
    for w in closes.windows(2).take(period) {
        let diff = w[1] - w[0];
        if diff > Decimal::ZERO { sum_gain += diff; } else { sum_loss += -diff; }
    }
    let p = Decimal::from(period as u64);
    let mut avg_gain = sum_gain / p;
    let mut avg_loss = sum_loss / p;
    out.push(Some(rsi_from(avg_gain, avg_loss)));

    // Wilder smoothing for the rest.
    let p_minus_1 = p - Decimal::ONE;
    for w in closes.windows(2).skip(period) {
        let diff = w[1] - w[0];
        let (gain, loss) = if diff > Decimal::ZERO { (diff, Decimal::ZERO) } else { (Decimal::ZERO, -diff) };
        avg_gain = (avg_gain * p_minus_1 + gain) / p;
        avg_loss = (avg_loss * p_minus_1 + loss) / p;
        out.push(Some(rsi_from(avg_gain, avg_loss)));
    }
    out
}

fn rsi_from(avg_gain: Decimal, avg_loss: Decimal) -> Decimal {
    if avg_loss == Decimal::ZERO {
        return Decimal::from(100);
    }
    let rs = avg_gain / avg_loss;
    Decimal::from(100) - (Decimal::from(100) / (Decimal::ONE + rs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rust_decimal_macros::dec;

    #[test]
    fn monotonic_rising_pegs_at_100() {
        let closes: Vec<Decimal> = (1..=20).map(|n| Decimal::from(n)).collect();
        let r = rsi(&closes, 14);
        assert_eq!(r[14], Some(dec!(100)));
    }

    proptest! {
        #[test]
        fn rsi_in_zero_hundred(seed in -100i64..100, len in 16usize..50) {
            let closes: Vec<Decimal> = (0..len)
                .map(|i| Decimal::from(seed + i as i64))
                .collect();
            let r = rsi(&closes, 14);
            for v in r.into_iter().flatten() {
                prop_assert!(v >= Decimal::ZERO && v <= Decimal::from(100));
            }
        }
    }
}
```

- [ ] **Step 2: `mod.rs`** — add `pub mod rsi;`.

- [ ] **Step 3: verify + commit**

```bash
cargo test -p domain indicators::rsi::
git add crates/domain/
git commit -m "feat(domain): add Wilder-smoothed RSI with proptest invariant"
```

---

### Task 7.4: MACD + Bollinger Bands

**Files:**
- Create: `crates/domain/src/indicators/macd.rs`
- Create: `crates/domain/src/indicators/bollinger.rs`
- Modify: `crates/domain/src/indicators/mod.rs`

- [ ] **Step 1: `macd.rs`**

```rust
use rust_decimal::Decimal;
use super::ema::ema;

pub struct MacdOutput {
    pub macd: Vec<Option<Decimal>>,
    pub signal: Vec<Option<Decimal>>,
    pub histogram: Vec<Option<Decimal>>,
}

/// Classic MACD(12, 26, 9): fast EMA - slow EMA, plus 9-EMA of MACD as signal.
pub fn macd(closes: &[Decimal], fast: usize, slow: usize, signal_period: usize) -> MacdOutput {
    let fast_ema = ema(closes, fast);
    let slow_ema = ema(closes, slow);

    let macd: Vec<Option<Decimal>> = fast_ema
        .iter()
        .zip(slow_ema.iter())
        .map(|(f, s)| match (f, s) { (Some(f), Some(s)) => Some(*f - *s), _ => None })
        .collect();

    // EMA over the Some-prefix of macd; we build a dense slice from first Some onward.
    let first_some = macd.iter().position(|x| x.is_some()).unwrap_or(macd.len());
    let dense: Vec<Decimal> = macd[first_some..].iter().filter_map(|x| *x).collect();
    let sig_dense = ema(&dense, signal_period);
    let mut signal: Vec<Option<Decimal>> = vec![None; first_some];
    signal.extend(sig_dense);
    signal.resize(macd.len(), None);

    let histogram: Vec<Option<Decimal>> = macd
        .iter()
        .zip(signal.iter())
        .map(|(m, s)| match (m, s) { (Some(m), Some(s)) => Some(*m - *s), _ => None })
        .collect();

    MacdOutput { macd, signal, histogram }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn constant_series_is_zero() {
        let closes = vec![dec!(50); 60];
        let m = macd(&closes, 12, 26, 9);
        let last = m.macd.last().unwrap().unwrap();
        let sig = m.signal.last().unwrap().unwrap();
        assert_eq!(last, Decimal::ZERO);
        assert_eq!(sig, Decimal::ZERO);
    }
}
```

- [ ] **Step 2: `bollinger.rs`**

```rust
use rust_decimal::Decimal;
use super::sma::sma;

pub struct BollingerOutput {
    pub middle: Vec<Option<Decimal>>,
    pub upper: Vec<Option<Decimal>>,
    pub lower: Vec<Option<Decimal>>,
}

/// `period`-SMA ± `k` * stddev. Default in finance literature: period=20, k=2.
pub fn bollinger(closes: &[Decimal], period: usize, k: Decimal) -> BollingerOutput {
    let middle = sma(closes, period);
    let mut upper = vec![None; closes.len()];
    let mut lower = vec![None; closes.len()];
    if period == 0 { return BollingerOutput { middle, upper, lower }; }
    let p = Decimal::from(period as u64);

    for (i, m) in middle.iter().enumerate() {
        let Some(m) = m else { continue; };
        let start = i + 1 - period;
        let mut var = Decimal::ZERO;
        for &c in &closes[start..=i] {
            let d = c - *m;
            var += d * d;
        }
        var /= p;
        let sd = sqrt_decimal(var);
        upper[i] = Some(*m + sd * k);
        lower[i] = Some(*m - sd * k);
    }
    BollingerOutput { middle, upper, lower }
}

/// Newton-Raphson sqrt for Decimal. Sufficient precision for indicator display (not for accounting).
fn sqrt_decimal(x: Decimal) -> Decimal {
    if x <= Decimal::ZERO { return Decimal::ZERO; }
    let mut guess = x;
    for _ in 0..32 {
        guess = (guess + x / guess) / Decimal::from(2);
    }
    guess
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn constant_series_has_zero_width() {
        let closes = vec![dec!(50); 25];
        let b = bollinger(&closes, 20, dec!(2));
        let upper = b.upper.last().unwrap().unwrap();
        let lower = b.lower.last().unwrap().unwrap();
        assert_eq!(upper, dec!(50));
        assert_eq!(lower, dec!(50));
    }
}
```

- [ ] **Step 3: `mod.rs`** — add `pub mod bollinger; pub mod macd;`.

- [ ] **Step 4: verify + commit**

```bash
cargo test -p domain indicators::
git add crates/domain/
git commit -m "feat(domain): add MACD(12,26,9) and Bollinger bands (period, k)"
```

---

### Task 7.5: Application service + IPC for indicator computation

**Files:**
- Create: `crates/application/src/indicator_service.rs`
- Modify: `crates/application/src/lib.rs`
- Modify: `app/src/ipc.rs`
- Modify: `app/src/main.rs` (register new command)

- [ ] **Step 1: `crates/application/src/indicator_service.rs`**

```rust
use domain::{
    candle::Candle,
    indicators::{bollinger::{bollinger, BollingerOutput}, ema::ema, macd::{macd, MacdOutput}, rsi::rsi, sma::sma},
};
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct IndicatorSnapshot {
    pub sma_20: Option<Decimal>,
    pub sma_50: Option<Decimal>,
    pub ema_20: Option<Decimal>,
    pub rsi_14: Option<Decimal>,
    pub macd: Option<Decimal>,
    pub macd_signal: Option<Decimal>,
    pub bollinger_upper: Option<Decimal>,
    pub bollinger_lower: Option<Decimal>,
}

pub fn compute_snapshot(candles: &[Candle]) -> IndicatorSnapshot {
    let closes: Vec<Decimal> = candles.iter().map(|c| c.close.money().amount()).collect();
    let last = |v: Vec<Option<Decimal>>| v.into_iter().last().flatten();
    let MacdOutput { macd, signal, .. } = macd(&closes, 12, 26, 9);
    let BollingerOutput { upper, lower, .. } = bollinger(&closes, 20, Decimal::from(2));
    IndicatorSnapshot {
        sma_20: last(sma(&closes, 20)),
        sma_50: last(sma(&closes, 50)),
        ema_20: last(ema(&closes, 20)),
        rsi_14: last(rsi(&closes, 14)),
        macd: last(macd),
        macd_signal: last(signal),
        bollinger_upper: last(upper),
        bollinger_lower: last(lower),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol,
    };
    use rust_decimal_macros::dec;

    #[test]
    fn snapshot_on_short_series_has_mostly_none() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let p = |v| Price::new(Money::new(v, Currency::new("USD").unwrap()));
        let candles: Vec<Candle> = (1..=5)
            .map(|n| Candle {
                symbol: s.clone(),
                open: p(Decimal::from(n)), high: p(Decimal::from(n)),
                low: p(Decimal::from(n)), close: p(Decimal::from(n)),
                volume: dec!(0), opened_at: Utc::now(),
            })
            .collect();
        let snap = compute_snapshot(&candles);
        assert_eq!(snap.sma_50, None); // not enough data
    }
}
```

- [ ] **Step 2:** register `pub mod indicator_service;` in `crates/application/src/lib.rs`.

- [ ] **Step 3:** Add an IPC command in `app/src/ipc.rs`:

```rust
use application::indicator_service::{compute_snapshot, IndicatorSnapshot};

#[derive(Serialize, Clone)]
pub struct IndicatorSnapshotDto {
    pub sma_20: Option<String>,
    pub sma_50: Option<String>,
    pub ema_20: Option<String>,
    pub rsi_14: Option<String>,
    pub macd: Option<String>,
    pub macd_signal: Option<String>,
    pub bollinger_upper: Option<String>,
    pub bollinger_lower: Option<String>,
}

#[tauri::command]
pub async fn indicators_for(
    state: State<'_, AppState>,
    symbol: SymbolDto,
    days: u32,
) -> Result<IndicatorSnapshotDto, String> {
    let s = dto_to_symbol(&symbol)?;
    let from = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let to = chrono::Utc::now();
    let candles = state.market.fetch_candles(&s, from, to).await.map_err(|e| e.to_string())?;
    let snap = compute_snapshot(&candles);
    let stringify = |d: Option<rust_decimal::Decimal>| d.map(|x| x.to_string());
    Ok(IndicatorSnapshotDto {
        sma_20: stringify(snap.sma_20),
        sma_50: stringify(snap.sma_50),
        ema_20: stringify(snap.ema_20),
        rsi_14: stringify(snap.rsi_14),
        macd: stringify(snap.macd),
        macd_signal: stringify(snap.macd_signal),
        bollinger_upper: stringify(snap.bollinger_upper),
        bollinger_lower: stringify(snap.bollinger_lower),
    })
}
```

Note: this requires a new method `MarketService::fetch_candles(symbol, from, to)`. Add it to `crates/application/src/market_service.rs`:

```rust
pub async fn fetch_candles(&self, symbol: &Symbol, from: chrono::DateTime<chrono::Utc>, to: chrono::DateTime<chrono::Utc>) -> Result<Vec<domain::candle::Candle>, MarketError> {
    let provider = self.providers.iter().find(|p| p.supports(symbol))
        .ok_or_else(|| MarketError::NoProvider(symbol.to_canonical_string()))?;
    Ok(provider.fetch_candles(symbol, from, to).await?)
}
```

Register `ipc::indicators_for` in `app/src/main.rs`'s `tauri::generate_handler![...]` list.

- [ ] **Step 4: verify + commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git add crates/application/ app/
git commit -m "feat(application): add IndicatorService + indicators_for IPC command"
```

---

## Phase 8 — Alerts

### Task 8.1: `AlertRule` entity + `AlertEvaluator` domain service

**Files:**
- Create: `crates/domain/src/alert.rs`
- Modify: `crates/domain/src/lib.rs` (add `pub mod alert;`)

- [ ] **Step 1: `alert.rs`**

```rust
use crate::{money::Money, symbol::Symbol};
use chrono::{DateTime, Duration, Utc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AlertCondition {
    Above(Money),
    Below(Money),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlertRule {
    pub id: i64,
    pub symbol: Symbol,
    pub condition: AlertCondition,
    pub enabled: bool,
    pub cooldown_secs: u32,
    pub last_fired_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AlertError {
    #[error("currency mismatch: rule {0} vs price {1}")]
    CurrencyMismatch(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertOutcome {
    NotTriggered,
    OnCooldown,
    Fire,
}

pub fn evaluate(rule: &AlertRule, price: Money, now: DateTime<Utc>) -> Result<AlertOutcome, AlertError> {
    if !rule.enabled {
        return Ok(AlertOutcome::NotTriggered);
    }
    let threshold = match &rule.condition {
        AlertCondition::Above(m) => m,
        AlertCondition::Below(m) => m,
    };
    if threshold.currency() != price.currency() {
        return Err(AlertError::CurrencyMismatch(
            threshold.currency().as_str().into(),
            price.currency().as_str().into(),
        ));
    }
    let crossed = match &rule.condition {
        AlertCondition::Above(m) => price.amount() >= m.amount(),
        AlertCondition::Below(m) => price.amount() <= m.amount(),
    };
    if !crossed {
        return Ok(AlertOutcome::NotTriggered);
    }
    if let Some(last) = rule.last_fired_at {
        if now - last < Duration::seconds(rule.cooldown_secs as i64) {
            return Ok(AlertOutcome::OnCooldown);
        }
    }
    Ok(AlertOutcome::Fire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::Currency};
    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn rule(condition: AlertCondition, last: Option<DateTime<Utc>>) -> AlertRule {
        AlertRule {
            id: 1,
            symbol: Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap(),
            condition, enabled: true, cooldown_secs: 60,
            last_fired_at: last,
        }
    }

    #[test]
    fn above_threshold_fires() {
        let r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        assert_eq!(evaluate(&r, usd(dec!(71000)), Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn below_threshold_fires() {
        let r = rule(AlertCondition::Below(usd(dec!(70000))), None);
        assert_eq!(evaluate(&r, usd(dec!(69000)), Utc::now()).unwrap(), AlertOutcome::Fire);
    }

    #[test]
    fn cooldown_suppresses_consecutive_fires() {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 30).unwrap();
        let prev = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap();
        let r = rule(AlertCondition::Above(usd(dec!(70000))), Some(prev));
        assert_eq!(evaluate(&r, usd(dec!(71000)), now).unwrap(), AlertOutcome::OnCooldown);
    }

    #[test]
    fn currency_mismatch_errors() {
        let r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        let krw_price = Money::new(dec!(100_000_000), Currency::new("KRW").unwrap());
        assert!(evaluate(&r, krw_price, Utc::now()).is_err());
    }

    #[test]
    fn disabled_never_fires() {
        let mut r = rule(AlertCondition::Above(usd(dec!(70000))), None);
        r.enabled = false;
        assert_eq!(evaluate(&r, usd(dec!(71000)), Utc::now()).unwrap(), AlertOutcome::NotTriggered);
    }
}
```

- [ ] **Step 2: register, test, commit**

```bash
cargo test -p domain alert::
git add crates/domain/
git commit -m "feat(domain): add AlertRule entity and pure AlertEvaluator with cooldown"
```

---

### Task 8.2: `AlertRepo` trait + SQLite migration + `SqliteAlertRepo`

**Files:**
- Create: `crates/infrastructure/migrations/20260514000001_alerts.sql`
- Modify: `crates/application/src/ports/repos.rs` (add `AlertRepo` trait)
- Create: `crates/infrastructure/src/sqlite/alert_repo.rs`
- Modify: `crates/infrastructure/src/sqlite/mod.rs` (add `pub mod alert_repo;`)

- [ ] **Step 1: Migration**

`crates/infrastructure/migrations/20260514000001_alerts.sql`:

```sql
CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    ticker TEXT NOT NULL,
    quote_currency TEXT,
    condition_kind TEXT NOT NULL,         -- 'above' | 'below'
    threshold_amount TEXT NOT NULL,
    threshold_currency TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    cooldown_secs INTEGER NOT NULL DEFAULT 60,
    last_fired_at TEXT
);
```

- [ ] **Step 2: `AlertRepo` trait**

Append to `crates/application/src/ports/repos.rs`:

```rust
use domain::alert::AlertRule;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AlertRepo: Send + Sync {
    async fn list(&self) -> Result<Vec<AlertRule>, RepoError>;
    async fn list_for_symbol(&self, symbol: &Symbol) -> Result<Vec<AlertRule>, RepoError>;
    async fn insert(&self, rule: &AlertRule) -> Result<i64, RepoError>;
    async fn update(&self, rule: &AlertRule) -> Result<(), RepoError>;
    async fn delete(&self, id: i64) -> Result<(), RepoError>;
    async fn record_fire(&self, id: i64, at: chrono::DateTime<chrono::Utc>) -> Result<(), RepoError>;
}
```

- [ ] **Step 3: `SqliteAlertRepo`**

`crates/infrastructure/src/sqlite/alert_repo.rs`:

```rust
use application::ports::repos::{AlertRepo, RepoError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    alert::{AlertCondition, AlertRule},
    asset::AssetKind, money::{Currency, Money}, symbol::Symbol,
};
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use std::str::FromStr;

pub struct SqliteAlertRepo { pool: SqlitePool }

impl SqliteAlertRepo { pub fn new(pool: SqlitePool) -> Self { Self { pool } } }

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

fn row_to_rule(
    id: i64, kind: &str, ticker: &str, qc: Option<&str>,
    cond_kind: &str, thresh_amt: &str, thresh_ccy: &str,
    enabled: i64, cooldown: i64, last: Option<&str>,
) -> Result<AlertRule, RepoError> {
    let symbol = Symbol::new(
        str_to_kind(kind).ok_or_else(|| RepoError::Storage(format!("bad kind: {kind}")))?,
        ticker, qc,
    ).map_err(|e| RepoError::Storage(e.to_string()))?;
    let amount = Decimal::from_str(thresh_amt).map_err(|e| RepoError::Storage(e.to_string()))?;
    let ccy = Currency::new(thresh_ccy).map_err(|e| RepoError::Storage(e.to_string()))?;
    let condition = match cond_kind {
        "above" => AlertCondition::Above(Money::new(amount, ccy)),
        "below" => AlertCondition::Below(Money::new(amount, ccy)),
        _ => return Err(RepoError::Storage(format!("bad condition: {cond_kind}"))),
    };
    let last_fired_at = last.and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc));
    Ok(AlertRule { id, symbol, condition, enabled: enabled != 0, cooldown_secs: cooldown as u32, last_fired_at })
}

#[async_trait]
impl AlertRepo for SqliteAlertRepo {
    async fn list(&self) -> Result<Vec<AlertRule>, RepoError> {
        let rows: Vec<(i64, String, String, Option<String>, String, String, String, i64, i64, Option<String>)> =
            sqlx::query_as("SELECT id, kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at FROM alerts")
            .fetch_all(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        rows.into_iter().map(|(id, k, t, qc, ck, ta, tc, en, cd, lf)| {
            row_to_rule(id, &k, &t, qc.as_deref(), &ck, &ta, &tc, en, cd, lf.as_deref())
        }).collect()
    }

    async fn list_for_symbol(&self, symbol: &Symbol) -> Result<Vec<AlertRule>, RepoError> {
        let rows: Vec<(i64, String, String, Option<String>, String, String, String, i64, i64, Option<String>)> =
            sqlx::query_as("SELECT id, kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at FROM alerts WHERE kind = ? AND ticker = ? AND coalesce(quote_currency,'') = coalesce(?,'')")
            .bind(kind_to_str(symbol.kind()))
            .bind(symbol.ticker())
            .bind(symbol.quote_currency())
            .fetch_all(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        rows.into_iter().map(|(id, k, t, qc, ck, ta, tc, en, cd, lf)| {
            row_to_rule(id, &k, &t, qc.as_deref(), &ck, &ta, &tc, en, cd, lf.as_deref())
        }).collect()
    }

    async fn insert(&self, rule: &AlertRule) -> Result<i64, RepoError> {
        let (cond_kind, thresh) = match &rule.condition {
            AlertCondition::Above(m) => ("above", m),
            AlertCondition::Below(m) => ("below", m),
        };
        let r = sqlx::query(
            "INSERT INTO alerts (kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(kind_to_str(rule.symbol.kind()))
        .bind(rule.symbol.ticker())
        .bind(rule.symbol.quote_currency())
        .bind(cond_kind)
        .bind(thresh.amount().to_string())
        .bind(thresh.currency().as_str())
        .bind(if rule.enabled { 1_i64 } else { 0_i64 })
        .bind(rule.cooldown_secs as i64)
        .bind(rule.last_fired_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(r.last_insert_rowid())
    }

    async fn update(&self, rule: &AlertRule) -> Result<(), RepoError> {
        let (cond_kind, thresh) = match &rule.condition {
            AlertCondition::Above(m) => ("above", m),
            AlertCondition::Below(m) => ("below", m),
        };
        sqlx::query(
            "UPDATE alerts SET kind=?, ticker=?, quote_currency=?, condition_kind=?, threshold_amount=?, threshold_currency=?, enabled=?, cooldown_secs=?, last_fired_at=? WHERE id = ?",
        )
        .bind(kind_to_str(rule.symbol.kind()))
        .bind(rule.symbol.ticker())
        .bind(rule.symbol.quote_currency())
        .bind(cond_kind)
        .bind(thresh.amount().to_string())
        .bind(thresh.currency().as_str())
        .bind(if rule.enabled { 1_i64 } else { 0_i64 })
        .bind(rule.cooldown_secs as i64)
        .bind(rule.last_fired_at.map(|t| t.to_rfc3339()))
        .bind(rule.id)
        .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM alerts WHERE id = ?")
            .bind(id)
            .execute(&self.pool).await.map_err(|e| RepoError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn record_fire(&self, id: i64, at: DateTime<Utc>) -> Result<(), RepoError> {
        sqlx::query("UPDATE alerts SET last_fired_at = ? WHERE id = ?")
            .bind(at.to_rfc3339())
            .bind(id)
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
    async fn insert_list_and_delete() {
        let dir = tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        let repo = SqliteAlertRepo::new(pool);
        let rule = AlertRule {
            id: 0,
            symbol: Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap(),
            condition: AlertCondition::Above(Money::new(dec!(70000), Currency::new("USD").unwrap())),
            enabled: true, cooldown_secs: 60, last_fired_at: None,
        };
        let id = repo.insert(&rule).await.unwrap();
        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        repo.delete(id).await.unwrap();
        assert!(repo.list().await.unwrap().is_empty());
    }
}
```

- [ ] **Step 4: register, verify, commit**

```bash
cargo test -p infrastructure sqlite::alert_repo::
cargo clippy --workspace --all-targets -- -D warnings
git add crates/
git commit -m "feat(infra): add alerts migration, AlertRepo port, SqliteAlertRepo"
```

---

### Task 8.3: `AlertService` (application) — orchestrates evaluator + notifier + repo

**Files:**
- Create: `crates/application/src/alert_service.rs`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: `alert_service.rs`**

```rust
use crate::ports::{
    clock::Clock, notifier::{NotifyError, Notifier}, repos::{AlertRepo, RepoError},
};
use domain::{alert::{evaluate, AlertCondition, AlertError, AlertOutcome, AlertRule}, money::Money, quote::Quote};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AlertServiceError {
    #[error("repo: {0}")]
    Repo(#[from] RepoError),
    #[error("alert eval: {0}")]
    Alert(#[from] AlertError),
    #[error("notify: {0}")]
    Notify(#[from] NotifyError),
}

pub struct AlertService {
    repo: Arc<dyn AlertRepo>,
    notifier: Arc<dyn Notifier>,
    clock: Arc<dyn Clock>,
}

impl AlertService {
    pub fn new(repo: Arc<dyn AlertRepo>, notifier: Arc<dyn Notifier>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, notifier, clock }
    }

    pub async fn list(&self) -> Result<Vec<AlertRule>, AlertServiceError> {
        Ok(self.repo.list().await?)
    }

    pub async fn create(&self, rule: AlertRule) -> Result<i64, AlertServiceError> {
        Ok(self.repo.insert(&rule).await?)
    }

    pub async fn delete(&self, id: i64) -> Result<(), AlertServiceError> {
        Ok(self.repo.delete(id).await?)
    }

    /// Called by MarketService on every quote-update. Skips silently on currency mismatch
    /// (an indicator of a misconfigured rule, not a runtime crash).
    pub async fn evaluate_quote(&self, quote: &Quote) -> Result<(), AlertServiceError> {
        let rules = self.repo.list_for_symbol(&quote.symbol).await?;
        let now = self.clock.now();
        for rule in rules {
            match evaluate(&rule, quote.price.money(), now) {
                Ok(AlertOutcome::Fire) => {
                    let body = format!(
                        "{} is {} {}",
                        quote.symbol.ticker(),
                        match &rule.condition {
                            AlertCondition::Above(_) => "above",
                            AlertCondition::Below(_) => "below",
                        },
                        match &rule.condition {
                            AlertCondition::Above(m) | AlertCondition::Below(m) =>
                                format!("{} {}", m.amount(), m.currency().as_str()),
                        },
                    );
                    self.notifier.notify(&format!("Alert · {}", quote.symbol.ticker()), &body).await?;
                    self.repo.record_fire(rule.id, now).await?;
                }
                Ok(_) => {}
                Err(_) => { /* currency mismatch — surface in UI later; don't crash polling */ }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{clock::MockClock, notifier::MockNotifier, repos::MockAlertRepo};
    use chrono::TimeZone;
    use chrono::Utc;
    use domain::{
        asset::AssetKind, money::{Currency, Money}, price::Price, symbol::Symbol,
    };
    use mockall::predicate::*;
    use rust_decimal_macros::dec;

    fn s_btc() -> Symbol { Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap() }
    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn q(amount: rust_decimal::Decimal) -> Quote {
        Quote::new(s_btc(), Price::new(usd(amount)), Utc::now())
    }

    #[tokio::test]
    async fn fires_when_above_threshold_and_records() {
        let mut repo = MockAlertRepo::new();
        repo.expect_list_for_symbol().returning(|_| Ok(vec![AlertRule {
            id: 1, symbol: s_btc(),
            condition: AlertCondition::Above(usd(dec!(70000))),
            enabled: true, cooldown_secs: 0, last_fired_at: None,
        }]));
        repo.expect_record_fire().with(eq(1), always()).times(1).returning(|_, _| Ok(()));

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().times(1).returning(|_, _| Ok(()));

        let mut clock = MockClock::new();
        clock.expect_now().returning(|| Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap());

        let svc = AlertService::new(Arc::new(repo), Arc::new(notifier), Arc::new(clock));
        svc.evaluate_quote(&q(dec!(71000))).await.unwrap();
    }
}
```

- [ ] **Step 2: register, verify, commit**

```bash
cargo test -p application alert_service
git add crates/application/
git commit -m "feat(application): add AlertService with notifier + cooldown bookkeeping"
```

---

### Task 8.4: `TauriNotifier` infrastructure adapter

**Files:**
- Create: `crates/infrastructure/src/tauri_notifier.rs`
- Modify: `crates/infrastructure/src/lib.rs`

- [ ] **Step 1: `tauri_notifier.rs`**

Tauri-plugin-notification is registered in the `app` binary already. The infrastructure crate cannot import `tauri::*` (DDD layer rule enforced by `scripts/check-layer-boundary.sh`). So this adapter lives in **`app/src/`**, not in `crates/infrastructure/`.

Move-and-rename: instead, create `app/src/tauri_notifier.rs`:

```rust
use application::ports::notifier::{NotifyError, Notifier};
use async_trait::async_trait;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub struct TauriNotifier {
    app: AppHandle,
}

impl TauriNotifier {
    pub fn new(app: AppHandle) -> Self { Self { app } }
}

#[async_trait]
impl Notifier for TauriNotifier {
    async fn notify(&self, title: &str, body: &str) -> Result<(), NotifyError> {
        self.app.notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| NotifyError::Backend(e.to_string()))
    }
}
```

(Update `app/src/main.rs` to `mod tauri_notifier;`.)

- [ ] **Step 2: verify, commit**

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
git add app/
git commit -m "feat(app): TauriNotifier adapter using tauri-plugin-notification"
```

---

### Task 8.5: Wire alerts into the poll cycle + IPC commands

**Files:**
- Modify: `crates/application/src/market_service.rs` (add an optional `AlertService` collaborator)
- Modify: `app/src/wiring.rs`
- Modify: `app/src/ipc.rs` (add `alerts_list`, `alerts_create`, `alerts_delete` commands)
- Modify: `app/src/main.rs` (register the commands)

- [ ] **Step 1: Push refresh result into AlertService**

The cleanest wiring keeps `MarketService` ignorant of alerts; instead, the poller (in `app/src/main.rs`) calls both `MarketService::refresh()` and then `AlertService::evaluate_quote(&q)` per quote.

Modify the periodic loop in `app/src/main.rs`:

```rust
let snap = state.market.refresh().await.ok().unwrap_or_default();
let alert_svc = state.alerts.clone();
for q in &snap {
    let _ = alert_svc.evaluate_quote(q).await;
}
// then emit "quote-update" as before
```

- [ ] **Step 2: Update `AppState` in `app/src/wiring.rs`**

Add `pub alerts: Arc<AlertService>` to `AppState`. Wire it:

```rust
let alert_repo = Arc::new(SqliteAlertRepo::new(pool.clone()));
let notifier = Arc::new(crate::tauri_notifier::TauriNotifier::new(app_handle.clone()));
let alerts = Arc::new(AlertService::new(alert_repo, notifier, clock.clone()));
```

(`assemble` will need an `AppHandle` argument; pass it from `setup` in main.)

- [ ] **Step 3: IPC commands**

Append to `app/src/ipc.rs`:

```rust
use domain::alert::{AlertCondition, AlertRule};

#[derive(Serialize, Deserialize, Clone)]
pub struct AlertRuleDto {
    pub id: i64,
    pub symbol: SymbolDto,
    pub condition: String,           // "above" | "below"
    pub threshold_amount: String,
    pub threshold_currency: String,
    pub enabled: bool,
    pub cooldown_secs: u32,
}

fn rule_to_dto(r: &AlertRule) -> AlertRuleDto {
    let (cond, thresh) = match &r.condition {
        AlertCondition::Above(m) => ("above", m),
        AlertCondition::Below(m) => ("below", m),
    };
    AlertRuleDto {
        id: r.id, symbol: symbol_to_dto(&r.symbol),
        condition: cond.into(),
        threshold_amount: thresh.amount().to_string(),
        threshold_currency: thresh.currency().as_str().into(),
        enabled: r.enabled, cooldown_secs: r.cooldown_secs,
    }
}

fn dto_to_rule(d: &AlertRuleDto) -> Result<AlertRule, String> {
    let symbol = dto_to_symbol(&d.symbol)?;
    let amount = rust_decimal::Decimal::from_str(&d.threshold_amount).map_err(|e| e.to_string())?;
    let ccy = Currency::new(&d.threshold_currency).map_err(|e| format!("{e:?}"))?;
    let cond = match d.condition.as_str() {
        "above" => AlertCondition::Above(Money::new(amount, ccy)),
        "below" => AlertCondition::Below(Money::new(amount, ccy)),
        other => return Err(format!("unknown condition: {other}")),
    };
    Ok(AlertRule {
        id: d.id, symbol, condition: cond, enabled: d.enabled,
        cooldown_secs: d.cooldown_secs, last_fired_at: None,
    })
}

#[tauri::command]
pub async fn alerts_list(state: State<'_, AppState>) -> Result<Vec<AlertRuleDto>, String> {
    let rules = state.alerts.list().await.map_err(|e| e.to_string())?;
    Ok(rules.iter().map(rule_to_dto).collect())
}

#[tauri::command]
pub async fn alerts_create(state: State<'_, AppState>, rule: AlertRuleDto) -> Result<i64, String> {
    let r = dto_to_rule(&rule)?;
    state.alerts.create(r).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn alerts_delete(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.alerts.delete(id).await.map_err(|e| e.to_string())
}
```

Register `ipc::alerts_list, ipc::alerts_create, ipc::alerts_delete` in `tauri::generate_handler![...]`.

- [ ] **Step 4: verify, commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git add app/ crates/
git commit -m "feat(app): wire AlertService into poll loop and expose IPC commands"
```

---

### Task 8.6: Frontend — Alerts panel

**Files:**
- Create: `src/components/AlertsPanel.tsx`
- Modify: `src/lib/ipc.ts` (add alert types + bindings)
- Modify: `src/App.tsx` (open AlertsPanel from a header button)

- [ ] **Step 1: IPC bindings**

Append to `src/lib/ipc.ts`:

```typescript
export interface AlertRuleDto {
  id: number;
  symbol: SymbolDto;
  condition: "above" | "below";
  threshold_amount: string;
  threshold_currency: string;
  enabled: boolean;
  cooldown_secs: number;
}

export const alertsIpc = {
  list: () => invoke<AlertRuleDto[]>("alerts_list"),
  create: (rule: AlertRuleDto) => invoke<number>("alerts_create", { rule }),
  delete: (id: number) => invoke<void>("alerts_delete", { id }),
};
```

- [ ] **Step 2: `AlertsPanel.tsx`**

```typescript
import { useEffect, useState } from "react";
import { alertsIpc, type AlertRuleDto, type AssetKind } from "../lib/ipc";

export function AlertsPanel({ onClose }: { onClose(): void }) {
  const [rules, setRules] = useState<AlertRuleDto[]>([]);
  const [draft, setDraft] = useState({
    kind: "crypto" as AssetKind, ticker: "BTC", quote: "USD",
    condition: "above" as "above" | "below", amount: "70000", ccy: "USD",
  });

  async function load() { setRules(await alertsIpc.list()); }
  useEffect(() => { load(); }, []);

  async function create(e: React.FormEvent) {
    e.preventDefault();
    await alertsIpc.create({
      id: 0,
      symbol: { kind: draft.kind, ticker: draft.ticker.toUpperCase(),
                quote_currency: draft.kind === "crypto" ? draft.quote.toUpperCase() : null },
      condition: draft.condition,
      threshold_amount: draft.amount, threshold_currency: draft.ccy.toUpperCase(),
      enabled: true, cooldown_secs: 60,
    });
    await load();
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-[28rem] space-y-3">
        <div className="flex justify-between">
          <h3 className="text-lg font-semibold">알림</h3>
          <button onClick={onClose}>×</button>
        </div>

        <form onSubmit={create} className="grid grid-cols-2 gap-2 text-xs">
          <select value={draft.kind} onChange={(e) => setDraft({ ...draft, kind: e.target.value as AssetKind })} className="bg-slate-800 rounded p-1.5">
            <option value="crypto">Crypto</option><option value="us">US Equity</option>
          </select>
          <input value={draft.ticker} onChange={(e) => setDraft({ ...draft, ticker: e.target.value })} className="bg-slate-800 rounded p-1.5" />
          {draft.kind === "crypto" && (
            <input value={draft.quote} onChange={(e) => setDraft({ ...draft, quote: e.target.value })} placeholder="quote ccy" className="bg-slate-800 rounded p-1.5 col-span-2" />
          )}
          <select value={draft.condition} onChange={(e) => setDraft({ ...draft, condition: e.target.value as "above" | "below" })} className="bg-slate-800 rounded p-1.5">
            <option value="above">상승</option><option value="below">하락</option>
          </select>
          <input value={draft.amount} onChange={(e) => setDraft({ ...draft, amount: e.target.value })} placeholder="임계값" className="bg-slate-800 rounded p-1.5" />
          <input value={draft.ccy} onChange={(e) => setDraft({ ...draft, ccy: e.target.value })} placeholder="통화" className="bg-slate-800 rounded p-1.5" />
          <button type="submit" className="col-span-2 bg-emerald-600 rounded py-1.5">추가</button>
        </form>

        <ul className="text-xs space-y-1 max-h-60 overflow-y-auto">
          {rules.map((r) => (
            <li key={r.id} className="flex justify-between border-b border-slate-800 py-1">
              <span>{r.symbol.ticker} {r.condition === "above" ? "≥" : "≤"} {r.threshold_amount} {r.threshold_currency}</span>
              <button onClick={async () => { await alertsIpc.delete(r.id); await load(); }} className="text-rose-400">삭제</button>
            </li>
          ))}
          {rules.length === 0 && <li className="text-slate-500 text-center py-2">규칙 없음</li>}
        </ul>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Open from header**

In `src/App.tsx` header section, add a button next to the "위젯" button:

```typescript
<button onClick={() => setShowAlerts(true)} className="text-xs px-2 py-1 rounded bg-slate-800">알림</button>
```

Declare `const [showAlerts, setShowAlerts] = useState(false);` and render `{showAlerts && <AlertsPanel onClose={() => setShowAlerts(false)} />}` at the bottom.

- [ ] **Step 4: verify, commit**

```bash
npm run typecheck
npm test
git add src/
git commit -m "feat(web): alerts panel with create/list/delete"
```

---

## Phase 9 — KR stocks

### Task 9.1: `NaverKrProvider`

**Files:**
- Create: `crates/infrastructure/src/providers/naver_kr.rs`
- Modify: `crates/infrastructure/src/providers/mod.rs`
- Modify: `crates/infrastructure/Cargo.toml` (add `scraper = "0.20"`)

- [ ] **Step 1: Cargo dep**

Append to `crates/infrastructure/Cargo.toml` under `[dependencies]`:

```toml
scraper = "0.20"
```

- [ ] **Step 2: `naver_kr.rs`**

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
use scraper::{Html, Selector};
use std::str::FromStr;
use std::sync::Arc;

pub struct NaverKrProvider { http: Arc<dyn HttpClient>, base: String }

impl NaverKrProvider {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self { http, base: "https://finance.naver.com".into() }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>) -> Self {
        Self { http, base: base.into() }
    }
}

#[async_trait]
impl AssetProvider for NaverKrProvider {
    fn name(&self) -> &'static str { "naver-kr" }
    fn supports(&self, s: &Symbol) -> bool { s.kind() == AssetKind::KrEquity }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::new();
        for s in symbols {
            let code = s.ticker();
            let url = format!("{}/item/main.naver?code={}", self.base, code);
            let resp = self.http.get(&url, &[]).await.map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status >= 500 { return Err(ProviderError::Upstream(resp.status.to_string())); }
            let html = String::from_utf8_lossy(&resp.body);
            let doc = Html::parse_document(&html);
            // Naver finance puts the current price in `.no_today .blind` (sometimes multiple .blind nodes).
            let sel = Selector::parse(".no_today .blind").map_err(|e| ProviderError::Parse(e.to_string()))?;
            let text = doc.select(&sel).next()
                .ok_or_else(|| ProviderError::Parse("price not found in DOM".into()))?
                .text().collect::<String>();
            let cleaned: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
            let amount = Decimal::from_str(&cleaned).map_err(|e| ProviderError::Parse(e.to_string()))?;
            let krw = Currency::new("KRW").unwrap();
            out.push(Quote::new(s.clone(), Price::new(Money::new(amount, krw)), Utc::now()));
        }
        Ok(out)
    }

    async fn fetch_candles(&self, _s: &Symbol, _from: DateTime<Utc>, _to: DateTime<Utc>) -> Result<Vec<Candle>, ProviderError> {
        Err(ProviderError::Upstream("Naver candle endpoint not implemented in M2; pending".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    const FAKE_HTML: &str = r#"
        <html><body>
          <div class="no_today">
            <span class="blind">76800</span>
          </div>
        </body></html>
    "#;

    #[tokio::test]
    async fn parses_naver_price() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/item/main.naver"))
            .and(query_param("code", "005930"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FAKE_HTML))
            .mount(&server).await;
        let provider = NaverKrProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].price.money().amount(), Decimal::from(76800));
        assert_eq!(q[0].price.money().currency().as_str(), "KRW");
    }
}
```

- [ ] **Step 3: register in providers/mod.rs**

Append `pub mod naver_kr;` (alphabetical).

- [ ] **Step 4: Wire into `app/src/wiring.rs`**

Add `Arc::new(NaverKrProvider::new(http.clone()))` to the providers list before pushing Finnhub.

- [ ] **Step 5: verify, commit**

```bash
cargo test -p infrastructure providers::naver_kr::
cargo clippy --workspace --all-targets -- -D warnings
git add crates/ app/ Cargo.lock
git commit -m "feat(infra): add NaverKrProvider scraping finance.naver.com for KR equities"
```

---

## Phase 10 — Hygiene + close-out

### Task 10.1: Wire poll interval from settings (deferred from M1)

**Files:**
- Modify: `app/src/main.rs`

- [ ] **Step 1: Read settings on startup, restart poller when changed**

Replace the hardcoded `Duration::from_secs(5)` in `app/src/main.rs` setup hook with a settings-driven value. Pseudocode:

```rust
let initial = state.settings.get().await.map(|s| s.poll_interval_secs).unwrap_or(5);
let market = state.market.clone();
let alerts = state.alerts.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(initial as u64));
    loop {
        interval.tick().await;
        if let Ok(snap) = market.refresh().await {
            for q in &snap {
                let _ = alerts.evaluate_quote(q).await;
            }
        }
        let _ = handle.emit("quote-update", /* dto build */);
    }
});
```

(Restart-on-settings-change is M3 nicety; for M2, the interval is read once at startup. Document this in CONTEXT.md.)

- [ ] **Step 2: verify, commit**

```bash
cargo check --workspace
git add app/
git commit -m "feat(app): poll interval read from settings at startup"
```

---

### Task 10.2: Tighten CSP

**Files:**
- Modify: `app/tauri.conf.json`

- [ ] **Step 1: Update CSP**

Current CSP allows broad `connect-src 'self' https:`. Narrow `connect-src` to the specific origins we use:

```json
"security": {
  "csp": "default-src 'self' ipc: http://ipc.localhost; connect-src 'self' ipc: http://ipc.localhost https://api.binance.com https://api.coingecko.com https://query1.finance.yahoo.com https://finnhub.io https://finance.naver.com; style-src 'self' 'unsafe-inline'; img-src 'self' data:; script-src 'self'"
}
```

- [ ] **Step 2: verify by running app locally if possible (optional)**

`npm run tauri dev` — confirm no CSP violations in the dev console. If CI doesn't run the app, skip this step.

- [ ] **Step 3: commit**

```bash
git add app/tauri.conf.json
git commit -m "fix(app): tighten CSP to explicit provider origins"
```

---

### Task 10.3: M2 close-out (ADR 0003 + docs)

**Files:**
- Create: `docs/adr/0003-grep-based-layer-check-and-naver-scraping.md`
- Modify: `docs/CONTEXT.md`
- Modify: `docs/progress.md`

- [ ] **Step 1: ADR 0003**

```markdown
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
```

- [ ] **Step 2: Update `CONTEXT.md`** "Current State" section:

```markdown
- **M2 complete.** Technical indicators (MA/EMA/RSI/MACD/Bollinger), price-threshold alerts with desktop notifications, KR stocks via Naver scraping. CSP tightened to explicit provider origins. Poll interval driven by settings at startup.
- **M3 next** — BYOK AI (OpenAI/Anthropic/Gemini), news providers, commentary/analysis prompts.
- **Known M2 limitations:** Naver KR scraping is fragile (no API). KIS OpenAPI deferred. AlertService runs synchronously inside the poll loop (no separate worker).
```

- [ ] **Step 3: Append to `progress.md`** the full Phase 7-10 section with all checkboxes marked done.

- [ ] **Step 4: commit**

```bash
git add docs/
git commit -m "docs: M2 close-out — ADR 0003, CONTEXT/progress updates"
```

---

## Done

M2 ships: indicators + alerts + KR coverage + hygiene. Next: M3 plan (BYOK AI, news, commentary).
