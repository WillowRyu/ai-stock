use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use domain::{
    asset::AssetKind,
    candle::Candle,
    money::{Currency, Money},
    price::Price,
    quote::Quote,
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
        Self {
            http,
            base: "https://api.binance.com".into(),
        }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
        }
    }
    fn binance_symbol(s: &Symbol) -> Option<String> {
        let qc = s.quote_currency()?;
        if s.kind() != AssetKind::Crypto {
            return None;
        }
        Some(format!("{}{}", s.ticker(), qc))
    }
}

#[derive(Deserialize)]
struct Ticker24h {
    #[serde(rename = "lastPrice")]
    last_price: String,
    #[serde(rename = "priceChangePercent")]
    price_change_percent: String,
    #[serde(rename = "quoteVolume")]
    quote_volume: String,
}

#[async_trait]
impl AssetProvider for BinanceProvider {
    fn name(&self) -> &'static str {
        "binance"
    }
    fn supports(&self, s: &Symbol) -> bool {
        s.kind() == AssetKind::Crypto && s.quote_currency().is_some()
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::with_capacity(symbols.len());
        for s in symbols {
            let bs = Self::binance_symbol(s)
                .ok_or_else(|| ProviderError::UnsupportedSymbol(s.to_canonical_string()))?;
            let url = format!("{}/api/v3/ticker/24hr?symbol={}", self.base, bs);
            let resp = self
                .http
                .get(&url, &[])
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status == 429 {
                return Err(ProviderError::RateLimited { retry_after_secs: 1 });
            }
            if resp.status >= 500 {
                return Err(ProviderError::Upstream(format!(
                    "{} {}",
                    resp.status,
                    String::from_utf8_lossy(&resp.body)
                )));
            }
            let t: Ticker24h = serde_json::from_slice(&resp.body)
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            let amount = Decimal::from_str(&t.last_price)
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            let ccy = Currency::new(s.quote_currency().unwrap())
                .map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
            let mut q = Quote::new(s.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = Decimal::from_str(&t.price_change_percent)
                .ok()
                .map(|d| d / Decimal::from(100));
            q.volume_24h = Decimal::from_str(&t.quote_volume).ok();
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(
        &self,
        s: &Symbol,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, ProviderError> {
        let bs = Self::binance_symbol(s)
            .ok_or_else(|| ProviderError::UnsupportedSymbol(s.to_canonical_string()))?;
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval=1h&startTime={}&endTime={}",
            self.base,
            bs,
            from.timestamp_millis(),
            to.timestamp_millis()
        );
        let resp = self
            .http
            .get(&url, &[])
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status >= 500 || resp.status == 429 {
            return Err(ProviderError::Upstream(format!("{}", resp.status)));
        }
        let arr: Vec<Vec<serde_json::Value>> = serde_json::from_slice(&resp.body)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        let ccy = Currency::new(s.quote_currency().unwrap())
            .map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
        let mut out = Vec::with_capacity(arr.len());
        for k in arr {
            let open_ms = k
                .first()
                .and_then(|v| v.as_i64())
                .ok_or_else(|| ProviderError::Parse("kline open time".into()))?;
            let o = k
                .get(1)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ProviderError::Parse("o".into()))?;
            let h = k
                .get(2)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ProviderError::Parse("h".into()))?;
            let l = k
                .get(3)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ProviderError::Parse("l".into()))?;
            let c = k
                .get(4)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ProviderError::Parse("c".into()))?;
            let v = k
                .get(5)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ProviderError::Parse("v".into()))?;
            let to_money = |s: &str| {
                Ok::<_, ProviderError>(Price::new(Money::new(
                    Decimal::from_str(s).map_err(|e| ProviderError::Parse(e.to_string()))?,
                    ccy,
                )))
            };
            let opened_at = Utc
                .timestamp_millis_opt(open_ms)
                .single()
                .ok_or_else(|| ProviderError::Parse("bad timestamp".into()))?;
            out.push(Candle {
                symbol: s.clone(),
                open: to_money(o)?,
                high: to_money(h)?,
                low: to_money(l)?,
                close: to_money(c)?,
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
        Mock::given(method("GET"))
            .and(path("/api/v3/ticker/24hr"))
            .and(query_param("symbol", "BTCUSDT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "lastPrice": "67000.50",
                "priceChangePercent": "1.24",
                "quoteVolume": "1234567.0",
            })))
            .mount(&server)
            .await;

        let provider = BinanceProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USDT")).unwrap();
        let quotes = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(
            quotes[0].price.money().amount(),
            Decimal::from_str("67000.50").unwrap()
        );
    }
}
