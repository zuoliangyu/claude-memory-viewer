use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use session_core::provider_sync::{
    self, CloneResult, ProviderSyncStatus, RestoreOptions, RestoreResult, SyncResult,
};

pub async fn get_status() -> Result<Json<ProviderSyncStatus>, (StatusCode, String)> {
    tokio::task::spawn_blocking(provider_sync::get_status)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncBody {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default = "default_keep")]
    pub keep: usize,
}

fn default_keep() -> usize {
    5
}

pub async fn sync(Json(body): Json<SyncBody>) -> Result<Json<SyncResult>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || provider_sync::run_sync(body.provider, body.keep))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBody {
    pub provider: String,
    #[serde(default = "default_keep")]
    pub keep: usize,
}

pub async fn switch(
    Json(body): Json<SwitchBody>,
) -> Result<Json<SyncResult>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || provider_sync::run_switch(body.provider, body.keep))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneBody {
    pub file_paths: Vec<String>,
    pub provider: String,
    #[serde(default = "default_keep")]
    pub keep: usize,
}

pub async fn clone(Json(body): Json<CloneBody>) -> Result<Json<CloneResult>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || {
        provider_sync::run_clone(body.file_paths, body.provider, body.keep)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map(Json)
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBody {
    pub backup_dir: String,
    #[serde(default)]
    pub options: Option<RestoreOptions>,
}

pub async fn restore(
    Json(body): Json<RestoreBody>,
) -> Result<Json<RestoreResult>, (StatusCode, String)> {
    let opts = body.options.unwrap_or(RestoreOptions {
        include_config: true,
        include_db: true,
        include_sessions: true,
        include_global_state: true,
    });
    tokio::task::spawn_blocking(move || provider_sync::run_restore(&body.backup_dir, opts))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
pub struct PruneQuery {
    #[serde(default = "default_keep")]
    pub keep: usize,
}

pub async fn prune_backups(Query(q): Query<PruneQuery>) -> Result<Json<u32>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || provider_sync::run_prune_backups(q.keep))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}
