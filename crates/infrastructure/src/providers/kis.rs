use application::ports::asset_provider::{AssetProvider, ProviderError};
use application::ports::http_client::HttpClient;
use application::ports::secret_store::{SecretError, SecretStore};
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
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const KIS_APP_KEY: &str = "kis_app_key";
pub const KIS_APP_SECRET: &str = "kis_app_secret";

struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

pub struct KisProvider {
    http: Arc<dyn HttpClient>,
    secrets: Arc<dyn SecretStore>,
    base: String,
    cache: Mutex<Option<CachedToken>>,
}

impl KisProvider {
    pub fn new(http: Arc<dyn HttpClient>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            http,
            secrets,
            base: "https://openapi.koreainvestment.com:9443".into(),
            cache: Mutex::new(None),
        }
    }

    pub fn with_base(
        http: Arc<dyn HttpClient>,
        secrets: Arc<dyn SecretStore>,
        base: impl Into<String>,
    ) -> Self {
        Self {
            http,
            secrets,
            base: base.into(),
            cache: Mutex::new(None),
        }
    }

    async fn credentials(&self) -> Result<(String, String), ProviderError> {
        let key = self.secrets.get(KIS_APP_KEY).await.map_err(map_secret_err)?;
        let secret = self
            .secrets
            .get(KIS_APP_SECRET)
            .await
            .map_err(map_secret_err)?;
        Ok((key, secret))
    }

    async fn get_token(&self) -> Result<(String, String, String), ProviderError> {
        // Returns (access_token, app_key, app_secret).
        let (key, secret) = self.credentials().await?;
        {
            let cache = self.cache.lock().await;
            if let Some(ref t) = *cache {
                if t.expires_at > Utc::now() + chrono::Duration::seconds(60) {
                    return Ok((t.access_token.clone(), key, secret));
                }
            }
        }

        let url = format!("{}/oauth2/tokenP", self.base);
        let body = serde_json::json!({
            "grant_type": "client_credentials",
            "appkey": key,
            "appsecret": secret,
        });
        // The current `HttpClient` port only exposes GET. To avoid widening the
        // application port for one adapter's needs, this provider issues the
        // token POST via a one-off reqwest client. This is a tactical exception
        // scoped to this adapter only.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if status >= 400 {
            return Err(ProviderError::Upstream(format!(
                "kis tokenP status {} body {}",
                status,
                String::from_utf8_lossy(&body_bytes)
            )));
        }
        let v: serde_json::Value = serde_json::from_slice(&body_bytes)
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        let token = v
            .get("access_token")
            .and_then(|x| x.as_str())
            .ok_or_else(|| ProviderError::Parse("missing access_token".into()))?
            .to_string();
        let ttl = v
            .get("expires_in")
            .and_then(|x| x.as_i64())
            .unwrap_or(60 * 60 * 23); // 23h fallback if missing
        let expires_at = Utc::now() + chrono::Duration::seconds(ttl);
        *self.cache.lock().await = Some(CachedToken {
            access_token: token.clone(),
            expires_at,
        });
        Ok((token, key, secret))
    }
}

fn map_secret_err(e: SecretError) -> ProviderError {
    match e {
        SecretError::NotFound(_) => ProviderError::Upstream("kis credentials not set".into()),
        other => ProviderError::Upstream(other.to_string()),
    }
}

