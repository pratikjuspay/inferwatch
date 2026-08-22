use crate::db::{conversations as conv_db, messages as msg_db};
use crate::errors::AppError;
use crate::providers::{ChatMessage, StreamChunk};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use uuid::Uuid;

const HISTORY_LIMIT: i64 = 20; // "short conversational context"

#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: Uuid,
    pub message: String,
}

pub async fn chat(
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
    Json(body): Json<ChatRequest>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    // ——— 1. verify ownership ———
    let conversation = conv_db::get_for_session(&state.pool, conversation_id, body.session_id)
        .await?
        .ok_or_else(|| AppError::NotFound("conversation".into()))?;

    // first message becomes the conversation title (truncated)
    if conversation.title == "New conversation" {
        let title: String = body.message.chars().take(40).collect();
        let _ = sqlx::query!(
            "UPDATE conversations SET title = $1 WHERE id = $2",
            title,
            conversation_id
        )
        .execute(&state.pool)
        .await;
    }

    // ——— 2. persist the user's message FIRST ———
    // If the LLM call later fails, the user turn is still on record.
    msg_db::insert(&state.pool, conversation_id, None, "user", &body.message).await?;
    conv_db::touch(&state.pool, conversation_id).await?;

    // ——— 3. build LLM input: history + new message ———
    let history = msg_db::recent(&state.pool, conversation_id, HISTORY_LIMIT).await?;
    let messages: Vec<ChatMessage> = history
        .iter()
        .map(|m| ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    // ——— 4. insert assistant message placeholder up front ———
    // inference_logs.message_id has an FK to messages.id. The worker may
    // write the log before the stream even finishes, so the row must
    // exist first. On Done we UPDATE it with the full content.
    // If the call fails, the placeholder stays (content empty) — fine,
    // the error path is captured in inference_logs anyway.
    let assistant_message_id = Uuid::new_v4();
    msg_db::insert(&state.pool, conversation_id, Some(assistant_message_id), "assistant", "").await?;

    // ——— 5. instrumented LLM call — logging is automatic here ———
    let stream = state
        .sdk
        .complete(conversation_id, assistant_message_id, messages)
        .await
        .map_err(|e| AppError::Provider(e.to_string()))?;

    // ——— 6. pump LLM chunks → SSE events for the browser ———
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);
    let pool = state.pool.clone();

    tokio::spawn(async move {
        let mut full_response = String::new();
        let mut stream = stream;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamChunk::Token(t)) => {
                    full_response.push_str(&t);
                    let payload = json!({ "type": "token", "content": t }).to_string();
                    if tx.send(Ok(Event::default().data(payload))).await.is_err() {
                        return; // client disconnected
                    }
                }
                Ok(StreamChunk::Done { input_tokens, output_tokens }) => {
                    // fill the placeholder with the complete reply
                    if !full_response.is_empty() {
                        if let Err(e) = sqlx::query!(
                            "UPDATE messages SET content = $1 WHERE id = $2",
                            full_response,
                            assistant_message_id
                        )
                        .execute(&pool)
                        .await
                        {
                            tracing::error!(error = %e, "failed to update assistant message");
                        }
                        let _ = conv_db::touch(&pool, conversation_id).await;
                    }

                    let payload = json!({
                        "type": "done",
                        "message_id": assistant_message_id,
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                    })
                    .to_string();
                    let _ = tx.send(Ok(Event::default().data(payload))).await;
                }
                Err(e) => {
                    let payload = json!({ "type": "error", "message": e.to_string() }).to_string();
                    let _ = tx.send(Ok(Event::default().data(payload))).await;
                    return;
                }
            }
        }
    });

    Ok(Sse::new(tokio_stream::wrappers::ReceiverStream::new(rx))
        .keep_alive(KeepAlive::default()))
}
