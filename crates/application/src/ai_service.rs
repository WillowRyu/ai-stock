use crate::indicator_service;
use crate::market_service::MarketService;
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

pub type ProviderFactory =
    Arc<dyn Fn(&str, &str) -> Option<Arc<dyn AiProvider>> + Send + Sync>;

pub struct AiService {
    secrets: Arc<dyn SecretStore>,
    market: Arc<MarketService>,
    news: Vec<Arc<dyn NewsProvider>>,
    provider_factory: ProviderFactory,
}

impl AiService {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        market: Arc<MarketService>,
        news: Vec<Arc<dyn NewsProvider>>,
        provider_factory: ProviderFactory,
    ) -> Self {
        Self { secrets, market, news, provider_factory }
    }

    pub async fn commentary(
        &self,
        provider_kind: &str,
        symbol: &Symbol,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiServiceError> {
        let key_name = format!("{}_api_key", provider_kind);
        let key = self.secrets.get(&key_name).await?;
        let provider = (self.provider_factory)(provider_kind, &key)
            .ok_or_else(|| AiServiceError::NotConfigured(provider_kind.into()))?;

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
            match self.market.fetch_candles(symbol, from, to, domain::candle::CandleInterval::OneDay).await {
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

        // Build refs after we own all the data so refs don't outlive the data.
        let headline_refs: Vec<HeadlineRef> = all_headlines.iter()
            .map(|h| HeadlineRef { title: &h.title, source: &h.source })
            .collect();
        let ctx = PromptContext {
            symbol,
            quote: quote.as_ref(),
            indicators,
            headlines: &headline_refs,
        };
        let (system, user) = build_commentary_prompt(&ctx);

        Ok(provider
            .stream(AiRequest {
                system,
                messages: vec![Message::user(user)],
                max_output_tokens: 600,
            })
            .await?)
    }
}
