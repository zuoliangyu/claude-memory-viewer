use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use session_core::models::stats::{
    ProjectCostEntry, RequestLogPage, SessionCostSummary, TokenUsageSummary,
};
use session_core::stats::{self, RequestLogFilter};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    pub source: String,
    #[serde(default)]
    pub time_zone: Option<String>,
}

pub async fn get_stats(
    Query(params): Query<StatsQuery>,
) -> Result<Json<TokenUsageSummary>, (StatusCode, String)> {
    let StatsQuery { source, time_zone } = params;
    let result =
        tokio::task::spawn_blocking(move || stats::get_stats(&source, time_zone.as_deref()))
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogQuery {
    pub source: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub time_zone: Option<String>,
}

pub async fn get_request_log(
    Query(params): Query<RequestLogQuery>,
) -> Result<Json<RequestLogPage>, (StatusCode, String)> {
    let filter = RequestLogFilter {
        source: params.source,
        project_id: params.project_id.filter(|s| !s.is_empty()),
        session_id: params.session_id.filter(|s| !s.is_empty()),
        start_date: params.start_date.filter(|s| !s.is_empty()),
        end_date: params.end_date.filter(|s| !s.is_empty()),
        model: params.model.filter(|s| !s.is_empty()),
        time_zone: params.time_zone.filter(|s| !s.is_empty()),
    };
    let page = params.page.unwrap_or(0);
    let page_size = params.page_size.unwrap_or(200);

    let result =
        tokio::task::spawn_blocking(move || stats::get_request_log(filter, page, page_size))
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(result))
}

pub async fn get_project_costs(
    Query(params): Query<StatsQuery>,
) -> Result<Json<Vec<ProjectCostEntry>>, (StatusCode, String)> {
    let source = params.source;
    let result = tokio::task::spawn_blocking(move || stats::get_project_costs(&source))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCostQuery {
    pub source: String,
    pub file_path: String,
}

pub async fn get_session_cost(
    Query(params): Query<SessionCostQuery>,
) -> Result<Json<SessionCostSummary>, (StatusCode, String)> {
    let SessionCostQuery { source, file_path } = params;
    let result = tokio::task::spawn_blocking(move || stats::get_session_cost(&source, &file_path))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(result))
}
