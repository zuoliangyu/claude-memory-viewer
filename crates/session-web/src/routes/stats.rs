use axum::extract::Query;
use axum::response::Json;
use axum::http::StatusCode;
use serde::Deserialize;
use session_core::models::stats::TokenUsageSummary;

#[derive(Deserialize)]
pub struct StatsQuery {
    pub source: String,
}

pub async fn get_stats(
    Query(params): Query<StatsQuery>,
) -> Result<Json<TokenUsageSummary>, (StatusCode, String)> {
    let source = params.source;
    let result = tokio::task::spawn_blocking(move || {
        session_core::stats::get_stats(&source)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}
