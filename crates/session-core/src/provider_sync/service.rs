use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::backup::{self, BackupMetadata, SessionMetaBackupItem};
use super::config;
use super::global_state;
use super::rollout;
use super::sqlite_state;
use super::types::*;

pub fn get_codex_home() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| h.join(".codex"))
        .ok_or_else(|| "Cannot resolve user home directory".to_string())
}

pub fn get_status() -> Result<ProviderSyncStatus, String> {
    let codex_home = get_codex_home()?;
    let config_path = codex_home.join("config.toml");
    let (current_provider, implicit) = config::read_current_provider(&config_path);
    let configured = config::list_configured_providers(&config_path);

    let sessions_dir = codex_home.join("sessions");
    let archived_dir = codex_home.join("archived_sessions");

    let mut rollout_counts: HashMap<String, u32> = HashMap::new();
    let mut archived_counts: HashMap<String, u32> = HashMap::new();
    let mut encrypted_warnings: HashMap<String, u32> = HashMap::new();
    let mut mismatched_rollouts = 0u32;
    let mut mismatched_archived = 0u32;

    for p in rollout::scan_session_dir(&sessions_dir) {
        if let Some(meta) = rollout::read_meta(&p, true) {
            *rollout_counts.entry(meta.provider.clone()).or_insert(0) += 1;
            if meta.provider != current_provider {
                mismatched_rollouts += 1;
                if meta.has_encrypted_content {
                    *encrypted_warnings.entry(meta.provider).or_insert(0) += 1;
                }
            }
        }
    }
    for p in rollout::scan_session_dir(&archived_dir) {
        if let Some(meta) = rollout::read_meta(&p, true) {
            *archived_counts.entry(meta.provider.clone()).or_insert(0) += 1;
            if meta.provider != current_provider {
                mismatched_archived += 1;
                if meta.has_encrypted_content {
                    *encrypted_warnings.entry(meta.provider).or_insert(0) += 1;
                }
            }
        }
    }

    let sqlite_path = sqlite_state::db_path(&codex_home);
    let sqlite_exists = sqlite_path.exists();
    let sqlite_stats: Vec<SqliteProviderCount> = if sqlite_exists {
        sqlite_state::read_provider_counts(&sqlite_path)
            .into_iter()
            .map(|(provider, archived, count)| SqliteProviderCount {
                provider,
                archived,
                count,
            })
            .collect()
    } else {
        Vec::new()
    };
    let mismatched_sqlite_threads = if sqlite_exists {
        sqlite_state::count_mismatched_threads(&sqlite_path, &current_provider)
    } else {
        0
    };

    let global_state_p = global_state::global_state_path(&codex_home);
    let backups = backup::list_backups(&codex_home);

    Ok(ProviderSyncStatus {
        codex_home: codex_home.to_string_lossy().into_owned(),
        current_provider,
        current_provider_implicit: implicit,
        config_toml_path: config_path.to_string_lossy().into_owned(),
        config_toml_exists: config_path.exists(),
        configured_providers: configured,
        rollout_stats: to_provider_counts(rollout_counts),
        archived_stats: to_provider_counts(archived_counts),
        sqlite_stats,
        sqlite_path: sqlite_path.to_string_lossy().into_owned(),
        sqlite_exists,
        global_state_path: global_state_p.to_string_lossy().into_owned(),
        global_state_exists: global_state_p.exists(),
        mismatched_rollouts,
        mismatched_archived,
        mismatched_sqlite_threads,
        encrypted_warnings: encrypted_warnings
            .into_iter()
            .map(|(provider, count)| EncryptedWarning { provider, count })
            .collect(),
        backups,
    })
}

fn to_provider_counts(map: HashMap<String, u32>) -> Vec<ProviderCount> {
    let mut v: Vec<ProviderCount> = map
        .into_iter()
        .map(|(provider, count)| ProviderCount { provider, count })
        .collect();
    v.sort_by_key(|item| std::cmp::Reverse(item.count));
    v
}

pub fn run_sync(target_override: Option<String>, keep: usize) -> Result<SyncResult, String> {
    let codex_home = get_codex_home()?;
    let config_path = codex_home.join("config.toml");
    let target = target_override.unwrap_or_else(|| config::read_current_provider(&config_path).0);
    sync_to_target(&codex_home, &config_path, &target, keep, false)
}

