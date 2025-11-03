use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Message {
    pub id: i64,
    pub from_user_id: i64,
    pub to_user_id: i64,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountResponse {
    pub token: String,
    pub user_id: i64,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub to_username: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageResponse {
    pub id: i64,
    pub from_username: String,
    pub to_username: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationResponse {
    pub username: String,
    pub last_message: String,
    pub last_message_time: DateTime<Utc>,
    pub unread_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUsernameRequest {
    pub new_username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUsernameResponse {
    pub username: String,
    pub updated_at: DateTime<Utc>,
}
