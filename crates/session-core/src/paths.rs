//! Path-validation helpers shared between Tauri commands and the web server.
//!
//! Every API entry that accepts a user-supplied session file path *must* run
//! it through [`validate_session_file`] before reading or modifying the file.
//! The validation rejects:
//!   - non-existent or non-`.jsonl` paths
//!   - paths outside the source's allowed root
//!     (`~/.claude/projects/`, `$CODEX_HOME/sessions/` or
//!     `$CODEX_HOME/archived_sessions/`, or `~/.grok/sessions/`)
//!   - paths with the wrong layout (e.g. a Codex rollout file not under
//!     `<year>/<month>/<day>/`)
//!
//! Both backends call into here so neither one can drift past the other.
//! Without this, a path like `~/.ssh/id_rsa` could be passed in and acted on.

use std::path::{Component, Path, PathBuf};

use crate::parser::path_encoder::get_projects_dir;
use crate::provider::codex;
use crate::provider::grok;
use crate::provider::omp;

/// A supported session source carried over the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionSourceKind {
    Claude,
    Codex,
    Grok,
    Omp,
}

impl SessionSourceKind {
    pub fn parse(source: &str) -> Result<Self, String> {
        match source {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "grok" => Ok(Self::Grok),
            "omp" => Ok(Self::Omp),
            _ => Err(format!("Unknown source: {}", source)),
        }
    }
}

fn canonicalize_dir(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve {}: {}", label, e))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", label));
    }
    Ok(canonical)
}

fn canonical_claude_root() -> Result<PathBuf, String> {
    let path =
        get_projects_dir().ok_or_else(|| "Could not find Claude projects directory".to_string())?;
    canonicalize_dir(path, "Claude projects directory")
}

fn canonical_codex_root() -> Result<PathBuf, String> {
    let path = codex::get_sessions_dir()
        .ok_or_else(|| "Could not find Codex sessions directory".to_string())?;
    canonicalize_dir(path, "Codex sessions directory")
}

fn canonical_codex_archived_root() -> Option<PathBuf> {
    let root = codex::get_sessions_dir()?
        .parent()?
        .join("archived_sessions");
    root.canonicalize().ok().filter(|path| path.is_dir())
}

fn canonical_grok_root() -> Result<PathBuf, String> {
    let path = grok::get_sessions_dir()
        .ok_or_else(|| "Could not find Grok sessions directory".to_string())?;
    canonicalize_dir(path, "Grok sessions directory")
}

fn canonical_omp_root() -> Result<PathBuf, String> {
    let path = omp::get_sessions_dir()
        .ok_or_else(|| "Could not find OMP sessions directory".to_string())?;
    canonicalize_dir(path, "OMP sessions directory")
}

fn validate_claude_layout(path: &Path, base: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(base)
        .map_err(|_| "Session file is outside the Claude projects directory".to_string())?;
    if relative.components().count() != 2 {
        return Err("Claude session file must live directly under a project directory".to_string());
    }
    Ok(())
}

fn validate_codex_layout(
    path: &Path,
    base: Option<&Path>,
    archived: Option<&Path>,
) -> Result<(), String> {
    let relative = if let Some(relative) = base.and_then(|root| path.strip_prefix(root).ok()) {
        if relative.components().count() != 4 {
            return Err(
                "Codex session file must live under sessions/<year>/<month>/<day>/".to_string(),
            );
        }
        relative
    } else if let Some(archived_root) = archived {
        path.strip_prefix(archived_root)
            .map_err(|_| "Session file is outside the Codex sessions directory".to_string())?
    } else {
        return Err("Session file is outside the Codex sessions directory".to_string());
    };
    let components: Vec<_> = relative.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Codex session file must live under a Codex session root".to_string());
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid session file name".to_string())?;
    if !file_name.starts_with("rollout-") {
        return Err("Codex session file name must start with 'rollout-'".to_string());
    }
    Ok(())
}

