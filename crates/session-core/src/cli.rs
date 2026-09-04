use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// On Windows, suppress the transient cmd console window that would otherwise
/// flash up every time we shell out to `where` / `<cli> --version` from a
/// `windows_subsystem = "windows"` parent. No-op on Unix.
#[cfg(windows)]
fn hide_console(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}
#[cfg(not(windows))]
#[inline]
fn hide_console(cmd: &mut Command) -> &mut Command {
    cmd
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstallation {
    pub path: String,
    pub version: Option<String>,
    pub cli_type: String, // "claude" | "codex" | "omp"
}

/// Normalize a source name to the supported CLI types.
pub fn normalize_source(source: &str) -> Result<&'static str, String> {
    let trimmed = source.trim();
    if trimmed.eq_ignore_ascii_case("claude") {
        Ok("claude")
    } else if trimmed.eq_ignore_ascii_case("codex") {
        Ok("codex")
    } else if trimmed.eq_ignore_ascii_case("omp") || trimmed.eq_ignore_ascii_case("oh-my-pi") {
        Ok("omp")
    } else {
        Err(format!("Unsupported source: {}", source))
    }
}

/// Find a CLI binary path by source name ("claude", "codex", or "omp").
pub fn find_cli(cli_type: &str) -> Result<String, String> {
    match normalize_source(cli_type)? {
        "codex" => find_codex()
            .ok_or_else(|| "Codex CLI not found. Run: npm install -g @openai/codex".to_string()),
        "omp" => find_omp()
            .ok_or_else(|| "Oh My Pi CLI not found. Install it from https://omp.sh".to_string()),
        "claude" => {
            // Claude: try system lookup first, then known paths
            if let Some(path) = which_binary("claude") {
                return Ok(path);
            }
            for candidate in claude_known_paths() {
                if candidate.exists() {
                    return Ok(candidate.to_string_lossy().to_string());
                }
            }
            Err("Claude CLI not found. Run: npm install -g @anthropic-ai/claude-code".to_string())
        }
        _ => unreachable!(),
    }
}

/// Find the Oh My Pi CLI binary.
fn find_omp() -> Option<String> {
    if let Some(path) = which_binary("omp") {
        return Some(path);
    }
    let home = dirs::home_dir()?;
    let candidates = if cfg!(windows) {
        vec![
            home.join("AppData/Local/omp/omp.exe"),
            home.join("AppData/Roaming/npm/omp.cmd"),
            home.join("AppData/Roaming/npm/omp.exe"),
        ]
    } else {
        vec![
            home.join(".local/bin/omp"),
            home.join(".npm-global/bin/omp"),
        ]
    };
    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(|path| path.to_string_lossy().to_string())
}

/// Find the Codex CLI binary path (npm/nvm only).
fn find_codex() -> Option<String> {
    if let Some(path) = which_binary("codex") {
        return Some(path);
    }
    for candidate in codex_npm_nvm_paths() {
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

/// Locate the `node` binary so its parent dir can be added to PATH for child CLIs
/// whose entry script uses `#!/usr/bin/env node`.
pub fn find_node() -> Option<String> {
    if let Some(path) = which_binary("node") {
        return Some(path);
    }

    let home = dirs::home_dir()?;
    let mut candidates: Vec<PathBuf> = Vec::new();

    if cfg!(windows) {
        candidates.push(home.join("AppData/Roaming/npm/node.exe"));
        if let Ok(appdata) = std::env::var("APPDATA") {
            let nvm_win_dir = PathBuf::from(&appdata).join("nvm");
            if nvm_win_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&nvm_win_dir) {
                    for entry in entries.flatten() {
                        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            candidates.push(entry.path().join("node.exe"));
                        }
                    }
                }
            }
        }
    } else {
        let nvm_dir = home.join(".nvm/versions/node");
        if nvm_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
                for entry in entries.flatten() {
                    candidates.push(entry.path().join("bin/node"));
                }
            }
        }
        if let Ok(nvm_dir_env) = std::env::var("NVM_DIR") {
            let nvm_versions = PathBuf::from(&nvm_dir_env).join("versions/node");
            if nvm_versions.exists() && nvm_versions != nvm_dir {
                if let Ok(entries) = std::fs::read_dir(&nvm_versions) {
                    for entry in entries.flatten() {
                        candidates.push(entry.path().join("bin/node"));
                    }
                }
            }
        }
        candidates.push(home.join(".npm-global/bin/node"));
        candidates.push(home.join(".local/bin/node"));
        candidates.push(PathBuf::from("/usr/local/bin/node"));
        candidates.push(PathBuf::from("/opt/homebrew/bin/node"));
        candidates.push(PathBuf::from("/usr/bin/node"));
    }

    candidates
        .into_iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
}

