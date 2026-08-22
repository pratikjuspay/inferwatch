use super::{ChatMessage, LlmError, LlmProvider, LlmStream, StreamChunk};
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio_stream::wrappers::ReceiverStream;

/// Google Gemini (AI Studio) — OpenAI-compatible concepts, different wire format.
/// Streaming endpoint: :streamGenerateContent?alt=sse
///
/// Key differences from OpenAI:
///   - role names: "model" instead of "assistant"; "system" maps to "user"
///   - contents alternate user/model, and must start with "user"
///   - auth via ?key= query param, not Bearer header
///   - usage arrives in the final frame as "usageMetadata"
pub struct GeminiProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }

    fn url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent",
            self.model
        )
    }
}

#[async_trait::async_trait]
impl LlmProvider for GeminiProvider {
    fn provider_name(&self) -> &'static str {
        "gemini"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn complete_stream(&self, messages: Vec<ChatMessage>) -> Result<LlmStream, LlmError> {
        let contents: Vec<Value> = messages
            .iter()
            .map(|m| {
                // Gemini roles: user / model. Map "system" to "user".
                let role = match m.role.as_str() {
                    "assistant" => "model",
                    "system" => "user",
                    other => other,
                };
                json!({
                    "role": role,
                    "parts": [{ "text": m.content }]
                })
            })
            .collect();

        let body = json!({
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": 2048,
                // Gemini 3.x defaults to high "thinking" depth → ~10s TTFT.
                // Minimal suits a chatbot: same replies, ~8x faster first token.
                "thinkingConfig": { "thinkingLevel": "minimal" }
            },
            // permissive settings so the demo never surfaces a safety block mid-demo
            "safetySettings": [
                { "category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE" },
                { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE" },
                { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE" },
                { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE" }
            ]
        });

        let response = self
            .client
            .post(self.url())
            .query(&[("key", &self.api_key), ("alt", &"sse".to_string())])
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

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamChunk, LlmError>>(64);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            // Gemini SSE framing is identical to OpenAI: "data: {json}\n\n"
            while let Some(item) = stream.next().await {
                match item {
                    Ok(bytes) => {
                        tracing::debug!(bytes = bytes.len(), "gemini chunk received");
                        // Gemini frames are CRLF-terminatanted (\r\n\r\n) — normalize to LF
                        buffer.push_str(&String::from_utf8_lossy(&bytes).replace('\r', ""));
                        while let Some(idx) = buffer.find("\n\n") {
                            let raw = buffer[..idx].to_string();
                            tracing::debug!(frame = %raw, "gemini frame");
                            buffer.drain(..idx + 2);

                            for line in raw.lines() {
                                tracing::debug!(line = %line, "gemini line");
                                let Some(data) = line.strip_prefix("data: ") else {
                                    continue;
                                };
                                match serde_json::from_str::<Value>(data) {
                                    Ok(v) => {
                                        tracing::debug!(parsed = %v, "gemini parsed");
                                        // mid-frame token(s) — Gemini can batch multiple parts
                                        if let Some(text) = v["candidates"][0]["content"]["parts"]
                                            [0]["text"]
                                            .as_str()
                                            .filter(|t| !t.is_empty())
                                        {
                                            tracing::debug!(token = %text, "gemini token");
                                            let _ = tx
                                                .send(Ok(StreamChunk::Token(text.to_string())))
                                                .await;
                                        }
                                        // terminal frame: usage present AND finishReason set
                                        if v["candidates"][0]["finishReason"].is_string() {
                                            if let Some(usage) = v.get("usageMetadata") {
                                            let _ = tx
                                                .send(Ok(StreamChunk::Done {
                                                    input_tokens: usage["promptTokenCount"]
                                                        .as_i64()
                                                        .map(|n| n as i32),
                                                    output_tokens: usage["candidatesTokenCount"]
                                                        .as_i64()
                                                        .map(|n| n as i32),
                                                    }))
                                                .await;
                                            }
                                        }
                                        // embedded error object
                                        if v.get("error").is_some() {
                                            let msg = v["error"]["message"]
                                                .as_str()
                                                .unwrap_or("unknown gemini error")
                                                .to_string();
                                            let _ = tx.send(Err(LlmError::Malformed(msg))).await;
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::debug!(error = %e, "gemini parse failed");
                                        let _ =
                                            tx.send(Err(LlmError::Malformed(e.to_string()))).await;
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
