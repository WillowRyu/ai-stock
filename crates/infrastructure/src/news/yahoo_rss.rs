use application::ports::http_client::HttpClient;
use application::ports::news_provider::{Headline, NewsError, NewsProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::symbol::Symbol;
use std::sync::Arc;

pub struct YahooNewsRss {
    http: Arc<dyn HttpClient>,
    base: String,
}

impl YahooNewsRss {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self {
            http,
            base: "https://feeds.finance.yahoo.com".into(),
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
impl NewsProvider for YahooNewsRss {
    async fn fetch(&self, symbol: &Symbol, limit: usize) -> Result<Vec<Headline>, NewsError> {
        let url = format!(
            "{}/rss/2.0/headline?s={}&region=US&lang=en-US",
            self.base,
            symbol.ticker()
        );
        let resp = self
            .http
            .get(&url, &[])
            .await
            .map_err(|e| NewsError::Upstream(e.to_string()))?;
        if resp.status >= 500 {
            return Err(NewsError::Upstream(resp.status.to_string()));
        }
        let xml = std::str::from_utf8(&resp.body).map_err(|e| NewsError::Parse(e.to_string()))?;
        let doc = roxmltree::Document::parse(xml).map_err(|e| NewsError::Parse(e.to_string()))?;
        let mut out = Vec::new();
        for item in doc
            .descendants()
            .filter(|n| n.has_tag_name("item"))
            .take(limit)
        {
            let mut title = String::new();
            let mut link = String::new();
            let mut date = String::new();
            for child in item.children() {
                match child.tag_name().name() {
                    "title" => title = child.text().unwrap_or("").to_string(),
                    "link" => link = child.text().unwrap_or("").to_string(),
                    "pubDate" => date = child.text().unwrap_or("").to_string(),
                    _ => {}
                }
            }
            let published_at = DateTime::parse_from_rfc2822(&date)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            out.push(Headline {
                title,
                url: link,
                source: "Yahoo Finance".into(),
                published_at,
            });
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
        Mock::given(method("GET"))
            .and(path("/rss/2.0/headline"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FAKE_RSS))
            .mount(&server)
            .await;
        let p = YahooNewsRss::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let h = p.fetch(&s, 5).await.unwrap();
        assert_eq!(h.len(), 2);
        assert!(h[0].title.contains("Apple"));
    }
}
