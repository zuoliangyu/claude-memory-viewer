use session_core::models::stats::TokenUsageSummary;

#[tauri::command]
pub fn get_stats(source: String) -> Result<TokenUsageSummary, String> {
    session_core::stats::get_stats(&source)
}
