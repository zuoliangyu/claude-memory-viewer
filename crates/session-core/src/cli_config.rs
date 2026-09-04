use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// CLI configuration info returned to the frontend (API key is masked).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CliConfig {
    pub source: String,
    /// Final resolved API key (masked), used for actual requests.
    pub api_key_masked: String,
    pub has_api_key: bool,
    /// Final resolved base URL.
    pub base_url: String,
    pub default_model: String,
    /// Primary config file path (settings.json for Claude, config.toml for Codex).
    pub config_path: String,

    // ── Codex-specific fields (empty for Claude) ──
    /// Path to ~/.codex/auth.json (Codex only).
    #[serde(default)]
    pub auth_json_path: String,
    /// API key found in auth.json (masked). Primary key source for Codex.
    #[serde(default)]
    pub auth_json_key_masked: String,
    #[serde(default)]
    pub auth_json_has_key: bool,
    /// API key found in config.toml (masked), if present.
    #[serde(default)]
    pub config_toml_key_masked: String,
    #[serde(default)]
    pub config_toml_has_key: bool,
    /// Base URL found in config.toml, if present.
    #[serde(default)]
    pub config_toml_url: String,
    /// Where the final API key came from: "auth.json" | "config.toml" | "env" | "".
    #[serde(default)]
    pub api_key_source: String,
    /// Where the final base URL came from: "config.toml" | "env" | "default".
    #[serde(default)]
    pub base_url_source: String,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedCliCredentials {
    pub api_key: String,
    pub base_url: String,
}

// ── Internal deserialization structures ──

/// Claude's `~/.claude/settings.json`
#[derive(Debug, Deserialize, Default)]
struct ClaudeSettings {
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    model: Option<String>,
}

/// Codex's `~/.codex/config.toml`
#[derive(Debug, Deserialize, Default)]
struct CodexConfig {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    /// Legacy single-provider block: `[provider]`.
    #[serde(default)]
    provider: Option<CodexProvider>,
    /// Currently selected provider name, used to resolve a key under
    /// `[model_providers.<name>]`.
    #[serde(default)]
    model_provider: Option<String>,
    /// New nested-provider format: `[model_providers.<name>]`.
    #[serde(default)]
    model_providers: Option<HashMap<String, CodexProvider>>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct CodexProvider {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
}

/// Pick the active provider from a Codex config — preferring the new
/// `[model_providers.<name>]` form keyed by `model_provider`, falling back
/// to the legacy `[provider]` block.
fn active_codex_provider(cfg: &CodexConfig) -> Option<CodexProvider> {
    if let Some(name) = cfg.model_provider.as_deref().filter(|s| !s.is_empty()) {
        if let Some(map) = cfg.model_providers.as_ref() {
            if let Some(p) = map.get(name) {
                return Some(p.clone());
            }
        }
    }
    cfg.provider.clone()
}

/// Codex's `~/.codex/auth.json`
#[derive(Debug, Deserialize, Default)]
struct CodexAuth {
    #[serde(rename = "OPENAI_API_KEY", default)]
    openai_api_key: Option<String>,
    #[serde(rename = "CODEX_API_KEY", default)]
    codex_api_key: Option<String>,
}

// ── Public interface ──

/// Read CLI configuration for the given source and return a masked version.
pub fn read_cli_config(source: &str) -> Result<CliConfig, String> {
    if source == "codex" {
        return read_codex_cli_config();
    }
    if source == "omp" {
        return Ok(CliConfig {
            source: "omp".to_string(),
            ..Default::default()
        });
    }
    let (api_key, base_url, default_model, config_path) = read_claude_config()?;
    Ok(CliConfig {
        source: "claude".to_string(),
        api_key_masked: mask_key(&api_key),
        has_api_key: !api_key.is_empty(),
        base_url,
        default_model,
        config_path,
        ..Default::default()
    })
}

/// Resolve effective credentials for runtime use.
/// Request overrides take precedence over CLI config fallback.
pub fn resolve_credentials(
    source: &str,
    api_key_override: Option<&str>,
    base_url_override: Option<&str>,
) -> Result<ResolvedCliCredentials, String> {
    let source = crate::cli::normalize_source(source)?;
    if source == "omp" {
        return Ok(ResolvedCliCredentials {
            api_key: api_key_override.unwrap_or_default().trim().to_owned(),
            base_url: base_url_override.unwrap_or_default().trim().to_owned(),
        });
    }
    let (fallback_api_key, fallback_base_url) = if source == "codex" {
        read_codex_credentials()
    } else {
        read_claude_credentials()?
    };

    let api_key = api_key_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
        .unwrap_or(fallback_api_key);
    let base_url = base_url_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
        .unwrap_or(fallback_base_url);

    Ok(ResolvedCliCredentials { api_key, base_url })
}

/// Get real credentials for internal use (e.g. model_list).
pub(crate) fn get_credentials(source: &str) -> (String, String) {
    match resolve_credentials(source, None, None) {
        Ok(credentials) => (credentials.api_key, credentials.base_url),
        Err(_) if source == "codex" => (String::new(), "https://api.openai.com".to_string()),
        Err(_) => (String::new(), "https://api.anthropic.com".to_string()),
    }
}

// ── Internal helpers ──

/// Read Codex config with full source attribution.
fn read_codex_cli_config() -> Result<CliConfig, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let codex_dir = home.join(".codex");
    let toml_path = codex_dir.join("config.toml");
    let auth_path = codex_dir.join("auth.json");

