# ai-stock M3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) tracking.

**Goal:** Bring optional AI commentary to the app via BYOK (Bring Your Own Key). Users paste an API key for OpenAI / Anthropic / Gemini; the app streams short market commentary for a selected symbol grounded in its current quote, indicator snapshot, and recent news headlines. AI is toggleable; with no key set or AI off, the rest of the app works exactly as before.

**Architecture:** Same DDD layering. Two new bounded contexts touch points:
- `AiProvider` trait (already declared in M1 — was vestigial). Three adapters: `OpenAiProvider`, `AnthropicProvider`, `GeminiProvider`, all SSE-streaming.
- `NewsProvider` trait (already declared in M1 — also vestigial). Two adapters: `YahooNewsRss`, `CoinDeskRss`.
- New domain code: `PromptTemplate` pure functions in `domain::prompt`.
- New application service: `AiService` orchestrating prompt build + stream invocation, surfacing chunks via a channel.
- Tauri IPC layer emits `ai-chunk` events as chunks arrive.

**Tech Stack:** Adds `eventsource-stream = "0.2"` for SSE parsing, `roxmltree = "0.20"` for RSS XML, optional `reqwest-eventsource = "0.6"` if simpler than rolling our own. Stick with `eventsource-stream` for control.

**Reference:** `docs/superpowers/specs/2026-05-13-ai-stock-design.md`, M1 + M2 plans/commits.

---

## Conventions

- Same as M1/M2.
- For streaming, every adapter exposes `fn complete_stream(&self, prompt) -> Pin<Box<dyn Stream<Item = Result<Chunk, AiError>> + Send>>`.
- Secrets live in OS keychain via the existing `KeyringSecretStore`. Keys are: `openai_api_key`, `anthropic_api_key`, `gemini_api_key`.

---

## Phase 11 — AI provider trait + adapters

### Task 11.1: Replace placeholder `AiProvider` with a streaming-aware trait

**Files:**
- Modify: `crates/application/src/ports/asset_provider.rs` (no change — leave alone)
- Replace: `crates/application/src/ports.rs` adjustments — actually edit the M1 stub `AiProvider` if it exists; otherwise create a new `ai_provider.rs` port.

- [ ] **Step 1:** Check current state of `crates/application/src/ports/`. If `ai_provider.rs` exists, replace; otherwise create.

Create `crates/application/src/ports/ai_provider.rs`:

```rust
use async_trait::async_trait;
use futures::stream::BoxStream;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AiPrompt {
    pub system: String,
    pub user: String,
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
    async fn stream(&self, prompt: AiPrompt) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError>;
}
```

Add `futures = "0.3"` to `[workspace.dependencies]` in root `Cargo.toml` and reference it in `crates/application/Cargo.toml` and `crates/infrastructure/Cargo.toml`.

Register in `ports/mod.rs`: `pub mod ai_provider;`.

- [ ] **Step 2: verify + commit**

```bash
cargo check --workspace
git add Cargo.toml crates/application/
git commit -m "feat(application): replace stub AiProvider with streaming-aware port"
```

---

### Task 11.2: `OpenAiProvider` adapter (commit 2)

**Files:**
- Create: `crates/infrastructure/src/providers/openai.rs`
- Modify: `crates/infrastructure/src/providers/mod.rs` (add `pub mod openai;` alpha order)
- Modify: `crates/infrastructure/Cargo.toml` (add `eventsource-stream = "0.2"`, `futures.workspace = true`)

- [ ] **Step 1:** Add deps to `crates/infrastructure/Cargo.toml`:

```toml
eventsource-stream = "0.2"
futures.workspace = true
```

- [ ] **Step 2: `openai.rs`**

```rust
use application::ports::ai_provider::{AiChunk, AiError, AiPrompt, AiProvider};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;
use std::sync::Arc;

pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key, base: "https://api.openai.com".into(), model,
        }
    }
    pub fn with_base(api_key: String, model: String, base: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key, base, model,
        }
    }
}

#[derive(Serialize)]
struct OpenAiMessage { role: &'static str, content: String }

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    max_tokens: u32,
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &'static str { "openai" }

    async fn stream(&self, prompt: AiPrompt) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let body = OpenAiRequest {
            model: self.model.clone(),
            messages: vec![
                OpenAiMessage { role: "system", content: prompt.system },
                OpenAiMessage { role: "user", content: prompt.user },
            ],
            stream: true,
            max_tokens: prompt.max_output_tokens,
        };
        let req = self.client
            .post(format!("{}/v1/chat/completions", self.base))
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .json(&body);
        let resp = req.send().await.map_err(|e| AiError::Network(e.to_string()))?;
        match resp.status().as_u16() {
            200 => {}
            401 | 403 => return Err(AiError::Unauthorized),
            429 => return Err(AiError::RateLimited { retry_after_secs: 60 }),
            code => return Err(AiError::Upstream(format!("status {}", code))),
        }
        let stream = resp.bytes_stream().eventsource().map(|event| match event {
            Ok(ev) => {
                if ev.data == "[DONE]" {
                    return Ok(AiChunk::Done);
                }
                let v: serde_json::Value = serde_json::from_str(&ev.data)
                    .map_err(|e| AiError::Parse(e.to_string()))?;
                let text = v.pointer("/choices/0/delta/content").and_then(|x| x.as_str()).unwrap_or("");
                Ok(AiChunk::Text(text.to_string()))
            }
            Err(e) => Err(AiError::Network(e.to_string())),
        });
        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streams_text_then_done() {
        let server = MockServer::start().await;
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\ndata: [DONE]\n\n";
        Mock::given(method("POST")).and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse).insert_header("content-type", "text/event-stream"))
            .mount(&server).await;

        let provider = OpenAiProvider::with_base("test-key".into(), "gpt-4o".into(), server.uri());
        let prompt = AiPrompt { system: "be brief".into(), user: "hello".into(), max_output_tokens: 100 };
        let mut stream = provider.stream(prompt).await.unwrap();
        let mut collected = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk.unwrap() {
                AiChunk::Text(t) => collected.push_str(&t),
                AiChunk::Done => break,
            }
        }
        assert_eq!(collected, "hi world");
    }
}
```

