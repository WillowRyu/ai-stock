# M4 — Multi-turn AI Assistant — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the single-shot AI commentary feature into a per-symbol multi-turn chat assistant with three preset prompt types and stream cancellation.

**Architecture:** A new pure `Conversation` aggregate in the domain layer holds the turn list. The `AiProvider` port carries a `system` string plus a `Vec<Message>` each call. `AiService` keeps per-symbol conversation state in memory and exposes `start_turn` / `send_message` / `commit_assistant`. The Tauri layer pumps the provider stream to `ai-chunk` events and supports cooperative cancellation via a `tokio::sync::watch` channel. The frontend `AiPanel` becomes a chat view backed by a new `aiStore`.

**Tech Stack:** Rust (domain/application/infrastructure crates), Tauri 2, React + TypeScript, Zustand, `tokio`, `wiremock` + `mockall` for tests, `vitest` for the frontend.

**Spec:** `docs/superpowers/specs/2026-05-17-m4-ai-assistant-design.md`

---

## Task 1: `Conversation` aggregate (domain)

**Files:**
- Create: `crates/domain/src/conversation.rs`
- Modify: `crates/domain/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/domain/src/conversation.rs` with only the test module:

```rust
//! A multi-turn conversation: an ordered list of user/assistant messages.
//! Pure — no IO, no async.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_records_messages_in_order() {
        let mut c = Conversation::new();
        assert!(c.is_empty());
        c.push_user("hello");
        c.push_assistant("hi there");
        assert_eq!(c.messages().len(), 2);
        assert_eq!(c.messages()[0].role, Role::User);
        assert_eq!(c.messages()[0].content, "hello");
        assert_eq!(c.messages()[1].role, Role::Assistant);
        assert_eq!(c.messages()[1].content, "hi there");
        assert!(!c.is_empty());
    }

    #[test]
    fn recent_returns_only_the_last_n_messages() {
        let mut c = Conversation::new();
        for i in 0..10 {
            c.push_user(format!("m{i}"));
        }
        let tail = c.recent(3);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].content, "m7");
        assert_eq!(tail[2].content, "m9");
    }

    #[test]
    fn recent_caps_at_total_length() {
        let mut c = Conversation::new();
        c.push_user("only");
        assert_eq!(c.recent(50).len(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain conversation`
Expected: FAIL — `cannot find type Conversation` / `Role`.

- [ ] **Step 3: Write minimal implementation**

Insert above the `#[cfg(test)]` line in `crates/domain/src/conversation.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Conversation {
    messages: Vec<Message>,
}

impl Conversation {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push_user(&mut self, content: impl Into<String>) {
        self.messages.push(Message::user(content));
    }
    pub fn push_assistant(&mut self, content: impl Into<String>) {
        self.messages.push(Message::assistant(content));
    }
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
    /// The most recent `n` messages — used to bound how much history is sent
    /// to the model. Returns all messages when `n` exceeds the length.
    pub fn recent(&self, n: usize) -> &[Message] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }
}
```

- [ ] **Step 4: Register the module**

In `crates/domain/src/lib.rs`, add `pub mod conversation;` in alphabetical order (between `pub mod candle;` and `pub mod fx;`):

```rust
pub mod candle;
pub mod conversation;
pub mod fx;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p domain conversation`
Expected: PASS — 3 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/domain/src/conversation.rs crates/domain/src/lib.rs
git commit -m "feat(domain): Conversation aggregate with Message/Role"
```

---

## Task 2: Prompt kinds and builders (domain)

**Files:**
- Modify: `crates/domain/src/prompt.rs`

- [ ] **Step 1: Write the failing test**

In `crates/domain/src/prompt.rs`, add these tests inside the existing `mod tests` block (after `handles_missing_data_gracefully`):

```rust
    #[test]
    fn prompt_kind_parses_known_strings() {
        assert_eq!(PromptKind::parse("commentary"), Some(PromptKind::Commentary));
        assert_eq!(PromptKind::parse("chart_analysis"), Some(PromptKind::ChartAnalysis));
        assert_eq!(PromptKind::parse("news_summary"), Some(PromptKind::NewsSummary));
        assert_eq!(PromptKind::parse("bogus"), None);
    }

    #[test]
    fn chart_analysis_prompt_emphasizes_indicators() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let ind = IndicatorContext {
            rsi_14: Some(dec!(71)), macd: Some(dec!(1.2)), macd_signal: Some(dec!(0.9)),
            sma_20: Some(dec!(180)), sma_50: Some(dec!(175)),
        };
        let ctx = PromptContext { symbol: &s, quote: None, indicators: Some(ind), headlines: &[] };
        let (sys, user) = build_prompt(PromptKind::ChartAnalysis, &ctx);
        assert!(sys.contains("technical-analysis"));
        assert!(user.contains("RSI(14): 71"));
        assert!(user.contains("interpret the RSI"));
    }

    #[test]
    fn news_summary_prompt_emphasizes_headlines() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let heads = [HeadlineRef { title: "ETF inflows surge", source: "Reuters" }];
        let ctx = PromptContext { symbol: &s, quote: None, indicators: None, headlines: &heads };
        let (sys, user) = build_prompt(PromptKind::NewsSummary, &ctx);
        assert!(sys.contains("news"));
        assert!(user.contains("ETF inflows surge"));
        assert!(user.contains("Summarize"));
    }

    #[test]
    fn build_prompt_commentary_matches_commentary_builder() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let ctx = PromptContext { symbol: &s, quote: None, indicators: None, headlines: &[] };
        let via_dispatch = build_prompt(PromptKind::Commentary, &ctx);
        let direct = build_commentary_prompt(&ctx);
        assert_eq!(via_dispatch, direct);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p domain prompt`
Expected: FAIL — `cannot find type PromptKind` / `cannot find function build_prompt`.

- [ ] **Step 3: Replace the implementation**

Replace the entire body of `crates/domain/src/prompt.rs` ABOVE the `#[cfg(test)]` line with the following (the `PromptContext` / `IndicatorContext` / `HeadlineRef` structs are unchanged — keep them; only the builder section changes):

