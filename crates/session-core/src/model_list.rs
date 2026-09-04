use serde::{Deserialize, Serialize};

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

/// Infer a human-friendly group name from a Claude model ID.
fn infer_group(id: &str) -> String {
    let lower = id.to_lowercase();
    if lower.contains("opus") {
        return "Claude Opus".to_string();
    }
    if lower.contains("sonnet") {
        return "Claude Sonnet".to_string();
    }
    if lower.contains("haiku") {
        return "Claude Haiku".to_string();
    }
    "Other".to_string()
}

/// Infer a human-friendly group name from an OpenAI model ID.
fn infer_openai_group(id: &str) -> String {
    let lower = id.to_lowercase();
    if lower.starts_with("codex") {
        return "Codex".to_string();
    }
    if lower.starts_with("o4") {
        return "OpenAI o4".to_string();
    }
    if lower.starts_with("o3") {
        return "OpenAI o3".to_string();
    }
    if lower.starts_with("o1") {
        return "OpenAI o1".to_string();
    }
    if lower.starts_with("gpt-4") {
        return "GPT-4".to_string();
    }
    if lower.starts_with("gpt-3") {
        return "GPT-3".to_string();
    }
    "OpenAI".to_string()
}

/// Resolve Codex credentials by going through `cli_config`, which knows how
/// to read `~/.codex/auth.json`, `~/.codex/config.toml` (including the new
/// `[model_providers.<name>]` form), env vars, and shell rc files.
fn get_codex_credentials() -> (String, String) {
    cli_config::get_credentials("codex")
}

/// Append `/v1/models` to a base URL, tolerating bases that already end in
/// `/v1` (e.g. `https://example.com/v1`).
fn join_models_endpoint(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{}/models", trimmed)
    } else {
        format!("{}/v1/models", trimmed)
    }
}

async fn fetch_anthropic_models(api_key: &str, base_url: &str) -> Result<Vec<ModelInfo>, String> {
    let url = join_models_endpoint(base_url);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("Authorization", format!("Bearer {}", api_key))
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
            let name = m.display_name.unwrap_or_else(|| m.id.clone());
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

    // When using a proxy, the /v1/models endpoint may return models from all
    // providers.  Only keep models that look like Claude models.
    models.retain(|m| m.id.to_lowercase().contains("claude"));

    // Sort by created desc (newest first)
    models.sort_by_key(|m| std::cmp::Reverse(m.created));
    Ok(models)
}

/// Fetch available models from OpenAI /v1/models and filter to relevant ones.
async fn fetch_openai_models(api_key: &str, base_url: &str) -> Result<Vec<ModelInfo>, String> {
    #[derive(Debug, Deserialize)]
    struct OpenAIModelsResponse {
        data: Vec<OpenAIModel>,
    }
    #[derive(Debug, Deserialize)]
    struct OpenAIModel {
        id: String,
        created: Option<i64>,
    }

    let effective_base = if base_url.is_empty() {
        "https://api.openai.com"
    } else {
        base_url
    };
    let url = join_models_endpoint(effective_base);
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
            let group = infer_openai_group(&m.id);
            let name =
                m.id.split('-')
                    .map(|s| {
                        let mut c = s.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().to_string() + c.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("-");
            ModelInfo {
                id: m.id,
                name,
                provider: "openai".to_string(),
                group,
                created: m.created,
            }
        })
        .collect();

    // Sort: codex first (by group priority), then by created desc within group
    models.sort_by(|a, b| {
        let priority = |g: &str| match g {
            "Codex" => 0,
            "OpenAI o4" => 1,
            "OpenAI o3" => 2,
            "OpenAI o1" => 3,
            "GPT-4" => 4,
            _ => 5,
        };
        priority(&a.group)
            .cmp(&priority(&b.group))
            .then(b.created.cmp(&a.created))
    });
    Ok(models)
}

#[derive(Debug, Deserialize)]
struct OmpModelsConfig {
    #[serde(default)]
    providers: std::collections::HashMap<String, OmpProviderConfig>,
}

#[derive(Debug, Deserialize)]
struct OmpProviderConfig {
    #[serde(default)]
    models: Vec<OmpModelConfig>,
}

#[derive(Debug, Deserialize)]
struct OmpModelConfig {
    id: String,
    name: Option<String>,
}

fn omp_models_config_path() -> Option<std::path::PathBuf> {
    let agent_dir = crate::provider::omp::get_sessions_dir()?.parent()?.to_path_buf();
    let yml = agent_dir.join("models.yml");
    if yml.is_file() {
        return Some(yml);
    }
    let yaml = agent_dir.join("models.yaml");
    yaml.is_file().then_some(yaml)
}

fn read_omp_models() -> Result<Vec<ModelInfo>, String> {
    let Some(path) = omp_models_config_path() else {
        return Ok(Vec::new());
    };
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read OMP model config {}: {}", path.display(), e))?;
    let config: OmpModelsConfig = serde_yml::from_str(&content)
        .map_err(|e| format!("Failed to parse OMP model config {}: {}", path.display(), e))?;

    let mut models = Vec::new();
    for (provider, provider_config) in config.providers {
        for model in provider_config.models {
            let id = model.id.trim();
            if id.is_empty() {
                continue;
            }
            let qualified_id = format!("{provider}/{id}");
            models.push(ModelInfo {
                id: qualified_id,
                name: model
                    .name
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| id.to_string()),
                provider: provider.clone(),
                group: provider.clone(),
                created: None,
            });
        }
    }
    models.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.name.cmp(&b.name)));
    Ok(models)
}

/// List available models.
///
/// - `source`: "claude", "codex", or "omp"
/// - `api_key`: user-provided key (empty = auto-detect from config/env)
/// - `base_url`: base URL (empty = default for the given source)
pub async fn list_models(
    source: &str,
    api_key: &str,
    base_url: &str,
) -> Result<Vec<ModelInfo>, String> {
    if source == "omp" {
        return read_omp_models();
    }
    if source == "codex" {
        let (cfg_key, cfg_url) = get_codex_credentials();
        let key = if api_key.is_empty() {
            cfg_key
        } else {
            api_key.to_string()
        };
        if key.is_empty() {
            return Ok(vec![]);
        }
        let url = if base_url.is_empty() {
            cfg_url
        } else {
            base_url.to_string()
        };
        return fetch_openai_models(&key, &url).await;
    }
    let (resolved_key, resolved_url) = if api_key.is_empty() && base_url.is_empty() {
        let (cli_key, cli_url) = cli_config::get_credentials("claude");
        let final_key = if cli_key.is_empty() {
            std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
        } else {
            cli_key
        };
        (final_key, cli_url)
    } else {
        let key = if api_key.is_empty() {
            std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
        } else {
            api_key.to_string()
        };
        let url = if base_url.is_empty() {
            std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string())
        } else {
            base_url.to_string()
        };
        (key, url)
    };

    if resolved_key.is_empty() {
        return Ok(vec![]);
    }

    fetch_anthropic_models(&resolved_key, &resolved_url).await
}
