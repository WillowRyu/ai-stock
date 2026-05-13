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
use scraper::{Html, Selector};
use std::str::FromStr;
use std::sync::Arc;

pub struct NaverKrProvider {
    http: Arc<dyn HttpClient>,
    base: String,
}

impl NaverKrProvider {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self {
            http,
            base: "https://finance.naver.com".into(),
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
impl AssetProvider for NaverKrProvider {
    fn name(&self) -> &'static str {
        "naver-kr"
    }
    fn supports(&self, s: &Symbol) -> bool {
        s.kind() == AssetKind::KrEquity
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::new();
        for s in symbols {
            let code = s.ticker();
            let url = format!("{}/item/main.naver?code={}", self.base, code);
            let resp = self
                .http
                .get(&url, &[])
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status >= 500 {
                return Err(ProviderError::Upstream(resp.status.to_string()));
            }
            let html = String::from_utf8_lossy(&resp.body);
            let doc = Html::parse_document(&html);
            // Naver finance puts the current price in `.no_today .blind`.
            let sel = Selector::parse(".no_today .blind")
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            let text = doc
                .select(&sel)
                .next()
                .ok_or_else(|| ProviderError::Parse("price not found in DOM".into()))?
                .text()
                .collect::<String>();
            let cleaned: String = text
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let amount =
                Decimal::from_str(&cleaned).map_err(|e| ProviderError::Parse(e.to_string()))?;
            let krw = Currency::new("KRW").unwrap();
            out.push(Quote::new(
                s.clone(),
                Price::new(Money::new(amount, krw)),
                Utc::now(),
            ));
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
            "Naver candle endpoint not implemented in M2; pending".into(),
        ))
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
        Mock::given(method("GET"))
            .and(path("/item/main.naver"))
            .and(query_param("code", "005930"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FAKE_HTML))
            .mount(&server)
            .await;
        let provider = NaverKrProvider::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].price.money().amount(), Decimal::from(76800));
        assert_eq!(q[0].price.money().currency().as_str(), "KRW");
    }
}
