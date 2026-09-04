use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Claude raw record types ──

/// A raw JSONL record from a Claude session file
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RawRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub message: Option<RawMessage>,
    #[serde(default)]
    pub is_sidechain: Option<bool>,
    pub cwd: Option<String>,
    pub version: Option<String>,
    pub git_branch: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMessage {
    pub role: String,
    pub content: ContentValue,
    pub model: Option<String>,
}

/// Content can be a simple string or an array of content blocks
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ContentValue {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// A single content block in a Claude message
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Option<Value>,
        #[serde(default)]
        is_error: Option<bool>,
    },
    #[serde(other)]
    Unknown,
}

// ── Unified display types (sent to frontend) ──

/// A display-ready message for the frontend
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayMessage {
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub role: String,
    pub timestamp: Option<String>,
    pub model: Option<String>,
    pub content: Vec<DisplayContentBlock>,
}

/// Unified content block enum covering Claude, Codex, and Grok types
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DisplayContentBlock {
    // Shared
    #[serde(rename = "text")]
    Text { text: String },
    // Claude-specific
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    // Codex-specific
    #[serde(rename = "reasoning")]
    Reasoning { text: String },
    #[serde(rename = "function_call")]
    FunctionCall {
        name: String,
        arguments: String,
        call_id: String,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput { call_id: String, output: String },
}

/// A lightweight, session-wide user-question index for navigation and summaries.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionIndexEntry {
    pub message_index: usize,
    pub message_id: String,
    pub preview: String,
    pub timestamp: Option<String>,
    pub parent_message_index: Option<usize>,
    pub reply_preview: String,
    pub reply_model: Option<String>,
    pub reply_timestamp: Option<String>,
    pub has_tool: bool,
}

/// Build one navigation item for each user prompt without returning the full
/// transcript. Parent and reply data are derived from the same visible-message
/// graph that the message page receives.
pub fn question_index(messages: &[DisplayMessage]) -> Vec<QuestionIndexEntry> {
    let message_indices: HashMap<_, _> = messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| message.uuid.as_deref().map(|id| (id, index)))
        .collect();
    let parents: Vec<_> = messages
        .iter()
        .map(|message| {
            message
                .parent_uuid
                .as_deref()
                .and_then(|id| message_indices.get(id).copied())
        })
        .collect();
    let mut children = vec![Vec::new(); messages.len()];
    for (index, parent) in parents.iter().enumerate() {
        if let Some(parent) = parent {
            children[*parent].push(index);
        }
    }

    messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role == "user")
        .map(|(index, message)| {
            let parent_message_index = nearest_user_parent(index, messages, &parents);
            let reply = first_assistant_descendant(index, messages, &children);
            let (reply_preview, reply_model, reply_timestamp, has_tool) = reply
                .map(|message| {
                    (
                        message_preview(message, 220),
                        message.model.clone(),
                        message.timestamp.clone(),
                        message.content.iter().any(|block| {
                            matches!(
                                block,
                                DisplayContentBlock::ToolUse { .. }
                                    | DisplayContentBlock::FunctionCall { .. }
                            )
                        }),
                    )
                })
                .unwrap_or_else(|| (String::new(), None, None, false));

            QuestionIndexEntry {
                message_index: index,
                message_id: message
                    .uuid
                    .clone()
                    .unwrap_or_else(|| format!("user-{index}")),
                preview: message_preview(message, 200),
                timestamp: message.timestamp.clone(),
                parent_message_index,
                reply_preview,
                reply_model,
                reply_timestamp,
                has_tool,
            }
        })
        .collect()
}

fn nearest_user_parent(
    index: usize,
    messages: &[DisplayMessage],
    parents: &[Option<usize>],
) -> Option<usize> {
    let mut current = parents[index];
    for _ in 0..messages.len() {
        let parent = current?;
        if messages[parent].role == "user" {
            return Some(parent);
        }
        current = parents[parent];
    }
    None
}

fn first_assistant_descendant<'a>(
    index: usize,
    messages: &'a [DisplayMessage],
    children: &[Vec<usize>],
) -> Option<&'a DisplayMessage> {
    let mut stack: Vec<_> = children[index].iter().rev().copied().collect();
    let mut visited = vec![false; messages.len()];
    while let Some(current) = stack.pop() {
        if visited[current] {
            continue;
        }
        visited[current] = true;
        if messages[current].role == "assistant" {
            return Some(&messages[current]);
        }
        stack.extend(children[current].iter().rev().copied());
    }
    None
}

fn message_preview(message: &DisplayMessage, limit: usize) -> String {
    let text = message
        .content
        .iter()
        .filter_map(|block| match block {
            DisplayContentBlock::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return String::new();
    }
    let mut chars = text.chars();
    let preview: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedMessages {
    pub messages: Vec<DisplayMessage>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_more: bool,
}

/// Result of a range-based message load: returns the slice
/// `messages[start..end)` along with the total count so the frontend can
/// extend the loaded window in either direction.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeMessages {
    pub messages: Vec<DisplayMessage>,
    pub total: usize,
    pub start: usize,
    pub end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(
        id: &str,
        parent_id: Option<&str>,
        role: &str,
        content: Vec<DisplayContentBlock>,
    ) -> DisplayMessage {
        DisplayMessage {
            uuid: Some(id.to_string()),
            parent_uuid: parent_id.map(str::to_string),
            role: role.to_string(),
            timestamp: Some("2026-09-04T00:00:00Z".to_string()),
            model: (role == "assistant").then(|| "test-model".to_string()),
            content,
        }
    }

    #[test]
    fn question_index_follows_visible_parent_chain_and_reply_metadata() {
        let messages = vec![
            message(
                "user-1",
                None,
                "user",
                vec![DisplayContentBlock::Text {
                    text: "First question".to_string(),
                }],
            ),
            message(
                "assistant-1",
                Some("user-1"),
                "assistant",
                vec![
                    DisplayContentBlock::Text {
                        text: "First reply".to_string(),
                    },
                    DisplayContentBlock::ToolUse {
                        id: "tool-1".to_string(),
                        name: "read".to_string(),
                        input: "{}".to_string(),
                    },
                ],
            ),
            message(
                "user-2",
                Some("assistant-1"),
                "user",
                vec![DisplayContentBlock::Text {
                    text: "Follow-up question".to_string(),
                }],
            ),
        ];

        let entries = question_index(&messages);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message_id, "user-1");
        assert_eq!(entries[0].reply_preview, "First reply");
        assert_eq!(entries[0].reply_model.as_deref(), Some("test-model"));
        assert!(entries[0].has_tool);
        assert_eq!(entries[1].message_id, "user-2");
        assert_eq!(entries[1].parent_message_index, Some(0));
    }
}
