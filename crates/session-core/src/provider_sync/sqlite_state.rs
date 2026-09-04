use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OpenFlags, OptionalExtension};

pub fn db_path(codex_home: &Path) -> PathBuf {
    codex_home.join("state_5.sqlite")
}

pub fn read_provider_counts(path: &Path) -> Vec<(String, bool, u32)> {
    let Ok(conn) = open_read(path) else {
        return Vec::new();
    };
    if !table_exists(&conn, "threads") {
        return Vec::new();
    }
    let has_archived = column_exists(&conn, "threads", "archived");
    let sql = if has_archived {
        "SELECT COALESCE(model_provider,''), COALESCE(archived,0), COUNT(*) \
         FROM threads GROUP BY model_provider, archived"
    } else {
        "SELECT COALESCE(model_provider,''), 0, COUNT(*) \
         FROM threads GROUP BY model_provider"
    };
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)? != 0,
            row.get::<_, i64>(2)? as u32,
        ))
    }) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

pub fn count_mismatched_threads(path: &Path, target: &str) -> u32 {
    let Ok(conn) = open_read(path) else { return 0 };
    if !table_exists(&conn, "threads") {
        return 0;
    }
    conn.query_row(
        "SELECT COUNT(*) FROM threads WHERE COALESCE(model_provider,'') <> ?1",
        params![target],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n as u32)
    .unwrap_or(0)
}

pub fn update_threads(
    path: &Path,
    target: &str,
    thread_ids_with_user_event: &[String],
    cwd_updates: &HashMap<String, String>,
) -> Result<u32, String> {
    let conn = open_write(path)?;
    if !table_exists(&conn, "threads") {
        return Ok(0);
    }
    let has_user_event = column_exists(&conn, "threads", "has_user_event");
    let has_cwd = column_exists(&conn, "threads", "cwd");

    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| format!("begin tx: {}", e))?;

    let provider_updated = match conn.execute(
        "UPDATE threads SET model_provider = ?1 WHERE COALESCE(model_provider,'') <> ?1",
        params![target],
    ) {
        Ok(n) => n as u32,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(format!("update model_provider: {}", e));
        }
    };

    if has_user_event {
        for tid in thread_ids_with_user_event {
            if let Err(e) = conn.execute(
                "UPDATE threads SET has_user_event = 1 \
                 WHERE id = ?1 AND COALESCE(has_user_event,0) <> 1",
                params![tid],
            ) {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(format!("update has_user_event: {}", e));
            }
        }
    }

    if has_cwd {
        for (tid, cwd) in cwd_updates {
            if let Err(e) = conn.execute(
                "UPDATE threads SET cwd = ?1 \
                 WHERE id = ?2 AND COALESCE(cwd,'') <> ?1",
                params![cwd, tid],
            ) {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(format!("update cwd: {}", e));
            }
        }
    }

    conn.execute_batch("COMMIT;")
        .map_err(|e| format!("commit: {}", e))?;
    Ok(provider_updated)
}

/// Duplicate the `threads` row identified by `old_id` into a new row with
/// `new_id`, `new_rollout_path` and `provider`, marked un-archived. Reads the
/// source row's columns generically (`SELECT *`) so it survives schema changes,
/// then re-inserts with the three overridden fields. Returns `Ok(false)` when
/// the source row doesn't exist (caller treats as skip). The cloned rollout
/// file is the actual conversation; this row just makes Codex Desktop list it.
pub fn clone_thread(
    path: &Path,
    old_id: &str,
    new_id: &str,
    new_rollout_path: &str,
    provider: &str,
) -> Result<bool, String> {
    let conn = open_write(path)?;
    if !table_exists(&conn, "threads") {
        return Ok(false);
    }

    // Column names in declared order.
    let cols: Vec<String> = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(\"threads\")")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        rows.flatten().collect()
    };
    if cols.is_empty() {
        return Ok(false);
    }

    // Read the source row's values as generic SQL values.
    let col_count = cols.len();
    let row_vals: Option<Vec<SqlValue>> = conn
        .query_row(
            "SELECT * FROM threads WHERE id = ?1",
            params![old_id],
            |row| {
                let mut vals = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    vals.push(row.get::<_, SqlValue>(i)?);
                }
                Ok(vals)
            },
        )
        .optional()
        .map_err(|e| format!("read source thread: {}", e))?;

    let Some(mut vals) = row_vals else {
        return Ok(false);
    };

    // Override the cloned identity / provider / location, and force it active.
    for (i, col) in cols.iter().enumerate() {
        match col.as_str() {
            "id" => vals[i] = SqlValue::Text(new_id.to_string()),
            "model_provider" => vals[i] = SqlValue::Text(provider.to_string()),
            "rollout_path" => vals[i] = SqlValue::Text(new_rollout_path.to_string()),
            "archived" => vals[i] = SqlValue::Integer(0),
            "archived_at" => vals[i] = SqlValue::Null,
            _ => {}
        }
    }

    let col_list = cols
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (1..=col_count)
        .map(|i| format!("?{}", i))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO threads ({}) VALUES ({})",
        col_list, placeholders
    );
    conn.execute(&sql, params_from_iter(vals.iter()))
        .map_err(|e| format!("insert cloned thread: {}", e))?;
    Ok(true)
}

fn open_read(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
    Ok(conn)
}

fn open_write(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
    Ok(conn)
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1",
        params![table],
        |_| Ok(true),
    )
    .unwrap_or(false)
}

fn column_exists(conn: &Connection, table: &str, col: &str) -> bool {
    let sql = format!("PRAGMA table_info(\"{}\")", table);
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return false;
    };
    let Ok(mut rows) = stmt.query([]) else {
        return false;
    };
    while let Ok(Some(row)) = rows.next() {
        if let Ok(name) = row.get::<_, String>(1) {
            if name == col {
                return true;
            }
        }
    }
    false
}
