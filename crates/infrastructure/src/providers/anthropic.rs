use application::ports::ai_provider::{AiChunk, AiError, AiProvider, AiRequest};
use domain::conversation::Role;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    base: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key,
            base: "https://api.anthropic.com".into(),
            model,
        }
    }
    pub fn with_base(api_key: String, model: String, base: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key,
            base,
            model,
        }
    }
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    system: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    stream: bool,
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn stream(
        &self,
        request: AiRequest,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let messages: Vec<AnthropicMessage> = request
            .messages
            .into_iter()
            .map(|m| AnthropicMessage {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                },
                content: m.content,
            })
            .collect();
        let body = AnthropicRequest {
            model: self.model.clone(),
            system: request.system,
            messages,
            max_tokens: request.max_output_tokens,
            stream: true,
        };
        let req = self
            .client
            .post(format!("{}/v1/messages", self.base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body);
        let resp = req
            .send()
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;
        match resp.status().as_u16() {
            200 => {}
            401 | 403 => return Err(AiError::Unauthorized),
            429 => return Err(AiError::RateLimited { retry_after_secs: 60 }),
            code => return Err(AiError::Upstream(format!("status {}", code))),
        }
        let stream = resp.bytes_stream().eventsource().map(|event| match event {
            Ok(ev) => {
                let v: serde_json::Value =
                    serde_json::from_str(&ev.data).map_err(|e| AiError::Parse(e.to_string()))?;
                let ty = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
                match ty {
                    "message_stop" => Ok(AiChunk::Done),
                    "content_block_delta" => {
                        let text = v
                            .pointer("/delta/text")
                            .and_then(|x| x.as_str())
                            .unwrap_or("");
                        Ok(AiChunk::Text(text.to_string()))
                    }
                    _ => Ok(AiChunk::Text(String::new())),
                }
            }
            Err(e) => Err(AiError::Network(e.to_string())),
        });
        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streams_content_block_deltas() {
        let server = MockServer::start().await;
        let sse = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\" world\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(body_string_contains("\"system\":"))
            .and(body_string_contains("\"role\":\"assistant\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider =
            AnthropicProvider::with_base("test".into(), "claude-3-7".into(), server.uri());
        let request = AiRequest {
            system: "x".into(),
            messages: vec![
                domain::conversation::Message::user("y"),
                domain::conversation::Message::assistant("earlier"),
            ],
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(request).await.unwrap();
        let mut text = String::new();
        while let Some(c) = stream.next().await {
            match c.unwrap() {
                AiChunk::Text(t) => text.push_str(&t),
                AiChunk::Done => break,
            }
        }
        assert_eq!(text, "hi world");
    }
}
