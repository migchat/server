use crate::models::Session;
use crate::db::DbPool;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use rand::Rng;
use sqlx::Row;

const TOKEN_LENGTH: usize = 32;

pub fn generate_token() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();

    (0..TOKEN_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, bcrypt::BcryptError> {
    bcrypt::verify(password, hash)
}

pub async fn get_user_id_from_token(pool: &DbPool, token: &str) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT user_id FROM sessions WHERE token = ?")
        .bind(token)
        .fetch_one(pool.as_ref())
        .await?;

    Ok(row.get("user_id"))
}

// Middleware to validate authentication token
pub async fn auth_middleware(
    State(pool): State<DbPool>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            &header[7..]
        }
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    match get_user_id_from_token(&pool, token).await {
        Ok(user_id) => {
            request.extensions_mut().insert(user_id);
            Ok(next.run(request).await)
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}
