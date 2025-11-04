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
            COUNT(CASE WHEN m.to_user_id = ? AND m.from_user_id != ? AND m.read_at IS NULL THEN 1 END) as unread_count
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

pub async fn update_username(
    State(pool): State<DbPool>,
    Extension(user_id): Extension<i64>,
    Json(payload): Json<UpdateUsernameRequest>,
) -> Result<Json<UpdateUsernameResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate username
    if payload.new_username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Username cannot be empty".to_string(),
            }),
        ));
    }

    // Check if username already exists (for a different user)
    let existing_user = sqlx::query("SELECT id FROM users WHERE username = ? AND id != ?")
        .bind(&payload.new_username)
        .bind(user_id)
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

    // Update the username
    let updated_at = Utc::now();
    sqlx::query("UPDATE users SET username = ? WHERE id = ?")
        .bind(&payload.new_username)
        .bind(user_id)
        .execute(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to update username: {}", e),
                }),
            )
        })?;

    Ok(Json(UpdateUsernameResponse {
        username: payload.new_username,
        updated_at,
    }))
}

pub async fn get_filtered_messages(
    State(pool): State<DbPool>,
    Extension(user_id): Extension<i64>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<MessageResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let with_user = params.get("with_user");

    if let Some(username) = with_user {
        // Get the other user's ID
        let other_user = sqlx::query("SELECT id FROM users WHERE username = ?")
            .bind(username)
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

        let other_user_id: i64 = match other_user {
            Some(row) => row.get("id"),
            None => {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "User not found".to_string(),
                    }),
                ))
            }
        };

        // Get messages between the two users
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
            WHERE (m.from_user_id = ? AND m.to_user_id = ?)
               OR (m.from_user_id = ? AND m.to_user_id = ?)
            ORDER BY m.created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(other_user_id)
        .bind(other_user_id)
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
    } else {
        // No filter, return all messages (same as get_messages)
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
}

pub async fn mark_messages_read(
    State(pool): State<DbPool>,
    Extension(user_id): Extension<i64>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let with_user = params.get("with_user");

    if let Some(username) = with_user {
        // Get the other user's ID
        let other_user = sqlx::query("SELECT id FROM users WHERE username = ?")
            .bind(username)
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

        let other_user_id: i64 = match other_user {
            Some(row) => row.get("id"),
            None => {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "User not found".to_string(),
                    }),
                ))
            }
        };

        // Mark all messages from other_user to current user as read
        let read_at = Utc::now();
        let result = sqlx::query(
            "UPDATE messages SET read_at = ? WHERE from_user_id = ? AND to_user_id = ? AND read_at IS NULL"
        )
        .bind(read_at.to_rfc3339())
        .bind(other_user_id)
        .bind(user_id)
        .execute(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to mark messages as read: {}", e),
                }),
            )
        })?;

        Ok(Json(serde_json::json!({
            "marked_read": result.rows_affected()
        })))
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "with_user parameter is required".to_string(),
            }),
        ))
    }
}

// E2E Encryption endpoints
pub async fn upload_keys(
    State(pool): State<DbPool>,
    Extension(user_id): Extension<i64>,
    Json(payload): Json<UploadKeysRequest>,
) -> Result<Json<UploadKeysResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if user already has keys
    let existing_keys = sqlx::query("SELECT user_id FROM user_keys WHERE user_id = ?")
        .bind(user_id)
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

    if existing_keys.is_some() {
        // Update existing keys
        sqlx::query(
            "UPDATE user_keys SET identity_key = ?, signed_prekey = ?, signed_prekey_signature = ? WHERE user_id = ?",
        )
        .bind(&payload.key_bundle.identity_key)
        .bind(&payload.key_bundle.signed_prekey)
        .bind(&payload.key_bundle.signed_prekey_signature)
        .bind(user_id)
        .execute(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to update keys: {}", e),
                }),
            )
        })?;

        // Delete old one-time prekeys
        sqlx::query("DELETE FROM one_time_prekeys WHERE user_id = ?")
            .bind(user_id)
            .execute(pool.as_ref())
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to delete old prekeys: {}", e),
                    }),
                )
            })?;
    } else {
        // Insert new keys
        sqlx::query(
            "INSERT INTO user_keys (user_id, identity_key, signed_prekey, signed_prekey_signature, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(&payload.key_bundle.identity_key)
        .bind(&payload.key_bundle.signed_prekey)
        .bind(&payload.key_bundle.signed_prekey_signature)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to insert keys: {}", e),
                }),
            )
        })?;
    }

    // Insert one-time prekeys
    for (i, prekey) in payload.key_bundle.one_time_prekeys.iter().enumerate() {
        sqlx::query(
            "INSERT INTO one_time_prekeys (user_id, key_id, public_key, used, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(i as i64)
        .bind(prekey)
        .bind(false)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.as_ref())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to insert one-time prekey: {}", e),
                }),
            )
        })?;
    }

    Ok(Json(UploadKeysResponse { success: true }))
}

pub async fn get_keys(
    State(pool): State<DbPool>,
    axum::extract::Path(username): axum::extract::Path<String>,
) -> Result<Json<GetKeysResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Get user ID from username
    let user = sqlx::query("SELECT id FROM users WHERE username = ?")
        .bind(&username)
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

    let user_id: i64 = match user {
        Some(row) => row.get("id"),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "User not found".to_string(),
                }),
            ))
        }
    };

    // Get user keys
    let keys_row = sqlx::query("SELECT user_id, identity_key, signed_prekey, signed_prekey_signature, created_at FROM user_keys WHERE user_id = ?")
        .bind(user_id)
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

    let keys = match keys_row {
        Some(row) => UserKey {
            user_id: row.get("user_id"),
            identity_key: row.get("identity_key"),
            signed_prekey: row.get("signed_prekey"),
            signed_prekey_signature: row.get("signed_prekey_signature"),
            created_at: row.get("created_at"),
        },
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Keys not found for this user".to_string(),
                }),
            ))
        }
    };

    // Get unused one-time prekeys (limit to 10)
    let prekeys_rows = sqlx::query(
        "SELECT id, user_id, key_id, public_key, used, created_at FROM one_time_prekeys WHERE user_id = ? AND used = ? LIMIT 10",
    )
    .bind(user_id)
    .bind(false)
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

    let one_time_prekeys: Vec<String> = prekeys_rows
        .iter()
        .map(|row| row.get::<String, _>("public_key"))
        .collect();

    // Mark the first one-time prekey as used (X3DH protocol requirement)
    if !prekeys_rows.is_empty() {
        let first_prekey_id: i64 = prekeys_rows[0].get("id");
        let _ = sqlx::query("UPDATE one_time_prekeys SET used = ? WHERE id = ?")
            .bind(true)
            .bind(first_prekey_id)
            .execute(pool.as_ref())
            .await;
        // Note: We don't fail if this update fails, just log it
    }

    Ok(Json(GetKeysResponse {
        key_bundle: KeyBundle {
            identity_key: keys.identity_key,
            signed_prekey: keys.signed_prekey,
            signed_prekey_signature: keys.signed_prekey_signature,
            one_time_prekeys,
        },
    }))
}
