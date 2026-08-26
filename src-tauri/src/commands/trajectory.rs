use std::time::Instant;

use serde_json::json;
use session_core::models::trajectory::Trajectory;
use session_core::paths::validate_session_file;
use session_core::provider::codex_trajectory;

use super::perf;

#[tauri::command]
pub async fn get_trajectory(
    source: String,
    file_path: String,
    max_records: Option<usize>,
    before_record: Option<usize>,
    fast: Option<bool>,
) -> Result<Trajectory, String> {
    if source != "codex" {
        return Err("轨迹视图目前只支持 Codex 数据源".to_string());
    }
    let path = validate_session_file(&source, &file_path)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();
    let fast = fast.unwrap_or(false);
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        if fast {
            codex_trajectory::parse_fast_page(&path, max_records)
        } else {
            codex_trajectory::parse_page(&path, max_records, before_record)
        }
    })
    .await
    .map_err(|error| format!("轨迹解析任务失败: {error}"))?;

    if perf::enabled() {
        let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
        match &result {
            Ok(trajectory) => perf::emit_backend(
                "trajectory.backend_parse",
                duration_ms,
                json!({
                    "fileName": file_name,
                    "fast": fast,
                    "maxRecords": max_records,
                    "beforeRecord": before_record,
                    "records": trajectory.records.len(),
                    "turns": trajectory.turns.len(),
                    "totalRecords": trajectory.stats.records,
                    "complete": trajectory.pagination.complete,
                }),
            ),
            Err(error) => perf::emit_backend(
                "trajectory.backend_error",
                duration_ms,
                json!({
                    "fileName": file_name,
                    "fast": fast,
                    "maxRecords": max_records,
                    "beforeRecord": before_record,
                    "errorType": if error.is_empty() { "empty" } else { "parse" },
                }),
            ),
        }
    }

    result
}
