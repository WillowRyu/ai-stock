use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    asset::AssetKind,
    candle::{Candle, CandleInterval},
    money::{Currency, Money},
    price::Price,
    quote::Quote,
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
        Self {
            http,
            base: "https://finnhub.io".into(),
            api_key: api_key.into(),
        }
    }
    pub fn with_base(
        http: Arc<dyn HttpClient>,
        api_key: impl Into<String>,
        base: impl Into<String>,
    ) -> Self {
        Self {
            http,
            base: base.into(),
            api_key: api_key.into(),
        }
    }
}

#[async_trait]
impl AssetProvider for FinnhubProvider {
    fn name(&self) -> &'static str {
        "finnhub"
    }
    fn supports(&self, s: &Symbol) -> bool {
        s.kind() == AssetKind::UsEquity
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::with_capacity(symbols.len());
        for s in symbols {
            let url = format!(
                "{}/api/v1/quote?symbol={}&token={}",
                self.base,
                s.ticker(),
                self.api_key
            );
            let resp = self
                .http
                .get(&url, &[])
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status == 429 {
                return Err(ProviderError::RateLimited { retry_after_secs: 60 });
            }
            if resp.status >= 500 {
                return Err(ProviderError::Upstream(resp.status.to_string()));
            }
            let v: serde_json::Value = serde_json::from_slice(&resp.body)
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            let c = v
                .get("c")
                .and_then(|x| x.as_f64())
                .ok_or_else(|| ProviderError::Parse("missing c (current)".into()))?;
            let dp = v.get("dp").and_then(|x| x.as_f64());
            let ccy = Currency::new("USD").unwrap();
            let amount =
                Decimal::from_f64_retain(c).ok_or_else(|| ProviderError::Parse("nan".into()))?;
            let mut q = Quote::new(s.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = dp.and_then(|f| Decimal::from_f64_retain(f / 100.0));
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(
        &self,
        _s: &Symbol,
        _from: DateTime<Utc>,
        _to: DateTime<Utc>,
        _interval: CandleInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        Err(ProviderError::Upstream(
            "Finnhub candle endpoint behind paid tier; deferred".into(),
        ))
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
        Mock::given(method("GET"))
            .and(path("/api/v1/quote"))
            .and(query_param("symbol", "AAPL"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "c": 182.45, "dp": 1.24
            })))
            .mount(&server)
            .await;
        let provider = FinnhubProvider::with_base(
            Arc::new(ReqwestHttpClient::new()),
            "test-key",
            server.uri(),
        );
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
    }
}
