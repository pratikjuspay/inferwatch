use sqlx::PgPool;
use uuid::Uuid;

pub struct Conversation {
    pub id: Uuid,
    pub session_id: Uuid,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create(pool: &PgPool, session_id: Uuid) -> Result<Conversation, sqlx::Error> {
    sqlx::query_as!(
        Conversation,
        r#"
        INSERT INTO conversations (session_id)
        VALUES ($1)
        RETURNING id, session_id, title, created_at, updated_at
        "#,
        session_id
    )
    .fetch_one(pool)
    .await
}

pub async fn list_by_session(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Vec<Conversation>, sqlx::Error> {
    sqlx::query_as!(
        Conversation,
        r#"
        SELECT id, session_id, title, created_at, updated_at
        FROM conversations
        WHERE session_id = $1
        ORDER BY updated_at DESC
        "#,
        session_id
    )
    .fetch_all(pool)
    .await
}

/// Fetch a conversation only if it belongs to the given session.
/// Returns None for both "missing" and "not yours" — we don't
/// leak which conversations exist.
pub async fn get_for_session(
    pool: &PgPool,
    conversation_id: Uuid,
    session_id: Uuid,
) -> Result<Option<Conversation>, sqlx::Error> {
    sqlx::query_as!(
        Conversation,
        r#"
        SELECT id, session_id, title, created_at, updated_at
        FROM conversations
        WHERE id = $1 AND session_id = $2
        "#,
        conversation_id,
        session_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn touch(pool: &PgPool, conversation_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE conversations SET updated_at = NOW() WHERE id = $1"#,
        conversation_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
