use serde::{Deserialize, Serialize};
use std::env;

use crate::cli_config;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub group: String,
    pub created: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
    display_name: Option<String>,
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
    created: Option<i64>,
}

/// Infer a human-friendly group name from a model ID.
fn infer_group(id: &str) -> String {
    let lower = id.to_lowercase();
    // Claude models
    if lower.contains("opus") {
        return "Claude Opus".to_string();
    }
    if lower.contains("sonnet") {
        return "Claude Sonnet".to_string();
    }
    if lower.contains("haiku") {
        return "Claude Haiku".to_string();
    }
    // OpenAI / Codex models
    if lower.starts_with("codex") {
        return "Codex".to_string();
    }
    if lower.starts_with("o4") {
        return "o4".to_string();
    }
    if lower.starts_with("o3") {
        return "o3".to_string();
    }
    if lower.starts_with("o1") {
        return "o1".to_string();
    }
    if lower.starts_with("gpt-4") {
        return "GPT-4".to_string();
    }
    if lower.starts_with("gpt-3") {
        return "GPT-3.5".to_string();
    }
    "Other".to_string()
}

/// Infer a short display name from a model ID.
fn infer_name(id: &str) -> String {
    // Try to create a cleaner display name
    let name = id
        .replace("claude-", "Claude ")
        .replace("codex-", "Codex ")
        .replace("-latest", " (latest)")
        .replace("-preview", " (preview)");
    // Capitalize first letter
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

async fn fetch_anthropic_models(api_key: &str, base_url: &str) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| format!("Anthropic API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error {}: {}", status, text));
    }

    let body: AnthropicModelsResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Anthropic models response: {}", e))?;

    let mut models: Vec<ModelInfo> = body
        .data
        .into_iter()
        .map(|m| {
            let name = m.display_name.unwrap_or_else(|| infer_name(&m.id));
            let group = infer_group(&m.id);
            let created = m.created_at.and_then(|ts| {
                chrono::DateTime::parse_from_rfc3339(&ts)
                    .ok()
                    .map(|dt| dt.timestamp())
            });
            ModelInfo {
                id: m.id,
                name,
                provider: "anthropic".to_string(),
                group,
                created,
            }
        })
        .collect();

    // Sort by created desc (newest first)
    models.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(models)
}

async fn fetch_openai_models(api_key: &str, base_url: &str) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, text));
    }

    let body: OpenAIModelsResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI models response: {}", e))?;

    let mut models: Vec<ModelInfo> = body
        .data
        .into_iter()
        .map(|m| {
            let name = infer_name(&m.id);
            let group = infer_group(&m.id);
            ModelInfo {
                id: m.id,
                name,
                provider: "openai".to_string(),
                group,
                created: m.created,
            }
        })
        .collect();

    // Sort by created desc (newest first)
    models.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(models)
}

/// Built-in Claude models — mirrors Claude CLI `/model` output.
/// Uses short IDs without date suffix for maximum proxy compatibility.
fn builtin_claude_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "claude-sonnet-4-6".to_string(),
            name: "Sonnet 4.6 (默认推荐)".to_string(),
            provider: "anthropic".to_string(),
            group: "Claude Sonnet".to_string(),
            created: None,
        },
        ModelInfo {
            id: "claude-opus-4-6".to_string(),
            name: "Opus 4.6".to_string(),
            provider: "anthropic".to_string(),
            group: "Claude Opus".to_string(),
            created: None,
        },
        ModelInfo {
            id: "claude-haiku-4-5".to_string(),
            name: "Haiku 4.5".to_string(),
            provider: "anthropic".to_string(),
            group: "Claude Haiku".to_string(),
            created: None,
        },
    ]
}