pub fn run_switch(new_provider: String, keep: usize) -> Result<SyncResult, String> {
    let codex_home = get_codex_home()?;
    let config_path = codex_home.join("config.toml");
    if new_provider.trim().is_empty() {
        return Err("provider id cannot be empty".into());
    }
    config::set_root_provider(&config_path, &new_provider)?;
    let mut result = sync_to_target(&codex_home, &config_path, &new_provider, keep, true)?;
    result.config_updated = true;
    Ok(result)
}

/// The path a clone's rollout file should take: the original filename with the
/// old session id swapped for the new one. Falls back to `rollout-<newid>.jsonl`
/// in the same directory if the old id isn't embedded in the name.
fn clone_target_path(orig: &Path, old_id: &str, new_id: &str) -> PathBuf {
    let name = orig.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let new_name = if name.contains(old_id) {
        name.replace(old_id, new_id)
    } else {
        format!("rollout-{new_id}.jsonl")
    };
    orig.with_file_name(new_name)
}

/// Non-destructive clone: for each source rollout, write a copy carrying a new
/// id + `target` provider and insert a matching `threads` row, leaving the
/// originals untouched. Backs up `state_5.sqlite` first (we INSERT into it).
pub fn run_clone(
    file_paths: Vec<String>,
    target_provider: String,
    keep: usize,
) -> Result<CloneResult, String> {
    if target_provider.trim().is_empty() {
        return Err("provider id cannot be empty".into());
    }
    let codex_home = get_codex_home()?;
    let sqlite_path = sqlite_state::db_path(&codex_home);

    // Back up the DB before inserting rows, so a bad clone is recoverable.
    let backup_dir = backup::create_backup_dir(&codex_home)?;
    let mut db_files: Vec<String> = Vec::new();
    if sqlite_path.exists() {
        let target_path = backup_dir.join("db").join("state_5.sqlite");
        if fs::copy(&sqlite_path, &target_path).is_ok() {
            db_files.push("state_5.sqlite".to_string());
        }
        for ext in ["sqlite-shm", "sqlite-wal"] {
            let src = codex_home.join(format!("state_5.{ext}"));
            if src.exists() {
                let dest = backup_dir.join("db").join(format!("state_5.{ext}"));
                if fs::copy(&src, &dest).is_ok() {
                    db_files.push(format!("state_5.{ext}"));
                }
            }
        }
    }

    let mut cloned = 0u32;
    let mut skipped: Vec<String> = Vec::new();
    let mut new_session_ids: Vec<String> = Vec::new();
    let mut encrypted_session_ids: Vec<String> = Vec::new();
    let mut created_files: Vec<String> = Vec::new();

    for fp in &file_paths {
        let path = PathBuf::from(fp);
        let Some(meta) = rollout::read_meta(&path, true) else {
            skipped.push(fp.clone());
            continue;
        };
        let Some(old_id) = meta.session_id.clone() else {
            skipped.push(fp.clone());
            continue;
        };

        let new_id = Uuid::new_v4().to_string();
        let new_path = clone_target_path(&path, &old_id, &new_id);

        if rollout::write_cloned_rollout(
            &path,
            &new_path,
            meta.session_meta_line_idx,
            &meta.original_line,
            &target_provider,
            &new_id,
            meta.mtime,
        )
        .is_err()
        {
            skipped.push(fp.clone());
            continue;
        }

        // Mirror the clone into state_5.sqlite so Codex Desktop lists it. If the
        // INSERT fails, drop the just-written rollout to avoid an orphan file.
        if sqlite_path.exists() {
            match sqlite_state::clone_thread(
                &sqlite_path,
                &old_id,
                &new_id,
                &new_path.to_string_lossy(),
                &target_provider,
            ) {
                Ok(_) => {}
                Err(_) => {
                    let _ = fs::remove_file(&new_path);
                    skipped.push(fp.clone());
                    continue;
                }
            }
        }

        if meta.has_encrypted_content {
            encrypted_session_ids.push(new_id.clone());
        }
        created_files.push(new_path.to_string_lossy().into_owned());
        new_session_ids.push(new_id);
        cloned += 1;
    }

    let now: DateTime<Utc> = Utc::now();
    let metadata = BackupMetadata {
        namespace: backup::NAMESPACE.to_string(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        target_provider: target_provider.clone(),
        created_at: now.to_rfc3339(),
        db_files,
        changed_session_files: created_files,
    };
    if let Ok(json) = serde_json::to_string_pretty(&metadata) {
        let _ = fs::write(backup_dir.join("metadata.json"), json);
    }

    let _ = backup::prune_backups(&codex_home, keep.max(1));
    crate::provider::codex::invalidate_sessions_cache();

    Ok(CloneResult {
        backup_dir: backup_dir.to_string_lossy().into_owned(),
        target_provider,
        cloned,
        skipped,
        new_session_ids,
        encrypted_session_ids,
    })
}

fn sync_to_target(
    codex_home: &Path,
    config_path: &Path,
    target: &str,
    keep: usize,
    config_already_changed: bool,
) -> Result<SyncResult, String> {
    let sessions_dir = codex_home.join("sessions");
    let archived_dir = codex_home.join("archived_sessions");

    let mut all_paths = rollout::scan_session_dir(&sessions_dir);
    all_paths.extend(rollout::scan_session_dir(&archived_dir));

    let mut to_rewrite: Vec<rollout::RolloutMetaInfo> = Vec::new();
    let mut thread_ids_with_user_event: Vec<String> = Vec::new();
    let mut cwd_updates: HashMap<String, String> = HashMap::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for p in &all_paths {
        let Some(meta) = rollout::read_meta(p, false) else {
            continue;
        };
        if let Some(sid) = &meta.session_id {
            if seen_ids.insert(sid.clone()) {
                thread_ids_with_user_event.push(sid.clone());
                if let Some(cwd) = &meta.cwd {
                    let normalized = global_state::normalize_path_string(cwd);
                    cwd_updates.insert(sid.clone(), normalized);
                }
            }
        }
        if meta.provider != target {
            to_rewrite.push(meta);
        }
    }

    let backup_dir = backup::create_backup_dir(codex_home)?;
    let sqlite_path = sqlite_state::db_path(codex_home);
    let mut db_files: Vec<String> = Vec::new();

    if sqlite_path.exists() {
        let target_path = backup_dir.join("db").join("state_5.sqlite");
        if fs::copy(&sqlite_path, &target_path).is_ok() {
            db_files.push("state_5.sqlite".to_string());
        }
        for ext in ["sqlite-shm", "sqlite-wal"] {
            let src = codex_home.join(format!("state_5.{}", ext));
            if src.exists() {
                let dest = backup_dir.join("db").join(format!("state_5.{}", ext));
                if fs::copy(&src, &dest).is_ok() {
                    db_files.push(format!("state_5.{}", ext));
                }
            }
        }
    }

    if config_path.exists() {
        let _ = fs::copy(config_path, backup_dir.join("config.toml"));
    }
    let global_state_p = global_state::global_state_path(codex_home);
    if global_state_p.exists() {
        let _ = fs::copy(&global_state_p, backup_dir.join(".codex-global-state.json"));
    }
    let global_state_bak = codex_home.join(".codex-global-state.json.bak");
    if global_state_bak.exists() {
        let _ = fs::copy(
            &global_state_bak,
            backup_dir.join(".codex-global-state.json.bak"),
        );
    }

    let backup_items: Vec<SessionMetaBackupItem> = to_rewrite
        .iter()
        .map(|m| {
            let secs = m
                .mtime
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            SessionMetaBackupItem {
                path: m.path.to_string_lossy().into_owned(),
                original_line: m.original_line.clone(),
                line_index: m.session_meta_line_idx,
                original_mtime_secs: secs,
            }
        })
        .collect();
    let backup_json = serde_json::to_string_pretty(&backup_items)
        .map_err(|e| format!("serialize session-meta-backup: {}", e))?;
    fs::write(backup_dir.join("session-meta-backup.json"), backup_json)
        .map_err(|e| e.to_string())?;

    let now: DateTime<Utc> = Utc::now();
    let metadata = BackupMetadata {
        namespace: backup::NAMESPACE.to_string(),
        codex_home: codex_home.to_string_lossy().into_owned(),
        target_provider: target.to_string(),
        created_at: now.to_rfc3339(),
        db_files,
        changed_session_files: backup_items.iter().map(|i| i.path.clone()).collect(),
    };
    let metadata_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("serialize metadata: {}", e))?;
    fs::write(backup_dir.join("metadata.json"), metadata_json).map_err(|e| e.to_string())?;

    let updated_sqlite_rows = if sqlite_path.exists() {
        sqlite_state::update_threads(
            &sqlite_path,
            target,
            &thread_ids_with_user_event,
            &cwd_updates,
        )?
    } else {
        0
    };

    let mut updated_rollouts = 0u32;
    let mut skipped_locked: Vec<String> = Vec::new();
    for meta in &to_rewrite {
        match rollout::rewrite_session_meta_provider(
            &meta.path,
            meta.session_meta_line_idx,
            &meta.original_line,
            target,
            meta.mtime,
        ) {
            Ok(_) => updated_rollouts += 1,
            Err(_) => skipped_locked.push(meta.path.to_string_lossy().into_owned()),
        }
    }

    let global_state_updated = global_state::normalize_paths(&global_state_p).unwrap_or(false);

    let _ = backup::prune_backups(codex_home, keep.max(1));

    crate::provider::codex::invalidate_sessions_cache();

    Ok(SyncResult {
        backup_dir: backup_dir.to_string_lossy().into_owned(),
        target_provider: target.to_string(),
        updated_rollouts,
        updated_sqlite_rows,
        global_state_updated,
        config_updated: config_already_changed,
        skipped_locked,
    })
}

