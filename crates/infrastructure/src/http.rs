use application::ports::http_client::{HttpClient, HttpError, HttpResponse};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(concat!("ai-stock/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client");
        Self { client }
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(
        &self,
        url: &str,
        headers: &[(&'static str, String)],
    ) -> Result<HttpResponse, HttpError> {
        let mut req = self.client.get(url);
        for (k, v) in headers {
            req = req.header(*k, v);
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                HttpError::Timeout(Duration::from_secs(5))
            } else if e.is_builder() {
                HttpError::InvalidUrl(url.into())
            } else {
                HttpError::Network(e.to_string())
            }
        })?;
        let status = resp.status().as_u16();
        let headers_map: HashMap<String, String> = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
            .collect();
        let body = resp
            .bytes()
            .await
            .map_err(|e| HttpError::Network(e.to_string()))?
            .to_vec();
        Ok(HttpResponse {
            status,
            headers: headers_map,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn round_trips_response_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/hello"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hi"))
            .mount(&server)
            .await;

        let client = ReqwestHttpClient::new();
        let resp = client
            .get(&format!("{}/hello", server.uri()), &[])
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"hi");
    }
}