```rust
use crate::{quote::Quote, symbol::Symbol};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct PromptContext<'a> {
    pub symbol: &'a Symbol,
    pub quote: Option<&'a Quote>,
    pub indicators: Option<IndicatorContext>,
    pub headlines: &'a [HeadlineRef<'a>],
}

#[derive(Debug, Clone, Copy)]
pub struct IndicatorContext {
    pub rsi_14: Option<Decimal>,
    pub macd: Option<Decimal>,
    pub macd_signal: Option<Decimal>,
    pub sma_20: Option<Decimal>,
    pub sma_50: Option<Decimal>,
}

#[derive(Debug, Clone, Copy)]
pub struct HeadlineRef<'a> {
    pub title: &'a str,
    pub source: &'a str,
}

/// Which preset analysis a turn is. `Default` is `Commentary` so a
/// conversation started by a free-form message has a sensible persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptKind {
    #[default]
    Commentary,
    ChartAnalysis,
    NewsSummary,
}

impl PromptKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "commentary" => Self::Commentary,
            "chart_analysis" => Self::ChartAnalysis,
            "news_summary" => Self::NewsSummary,
            _ => return None,
        })
    }
}

/// The system prompt for a conversation of the given kind. Used both for the
/// first preset turn and for free-form follow-ups in the same conversation.
pub fn system_prompt(kind: PromptKind) -> String {
    match kind {
        PromptKind::Commentary => "You are a concise financial-data assistant. Reply in 3-4 short sentences. Avoid jargon. Never give buy/sell advice. If data is missing, say so.",
        PromptKind::ChartAnalysis => "You are a technical-analysis assistant. Interpret indicators objectively in 3-5 short sentences. Explain what RSI, MACD, and the moving averages suggest about momentum and trend. Never give buy/sell advice.",
        PromptKind::NewsSummary => "You are a financial-news summarizer. Summarize headlines neutrally in 3-4 short sentences, grouping related themes. Never give buy/sell advice. If there are no headlines, say so.",
    }
    .to_string()
}

/// The shared data dump (asset, price, indicators, headlines) that opens every
/// preset's first user message.
fn data_block(ctx: &PromptContext) -> String {
    let mut user = format!("Asset: {} ({:?})\n", ctx.symbol.ticker(), ctx.symbol.kind());
    if let Some(q) = ctx.quote {
        user.push_str(&format!(
            "Current price: {} {}\n",
            q.price.money().amount(),
            q.price.money().currency().as_str()
        ));
        if let Some(c) = q.change_24h {
            user.push_str(&format!("24h change: {}%\n", c * Decimal::from(100)));
        }
    }
    if let Some(ind) = ctx.indicators {
        if let Some(rsi) = ind.rsi_14 {
            user.push_str(&format!("RSI(14): {}\n", rsi));
        }
        if let (Some(m), Some(s)) = (ind.macd, ind.macd_signal) {
            user.push_str(&format!("MACD: {} (signal {})\n", m, s));
        }
        if let Some(sma20) = ind.sma_20 {
            user.push_str(&format!("SMA(20): {}\n", sma20));
        }
        if let Some(sma50) = ind.sma_50 {
            user.push_str(&format!("SMA(50): {}\n", sma50));
        }
    }
    if !ctx.headlines.is_empty() {
        user.push_str("Recent headlines:\n");
        for h in ctx.headlines {
            user.push_str(&format!("- {} ({})\n", h.title, h.source));
        }
    }
    user
}

pub fn build_commentary_prompt(ctx: &PromptContext) -> (String, String) {
    let mut user = data_block(ctx);
    user.push_str("\nBriefly summarize what's driving the recent price action and what the indicators are saying.");
    (system_prompt(PromptKind::Commentary), user)
}

pub fn build_chart_analysis_prompt(ctx: &PromptContext) -> (String, String) {
    let mut user = data_block(ctx);
    user.push_str("\nAnalyze the technical indicators above: interpret the RSI level, the MACD versus its signal line, and the SMA(20) versus SMA(50) relationship. Say what they jointly suggest about momentum and trend.");
    (system_prompt(PromptKind::ChartAnalysis), user)
}

pub fn build_news_summary_prompt(ctx: &PromptContext) -> (String, String) {
    let mut user = data_block(ctx);
    user.push_str("\nSummarize the key themes across the recent headlines above. If no headlines are present, say that no recent news was found.");
    (system_prompt(PromptKind::NewsSummary), user)
}

/// Dispatch to the builder for `kind`.
pub fn build_prompt(kind: PromptKind, ctx: &PromptContext) -> (String, String) {
    match kind {
        PromptKind::Commentary => build_commentary_prompt(ctx),
        PromptKind::ChartAnalysis => build_chart_analysis_prompt(ctx),
        PromptKind::NewsSummary => build_news_summary_prompt(ctx),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p domain prompt`
Expected: PASS — the 2 original tests plus the 4 new ones (6 total).

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/prompt.rs
git commit -m "feat(domain): PromptKind + chart-analysis/news-summary prompt builders"
```

---

## Task 3: Migrate `AiProvider` trait to multi-turn `AiRequest`

This is one atomic cross-cutting change: the trait, all three adapters, and the
one current caller (`AiService::commentary`) change together so the workspace
stays compiling. Behavior is still single-turn after this task.

**Files:**
- Modify: `crates/application/src/ports/ai_provider.rs`
- Modify: `crates/infrastructure/src/providers/openai.rs`
- Modify: `crates/infrastructure/src/providers/anthropic.rs`
- Modify: `crates/infrastructure/src/providers/gemini.rs`
- Modify: `crates/application/src/ai_service.rs`

- [ ] **Step 1: Replace `AiPrompt` with `AiRequest` in the port**

In `crates/application/src/ports/ai_provider.rs`, replace the `AiPrompt` struct and the trait. The full new file:

```rust
use async_trait::async_trait;
use domain::conversation::Message;
use futures::stream::BoxStream;
use thiserror::Error;

/// A multi-turn request to an AI provider: a static system prompt plus the
/// ordered message history.
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum AiChunk {
    Text(String),
    Done,
}