/// Discover installed CLIs (Claude + Codex).
pub fn discover_installations() -> Vec<CliInstallation> {
    let mut installations = Vec::new();

    if let Ok(path) = find_cli("claude") {
        let version = get_cli_version(&path);
        installations.push(CliInstallation {
            path,
            version,
            cli_type: "claude".to_string(),
        });
    }

    if let Some(path) = find_codex() {
        let version = get_cli_version(&path);
        installations.push(CliInstallation {
            path,
            version,
            cli_type: "codex".to_string(),
        });
    }
    if let Some(path) = find_omp() {
        let version = get_cli_version(&path);
        installations.push(CliInstallation {
            path,
            version,
            cli_type: "omp".to_string(),
        });
    }

    installations
}

/// Use `where` (Windows) or `which` (Unix) to find a binary.
fn which_binary(name: &str) -> Option<String> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("where");
        cmd.arg(name);
        hide_console(&mut cmd);
        let result = cmd.output();
        if let Ok(output) = result {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let lower = trimmed.to_lowercase();
                    if lower.ends_with(".exe") || lower.ends_with(".cmd") {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        // First try direct which
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
        // Fallback: run via login shell to inherit nvm / asdf PATH.
        // Try multiple invocation styles:
        //   1. `zsh -l -c` — sources .zprofile (standard login shell)
        //   2. `zsh -c '. ~/.zprofile 2>/dev/null; . ~/.zshrc 2>/dev/null; which X'`
        //      — also sources .zshrc where Homebrew-nvm users typically init nvm
        //   3. Same for bash (.bash_profile / .bashrc)
        let shell_cmds: &[(&str, &[&str])] = &[("zsh", &["-l", "-c"]), ("bash", &["-l", "-c"])];
        for (shell, args) in shell_cmds {
            let cmd = format!("which {name}");
            if let Ok(output) = Command::new(shell).args(*args).arg(&cmd).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() && !path.contains("not found") {
                        return Some(path);
                    }
                }
            }
        }
        // Also try sourcing rc files explicitly (covers Homebrew nvm in .zshrc / .bashrc)
        let rc_cmds: &[(&str, &str)] = &[
            ("zsh", ". ~/.zprofile 2>/dev/null; . ~/.zshrc 2>/dev/null"),
            (
                "bash",
                ". ~/.bash_profile 2>/dev/null; . ~/.bashrc 2>/dev/null",
            ),
        ];
        for (shell, source_cmds) in rc_cmds {
            let cmd = format!("{source_cmds}; which {name}");
            if let Ok(output) = Command::new(shell).args(["-c", &cmd]).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() && !path.contains("not found") {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}

/// Known installation paths for Claude CLI.
fn claude_known_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let home = dirs::home_dir();

    if let Some(ref home) = home {
        // npm global (Windows: .cmd shim; Unix: plain binary)
        if cfg!(windows) {
            paths.push(home.join("AppData/Roaming/npm/claude.cmd"));
            paths.push(home.join("AppData/Roaming/npm/claude.exe"));
        } else {
            paths.push(home.join(".npm-global/bin/claude"));
        }

        // NVM (Unix/Mac): ~/.nvm/versions/node/{version}/bin/claude
        let nvm_dir = home.join(".nvm/versions/node");
        if nvm_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
                for entry in entries.flatten() {
                    let bin_dir = entry.path().join("bin");
                    if cfg!(windows) {
                        paths.push(bin_dir.join("claude.cmd"));
                        paths.push(bin_dir.join("claude.exe"));
                    } else {
                        paths.push(bin_dir.join("claude"));
                    }
                }
            }
        }

        // nvm-windows: %APPDATA%\nvm\{version}\claude.cmd
        #[cfg(windows)]
        if let Ok(appdata) = std::env::var("APPDATA") {
            let nvm_win_dir = PathBuf::from(&appdata).join("nvm");
            if nvm_win_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&nvm_win_dir) {
                    for entry in entries.flatten() {
                        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            paths.push(entry.path().join("claude.cmd"));
                            paths.push(entry.path().join("claude.exe"));
                        }
                    }
                }
            }
        }

        // NVM_DIR env var (nvm sets this; works even if not in ~/.nvm)
        #[cfg(not(windows))]
        if let Ok(nvm_dir_env) = std::env::var("NVM_DIR") {
            let nvm_versions = PathBuf::from(&nvm_dir_env).join("versions/node");
            if nvm_versions.exists() && nvm_versions != nvm_dir {
                if let Ok(entries) = std::fs::read_dir(&nvm_versions) {
                    for entry in entries.flatten() {
                        paths.push(entry.path().join("bin").join("claude"));
                    }
                }
            }
        }

        // Homebrew nvm alternative NVM_DIR (macOS)
        #[cfg(target_os = "macos")]
        for brew_nvm in &[
            "/opt/homebrew/var/nvm/versions/node",
            "/usr/local/var/nvm/versions/node",
        ] {
            let brew_nvm_path = PathBuf::from(brew_nvm);
            if brew_nvm_path.exists() && brew_nvm_path != nvm_dir {
                if let Ok(entries) = std::fs::read_dir(&brew_nvm_path) {
                    for entry in entries.flatten() {
                        paths.push(entry.path().join("bin").join("claude"));
                    }
                }
            }
        }

        // Local bin (Unix)
        if !cfg!(windows) {
            paths.push(home.join(".local/bin/claude"));
        }

        // Bun global
        if cfg!(windows) {
            paths.push(home.join(".bun/bin/claude.exe"));
        } else {
            paths.push(home.join(".bun/bin/claude"));
        }
    }

    // System paths (Unix)
    #[cfg(not(windows))]
    {
        paths.push(PathBuf::from("/usr/local/bin/claude"));
        paths.push(PathBuf::from("/opt/homebrew/bin/claude"));
    }

    paths
}

