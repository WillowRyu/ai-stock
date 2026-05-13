use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
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
    main_base: String,   // finance.naver.com — quote scraping
    fchart_base: String, // fchart.stock.naver.com — candles + name
}

impl NaverKrProvider {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self {
            http,
            main_base: "https://finance.naver.com".into(),
            fchart_base: "https://fchart.stock.naver.com".into(),
        }
    }
    pub fn with_bases(
        http: Arc<dyn HttpClient>,
        main_base: impl Into<String>,
        fchart_base: impl Into<String>,
    ) -> Self {
        Self {
            http,
            main_base: main_base.into(),
            fchart_base: fchart_base.into(),
        }
    }
}

fn timeframe_for(interval: CandleInterval) -> Option<&'static str> {
    // Naver fchart only supports minute / day / week / month.
    // Map our coarser intraday intervals to minute (with count adjustment is
    // the caller's job); 5m/15m/30m/1h aren't first-class. We return Err for
    // those rather than silently downgrade.
    match interval {
        CandleInterval::OneMin => Some("minute"),
        CandleInterval::OneDay => Some("day"),
        CandleInterval::OneWeek => Some("week"),
        _ => None,
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
            let url = format!("{}/item/main.naver?code={}", self.main_base, code);
            let resp = self
                .http
                .get(&url, &[])
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status >= 400 {
                return Err(ProviderError::Upstream(format!("naver main {}", resp.status)));
            }
            let html = String::from_utf8_lossy(&resp.body);
            let doc = Html::parse_document(&html);

            // Price.
            let price_sel = Selector::parse(".no_today .blind")
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            let text = doc
                .select(&price_sel)
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

            // Company name. Naver puts the name in `.wrap_company h2 a`. If
            // missing, fall back to <title> stripped of the suffix.
            let name_sel = Selector::parse(".wrap_company h2 a").ok();
            let mut display_name = name_sel
                .as_ref()
                .and_then(|sel| doc.select(sel).next())
                .map(|n| n.text().collect::<String>().trim().to_string())
                .filter(|n| !n.is_empty());
            if display_name.is_none() {
                let title_sel = Selector::parse("title").ok();
                display_name = title_sel
                    .as_ref()
                    .and_then(|sel| doc.select(sel).next())
                    .map(|n| {
                        let t = n.text().collect::<String>();
                        // Title looks like "삼성전자 : 네이버페이 증권" — split on " : ".
                        t.split(" : ").next().unwrap_or(t.as_str()).trim().to_string()
                    })
                    .filter(|n| !n.is_empty());
            }

            let mut q = Quote::new(s.clone(), Price::new(Money::new(amount, krw)), Utc::now());
            q.display_name = display_name;
            out.push(q);
        }
        Ok(out)
    }

    async fn fetch_candles(
        &self,
        s: &Symbol,
        _from: DateTime<Utc>,
        _to: DateTime<Utc>,
        interval: CandleInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        let tf = timeframe_for(interval).ok_or_else(|| {
            ProviderError::Upstream(format!(
                "naver fchart doesn't support interval {} for KR equities",
                interval.as_str(),
            ))
        })?;
        // Naver fchart `count` is the number of bars; we ask for a generous
        // window and let the caller crop. 300 is plenty for any UI preset.
        let count = 300;
        let url = format!(
            "{}/sise.nhn?symbol={}&timeframe={}&count={}&requestType=0",
            self.fchart_base,
            s.ticker(),
            tf,
            count,
        );
        let resp = self
            .http
            .get(&url, &[])
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if resp.status >= 400 {
            return Err(ProviderError::Upstream(format!("naver fchart {}", resp.status)));
        }
        // Naver's fchart response is declared `encoding="EUC-KR"` and the
        // `name` attribute on chartdata contains EUC-KR bytes (the rest of
        // the document is ASCII). Lossy UTF-8 conversion replaces those
        // bytes with U+FFFD — fine for us since we only read the ASCII
        // `<item data="..."/>` payloads.
        let xml = String::from_utf8_lossy(&resp.body);
        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        let krw = Currency::new("KRW").unwrap();
        let to_money = |s: &str| -> Result<Price, ProviderError> {
            let amt = Decimal::from_str(s).map_err(|e| ProviderError::Parse(e.to_string()))?;
            Ok(Price::new(Money::new(amt, krw)))
        };

        let mut out = Vec::new();
        for item in doc.descendants().filter(|n| n.has_tag_name("item")) {
            let Some(data) = item.attribute("data") else {
                continue;
            };
            let parts: Vec<&str> = data.split('|').collect();
            if parts.len() < 6 {
                continue;
            }
            let opened_at = parse_naver_timestamp(parts[0])
                .ok_or_else(|| ProviderError::Parse(format!("bad timestamp: {}", parts[0])))?;
            let candle = Candle {
                symbol: s.clone(),
                open: to_money(parts[1])?,
                high: to_money(parts[2])?,
                low: to_money(parts[3])?,
                close: to_money(parts[4])?,
                volume: Decimal::from_str(parts[5])
                    .map_err(|e| ProviderError::Parse(e.to_string()))?,
                opened_at,
            };
            out.push(candle);
        }
        Ok(out)
    }
}

