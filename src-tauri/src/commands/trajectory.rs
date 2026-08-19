use session_core::models::trajectory::Trajectory;
use session_core::paths::validate_session_file;
use session_core::provider::codex_trajectory;

#[tauri::command]
pub async fn get_trajectory(
    source: String,
    file_path: String,
    max_records: Option<usize>,
    before_record: Option<usize>,
) -> Result<Trajectory, String> {
    if source != "codex" {
        return Err("轨迹视图目前只支持 Codex 数据源".to_string());
    }
    let path = validate_session_file(&source, &file_path)?;
    tauri::async_runtime::spawn_blocking(move || {
        codex_trajectory::parse_page(&path, max_records, before_record)
    })
    .await
    .map_err(|error| format!("轨迹解析任务失败: {error}"))?
}