pub fn run_restore(backup_dir: &str, opts: RestoreOptions) -> Result<RestoreResult, String> {
    let codex_home = get_codex_home()?;
    let dir = PathBuf::from(backup_dir);
    let meta_text = fs::read_to_string(dir.join("metadata.json"))
        .map_err(|e| format!("read metadata.json: {}", e))?;
    let metadata: BackupMetadata =
        serde_json::from_str(&meta_text).map_err(|e| format!("parse metadata: {}", e))?;
    if metadata.namespace != backup::NAMESPACE {
        return Err("not a provider-sync backup".into());
    }

    let mut restored_files = 0u32;
    let mut restored_sessions = 0u32;

    if opts.include_config {
        let src = dir.join("config.toml");
        if src.exists() {
            fs::copy(&src, codex_home.join("config.toml")).map_err(|e| e.to_string())?;
            restored_files += 1;
        }
    }
    if opts.include_db {
        for name in &metadata.db_files {
            let src = dir.join("db").join(name);
            if src.exists() {
                fs::copy(&src, codex_home.join(name)).map_err(|e| e.to_string())?;
                restored_files += 1;
            }
        }
    }
    if opts.include_global_state {
        let src = dir.join(".codex-global-state.json");
        if src.exists() {
            fs::copy(&src, codex_home.join(".codex-global-state.json"))
                .map_err(|e| e.to_string())?;
            restored_files += 1;
        }
        let src_bak = dir.join(".codex-global-state.json.bak");
        if src_bak.exists() {
            let _ = fs::copy(&src_bak, codex_home.join(".codex-global-state.json.bak"));
        }
    }
    if opts.include_sessions {
        let text = fs::read_to_string(dir.join("session-meta-backup.json")).unwrap_or_default();
        if !text.is_empty() {
            let items: Vec<SessionMetaBackupItem> = serde_json::from_str(&text)
                .map_err(|e| format!("parse session-meta-backup: {}", e))?;
            for item in items {
                let path = PathBuf::from(&item.path);
                if !path.exists() {
                    continue;
                }
                if rollout::restore_session_meta(
                    &path,
                    item.line_index,
                    &item.original_line,
                    item.original_mtime_secs,
                )
                .is_ok()
                {
                    restored_sessions += 1;
                }
            }
        }
    }

    crate::provider::codex::invalidate_sessions_cache();

    Ok(RestoreResult {
        restored_files,
        restored_sessions,
    })
}

pub fn run_prune_backups(keep: usize) -> Result<u32, String> {
    let codex_home = get_codex_home()?;
    backup::prune_backups(&codex_home, keep)
}