    let toml_cfg: CodexConfig = std::fs::read_to_string(&toml_path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    let auth: CodexAuth = read_json_file(&auth_path).unwrap_or_default();

    // ── Per-file key extraction ──
    // config.toml key (direct field, [provider], or active [model_providers.<name>])
    let active_provider = active_codex_provider(&toml_cfg);
    let toml_key = toml_cfg
        .api_key
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            active_provider
                .as_ref()
                .and_then(|p| p.api_key.clone())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();

    // auth.json key: prefer CODEX_API_KEY, fall back to OPENAI_API_KEY
    let auth_key = auth
        .codex_api_key
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| auth.openai_api_key.clone().filter(|s| !s.is_empty()))
        .unwrap_or_default();

    // ── Final API key: auth.json is primary (it's the dedicated key store),
    //    config.toml key is secondary, env vars are last resort ──
    let (api_key, api_key_source) = if !auth_key.is_empty() {
        (auth_key.clone(), "auth.json".to_string())
    } else if !toml_key.is_empty() {
        (toml_key.clone(), "config.toml".to_string())
    } else if let Some(k) = env::var("CODEX_API_KEY").ok().filter(|v| !v.is_empty()) {
        (k, "env:CODEX_API_KEY".to_string())
    } else if let Some(k) = env::var("OPENAI_API_KEY").ok().filter(|v| !v.is_empty()) {
        (k, "env:OPENAI_API_KEY".to_string())
    } else {
        (String::new(), String::new())
    };

    // ── Base URL: config.toml is primary (it's the dedicated URL store) ──
    let toml_url = toml_cfg
        .base_url
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            active_provider
                .as_ref()
                .and_then(|p| p.base_url.clone())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();

    let (base_url, base_url_source) = if !toml_url.is_empty() {
        (toml_url.clone(), "config.toml".to_string())
    } else if let Some(u) = env::var("OPENAI_BASE_URL").ok().filter(|v| !v.is_empty()) {
        (u, "env:OPENAI_BASE_URL".to_string())
    } else if let Some(u) = env::var("CODEX_BASE_URL").ok().filter(|v| !v.is_empty()) {
        (u, "env:CODEX_BASE_URL".to_string())
    } else {
        ("https://api.openai.com".to_string(), "default".to_string())
    };

    let default_model = toml_cfg.model.unwrap_or_default();

    Ok(CliConfig {
        source: "codex".to_string(),
        api_key_masked: mask_key(&api_key),
        has_api_key: !api_key.is_empty(),
        base_url,
        default_model,
        config_path: toml_path.display().to_string(),
        auth_json_path: auth_path.display().to_string(),
        auth_json_key_masked: mask_key(&auth_key),
        auth_json_has_key: !auth_key.is_empty(),
        config_toml_key_masked: mask_key(&toml_key),
        config_toml_has_key: !toml_key.is_empty(),
        config_toml_url: toml_url,
        api_key_source,
        base_url_source,
    })
}