fn validate_grok_layout(path: &Path, base: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(base)
        .map_err(|_| "Session file is outside the Grok sessions directory".to_string())?;
    let components: Vec<_> = relative.components().collect();
    if components.len() != 3
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.file_name().and_then(|name| name.to_str()) != Some("chat_history.jsonl")
    {
        return Err(
            "Grok session file must be sessions/<encoded-cwd>/<session-id>/chat_history.jsonl"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_omp_layout(path: &Path, base: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(base)
        .map_err(|_| "Session file is outside the OMP sessions directory".to_string())?;
    let components: Vec<_> = relative.components().collect();
    let valid_components = match components.as_slice() {
        [Component::Normal(_)] => true,
        [Component::Normal(parent), Component::Normal(_)] => {
            !base.join(parent).with_extension("jsonl").is_file()
        }
        _ => false,
    };
    if !valid_components {
        return Err(
            "OMP session file must live at sessions/<session>.jsonl or sessions/<project>/<session>.jsonl"
                .to_string(),
        );
    }
    if omp::extract_session_meta(path).is_none() {
        return Err("OMP session file has an invalid session header".to_string());
    }
    Ok(())
}

/// Canonicalize and validate a user-supplied session file path. Returns the
/// canonical path on success; returns an error if anything looks suspicious.
pub fn validate_session_file(source: &str, file_path: &str) -> Result<PathBuf, String> {
    if file_path.trim().is_empty() {
        return Err("Session file path is required".to_string());
    }
    let kind = SessionSourceKind::parse(source)?;
    let requested = PathBuf::from(file_path);
    let canonical = requested
        .canonicalize()
        .map_err(|e| format!("Failed to resolve session file: {}", e))?;

    if !canonical.is_file() {
        return Err(format!("Session file not found: {}", file_path));
    }
    if canonical.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
        return Err("Session file must be a .jsonl file".to_string());
    }

    match kind {
        SessionSourceKind::Claude => {
            let base = canonical_claude_root()?;
            validate_claude_layout(&canonical, &base)?;
        }
        SessionSourceKind::Codex => {
            let base = canonical_codex_root().ok();
            let archived = canonical_codex_archived_root();
            if base.is_none() && archived.is_none() {
                return Err("Could not find Codex sessions directory".to_string());
            }
            validate_codex_layout(&canonical, base.as_deref(), archived.as_deref())?;
        }
        SessionSourceKind::Grok => {
            let base = canonical_grok_root()?;
            validate_grok_layout(&canonical, &base)?;
        }
        SessionSourceKind::Omp => {
            let base = canonical_omp_root()?;
            validate_omp_layout(&canonical, &base)?;
        }
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ai-session-viewer-paths-{unique}"))
    }

    #[test]
    fn omp_layout_allows_project_sessions_and_rejects_artifacts() {
        let root = temporary_dir();
        let artifact_dir = root.join("session");
        fs::create_dir_all(&artifact_dir).unwrap();
        let valid = root.join("session.jsonl");
        fs::write(
            &valid,
            r#"{"type":"session","id":"session","cwd":"/workspace"}"#,
        )
        .unwrap();
        let project_dir = root.join("project");
        fs::create_dir_all(&project_dir).unwrap();
        let project_session = project_dir.join("project-session.jsonl");
        fs::write(
            &project_session,
            r#"{"type":"session","id":"project-session","cwd":"/workspace"}"#,
        )
        .unwrap();
        let artifact_child = artifact_dir.join("child.jsonl");
        fs::write(
            &artifact_child,
            r#"{"type":"session","id":"child","cwd":"/workspace"}"#,
        )
        .unwrap();
        let invalid = root.join("invalid.jsonl");
        fs::write(&invalid, r#"{"type":"session","id":"missing-cwd"}"#).unwrap();

        assert!(validate_omp_layout(&valid, &root).is_ok());
        assert!(validate_omp_layout(&project_session, &root).is_ok());
        assert!(validate_omp_layout(&artifact_child, &root).is_err());
        assert!(validate_omp_layout(&invalid, &root).is_err());

        let _ = fs::remove_dir_all(root);
    }
}