#[derive(Debug, Error)]
pub enum AiError {
    #[error("not configured (no api key)")]
    NotConfigured,
    #[error("unauthorized — invalid api key")]
    Unauthorized,
    #[error("rate limited; retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("network error: {0}")]
    Network(String),
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn stream(
        &self,
        request: AiRequest,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError>;
}
```

- [ ] **Step 2: Update the OpenAI adapter and its test**

In `crates/infrastructure/src/providers/openai.rs`:

Change the import line 1 to:

```rust
use application::ports::ai_provider::{AiChunk, AiError, AiProvider, AiRequest};
use domain::conversation::Role;
```

Change `OpenAiMessage` so `role` is owned:

```rust
#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}
```

Replace the `stream` method body's request construction. The new `stream` method:

```rust
    async fn stream(
        &self,
        request: AiRequest,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let mut messages = vec![OpenAiMessage {
            role: "system".into(),
            content: request.system,
        }];
        for m in request.messages {
            messages.push(OpenAiMessage {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                },
                content: m.content,
            });
        }
        let body = OpenAiRequest {
            model: self.model.clone(),
            messages,
            stream: true,
            max_tokens: request.max_output_tokens,
        };
```

(The rest of `stream` — building `req`, sending, status match, SSE parsing — is unchanged.)

Replace the test `streams_text_then_done` body's prompt construction. Change:

```rust
        let provider = OpenAiProvider::with_base("test-key".into(), "gpt-4o".into(), server.uri());
        let prompt = AiPrompt {
            system: "be brief".into(),
            user: "hello".into(),
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(prompt).await.unwrap();
```

to:

```rust
        let provider = OpenAiProvider::with_base("test-key".into(), "gpt-4o".into(), server.uri());
        let request = AiRequest {
            system: "be brief".into(),
            messages: vec![
                domain::conversation::Message::user("hello"),
                domain::conversation::Message::assistant("hi"),
                domain::conversation::Message::user("more"),
            ],
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(request).await.unwrap();
```

Add a body assertion to the same test by changing the `Mock::given` chain to also match the multi-turn body. Replace:

```rust
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
```

with:

```rust
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_string_contains("\"role\":\"assistant\""))
            .and(body_string_contains("\"role\":\"system\""))
```

- [ ] **Step 3: Update the Anthropic adapter and its test**

In `crates/infrastructure/src/providers/anthropic.rs`:

Change import line 1 to:

```rust
use application::ports::ai_provider::{AiChunk, AiError, AiProvider, AiRequest};
use domain::conversation::Role;
```

Change `AnthropicMessage`:

```rust
#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}
```

Replace the `stream` signature and request construction:

```rust
    async fn stream(
        &self,
        request: AiRequest,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let messages: Vec<AnthropicMessage> = request
            .messages
            .into_iter()
            .map(|m| AnthropicMessage {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                },
                content: m.content,
            })
            .collect();
        let body = AnthropicRequest {
            model: self.model.clone(),
            system: request.system,
            messages,
            max_tokens: request.max_output_tokens,
            stream: true,
        };
```

(The rest of `stream` is unchanged.)

In the test `streams_content_block_deltas`, replace:

```rust
        let provider =
            AnthropicProvider::with_base("test".into(), "claude-3-7".into(), server.uri());
        let prompt = AiPrompt {
            system: "x".into(),
            user: "y".into(),
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(prompt).await.unwrap();
```

with:

```rust
        let provider =
            AnthropicProvider::with_base("test".into(), "claude-3-7".into(), server.uri());
        let request = AiRequest {
            system: "x".into(),
            messages: vec![
                domain::conversation::Message::user("y"),
                domain::conversation::Message::assistant("earlier"),
            ],
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(request).await.unwrap();
```

And change the `Mock::given` chain. Replace:

```rust
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
```

with:

```rust
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(body_string_contains("\"system\":"))
            .and(body_string_contains("\"role\":\"assistant\""))
```

- [ ] **Step 4: Update the Gemini adapter and its test**

In `crates/infrastructure/src/providers/gemini.rs`:

Change import line 1 to:

```rust
use application::ports::ai_provider::{AiChunk, AiError, AiProvider, AiRequest};
use domain::conversation::Role;
```

Change `GeminiContent` so `role` is owned:

```rust
#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
    role: String,
}
```

Replace the `stream` signature and request construction:

```rust
    async fn stream(
        &self,
        request: AiRequest,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let contents: Vec<GeminiContent> = request
            .messages
            .into_iter()
            .map(|m| GeminiContent {
                parts: vec![GeminiPart { text: m.content }],
                // Gemini names the assistant role "model".
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "model".into(),
                },
            })
            .collect();
        let body = GeminiRequest {
            contents,
            system_instruction: GeminiContent {
                parts: vec![GeminiPart { text: request.system }],
                role: "system".into(),
            },
        };
```

(The rest of `stream` — building the URL, sending, status match, SSE parsing — is unchanged.)

In the test `streams_gemini_parts`, replace:

```rust
        let provider =
            GeminiProvider::with_base("k".into(), "gemini-2.0-flash".into(), server.uri());
        let prompt = AiPrompt {
            system: "x".into(),
            user: "y".into(),
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(prompt).await.unwrap();
```

with:

```rust
        let provider =
            GeminiProvider::with_base("k".into(), "gemini-2.0-flash".into(), server.uri());
        let request = AiRequest {
            system: "x".into(),
            messages: vec![
                domain::conversation::Message::user("y"),
                domain::conversation::Message::assistant("earlier"),
            ],
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(request).await.unwrap();
```

And change the `Mock::given` chain. Replace:

```rust
        Mock::given(method("POST"))
            .and(path_regex(r"^/v1beta/models/.*:streamGenerateContent"))
```

with:

```rust
        Mock::given(method("POST"))
            .and(path_regex(r"^/v1beta/models/.*:streamGenerateContent"))
            .and(body_string_contains("\"role\":\"model\""))
```

- [ ] **Step 5: Update `AiService::commentary` to build `AiRequest`**

In `crates/application/src/ai_service.rs`, change the imports block (lines 3-11) to:

```rust
use crate::ports::{
    ai_provider::{AiChunk, AiError, AiProvider, AiRequest},
    news_provider::NewsProvider,
    secret_store::{SecretError, SecretStore},
};
use domain::{
    conversation::Message,
    prompt::{build_commentary_prompt, HeadlineRef, IndicatorContext, PromptContext},
    symbol::Symbol,
};
```

Replace the last statement of the `commentary` method. Change:

```rust
        let (system, user) = build_commentary_prompt(&ctx);

        Ok(provider.stream(AiPrompt { system, user, max_output_tokens: 600 }).await?)
```

to:

```rust
        let (system, user) = build_commentary_prompt(&ctx);

        Ok(provider
            .stream(AiRequest {
                system,
                messages: vec![Message::user(user)],
                max_output_tokens: 600,
            })
            .await?)
```

- [ ] **Step 6: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS — all backend tests still green (the three provider tests now exercise multi-turn bodies).

- [ ] **Step 7: Commit**

```bash
git add crates/application/src/ports/ai_provider.rs \
        crates/infrastructure/src/providers/openai.rs \
        crates/infrastructure/src/providers/anthropic.rs \
        crates/infrastructure/src/providers/gemini.rs \
        crates/application/src/ai_service.rs
git commit -m "refactor(ai): AiProvider trait carries multi-turn AiRequest"
```

---

## Task 4: `AiService` multi-turn methods

Adds per-symbol conversation state and the `start_turn` / `send_message` /
`commit_assistant` / `history` API. The existing `commentary` method stays for
now so the workspace keeps compiling; Task 5 removes it.

**Files:**
- Modify: `crates/application/src/ai_service.rs`

- [ ] **Step 1: Write the failing tests**

Append a test module at the end of `crates/application/src/ai_service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::MarketService;
    use crate::ports::ai_provider::{AiChunk, AiError, AiRequest};
    use crate::ports::repos::MockWatchlistRepo;
    use crate::ports::secret_store::MockSecretStore;
    use domain::asset::AssetKind;
    use domain::conversation::Role;
    use domain::prompt::PromptKind;
    use domain::symbol::Symbol;
    use futures::stream::StreamExt;

    struct MockAi {
        reply: String,
    }

    #[async_trait::async_trait]
    impl AiProvider for MockAi {
        fn name(&self) -> &'static str {
            "mock"
        }
        async fn stream(
            &self,
            _request: AiRequest,
        ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
            let reply = self.reply.clone();
            Ok(futures::stream::iter(vec![
                Ok(AiChunk::Text(reply)),
                Ok(AiChunk::Done),
            ])
            .boxed())
        }
    }

    fn build_service(reply: &str) -> AiService {
        let mut secrets = MockSecretStore::new();
        secrets
            .expect_get()
            .returning(|_| Ok("test-key".to_string()));
        let market = Arc::new(MarketService::new(
            Arc::new(MockWatchlistRepo::new()),
            vec![],
        ));
        let reply = reply.to_string();
        let factory: ProviderFactory = Arc::new(move |_kind: &str, _key: &str| {
            Some(Arc::new(MockAi { reply: reply.clone() }) as Arc<dyn AiProvider>)
        });
        AiService::new(Arc::new(secrets), market, vec![], factory)
    }

    async fn drain(mut stream: BoxStream<'static, Result<AiChunk, AiError>>) -> String {
        let mut acc = String::new();
        while let Some(c) = stream.next().await {
            match c.unwrap() {
                AiChunk::Text(t) => acc.push_str(&t),
                AiChunk::Done => break,
            }
        }
        acc
    }

    #[tokio::test]
    async fn send_message_records_user_and_assistant_turns() {
        let svc = build_service("first reply");
        let sym = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();

        let stream = svc.send_message("openai", &sym, "hello").await.unwrap();
        let reply = drain(stream).await;
        svc.commit_assistant(&sym, reply).await;

        let h = svc.history(&sym).await;
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].role, Role::User);
        assert_eq!(h[0].content, "hello");
        assert_eq!(h[1].role, Role::Assistant);
        assert_eq!(h[1].content, "first reply");
    }

    #[tokio::test]
    async fn second_turn_appends_to_existing_conversation() {
        let svc = build_service("reply");
        let sym = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();

        let r1 = drain(svc.send_message("openai", &sym, "q1").await.unwrap()).await;
        svc.commit_assistant(&sym, r1).await;
        let r2 = drain(svc.send_message("openai", &sym, "q2").await.unwrap()).await;
        svc.commit_assistant(&sym, r2).await;

        let h = svc.history(&sym).await;
        assert_eq!(h.len(), 4);
        assert_eq!(h[2].content, "q2");
    }

    #[tokio::test]
    async fn commit_assistant_keeps_partial_text_after_cancel() {
        let svc = build_service("ignored");
        let sym = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();

        let _ = svc.send_message("openai", &sym, "q").await.unwrap();
        // Simulate a cancelled stream: only partial text was accumulated.
        svc.commit_assistant(&sym, "partial".to_string()).await;

        let h = svc.history(&sym).await;
        assert_eq!(h.len(), 2);
        assert_eq!(h[1].content, "partial");
    }

    #[tokio::test]
    async fn start_turn_records_a_preset_user_message() {
        let svc = build_service("analysis");
        let sym = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();

        let stream = svc
            .start_turn("openai", &sym, PromptKind::ChartAnalysis)
            .await
            .unwrap();
        let reply = drain(stream).await;
        svc.commit_assistant(&sym, reply).await;

        let h = svc.history(&sym).await;
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].role, Role::User);
        assert!(h[0].content.contains("AAPL"));
        assert_eq!(h[1].content, "analysis");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p application ai_service`
Expected: FAIL — `no method named start_turn` / `send_message` / `commit_assistant` / `history`.

- [ ] **Step 3: Add the multi-turn implementation**

In `crates/application/src/ai_service.rs`, change the import block to add the new domain imports — replace the `use domain::{...}` block with:

```rust
use domain::{
    conversation::{Conversation, Message},
    prompt::{build_commentary_prompt, build_prompt, system_prompt, HeadlineRef, IndicatorContext,
        PromptContext, PromptKind},
    symbol::Symbol,
};
use std::collections::HashMap;
use tokio::sync::Mutex;
```

(Keep the existing `use futures::stream::BoxStream;` and `use std::sync::Arc;` lines.)

Add a constant and a per-symbol state struct above `pub struct AiService`:

```rust
/// How many trailing messages of a conversation are sent to the model. Bounds
/// token usage; the full conversation stays in memory.
const MAX_CONTEXT_MESSAGES: usize = 20;

/// Per-symbol chat state. `kind` is the persona of the conversation — set by
/// the most recent preset turn, reused for free-form follow-ups.
#[derive(Default)]
struct SymbolChat {
    kind: PromptKind,
    conversation: Conversation,
}
```

Add the `conversations` field to `AiService`:

```rust
pub struct AiService {
    secrets: Arc<dyn SecretStore>,
    market: Arc<MarketService>,
    news: Vec<Arc<dyn NewsProvider>>,
    provider_factory: ProviderFactory,
    conversations: Mutex<HashMap<Symbol, SymbolChat>>,
}
```

Update `AiService::new` to initialize the field:

```rust
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        market: Arc<MarketService>,
        news: Vec<Arc<dyn NewsProvider>>,
        provider_factory: ProviderFactory,
    ) -> Self {
        Self {
            secrets,
            market,
            news,
            provider_factory,
            conversations: Mutex::new(HashMap::new()),
        }
    }
