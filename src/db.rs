use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::sync::Arc;

pub type DbPool = Arc<SqlitePool>;

pub async fn init_db() -> Result<DbPool, sqlx::Error> {
    // Use file-based SQLite for persistence across restarts
    // Determine database path based on environment
    let database_url = if std::path::Path::new("/data").exists() {
        // Production: use /data mounted volume with create_if_missing option
        "sqlite:/data/migchat.db?mode=rwc"
    } else {
        // Local dev: use ./data directory
        std::fs::create_dir_all("./data").ok();
        "sqlite:./data/migchat.db?mode=rwc"
    };

    eprintln!("Connecting to database: {}", database_url);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    eprintln!("Database connected successfully");

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