- [ ] **Step 3: verify + commit**

```bash
cargo test -p infrastructure providers::openai::
cargo clippy --workspace --all-targets -- -D warnings
git add crates/ Cargo.toml
git commit -m "feat(infra): add OpenAiProvider with SSE streaming"
```

---

### Task 11.3: `AnthropicProvider` adapter (commit 3)

**Files:**
- Create: `crates/infrastructure/src/providers/anthropic.rs`
- Modify: `crates/infrastructure/src/providers/mod.rs`

- [ ] **Step 1: `anthropic.rs`**

Anthropic's Messages API streams with a slightly different event shape. Key differences:
- Header: `x-api-key: <key>` and `anthropic-version: 2023-06-01`.
- Endpoint: `/v1/messages`.
- Stream events have a `type` field; we want `content_block_delta` with `delta.text`.

```rust
use application::ports::ai_provider::{AiChunk, AiError, AiPrompt, AiProvider};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    base: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key, base: "https://api.anthropic.com".into(), model,
        }
    }
    pub fn with_base(api_key: String, model: String, base: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key, base, model,
        }
    }
}

#[derive(Serialize)]
struct AnthropicMessage { role: &'static str, content: String }

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    system: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    stream: bool,
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &'static str { "anthropic" }

    async fn stream(&self, prompt: AiPrompt) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() { return Err(AiError::NotConfigured); }
        let body = AnthropicRequest {
            model: self.model.clone(),
            system: prompt.system,
            messages: vec![AnthropicMessage { role: "user", content: prompt.user }],
            max_tokens: prompt.max_output_tokens,
            stream: true,
        };
        let req = self.client
            .post(format!("{}/v1/messages", self.base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body);
        let resp = req.send().await.map_err(|e| AiError::Network(e.to_string()))?;
        match resp.status().as_u16() {
            200 => {}
            401 | 403 => return Err(AiError::Unauthorized),
            429 => return Err(AiError::RateLimited { retry_after_secs: 60 }),
            code => return Err(AiError::Upstream(format!("status {}", code))),
        }
        let stream = resp.bytes_stream().eventsource().map(|event| match event {
            Ok(ev) => {
                let v: serde_json::Value = serde_json::from_str(&ev.data)
                    .map_err(|e| AiError::Parse(e.to_string()))?;
                let ty = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
                match ty {
                    "message_stop" => Ok(AiChunk::Done),
                    "content_block_delta" => {
                        let text = v.pointer("/delta/text").and_then(|x| x.as_str()).unwrap_or("");
                        Ok(AiChunk::Text(text.to_string()))
                    }
                    _ => Ok(AiChunk::Text(String::new())),
                }
            }
            Err(e) => Err(AiError::Network(e.to_string())),
        });
        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streams_content_block_deltas() {
        let server = MockServer::start().await;
        let sse = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\" world\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        Mock::given(method("POST")).and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse).insert_header("content-type", "text/event-stream"))
            .mount(&server).await;

        let provider = AnthropicProvider::with_base("test".into(), "claude-3-7".into(), server.uri());
        let prompt = AiPrompt { system: "x".into(), user: "y".into(), max_output_tokens: 100 };
        let mut stream = provider.stream(prompt).await.unwrap();
        let mut text = String::new();
        while let Some(c) = stream.next().await {
            match c.unwrap() {
                AiChunk::Text(t) => text.push_str(&t),
                AiChunk::Done => break,
            }
        }
        assert_eq!(text, "hi world");
    }
}
```

Register `pub mod anthropic;` in `mod.rs`.

- [ ] **Step 2: verify + commit**

```bash
cargo test -p infrastructure providers::anthropic::
git commit -am "feat(infra): add AnthropicProvider with content_block_delta streaming"
```

---

### Task 11.4: `GeminiProvider` adapter (commit 4)

**Files:**
- Create: `crates/infrastructure/src/providers/gemini.rs`
- Modify: `crates/infrastructure/src/providers/mod.rs`

- [ ] **Step 1: `gemini.rs`**

Gemini uses `streamGenerateContent` with a JSON-array stream (not strict SSE). For simplicity, we accept its "newline-delimited JSON object stream" and parse each line.

