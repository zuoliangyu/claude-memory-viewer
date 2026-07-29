use session_core::models::project::ProjectEntry;
use session_core::provider::{claude, codex, grok};
use session_core::provider::claude::{DeleteLevel, DeleteResult};

#[tauri::command]
pub fn get_projects(source: String) -> Result<Vec<ProjectEntry>, String> {
    match source.as_str() {
        "claude" => claude::get_projects(),
        "codex" => codex::get_projects(),
        "grok" => grok::get_projects(),
        _ => Err(format!("Unknown source: {}", source)),
    }
}

#[tauri::command]
pub fn refresh_projects_cache(source: String) -> Result<Vec<ProjectEntry>, String> {
    match source.as_str() {
        "claude" => claude::refresh_projects_cache(),
        "codex" => codex::get_projects(),
        "grok" => grok::get_projects(),
        _ => Err(format!("Unknown source: {}", source)),
    }
}

#[tauri::command]
pub fn rebuild_projects_cache(source: String) -> Result<Vec<ProjectEntry>, String> {
    match source.as_str() {
        "claude" => claude::refresh_projects_cache(),
        "codex" => codex::rebuild_projects_cache(),
        "grok" => grok::rebuild_projects_cache(),
        _ => Err(format!("Unknown source: {}", source)),
    }
}

#[tauri::command]
pub fn delete_project(
    source: String,
    project_id: String,
    level: DeleteLevel,
) -> Result<DeleteResult, String> {
    match source.as_str() {
        "claude" => claude::delete_project(&project_id, level),
        "codex" => codex::delete_project(&project_id),
        "grok" => grok::delete_project(&project_id),
        _ => Err(format!("Delete project not supported for source: {}", source)),
    }
}

#[tauri::command]
pub fn set_project_alias(
    source: String,
    project_id: String,
    alias: Option<String>,
) -> Result<(), String> {
    match source.as_str() {
        "claude" => claude::set_project_alias(&project_id, alias),
        _ => Err(format!("set_project_alias not supported for source: {}", source)),
    }
}
