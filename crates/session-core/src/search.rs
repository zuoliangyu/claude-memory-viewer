use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::metadata;
use crate::models::message::{DisplayContentBlock, DisplayMessage};
use crate::parser::jsonl as claude_parser;
use crate::provider::{claude, codex, grok};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    All,
    Session,
    Content,
    Tags,
}

impl SearchScope {
    pub fn from_query(value: &str) -> Self {
        match value {
            "session" => Self::Session,
            "content" => Self::Content,
            "tags" => Self::Tags,
            _ => Self::All,
        }
    }

    fn includes_session(self) -> bool {
        matches!(self, Self::All | Self::Session)
    }

    fn includes_content(self) -> bool {
        matches!(self, Self::All | Self::Content)
    }

    fn includes_tags(self) -> bool {
        matches!(self, Self::All | Self::Tags)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub source: String,
    pub project_id: String,
    pub project_name: String,
    pub session_id: String,
    pub first_prompt: Option<String>,
    /// Codex only: Codex Desktop's thread title (session_index.jsonl).
    /// Preferred over first_prompt for display. `None` for Claude.
    pub thread_name: Option<String>,
    pub alias: Option<String>,
    pub tags: Option<Vec<String>>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
    pub file_path: String,
    pub total_message_count: u32,
    pub matched_message_id: Option<String>,
}

#[derive(Clone)]
struct SearchSessionContext {
    source: String,
    project_id: String,
    project_name: String,
    session_id: String,
    thread_name: Option<String>,
    alias: Option<String>,
    search_aliases: Vec<String>,
    tags: Option<Vec<String>>,
    file_path: String,
}

fn push_search_alias(search_aliases: &mut Vec<String>, alias: Option<String>) {
    if let Some(alias) = alias
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        if !search_aliases.iter().any(|existing| existing == &alias) {
            search_aliases.push(alias);
        }
    }
}