```rust
use application::ports::ai_provider::{AiChunk, AiError, AiPrompt, AiProvider};
use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;

pub struct GeminiProvider {
    client: reqwest::Client,
    api_key: String,
    base: String,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build().expect("reqwest"),
            api_key, base: "https://generativelanguage.googleapis.com".into(), model,
        }
    }
    pub fn with_base(api_key: String, model: String, base: String) -> Self {
        Self {
            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build().expect("reqwest"),
            api_key, base, model,
        }
    }
}

#[derive(Serialize)]
struct GeminiPart { text: String }

#[derive(Serialize)]
struct GeminiContent { parts: Vec<GeminiPart>, role: &'static str }

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction")]
    system_instruction: GeminiContent,
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn name(&self) -> &'static str { "gemini" }

    async fn stream(&self, prompt: AiPrompt) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() { return Err(AiError::NotConfigured); }
        let body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: prompt.user }],
                role: "user",
            }],
            system_instruction: GeminiContent {
                parts: vec![GeminiPart { text: prompt.system }],
                role: "system",
            },
        };
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base, self.model, self.api_key,
        );
        let resp = self.client.post(&url).json(&body).send().await
            .map_err(|e| AiError::Network(e.to_string()))?;
        match resp.status().as_u16() {
            200 => {}
            401 | 403 => return Err(AiError::Unauthorized),
            429 => return Err(AiError::RateLimited { retry_after_secs: 30 }),
            code => return Err(AiError::Upstream(format!("status {}", code))),
        }

        use eventsource_stream::Eventsource;
        let stream = resp.bytes_stream().eventsource().map(|event| match event {
            Ok(ev) => {
                let v: serde_json::Value = serde_json::from_str(&ev.data)
                    .map_err(|e| AiError::Parse(e.to_string()))?;
                // Gemini: candidates[0].content.parts[0].text
                let text = v.pointer("/candidates/0/content/parts/0/text").and_then(|x| x.as_str()).unwrap_or("");
                // Heuristic for done: finishReason present
                let done = v.pointer("/candidates/0/finishReason").is_some();
                if done && text.is_empty() {
                    Ok(AiChunk::Done)
                } else {
                    Ok(AiChunk::Text(text.to_string()))
                }
            }
            Err(e) => Err(AiError::Network(e.to_string())),
        });
        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streams_gemini_parts() {
        let server = MockServer::start().await;
        let sse = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\ndata: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\" world\"}]}}]}\n\ndata: {\"candidates\":[{\"finishReason\":\"STOP\",\"content\":{\"parts\":[{\"text\":\"\"}]}}]}\n\n";
        Mock::given(method("POST")).and(path_regex(r"^/v1beta/models/.*:streamGenerateContent"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse).insert_header("content-type", "text/event-stream"))
            .mount(&server).await;

        let provider = GeminiProvider::with_base("k".into(), "gemini-2.0-flash".into(), server.uri());
        let prompt = AiPrompt { system: "x".into(), user: "y".into(), max_output_tokens: 100 };
        let mut stream = provider.stream(prompt).await.unwrap();
        let mut text = String::new();
        while let Some(c) = stream.next().await {
            match c.unwrap() {
                AiChunk::Text(t) => text.push_str(&t),
                AiChunk::Done => break,
            }
        }
        assert_eq!(text, "hi world");
    }
}
```

Register `pub mod gemini;` in `mod.rs`.

- [ ] **Step 2: verify + commit**

```bash
cargo test -p infrastructure providers::gemini::
git commit -am "feat(infra): add GeminiProvider streaming generative content"
```

---

## Phase 12 — News providers

### Task 12.1: `YahooNewsRss` (commit 5)

**Files:**
- Modify: `crates/infrastructure/Cargo.toml` (add `roxmltree = "0.20"`)
- Create: `crates/infrastructure/src/news/mod.rs`
- Create: `crates/infrastructure/src/news/yahoo_rss.rs`
- Modify: `crates/infrastructure/src/lib.rs` (add `pub mod news;`)

- [ ] **Step 1: deps**

Add to `crates/infrastructure/Cargo.toml`:

```toml
roxmltree = "0.20"
```

- [ ] **Step 2: `yahoo_rss.rs`**

```rust
use application::ports::http_client::HttpClient;
use application::ports::news_provider::{Headline, NewsError, NewsProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::symbol::Symbol;
use std::sync::Arc;

pub struct YahooNewsRss { http: Arc<dyn HttpClient>, base: String }

impl YahooNewsRss {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self { http, base: "https://feeds.finance.yahoo.com".into() }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>) -> Self {
        Self { http, base: base.into() }
    }
}

#[async_trait]
impl NewsProvider for YahooNewsRss {
    async fn fetch(&self, symbol: &Symbol, limit: usize) -> Result<Vec<Headline>, NewsError> {
        let url = format!("{}/rss/2.0/headline?s={}&region=US&lang=en-US", self.base, symbol.ticker());
        let resp = self.http.get(&url, &[]).await.map_err(|e| NewsError::Upstream(e.to_string()))?;
        if resp.status >= 500 { return Err(NewsError::Upstream(resp.status.to_string())); }
        let xml = std::str::from_utf8(&resp.body).map_err(|e| NewsError::Parse(e.to_string()))?;
        let doc = roxmltree::Document::parse(xml).map_err(|e| NewsError::Parse(e.to_string()))?;
        let mut out = Vec::new();
        for item in doc.descendants().filter(|n| n.has_tag_name("item")).take(limit) {
            let mut title = String::new();
            let mut link = String::new();
            let mut date = String::new();
            for child in item.children() {
                match child.tag_name().name() {
                    "title" => title = child.text().unwrap_or("").to_string(),
                    "link"  => link  = child.text().unwrap_or("").to_string(),
                    "pubDate" => date = child.text().unwrap_or("").to_string(),
                    _ => {}
                }
            }
            let published_at = DateTime::parse_from_rfc2822(&date).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
            out.push(Headline { title, url: link, source: "Yahoo Finance".into(), published_at });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use domain::asset::AssetKind;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    const FAKE_RSS: &str = r#"<?xml version="1.0"?>
        <rss><channel>
          <item>
            <title>Apple rises on guidance</title>
            <link>https://example.com/a</link>
            <pubDate>Tue, 12 May 2026 10:00:00 GMT</pubDate>
          </item>
          <item>
            <title>Apple supplier news</title>
            <link>https://example.com/b</link>
            <pubDate>Tue, 12 May 2026 09:00:00 GMT</pubDate>
          </item>
        </channel></rss>"#;

    #[tokio::test]
    async fn parses_two_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/rss/2.0/headline"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FAKE_RSS))
            .mount(&server).await;
        let p = YahooNewsRss::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let h = p.fetch(&s, 5).await.unwrap();
        assert_eq!(h.len(), 2);
        assert!(h[0].title.contains("Apple"));
    }
}
```

