use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use session_core::search::SearchResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub source: String,
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default)]
    pub scope: Option<String>,
}

fn default_max_results() -> usize {
    50
}

pub async fn global_search(
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, (StatusCode, String)> {
    let source = params.source;
    let query = params.query;
    let max_results = params.max_results;
    let scope = params.scope.unwrap_or_else(|| "all".to_string());

    let result = tokio::task::spawn_blocking(move || {
        session_core::search::global_search(
            &source,
            &query,
            max_results,
            session_core::search::SearchScope::from_query(&scope),
        )
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}
