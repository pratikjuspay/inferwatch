use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod openai;
pub mod gemini;

/// One message in a conversation — provider-agnostic.
/// Same struct works for OpenAI, Anthropic, or anything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,    // "user" | "assistant" | "system"
    pub content: String,
}

/// One chunk of a streamed LLM response.
/// Tokens arrive many times; Done arrives exactly once at the end.
#[derive(Debug)]
pub enum StreamChunk {
    Token(String),
    Done {
        input_tokens: Option<i32>,
        output_tokens: Option<i32>,
    },
}

/// A boxed stream of chunks — what every provider returns.
/// BoxStream<'static, T> = dyn Stream of T, owned, safe to move across tasks.
pub type LlmStream = BoxStream<'static, Result<StreamChunk, LlmError>>;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("provider returned {status}: {body}")]
    Api { status: u16, body: String },

    #[error("malformed stream chunk: {0}")]
    Malformed(String),

    #[error("internal: {0}")]
    Internal(String),
}

/// The multi-provider contract.
/// OpenAI and Anthropic both implement this — the rest of the app
/// never knows or cares which one is behind it (swap via config).
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn model_name(&self) -> &str;

    /// Start a streamed completion. Returns the stream immediately;
    /// tokens are produced as the upstream API sends them.
    async fn complete_stream(&self, messages: Vec<ChatMessage>) -> Result<LlmStream, LlmError>;
}
