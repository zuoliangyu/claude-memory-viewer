use session_core::search::SearchResult;

#[tauri::command]
pub fn global_search(
    source: String,
    query: String,
    max_results: usize,
) -> Result<Vec<SearchResult>, String> {
    session_core::search::global_search(&source, &query, max_results)
}
