use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::cli_config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

/// Stream a chat completion from Claude (Anthropic) or Codex (OpenAI-compatible) API.
///
/// Calls `on_chunk` with each text delta as it arrives.
/// The `model` parameter must be a full API model ID (e.g. "claude-sonnet-4-6"),
/// not a CLI alias (e.g. "sonnet").
pub async fn stream_chat(
    source: &str,
    messages: Vec<ChatMsg>,
    model: &str,
    on_chunk: impl Fn(&str),
) -> Result<(), String> {
    let (api_key, base_url) = cli_config::get_credentials(source);
    if api_key.is_empty() {
        return Err(format!(
            "No API key found for {}. Please configure your CLI or set the environment variable.",
            source
        ));
    }

    eprintln!("[quick_chat] source={}, model={}, base_url={}", source, model, base_url);

    match source {
        "claude" => stream_anthropic(&api_key, &base_url, messages, model, &on_chunk).await,
        "codex" => stream_openai(&api_key, &base_url, messages, model, &on_chunk).await,
        _ => Err(format!("Unknown source: {}", source)),
    }
}

// ── Anthropic streaming ──

async fn stream_anthropic(
    api_key: &str,
    base_url: &str,
    messages: Vec<ChatMsg>,
    model: &str,
    on_chunk: &impl Fn(&str),
) -> Result<(), String> {
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let api_messages: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 16384,
        "stream": true,
        "messages": api_messages,
    });

    let resp = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        eprintln!("[quick_chat] Anthropic API error {}: {}", status, text);
        return Err(format!("API Error: {} {}", status, text));
    }

    parse_sse_stream(resp, "anthropic", on_chunk).await
}

// ── OpenAI streaming ──

async fn stream_openai(
    api_key: &str,
    base_url: &str,
    messages: Vec<ChatMsg>,
    model: &str,
    on_chunk: &impl Fn(&str),
) -> Result<(), String> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let client = Client::new();

    let api_messages: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();

    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "messages": api_messages,
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, text));
    }

    parse_sse_stream(resp, "openai", on_chunk).await
}

// ── SSE parser ──

async fn parse_sse_stream(
    resp: reqwest::Response,
    provider: &str,
    on_chunk: &impl Fn(&str),
) -> Result<(), String> {
    use tokio::io::AsyncBufReadExt;
    use tokio_util::io::StreamReader;
    use futures_util::TryStreamExt;

    let stream = resp
        .bytes_stream()
        .map_err(std::io::Error::other);
    let reader = StreamReader::new(stream);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if !line.starts_with("data: ") {
            continue;
        }
        let data = &line[6..];
        if data == "[DONE]" {
            break;
        }

        let json: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let text = match provider {
            "anthropic" => extract_anthropic_delta(&json),
            "openai" => extract_openai_delta(&json),
            _ => None,
        };

        if let Some(t) = text {
            if !t.is_empty() {
                on_chunk(&t);
            }
        }
    }

    Ok(())
}

fn extract_anthropic_delta(json: &serde_json::Value) -> Option<String> {
    // Anthropic SSE events: content_block_delta with delta.text
    let event_type = json.get("type")?.as_str()?;
    if event_type == "content_block_delta" {
        let delta = json.get("delta")?;
        return delta.get("text").and_then(|v| v.as_str()).map(|s| s.to_string());
    }
    None
}

fn extract_openai_delta(json: &serde_json::Value) -> Option<String> {
    // OpenAI SSE: choices[0].delta.content
    let choices = json.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let delta = choice.get("delta")?;
    delta.get("content").and_then(|v| v.as_str()).map(|s| s.to_string())
}
