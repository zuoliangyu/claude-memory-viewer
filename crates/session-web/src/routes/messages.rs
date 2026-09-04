use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use session_core::models::message::{
    question_index, PaginatedMessages, QuestionIndexEntry, RangeMessages,
};
use session_core::provider::{claude, codex, grok, omp};

use crate::resolve_session_file_path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagesQuery {
    pub source: String,
    pub file_path: String,
    #[serde(default)]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
    #[serde(default)]
    pub from_end: bool,
}

fn default_page_size() -> usize {
    50
}

pub async fn get_messages(
    Query(params): Query<MessagesQuery>,
) -> Result<Json<PaginatedMessages>, (StatusCode, String)> {
    let source = params.source;
    let resolved_path = resolve_session_file_path(&source, &params.file_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let page = params.page;
    let page_size = params.page_size;
    let from_end = params.from_end;

    let result = tokio::task::spawn_blocking(move || match source.as_str() {
        "claude" => claude::parse_session_messages(&resolved_path, page, page_size, from_end),
        "codex" => codex::parse_session_messages(&resolved_path, page, page_size, from_end),
        "grok" => grok::parse_session_messages(&resolved_path, page, page_size, from_end),
        "omp" => omp::parse_session_messages(&resolved_path, page, page_size, from_end),
        _ => Err(format!("Unknown source: {}", source)),
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagesRangeQuery {
    pub source: String,
    pub file_path: String,
    pub start: usize,
    pub end: usize,
}

/// Load `[start, end)` slice of a session for the windowed message view.
pub async fn get_messages_range(
    Query(params): Query<MessagesRangeQuery>,
) -> Result<Json<RangeMessages>, (StatusCode, String)> {
    let source = params.source;
    let resolved_path = resolve_session_file_path(&source, &params.file_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let start = params.start;
    let end = params.end;

    let result = tokio::task::spawn_blocking(move || match source.as_str() {
        "claude" => claude::parse_messages_range(&resolved_path, start, end),
        "codex" => codex::parse_messages_range(&resolved_path, start, end),
        "grok" => grok::parse_messages_range(&resolved_path, start, end),
        "omp" => omp::parse_messages_range(&resolved_path, start, end),
        _ => Err(format!("Unknown source: {}", source)),
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionIndexQuery {
    pub source: String,
    pub file_path: String,
}

/// Return session-wide user-question metadata without widening the message window.
pub async fn get_question_index(
    Query(params): Query<QuestionIndexQuery>,
) -> Result<Json<Vec<QuestionIndexEntry>>, (StatusCode, String)> {
    let source = params.source;
    let resolved_path = resolve_session_file_path(&source, &params.file_path)
        .map_err(|error| (StatusCode::BAD_REQUEST, error))?;

    let result = tokio::task::spawn_blocking(move || {
        let messages = match source.as_str() {
            "claude" => claude::parse_all_messages(&resolved_path),
            "codex" => codex::parse_all_messages(&resolved_path),
            "grok" => grok::parse_all_messages(&resolved_path),
            "omp" => omp::parse_all_messages(&resolved_path),
            _ => Err(format!("Unknown source: {source}")),
        }?;
        Ok(question_index(&messages))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;

    Ok(Json(result))
}