/// Built-in Codex/OpenAI models.
fn builtin_codex_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "codex-mini-latest".to_string(),
            name: "Codex Mini (latest)".to_string(),
            provider: "openai".to_string(),
            group: "Codex".to_string(),
            created: None,
        },
        ModelInfo {
            id: "o4-mini".to_string(),
            name: "o4 Mini".to_string(),
            provider: "openai".to_string(),
            group: "o4".to_string(),
            created: None,
        },
        ModelInfo {
            id: "o3".to_string(),
            name: "o3".to_string(),
            provider: "openai".to_string(),
            group: "o3".to_string(),
            created: None,
        },
        ModelInfo {
            id: "o3-mini".to_string(),
            name: "o3 Mini".to_string(),
            provider: "openai".to_string(),
            group: "o3".to_string(),
            created: None,
        },
        ModelInfo {
            id: "gpt-4.1".to_string(),
            name: "GPT-4.1".to_string(),
            provider: "openai".to_string(),
            group: "GPT-4".to_string(),
            created: None,
        },
        ModelInfo {
            id: "gpt-4.1-mini".to_string(),
            name: "GPT-4.1 Mini".to_string(),
            provider: "openai".to_string(),
            group: "GPT-4".to_string(),
            created: None,
        },
    ]
}

/// Merge: built-in models first, then append any API-only extras (deduped).
fn merge_models(builtin: Vec<ModelInfo>, api_models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    use std::collections::HashSet;
    let builtin_ids: HashSet<String> = builtin.iter().map(|m| m.id.clone()).collect();
    let mut result = builtin;
    for m in api_models {
        if !builtin_ids.contains(&m.id) {
            result.push(m);
        }
    }
    result
}

/// Unified entry point: try API first, fallback to hardcoded list on failure.
///
/// - `source`: "claude" or "codex"
/// - `api_key`: user-provided key (empty string = use env var)
/// - `base_url`: base URL for the API (empty string = use env var or default)
pub async fn list_models(
    source: &str,
    api_key: &str,
    base_url: &str,
) -> Result<Vec<ModelInfo>, String> {
    // When api_key/base_url are empty, try CLI config before falling back to env vars
    let (resolved_key, resolved_url) = if api_key.is_empty() && base_url.is_empty() {
        let (cli_key, cli_url) = cli_config::get_credentials(source);
        let final_key = if cli_key.is_empty() {
            match source {
                "claude" => env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
                "codex" => env::var("OPENAI_API_KEY").unwrap_or_default(),
                _ => String::new(),
            }
        } else {
            cli_key
        };
        (final_key, cli_url)
    } else {
        let key = if api_key.is_empty() {
            match source {
                "claude" => env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
                "codex" => env::var("OPENAI_API_KEY").unwrap_or_default(),
                _ => String::new(),
            }
        } else {
            api_key.to_string()
        };
        let url = if base_url.is_empty() {
            match source {
                "claude" => env::var("ANTHROPIC_BASE_URL")
                    .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
                "codex" => env::var("OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com".to_string()),
                _ => String::new(),
            }
        } else {
            base_url.to_string()
        };
        (key, url)
    };

    match source {
        "claude" => {
            let builtin = builtin_claude_models();
            if resolved_key.is_empty() {
                return Ok(builtin);
            }

            // Fetch API models in background; failures are non-fatal
            let api_models = match fetch_anthropic_models(&resolved_key, &resolved_url).await {
                Ok(models) => models,
                Err(e) => {
                    eprintln!("Warning: failed to fetch Anthropic models: {}", e);
                    vec![]
                }
            };

            // Built-in first, then any API-only extras
            Ok(merge_models(builtin, api_models))
        }
        "codex" => {
            let builtin = builtin_codex_models();
            if resolved_key.is_empty() {
                return Ok(builtin);
            }

            let api_models = match fetch_openai_models(&resolved_key, &resolved_url).await {
                Ok(models) => models,
                Err(e) => {
                    eprintln!("Warning: failed to fetch OpenAI models: {}", e);
                    vec![]
                }
            };

            Ok(merge_models(builtin, api_models))
        }
        _ => Err(format!("Unknown source: {}", source)),
    }
}
