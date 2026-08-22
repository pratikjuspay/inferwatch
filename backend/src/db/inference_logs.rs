use crate::sdk::LogEvent;
use sqlx::PgPool;

/// Called ONLY by the ingestion worker — never from a request path.
pub async fn insert(pool: &PgPool, event: &LogEvent) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO inference_logs (
            conversation_id, message_id, model, provider,
            latency_ms, input_tokens, output_tokens,
            status, error_msg, input_preview, output_preview
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
        event.conversation_id,
        event.message_id,
        event.model,
        event.provider,
        event.latency_ms,
        event.input_tokens,
        event.output_tokens,
        event.status,
        event.error_msg,
        event.input_preview,
        event.output_preview
    )
    .execute(pool)
    .await?;
    Ok(())
}