/// Safely truncate a string to approximately `max_chars` characters
fn safe_truncate(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

/// Extract a context window around a match, operating on characters (not
/// bytes). Implementation note: the previous version materialized two full
/// `Vec<char>` copies of the input (one lowercased), which on a 10MB tool
/// output means ~80MB of allocation just to find a match — bad for large
/// `tool_result` blocks. We instead use byte-level `find` on the lowercased
/// string and convert byte offsets to char offsets only for the matched
/// region we actually need.
fn extract_context(text: &str, query_lower: &str, context_chars: usize) -> String {
    let text_lower = text.to_lowercase();

    // Byte position of the match in the lowercased haystack. `find` works
    // on UTF-8 byte boundaries, so it's safe to slice at the same byte
    // index in the original `text` (lowercasing in Rust is per-codepoint
    // and doesn't change byte alignment for ASCII queries; for non-ASCII
    // queries that change length when lowercased, we fall back to
    // safe_truncate below).
    let match_byte = match text_lower.find(query_lower) {
        Some(b) => b,
        None => return safe_truncate(text, context_chars * 2),
    };

    // Bail out if the lowercase transformation shifted byte alignments
    // (e.g. a multi-byte uppercase mapping). In that case the byte offset
    // doesn't correspond to a valid char boundary in `text`, so we
    // degrade to a safe truncation rather than risk a slicing panic.
    if !text.is_char_boundary(match_byte) {
        return safe_truncate(text, context_chars * 2);
    }

    // Walk back `context_chars` chars from the match start.
    let prefix = &text[..match_byte];
    let prefix_chars = prefix.chars().count();
    let drop_chars = prefix_chars.saturating_sub(context_chars);
    let mut iter = prefix.char_indices();
    let start = if drop_chars == 0 {
        0
    } else {
        iter.nth(drop_chars - 1)
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(0)
    };

    // Walk forward `query.chars().count() + context_chars` chars from
    // `match_byte`.
    let query_char_len = query_lower.chars().count();
    let want_chars = query_char_len + context_chars;
    let mut end = text.len();
    for (i, (idx, ch)) in text[match_byte..].char_indices().enumerate() {
        if i == want_chars {
            end = match_byte + idx;
            break;
        }
        let _ = ch;
    }

    text[start..end].to_string()
}

/// Extract searchable text from a DisplayContentBlock
fn block_text(block: &DisplayContentBlock) -> &str {
    match block {
        DisplayContentBlock::Text { text } => text,
        DisplayContentBlock::Thinking { thinking } => thinking,
        DisplayContentBlock::ToolUse { input, .. } => input,
        DisplayContentBlock::ToolResult { content, .. } => content,
        DisplayContentBlock::Reasoning { text } => text,
        DisplayContentBlock::FunctionCall { arguments, .. } => arguments,
        DisplayContentBlock::FunctionCallOutput { output, .. } => output,
    }
}

fn push_limited_result(
    results: &mut Vec<SearchResult>,
    counter: &AtomicUsize,
    max_results: usize,
    result: SearchResult,
) -> bool {
    if counter.fetch_add(1, Ordering::Relaxed) >= max_results {
        return false;
    }
    results.push(result);
    true
}

fn search_messages_for_session(
    ctx: &SearchSessionContext,
    messages: &[DisplayMessage],
    query_lower: &str,
    scope: SearchScope,
    counter: &AtomicUsize,
    max_results: usize,
) -> Vec<SearchResult> {
    let total_message_count = messages.len() as u32;
    let mut results = Vec::new();
    let mut session_name_matched = false;
    let mut message_match_count = 0usize;
    let mut first_prompt = None;

    if scope.includes_tags() {
        if let Some(matched_tag) = ctx
            .tags
            .as_ref()
            .and_then(|tags| tags.iter().find(|t| t.to_lowercase().contains(query_lower)))
        {
            if !push_limited_result(
                &mut results,
                counter,
                max_results,
                SearchResult {
                    source: ctx.source.clone(),
                    project_id: ctx.project_id.clone(),
                    project_name: ctx.project_name.clone(),
                    session_id: ctx.session_id.clone(),
                    first_prompt: None,
                    thread_name: ctx.thread_name.clone(),
                    alias: ctx.alias.clone(),
                    tags: ctx.tags.clone(),
                    matched_text: matched_tag.clone(),
                    role: "tag".to_string(),
                    timestamp: None,
                    file_path: ctx.file_path.clone(),
                    total_message_count,
                    matched_message_id: None,
                },
            ) {
                return results;
            }
        }
    }

    if scope.includes_session() {
        if let Some(matched_alias) = ctx
            .search_aliases
            .iter()
            .find(|alias| alias.to_lowercase().contains(query_lower))
        {
            session_name_matched = true;
            if !push_limited_result(
                &mut results,
                counter,
                max_results,
                SearchResult {
                    source: ctx.source.clone(),
                    project_id: ctx.project_id.clone(),
                    project_name: ctx.project_name.clone(),
                    session_id: ctx.session_id.clone(),
                    first_prompt: None,
                    thread_name: ctx.thread_name.clone(),
                    alias: ctx.alias.clone(),
                    tags: ctx.tags.clone(),
                    matched_text: matched_alias.clone(),
                    role: "session".to_string(),
                    timestamp: None,
                    file_path: ctx.file_path.clone(),
                    total_message_count,
                    matched_message_id: None,
                },
            ) {
                return results;
            }
        }
    }

    for msg in messages {
        if counter.load(Ordering::Relaxed) >= max_results {
            break;
        }

        if msg.role == "user" && first_prompt.is_none() {
            for block in &msg.content {
                if let DisplayContentBlock::Text { text } = block {
                    first_prompt = Some(safe_truncate(text, 100));
                    if scope.includes_session()
                        && !session_name_matched
                        && text.to_lowercase().contains(query_lower)
                    {
                        session_name_matched = true;
                        if !push_limited_result(
                            &mut results,
                            counter,
                            max_results,
                            SearchResult {
                                source: ctx.source.clone(),
                                project_id: ctx.project_id.clone(),
                                project_name: ctx.project_name.clone(),
                                session_id: ctx.session_id.clone(),
                                first_prompt: first_prompt.clone(),
                                thread_name: ctx.thread_name.clone(),
                                alias: ctx.alias.clone(),
                                tags: ctx.tags.clone(),
                                matched_text: safe_truncate(text, 100),
                                role: "session".to_string(),
                                timestamp: msg.timestamp.clone(),
                                file_path: ctx.file_path.clone(),
                                total_message_count,
                                matched_message_id: msg.uuid.clone(),
                            },
                        ) {
                            return results;
                        }
                    }
                    break;
                }
            }
        }

        if !scope.includes_content() {
            continue;
        }

        for block in &msg.content {
            if counter.load(Ordering::Relaxed) >= max_results {
                return results;
            }

            let text = block_text(block);
            if text.to_lowercase().contains(query_lower) {
                if !push_limited_result(
                    &mut results,
                    counter,
                    max_results,
                    SearchResult {
                        source: ctx.source.clone(),
                        project_id: ctx.project_id.clone(),
                        project_name: ctx.project_name.clone(),
                        session_id: ctx.session_id.clone(),
                        first_prompt: first_prompt.clone(),
                        thread_name: ctx.thread_name.clone(),
                        alias: ctx.alias.clone(),
                        tags: ctx.tags.clone(),
                        matched_text: extract_context(text, query_lower, 50),
                        role: msg.role.clone(),
                        timestamp: msg.timestamp.clone(),
                        file_path: ctx.file_path.clone(),
                        total_message_count,
                        matched_message_id: msg.uuid.clone(),
                    },
                ) {
                    return results;
                }

                message_match_count += 1;
                if message_match_count >= 5 {
                    return results;
                }
            }
        }
    }

    results
}

pub fn global_search(
    source: &str,
    query: &str,
    max_results: usize,
    scope: SearchScope,
) -> Result<Vec<SearchResult>, String> {
    let query_lower = query.to_lowercase();

    let results: Vec<SearchResult> = match source {
        "claude" => search_claude(&query_lower, max_results, scope),
        "codex" => search_codex(&query_lower, max_results, scope),
        "grok" => search_grok(&query_lower, max_results, scope),
        _ => return Err(format!("Unknown source: {}", source)),
    };

    Ok(results)
}

fn search_claude(query_lower: &str, max_results: usize, scope: SearchScope) -> Vec<SearchResult> {
    if max_results == 0 {
        return Vec::new();
    }

    let jsonl_files = claude::collect_all_jsonl_files();
    let result_count = AtomicUsize::new(0);

    // Pre-load metadata per project for alias lookup
    let mut meta_cache: std::collections::HashMap<String, metadata::MetadataFile> =
        std::collections::HashMap::new();
    for (encoded_name, _, _) in &jsonl_files {
        meta_cache
            .entry(encoded_name.clone())
            .or_insert_with(|| metadata::load_metadata("claude", encoded_name));
    }

    let results: Vec<SearchResult> = jsonl_files
        .par_iter()
        .flat_map(|(encoded_name, project_name, file_path)| {
            if result_count.load(Ordering::Relaxed) >= max_results {
                return Vec::new();
            }

            let session_id = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => return Vec::new(),
            };

            // Lookup alias and tags from metadata
            let session_meta = meta_cache
                .get(encoded_name)
                .and_then(|m| m.sessions.get(&session_id));
            let metadata_alias = session_meta.and_then(|s| s.alias.clone());
            let tags = session_meta
                .map(|s| s.tags.clone())
                .filter(|t| !t.is_empty());
            let custom_title = claude_parser::scan_session_file_once(file_path)
                .and_then(|scan| scan.custom_title);
            let alias = custom_title.clone().or(metadata_alias.clone());
            let mut search_aliases = Vec::with_capacity(2);
            push_search_alias(&mut search_aliases, custom_title);
            push_search_alias(&mut search_aliases, metadata_alias);
            let content_has_query = content.to_lowercase().contains(query_lower);
            let alias_has_query = search_aliases
                .iter()
                .any(|candidate| candidate.to_lowercase().contains(query_lower));
            let tags_has_query = tags
                .as_ref()
                .map(|t| t.iter().any(|tag| tag.to_lowercase().contains(query_lower)))
                .unwrap_or(false);

            if !content_has_query && !alias_has_query && !tags_has_query {
                return Vec::new();
            }

            if let Ok(messages) = claude::parse_all_messages(file_path) {
                let ctx = SearchSessionContext {
                    source: "claude".to_string(),
                    project_id: encoded_name.clone(),
                    project_name: project_name.clone(),
                    session_id,
                    thread_name: None,
                    alias,
                    search_aliases,
                    tags,
                    file_path: file_path.to_string_lossy().to_string(),
                };
                return search_messages_for_session(
                    &ctx,
                    &messages,
                    query_lower,
                    scope,
                    &result_count,
                    max_results,
                );
            }

            Vec::new()
        })
        .collect();

    let mut results = results;
    results.truncate(max_results);
    results
}

