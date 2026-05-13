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
use std::collections::HashMap;
use std::sync::Arc;

pub struct CoinGeckoProvider {
    http: Arc<dyn HttpClient>,
    base: String,
    id_for_ticker: HashMap<String, String>,
}

impl CoinGeckoProvider {
    /// `id_for_ticker` maps ticker (e.g. "BTC") to CoinGecko id (e.g. "bitcoin").
    pub fn new(http: Arc<dyn HttpClient>, id_for_ticker: HashMap<String, String>) -> Self {
        Self {
            http,
            base: "https://api.coingecko.com".into(),
            id_for_ticker,
        }
    }
    pub fn with_base(
        http: Arc<dyn HttpClient>,
        base: impl Into<String>,
        ids: HashMap<String, String>,
    ) -> Self {
        Self {
            http,
            base: base.into(),
            id_for_ticker: ids,
        }
    }
}

#[async_trait]
impl AssetProvider for CoinGeckoProvider {
    fn name(&self) -> &'static str {
        "coingecko"
    }
    fn supports(&self, s: &Symbol) -> bool {
        s.kind() == AssetKind::Crypto && self.id_for_ticker.contains_key(s.ticker())
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let ids: Vec<&str> = symbols
            .iter()
            .filter_map(|s| self.id_for_ticker.get(s.ticker()).map(|x| x.as_str()))
            .collect();
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let url = format!(
            "{}/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true",
            self.base,
            ids.join(",")
        );
        let resp = self
            .http
            .get(&url, &[])
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status == 429 {
            return Err(ProviderError::RateLimited {
                retry_after_secs: 60,
            });
        }
        if resp.status >= 500 {
            return Err(ProviderError::Upstream(resp.status.to_string()));
        }
        let map: HashMap<String, HashMap<String, f64>> = serde_json::from_slice(&resp.body)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        let ccy = Currency::new("USD").unwrap();
        let mut out = Vec::new();
        for s in symbols {
            let Some(id) = self.id_for_ticker.get(s.ticker()) else {
                continue;
            };
            let Some(entry) = map.get(id) else {
                continue;
            };
            let Some(usd) = entry.get("usd") else {
                continue;
            };
            let amount = Decimal::from_f64_retain(*usd)
                .ok_or_else(|| ProviderError::Parse("price not decimal".into()))?;
            let mut q = Quote::new(s.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = entry
                .get("usd_24h_change")
                .and_then(|f| Decimal::from_f64_retain(*f / 100.0));
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
            "candles not implemented for CoinGecko in M1".into(),
        ))
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
        Mock::given(method("GET"))
            .and(path("/api/v3/simple/price"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bitcoin": { "usd": 67000.5, "usd_24h_change": 1.24 }
            })))
            .mount(&server)
            .await;
        let mut map = HashMap::new();
        map.insert("BTC".into(), "bitcoin".into());
        let p =
            CoinGeckoProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri(), map);
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let q = p.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].price.money().currency().as_str(), "USD");
    }
}
