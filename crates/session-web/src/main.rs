mod chat_ws;
mod config;
mod routes;
mod static_files;
mod ws;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post, put},
    Json, Router,
};
use futures_util::StreamExt;
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

async fn detect_cli_handler() -> Json<Vec<session_core::cli::CliInstallation>> {
    Json(session_core::cli::discover_installations())
}

#[derive(serde::Deserialize)]
struct CliConfigQuery {
    source: String,
}

async fn cli_config_handler(
    axum::extract::Query(query): axum::extract::Query<CliConfigQuery>,
) -> Result<Json<session_core::cli_config::CliConfig>, (StatusCode, String)> {
    session_core::cli_config::read_cli_config(&query.source)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuickChatRequest {
    source: String,
    messages: Vec<session_core::quick_chat::ChatMsg>,
    model: String,
}

async fn quick_chat_handler(
    Json(req): Json<QuickChatRequest>,
) -> axum::response::Sse<impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>
{
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);

    tokio::spawn(async move {
        let result = session_core::quick_chat::stream_chat(
            &req.source,
            req.messages,
            &req.model,
            |chunk| {
                let _ = tx.try_send(chunk.to_string());
            },
        )
        .await;

        if let Err(e) = result {
            let err_json = serde_json::json!({ "error": e }).to_string();
            let _ = tx.try_send(format!("[ERROR]{}", err_json));
        }
        // Send done marker
        let _ = tx.send("[DONE]".to_string()).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|chunk| {
        if chunk == "[DONE]" {
            Ok(axum::response::sse::Event::default().data("[DONE]"))
        } else if let Some(err) = chunk.strip_prefix("[ERROR]") {
            Ok(axum::response::sse::Event::default()
                .event("error")
                .data(err))
        } else {
            Ok(axum::response::sse::Event::default().data(chunk))
        }
    });

    axum::response::Sse::new(stream)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListModelsRequest {
    source: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    base_url: String,
}

async fn list_models_handler(
    Json(req): Json<ListModelsRequest>,
) -> Result<Json<Vec<session_core::model_list::ModelInfo>>, (StatusCode, String)> {
    session_core::model_list::list_models(&req.source, &req.api_key, &req.base_url)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
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
        .route(
            "/api/sessions/meta",
            put(routes::sessions::update_session_meta),
        )
        .route("/api/tags", get(routes::sessions::get_all_tags))
        .route("/api/cross-tags", get(routes::sessions::get_cross_project_tags))
        .route("/api/messages", get(routes::messages::get_messages))
        .route("/api/search", get(routes::search::global_search))
        .route("/api/stats", get(routes::stats::get_stats))
        .layer(middleware::from_fn(check_auth));

    // WebSocket route (with auth via query param or header)
    let ws_routes = Router::new()
        .route("/ws", get(ws::ws_handler))
        .with_state(Arc::clone(&fs_tx));

    // Chat WebSocket route (no state needed, stateless per connection)
    let chat_ws_routes = Router::new()
        .route("/ws/chat", get(chat_ws::chat_ws_handler));

    // CLI detection + models + config route (with auth)
    let cli_routes = Router::new()
        .route("/api/cli/detect", get(detect_cli_handler))
        .route("/api/cli/config", get(cli_config_handler))
        .route("/api/models", post(list_models_handler))
        .route("/api/quick-chat", post(quick_chat_handler))
        .layer(middleware::from_fn(check_auth));

    // Static file fallback (no auth needed)
    let static_routes = Router::new().fallback(static_files::static_handler);

    let app = Router::new()
        .merge(api_routes)
        .merge(cli_routes)
        .merge(ws_routes)
        .merge(chat_ws_routes)
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