fn search_codex(query_lower: &str, max_results: usize, scope: SearchScope) -> Vec<SearchResult> {
    if max_results == 0 {
        return Vec::new();
    }

    let files = codex::scan_all_session_files();
    let result_count = AtomicUsize::new(0);

    // Pre-load codex metadata (single file for all sessions)
    let codex_meta = metadata::load_metadata("codex", "");
    // Pre-load Codex Desktop thread titles (session_index.jsonl, one small file)
    let thread_names = codex::load_thread_names();

    let results: Vec<SearchResult> = files
        .par_iter()
        .flat_map(|file_path| {
            if result_count.load(Ordering::Relaxed) >= max_results {
                return Vec::new();
            }

            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => return Vec::new(),
            };

            let meta = codex::extract_session_meta(file_path);
            let (session_id, cwd) = match &meta {
                Some(m) => (m.id.clone(), m.cwd.clone()),
                None => {
                    let stem = file_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    (stem, String::new())
                }
            };
            let short_name = cwd
                .rsplit(['/', '\\'])
                .find(|s| !s.is_empty())
                .unwrap_or(&cwd)
                .to_string();

            let session_meta = codex_meta.sessions.get(&session_id);
            let alias = session_meta.and_then(|s| s.alias.clone());
            let tags = session_meta
                .map(|s| s.tags.clone())
                .filter(|t| !t.is_empty());
            let mut search_aliases = Vec::with_capacity(1);
            push_search_alias(&mut search_aliases, alias.clone());
            let content_has_query = content.to_lowercase().contains(query_lower);
            let alias_has_query = search_aliases
                .iter()
                .any(|candidate| candidate.to_lowercase().contains(query_lower));
            let tags_has_query = tags
                .as_ref()
                .map(|t| t.iter().any(|tag| tag.to_lowercase().contains(query_lower)))
                .unwrap_or(false);

            if !content_has_query && !alias_has_query && !tags_has_query {
                return Vec::new();
            }

            if let Ok(messages) = codex::parse_all_messages(file_path) {
                let thread_name = thread_names.get(&session_id).cloned();
                let ctx = SearchSessionContext {
                    source: "codex".to_string(),
                    project_id: cwd,
                    project_name: short_name,
                    session_id,
                    thread_name,
                    alias,
                    search_aliases,
                    tags,
                    file_path: file_path.to_string_lossy().to_string(),
                };
                return search_messages_for_session(
                    &ctx,
                    &messages,
                    query_lower,
                    scope,
                    &result_count,
                    max_results,
                );
            }

            Vec::new()
        })
        .collect();

    let mut results = results;
    results.truncate(max_results);
    results
}

