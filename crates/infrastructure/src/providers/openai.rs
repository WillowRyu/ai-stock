use application::ports::ai_provider::{AiChunk, AiError, AiProvider, AiRequest};
use domain::conversation::Role;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;

pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key,
            base: "https://api.openai.com".into(),
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
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    max_tokens: u32,
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn stream(
        &self,
        request: AiRequest,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let mut messages = vec![OpenAiMessage {
            role: "system".into(),
            content: request.system,
        }];
        for m in request.messages {
            messages.push(OpenAiMessage {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                },
                content: m.content,
            });
        }
        let body = OpenAiRequest {
            model: self.model.clone(),
            messages,
            stream: true,
            max_tokens: request.max_output_tokens,
        };
        let req = self
            .client
            .post(format!("{}/v1/chat/completions", self.base))
            .bearer_auth(&self.api_key)
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
                if ev.data == "[DONE]" {
                    return Ok(AiChunk::Done);
                }
                let v: serde_json::Value =
                    serde_json::from_str(&ev.data).map_err(|e| AiError::Parse(e.to_string()))?;
                let text = v
                    .pointer("/choices/0/delta/content")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                Ok(AiChunk::Text(text.to_string()))
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
    async fn streams_text_then_done() {
        let server = MockServer::start().await;
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\ndata: [DONE]\n\n";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_string_contains("\"role\":\"assistant\""))
            .and(body_string_contains("\"role\":\"system\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = OpenAiProvider::with_base("test-key".into(), "gpt-4o".into(), server.uri());
        let request = AiRequest {
            system: "be brief".into(),
            messages: vec![
                domain::conversation::Message::user("hello"),
                domain::conversation::Message::assistant("hi"),
                domain::conversation::Message::user("more"),
            ],
            max_output_tokens: 100,
        };
        let mut stream = provider.stream(request).await.unwrap();
        let mut collected = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk.unwrap() {
                AiChunk::Text(t) => collected.push_str(&t),
                AiChunk::Done => break,
            }
        }
        assert_eq!(collected, "hi world");
    }
}
