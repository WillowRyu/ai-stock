use crate::indicator_service;
use crate::market_service::MarketService;
use crate::ports::{
    ai_provider::{AiChunk, AiError, AiProvider, AiRequest},
    news_provider::NewsProvider,
    secret_store::{SecretError, SecretStore},
};
use domain::{
    conversation::{Conversation, Message},
    prompt::{build_prompt, system_prompt, HeadlineRef, IndicatorContext, PromptContext, PromptKind},
    symbol::Symbol,
};
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
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

pub type ProviderFactory =
    Arc<dyn Fn(&str, &str) -> Option<Arc<dyn AiProvider>> + Send + Sync>;

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

pub struct AiService {
    secrets: Arc<dyn SecretStore>,
    market: Arc<MarketService>,
    news: Vec<Arc<dyn NewsProvider>>,
    provider_factory: ProviderFactory,
    conversations: Mutex<HashMap<Symbol, SymbolChat>>,
}

impl AiService {
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
}

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
        last_request: std::sync::Arc<std::sync::Mutex<Option<AiRequest>>>,
    }

    #[async_trait::async_trait]
    impl AiProvider for MockAi {
        fn name(&self) -> &'static str {
            "mock"
        }
        async fn stream(
            &self,
            request: AiRequest,
        ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
            *self.last_request.lock().unwrap() = Some(request.clone());
            let reply = self.reply.clone();
            Ok(futures::stream::iter(vec![
                Ok(AiChunk::Text(reply)),
                Ok(AiChunk::Done),
            ])
            .boxed())
        }
    }

    fn build_service(reply: &str) -> (AiService, std::sync::Arc<std::sync::Mutex<Option<AiRequest>>>) {
        let mut secrets = MockSecretStore::new();
        secrets
            .expect_get()
            .returning(|_| Ok("test-key".to_string()));
        let market = Arc::new(MarketService::new(
            Arc::new(MockWatchlistRepo::new()),
            vec![],
        ));
        let reply = reply.to_string();
        let last_request: std::sync::Arc<std::sync::Mutex<Option<AiRequest>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let captured = last_request.clone();
        let factory: ProviderFactory = Arc::new(move |_kind: &str, _key: &str| {
            Some(Arc::new(MockAi {
                reply: reply.clone(),
                last_request: captured.clone(),
            }) as Arc<dyn AiProvider>)
        });
        let service = AiService::new(Arc::new(secrets), market, vec![], factory);
        (service, last_request)
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
        let (svc, _) = build_service("first reply");
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
        let (svc, _) = build_service("reply");
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
        let (svc, _) = build_service("ignored");
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
        let (svc, _) = build_service("analysis");
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

    #[tokio::test]
    async fn context_sent_to_provider_is_capped() {
        let (svc, last_request) = build_service("ok");
        let sym = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();

        // Build a conversation longer than the cap: each cycle adds a user
        // turn plus an assistant turn (2 messages).
        for i in 0..(MAX_CONTEXT_MESSAGES) {
            let stream = svc
                .send_message("openai", &sym, &format!("q{i}"))
                .await
                .unwrap();
            let reply = drain(stream).await;
            svc.commit_assistant(&sym, reply).await;
        }

        let sent = last_request.lock().unwrap().clone().unwrap();
        assert_eq!(sent.messages.len(), MAX_CONTEXT_MESSAGES);
    }
}
