use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use session_core::recyclebin::{self, RecycledItem};

pub async fn list_items() -> Json<Vec<RecycledItem>> {
    Json(
        tokio::task::spawn_blocking(recyclebin::list_items)
            .await
            .unwrap_or_default(),
    )
}

pub async fn restore_item(Path(id): Path<String>) -> Result<Json<()>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || recyclebin::restore_item(&id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

pub async fn permanently_delete_item(
    Path(id): Path<String>,
) -> Result<Json<()>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || recyclebin::permanently_delete_item(&id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

#[derive(Serialize)]
pub struct EmptyResult {
    pub deleted: usize,
}

pub async fn empty_recyclebin() -> Result<Json<EmptyResult>, (StatusCode, String)> {
    let deleted = tokio::task::spawn_blocking(recyclebin::empty_recyclebin)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(EmptyResult { deleted }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupQuery {
    pub source: String,
}

pub async fn cleanup_orphan_dirs(
    Query(params): Query<CleanupQuery>,
) -> Result<Json<EmptyResult>, (StatusCode, String)> {
    let source = params.source;
    let count = tokio::task::spawn_blocking(move || match source.as_str() {
        "claude" => session_core::provider::claude::cleanup_all_orphan_dirs(),
        _ => Err(format!(
            "Orphan dir cleanup not supported for source: {}",
            source
        )),
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(EmptyResult { deleted: count }))
}
