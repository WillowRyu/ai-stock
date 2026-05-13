use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
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

pub struct YahooProvider {
    http: Arc<dyn HttpClient>,
    base: String,
}

impl YahooProvider {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self {
            http,
            base: "https://query1.finance.yahoo.com".into(),
        }
    }
    pub fn with_base(http: Arc<dyn HttpClient>, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
        }
    }
}

#[async_trait]
impl AssetProvider for YahooProvider {
    fn name(&self) -> &'static str {
        "yahoo"
    }
    fn supports(&self, s: &Symbol) -> bool {
        matches!(
            s.kind(),
            AssetKind::UsEquity | AssetKind::Forex | AssetKind::Commodity
        )
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        let tickers: Vec<&str> = symbols.iter().map(|s| s.ticker()).collect();
        let url = format!(
            "{}/v7/finance/quote?symbols={}",
            self.base,
            tickers.join(",")
        );
        let resp = self
            .http
            .get(&url, &[])
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status == 429 {
            return Err(ProviderError::RateLimited { retry_after_secs: 5 });
        }
        if resp.status >= 500 {
            return Err(ProviderError::Upstream(resp.status.to_string()));
        }

        let v: serde_json::Value =
            serde_json::from_slice(&resp.body).map_err(|e| ProviderError::Parse(e.to_string()))?;
        let arr = v
            .pointer("/quoteResponse/result")
            .and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("missing quoteResponse.result".into()))?;
        let mut out = Vec::new();
        for item in arr {
            let ticker = item.get("symbol").and_then(|x| x.as_str()).unwrap_or("");
            let Some(symbol) = symbols.iter().find(|s| s.ticker() == ticker) else {
                continue;
            };
            let price_f = item
                .get("regularMarketPrice")
                .and_then(|x| x.as_f64())
                .ok_or_else(|| ProviderError::Parse("missing regularMarketPrice".into()))?;
            let ccy_s = item
                .get("currency")
                .and_then(|x| x.as_str())
                .unwrap_or("USD");
            let ccy = Currency::new(ccy_s).map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
            let amount = Decimal::from_f64_retain(price_f)
                .ok_or_else(|| ProviderError::Parse("price not decimal".into()))?;
            let mut q = Quote::new(symbol.clone(), Price::new(Money::new(amount, ccy)), Utc::now());
            q.change_24h = item
                .get("regularMarketChangePercent")
                .and_then(|x| x.as_f64())
                .and_then(|f| Decimal::from_f64_retain(f / 100.0));
            q.volume_24h = item
                .get("regularMarketVolume")
                .and_then(|x| x.as_f64())
                .and_then(Decimal::from_f64_retain);
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(
        &self,
        s: &Symbol,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        interval: CandleInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        // Yahoo uses `60m` (not `1h`) and `1wk` (not `1w`).
        let interval_str = match interval {
            CandleInterval::OneMin => "1m",
            CandleInterval::FiveMin => "5m",
            CandleInterval::FifteenMin => "15m",
            CandleInterval::ThirtyMin => "30m",
            CandleInterval::OneHour => "60m",
            CandleInterval::OneDay => "1d",
            CandleInterval::OneWeek => "1wk",
        };
        let url = format!(
            "{}/v8/finance/chart/{}?period1={}&period2={}&interval={}",
            self.base,
            s.ticker(),
            from.timestamp(),
            to.timestamp(),
            interval_str,
        );
        let resp = self
            .http
            .get(&url, &[])
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status >= 500 {
            return Err(ProviderError::Upstream(resp.status.to_string()));
        }
        let v: serde_json::Value =
            serde_json::from_slice(&resp.body).map_err(|e| ProviderError::Parse(e.to_string()))?;
        let result = v
            .pointer("/chart/result/0")
            .ok_or_else(|| ProviderError::Parse("no result".into()))?;
        let timestamps = result
            .pointer("/timestamp")
            .and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("no timestamps".into()))?;
        let q = result
            .pointer("/indicators/quote/0")
            .ok_or_else(|| ProviderError::Parse("no quote".into()))?;
        let opens = q
            .get("open")
            .and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("opens".into()))?;
        let highs = q
            .get("high")
            .and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("highs".into()))?;
        let lows = q
            .get("low")
            .and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("lows".into()))?;
        let closes = q
            .get("close")
            .and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("closes".into()))?;
        let volumes = q
            .get("volume")
            .and_then(|x| x.as_array())
            .ok_or_else(|| ProviderError::Parse("volumes".into()))?;

        let ccy_s = result
            .pointer("/meta/currency")
            .and_then(|x| x.as_str())
            .unwrap_or("USD");
        let ccy = Currency::new(ccy_s).map_err(|e| ProviderError::Parse(format!("{e:?}")))?;
        let to_money = |f: f64| -> Result<Price, ProviderError> {
            Ok(Price::new(Money::new(
                Decimal::from_f64_retain(f).ok_or_else(|| ProviderError::Parse("nan".into()))?,
                ccy,
            )))
        };

        let mut out = Vec::new();
        for (i, ts_val) in timestamps.iter().enumerate() {
            let ts = ts_val
                .as_i64()
                .ok_or_else(|| ProviderError::Parse("ts".into()))?;
            let opened_at = Utc
                .timestamp_opt(ts, 0)
                .single()
                .ok_or_else(|| ProviderError::Parse("ts".into()))?;
            let (Some(o), Some(h), Some(l), Some(c), Some(v)) = (
                opens.get(i).and_then(|x| x.as_f64()),
                highs.get(i).and_then(|x| x.as_f64()),
                lows.get(i).and_then(|x| x.as_f64()),
                closes.get(i).and_then(|x| x.as_f64()),
                volumes.get(i).and_then(|x| x.as_f64()),
            ) else {
                continue;
            };
            out.push(Candle {
                symbol: s.clone(),
                open: to_money(o)?,
                high: to_money(h)?,
                low: to_money(l)?,
                close: to_money(c)?,
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
        Mock::given(method("GET"))
            .and(path("/v7/finance/quote"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "quoteResponse": { "result": [
                    { "symbol": "AAPL", "regularMarketPrice": 182.45, "currency": "USD",
                      "regularMarketChangePercent": 1.24, "regularMarketVolume": 52_000_000 }
                ]}
            })))
            .mount(&server)
            .await;
        let provider = YahooProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert!(q[0].change_24h.is_some());
    }
}
