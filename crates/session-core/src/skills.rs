//! Discovery of Claude Code skills for the viewer.
//!
//! Scans three sources:
//!   - **global**  — `~/.claude/skills/<skill>/SKILL.md` (entries may be symlinks)
//!   - **plugin**  — `~/.claude/plugins/{marketplaces,cache}/**/<skill>/SKILL.md`
//!   - **project** — `<project_path>/.claude/skills/<skill>/SKILL.md`
//!
//! Each `SKILL.md` carries a YAML frontmatter block delimited by `---` lines;
//! we extract `name` and `description` from it (ignoring all other keys).

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::models::skill::{ImportResult, SkillEntry, SkillsResult};
use crate::parser::path_encoder::get_claude_home;

/// Only the two frontmatter fields we care about. serde ignores every other
/// key (including nested blocks like `metadata:` and hyphenated ones like
/// `allowed-tools:`), so this stays robust against arbitrary skill metadata.
#[derive(serde::Deserialize, Default)]
struct Frontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// How deep to recurse when hunting for `SKILL.md` under the plugins tree.
/// Marketplace layouts nest a few levels (`<marketplace>/skills/<skill>/`),
/// cache snapshots add a version segment — 6 is comfortably beyond both.
const PLUGIN_WALK_MAX_DEPTH: usize = 6;

/// Extract the raw YAML between the leading `---` and the next `---` line.
/// Returns None when the file doesn't start with a frontmatter block.
fn extract_frontmatter(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut yaml = String::new();
    for line in lines {
        if line.trim() == "---" {
            return Some(yaml);
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    None
}

/// Read a single skill directory into a [`SkillEntry`]. Returns None when the
/// directory has no readable `SKILL.md`.
fn read_skill(dir: &Path, scope: &str) -> Option<SkillEntry> {
    let skill_md = dir.join("SKILL.md");
    let raw = fs::read_to_string(&skill_md).ok()?;
    let content = raw.strip_prefix('\u{feff}').unwrap_or(&raw);

    let fm = extract_frontmatter(content)
        .and_then(|yaml| serde_yml::from_str::<Frontmatter>(&yaml).ok())
        .unwrap_or_default();

    let slug = dir.file_name()?.to_string_lossy().to_string();
    let name = fm
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| slug.clone());
    let description = fm.description.unwrap_or_default().trim().to_string();
    let is_symlink = fs::symlink_metadata(dir)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    Some(SkillEntry {
        name,
        description,
        path: skill_md.to_string_lossy().to_string(),
        scope: scope.to_string(),
        source_label: None,
        slug,
        is_symlink,
    })
}

/// Scan a "skills root" — a directory whose immediate children are skill dirs.
/// `is_dir()` follows symlinks, so symlinked skills (e.g. global `lark-*`) are
/// picked up the same as real directories.
fn scan_skills_root(root: &Path, scope: &str) -> Vec<SkillEntry> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(root) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(skill) = read_skill(&path, scope) {
                out.push(skill);
            }
        }
    }
    out
}

/// First path component under `base_root` — used as the marketplace label.
fn marketplace_label(base_root: &Path, skill_dir: &Path) -> Option<String> {
    skill_dir
        .strip_prefix(base_root)
        .ok()
        .and_then(|rel| rel.components().next())
        .and_then(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
}

/// Recursively walk `dir` looking for skill directories (those containing a
/// `SKILL.md`). Dedups by lowercased skill name via `seen_names` so the same
/// skill present in both the marketplace clone and its cache snapshot is only
/// listed once.
fn walk_for_plugin_skills(
    base_root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<SkillEntry>,
    seen_names: &mut HashSet<String>,
) {
    if depth > PLUGIN_WALK_MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("SKILL.md").is_file() {
            // This is a skill dir; don't recurse further into it.
            if let Some(mut skill) = read_skill(&path, "plugin") {
                if seen_names.insert(skill.name.to_lowercase()) {
                    skill.source_label = marketplace_label(base_root, &path);
                    out.push(skill);
                }
            }
            continue;
        }
        walk_for_plugin_skills(base_root, &path, depth + 1, out, seen_names);
    }
}

/// Scan plugin / marketplace skills under `~/.claude/plugins/`.
fn scan_plugin_skills() -> Vec<SkillEntry> {
    let Some(home) = get_claude_home() else {
        return Vec::new();
    };
    let plugins = home.join("plugins");
    let mut out = Vec::new();
    let mut seen_names = HashSet::new();
    // marketplaces first so its label wins over the cache snapshot on dedup.
    for sub in ["marketplaces", "cache"] {
        let root = plugins.join(sub);
        if root.is_dir() {
            walk_for_plugin_skills(&root, &root, 0, &mut out, &mut seen_names);
        }
    }
    out
}