#[async_trait]
impl AssetProvider for KisProvider {
    fn name(&self) -> &'static str {
        "kis"
    }
    fn supports(&self, s: &Symbol) -> bool {
        s.kind() == AssetKind::KrEquity
    }

    async fn fetch_quotes(&self, symbols: &[Symbol]) -> Result<Vec<Quote>, ProviderError> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        let (token, key, secret) = self.get_token().await?;
        let mut out = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let code = symbol.ticker();
            let url = format!(
                "{}/uapi/domestic-stock/v1/quotations/inquire-price?FID_COND_MRKT_DIV_CODE=J&FID_INPUT_ISCD={}",
                self.base, code,
            );
            let headers: &[(&'static str, String)] = &[
                ("authorization", format!("Bearer {}", token)),
                ("appkey", key.clone()),
                ("appsecret", secret.clone()),
                ("tr_id", "FHKST01010100".to_string()),
                ("custtype", "P".to_string()),
            ];
            let resp = self
                .http
                .get(&url, headers)
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;
            if resp.status == 401 {
                // Invalidate the cached token and surface as Upstream so the
                // fallback loop in MarketService can move on; the next refresh
                // tick will retry with a freshly minted token.
                *self.cache.lock().await = None;
                return Err(ProviderError::Upstream(
                    "kis 401 unauthorized; token cleared".into(),
                ));
            }
            if resp.status >= 400 {
                return Err(ProviderError::Upstream(format!(
                    "kis inquire-price {} for {}",
                    resp.status, code
                )));
            }

            let v: serde_json::Value = serde_json::from_slice(&resp.body)
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            // rt_cd == "0" means OK; otherwise msg1 has the reason.
            if v.get("rt_cd").and_then(|x| x.as_str()) != Some("0") {
                let msg = v.get("msg1").and_then(|x| x.as_str()).unwrap_or("");
                return Err(ProviderError::Upstream(format!(
                    "kis rt_cd != 0 ({})",
                    msg
                )));
            }
            let output = v
                .get("output")
                .ok_or_else(|| ProviderError::Parse("missing output".into()))?;
            let prpr = output
                .get("stck_prpr")
                .and_then(|x| x.as_str())
                .ok_or_else(|| ProviderError::Parse("missing stck_prpr".into()))?;
            let amount = Decimal::from_str(prpr).map_err(|e| ProviderError::Parse(e.to_string()))?;
            let krw = Currency::new("KRW").unwrap();
            let mut q = Quote::new(
                symbol.clone(),
                Price::new(Money::new(amount, krw)),
                Utc::now(),
            );

            // prdy_ctrt is the day-over-day percent change in percent (e.g. "1.59" for +1.59%).
            if let Some(pct) = output.get("prdy_ctrt").and_then(|x| x.as_str()) {
                if let Ok(d) = Decimal::from_str(pct) {
                    // Convert percent → ratio.
                    q.change_24h = Some(d / Decimal::from(100));
                }
            }
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
            "kis candles not implemented yet".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::ReqwestHttpClient;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    /// Tiny in-test SecretStore stub. The `mockall::automock` impl in the
    /// application crate is gated by `cfg(test)` for that crate, so it's not
    /// reachable from other crates' tests.
    struct StubSecrets {
        app_key: String,
        app_secret: String,
    }

    #[async_trait]
    impl SecretStore for StubSecrets {
        async fn get(&self, key: &str) -> Result<String, SecretError> {
            match key {
                KIS_APP_KEY => Ok(self.app_key.clone()),
                KIS_APP_SECRET => Ok(self.app_secret.clone()),
                other => Err(SecretError::NotFound(other.into())),
            }
        }
        async fn set(&self, _key: &str, _value: &str) -> Result<(), SecretError> {
            Ok(())
        }
        async fn delete(&self, _key: &str) -> Result<(), SecretError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn fetches_quote_with_token() {
        let server = MockServer::start().await;
        // Token endpoint
        Mock::given(method("POST"))
            .and(path("/oauth2/tokenP"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "tk-1",
                "expires_in": 86400,
            })))
            .mount(&server)
            .await;
        // Quote endpoint
        Mock::given(method("GET"))
            .and(path("/uapi/domestic-stock/v1/quotations/inquire-price"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rt_cd": "0",
                "msg1": "정상처리 되었습니다.",
                "output": {
                    "stck_prpr": "76800",
                    "prdy_ctrt": "1.59",
                },
            })))
            .mount(&server)
            .await;

        let secrets = Arc::new(StubSecrets {
            app_key: "app-key".into(),
            app_secret: "app-secret".into(),
        });

        let provider = KisProvider::with_base(
            Arc::new(ReqwestHttpClient::new()),
            secrets,
            server.uri(),
        );
        let s = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let quotes = provider.fetch_quotes(&[s]).await.unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].price.money().amount(), Decimal::from(76800));
        assert_eq!(quotes[0].price.money().currency().as_str(), "KRW");
        assert!(quotes[0].change_24h.is_some());
    }
}
