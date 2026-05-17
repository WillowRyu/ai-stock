use application::ports::ai_provider::{AiChunk, AiError, AiProvider, AiRequest};
use domain::conversation::Role;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;

pub struct GeminiProvider {
    client: reqwest::Client,
    api_key: String,
    base: String,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest"),
            api_key,
            base: "https://generativelanguage.googleapis.com".into(),
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
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
    role: String,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "systemInstruction")]
    system_instruction: GeminiContent,
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    async fn stream(
        &self,
        request: AiRequest,
    ) -> Result<BoxStream<'static, Result<AiChunk, AiError>>, AiError> {
        if self.api_key.is_empty() {
            return Err(AiError::NotConfigured);
        }
        let contents: Vec<GeminiContent> = request
            .messages
            .into_iter()
            .map(|m| GeminiContent {
                parts: vec![GeminiPart { text: m.content }],
                // Gemini names the assistant role "model".
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "model".into(),
                },
            })
            .collect();
        let body = GeminiRequest {
            contents,
            system_instruction: GeminiContent {
                parts: vec![GeminiPart { text: request.system }],
                role: "system".into(),
            },
        };
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base, self.model, self.api_key,
        );
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;
        match resp.status().as_u16() {
            200 => {}
            401 | 403 => return Err(AiError::Unauthorized),
            429 => return Err(AiError::RateLimited { retry_after_secs: 30 }),
            code => return Err(AiError::Upstream(format!("status {}", code))),
        }

        let stream = resp.bytes_stream().eventsource().map(|event| match event {
            Ok(ev) => {
                let v: serde_json::Value =
                    serde_json::from_str(&ev.data).map_err(|e| AiError::Parse(e.to_string()))?;
                let text = v
                    .pointer("/candidates/0/content/parts/0/text")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                let done = v.pointer("/candidates/0/finishReason").is_some();
                if done && text.is_empty() {
                    Ok(AiChunk::Done)
                } else {
                    Ok(AiChunk::Text(text.to_string()))
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
    async fn streams_gemini_parts() {
        let server = MockServer::start().await;
        let sse = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\ndata: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\" world\"}]}}]}\n\ndata: {\"candidates\":[{\"finishReason\":\"STOP\",\"content\":{\"parts\":[{\"text\":\"\"}]}}]}\n\n";
        Mock::given(method("POST"))
            .and(path_regex(r"^/v1beta/models/.*:streamGenerateContent"))
            .and(body_string_contains("\"role\":\"model\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider =
            GeminiProvider::with_base("k".into(), "gemini-2.0-flash".into(), server.uri());
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
