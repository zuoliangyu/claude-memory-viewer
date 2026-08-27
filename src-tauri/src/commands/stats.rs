use std::path::Path;
use std::time::Instant;

use serde_json::json;
use session_core::models::stats::{
    ProjectCostEntry, RequestLogPage, SessionCostSummary, TokenUsageSummary,
};
use session_core::stats::{self, RequestLogFilter};

use super::perf;

#[tauri::command]
pub async fn get_stats(
    source: String,
    time_zone: Option<String>,
) -> Result<TokenUsageSummary, String> {
    tauri::async_runtime::spawn_blocking(move || stats::get_stats(&source, time_zone.as_deref()))
        .await
        .map_err(|error| format!("统计读取任务失败: {error}"))?
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn get_request_log(
    source: String,
    project_id: Option<String>,
    session_id: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    model: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
    time_zone: Option<String>,
) -> Result<RequestLogPage, String> {
    let filter = RequestLogFilter {
        source,
        project_id: project_id.filter(|s| !s.is_empty()),
        session_id: session_id.filter(|s| !s.is_empty()),
        start_date: start_date.filter(|s| !s.is_empty()),
        end_date: end_date.filter(|s| !s.is_empty()),
        model: model.filter(|s| !s.is_empty()),
        time_zone: time_zone.filter(|s| !s.is_empty()),
    };
    let page = page.unwrap_or(0);
    let page_size = page_size.unwrap_or(200);
    tauri::async_runtime::spawn_blocking(move || stats::get_request_log(filter, page, page_size))
        .await
        .map_err(|error| format!("请求账单读取任务失败: {error}"))?
}

#[tauri::command]
pub async fn get_project_costs(source: String) -> Result<Vec<ProjectCostEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || stats::get_project_costs(&source))
        .await
        .map_err(|error| format!("项目账单读取任务失败: {error}"))?
}

#[tauri::command]
pub async fn get_session_cost(
    source: String,
    file_path: String,
) -> Result<SessionCostSummary, String> {
    let file_name = Path::new(&file_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();
    let source_for_task = source.clone();
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        stats::get_session_cost(&source_for_task, &file_path)
    })
    .await
    .map_err(|error| format!("会话账单读取任务失败: {error}"))?;

    if perf::enabled() {
        let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
        match &result {
            Ok(summary) => perf::emit_backend(
                "stats.session_cost_backend",
                duration_ms,
                json!({
                    "source": source,
                    "fileName": file_name,
                    "requests": summary.request_count,
                }),
            ),
            Err(error) => perf::emit_backend(
                "stats.session_cost_backend_error",
                duration_ms,
                json!({
                    "source": source,
                    "fileName": file_name,
                    "errorType": if error.is_empty() { "empty" } else { "read" },
                }),
            ),
        }
    }

    result
}
