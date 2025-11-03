use crate::auth::{generate_token, hash_password};
use crate::db::DbPool;
use crate::models::*;
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use sqlx::Row;

pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn create_account(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<Json<CreateAccountResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate username
    if payload.username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Username cannot be empty".to_string(),
            }),
        ));
    }

    if payload.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Password cannot be empty".to_string(),
            }),
        ));
    }

    // Check if username already exists
    let existing_user = sqlx::query("SELECT id FROM users WHERE username = ?")
        .bind(&payload.username)
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    if existing_user.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Username already exists".to_string(),
            }),
        ));
    }

    // Hash password
    let password_hash = hash_password(&payload.password).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Password hashing error: {}", e),
            }),
        )
    })?;

    // Create user
    let result = sqlx::query(
        "INSERT INTO users (username, password_hash, created_at) VALUES (?, ?, ?)",
    )
    .bind(&payload.username)
    .bind(&password_hash)
    .bind(Utc::now().to_rfc3339())
    .execute(pool.as_ref())
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create user: {}", e),
            }),
        )
    })?;

    let user_id = result.last_insert_rowid();

    // Generate token
    let token = generate_token();

    // Create session
    sqlx::query("INSERT INTO sessions (user_id, token, created_at) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(&token)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create session: {}", e),
                }),
            )
        })?;

    Ok(Json(CreateAccountResponse {
        token,
        user_id,
        username: payload.username,
    }))
}

pub async fn send_message(
    State(pool): State<DbPool>,
    Extension(user_id): Extension<i64>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, (StatusCode, Json<ErrorResponse>)> {
    if payload.content.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Message content cannot be empty".to_string(),
            }),
        ));
    }

    // Find recipient user by username
    let recipient = sqlx::query("SELECT id FROM users WHERE username = ?")
        .bind(&payload.to_username)
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Database error: {}", e),
                }),
            )
        })?;

    let recipient_id: i64 = match recipient {
        Some(row) => row.get("id"),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipient user not found".to_string(),
                }),
            ))
        }
    };

    // Insert message
    let created_at = Utc::now();
    let result = sqlx::query(
        "INSERT INTO messages (from_user_id, to_user_id, content, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(recipient_id)
    .bind(&payload.content)
    .bind(created_at.to_rfc3339())
    .execute(pool.as_ref())
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to send message: {}", e),
            }),
        )
    })?;

    let message_id = result.last_insert_rowid();

    Ok(Json(SendMessageResponse {
        message_id,
        created_at,
    }))
}

pub async fn get_messages(
    State(pool): State<DbPool>,
    Extension(user_id): Extension<i64>,
) -> Result<Json<Vec<MessageResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let rows = sqlx::query(
        r#"
        SELECT
            m.id,
            m.content,
            m.created_at,
            from_user.username as from_username,
            to_user.username as to_username
        FROM messages m
        JOIN users from_user ON m.from_user_id = from_user.id
        JOIN users to_user ON m.to_user_id = to_user.id
        WHERE m.to_user_id = ? OR m.from_user_id = ?
        ORDER BY m.created_at DESC
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let messages: Vec<MessageResponse> = rows
        .iter()
        .map(|row| {
            let created_at_str: String = row.get("created_at");
            MessageResponse {
                id: row.get("id"),
                from_username: row.get("from_username"),
                to_username: row.get("to_username"),
                content: row.get("content"),
                created_at: created_at_str.parse().unwrap_or(Utc::now()),
            }
        })
        .collect();

    Ok(Json(messages))
}

pub async fn get_conversations(
    State(pool): State<DbPool>,
    Extension(user_id): Extension<i64>,
) -> Result<Json<Vec<ConversationResponse>>, (StatusCode, Json<ErrorResponse>)> {
    // Get all conversations with latest message info
    let rows = sqlx::query(
        r#"
        SELECT
            CASE
                WHEN m.from_user_id = ? THEN to_user.username
                ELSE from_user.username
            END as other_username,
            m.content as last_message,
            m.created_at as last_message_time,
            COUNT(CASE WHEN m.to_user_id = ? AND m.from_user_id != ? THEN 1 END) as unread_count
        FROM messages m
        JOIN users from_user ON m.from_user_id = from_user.id
        JOIN users to_user ON m.to_user_id = to_user.id
        WHERE m.from_user_id = ? OR m.to_user_id = ?
        GROUP BY other_username
        ORDER BY m.created_at DESC
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let conversations: Vec<ConversationResponse> = rows
        .iter()
        .map(|row| {
            let last_message_time_str: String = row.get("last_message_time");
            ConversationResponse {
                username: row.get("other_username"),
                last_message: row.get("last_message"),
                last_message_time: last_message_time_str.parse().unwrap_or(Utc::now()),
                unread_count: row.get("unread_count"),
            }
        })
        .collect();

    Ok(Json(conversations))
}