/// Returns raw (api_key, base_url) for Codex — for internal use only.
pub(crate) fn read_codex_credentials() -> (String, String) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return (String::new(), "https://api.openai.com".to_string()),
    };
    let codex_dir = home.join(".codex");

    let auth: CodexAuth = read_json_file(&codex_dir.join("auth.json")).unwrap_or_default();
    let toml_cfg: CodexConfig = std::fs::read_to_string(codex_dir.join("config.toml"))
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    // API key: auth.json first (primary), then config.toml, then env
    let auth_key = auth
        .codex_api_key
        .filter(|s| !s.is_empty())
        .or_else(|| auth.openai_api_key.filter(|s| !s.is_empty()))
        .unwrap_or_default();
    let active_provider = active_codex_provider(&toml_cfg);
    let toml_key = toml_cfg
        .api_key
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            active_provider
                .as_ref()
                .and_then(|p| p.api_key.clone())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();
    let api_key = if !auth_key.is_empty() {
        auth_key
    } else if !toml_key.is_empty() {
        toml_key
    } else {
        env::var("CODEX_API_KEY")
            .unwrap_or_default()
            .pipe_if_empty(|| env::var("OPENAI_API_KEY").unwrap_or_default())
    };

    // Base URL: config.toml first, then env, then default
    let toml_url = toml_cfg
        .base_url
        .filter(|s| !s.is_empty())
        .or_else(|| {
            active_provider
                .as_ref()
                .and_then(|p| p.base_url.clone())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();
    let base_url = if !toml_url.is_empty() {
        toml_url
    } else {
        env::var("OPENAI_BASE_URL")
            .unwrap_or_default()
            .pipe_if_empty(|| env::var("CODEX_BASE_URL").unwrap_or_default())
            .pipe_if_empty(|| "https://api.openai.com".to_string())
    };

    (api_key, base_url)
}

fn read_claude_credentials() -> Result<(String, String), String> {
    let (api_key, base_url, _, _) = read_claude_config()?;
    Ok((api_key, base_url))
}

/// Helper: if `self` is empty, call `f` and return its result.
trait PipeIfEmpty {
    fn pipe_if_empty(self, f: impl FnOnce() -> String) -> String;
}
impl PipeIfEmpty for String {
    fn pipe_if_empty(self, f: impl FnOnce() -> String) -> String {
        if self.is_empty() {
            f()
        } else {
            self
        }
    }
}

/// Returns (api_key, base_url, default_model, config_path).
fn read_claude_config() -> Result<(String, String, String, String), String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let settings_path = home.join(".claude").join("settings.json");
    let config_path_str = settings_path.display().to_string();

    let settings = read_json_file::<ClaudeSettings>(&settings_path).unwrap_or_default();

    // Shell rc fallback: useful when the process is launched without sourcing shell init files
    // (e.g. Tauri desktop shortcut, systemd service running session-web binary)
    let shell_env = read_shell_env(&home);

    // API key priority: settings.json env → process env → shell rc files
    let api_key = settings
        .env
        .get("ANTHROPIC_AUTH_TOKEN")
        .filter(|s| !s.is_empty())
        .or_else(|| {
            settings
                .env
                .get("ANTHROPIC_API_KEY")
                .filter(|s| !s.is_empty())
        })
        .cloned()
        .or_else(|| {
            env::var("ANTHROPIC_AUTH_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty()))
        .or_else(|| {
            shell_env
                .get("ANTHROPIC_AUTH_TOKEN")
                .filter(|s| !s.is_empty())
                .cloned()
        })
        .or_else(|| {
            shell_env
                .get("ANTHROPIC_API_KEY")
                .filter(|s| !s.is_empty())
                .cloned()
        })
        .unwrap_or_default();

    // Base URL: settings.json env → process env → shell rc files → default
    let base_url = settings
        .env
        .get("ANTHROPIC_BASE_URL")
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| {
            env::var("ANTHROPIC_BASE_URL")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            shell_env
                .get("ANTHROPIC_BASE_URL")
                .filter(|s| !s.is_empty())
                .cloned()
        })
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());

    let default_model = settings.model.unwrap_or_default();

    Ok((api_key, base_url, default_model, config_path_str))
}

/// Parse common shell rc files and extract `export KEY=value` assignments.
/// Only runs on Unix-like systems; returns empty map on Windows.
fn read_shell_env(home: &std::path::Path) -> HashMap<String, String> {
    #[cfg(not(unix))]
    {
        let _ = home;
        HashMap::new()
    }

    #[cfg(unix)]
    {
        let candidates = [
            ".bashrc",
            ".bash_profile",
            ".profile",
            ".zshrc",
            ".zprofile",
        ];

        let mut map = HashMap::new();
        for name in &candidates {
            let path = home.join(name);
            if let Ok(content) = std::fs::read_to_string(&path) {
                parse_shell_exports(&content, &mut map);
            }
        }
        map
    }
}

/// Extract `export KEY=value` (and bare `KEY=value`) lines from shell script content.
/// Handles single-quoted, double-quoted, and unquoted values.
/// Does not evaluate variable references or subshells — purely static parsing.
#[cfg(unix)]
fn parse_shell_exports(content: &str, map: &mut HashMap<String, String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Strip optional leading `export `
        let assignment = if let Some(rest) = trimmed.strip_prefix("export ") {
            rest.trim_start()
        } else {
            trimmed
        };
        // Must contain `=`
        let Some(eq_pos) = assignment.find('=') else {
            continue;
        };
        let key = assignment[..eq_pos].trim();
        // Key must be a valid identifier (letters, digits, underscores, no spaces)
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let raw_value = &assignment[eq_pos + 1..];
        let value = unquote(raw_value);
        // Only keep the first occurrence (earlier rc files take precedence)
        map.entry(key.to_string()).or_insert(value);
    }
}

/// Remove surrounding single or double quotes from a shell value string.
#[cfg(unix)]
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        // Strip inline comment: `value # comment`
        if let Some(idx) = s.find(" #") {
            s[..idx].trim().to_string()
        } else {
            s.to_string()
        }
    }
}

fn mask_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    let len = key.len();
    if len <= 8 {
        return "*".repeat(len);
    }
    let prefix = &key[..3];
    let suffix = &key[len - 4..];
    format!("{}...{}", prefix, suffix)
}

fn read_json_file<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Option<T> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}
