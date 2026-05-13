use application::ports::http_client::HttpClient;
use application::ports::news_provider::{Headline, NewsError, NewsProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{asset::AssetKind, symbol::Symbol};
use std::sync::Arc;

pub struct CoinDeskRss {
    http: Arc<dyn HttpClient>,
    base: String,
}

impl CoinDeskRss {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self {
            http,
            base: "https://www.coindesk.com".into(),
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
impl NewsProvider for CoinDeskRss {
    async fn fetch(&self, symbol: &Symbol, limit: usize) -> Result<Vec<Headline>, NewsError> {
        if symbol.kind() != AssetKind::Crypto {
            return Ok(vec![]);
        }
        let url = format!("{}/arc/outboundfeeds/rss/", self.base);
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
        let ticker = symbol.ticker().to_lowercase();
        let aliases: &[&str] = match symbol.ticker() {
            "BTC" => &["btc", "bitcoin"],
            "ETH" => &["eth", "ether", "ethereum"],
            "SOL" => &["sol", "solana"],
            _ => &[],
        };
        let mut out = Vec::new();
        for item in doc.descendants().filter(|n| n.has_tag_name("item")) {
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
            let lowercase_title = title.to_lowercase();
            let matches = lowercase_title.contains(&ticker)
                || aliases.iter().any(|a| lowercase_title.contains(a));
            if !matches {
                continue;
            }
            let published_at = DateTime::parse_from_rfc2822(&date)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            out.push(Headline {
                title,
                url: link,
                source: "CoinDesk".into(),
                published_at,
            });
            if out.len() >= limit {
                break;
            }
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
        Mock::given(method("GET"))
            .and(path("/arc/outboundfeeds/rss/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(RSS))
            .mount(&server)
            .await;
        let p = CoinDeskRss::with_base(Arc::new(ReqwestHttpClient::new()), server.uri());
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let h = p.fetch(&s, 5).await.unwrap();
        assert_eq!(h.len(), 1);
        assert!(h[0].title.contains("Bitcoin"));
    }
}