fn sort_by_name(skills: &mut [SkillEntry]) {
    skills.sort_by_key(|s| s.name.to_lowercase());
}

/// Scan all skill sources. `project_path` is the real filesystem path of the
/// project whose `<path>/.claude/skills/` should also be scanned; pass None to
/// scan only global + plugin skills.
pub fn scan_skills(project_path: Option<String>) -> Result<SkillsResult, String> {
    let mut result = SkillsResult::default();

    if let Some(home) = get_claude_home() {
        result.global = scan_skills_root(&home.join("skills"), "global");
    }

    result.plugin = scan_plugin_skills();

    if let Some(pp) = project_path {
        let trimmed = pp.trim();
        if !trimmed.is_empty() {
            let root = Path::new(trimmed).join(".claude").join("skills");
            result.project = scan_skills_root(&root, "project");
            result.project_path = Some(trimmed.to_string());
        }
    }

    sort_by_name(&mut result.global);
    sort_by_name(&mut result.plugin);
    sort_by_name(&mut result.project);

    Ok(result)
}

fn path_has_no_parent_traversal(p: &Path) -> bool {
    !p.components().any(|c| matches!(c, Component::ParentDir))
}

fn is_under_claude_home(p: &Path) -> bool {
    get_claude_home()
        .map(|home| p.starts_with(home))
        .unwrap_or(false)
}

/// True if the path contains a consecutive `.claude/skills` segment pair
/// (covers global and project skill files).
fn has_claude_skills_segment(p: &Path) -> bool {
    let names: Vec<&str> = p
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    names
        .windows(2)
        .any(|w| w[0] == ".claude" && w[1] == "skills")
}

/// Read the full text of a `SKILL.md` for the detail view.
///
/// The path is supplied by the client, so we validate it before reading:
///   - no `..` traversal components
///   - the file name must be `SKILL.md`
///   - it must live under `~/.claude/` (global + plugins) **or** contain a
///     `.claude/skills` segment (project skills)
///
/// We deliberately validate the *requested* path rather than its canonical form
/// so symlinked global skills (whose targets live outside `~/.claude`, e.g.
/// `~/.agents/skills/`) still resolve. `..` is rejected, so this can't be used
/// to escape the allowed roots.
pub fn read_skill_content(path: &str) -> Result<String, String> {
    let requested = PathBuf::from(path);

    if !path_has_no_parent_traversal(&requested) {
        return Err("Invalid skill path".to_string());
    }
    if requested.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
        return Err("Not a SKILL.md file".to_string());
    }
    if !is_under_claude_home(&requested) && !has_claude_skills_segment(&requested) {
        return Err("Skill file is outside allowed directories".to_string());
    }
    if !requested.is_file() {
        return Err(format!("Skill file not found: {}", path));
    }

    fs::read_to_string(&requested).map_err(|e| format!("Failed to read skill file: {}", e))
}

// ---------------------------------------------------------------------------
// Delete / import (global + project scope only — plugin skills are managed by
// the plugin system and are never touched here).
// ---------------------------------------------------------------------------

/// Resolve the skills root directory for a writable scope.
fn skills_root_for_scope(scope: &str, project_path: Option<&str>) -> Result<PathBuf, String> {
    match scope {
        "global" => get_claude_home()
            .map(|h| h.join("skills"))
            .ok_or_else(|| "无法定位 ~/.claude 目录".to_string()),
        "project" => {
            let pp = project_path
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "项目作用域需要项目路径".to_string())?;
            Ok(Path::new(pp).join(".claude").join("skills"))
        }
        other => Err(format!("不支持的作用域: {}", other)),
    }
}

/// A safe skill slug is a single non-empty path component (no separators / `..`).
fn slug_is_safe(slug: &str) -> bool {
    let mut comps = Path::new(slug).components();
    matches!(comps.next(), Some(Component::Normal(_))) && comps.next().is_none()
}

fn append_skill_files(
    zip: &mut zip::ZipWriter<std::io::Cursor<Vec<u8>>>,
    root: &Path,
    dir: &Path,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("读取 skill 目录失败: {}", e))?;
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in entries.flatten() {
        let path = entry.path();
        let meta =
            fs::symlink_metadata(&path).map_err(|e| format!("读取 skill 文件信息失败: {}", e))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            append_skill_files(zip, root, &path)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }

        let name = path
            .strip_prefix(root)
            .map_err(|e| format!("生成 skill 相对路径失败: {}", e))?
            .to_string_lossy()
            .replace('\\', "/");
        zip.start_file(name, options)
            .map_err(|e| format!("创建 skill 压缩包失败: {}", e))?;
        let mut file = fs::File::open(&path).map_err(|e| format!("读取 skill 文件失败: {}", e))?;
        std::io::copy(&mut file, &mut *zip).map_err(|e| format!("写入 skill 压缩包失败: {}", e))?;
    }
    Ok(())
}