/// Known npm/nvm-only paths for Codex CLI (codex is npm-only, no bun/brew support).
fn codex_npm_nvm_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let home = dirs::home_dir();

    if let Some(ref home) = home {
        // npm global
        if cfg!(windows) {
            paths.push(home.join("AppData/Roaming/npm/codex.cmd"));
            paths.push(home.join("AppData/Roaming/npm/codex.exe"));
        } else {
            paths.push(home.join(".npm-global/bin/codex"));
        }

        // NVM (Unix/Mac): ~/.nvm/versions/node/{version}/bin/codex
        let nvm_dir = home.join(".nvm/versions/node");
        if nvm_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
                for entry in entries.flatten() {
                    let bin_dir = entry.path().join("bin");
                    if cfg!(windows) {
                        paths.push(bin_dir.join("codex.cmd"));
                        paths.push(bin_dir.join("codex.exe"));
                    } else {
                        paths.push(bin_dir.join("codex"));
                    }
                }
            }
        }

        // nvm-windows: %APPDATA%\nvm\{version}\codex.cmd
        #[cfg(windows)]
        if let Ok(appdata) = std::env::var("APPDATA") {
            let nvm_win_dir = PathBuf::from(&appdata).join("nvm");
            if nvm_win_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&nvm_win_dir) {
                    for entry in entries.flatten() {
                        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            paths.push(entry.path().join("codex.cmd"));
                            paths.push(entry.path().join("codex.exe"));
                        }
                    }
                }
            }
        }

        // NVM_DIR env var (Unix/Mac)
        #[cfg(not(windows))]
        if let Ok(nvm_dir_env) = std::env::var("NVM_DIR") {
            let nvm_versions = PathBuf::from(&nvm_dir_env).join("versions/node");
            if nvm_versions.exists() && nvm_versions != nvm_dir {
                if let Ok(entries) = std::fs::read_dir(&nvm_versions) {
                    for entry in entries.flatten() {
                        paths.push(entry.path().join("bin").join("codex"));
                    }
                }
            }
        }

        // Homebrew nvm alternative NVM_DIR: /opt/homebrew/var/nvm (Apple Silicon)
        // and /usr/local/var/nvm (Intel Mac). Some users set NVM_DIR to these paths.
        #[cfg(target_os = "macos")]
        for brew_nvm in &[
            "/opt/homebrew/var/nvm/versions/node",
            "/usr/local/var/nvm/versions/node",
        ] {
            let brew_nvm_path = PathBuf::from(brew_nvm);
            if brew_nvm_path.exists() && brew_nvm_path != nvm_dir {
                if let Ok(entries) = std::fs::read_dir(&brew_nvm_path) {
                    for entry in entries.flatten() {
                        paths.push(entry.path().join("bin").join("codex"));
                    }
                }
            }
        }
    }

    paths
}

/// Get CLI version by running `<cli> --version`.
fn get_cli_version(path: &str) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("--version");
    hide_console(&mut cmd);
    let output = cmd.output().ok()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout.trim().to_string();
        if version.is_empty() {
            None
        } else {
            Some(version)
        }
    } else {
        None
    }
}