fn parse_naver_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // 8-digit "YYYYMMDD" for day/week/month; 12-digit "YYYYMMDDHHMM" for minute.
    match s.len() {
        8 => NaiveDate::parse_from_str(s, "%Y%m%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .and_then(|ndt| Utc.from_local_datetime(&ndt).single()),
        12 => NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M")
            .ok()
            .and_then(|ndt| Utc.from_local_datetime(&ndt).single()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    const FAKE_QUOTE_HTML: &str = r##"
        <html><head><title>삼성전자 : 네이버페이 증권</title></head><body>
          <div class="wrap_company"><h2><a href="#">삼성전자</a></h2></div>
          <div class="no_today">
            <span class="blind">76800</span>
          </div>
        </body></html>
    "##;

    const FAKE_FCHART_XML: &str = r#"<?xml version="1.0" encoding="EUC-KR" standalone="yes" ?>
<protocol>
  <chartdata symbol="005930" company="삼성전자" count="2" timeframe="day" precision="0" origintime="20000101">
    <item data="20251101|71000|72000|70500|71500|1234567" />
    <item data="20251104|71500|72500|71000|72000|1100000" />
  </chartdata>
</protocol>"#;

    #[tokio::test]
    async fn parses_naver_price() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/item/main.naver"))
            .and(query_param("code", "005930"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FAKE_QUOTE_HTML))
            .mount(&server)
            .await;
        let provider = NaverKrProvider::with_bases(
            Arc::new(ReqwestHttpClient::new()),
            server.uri(),
            "https://fchart-unused.example",
        );
        let s = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].price.money().amount(), Decimal::from(76800));
        assert_eq!(q[0].price.money().currency().as_str(), "KRW");
    }

    #[tokio::test]
    async fn quote_includes_company_name() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/item/main.naver"))
            .and(query_param("code", "005930"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FAKE_QUOTE_HTML))
            .mount(&server)
            .await;
        let provider = NaverKrProvider::with_bases(
            Arc::new(ReqwestHttpClient::new()),
            server.uri(),
            "https://fchart-unused.example",
        );
        let s = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let q = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].display_name.as_deref(), Some("삼성전자"));
    }

    #[tokio::test]
    async fn fchart_returns_daily_candles() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sise.nhn"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FAKE_FCHART_XML))
            .mount(&server)
            .await;
        let provider = NaverKrProvider::with_bases(
            Arc::new(ReqwestHttpClient::new()),
            "https://unused.example",
            server.uri(),
        );
        let s = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let candles = provider
            .fetch_candles(&s, Utc::now(), Utc::now(), CandleInterval::OneDay)
            .await
            .unwrap();
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].close.money().amount(), Decimal::from(71500));
        assert_eq!(candles[1].open.money().amount(), Decimal::from(71500));
    }

    #[test]
    fn unsupported_interval_errors() {
        // Direct map check — no HTTP call needed.
        assert!(timeframe_for(CandleInterval::FiveMin).is_none());
        assert!(timeframe_for(CandleInterval::ThirtyMin).is_none());
        assert!(timeframe_for(CandleInterval::OneHour).is_none());
        assert!(timeframe_for(CandleInterval::OneDay).is_some());
        assert!(timeframe_for(CandleInterval::OneMin).is_some());
    }
}