fn search_grok(query_lower: &str, max_results: usize, scope: SearchScope) -> Vec<SearchResult> {
    if max_results == 0 {
        return Vec::new();
    }

    let mut sessions = Vec::new();
    for project in grok::get_projects().unwrap_or_default() {
        let meta = metadata::load_metadata("grok", &project.id);
        for session in grok::get_sessions(&project.id).unwrap_or_default() {
            let session_meta = meta.sessions.get(&session.session_id);
            sessions.push((
                project.id.clone(),
                project.short_name.clone(),
                session,
                session_meta.and_then(|item| item.alias.clone()),
                session_meta
                    .map(|item| item.tags.clone())
                    .filter(|tags| !tags.is_empty()),
            ));
        }
    }

    let result_count = AtomicUsize::new(0);
    let results: Vec<SearchResult> = sessions
        .par_iter()
        .flat_map(|(project_id, project_name, session, alias, tags)| {
            if result_count.load(Ordering::Relaxed) >= max_results {
                return Vec::new();
            }

            let Ok(messages) = grok::parse_all_messages(std::path::Path::new(&session.file_path))
            else {
                return Vec::new();
            };
            let mut search_aliases = Vec::with_capacity(2);
            push_search_alias(&mut search_aliases, session.thread_name.clone());
            push_search_alias(&mut search_aliases, alias.clone());
            let ctx = SearchSessionContext {
                source: "grok".to_string(),
                project_id: project_id.clone(),
                project_name: project_name.clone(),
                session_id: session.session_id.clone(),
                thread_name: session.thread_name.clone(),
                alias: alias.clone(),
                search_aliases,
                tags: tags.clone(),
                file_path: session.file_path.clone(),
            };

            search_messages_for_session(
                &ctx,
                &messages,
                query_lower,
                scope,
                &result_count,
                max_results,
            )
        })
        .collect();

    let mut results = results;
    results.truncate(max_results);
    results
}