fn archive_skill_dir(target: &Path) -> Result<Vec<u8>, String> {
    if !target.is_dir() || !target.join("SKILL.md").is_file() {
        return Err("skill 目录无效或缺少 SKILL.md".to_string());
    }
    let root = fs::canonicalize(target).map_err(|e| format!("无法解析 skill 目录: {}", e))?;
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    append_skill_files(&mut zip, &root, &root)?;
    zip.finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|e| format!("完成 skill 压缩包失败: {}", e))
}

fn archive_file_map(archive: &[u8]) -> Result<std::collections::BTreeMap<String, Vec<u8>>, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))
        .map_err(|e| format!("无法校验 skill 压缩包: {}", e))?;
    let mut files = std::collections::BTreeMap::new();
    for index in 0..zip.len() {
        let mut file = zip
            .by_index(index)
            .map_err(|e| format!("读取 skill 校验文件失败: {}", e))?;
        if file.is_dir() {
            continue;
        }
        let Some(path) = file.enclosed_name() else {
            return Err("skill 压缩包包含非法路径".to_string());
        };
        let name = path.to_string_lossy().replace('\\', "/");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| format!("读取 skill 校验内容失败: {}", e))?;
        if files.insert(name.clone(), bytes).is_some() {
            return Err(format!("skill 压缩包包含重复文件: {}", name));
        }
    }
    Ok(files)
}

/// Export one writable skill as a root-level ZIP archive. A top-level
/// symlink is materialized, while nested symlinks are skipped.
pub fn export_skill_to_bytes(
    scope: &str,
    project_path: Option<&str>,
    slug: &str,
) -> Result<Vec<u8>, String> {
    if !slug_is_safe(slug) {
        return Err(format!("非法的 skill 名称: {}", slug));
    }
    archive_skill_dir(&skills_root_for_scope(scope, project_path)?.join(slug))
}

fn persist_skill_backup(slug: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = dirs::config_dir()
        .ok_or_else(|| "无法定位应用配置目录".to_string())?
        .join("ai-session-viewer")
        .join("skill-backups")
        .join(stamp.to_string());
    fs::create_dir_all(&dir).map_err(|e| format!("创建 skill 备份目录失败: {}", e))?;
    let path = dir.join(format!("{}.zip", slug));
    fs::write(&path, bytes).map_err(|e| format!("写入 skill 备份失败: {}", e))?;
    Ok(path)
}

/// Apply a single global skill received from another node. Existing content
/// is archived first; failed extraction removes partial output and restores
/// the backup through the same validated importer.
pub fn apply_synced_global_skill(
    archive: &[u8],
    slug: &str,
    overwrite: bool,
) -> Result<ImportResult, String> {
    if !slug_is_safe(slug) {
        return Err(format!("非法的 skill 名称: {}", slug));
    }
    let expected_files = archive_file_map(archive)?;
    if !expected_files.contains_key("SKILL.md")
        || expected_files
            .keys()
            .any(|path| path != "SKILL.md" && path.ends_with("/SKILL.md"))
    {
        return Err("同步包必须只包含一个根级 SKILL.md".to_string());
    }
    let root = skills_root_for_scope("global", None)?;
    let target = root.join(slug);
    let previous = if fs::symlink_metadata(&target).is_ok() {
        if !overwrite {
            return Err(format!("目标已存在: {}", slug));
        }
        let bytes = archive_skill_dir(&target)?;
        persist_skill_backup(slug, &bytes)?;
        Some(bytes)
    } else {
        None
    };

    let archive_name = format!("{}.zip", slug);
    let detail =
        match import_skills_from_bytes(archive, "global", None, overwrite, Some(&archive_name)) {
            Ok(result)
                if result.errors.is_empty() && result.imported.iter().any(|item| item == slug) =>
            {
                match archive_skill_dir(&target).and_then(|bytes| archive_file_map(&bytes)) {
                    Ok(actual_files) if actual_files == expected_files => return Ok(result),
                    Ok(_) => "写入后校验不一致".to_string(),
                    Err(error) => error,
                }
            }
            Ok(result) if result.errors.is_empty() => "同步结果未包含目标 Skill".to_string(),
            Ok(result) => result.errors.join("; "),
            Err(error) => error,
        };

    if fs::symlink_metadata(&target).is_ok() {
        let _ = remove_skill_entry(&target);
    }
    if let Some(previous) = previous {
        let restored =
            import_skills_from_bytes(&previous, "global", None, false, Some(&archive_name));
        match restored {
            Ok(restored)
                if restored.errors.is_empty()
                    && restored.imported.iter().any(|item| item == slug) => {}
            Ok(restored) => {
                return Err(format!(
                    "同步失败且恢复备份不完整: {}",
                    restored.errors.join("; ")
                ));
            }
            Err(error) => {
                return Err(format!("同步失败且恢复备份失败: {}", error));
            }
        }
    }
    Err(format!("同步 skill 失败: {}", detail))
}

