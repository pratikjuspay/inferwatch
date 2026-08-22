use crate::db::{conversations as conv_db, messages as msg_db};
use crate::errors::AppError;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SessionQuery {
    pub session_id: Uuid,
}

#[derive(Deserialize)]
pub struct CreateConversationRequest {
    pub session_id: Uuid,
}

#[derive(Serialize)]
pub struct ConversationDto {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<conv_db::Conversation> for ConversationDto {
    fn from(c: conv_db::Conversation) -> Self {
        Self {
            id: c.id,
            title: c.title,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

#[derive(Serialize)]
pub struct MessageDto {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl From<msg_db::Message> for MessageDto {
    fn from(m: msg_db::Message) -> Self {
        Self {
            id: m.id,
            role: m.role,
            content: m.content,
            created_at: m.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct ConversationDetailDto {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<MessageDto>,
}

pub async fn create_conversation(
    State(state): State<AppState>,
    Json(body): Json<CreateConversationRequest>,
) -> Result<Json<ConversationDto>, AppError> {
    let conversation = conv_db::create(&state.pool, body.session_id).await?;
    Ok(Json(conversation.into()))
}

pub async fn list_conversations(
    State(state): State<AppState>,
    Query(q): Query<SessionQuery>,
) -> Result<Json<Vec<ConversationDto>>, AppError> {
    let rows = conv_db::list_by_session(&state.pool, q.session_id).await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

/// Resume: conversation + its full message history in one call.
pub async fn get_conversation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<SessionQuery>,
) -> Result<Json<ConversationDetailDto>, AppError> {
    let conversation = conv_db::get_for_session(&state.pool, id, q.session_id)
        .await?
        .ok_or_else(|| AppError::NotFound("conversation".into()))?;

    let messages = msg_db::recent(&state.pool, id, 200)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(Json(ConversationDetailDto {
        id: conversation.id,
        title: conversation.title,
        created_at: conversation.created_at,
        updated_at: conversation.updated_at,
        messages,
    }))
}
