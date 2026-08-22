use super::{ChatMessage, LlmError, LlmProvider, LlmStream, StreamChunk};
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use tokio_stream::wrappers::ReceiverStream;

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAIProvider {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn complete_stream(&self, messages: Vec<ChatMessage>) -> Result<LlmStream, LlmError> {
        let body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            // ask OpenAI to include token usage in the final stream chunk
            "stream_options": { "include_usage": true }
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                status: status.as_u16(),
                body,
            });
        }

        // Tokio channel bridges reqwest's byte stream into our LlmStream.
        // The pump task reads raw SSE lines upstream and sends parsed chunks down.
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamChunk, LlmError>>(64);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(item) = stream.next().await {
                match item {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // SSE messages are separated by blank lines ("\n\n")
                        while let Some(idx) = buffer.find("\n\n") {
                            let raw_event = buffer[..idx].to_string();
                            buffer.drain(..idx + 2);

                            for line in raw_event.lines() {
                                let Some(data) = line.strip_prefix("data: ") else {
                                    continue; // ignore comments/other fields
                                };

                                if data == "[DONE]" {
                                    return; // stream finished
                                }

                                match serde_json::from_str::<serde_json::Value>(data) {
                                    Ok(v) => {
                                        // final chunk (stream_options) carries usage
                                        if let Some(usage) = v.get("usage") {
                                            if !usage.is_null() {
                                                let _ = tx
                                                    .send(Ok(StreamChunk::Done {
                                                        input_tokens: usage["prompt_tokens"]
                                                            .as_i64()
                                                            .map(|n| n as i32),
                                                        output_tokens: usage
                                                            ["completion_tokens"]
                                                            .as_i64()
                                                            .map(|n| n as i32),
                                                    }))
                                                    .await;
                                                continue;
                                            }
                                        }
                                        // regular chunk carries a token (or nothing)
                                        if let Some(token) = v["choices"][0]["delta"]["content"]
                                            .as_str()
                                        {
                                            let _ =
                                                tx.send(Ok(StreamChunk::Token(token.to_string())))
                                                    .await;
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx
                                            .send(Err(LlmError::Malformed(e.to_string())))
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(LlmError::Http(e))).await;
                        return;
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
