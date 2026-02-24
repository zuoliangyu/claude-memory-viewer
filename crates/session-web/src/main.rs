mod config;
mod routes;
mod static_files;
mod ws;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{delete, get},
    Router,
};
use clap::Parser;
use config::Config;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppToken(Option<String>);

/// Auth check middleware — reads token from AppToken extension
async fn check_auth(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = request
        .extensions()
        .get::<AppToken>()
        .cloned()
        .unwrap_or(AppToken(None));

    if let Some(token) = expected.0 {
        let auth_header = request
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let provided = &header[7..];
                if provided != token {
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }

    Ok(next.run(request).await)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::parse();

    // Start file watcher
    let fs_tx = ws::start_file_watcher();

    let app_token = AppToken(config.token.clone());

    // API routes (with auth middleware)
    let api_routes = Router::new()
        .route("/api/projects", get(routes::projects::get_projects))
        .route("/api/sessions", get(routes::sessions::get_sessions))
        .route("/api/sessions", delete(routes::sessions::delete_session))
        .route("/api/messages", get(routes::messages::get_messages))
        .route("/api/search", get(routes::search::global_search))
        .route("/api/stats", get(routes::stats::get_stats))
        .layer(middleware::from_fn(check_auth));

    // WebSocket route (with auth via query param or header)
    let ws_routes = Router::new()
        .route("/ws", get(ws::ws_handler))
        .with_state(Arc::clone(&fs_tx));

    // Static file fallback (no auth needed)
    let static_routes = Router::new().fallback(static_files::static_handler);

    let app = Router::new()
        .merge(api_routes)
        .merge(ws_routes)
        .merge(static_routes)
        .layer(CorsLayer::permissive())
        .layer(axum::Extension(app_token));

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("AI Session Viewer Web Server listening on http://{}", addr);
    if config.token.is_some() {
        tracing::info!("Authentication enabled (Bearer token required)");
    } else {
        tracing::info!("No authentication (set --token or ASV_TOKEN to enable)");
    }

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