/// Remove a symlink (the link itself, never its target).
fn remove_symlink(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        // On Windows a *directory* symlink must be removed with remove_dir.
        let is_dir = fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false);
        let res = if is_dir {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        };
        res.map_err(|e| format!("移除符号链接失败: {}", e))
    }
    #[cfg(not(windows))]
    {
        fs::remove_file(path).map_err(|e| format!("移除符号链接失败: {}", e))
    }
}

/// Remove a skill directory entry, treating symlinks as links (not following).
fn remove_skill_entry(target: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(target).map_err(|e| format!("无法访问目标: {}", e))?;
    if meta.file_type().is_symlink() {
        remove_symlink(target)
    } else if meta.is_dir() {
        fs::remove_dir_all(target).map_err(|e| format!("删除 skill 失败: {}", e))
    } else {
        fs::remove_file(target).map_err(|e| format!("删除 skill 失败: {}", e))
    }
}

/// Permanently delete a global or project skill by slug. Symlinked skills have
/// only their link removed (the target is preserved).
pub fn delete_skill(scope: &str, project_path: Option<&str>, slug: &str) -> Result<(), String> {
    if !slug_is_safe(slug) {
        return Err(format!("非法的 skill 名称: {}", slug));
    }
    let root = skills_root_for_scope(scope, project_path)?;
    let target = root.join(slug);
    if fs::symlink_metadata(&target).is_err() {
        return Err(format!("skill 不存在: {}", slug));
    }
    remove_skill_entry(&target)
}

/// Sanitize a name into a single safe directory slug.
fn sanitize_slug(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' || c == '\0' {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "imported-skill".to_string()
    } else {
        cleaned.to_string()
    }
}

