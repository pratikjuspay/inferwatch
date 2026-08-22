use sqlx::PgPool;
use uuid::Uuid;

pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Insert a message. If `id` is Some, use it — chat handler pre-generates
/// assistant message IDs so inference_logs can reference them.
pub async fn insert(
    pool: &PgPool,
    conversation_id: Uuid,
    id: Option<Uuid>,
    role: &str,
    content: &str,
) -> Result<Message, sqlx::Error> {
    match id {
        Some(id) => {
            sqlx::query_as!(
                Message,
                r#"
                INSERT INTO messages (id, conversation_id, role, content)
                VALUES ($1, $2, $3, $4)
                RETURNING id, conversation_id, role, content, created_at
                "#,
                id,
                conversation_id,
                role,
                content
            )
            .fetch_one(pool)
            .await
        }
        None => {
            sqlx::query_as!(
                Message,
                r#"
                INSERT INTO messages (conversation_id, role, content)
                VALUES ($1, $2, $3)
                RETURNING id, conversation_id, role, content, created_at
                "#,
                conversation_id,
                role,
                content
            )
            .fetch_one(pool)
            .await
        }
    }
}

/// Load newest-first then reverse → chronological order for the LLM.
/// Limit keeps context (and cost) bounded — "short conversational context".
pub async fn recent(
    pool: &PgPool,
    conversation_id: Uuid,
    limit: i64,
) -> Result<Vec<Message>, sqlx::Error> {
    let mut rows = sqlx::query_as!(
        Message,
        r#"
        SELECT id, conversation_id, role, content, created_at
        FROM messages
        WHERE conversation_id = $1
        ORDER BY created_at DESC, id DESC
        LIMIT $2
        "#,
        conversation_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    rows.reverse();
    Ok(rows)
}
