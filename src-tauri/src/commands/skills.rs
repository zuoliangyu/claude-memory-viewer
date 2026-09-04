use session_core::models::skill::{ImportResult, SkillsResult};
use session_core::skills;

#[tauri::command]
pub fn list_skills(project_path: Option<String>) -> Result<SkillsResult, String> {
    skills::scan_skills(project_path)
}

#[tauri::command]
pub fn get_skill_content(path: String) -> Result<String, String> {
    skills::read_skill_content(&path)
}

#[tauri::command]
pub fn delete_skill(
    scope: String,
    project_path: Option<String>,
    slug: String,
) -> Result<(), String> {
    skills::delete_skill(&scope, project_path.as_deref(), &slug)
}

#[tauri::command]
pub fn import_skills(
    archive_path: String,
    scope: String,
    project_path: Option<String>,
    overwrite: bool,
) -> Result<ImportResult, String> {
    let bytes = std::fs::read(&archive_path).map_err(|e| format!("读取压缩包失败: {}", e))?;
    let archive_name = std::path::Path::new(&archive_path)
        .file_name()
        .and_then(|n| n.to_str());
    skills::import_skills_from_bytes(
        &bytes,
        &scope,
        project_path.as_deref(),
        overwrite,
        archive_name,
    )
}
