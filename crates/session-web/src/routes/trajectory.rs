use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use session_core::models::trajectory::Trajectory;
use session_core::provider::codex_trajectory;

use crate::resolve_session_file_path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryQuery {
    pub source: String,
    pub file_path: String,
    pub max_records: Option<usize>,
    pub before_record: Option<usize>,
    pub fast: Option<bool>,
}

pub async fn get_trajectory(
    Query(params): Query<TrajectoryQuery>,
) -> Result<Json<Trajectory>, (StatusCode, String)> {
    if params.source != "codex" {
        return Err((
            StatusCode::BAD_REQUEST,
            "轨迹视图目前只支持 Codex 数据源".to_string(),
        ));
    }
    let resolved_path = resolve_session_file_path(&params.source, &params.file_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let result = tokio::task::spawn_blocking(move || {
        if params.fast.unwrap_or(false) {
            codex_trajectory::parse_fast_page(&resolved_path, params.max_records)
        } else {
            codex_trajectory::parse_page(&resolved_path, params.max_records, params.before_record)
        }
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(result))
}
