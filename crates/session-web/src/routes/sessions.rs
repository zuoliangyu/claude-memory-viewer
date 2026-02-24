use axum::extract::Query;
use axum::response::Json;
use axum::http::StatusCode;
use serde::Deserialize;
use session_core::models::session::SessionIndexEntry;
use session_core::provider::{claude, codex};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsQuery {
    pub source: String,
    pub project_id: String,
}

pub async fn get_sessions(
    Query(params): Query<SessionsQuery>,
) -> Result<Json<Vec<SessionIndexEntry>>, (StatusCode, String)> {
    let source = params.source;
    let project_id = params.project_id;
    let result = tokio::task::spawn_blocking(move || match source.as_str() {
        "claude" => claude::get_sessions(&project_id),
        "codex" => codex::get_sessions(&project_id),
        _ => Err(format!("Unknown source: {}", source)),
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteQuery {
    pub file_path: String,
}

pub async fn delete_session(
    Query(params): Query<DeleteQuery>,
) -> Result<Json<()>, (StatusCode, String)> {
    let file_path = params.file_path;
    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&file_path);
        if !path.exists() {
            return Err(format!("File not found: {}", file_path));
        }
        std::fs::remove_file(path).map_err(|e| format!("Failed to delete session: {}", e))
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(()))
}