/// Import one or more skills from a zip archive into the given scope.
///
/// A "skill" is any directory in the archive containing a `SKILL.md` (a
/// top-level `SKILL.md` makes the whole archive one skill, named after
/// `archive_name`). Each entry is assigned to the deepest matching skill so
/// nested layouts extract correctly. `enclosed_name()` guards against zip-slip.
pub fn import_skills_from_bytes(
    archive: &[u8],
    scope: &str,
    project_path: Option<&str>,
    overwrite: bool,
    archive_name: Option<&str>,
) -> Result<ImportResult, String> {
    let root = skills_root_for_scope(scope, project_path)?;

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))
        .map_err(|e| format!("无法打开压缩包: {}", e))?;

    // 1. Discover skill prefixes (parent dirs of each SKILL.md; "" = archive root).
    let mut prefixes: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let file = zip
            .by_index(i)
            .map_err(|e| format!("读取压缩包出错: {}", e))?;
        let Some(path) = file.enclosed_name() else {
            continue;
        };
        if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            let prefix = path
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
    }
    if prefixes.is_empty() {
        return Err("压缩包中未找到 SKILL.md".to_string());
    }

    let archive_stem = archive_name
        .and_then(|n| Path::new(n).file_stem().and_then(|s| s.to_str()))
        .map(sanitize_slug)
        .unwrap_or_else(|| "imported-skill".to_string());

    // (prefix, slug), longest prefix first so deepest skill wins on assignment.
    let mut skills: Vec<(String, String)> = prefixes
        .iter()
        .map(|prefix| {
            let slug = if prefix.is_empty() {
                archive_stem.clone()
            } else {
                sanitize_slug(prefix.rsplit('/').next().unwrap_or(prefix))
            };
            (prefix.clone(), slug)
        })
        .collect();
    skills.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));

    let mut result = ImportResult::default();

    // Resolve conflicts up front; `active` holds the skills we will extract.
    let mut active: Vec<(String, String, PathBuf)> = Vec::new();
    for (prefix, slug) in &skills {
        let target_dir = root.join(slug);
        if target_dir.exists() {
            if overwrite {
                if let Err(e) = remove_skill_entry(&target_dir) {
                    result.errors.push(format!("{}: 无法覆盖: {}", slug, e));
                    continue;
                }
            } else {
                result.skipped.push(slug.clone());
                continue;
            }
        }
        active.push((prefix.clone(), slug.clone(), target_dir));
    }

    if active.is_empty() {
        return Ok(result);
    }

    fs::create_dir_all(&root).map_err(|e| format!("无法创建 skills 目录: {}", e))?;

    // 2. Extract every file under an active prefix.
    for i in 0..zip.len() {
        let mut file = zip
            .by_index(i)
            .map_err(|e| format!("读取压缩包出错: {}", e))?;
        if file.is_dir() {
            continue;
        }
        // Scope the (possibly borrowing) enclosed_name() so the immutable
        // borrow of `file` ends before the later mutable read_to_end().
        let norm = match file.enclosed_name() {
            Some(p) => p.to_string_lossy().replace('\\', "/"),
            None => continue,
        };

        let matched = active
            .iter()
            .find(|(prefix, _, _)| prefix.is_empty() || norm.starts_with(&format!("{}/", prefix)));
        let Some((prefix, slug, target_dir)) = matched else {
            continue;
        };

        let rel = if prefix.is_empty() {
            norm.clone()
        } else {
            norm[prefix.len() + 1..].to_string()
        };
        if rel.is_empty() {
            continue;
        }

        let dest = target_dir.join(&rel);
        if !dest.starts_with(target_dir) {
            result
                .errors
                .push(format!("{}: 跳过非法路径 {}", slug, rel));
            continue;
        }
        if let Some(parent) = dest.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                result.errors.push(format!("{}: {}", slug, e));
                continue;
            }
        }
        let mut buf = Vec::new();
        if let Err(e) = file.read_to_end(&mut buf) {
            result.errors.push(format!("{}: 读取失败: {}", slug, e));
            continue;
        }
        if let Err(e) = fs::write(&dest, &buf) {
            result.errors.push(format!("{}: 写入失败: {}", slug, e));
            continue;
        }
    }

    for (_, slug, _) in &active {
        if !result.imported.contains(slug) {
            result.imported.push(slug.clone());
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_frontmatter_basic() {
        let md = "---\nname: foo\ndescription: bar\n---\n# Body\n";
        let yaml = extract_frontmatter(md).unwrap();
        let fm: Frontmatter = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(fm.name.as_deref(), Some("foo"));
        assert_eq!(fm.description.as_deref(), Some("bar"));
    }

    #[test]
    fn extract_frontmatter_ignores_extra_and_nested_keys() {
        let md = "---\nname: foo\nversion: 1.0.0\ndescription: \"a: colon, and quotes\"\nmetadata:\n  requires:\n    bins: [\"x\"]\nallowed-tools: Bash, Read\n---\nbody";
        let yaml = extract_frontmatter(md).unwrap();
        let fm: Frontmatter = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(fm.name.as_deref(), Some("foo"));
        assert_eq!(fm.description.as_deref(), Some("a: colon, and quotes"));
    }

    #[test]
    fn extract_frontmatter_none_without_block() {
        assert!(extract_frontmatter("# Just a heading\n").is_none());
    }

    #[test]
    fn rejects_parent_traversal() {
        assert!(read_skill_content("~/.claude/skills/../../secret/SKILL.md").is_err());
    }

    #[test]
    fn rejects_non_skill_filename() {
        let err = read_skill_content("/home/u/.claude/skills/foo/README.md").unwrap_err();
        assert!(err.contains("Not a SKILL.md"));
    }

    #[test]
    fn has_claude_skills_segment_detects_pair() {
        assert!(has_claude_skills_segment(Path::new(
            "/home/u/proj/.claude/skills/foo/SKILL.md"
        )));
        assert!(!has_claude_skills_segment(Path::new(
            "/home/u/proj/.claude/agents/foo/SKILL.md"
        )));
    }

    #[test]
    fn archives_skill_files_for_node_sync() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("asv-skill-sync-{}", stamp));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("SKILL.md"), "# test").unwrap();
        fs::write(root.join("assets").join("note.txt"), "asset").unwrap();

        let archive = archive_skill_dir(&root).unwrap();
        let files = archive_file_map(&archive).unwrap();
        assert_eq!(
            files.get("SKILL.md").map(Vec::as_slice),
            Some(&b"# test"[..])
        );
        assert_eq!(
            files.get("assets/note.txt").map(Vec::as_slice),
            Some(&b"asset"[..])
        );

        fs::remove_dir_all(root).unwrap();
    }
}
