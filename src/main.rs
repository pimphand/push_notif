mod auth;
mod db;
mod handlers;
mod keys;
mod push_service;
mod state;

use axum::{
    http::StatusCode,
    routing::{get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging()?;

    let state = AppState::new().await?;
    let api_protected = Router::new()
        .route("/me", get(handlers::me))
        .route("/keys", get(handlers::keys_list).post(handlers::key_create))
        .route("/keys/:id", put(handlers::key_update).delete(handlers::key_delete))
        .route("/keys/:id/regenerate", post(handlers::key_regenerate));
    let app = Router::new()
        .route("/vapid-public-key", get(handlers::vapid_public_key))
        .route("/subscribe", post(handlers::subscribe))
        .route("/notify", post(handlers::notify))
        .route("/notify/last", get(handlers::notify_last))
        .route(
            "/api/login",
            post(handlers::login).options(|| async { StatusCode::NO_CONTENT }),
        )
        .route(
            "/api/register",
            post(handlers::register).options(|| async { StatusCode::NO_CONTENT }),
        )
        .route(
            "/api/logout",
            post(handlers::logout).options(|| async { StatusCode::NO_CONTENT }),
        )
        .nest("/api", api_protected)
        .nest_service("/static", ServeDir::new("static"))
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    info!(%addr, "Web Push server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_logging() -> anyhow::Result<()> {
    let _ = std::fs::create_dir_all("logs");
    let file_appender = tracing_appender::rolling::daily("logs", "web-push.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    Ok(())
}
