use axum::extract::Query;
use axum::response::Json;
use axum::http::StatusCode;
use serde::Deserialize;
use session_core::models::project::ProjectEntry;
use session_core::provider::{claude, codex};

#[derive(Deserialize)]
pub struct ProjectsQuery {
    pub source: String,
}

pub async fn get_projects(
    Query(params): Query<ProjectsQuery>,
) -> Result<Json<Vec<ProjectEntry>>, (StatusCode, String)> {
    let source = params.source;
    let result = tokio::task::spawn_blocking(move || match source.as_str() {
        "claude" => claude::get_projects(),
        "codex" => codex::get_projects(),
        _ => Err(format!("Unknown source: {}", source)),
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}
