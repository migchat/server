use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::sync::Arc;

pub type DbPool = Arc<SqlitePool>;

pub async fn init_db() -> Result<DbPool, sqlx::Error> {
    // Use file-based SQLite for persistence across restarts
    // Create data directory if it doesn't exist
    std::fs::create_dir_all("/data").unwrap_or_else(|_| {
        // If /data is not writable (local dev), use current directory
        std::fs::create_dir_all("./data").ok();
    });

    // Try /data first (for production), fall back to ./data (for local dev)
    let database_url = if std::path::Path::new("/data").exists() {
        "sqlite:///data/migchat.db"
    } else {
        "sqlite://./data/migchat.db"
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Create tables
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_user_id INTEGER NOT NULL,
            to_user_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (from_user_id) REFERENCES users(id),
            FOREIGN KEY (to_user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create indexes for better query performance
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token)")
        .execute(&pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_to_user ON messages(to_user_id)")
        .execute(&pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_from_user ON messages(from_user_id)")
        .execute(&pool)
        .await?;

    Ok(Arc::new(pool))
}