```

Add these methods inside `impl AiService` (after `commentary`):

```rust
    async fn resolve_provider(
        &self,
        provider_kind: &str,
    ) -> Result<Arc<dyn AiProvider>, AiServiceError> {
        let key_name = format!("{}_api_key", provider_kind);
        let key = self.secrets.get(&key_name).await?;
        (self.provider_factory)(provider_kind, &key)
            .ok_or_else(|| AiServiceError::NotConfigured(provider_kind.into()))
    }

    /// Gather live context for `symbol` and build the `(system, user)` pair for
    /// a preset turn of the given kind.
    async fn build_preset(&self, symbol: &Symbol, kind: PromptKind) -> (String, String) {
        let snapshot = self.market.snapshot().await;
        let quote = snapshot.get(symbol).cloned();

        let mut all_headlines = Vec::new();
        for n in &self.news {
            if let Ok(h) = n.fetch(symbol, 3).await {
                all_headlines.extend(h);
            }
        }

        let indicators = {
            let from = chrono::Utc::now() - chrono::Duration::days(60);
            let to = chrono::Utc::now();
            match self
                .market
                .fetch_candles(symbol, from, to, domain::candle::CandleInterval::OneDay)
                .await
            {
                Ok(candles) if !candles.is_empty() => {
                    let snap = indicator_service::compute_snapshot(&candles);
                    Some(IndicatorContext {
                        rsi_14: snap.rsi_14,
                        macd: snap.macd,
                        macd_signal: snap.macd_signal,
                        sma_20: snap.sma_20,
                        sma_50: snap.sma_50,
                    })
                }
                _ => None,
            }
        };

        let headline_refs: Vec<HeadlineRef> = all_headlines
            .iter()
            .map(|h| HeadlineRef { title: &h.title, source: &h.source })
            .collect();
        let ctx = PromptContext {
            symbol,
            quote: quote.as_ref(),
            indicators,
            headlines: &headline_refs,
        };
        build_prompt(kind, &ctx)
    }

    /// Start (or extend) a conversation with a preset analysis turn.
    pub async fn start_turn(
        &self,
        provider_kind: &str,
        symbol: &Symbol,
        kind: PromptKind,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiServiceError> {
        let provider = self.resolve_provider(provider_kind).await?;
        let (system, user) = self.build_preset(symbol, kind).await;
        let messages = {
            let mut chats = self.conversations.lock().await;
            let chat = chats.entry(symbol.clone()).or_default();
            chat.kind = kind;
            chat.conversation.push_user(user);
            chat.conversation.recent(MAX_CONTEXT_MESSAGES).to_vec()
        };
        Ok(provider
            .stream(AiRequest { system, messages, max_output_tokens: 600 })
            .await?)
    }

    /// Send a free-form follow-up message in the symbol's conversation.
    pub async fn send_message(
        &self,
        provider_kind: &str,
        symbol: &Symbol,
        text: &str,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiServiceError> {
        let provider = self.resolve_provider(provider_kind).await?;
        let (kind, messages) = {
            let mut chats = self.conversations.lock().await;
            let chat = chats.entry(symbol.clone()).or_default();
            chat.conversation.push_user(text.to_string());
            (chat.kind, chat.conversation.recent(MAX_CONTEXT_MESSAGES).to_vec())
        };
        Ok(provider
            .stream(AiRequest {
                system: system_prompt(kind),
                messages,
                max_output_tokens: 600,
            })
            .await?)
    }

    /// Append the assistant's reply (full or partial-on-cancel) to the
    /// conversation. A no-op when the text is empty.
    pub async fn commit_assistant(&self, symbol: &Symbol, text: String) {
        if text.is_empty() {
            return;
        }
        let mut chats = self.conversations.lock().await;
        if let Some(chat) = chats.get_mut(symbol) {
            chat.conversation.push_assistant(text);
        }
    }

    /// The full message history for a symbol (used by tests).
    pub async fn history(&self, symbol: &Symbol) -> Vec<Message> {
        let chats = self.conversations.lock().await;
        chats
            .get(symbol)
            .map(|c| c.conversation.messages().to_vec())
            .unwrap_or_default()
    }
```

NOTE: `build_commentary_prompt` is still referenced by the existing `commentary`
method — keep it in the import list as shown above.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p application ai_service`
Expected: PASS — 4 tests.

- [ ] **Step 5: Run the full workspace suite**

Run: `cargo test --workspace`
Expected: PASS — everything still green.

- [ ] **Step 6: Commit**

```bash
git add crates/application/src/ai_service.rs
git commit -m "feat(application): AiService multi-turn start_turn/send_message/commit"
```

---

## Task 5: IPC commands + cancellation

Replaces `ai_commentary` with `ai_start_turn` / `ai_send_message` / `ai_cancel`,
adds a `watch`-channel cancel handle to `AppState`, and removes the now-dead
`AiService::commentary`.

**Files:**
- Modify: `app/src/wiring.rs`
- Modify: `app/src/ipc.rs`
- Modify: `app/src/main.rs`
- Modify: `crates/application/src/ai_service.rs`

- [ ] **Step 1: Add the cancel handle to `AppState`**

In `app/src/wiring.rs`, add a field to the `AppState` struct:

```rust
pub struct AppState {
    pub market: Arc<MarketService>,
    pub portfolio: Arc<PortfolioService>,
    pub settings: Arc<SettingsService>,
    pub alerts: Arc<AlertService>,
    pub secrets: Arc<KeyringSecretStore>,
    pub ai: Arc<AiService>,
    pub fx: FxRateBook,
    /// Sender side of a `watch` channel used to cancel the in-flight AI turn.
    /// Each turn replaces this with a fresh channel.
    pub ai_cancel: std::sync::Mutex<tokio::sync::watch::Sender<bool>>,
}
```

Change the final constructor line of `assemble` from:

```rust
    AppState { market, portfolio, settings, alerts, secrets, ai, fx }
```

to:

```rust
    AppState {
        market,
        portfolio,
        settings,
        alerts,
        secrets,
        ai,
        fx,
        ai_cancel: std::sync::Mutex::new(tokio::sync::watch::channel(false).0),
    }
```

- [ ] **Step 2: Replace the `ai_commentary` command in `app/src/ipc.rs`**

Delete the entire `ai_commentary` function (the `#[tauri::command] pub async fn ai_commentary(...)` block at the end of the file) and replace it with:

```rust
/// Pump a provider stream to `ai-chunk` / `ai-done` / `ai-error` events while
/// watching for cancellation. On completion (done, error, or cancel) the
/// accumulated text — full or partial — is committed to the conversation.
async fn pump_stream(
    app: &tauri::AppHandle,
    state: &AppState,
    symbol: &Symbol,
    mut stream: futures::stream::BoxStream<
        'static,
        Result<application::ports::ai_provider::AiChunk, application::ports::ai_provider::AiError>,
    >,
) {
    use application::ports::ai_provider::AiChunk;

    // Install a fresh cancel channel for this turn.
    let mut cancel_rx = {
        let (tx, rx) = tokio::sync::watch::channel(false);
        *state.ai_cancel.lock().unwrap() = tx;
        rx
    };

    let mut acc = String::new();
    loop {
        tokio::select! {
            _ = cancel_rx.changed() => {
                let _ = app.emit("ai-done", ());
                break;
            }
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(AiChunk::Text(t))) => {
                        acc.push_str(&t);
                        let _ = app.emit("ai-chunk", t);
                    }
                    Some(Ok(AiChunk::Done)) | None => {
                        let _ = app.emit("ai-done", ());
                        break;
                    }
                    Some(Err(e)) => {
                        let _ = app.emit("ai-error", e.to_string());
                        break;
                    }
                }
            }
        }
    }
    state.ai.commit_assistant(symbol, acc).await;
}

#[tauri::command]
pub async fn ai_start_turn(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: String,
    symbol: SymbolDto,
    kind: String,
) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    let prompt_kind = domain::prompt::PromptKind::parse(&kind)
        .ok_or_else(|| format!("bad prompt kind: {kind}"))?;
    let stream = state
        .ai
        .start_turn(&provider, &s, prompt_kind)
        .await
        .map_err(|e| e.to_string())?;
    pump_stream(&app, state.inner(), &s, stream).await;
    Ok(())
}

#[tauri::command]
pub async fn ai_send_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: String,
    symbol: SymbolDto,
    text: String,
) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    let stream = state
        .ai
        .send_message(&provider, &s, &text)
        .await
        .map_err(|e| e.to_string())?;
    pump_stream(&app, state.inner(), &s, stream).await;
    Ok(())
}

#[tauri::command]
pub async fn ai_cancel(state: State<'_, AppState>) -> Result<(), String> {
    // `send` fails only when no turn is in flight (receiver dropped) — ignore.
    let _ = state.ai_cancel.lock().unwrap().send(true);
    Ok(())
}
```

- [ ] **Step 3: Update the command handler list in `app/src/main.rs`**

In the `tauri::generate_handler!` macro, replace this line:

```rust
            ipc::ai_set_key, ipc::ai_clear_key, ipc::ai_has_key, ipc::ai_commentary,
```

with:

```rust
            ipc::ai_set_key, ipc::ai_clear_key, ipc::ai_has_key,
            ipc::ai_start_turn, ipc::ai_send_message, ipc::ai_cancel,
```

- [ ] **Step 4: Remove the dead `commentary` method from `AiService`**

In `crates/application/src/ai_service.rs`, delete the entire `pub async fn commentary(...)` method (it is no longer called). Then remove the now-unused `build_commentary_prompt` from the `use domain::{...}` import — change:

```rust
    prompt::{build_commentary_prompt, build_prompt, system_prompt, HeadlineRef, IndicatorContext,
        PromptContext, PromptKind},
```

to:

```rust
    prompt::{build_prompt, system_prompt, HeadlineRef, IndicatorContext, PromptContext, PromptKind},
```

- [ ] **Step 5: Build and test the workspace**

Run: `cargo test --workspace`
Expected: PASS — no references to `commentary` or `ai_commentary` remain; all tests green.

- [ ] **Step 6: Commit**

```bash
git add app/src/wiring.rs app/src/ipc.rs app/src/main.rs crates/application/src/ai_service.rs
git commit -m "feat(app): ai_start_turn/ai_send_message/ai_cancel IPC commands"
```

---

## Task 6: Frontend `aiStore`

**Files:**
- Create: `src/lib/state/aiStore.ts`
- Test: `src/lib/state/aiStore.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/lib/state/aiStore.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useAiStore } from "./aiStore";

describe("aiStore", () => {
  beforeEach(() => {
    useAiStore.setState({ bySymbol: {}, streaming: false });
  });

  it("accumulates a user turn then a streamed assistant reply", () => {
    const s = useAiStore.getState();
    s.pushUser("crypto:BTC:USD", "hello");
    s.startAssistant("crypto:BTC:USD");
    s.appendChunk("crypto:BTC:USD", "hi");
    s.appendChunk("crypto:BTC:USD", " there");
    s.finishStreaming();

    const msgs = useAiStore.getState().bySymbol["crypto:BTC:USD"];
    expect(msgs).toEqual([
      { role: "user", content: "hello" },
      { role: "assistant", content: "hi there" },
    ]);
    expect(useAiStore.getState().streaming).toBe(false);
  });

  it("keeps conversations separate per symbol key", () => {
    const s = useAiStore.getState();
    s.pushUser("crypto:BTC:USD", "btc?");
    s.pushUser("us:AAPL", "aapl?");
    expect(useAiStore.getState().bySymbol["crypto:BTC:USD"]).toHaveLength(1);
    expect(useAiStore.getState().bySymbol["us:AAPL"]).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/lib/state/aiStore.test.ts`
Expected: FAIL — cannot resolve `./aiStore`.

- [ ] **Step 3: Write the store**

Create `src/lib/state/aiStore.ts`:

```ts
import { create } from "zustand";

export type AiRole = "user" | "assistant";

export interface AiMessage {
  role: AiRole;
  content: string;
}

interface AiState {
  /** Per-symbol message lists, keyed by `quoteKey(symbol)`. */
  bySymbol: Record<string, AiMessage[]>;
  streaming: boolean;
  pushUser(symKey: string, content: string): void;
  /** Push an empty assistant message and enter the streaming state. */
  startAssistant(symKey: string): void;
  /** Append a streamed chunk to the trailing assistant message. */
  appendChunk(symKey: string, text: string): void;
  finishStreaming(): void;
}

export const useAiStore = create<AiState>((set) => ({
  bySymbol: {},
  streaming: false,

  pushUser(symKey, content) {
    set((prev) => {
      const msgs = prev.bySymbol[symKey] ?? [];
      return {
        bySymbol: { ...prev.bySymbol, [symKey]: [...msgs, { role: "user", content }] },
      };
    });
  },

  startAssistant(symKey) {
    set((prev) => {
      const msgs = prev.bySymbol[symKey] ?? [];
      return {
        streaming: true,
        bySymbol: {
          ...prev.bySymbol,
          [symKey]: [...msgs, { role: "assistant", content: "" }],
        },
      };
    });
  },

  appendChunk(symKey, text) {
    set((prev) => {
      const msgs = prev.bySymbol[symKey] ?? [];
      if (msgs.length === 0) return prev;
      const last = msgs[msgs.length - 1];
      const updated = [...msgs.slice(0, -1), { ...last, content: last.content + text }];
      return { bySymbol: { ...prev.bySymbol, [symKey]: updated } };
    });
  },

  finishStreaming() {
    set({ streaming: false });
  },
}));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/lib/state/aiStore.test.ts`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add src/lib/state/aiStore.ts src/lib/state/aiStore.test.ts
git commit -m "feat(web): aiStore — per-symbol multi-turn chat state"
```

---

## Task 7: Frontend IPC bindings

**Files:**
- Modify: `src/lib/ipc.ts`

- [ ] **Step 1: Replace the `aiIpc.commentary` binding**

In `src/lib/ipc.ts`, add the `AiPromptKind` type next to `AiProviderKind` (line 89):

```ts
export type AiProviderKind = "openai" | "anthropic" | "gemini";
export type AiPromptKind = "commentary" | "chart_analysis" | "news_summary";
```

Then replace the `aiIpc` object (the `commentary` entry) so it reads:

```ts
export const aiIpc = {
  setKey: (provider: AiProviderKind, key: string) =>
    invoke<void>("ai_set_key", { provider, key }),
  clearKey: (provider: AiProviderKind) => invoke<void>("ai_clear_key", { provider }),
  hasKey: (provider: AiProviderKind) => invoke<boolean>("ai_has_key", { provider }),
  startTurn: (provider: AiProviderKind, symbol: SymbolDto, kind: AiPromptKind) =>
    invoke<void>("ai_start_turn", { provider, symbol, kind }),
  sendMessage: (provider: AiProviderKind, symbol: SymbolDto, text: string) =>
    invoke<void>("ai_send_message", { provider, symbol, text }),
  cancel: () => invoke<void>("ai_cancel"),
};
```

(Leave `onAiChunk`, `onAiDone`, `onAiError` unchanged.)

- [ ] **Step 2: Verify the type-check passes**

Run: `npm run typecheck`
Expected: FAIL — `AiPanel.tsx` still calls `aiIpc.commentary`, which no longer exists. This is expected; Task 8 fixes `AiPanel.tsx`. Do not commit yet.

- [ ] **Step 3: Proceed to Task 8**

`ipc.ts` is committed together with `AiPanel.tsx` at the end of Task 8, since the
two must change together to keep the build green.

---

## Task 8: `AiPanel` chat UI

**Files:**
- Modify: `src/components/AiPanel.tsx`

- [ ] **Step 1: Rewrite `AiPanel.tsx` as a chat view**

Replace the entire contents of `src/components/AiPanel.tsx` with:

```tsx
import { useEffect, useRef, useState } from "react";
import {
  aiIpc, onAiChunk, onAiDone, onAiError,
  type AiProviderKind, type AiPromptKind, type SymbolDto,
} from "../lib/ipc";
import { useAiStore } from "../lib/state/aiStore";
import { quoteKey } from "../lib/state/quotesStore";

const PRESETS: { kind: AiPromptKind; label: string }[] = [
  { kind: "commentary", label: "시장 해석" },
  { kind: "chart_analysis", label: "차트·지표 분석" },
  { kind: "news_summary", label: "뉴스 요약" },
];

// The backend builds the real preset user message (a data dump). The chat view
// shows a friendly label instead — assistant replies stay identical either way.
function presetUserLabel(kind: AiPromptKind): string {
  switch (kind) {
    case "commentary": return "시장 해석 요청";
    case "chart_analysis": return "차트·지표 분석 요청";
    case "news_summary": return "뉴스 요약 요청";
  }
}

export function AiPanel({ symbol, onClose }: { symbol: SymbolDto | null; onClose(): void }) {
  const [provider, setProvider] = useState<AiProviderKind>("openai");
  const [hasKey, setHasKey] = useState(false);
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const unsubs = useRef<Array<() => void>>([]);

  const { bySymbol, streaming, pushUser, startAssistant, appendChunk, finishStreaming } =
    useAiStore();

  const symKey = symbol ? quoteKey(symbol) : null;
  const messages = symKey ? bySymbol[symKey] ?? [] : [];

  // The chunk listener is registered once; it reads the current symbol key
  // through a ref so streamed text always lands on the active conversation.
  const symKeyRef = useRef<string | null>(symKey);
  symKeyRef.current = symKey;

  useEffect(() => {
    aiIpc.hasKey(provider).then(setHasKey);
  }, [provider]);

  useEffect(() => {
    let mounted = true;
    Promise.all([
      onAiChunk((t) => {
        if (mounted && symKeyRef.current) appendChunk(symKeyRef.current, t);
      }),
      onAiDone(() => { if (mounted) finishStreaming(); }),
      onAiError((e) => { if (mounted) { finishStreaming(); setError(e); } }),
    ]).then((arr) => { unsubs.current = arr; });
    return () => { mounted = false; unsubs.current.forEach((u) => u()); };
  }, [appendChunk, finishStreaming]);

  // Auto-cancel an in-flight stream when the user switches symbols.
  useEffect(() => {
    return () => {
      if (useAiStore.getState().streaming) aiIpc.cancel();
    };
  }, [symKey]);

  function startPreset(kind: AiPromptKind) {
    if (!symbol || !symKey || streaming) return;
    setError(null);
    pushUser(symKey, presetUserLabel(kind));
    startAssistant(symKey);
    aiIpc.startTurn(provider, symbol, kind).catch((e) => {
      finishStreaming();
      setError(String(e));
    });
  }

  function send() {
    if (!symbol || !symKey || streaming) return;
    const text = input.trim();
    if (!text) return;
    setError(null);
    pushUser(symKey, text);
    startAssistant(symKey);
    setInput("");
    aiIpc.sendMessage(provider, symbol, text).catch((e) => {
      finishStreaming();
      setError(String(e));
    });
  }

  return (
    <div
      className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="glass-panel rounded-lg p-5 w-[36rem] flex flex-col gap-3 max-h-[80vh]"
      >
        <div className="flex justify-between items-center">
          <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
            AI 어시스턴트 {symbol && `· ${symbol.ticker}`}
          </h3>
          <button
            onClick={onClose}
            className="text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200"
          >
            ×
          </button>
        </div>

        <div className="flex gap-2 text-xs items-center">
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as AiProviderKind)}
            className="glass-inset rounded p-1.5 text-slate-700 dark:text-slate-200"
          >
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="gemini">Gemini</option>
          </select>
          <span
            className={
              hasKey
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-slate-500 dark:text-slate-500"
            }
          >
            {hasKey ? "키 설정됨" : "키 없음 (설정에서 입력)"}
          </span>
        </div>

        <div className="flex gap-1.5 flex-wrap">
          {PRESETS.map((p) => (
            <button
              key={p.kind}
              onClick={() => startPreset(p.kind)}
              disabled={!symbol || !hasKey || streaming}
              className="glass-inset rounded px-2.5 py-1 text-xs text-slate-700 dark:text-slate-200 disabled:opacity-40 hover:bg-white/40 dark:hover:bg-white/10"
            >
              {p.label}
            </button>
          ))}
        </div>

        <div className="glass-inset rounded p-3 flex-1 overflow-y-auto min-h-[14rem] space-y-3 text-sm">
          {messages.length === 0 && (
            <div className="text-slate-500 dark:text-slate-500">
              {symbol
                ? "위 버튼으로 분석을 시작하거나 질문을 입력하세요."
                : "워치리스트에서 종목을 선택하세요."}
            </div>
          )}
          {messages.map((m, i) => (
            <div key={i} className={m.role === "user" ? "text-right" : "text-left"}>
              <div
                className={
                  "inline-block rounded-lg px-3 py-2 whitespace-pre-wrap " +
                  (m.role === "user"
                    ? "bg-sky-500/20 text-slate-800 dark:text-slate-100"
                    : "bg-white/40 dark:bg-white/10 text-slate-700 dark:text-slate-200")
                }
              >
                {m.content ||
                  (streaming && i === messages.length - 1 ? "생성 중..." : "")}
              </div>
            </div>
          ))}
        </div>

        {error && <div className="text-rose-600 dark:text-rose-400 text-xs">{error}</div>}

        <div className="flex gap-2">
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") send(); }}
            disabled={!symbol || !hasKey}
            placeholder="후속 질문 입력..."
            className="glass-inset rounded p-2 flex-1 text-sm text-slate-700 dark:text-slate-200 disabled:opacity-40"
          />
          {streaming ? (
            <button
              onClick={() => aiIpc.cancel()}
              className="btn-primary bg-rose-500 hover:bg-rose-600"
            >
              중지
            </button>
          ) : (
            <button
              onClick={send}
              disabled={!symbol || !hasKey || !input.trim()}
              className="btn-primary disabled:opacity-50"
            >
              전송
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Type-check the frontend**

Run: `npm run typecheck`
Expected: PASS — `AiPanel.tsx` now uses the new `aiIpc` API.

- [ ] **Step 3: Build the frontend**

Run: `npm run build`
Expected: PASS — `tsc -b` and `vite build` both succeed.

- [ ] **Step 4: Run the frontend test suite**

Run: `npm test`
Expected: PASS — 3 test files (toastStore, quotesStore, aiStore).

- [ ] **Step 5: Commit**

```bash
git add src/lib/ipc.ts src/components/AiPanel.tsx
git commit -m "feat(web): AiPanel multi-turn chat UI with preset buttons + cancel"
```

---

## Task 9: ADR + docs close-out

**Files:**
- Create: `docs/adr/0007-multi-turn-ai-conversation.md`
- Modify: `docs/CONTEXT.md`
- Modify: `docs/progress.md`

- [ ] **Step 1: Write ADR 0007**

Create `docs/adr/0007-multi-turn-ai-conversation.md`:

```markdown
# ADR 0007 — Multi-turn AI conversation

- **Status:** Accepted
- **Date:** 2026-05-17 (M4)

## Context

M3 shipped a single-shot `commentary` call: one prompt in, one stream out, no
memory. ADR 0004 anticipated this would be extended ("Adding chat history
requires extending `AiPrompt` and the templates — straightforward"). M4 makes
the AI panel a per-symbol multi-turn chat.

Three API formats (OpenAI, Anthropic, Gemini) are all natively message-array
based, so the single `AiPrompt { system, user }` shape was the temporary
simplification, not the natural one.

## Decision

- A pure `Conversation` aggregate (`domain::conversation`) holds an ordered
  `Vec<Message>`; `Message` carries a `Role` (`User` / `Assistant`).
- The `AiProvider` port takes `AiRequest { system, messages, max_output_tokens }`
  instead of `AiPrompt`. `AiPrompt` is removed.
- `AiService` keeps per-symbol conversation state in memory
  (`HashMap<Symbol, SymbolChat>`), session-scoped — no persistence. It exposes
  `start_turn` (preset analysis), `send_message` (free-form follow-up), and
  `commit_assistant` (append the reply, full or partial-on-cancel).
- Context sent to the model is capped at the most recent `MAX_CONTEXT_MESSAGES`
  turns; the full conversation stays in memory.
- Cancellation is cooperative: the Tauri layer holds a `tokio::sync::watch`
  channel, `ai_cancel` flips it, and the chunk-pump loop exits — committing the
  partial assistant text so the next turn's context stays coherent.

## Consequences

- Each provider adapter maps `Role` to its wire format; Gemini needs
  `Assistant -> "model"`. A new provider is still one file.
- Conversation state is lost on app restart (session memory). Persisting it
  (SQLite) is deferred to M5+ — the design note in the M4 spec records why.
- The frontend `aiStore` mirrors turns for rendering; preset turns show a
  friendly label while the backend conversation holds the real data dump. The
  two stay structurally in sync (same turn count/order); assistant replies are
  identical.
- This ADR supersedes the single-shot assumption in ADR 0004.
```

- [ ] **Step 2: Update `docs/CONTEXT.md`**

In `docs/CONTEXT.md`, change the `Last updated` line to:

```markdown
> Last updated: 2026-05-17 (M4 complete)
```

In the Ubiquitous Language table, add these rows after the `FxRates` row:

```markdown
| Conversation | An ordered, per-symbol list of user/assistant Messages — a multi-turn AI chat. |
| Message | One turn in a Conversation: a Role (User/Assistant) plus text content. |
| PromptKind | Which preset analysis a turn is: Commentary, ChartAnalysis, or NewsSummary. |
```

Replace the first bullet of the `Current State` section (the `M1 + M2 + M3 complete...` line) with:

```markdown
- **M1 + M2 + M3 + M4 complete.** See `docs/progress.md`. 100+ backend test functions.
- **AI assistant (M4).** The AI panel is a per-symbol multi-turn chat: free-form
  follow-up messages with three preset quick-start buttons (commentary,
  chart-analysis, news-summary). History is session memory; streaming turns can
  be cancelled. See ADR 0007.
```

Add to the `Architecture Decisions` list:

```markdown
- 0007 — Multi-turn AI conversation.
```

- [ ] **Step 3: Append the M4 entry to `docs/progress.md`**

Add at the end of `docs/progress.md`:

```markdown
## 2026-05-17 (M4) — Multi-turn AI assistant

Spec: `docs/superpowers/specs/2026-05-17-m4-ai-assistant-design.md`.
Plan: `docs/superpowers/plans/2026-05-17-m4-ai-assistant.md`.

- [x] Task 1: `Conversation` aggregate + `Message`/`Role` (domain).
- [x] Task 2: `PromptKind` + chart-analysis/news-summary prompt builders.
- [x] Task 3: `AiProvider` trait migrated to multi-turn `AiRequest`.
- [x] Task 4: `AiService` multi-turn — start_turn/send_message/commit_assistant.
- [x] Task 5: `ai_start_turn`/`ai_send_message`/`ai_cancel` IPC + cancellation.
- [x] Task 6: frontend `aiStore`.
- [x] Task 7: frontend IPC bindings.
- [x] Task 8: `AiPanel` multi-turn chat UI.
- [x] Task 9: ADR 0007 + CONTEXT/progress updates.

### M4 complete

- Per-symbol multi-turn AI chat with preset quick-start buttons and stream
  cancellation. Session-memory history (no persistence).
- Next: M5 — 1.0 release prep (packaging, code signing, CI restoration, Naver
  contract test) + persistent chat history.
```

- [ ] **Step 4: Verify the full suite once more**

Run: `cargo test --workspace && npm test`
Expected: PASS — backend and frontend all green.

- [ ] **Step 5: Commit**

```bash
git add docs/adr/0007-multi-turn-ai-conversation.md docs/CONTEXT.md docs/progress.md
git commit -m "docs: ADR 0007 + CONTEXT/progress for M4 multi-turn AI"
```

---

## Self-Review

**Spec coverage:**
- Domain `Conversation`/`Message`/`Role` → Task 1. ✓
- `PromptKind` + three builders → Task 2. ✓
- `AiProvider` trait → `AiRequest` → Task 3. ✓
- `AiService` per-symbol state, `start_turn`/`send_message`/`commit_assistant`,
  history cap → Task 4. ✓
- Cancellation (Stop + auto-cancel) → Task 5 (`ai_cancel`, watch channel) +
  Task 8 (Stop button, symbol-switch auto-cancel). ✓
- Adapters multi-turn + Gemini role mapping → Task 3. ✓
- IPC commands `ai_start_turn`/`ai_send_message`/`ai_cancel` → Task 5. ✓
- `aiStore` + `AiPanel` chat UI + preset buttons → Tasks 6, 8. ✓
- Testing strategy (domain pure, application mock provider, infra wiremock,
  frontend store) → Tasks 1, 2, 4, 6 and updated provider tests in Task 3. ✓
- Docs (new ADR, CONTEXT, progress) → Task 9. ✓
- Out of scope (persistence, 1.0 prep) — correctly excluded; M5 noted.

**Placeholder scan:** No TBD/TODO. All code steps carry complete code blocks.

**Type consistency:** `AiRequest { system, messages, max_output_tokens }` is used
identically in the port (Task 3), all three adapters (Task 3), and `AiService`
(Tasks 3-4). `Message::user` / `Message::assistant` / `Role::User` /
`Role::Assistant` consistent across Tasks 1, 3, 4. `PromptKind::{Commentary,
ChartAnalysis,NewsSummary}` + `parse` consistent across Tasks 2, 4, 5. Frontend
`AiPromptKind` string values (`commentary`/`chart_analysis`/`news_summary`)
match `PromptKind::parse` (Tasks 5, 7, 8). `aiStore` method names
(`pushUser`/`startAssistant`/`appendChunk`/`finishStreaming`) consistent between
Tasks 6 and 8.
```