Create `crates/infrastructure/src/news/mod.rs`:

```rust
pub mod yahoo_rss;
```

Register in `lib.rs`: `pub mod news;`.

- [ ] **Step 3: verify + commit**

```bash
cargo test -p infrastructure news::yahoo_rss::
git commit -am "feat(infra): add YahooNewsRss provider with RSS parsing"
```

---

### Task 12.2: `CoinDeskRss` (commit 6)

**Files:**
- Create: `crates/infrastructure/src/news/coindesk_rss.rs`
- Modify: `crates/infrastructure/src/news/mod.rs`

- [ ] **Step 1: `coindesk_rss.rs`**

CoinDesk RSS is at `https://www.coindesk.com/arc/outboundfeeds/rss/`. It's not symbol-keyed; the provider just returns latest crypto headlines and we filter client-side by ticker mention in the title.

```rust
use application::ports::http_client::HttpClient;
use application::ports::news_provider::{Headline, NewsError, NewsProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{asset::AssetKind, symbol::Symbol};
use std::sync::Arc;

pub struct CoinDeskRss { http: Arc<dyn HttpClient>, base: String }

impl CoinDeskRss {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self { http, base: "https://www.coindesk.com".into() }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>) -> Self {
        Self { http, base: base.into() }
    }
}

#[async_trait]
impl NewsProvider for CoinDeskRss {
    async fn fetch(&self, symbol: &Symbol, limit: usize) -> Result<Vec<Headline>, NewsError> {
        if symbol.kind() != AssetKind::Crypto { return Ok(vec![]); }
        let url = format!("{}/arc/outboundfeeds/rss/", self.base);
        let resp = self.http.get(&url, &[]).await.map_err(|e| NewsError::Upstream(e.to_string()))?;
        if resp.status >= 500 { return Err(NewsError::Upstream(resp.status.to_string())); }
        let xml = std::str::from_utf8(&resp.body).map_err(|e| NewsError::Parse(e.to_string()))?;
        let doc = roxmltree::Document::parse(xml).map_err(|e| NewsError::Parse(e.to_string()))?;
        let ticker = symbol.ticker().to_lowercase();
        let aliases: &[&str] = match symbol.ticker() {
            "BTC" => &["btc", "bitcoin"],
            "ETH" => &["eth", "ether", "ethereum"],
            "SOL" => &["sol", "solana"],
            _ => &[],
        };
        let mut out = Vec::new();
        for item in doc.descendants().filter(|n| n.has_tag_name("item")) {
            let mut title = String::new(); let mut link = String::new(); let mut date = String::new();
            for child in item.children() {
                match child.tag_name().name() {
                    "title" => title = child.text().unwrap_or("").to_string(),
                    "link"  => link  = child.text().unwrap_or("").to_string(),
                    "pubDate" => date = child.text().unwrap_or("").to_string(),
                    _ => {}
                }
            }
            let lowercase_title = title.to_lowercase();
            let matches = lowercase_title.contains(&ticker)
                || aliases.iter().any(|a| lowercase_title.contains(a));
            if !matches { continue; }
            let published_at = DateTime::parse_from_rfc2822(&date).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
            out.push(Headline { title, url: link, source: "CoinDesk".into(), published_at });
            if out.len() >= limit { break; }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    const RSS: &str = r#"<?xml version="1.0"?><rss><channel>
        <item><title>Bitcoin breaks $70k</title><link>x</link><pubDate>Tue, 12 May 2026 10:00:00 GMT</pubDate></item>
        <item><title>Solana ecosystem news</title><link>y</link><pubDate>Tue, 12 May 2026 09:00:00 GMT</pubDate></item>
        <item><title>Random tech post</title><link>z</link><pubDate>Tue, 12 May 2026 08:00:00 GMT</pubDate></item>
    </channel></rss>"#;

    #[tokio::test]
    async fn filters_to_symbol_aliases() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/arc/outboundfeeds/rss/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(RSS))
            .mount(&server).await;
        let p = CoinDeskRss::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let h = p.fetch(&s, 5).await.unwrap();
        assert_eq!(h.len(), 1);
        assert!(h[0].title.contains("Bitcoin"));
    }
}
```

Add `pub mod coindesk_rss;` to `news/mod.rs`.

- [ ] **Step 2: verify + commit**

```bash
cargo test -p infrastructure news::coindesk_rss::
git commit -am "feat(infra): add CoinDeskRss filtering RSS by symbol aliases"
```

---

## Phase 13 — AI service + prompt templates

### Task 13.1: PromptTemplate (domain) + AiService (application) (commit 7)

