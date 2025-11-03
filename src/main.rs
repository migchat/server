mod auth;
mod db;
mod handlers;
mod models;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "migchat_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize database
    let pool = db::init_db().await.expect("Failed to initialize database");
    tracing::info!("Database initialized successfully");

    // Setup CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build our application with routes
    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/api/account/create", post(handlers::create_account))
        .route(
            "/api/messages/send",
            post(handlers::send_message).route_layer(middleware::from_fn_with_state(
                pool.clone(),
                auth::auth_middleware,
            )),
        )
        .route(
            "/api/messages",
            get(handlers::get_messages).route_layer(middleware::from_fn_with_state(
                pool.clone(),
                auth::auth_middleware,
            )),
        )
        .route(
            "/api/conversations",
            get(handlers::get_conversations).route_layer(middleware::from_fn_with_state(
                pool.clone(),
                auth::auth_middleware,
            )),
        )
        .layer(cors)
        .with_state(pool);

    // Get port from environment variable or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}