**Files:**
- Create: `crates/domain/src/prompt.rs`
- Modify: `crates/domain/src/lib.rs` (add `pub mod prompt;`)
- Create: `crates/application/src/ai_service.rs`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: `crates/domain/src/prompt.rs`**

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

pub fn build_commentary_prompt(ctx: &PromptContext) -> (String, String) {
    let system = "You are a concise financial-data assistant. Reply in 3-4 short sentences. Avoid jargon. Never give buy/sell advice. If data is missing, say so.".to_string();
    let mut user = format!("Asset: {} ({:?})\n", ctx.symbol.ticker(), ctx.symbol.kind());
    if let Some(q) = ctx.quote {
        user.push_str(&format!("Current price: {} {}\n", q.price.money().amount(), q.price.money().currency().as_str()));
        if let Some(c) = q.change_24h {
            user.push_str(&format!("24h change: {}%\n", c * Decimal::from(100)));
        }
    }
    if let Some(ind) = ctx.indicators {
        if let Some(rsi) = ind.rsi_14 { user.push_str(&format!("RSI(14): {}\n", rsi)); }
        if let (Some(m), Some(s)) = (ind.macd, ind.macd_signal) {
            user.push_str(&format!("MACD: {} (signal {})\n", m, s));
        }
        if let Some(sma20) = ind.sma_20 { user.push_str(&format!("SMA(20): {}\n", sma20)); }
        if let Some(sma50) = ind.sma_50 { user.push_str(&format!("SMA(50): {}\n", sma50)); }
    }
    if !ctx.headlines.is_empty() {
        user.push_str("Recent headlines:\n");
        for h in ctx.headlines {
            user.push_str(&format!("- {} ({})\n", h.title, h.source));
        }
    }
    user.push_str("\nBriefly summarize what's driving the recent price action and what the indicators are saying.");
    (system, user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset::AssetKind, money::{Currency, Money}, price::Price,
    };
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn includes_symbol_and_price_when_quote_provided() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let q = Quote::new(s.clone(), Price::new(Money::new(dec!(67000), Currency::new("USD").unwrap())), Utc::now());
        let ctx = PromptContext { symbol: &s, quote: Some(&q), indicators: None, headlines: &[] };
        let (sys, user) = build_commentary_prompt(&ctx);
        assert!(sys.contains("financial-data assistant"));
        assert!(user.contains("BTC"));
        assert!(user.contains("67000"));
    }

    #[test]
    fn handles_missing_data_gracefully() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let ctx = PromptContext { symbol: &s, quote: None, indicators: None, headlines: &[] };
        let (_sys, user) = build_commentary_prompt(&ctx);
        assert!(user.contains("AAPL"));
    }
}
```

- [ ] **Step 2: `crates/application/src/ai_service.rs`**

```rust
use crate::indicator_service;
use crate::market_service::MarketService;
use crate::ports::{
    ai_provider::{AiChunk, AiError, AiPrompt, AiProvider},
    news_provider::NewsProvider,
    secret_store::{SecretError, SecretStore},
};
use domain::{
    prompt::{build_commentary_prompt, HeadlineRef, IndicatorContext, PromptContext},
    symbol::Symbol,
};
use futures::stream::BoxStream;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiServiceError {
    #[error("secret: {0}")]
    Secret(#[from] SecretError),
    #[error("ai: {0}")]
    Ai(#[from] AiError),
    #[error("provider not configured: {0}")]
    NotConfigured(String),
}

pub struct AiService {
    secrets: Arc<dyn SecretStore>,
    market: Arc<MarketService>,
    news: Vec<Arc<dyn NewsProvider>>,
    /// Factory that builds an AiProvider given an api key + provider name.
    /// Indirection lets us swap providers without compile-time deps.
    provider_factory: Arc<dyn Fn(&str, &str) -> Option<Arc<dyn AiProvider>> + Send + Sync>,
}

impl AiService {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        market: Arc<MarketService>,
        news: Vec<Arc<dyn NewsProvider>>,
        provider_factory: Arc<dyn Fn(&str, &str) -> Option<Arc<dyn AiProvider>> + Send + Sync>,
    ) -> Self {
        Self { secrets, market, news, provider_factory }
    }

    /// `provider_kind` is "openai" | "anthropic" | "gemini" — keyed in keychain accordingly.
    pub async fn commentary(
        &self,
        provider_kind: &str,
        symbol: &Symbol,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiServiceError> {
        let key_name = format!("{}_api_key", provider_kind);
        let key = self.secrets.get(&key_name).await?;
        let provider = (self.provider_factory)(provider_kind, &key)
            .ok_or_else(|| AiServiceError::NotConfigured(provider_kind.into()))?;

        // Build context (best-effort — missing pieces just degrade the prompt).
        let snapshot = self.market.snapshot().await;
        let quote = snapshot.get(symbol);

        let mut all_headlines = Vec::new();
        for n in &self.news {
            if let Ok(h) = n.fetch(symbol, 3).await {
                all_headlines.extend(h);
            }
        }
        let headline_refs: Vec<HeadlineRef> = all_headlines.iter()
            .map(|h| HeadlineRef { title: &h.title, source: &h.source })
            .collect();

        let indicators = {
            let from = chrono::Utc::now() - chrono::Duration::days(60);
            let to = chrono::Utc::now();
            match self.market.fetch_candles(symbol, from, to).await {
                Ok(candles) if !candles.is_empty() => {
                    let snap = indicator_service::compute_snapshot(&candles);
                    Some(IndicatorContext {
                        rsi_14: snap.rsi_14, macd: snap.macd, macd_signal: snap.macd_signal,
                        sma_20: snap.sma_20, sma_50: snap.sma_50,
                    })
                }
                _ => None,
            }
        };

        let ctx = PromptContext { symbol, quote, indicators, headlines: &headline_refs };
        let (system, user) = build_commentary_prompt(&ctx);

        Ok(provider.stream(AiPrompt { system, user, max_output_tokens: 600 }).await?)
    }
}
```

- [ ] **Step 3:** Register in `crates/application/src/lib.rs`:

```rust
pub mod ai_service;
```

- [ ] **Step 4:** Add `futures.workspace = true` to `crates/application/Cargo.toml` `[dependencies]`.

- [ ] **Step 5: verify + commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git commit -am "feat(application+domain): add PromptTemplate and AiService streaming commentary"
```

---

## Phase 14 — Wiring + IPC

### Task 14.1: Wire AiService + AI IPC commands (commit 8)

**Files:**
- Modify: `app/src/wiring.rs` (add `ai: Arc<AiService>` to `AppState`, build news providers + provider factory)
- Modify: `app/src/ipc.rs` (add commands `ai_set_key`, `ai_clear_key`, `ai_has_key`, `ai_commentary`)
- Modify: `app/src/main.rs` (register commands; spawn the `ai_commentary` stream into `ai-chunk` events)

- [ ] **Step 1: Build provider factory in `wiring.rs`**

```rust
use application::ai_service::AiService;
use application::ports::ai_provider::AiProvider;
use application::ports::news_provider::NewsProvider;
use infrastructure::{
    news::{coindesk_rss::CoinDeskRss, yahoo_rss::YahooNewsRss},
    providers::{anthropic::AnthropicProvider, gemini::GeminiProvider, openai::OpenAiProvider},
};

let news: Vec<Arc<dyn NewsProvider>> = vec![
    Arc::new(YahooNewsRss::new(http.clone())),
    Arc::new(CoinDeskRss::new(http.clone())),
];

let provider_factory: Arc<dyn Fn(&str, &str) -> Option<Arc<dyn AiProvider>> + Send + Sync> =
    Arc::new(|kind: &str, key: &str| -> Option<Arc<dyn AiProvider>> {
        match kind {
            "openai" => Some(Arc::new(OpenAiProvider::new(key.into(), "gpt-4o-mini".into()))),
            "anthropic" => Some(Arc::new(AnthropicProvider::new(key.into(), "claude-haiku-4-5-20251001".into()))),
            "gemini" => Some(Arc::new(GeminiProvider::new(key.into(), "gemini-2.0-flash".into()))),
            _ => None,
        }
    });

let secrets_dyn: Arc<dyn application::ports::secret_store::SecretStore> = secrets.clone();
let ai = Arc::new(AiService::new(secrets_dyn, market.clone(), news, provider_factory));
```

Add `pub ai: Arc<AiService>` to `AppState` and return it from `assemble`.

- [ ] **Step 2: IPC commands**

Append to `app/src/ipc.rs`:

```rust
use futures::StreamExt;

#[tauri::command]
pub async fn ai_set_key(state: State<'_, AppState>, provider: String, key: String) -> Result<(), String> {
    let name = format!("{}_api_key", provider);
    state.secrets.set(&name, &key).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_clear_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    let name = format!("{}_api_key", provider);
    state.secrets.delete(&name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_has_key(state: State<'_, AppState>, provider: String) -> Result<bool, String> {
    let name = format!("{}_api_key", provider);
    match state.secrets.get(&name).await {
        Ok(_) => Ok(true),
        Err(application::ports::secret_store::SecretError::NotFound(_)) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn ai_commentary(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: String,
    symbol: SymbolDto,
) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    let mut stream = state.ai.commentary(&provider, &s).await.map_err(|e| e.to_string())?;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(application::ports::ai_provider::AiChunk::Text(t)) => {
                let _ = app.emit("ai-chunk", t);
            }
            Ok(application::ports::ai_provider::AiChunk::Done) => {
                let _ = app.emit("ai-done", ());
                break;
            }
            Err(e) => {
                let _ = app.emit("ai-error", e.to_string());
                break;
            }
        }
    }
    Ok(())
}
```

Register all four in `tauri::generate_handler![...]`. Also need `use tauri::Emitter` at the top of `ipc.rs`.

- [ ] **Step 3: Add `secrets` as `Arc<KeyringSecretStore>` exposed via trait**

The existing `AppState::secrets` is `Arc<KeyringSecretStore>` (concrete type). The `ai_has_key` command above calls `state.secrets.get(...)` which works because `KeyringSecretStore` implements `SecretStore`. Confirm imports.

If `state.secrets.get(...)` doesn't compile because the trait isn't in scope, add at the top of `ipc.rs`:

```rust
use application::ports::secret_store::SecretStore;
```

- [ ] **Step 4: verify + commit**

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
git commit -am "feat(app): wire AiService + ai_set_key/has_key/commentary IPC commands"
```

---

## Phase 15 — Frontend AI chat panel

### Task 15.1: AI chat panel + settings BYOK entry (commit 9)

**Files:**
- Modify: `src/lib/ipc.ts` (add ai bindings)
- Create: `src/components/AiPanel.tsx`
- Modify: `src/components/Settings.tsx` (add API key entry per provider)
- Modify: `src/App.tsx` (header button for AI panel)

- [ ] **Step 1: IPC bindings**

Append to `src/lib/ipc.ts`:

```typescript
export type AiProviderKind = "openai" | "anthropic" | "gemini";

export const aiIpc = {
  setKey: (provider: AiProviderKind, key: string) => invoke<void>("ai_set_key", { provider, key }),
  clearKey: (provider: AiProviderKind) => invoke<void>("ai_clear_key", { provider }),
  hasKey: (provider: AiProviderKind) => invoke<boolean>("ai_has_key", { provider }),
  commentary: (provider: AiProviderKind, symbol: SymbolDto) =>
    invoke<void>("ai_commentary", { provider, symbol }),
};

export function onAiChunk(cb: (text: string) => void): Promise<UnlistenFn> {
  return listen<string>("ai-chunk", (e) => cb(e.payload));
}
export function onAiDone(cb: () => void): Promise<UnlistenFn> {
  return listen<null>("ai-done", () => cb());
}
export function onAiError(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("ai-error", (e) => cb(e.payload));
}
```

- [ ] **Step 2: `AiPanel.tsx`**

```typescript
import { useEffect, useRef, useState } from "react";
import {
  aiIpc, onAiChunk, onAiDone, onAiError,
  type AiProviderKind, type SymbolDto,
} from "../lib/ipc";

export function AiPanel({ symbol, onClose }: { symbol: SymbolDto | null; onClose(): void }) {
  const [provider, setProvider] = useState<AiProviderKind>("openai");
  const [hasKey, setHasKey] = useState(false);
  const [text, setText] = useState("");
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const unsubs = useRef<Array<() => void>>([]);

  useEffect(() => {
    aiIpc.hasKey(provider).then(setHasKey);
  }, [provider]);

  useEffect(() => {
    let mounted = true;
    Promise.all([
      onAiChunk((t) => mounted && setText((prev) => prev + t)),
      onAiDone(() => mounted && setRunning(false)),
      onAiError((e) => mounted && (setRunning(false), setError(e))),
    ]).then((arr) => { unsubs.current = arr; });
    return () => { mounted = false; unsubs.current.forEach((u) => u()); };
  }, []);

  async function run() {
    if (!symbol) return;
    setError(null); setText(""); setRunning(true);
    try { await aiIpc.commentary(provider, symbol); }
    catch (e) { setError(String(e)); setRunning(false); }
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-[36rem] space-y-3">
        <div className="flex justify-between items-center">
          <h3 className="text-lg font-semibold">AI 해석 {symbol && `· ${symbol.ticker}`}</h3>
          <button onClick={onClose}>×</button>
        </div>
        <div className="flex gap-2 text-xs items-center">
          <select value={provider} onChange={(e) => setProvider(e.target.value as AiProviderKind)} className="bg-slate-800 rounded p-1.5">
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="gemini">Gemini</option>
          </select>
          <span className={hasKey ? "text-emerald-400" : "text-slate-500"}>
            {hasKey ? "키 설정됨" : "키 없음 (설정에서 입력)"}
          </span>
          <button onClick={run} disabled={!symbol || !hasKey || running}
            className="ml-auto bg-emerald-600 disabled:bg-slate-700 rounded px-3 py-1.5">
            {running ? "생성 중..." : "해석 요청"}
          </button>
        </div>
        <div className="bg-slate-950 border border-slate-800 rounded p-3 min-h-[12rem] whitespace-pre-wrap text-sm">
          {text || (symbol ? "버튼을 눌러 시작" : "워치리스트에서 종목 선택")}
        </div>
        {error && <div className="text-rose-400 text-xs">{error}</div>}
      </div>
    </div>
  );
}
```

- [ ] **Step 3:** Extend `Settings.tsx` with BYOK entry section

Add a new section to `Settings.tsx`:

```typescript
import { aiIpc, type AiProviderKind } from "../lib/ipc";

// inside Settings(): add state for the keys + a save handler:
const [keyDraft, setKeyDraft] = useState<{ provider: AiProviderKind; key: string }>({ provider: "openai", key: "" });

async function saveKey() {
  await aiIpc.setKey(keyDraft.provider, keyDraft.key);
  setKeyDraft({ ...keyDraft, key: "" });
}
async function clearKey() {
  await aiIpc.clearKey(keyDraft.provider);
}
```

And in the JSX, add below the existing settings inputs:

```typescript
<div className="border-t border-slate-800 pt-3">
  <div className="text-xs uppercase text-slate-400 mb-2">AI API 키 (BYOK)</div>
  <div className="flex gap-2">
    <select value={keyDraft.provider} onChange={(e) => setKeyDraft({ ...keyDraft, provider: e.target.value as AiProviderKind })} className="bg-slate-800 rounded p-1.5 text-xs">
      <option value="openai">OpenAI</option>
      <option value="anthropic">Anthropic</option>
      <option value="gemini">Gemini</option>
    </select>
    <input type="password" value={keyDraft.key} onChange={(e) => setKeyDraft({ ...keyDraft, key: e.target.value })}
           placeholder="sk-..." className="flex-1 bg-slate-800 rounded p-1.5 text-xs" />
    <button type="button" onClick={saveKey} className="bg-emerald-600 rounded px-3 text-xs">저장</button>
    <button type="button" onClick={clearKey} className="bg-rose-900 rounded px-3 text-xs">삭제</button>
  </div>
  <p className="text-[10px] text-slate-500 mt-1">키는 OS 키체인에 암호화 저장됨</p>
</div>
```

- [ ] **Step 4:** Add a header button + state in `App.tsx`:

```typescript
import { AiPanel } from "./components/AiPanel";

const [showAi, setShowAi] = useState(false);

// in the header buttons group:
<button onClick={() => setShowAi(true)} className="text-xs px-2 py-1 rounded bg-slate-800">AI</button>

// at the bottom:
{showAi && <AiPanel symbol={selected} onClose={() => setShowAi(false)} />}
```

- [ ] **Step 5: verify + commit**

```bash
npm run typecheck
npm test
git add src/
git commit -m "feat(web): AI panel + BYOK key entry in settings + ai-chunk streaming UI"
```

---

## Phase 16 — Close-out

### Task 16.1: ADR 0004 + close-out (commit 10)

**Files:**
- Create: `docs/adr/0004-byok-ai-streaming-architecture.md`
- Modify: `docs/CONTEXT.md`
- Modify: `docs/progress.md`

- [ ] **Step 1: ADR 0004**

```markdown
# ADR 0004 — BYOK AI with streaming over Tauri events

- **Status:** Accepted
- **Date:** 2026-05-13 (M3)

## Context

The app needs optional AI commentary. We chose BYOK (user supplies an API key) for three reasons:

1. **Zero operating cost.** No hosted backend, no billing, no rate limiting on our side.
2. **Privacy.** API calls go directly from the user's machine to the provider; nothing routes through us.
3. **Provider choice.** Users pick the model they prefer (OpenAI / Anthropic / Gemini).

Three streaming formats had to be normalized: OpenAI SSE (`data: {...}`), Anthropic SSE with event types (`event: content_block_delta`), and Gemini's `streamGenerateContent?alt=sse` (similar to OpenAI). We unify behind an `AiProvider::stream(prompt) -> BoxStream<AiChunk>` trait.

Streaming flows through Tauri events (`ai-chunk` / `ai-done` / `ai-error`) rather than command return values so the UI can render tokens as they arrive without blocking the IPC roundtrip.

## Decision

- One `AiProvider` trait in `application::ports::ai_provider`; adapters in `infrastructure::providers::{openai, anthropic, gemini}`.
- One `PromptTemplate` pure function in `domain::prompt`. Context is composed in `AiService`.
- `AiService` is the only code that touches `SecretStore`. Keys are scoped per-provider.
- Frontend subscribes to `ai-chunk` and renders incrementally.
- AI is independent: with no key set, the app works exactly as M2.

## Consequences

- Each new provider is a single file + a `match` arm in the wiring factory.
- Adding chat history requires extending `AiPrompt` and the templates — straightforward.
- Switching to a hosted model later means a `HostedAiProvider` adapter, no changes elsewhere.
- BYOK puts the cost decision on the user. We never see their keys outside the keychain.
```

- [ ] **Step 2: Update `CONTEXT.md` "Current State"**

```markdown
- **M3 complete.** BYOK AI with OpenAI / Anthropic / Gemini streaming, news context from Yahoo + CoinDesk, prompt template in domain. AI is toggleable; without a key set, the app works exactly as M2.
- **Known M3 limitations:** Only commentary prompt type — chart-analysis and news-summary variants are stubs. No chat history (each request is independent). RSS news is best-effort and unsymmetric (Yahoo per-symbol, CoinDesk filtered). Stream cancellation isn't wired — closing the AI panel during generation lets it finish to completion.
- **Next:** post-M3 polish (chart panel rendering historical candles with overlays, chat history, news summary prompts), then a 1.0 release.
```

Bump "Last updated" to `2026-05-13 (M3 complete)`.

- [ ] **Step 3: Append to `progress.md`**

```markdown
## 2026-05-13 (M3)

### Phase 11 — AI provider trait + adapters

- [x] Task 11.1: AiProvider port (streaming) + ports/mod.rs.
- [x] Task 11.2: OpenAiProvider.
- [x] Task 11.3: AnthropicProvider.
- [x] Task 11.4: GeminiProvider.

### Phase 12 — News providers

- [x] Task 12.1: YahooNewsRss.
- [x] Task 12.2: CoinDeskRss with symbol-alias filtering.

### Phase 13 — AI service + prompt templates

- [x] Task 13.1: PromptTemplate (domain) + AiService (application).

### Phase 14 — Wiring + IPC

- [x] Task 14.1: AiService wired in AppState; ai_* commands + ai-chunk/done/error events.

### Phase 15 — Frontend

- [x] Task 15.1: AiPanel + BYOK in Settings.

### Phase 16 — Close-out

- [x] Task 16.1: ADR 0004 + CONTEXT update + this entry.

## 2026-05-13 — M3 complete

- BYOK AI commentary streaming to the UI for any watchlist symbol.
- ~70 backend unit tests (including 3 AI streaming wiremock tests and 2 RSS parsing tests), 1 frontend test.
- Tauri app: M1 (core) + M2 (indicators/alerts/KR) + M3 (AI) all working together.
- Next: post-M3 polish.
```

- [ ] **Step 4: verify + commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/check-layer-boundary.sh
npm test
git add docs/
git commit -m "docs: M3 close-out — ADR 0004, CONTEXT/progress updates"
```

---

## Done

M3 ships: BYOK AI commentary + news context. Total project: M1 (core 36) + M2 (15) + M3 (10) = 61 tasks across 60+ commits.
