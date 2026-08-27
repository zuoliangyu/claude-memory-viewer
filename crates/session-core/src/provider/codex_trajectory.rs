//! Projection of Codex rollout JSONL into a compact turn/record ledger.
//!
//! This intentionally lives beside the existing message parser. The message
//! parser is optimized for rendering a conversation; this module preserves
//! event timing, tool correlation and token deltas for a trajectory view.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use lru::LruCache;
use parking_lot::Mutex;
use serde_json::Value;
use uuid::Uuid;

use crate::models::trajectory::{
    TokenUsage, Trajectory, TrajectoryPagination, TrajectoryRecord, TrajectorySession,
    TrajectoryStats, TrajectoryTurn, TrajectoryWarning,
};

const MAX_RECORD_TEXT: usize = 12_000;
const MAX_WARNINGS: usize = 100;
const MAX_LINEAGE_SEGMENTS: usize = 1_024;
const PROJECTION_CACHE_CAPACITY: usize = 2;
const DEFAULT_PAGE_RECORDS: usize = 500;
const MIN_PAGE_RECORDS: usize = 50;
const MAX_PAGE_RECORDS: usize = 1_000;
const FAST_TAIL_BYTES: u64 = 8 * 1024 * 1024;

type RolloutIndex = HashMap<String, PathBuf>;
type RolloutIndexCache = Option<(PathBuf, RolloutIndex)>;

#[derive(Clone)]
struct Segment {
    path: PathBuf,
    start_byte: Option<u64>,
    start_ordinal: Option<u64>,
    end_ordinal: Option<u64>,
    end_byte: Option<u64>,
}

#[derive(Clone)]
struct CachedProjection {
    modified_key: u64,
    file_size: u64,
    trajectory: Arc<Trajectory>,
}

fn projection_cache() -> &'static Mutex<LruCache<String, CachedProjection>> {
    static CACHE: OnceLock<Mutex<LruCache<String, CachedProjection>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(PROJECTION_CACHE_CAPACITY).expect("non-zero trajectory cache"),
        ))
    })
}

fn projection_build_locks() -> &'static Mutex<HashMap<String, Arc<Mutex<()>>>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_project(path: &Path) -> Result<Arc<Trajectory>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("Failed to read session metadata: {error}"))?;
    let modified_key = crate::state::file_modified_key(path)?;
    let file_size = metadata.len();
    let key = path.to_string_lossy().into_owned();
    let cached = {
        let mut cache = projection_cache().lock();
        cache
            .get(&key)
            .filter(|cached| cached.modified_key == modified_key && cached.file_size == file_size)
            .map(|cached| Arc::clone(&cached.trajectory))
    };
    if let Some(cached) = cached {
        return Ok(cached);
    }

    // React StrictMode can issue the same request twice in development. Serialize
    // cache misses and check again so a large rollout is projected only once.
    let build_lock = projection_build_locks()
        .lock()
        .entry(key.clone())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let _build_guard = build_lock.lock();
    let cached = {
        let mut cache = projection_cache().lock();
        cache
            .get(&key)
            .filter(|cached| cached.modified_key == modified_key && cached.file_size == file_size)
            .map(|cached| Arc::clone(&cached.trajectory))
    };
    if let Some(cached) = cached {
        return Ok(cached);
    }

    let trajectory = Arc::new(project(path)?);
    projection_cache().lock().put(
        key,
        CachedProjection {
            modified_key,
            file_size,
            trajectory: Arc::clone(&trajectory),
        },
    );
    Ok(trajectory)
}

fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
}

fn add_warning(warnings: &mut Vec<TrajectoryWarning>, code: &str, line: usize, message: String) {
    if warnings.len() < MAX_WARNINGS {
        warnings.push(TrajectoryWarning {
            code: code.to_string(),
            line,
            message,
        });
    }
}

fn truncate(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

fn bounded_value(value: Option<&Value>) -> Option<String> {
    value.map(|value| {
        let text = value
            .as_str()
            .map(ToString::to_string)
            .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_else(|_| value.to_string()));
        truncate(&text, MAX_RECORD_TEXT)
    })
}

fn text_content(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(content) = value.get("content") {
        return text_content(Some(content));
    }
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return text.to_string();
    }
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.as_str())
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn timestamp(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToString::to_string)
}

fn timestamp_millis(value: Option<&Value>) -> Option<String> {
    let value = value?.as_u64()?;
    let millis = i64::try_from(value).ok()?;
    DateTime::<Utc>::from_timestamp_millis(millis).map(|value| value.to_rfc3339())
}

fn normalized_item_type(value: Option<&Value>) -> String {
    let Some(value) = value
        .and_then(Value::as_str)
        .filter(|value| value.len() <= 200)
    else {
        return String::new();
    };
    let mut normalized = String::with_capacity(value.len() + 4);
    let mut previous_lowercase = false;
    for character in value.chars() {
        if character.is_ascii_uppercase() && previous_lowercase {
            normalized.push('_');
        }
        normalized.push(if character == '-' {
            '_'
        } else {
            character.to_ascii_lowercase()
        });
        previous_lowercase = character.is_ascii_lowercase();
    }
    normalized
}

fn failed_status(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(|status| {
            matches!(
                status.to_ascii_lowercase().as_str(),
                "failed" | "error" | "declined" | "incomplete"
            )
        })
        .unwrap_or(false)
}

fn millis_between(start: Option<&str>, end: Option<&str>) -> Option<u64> {
    let start = DateTime::parse_from_rfc3339(start?)
        .ok()?
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339(end?).ok()?.with_timezone(&Utc);
    (end - start).num_milliseconds().try_into().ok()
}

fn token_usage(value: Option<&Value>) -> Option<TokenUsage> {
    let value = value?.as_object()?;
    let input_tokens = value
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cached_input_tokens = value
        .get("cached_input_tokens")
        .or_else(|| value.get("cache_read_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = value
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning_output_tokens = value
        .get("reasoning_output_tokens")
        .or_else(|| value.get("reasoning_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = value
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(input_tokens.saturating_add(output_tokens));
    Some(TokenUsage {
        input_tokens,
        cached_input_tokens,
        output_tokens,
        reasoning_output_tokens,
        total_tokens,
    })
}

fn add_usage(previous: Option<&TokenUsage>, current: &TokenUsage) -> TokenUsage {
    let previous = previous.cloned().unwrap_or_default();
    TokenUsage {
        input_tokens: previous.input_tokens.saturating_add(current.input_tokens),
        cached_input_tokens: previous
            .cached_input_tokens
            .saturating_add(current.cached_input_tokens),
        output_tokens: previous.output_tokens.saturating_add(current.output_tokens),
        reasoning_output_tokens: previous
            .reasoning_output_tokens
            .saturating_add(current.reasoning_output_tokens),
        total_tokens: previous.total_tokens.saturating_add(current.total_tokens),
    }
}

fn output_is_error(value: Option<&Value>) -> bool {
    let Some(value) = value else { return false };
    if value
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    value
        .as_str()
        .map(|text| text.to_ascii_lowercase().contains("error"))
        .unwrap_or(false)
}

fn rollout_id(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    if stem.len() < 36 {
        return None;
    }
    for start in 0..=stem.len() - 36 {
        let candidate = &stem[start..start + 36];
        if start + 36 == stem.len() && Uuid::parse_str(candidate).is_ok() {
            return Some(candidate.to_ascii_lowercase());
        }
    }
    None
}

fn first_metadata(path: &Path) -> Option<Value> {
    let file = fs::File::open(path).ok()?;
    for line in BufReader::new(file).lines().take(80).flatten() {
        let Ok(row) = serde_json::from_str::<Value>(line.trim_start_matches('\u{feff}').trim())
        else {
            continue;
        };
        if row.get("type").and_then(Value::as_str) == Some("session_meta") {
            return row.get("payload").cloned();
        }
    }
    None
}

fn find_rollout_in_directory(directory: Option<&Path>, id: &str) -> Option<PathBuf> {
    fs::read_dir(directory?)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|candidate| {
            candidate.is_file()
                && candidate.extension().and_then(|ext| ext.to_str()) == Some("jsonl")
                && rollout_id(candidate).as_deref() == Some(id)
        })
}

fn rollout_index() -> RolloutIndex {
    let Some(home) = codex_home() else {
        return HashMap::new();
    };
    let mut index = HashMap::new();
    let mut roots = vec![home.join("sessions"), home.join("archived_sessions")];
    while let Some(path) = roots.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path();
            if candidate.is_dir() {
                roots.push(candidate);
            } else if candidate.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                if let Some(id) = rollout_id(&candidate) {
                    index.entry(id).or_insert(candidate);
                }
            }
        }
    }
    index
}

fn cached_rollout_index() -> RolloutIndex {
    static INDEX: OnceLock<Mutex<RolloutIndexCache>> = OnceLock::new();
    let Some(home) = codex_home() else {
        return HashMap::new();
    };
    let cache = INDEX.get_or_init(|| Mutex::new(None));
    let mut cache = cache.lock();
    if let Some((cached_home, index)) = cache.as_ref() {
        if cached_home == &home {
            return index.clone();
        }
    }
    let index = rollout_index();
    *cache = Some((home, index.clone()));
    index
}

fn history_base(value: Option<&Value>) -> Option<(String, u64, u64)> {
    let value = value?.as_object()?;
    let id = value.get("thread_id")?.as_str()?.to_ascii_lowercase();
    if Uuid::parse_str(&id).is_err() {
        return None;
    }
    let ordinal = value.get("end_ordinal_exclusive")?.as_u64()?;
    let byte = value.get("end_byte_offset")?.as_u64()?;
    (ordinal > 0 && byte > 0).then_some((id, ordinal, byte))
}

fn resolve_segments(path: &Path, warnings: &mut Vec<TrajectoryWarning>) -> Vec<Segment> {
    let Some(metadata) = first_metadata(path) else {
        return vec![Segment {
            path: path.to_path_buf(),
            start_byte: None,
            start_ordinal: None,
            end_ordinal: None,
            end_byte: None,
        }];
    };
    if metadata
        .get("history_mode")
        .and_then(Value::as_str)
        .unwrap_or("legacy")
        != "paginated"
    {
        return vec![Segment {
            path: path.to_path_buf(),
            start_byte: None,
            start_ordinal: None,
            end_ordinal: None,
            end_byte: None,
        }];
    }

    let mut segments = Vec::new();
    let mut current = path.to_path_buf();
    let mut end_ordinal = None;
    let mut end_byte = None;
    let mut seen = HashMap::<String, ()>::new();
    let mut indexed_rollouts: Option<HashMap<String, PathBuf>> = None;

    for _ in 0..MAX_LINEAGE_SEGMENTS {
        let Some(id) = rollout_id(&current) else {
            add_warning(
                warnings,
                "invalid_lineage",
                0,
                "分页 rollout 文件名缺少 UUID。".to_string(),
            );
            return vec![Segment {
                path: path.to_path_buf(),
                start_byte: None,
                start_ordinal: None,
                end_ordinal: None,
                end_byte: None,
            }];
        };
        if seen.insert(id.clone(), ()).is_some() {
            add_warning(
                warnings,
                "lineage_cycle",
                0,
                "分页 rollout lineage 包含循环引用。".to_string(),
            );
            return vec![Segment {
                path: path.to_path_buf(),
                start_byte: None,
                start_ordinal: None,
                end_ordinal: None,
                end_byte: None,
            }];
        }
        let Some(meta) = first_metadata(&current) else {
            break;
        };
        let base = history_base(meta.get("history_base"));
        let start_ordinal = base.as_ref().map(|(_, ordinal, _)| ordinal + 1).or(Some(1));
        segments.push(Segment {
            path: current.clone(),
            start_byte: None,
            start_ordinal,
            end_ordinal,
            end_byte,
        });
        let Some((source_id, source_end_ordinal, source_end_byte)) = base else {
            break;
        };
        let source = find_rollout_in_directory(current.parent(), &source_id).or_else(|| {
            indexed_rollouts
                .get_or_insert_with(cached_rollout_index)
                .get(&source_id)
                .cloned()
        });
        let Some(source) = source else {
            add_warning(
                warnings,
                "lineage_missing",
                0,
                format!("找不到分页 lineage 源文件 {source_id}。"),
            );
            break;
        };
        current = source;
        end_ordinal = Some(source_end_ordinal);
        end_byte = Some(source_end_byte);
    }
    if segments.len() >= MAX_LINEAGE_SEGMENTS {
        add_warning(
            warnings,
            "lineage_too_long",
            0,
            "分页 rollout lineage 超过支持上限。".to_string(),
        );
    }
    segments.reverse();
    segments
}

fn read_entries<F>(segments: &[Segment], mut handle: F)
where
    F: FnMut(usize, Result<Value, ()>),
{
    for segment in segments {
        let Ok(mut file) = fs::File::open(&segment.path) else {
            continue;
        };
        let start_byte = segment
            .start_byte
            .and_then(|candidate| next_line_start(&mut file, candidate).ok())
            .unwrap_or(0);
        if file.seek(SeekFrom::Start(start_byte)).is_err() {
            continue;
        }
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut line_number = 0usize;
        let mut bytes_read = start_byte;
        loop {
            line.clear();
            let Ok(count) = reader.read_line(&mut line) else {
                break;
            };
            if count == 0 {
                break;
            }
            line_number += 1;
            let line_end = bytes_read.saturating_add(count as u64);
            if segment.end_byte.is_some_and(|end| line_end > end) {
                break;
            }
            bytes_read = line_end;
            let trimmed = line.trim_start_matches('\u{feff}').trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(row) = serde_json::from_str::<Value>(trimmed) else {
                handle(line_number, Err(()));
                continue;
            };
            if let Some(ordinal) = row.get("ordinal").and_then(Value::as_u64) {
                if segment.start_ordinal.is_some_and(|start| ordinal < start)
                    || segment.end_ordinal.is_some_and(|end| ordinal >= end)
                {
                    continue;
                }
            }
            handle(line_number, Ok(row));
        }
    }
}

fn next_line_start(file: &mut fs::File, candidate: u64) -> std::io::Result<u64> {
    if candidate == 0 {
        return Ok(0);
    }
    file.seek(SeekFrom::Start(candidate))?;
    let mut buffer = [0u8; 16 * 1024];
    let mut position = candidate;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            return Ok(position);
        }
        if let Some(offset) = buffer[..count].iter().position(|byte| *byte == b'\n') {
            return Ok(position.saturating_add(offset as u64).saturating_add(1));
        }
        position = position.saturating_add(count as u64);
    }
}

fn ensure_turn(
    turns: &mut Vec<TrajectoryTurn>,
    active: &mut Option<usize>,
    timestamp: Option<String>,
    id: Option<String>,
    model: Option<String>,
) -> usize {
    if let Some(index) = *active {
        return index;
    }
    turns.push(TrajectoryTurn {
        index: turns.len() + 1,
        id,
        started_at: timestamp,
        status: "running".to_string(),
        model,
        ..Default::default()
    });
    let index = turns.len() - 1;
    *active = Some(index);
    index
}

fn add_record(
    records: &mut Vec<TrajectoryRecord>,
    turns: &mut [TrajectoryTurn],
    turn: usize,
    mut record: TrajectoryRecord,
) -> usize {
    record.index = records.len() + 1;
    record.turn = turns[turn].index;
    turns[turn].records += 1;
    if let Some(step) = record.step {
        turns[turn].steps = turns[turn].steps.max(step);
    }
    records.push(record);
    records.len() - 1
}

fn finish_turn(
    turn: &mut TrajectoryTurn,
    completed_at: Option<String>,
    error: Option<String>,
    aborted: bool,
) {
    turn.completed_at = completed_at;
    turn.duration_ms = millis_between(turn.started_at.as_deref(), turn.completed_at.as_deref());
    if error.is_some() {
        turn.error = error;
    }
    turn.status = if aborted {
        "aborted"
    } else if turn.error.is_some() {
        "error"
    } else {
        "complete"
    }
    .to_string();
}

fn record_base(
    timestamp: Option<String>,
    step: Option<usize>,
    kind: &str,
    event: &str,
    summary: String,
) -> TrajectoryRecord {
    TrajectoryRecord {
        timestamp: timestamp.clone(),
        started_at: timestamp,
        step,
        kind: kind.to_string(),
        event: event.to_string(),
        summary,
        status: "complete".to_string(),
        ..Default::default()
    }
}

fn apply_record_timing(
    record: &mut TrajectoryRecord,
    started_at: Option<String>,
    completed_at: Option<String>,
) {
    record.timestamp = started_at.clone();
    record.started_at = started_at;
    record.completed_at = completed_at;
    record.duration_ms =
        millis_between(record.started_at.as_deref(), record.completed_at.as_deref());
}

pub fn parse(path: &Path) -> Result<Trajectory, String> {
    parse_page(path, None, None)
}

pub fn parse_page(
    path: &Path,
    max_records: Option<usize>,
    before_record: Option<usize>,
) -> Result<Trajectory, String> {
    let projection = cached_project(path)?;
    Ok(paginate(&projection, max_records, before_record, true))
}

pub fn parse_fast_page(path: &Path, max_records: Option<usize>) -> Result<Trajectory, String> {
    if !path.is_file() {
        return Err(format!("Session file not found: {}", path.display()));
    }
    let file_size = fs::metadata(path)
        .map_err(|error| format!("Failed to read session metadata: {error}"))?
        .len();
    if file_size <= FAST_TAIL_BYTES {
        return parse_page(path, max_records, None);
    }

    let segment = Segment {
        path: path.to_path_buf(),
        start_byte: Some(file_size.saturating_sub(FAST_TAIL_BYTES)),
        start_ordinal: None,
        end_ordinal: None,
        end_byte: None,
    };
    let projection = project_segments(path, vec![segment], Vec::new())?;
    Ok(paginate(&projection, max_records, None, false))
}

fn paginate(
    projection: &Trajectory,
    max_records: Option<usize>,
    before_record: Option<usize>,
    complete: bool,
) -> Trajectory {
    let total = projection.records.len();
    let page_size = max_records
        .unwrap_or(DEFAULT_PAGE_RECORDS)
        .clamp(MIN_PAGE_RECORDS, MAX_PAGE_RECORDS);
    let end_exclusive = before_record
        .unwrap_or(total.saturating_add(1))
        .clamp(1, total.saturating_add(1));
    let first_inclusive = end_exclusive.saturating_sub(page_size).max(1);

    let records = projection
        .records
        .iter()
        .filter(|record| record.index >= first_inclusive && record.index < end_exclusive)
        .cloned()
        .collect::<Vec<_>>();
    let visible_turns = records
        .iter()
        .map(|record| record.turn)
        .collect::<HashSet<_>>();
    let turns = projection
        .turns
        .iter()
        .filter(|turn| visible_turns.contains(&turn.index))
        .cloned()
        .collect::<Vec<_>>();

    let first_record = records.first().map(|record| record.index);
    let last_record = records.last().map(|record| record.index);
    let earlier_records = first_record.map_or(0, |index| index.saturating_sub(1));
    let later_records = last_record.map_or(total, |index| total.saturating_sub(index));
    let mut stats = projection.stats.clone();
    stats.visible_records = records.len();
    Trajectory {
        schema_version: projection.schema_version,
        generated_at: Utc::now().to_rfc3339(),
        session: projection.session.clone(),
        stats,
        pagination: TrajectoryPagination {
            complete,
            first_record,
            last_record,
            earlier_records,
            later_records,
            has_earlier: earlier_records > 0,
            has_later: later_records > 0,
            next_before_record: (earlier_records > 0).then_some(first_inclusive),
        },
        turns,
        records,
        warnings: projection.warnings.clone(),
    }
}

fn project(path: &Path) -> Result<Trajectory, String> {
    if !path.is_file() {
        return Err(format!("Session file not found: {}", path.display()));
    }
    let mut warnings = Vec::new();
    let segments = resolve_segments(path, &mut warnings);
    project_segments(path, segments, warnings)
}

fn project_segments(
    path: &Path,
    segments: Vec<Segment>,
    mut warnings: Vec<TrajectoryWarning>,
) -> Result<Trajectory, String> {
    let metadata = first_metadata(path).unwrap_or(Value::Object(Default::default()));
    let paginated = metadata
        .get("history_mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode == "paginated");
    let meta_id = metadata
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut session = TrajectorySession {
        id: meta_id,
        title: metadata
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        cwd: metadata
            .get("cwd")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        model: metadata
            .get("model")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        effort: metadata
            .get("effort")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        originator: metadata
            .get("originator")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        source_kind: metadata
            .get("source")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        started_at: None,
        updated_at: None,
        archived: path.to_string_lossy().contains("archived_sessions"),
        parent_thread_id: metadata
            .get("parent_thread_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        agent_path: metadata
            .get("agent_path")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        git_branch: metadata
            .get("git")
            .and_then(|v| v.get("branch"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
    };

    let mut turns = Vec::new();
    let mut records = Vec::new();
    let mut active_turn = None;
    let mut current_step = 0usize;
    let mut after_tool = false;
    let mut tool_records = HashMap::<String, usize>::new();
    let mut latest_total: Option<TokenUsage> = None;
    let mut first_time: Option<String> = None;
    let mut last_time: Option<String> = None;

    read_entries(&segments, |line_number, parsed_row| {
        let Ok(row) = parsed_row else {
            add_warning(
                &mut warnings,
                "malformed_json",
                line_number,
                "跳过无法解析的 JSONL 行。".to_string(),
            );
            return;
        };
        for _row in 0..1 {
            let row_type = row.get("type").and_then(Value::as_str).unwrap_or_default();
            let payload = row.get("payload").unwrap_or(&Value::Null);
            let payload_type = payload
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let row_timestamp = timestamp(row.get("timestamp"));
            if let Some(value) = row_timestamp.as_ref() {
                if first_time.as_ref().is_none_or(|current| value < current) {
                    first_time = Some(value.clone());
                }
                if last_time.as_ref().is_none_or(|current| value > current) {
                    last_time = Some(value.clone());
                }
            }

            if row_type == "session_meta" {
                continue;
            }
            if row_type == "event_msg" && payload_type == "thread_rolled_back" {
                match payload.get("num_turns").and_then(Value::as_u64) {
                    Some(turns) => add_warning(
                        &mut warnings,
                        "thread_rolled_back",
                        line_number,
                        format!("会话历史回滚了 {turns} 个用户 Turn；此前轨迹仍作为历史执行保留。"),
                    ),
                    None => add_warning(
                        &mut warnings,
                        "malformed_thread_rollback",
                        line_number,
                        "会话回滚事件缺少有效的 Turn 数量。".to_string(),
                    ),
                }
                continue;
            }
            if row_type == "inter_agent_communication" {
                let turn = ensure_turn(
                    &mut turns,
                    &mut active_turn,
                    row_timestamp.clone(),
                    None,
                    None,
                );
                let content = text_content(payload.get("content"));
                let mut record = record_base(
                    row_timestamp.clone(),
                    (current_step > 0).then_some(current_step),
                    "subagent",
                    "Agent communication",
                    if content.is_empty() {
                        "Agent 间通信".to_string()
                    } else {
                        truncate(&content, MAX_RECORD_TEXT)
                    },
                );
                record.output = (!content.is_empty()).then(|| truncate(&content, MAX_RECORD_TEXT));
                add_record(&mut records, &mut turns, turn, record);
                continue;
            }
            if row_type == "event_msg"
                && matches!(payload_type, "entered_review_mode" | "exited_review_mode")
            {
                let turn = ensure_turn(
                    &mut turns,
                    &mut active_turn,
                    row_timestamp.clone(),
                    None,
                    None,
                );
                let entered = payload_type == "entered_review_mode";
                let mut record = record_base(
                    row_timestamp.clone(),
                    (current_step > 0).then_some(current_step),
                    "assistant",
                    "Review mode",
                    if entered {
                        "进入审查模式"
                    } else {
                        "退出审查模式"
                    }
                    .to_string(),
                );
                record.output = bounded_value(
                    payload
                        .get("review_output")
                        .or_else(|| payload.get("target")),
                );
                add_record(&mut records, &mut turns, turn, record);
                continue;
            }
            if paginated && row_type == "response_item" && payload_type != "agent_message" {
                continue;
            }
            if paginated && row_type == "event_msg" && payload_type == "user_message" {
                continue;
            }
            if row_type == "turn_context" {
                let model = payload
                    .get("model")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                let turn = ensure_turn(
                    &mut turns,
                    &mut active_turn,
                    row_timestamp.clone(),
                    None,
                    model.clone(),
                );
                if model.is_some() {
                    turns[turn].model = model;
                }
                if session.model.is_none() {
                    session.model = payload
                        .get("model")
                        .and_then(Value::as_str)
                        .map(ToString::to_string);
                }
                continue;
            }
            if row_type == "event_msg" && matches!(payload_type, "task_started" | "turn_started") {
                if active_turn.is_some() {
                    let previous = active_turn.take().unwrap_or_default();
                    finish_turn(&mut turns[previous], row_timestamp.clone(), None, true);
                }
                let id = payload
                    .get("turn_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                let started = timestamp(payload.get("started_at")).or(row_timestamp.clone());
                ensure_turn(&mut turns, &mut active_turn, started, id, None);
                continue;
            }
            if row_type == "event_msg"
                && matches!(
                    payload_type,
                    "task_complete" | "turn_complete" | "turn_aborted"
                )
            {
                let turn = ensure_turn(
                    &mut turns,
                    &mut active_turn,
                    row_timestamp.clone(),
                    None,
                    None,
                );
                let error = payload
                    .get("error")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .or_else(|| {
                        payload
                            .get("status")
                            .and_then(Value::as_str)
                            .filter(|status| matches!(*status, "failed" | "error"))
                            .map(|status| status.to_string())
                    });
                let aborted = payload_type == "turn_aborted";
                finish_turn(&mut turns[turn], row_timestamp.clone(), error, aborted);
                active_turn = None;
                continue;
            }

            let needs_turn = row_type == "event_msg" && payload_type == "user_message"
                || row_type == "response_item"
                    && matches!(
                        payload_type,
                        "message"
                            | "reasoning"
                            | "function_call"
                            | "custom_tool_call"
                            | "local_shell_call"
                            | "tool_search_call"
                            | "web_search_call"
                            | "image_generation_call"
                            | "image_view"
                            | "file_change"
                            | "collab_agent_tool_call"
                    )
                || row_type == "response_item"
                    && matches!(
                        payload_type,
                        "function_call_output" | "custom_tool_call_output" | "tool_search_output"
                    )
                || row_type == "event_msg" && payload_type == "token_count";
            if !needs_turn {
                if row_type == "event_msg"
                    && matches!(
                        payload_type,
                        "mcp_tool_call_end"
                            | "patch_apply_end"
                            | "web_search_end"
                            | "image_generation_end"
                    )
                {
                    let turn = ensure_turn(
                        &mut turns,
                        &mut active_turn,
                        row_timestamp.clone(),
                        None,
                        None,
                    );
                    let call_id = payload
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let failed = payload.get("success").and_then(Value::as_bool) == Some(false)
                        || payload
                            .get("status")
                            .and_then(Value::as_str)
                            .map(|status| matches!(status, "failed" | "error" | "declined"))
                            .unwrap_or(false)
                        || output_is_error(payload.get("result"));
                    let event = match payload_type {
                        "mcp_tool_call_end" => "MCP tool",
                        "patch_apply_end" => "Apply patch",
                        "web_search_end" => "Web search",
                        _ => "Image generation",
                    };
                    if let Some(index) = tool_records.remove(&call_id) {
                        let record = &mut records[index];
                        record.output =
                            bounded_value(payload.get("result").or_else(|| payload.get("stdout")));
                        record.completed_at = row_timestamp.clone();
                        let duration =
                            payload.get("duration").and_then(Value::as_u64).or_else(|| {
                                millis_between(
                                    record.started_at.as_deref(),
                                    row_timestamp.as_deref(),
                                )
                            });
                        record.duration_ms = duration;
                        record.status = if failed { "error" } else { "complete" }.to_string();
                        record.summary = format!("{} · {}", record.event, record.status);
                    } else {
                        let mut record = record_base(
                            row_timestamp.clone(),
                            (current_step > 0).then_some(current_step),
                            "tool",
                            event,
                            event.to_string(),
                        );
                        record.status = if failed { "error" } else { "complete" }.to_string();
                        record.output =
                            bounded_value(payload.get("result").or_else(|| payload.get("stdout")));
                        record.call_id = (!call_id.is_empty()).then_some(call_id);
                        record.duration_ms = payload.get("duration").and_then(Value::as_u64);
                        add_record(&mut records, &mut turns, turn, record);
                    }
                    if failed {
                        turns[turn].error = Some("工具调用失败".to_string());
                    }
                    after_tool = true;
                    continue;
                }
                if row_type == "event_msg" && payload_type == "item_completed" {
                    let Some(item) = payload.get("item").and_then(Value::as_object) else {
                        add_warning(
                            &mut warnings,
                            "malformed_item_completed",
                            line_number,
                            "完成事件缺少有效的 item。".to_string(),
                        );
                        continue;
                    };
                    let item = Value::Object(item.clone());
                    let item_type = normalized_item_type(item.get("type"));
                    let started_at = timestamp_millis(payload.get("started_at_ms"))
                        .or_else(|| row_timestamp.clone());
                    let completed_at = timestamp_millis(payload.get("completed_at_ms"))
                        .or_else(|| row_timestamp.clone());
                    let turn = ensure_turn(
                        &mut turns,
                        &mut active_turn,
                        started_at.clone(),
                        payload
                            .get("turn_id")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        None,
                    );
                    match item_type.as_str() {
                        "user_message" | "hook_prompt" => {
                            let text = text_content(item.get("content"));
                            if session.title.trim().is_empty() && !text.trim().is_empty() {
                                session.title = truncate(&text, 120);
                            }
                            let summary = if text.is_empty() {
                                if item_type == "hook_prompt" {
                                    "内部 Hook prompt".to_string()
                                } else {
                                    "用户消息".to_string()
                                }
                            } else {
                                truncate(&text, MAX_RECORD_TEXT)
                            };
                            let mut record = record_base(
                                started_at.clone(),
                                None,
                                "user",
                                if item_type == "hook_prompt" {
                                    "Hook prompt"
                                } else {
                                    "User message"
                                },
                                summary,
                            );
                            record.input = (!text.is_empty())
                                .then(|| truncate(&text, MAX_RECORD_TEXT))
                                .or_else(|| bounded_value(item.get("fragments")));
                            apply_record_timing(
                                &mut record,
                                started_at.clone(),
                                completed_at.clone(),
                            );
                            add_record(&mut records, &mut turns, turn, record);
                        }
                        "agent_message" | "message" | "plan" | "reasoning" => {
                            if after_tool {
                                current_step = current_step.saturating_add(1).max(1);
                                after_tool = false;
                            }
                            if current_step == 0 {
                                current_step = 1;
                            }
                            let text = if item_type == "reasoning" {
                                text_content(
                                    item.get("summary_text")
                                        .or_else(|| item.get("summary"))
                                        .or_else(|| item.get("text")),
                                )
                            } else if item_type == "plan" {
                                text_content(item.get("text"))
                            } else {
                                text_content(item.get("content"))
                            };
                            let (kind, event) = if item_type == "reasoning" {
                                ("reasoning", "Reasoning")
                            } else if item_type == "plan" {
                                ("assistant", "Plan")
                            } else {
                                ("assistant", "Assistant")
                            };
                            let mut record = record_base(
                                started_at.clone(),
                                Some(current_step),
                                kind,
                                event,
                                if text.is_empty() {
                                    event.to_string()
                                } else {
                                    truncate(&text, MAX_RECORD_TEXT)
                                },
                            );
                            record.output =
                                (!text.is_empty()).then(|| truncate(&text, MAX_RECORD_TEXT));
                            apply_record_timing(
                                &mut record,
                                started_at.clone(),
                                completed_at.clone(),
                            );
                            let record_index = add_record(&mut records, &mut turns, turn, record);
                            if kind == "assistant" && turns[turn].time_to_first_token_ms.is_none() {
                                turns[turn].time_to_first_token_ms = millis_between(
                                    turns[turn].started_at.as_deref(),
                                    records[record_index].started_at.as_deref(),
                                );
                            }
                        }
                        "context_compaction" | "compaction" => {
                            let mut record = record_base(
                                started_at.clone(),
                                (current_step > 0).then_some(current_step),
                                "compaction",
                                "Compaction",
                                "上下文已压缩".to_string(),
                            );
                            record.output = bounded_value(item.get("message"));
                            apply_record_timing(
                                &mut record,
                                started_at.clone(),
                                completed_at.clone(),
                            );
                            add_record(&mut records, &mut turns, turn, record);
                        }
                        "sub_agent_activity" => {
                            let activity = item
                                .get("kind")
                                .and_then(Value::as_str)
                                .unwrap_or("activity");
                            let agent = item
                                .get("agent_path")
                                .and_then(Value::as_str)
                                .unwrap_or("subagent");
                            let mut record = record_base(
                                started_at.clone(),
                                (current_step > 0).then_some(current_step),
                                "subagent",
                                &format!("Subagent · {activity}"),
                                format!("{agent} · {activity}"),
                            );
                            apply_record_timing(
                                &mut record,
                                started_at.clone(),
                                completed_at.clone(),
                            );
                            add_record(&mut records, &mut turns, turn, record);
                        }
                        "entered_review_mode" | "exited_review_mode" => {
                            let entered = item_type == "entered_review_mode";
                            let mut record = record_base(
                                started_at.clone(),
                                (current_step > 0).then_some(current_step),
                                "assistant",
                                "Review mode",
                                if entered {
                                    "进入审查模式"
                                } else {
                                    "退出审查模式"
                                }
                                .to_string(),
                            );
                            record.output = bounded_value(
                                item.get("review_output")
                                    .or_else(|| item.get("target"))
                                    .or_else(|| item.get("user_facing_hint")),
                            );
                            apply_record_timing(
                                &mut record,
                                started_at.clone(),
                                completed_at.clone(),
                            );
                            add_record(&mut records, &mut turns, turn, record);
                        }
                        "command_execution"
                        | "dynamic_tool_call"
                        | "collab_agent_tool_call"
                        | "web_search"
                        | "image_view"
                        | "image_generation"
                        | "file_change"
                        | "mcp_tool_call"
                        | "extension" => {
                            let event = match item_type.as_str() {
                                "command_execution" => "Command".to_string(),
                                "dynamic_tool_call" => item
                                    .get("tool")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Dynamic tool")
                                    .to_string(),
                                "collab_agent_tool_call" => item
                                    .get("tool")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Agent tool")
                                    .to_string(),
                                "web_search" => "Web search".to_string(),
                                "image_view" => "View image".to_string(),
                                "image_generation" => "Image generation".to_string(),
                                "file_change" => "Apply patch".to_string(),
                                "extension" => item
                                    .get("kind")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Extension")
                                    .to_string(),
                                _ => item
                                    .get("tool")
                                    .or_else(|| item.get("name"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("Tool")
                                    .to_string(),
                            };
                            let input = match item_type.as_str() {
                                "command_execution" => item.get("command"),
                                "dynamic_tool_call" | "mcp_tool_call" => item.get("arguments"),
                                "collab_agent_tool_call" => item.get("prompt"),
                                "web_search" => item.get("action").or_else(|| item.get("query")),
                                "image_view" => item.get("path"),
                                "image_generation" => item.get("revised_prompt"),
                                "file_change" => item.get("changes"),
                                _ => item.get("input"),
                            };
                            let output = item
                                .get("output")
                                .or_else(|| item.get("result"))
                                .or_else(|| item.get("stdout"))
                                .or_else(|| item.get("agents_states"))
                                .or_else(|| item.get("saved_path"))
                                .or_else(|| item.get("error"));
                            let failed = failed_status(item.get("status"))
                                || item.get("success").and_then(Value::as_bool) == Some(false)
                                || item.get("error").is_some_and(|value| !value.is_null())
                                || output_is_error(output);
                            let mut record = record_base(
                                started_at.clone(),
                                (current_step > 0).then_some(current_step),
                                "tool",
                                &event,
                                format!("{event} · {}", if failed { "error" } else { "complete" }),
                            );
                            record.status = if failed { "error" } else { "complete" }.to_string();
                            record.input = bounded_value(input);
                            record.output = bounded_value(output);
                            record.call_id = item
                                .get("call_id")
                                .or_else(|| item.get("id"))
                                .and_then(Value::as_str)
                                .map(ToString::to_string);
                            apply_record_timing(
                                &mut record,
                                started_at.clone(),
                                completed_at.clone(),
                            );
                            add_record(&mut records, &mut turns, turn, record);
                            if failed {
                                turns[turn].error = Some("工具调用失败".to_string());
                            }
                            after_tool = true;
                        }
                        _ => add_warning(
                            &mut warnings,
                            "unsupported_item",
                            line_number,
                            format!("跳过未支持的完成事件 {item_type}。"),
                        ),
                    }
                    continue;
                }
                if row_type == "compacted"
                    || row_type == "event_msg" && payload_type == "context_compacted"
                {
                    let turn = ensure_turn(
                        &mut turns,
                        &mut active_turn,
                        row_timestamp.clone(),
                        None,
                        None,
                    );
                    let record = record_base(
                        row_timestamp.clone(),
                        (current_step > 0).then_some(current_step),
                        "compaction",
                        "Compaction",
                        "上下文已压缩".to_string(),
                    );
                    add_record(&mut records, &mut turns, turn, record);
                    continue;
                }
                if row_type == "event_msg" && payload_type == "sub_agent_activity" {
                    let turn = ensure_turn(
                        &mut turns,
                        &mut active_turn,
                        row_timestamp.clone(),
                        None,
                        None,
                    );
                    let activity = payload
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("activity");
                    let agent = payload
                        .get("agent_path")
                        .and_then(Value::as_str)
                        .unwrap_or("subagent");
                    let record = record_base(
                        row_timestamp.clone(),
                        (current_step > 0).then_some(current_step),
                        "subagent",
                        &format!("Subagent · {activity}"),
                        format!("{agent} · {activity}"),
                    );
                    add_record(&mut records, &mut turns, turn, record);
                    continue;
                }
                if row_type == "response_item" && payload_type == "agent_message" {
                    let turn = ensure_turn(
                        &mut turns,
                        &mut active_turn,
                        row_timestamp.clone(),
                        None,
                        None,
                    );
                    let text = text_content(payload.get("content"));
                    let mut record = record_base(
                        row_timestamp.clone(),
                        (current_step > 0).then_some(current_step),
                        "subagent",
                        "Agent message",
                        truncate(&text, MAX_RECORD_TEXT),
                    );
                    record.output = (!text.is_empty()).then(|| truncate(&text, MAX_RECORD_TEXT));
                    add_record(&mut records, &mut turns, turn, record);
                    continue;
                }
                if row_type == "response_item" && !payload_type.is_empty() {
                    add_warning(
                        &mut warnings,
                        "unsupported_response_item",
                        line_number,
                        format!("跳过未支持的持久化响应事件 {payload_type}。"),
                    );
                }
                continue;
            }

            let turn = ensure_turn(
                &mut turns,
                &mut active_turn,
                row_timestamp.clone(),
                None,
                None,
            );
            if row_type == "event_msg" && payload_type == "token_count" {
                let info = payload.get("info");
                let total = token_usage(info.and_then(|v| v.get("total_token_usage")));
                let last = token_usage(info.and_then(|v| v.get("last_token_usage")));
                if let Some(total_usage) = total.clone() {
                    if latest_total.as_ref() != Some(&total_usage) {
                        let delta = last.unwrap_or_else(|| {
                            let previous = latest_total.clone().unwrap_or_default();
                            TokenUsage {
                                input_tokens: total_usage
                                    .input_tokens
                                    .saturating_sub(previous.input_tokens),
                                cached_input_tokens: total_usage
                                    .cached_input_tokens
                                    .saturating_sub(previous.cached_input_tokens),
                                output_tokens: total_usage
                                    .output_tokens
                                    .saturating_sub(previous.output_tokens),
                                reasoning_output_tokens: total_usage
                                    .reasoning_output_tokens
                                    .saturating_sub(previous.reasoning_output_tokens),
                                total_tokens: total_usage
                                    .total_tokens
                                    .saturating_sub(previous.total_tokens),
                            }
                        });
                        let usage = add_usage(turns[turn].usage.as_ref(), &delta);
                        turns[turn].usage = Some(usage);
                        turns[turn].model_calls += 1;
                        latest_total = Some(total_usage);
                    }
                }
                continue;
            }

            if row_type == "event_msg" && payload_type == "user_message" {
                let text = text_content(payload.get("message").or_else(|| payload.get("content")));
                if session.title.trim().is_empty() && !text.trim().is_empty() {
                    session.title = truncate(&text, 120);
                }
                let record = record_base(
                    row_timestamp.clone(),
                    None,
                    "user",
                    "User message",
                    truncate(&text, MAX_RECORD_TEXT),
                );
                add_record(&mut records, &mut turns, turn, record);
                continue;
            }

            if row_type == "response_item" && payload_type == "message" {
                let role = payload
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if role != "user" && role != "assistant" {
                    continue;
                }
                let text = text_content(payload.get("content"));
                if role == "user" && session.title.trim().is_empty() && !text.trim().is_empty() {
                    session.title = truncate(&text, 120);
                }
                if role == "assistant" && after_tool {
                    current_step = current_step.saturating_add(1).max(1);
                    after_tool = false;
                }
                if current_step == 0 {
                    current_step = 1;
                }
                let model = payload
                    .get("model")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                if model.is_some() {
                    turns[turn].model = model;
                }
                let mut record = record_base(
                    row_timestamp.clone(),
                    Some(current_step),
                    role,
                    if role == "assistant" {
                        "Assistant"
                    } else {
                        "User message"
                    },
                    truncate(&text, MAX_RECORD_TEXT),
                );
                record.output = (role == "assistant" && !text.is_empty())
                    .then(|| truncate(&text, MAX_RECORD_TEXT));
                record.input =
                    (role == "user" && !text.is_empty()).then(|| truncate(&text, MAX_RECORD_TEXT));
                let index = add_record(&mut records, &mut turns, turn, record);
                if role == "assistant" && turns[turn].time_to_first_token_ms.is_none() {
                    let started_at = turns[turn].started_at.clone();
                    let record_timestamp = records[index].timestamp.clone();
                    turns[turn].time_to_first_token_ms =
                        millis_between(started_at.as_deref(), record_timestamp.as_deref());
                }
                continue;
            }

            if row_type == "response_item" && payload_type == "reasoning" {
                let text = text_content(payload.get("text").or_else(|| payload.get("summary")));
                if text.is_empty() {
                    continue;
                }
                if after_tool {
                    current_step = current_step.saturating_add(1).max(1);
                    after_tool = false;
                }
                if current_step == 0 {
                    current_step = 1;
                }
                let mut record = record_base(
                    row_timestamp.clone(),
                    Some(current_step),
                    "reasoning",
                    "Reasoning",
                    truncate(&text, MAX_RECORD_TEXT),
                );
                record.output = Some(truncate(&text, MAX_RECORD_TEXT));
                add_record(&mut records, &mut turns, turn, record);
                continue;
            }

            if row_type == "response_item" && payload_type == "tool_search_call" {
                if current_step == 0 {
                    current_step = 1;
                }
                let call_id = payload
                    .get("call_id")
                    .or_else(|| payload.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let event = payload
                    .get("execution")
                    .and_then(Value::as_str)
                    .unwrap_or("Tool search");
                let failed = failed_status(payload.get("status"));
                let complete =
                    payload
                        .get("status")
                        .and_then(Value::as_str)
                        .is_some_and(|status| {
                            matches!(status, "completed" | "complete" | "failed" | "incomplete")
                        });
                let mut record = record_base(
                    row_timestamp.clone(),
                    Some(current_step),
                    "tool",
                    event,
                    format!(
                        "{event} · {}",
                        if complete {
                            if failed {
                                "error"
                            } else {
                                "complete"
                            }
                        } else {
                            "running"
                        }
                    ),
                );
                record.status = if complete {
                    if failed {
                        "error"
                    } else {
                        "complete"
                    }
                } else {
                    "running"
                }
                .to_string();
                record.input = bounded_value(payload.get("arguments"));
                record.call_id = (!call_id.is_empty()).then_some(call_id.clone());
                let index = add_record(&mut records, &mut turns, turn, record);
                if !complete && !call_id.is_empty() {
                    tool_records.insert(call_id, index);
                }
                if complete {
                    after_tool = true;
                }
                continue;
            }

            if row_type == "response_item" && payload_type == "tool_search_output" {
                let call_id = payload
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let output = bounded_value(payload.get("tools"));
                let failed = failed_status(payload.get("status"));
                if let Some(index) = tool_records.remove(&call_id) {
                    let record = &mut records[index];
                    record.output = output;
                    record.completed_at = row_timestamp.clone();
                    record.duration_ms =
                        millis_between(record.started_at.as_deref(), row_timestamp.as_deref());
                    record.status = if failed { "error" } else { "complete" }.to_string();
                    record.summary = format!("{} · {}", record.event, record.status);
                } else {
                    let mut record = record_base(
                        row_timestamp.clone(),
                        (current_step > 0).then_some(current_step),
                        "tool",
                        "Tool search result",
                        "未匹配的工具搜索结果".to_string(),
                    );
                    record.output = output;
                    record.status = if failed { "error" } else { "complete" }.to_string();
                    record.call_id = (!call_id.is_empty()).then_some(call_id);
                    add_record(&mut records, &mut turns, turn, record);
                }
                if failed {
                    turns[turn].error = Some("工具调用失败".to_string());
                }
                after_tool = true;
                continue;
            }

            if row_type == "response_item"
                && matches!(
                    payload_type,
                    "web_search_call"
                        | "image_generation_call"
                        | "image_view"
                        | "file_change"
                        | "collab_agent_tool_call"
                )
            {
                if current_step == 0 {
                    current_step = 1;
                }
                let event = match payload_type {
                    "web_search_call" => "Web search",
                    "image_generation_call" => "Image generation",
                    "image_view" => "View image",
                    "file_change" => "Apply patch",
                    _ => payload
                        .get("tool")
                        .and_then(Value::as_str)
                        .unwrap_or("Agent tool"),
                };
                let input = match payload_type {
                    "web_search_call" => payload.get("action"),
                    "image_generation_call" => payload.get("revised_prompt"),
                    "image_view" => payload.get("path"),
                    "file_change" => payload.get("changes"),
                    _ => payload.get("prompt").or_else(|| payload.get("arguments")),
                };
                let output = payload
                    .get("output")
                    .or_else(|| payload.get("result"))
                    .or_else(|| payload.get("saved_path"))
                    .or_else(|| payload.get("agents_states"))
                    .or_else(|| payload.get("error"));
                let failed = failed_status(payload.get("status"))
                    || payload.get("success").and_then(Value::as_bool) == Some(false)
                    || payload.get("error").is_some_and(|value| !value.is_null());
                let mut record = record_base(
                    row_timestamp.clone(),
                    Some(current_step),
                    "tool",
                    event,
                    format!("{event} · {}", if failed { "error" } else { "complete" }),
                );
                record.status = if failed { "error" } else { "complete" }.to_string();
                record.input = bounded_value(input);
                record.output = bounded_value(output);
                record.call_id = payload
                    .get("call_id")
                    .or_else(|| payload.get("id"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                add_record(&mut records, &mut turns, turn, record);
                if failed {
                    turns[turn].error = Some("工具调用失败".to_string());
                }
                after_tool = true;
                continue;
            }

            if row_type == "response_item"
                && matches!(
                    payload_type,
                    "function_call" | "custom_tool_call" | "local_shell_call"
                )
            {
                if current_step == 0 {
                    current_step = 1;
                }
                let name = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .or_else(|| payload.get("call_id").and_then(Value::as_str))
                    .unwrap_or("Tool");
                let call_id = payload
                    .get("call_id")
                    .or_else(|| payload.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let input = bounded_value(
                    payload
                        .get("arguments")
                        .or_else(|| payload.get("input"))
                        .or_else(|| payload.get("action")),
                );
                let terminal = payload_type == "local_shell_call"
                    && payload
                        .get("status")
                        .and_then(Value::as_str)
                        .is_some_and(|status| {
                            matches!(status, "completed" | "complete" | "failed" | "incomplete")
                        });
                let failed = failed_status(payload.get("status"));
                let mut record = record_base(
                    row_timestamp.clone(),
                    Some(current_step),
                    "tool",
                    name,
                    format!(
                        "{name} · {}",
                        if terminal {
                            if failed {
                                "error"
                            } else {
                                "complete"
                            }
                        } else {
                            "running"
                        }
                    ),
                );
                record.status = if terminal {
                    if failed {
                        "error"
                    } else {
                        "complete"
                    }
                } else {
                    "running"
                }
                .to_string();
                record.input = input;
                record.output = terminal
                    .then(|| bounded_value(payload.get("output").or_else(|| payload.get("result"))))
                    .flatten();
                record.call_id = (!call_id.is_empty()).then_some(call_id.clone());
                let index = add_record(&mut records, &mut turns, turn, record);
                if !terminal && !call_id.is_empty() {
                    tool_records.insert(call_id, index);
                }
                if terminal {
                    if failed {
                        turns[turn].error = Some("工具调用失败".to_string());
                    }
                    after_tool = true;
                }
                continue;
            }

            if row_type == "response_item"
                && matches!(
                    payload_type,
                    "function_call_output" | "custom_tool_call_output"
                )
            {
                let call_id = payload
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let output = bounded_value(payload.get("output"));
                let failed = output_is_error(payload.get("output"));
                if let Some(index) = tool_records.remove(&call_id) {
                    let record = &mut records[index];
                    record.output = output;
                    record.completed_at = row_timestamp.clone();
                    record.duration_ms =
                        millis_between(record.started_at.as_deref(), row_timestamp.as_deref());
                    record.status = if failed { "error" } else { "complete" }.to_string();
                    record.summary = format!("{} · {}", record.event, record.status);
                    if failed {
                        turns[turn].error = Some("工具调用失败".to_string());
                    }
                } else {
                    let mut record = record_base(
                        row_timestamp.clone(),
                        (current_step > 0).then_some(current_step),
                        "tool",
                        "Tool result",
                        "未匹配的工具输出".to_string(),
                    );
                    record.output = output;
                    record.status = if failed { "error" } else { "complete" }.to_string();
                    record.call_id = (!call_id.is_empty()).then_some(call_id);
                    add_record(&mut records, &mut turns, turn, record);
                }
                after_tool = true;
                continue;
            }
        }
    });

    if let Some(index) = active_turn {
        finish_turn(&mut turns[index], last_time.clone(), None, false);
    }
    for turn in &mut turns {
        if turn.status.is_empty() {
            turn.status = "complete".to_string();
        }
        if turn.completed_at.is_none() {
            turn.completed_at = last_time.clone();
        }
        if turn.duration_ms.is_none() {
            turn.duration_ms =
                millis_between(turn.started_at.as_deref(), turn.completed_at.as_deref());
        }
    }
    session.started_at = first_time.clone();
    session.updated_at = last_time.clone();
    let failed_tools = records
        .iter()
        .filter(|record| record.kind == "tool" && record.status == "error")
        .count();
    let compactions = records
        .iter()
        .filter(|record| record.kind == "compaction")
        .count();
    let duration_ms = millis_between(first_time.as_deref(), last_time.as_deref());
    Ok(Trajectory {
        schema_version: 1,
        generated_at: Utc::now().to_rfc3339(),
        session,
        stats: TrajectoryStats {
            turns: turns.len(),
            records: records.len(),
            visible_records: records.len(),
            tool_calls: records
                .iter()
                .filter(|record| record.kind == "tool")
                .count(),
            failed_tools,
            compactions,
            tokens: latest_total,
            duration_ms,
        },
        pagination: TrajectoryPagination::default(),
        turns,
        records,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn projects_turns_tools_and_token_deltas() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("asv-trajectory-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("rollout-test.jsonl");
        let rows = [
            serde_json::json!({"type":"session_meta","payload":{"id":"test","cwd":"C:/repo","source":"cli","model":"gpt-5"}}),
            serde_json::json!({"type":"event_msg","timestamp":"2026-08-19T00:00:00Z","payload":{"type":"task_started","turn_id":"turn-1"}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-08-19T00:00:01Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"inspect"}]}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-08-19T00:00:02Z","payload":{"type":"function_call","name":"shell","call_id":"call-1","arguments":"{\"command\":\"pwd\"}"}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-08-19T00:00:03Z","payload":{"type":"function_call_output","call_id":"call-1","output":"C:/repo"}}),
            serde_json::json!({"type":"response_item","timestamp":"2026-08-19T00:00:04Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}),
            serde_json::json!({"type":"event_msg","timestamp":"2026-08-19T00:00:05Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"output_tokens":4},"total_token_usage":{"input_tokens":10,"output_tokens":4,"total_tokens":14}}}}),
            serde_json::json!({"type":"event_msg","timestamp":"2026-08-19T00:00:06Z","payload":{"type":"task_complete"}}),
        ];
        let data = rows
            .iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>();
        fs::write(&path, data).expect("write rollout");

        let trajectory = parse(&path).expect("parse trajectory");
        assert_eq!(trajectory.schema_version, 1);
        assert_eq!(trajectory.turns.len(), 1);
        assert_eq!(trajectory.stats.tool_calls, 1);
        assert_eq!(trajectory.stats.failed_tools, 0);
        assert_eq!(
            trajectory
                .stats
                .tokens
                .as_ref()
                .map(|usage| usage.total_tokens),
            Some(14)
        );
        assert!(trajectory
            .records
            .iter()
            .any(|record| record.event == "shell" && record.status == "complete"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn paginates_by_stable_record_index_without_overlap() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("asv-trajectory-page-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("rollout-page.jsonl");
        let mut rows = vec![
            serde_json::json!({"type":"session_meta","payload":{"id":"page-test","history_mode":"paginated"}}),
            serde_json::json!({"type":"event_msg","timestamp":"2026-08-19T00:00:00Z","payload":{"type":"turn_started","turn_id":"turn-page"}}),
        ];
        for index in 1..=120 {
            rows.push(serde_json::json!({
                "type":"event_msg",
                "timestamp":"2026-08-19T00:00:01Z",
                "payload":{
                    "type":"item_completed",
                    "turn_id":"turn-page",
                    "started_at_ms":1787097601000u64 + index,
                    "completed_at_ms":1787097601000u64 + index,
                    "item":{"type":"UserMessage","id":format!("item-{index}"),"content":[{"type":"text","text":format!("message {index}")}]}
                }
            }));
        }
        rows.push(serde_json::json!({"type":"event_msg","timestamp":"2026-08-19T00:01:00Z","payload":{"type":"turn_complete"}}));
        let data = rows
            .iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>();
        fs::write(&path, data).expect("write rollout");

        let latest = parse_page(&path, Some(50), None).expect("latest page");
        let earlier = parse_page(&path, Some(50), latest.pagination.next_before_record)
            .expect("earlier page");

        assert_eq!(latest.stats.records, 120);
        assert_eq!(earlier.stats.records, 120);
        assert_eq!(latest.stats.visible_records, 50);
        assert_eq!(latest.pagination.first_record, Some(71));
        assert_eq!(earlier.pagination.first_record, Some(21));
        assert_eq!(earlier.pagination.last_record, Some(70));
        assert!(earlier.records.iter().all(|record| record.index < 71));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn projects_modern_completed_items_and_rollback_warning() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("asv-trajectory-events-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("rollout-events.jsonl");
        let item_types = [
            serde_json::json!({"type":"AgentMessage","content":[{"type":"text","text":"answer"}]}),
            serde_json::json!({"type":"Reasoning","summary_text":["thinking"]}),
            serde_json::json!({"type":"WebSearch","status":"completed","action":{"type":"search","query":"query"}}),
            serde_json::json!({"type":"ImageGeneration","status":"completed","revised_prompt":"draw","saved_path":"image.png"}),
            serde_json::json!({"type":"FileChange","status":"completed","changes":[{"path":"src/lib.rs"}]}),
            serde_json::json!({"type":"CollabAgentToolCall","status":"completed","tool":"spawn_agent","prompt":"inspect"}),
            serde_json::json!({"type":"EnteredReviewMode","target":"working tree"}),
            serde_json::json!({"type":"ContextCompaction"}),
        ];
        let mut rows = vec![
            serde_json::json!({"type":"session_meta","payload":{"id":"events-test","history_mode":"paginated"}}),
            serde_json::json!({"type":"event_msg","timestamp":"2026-08-19T00:00:00Z","payload":{"type":"turn_started","turn_id":"turn-events"}}),
        ];
        for (index, item) in item_types.into_iter().enumerate() {
            rows.push(serde_json::json!({
                "type":"event_msg",
                "timestamp":"2026-08-19T00:00:01Z",
                "payload":{
                    "type":"item_completed",
                    "turn_id":"turn-events",
                    "started_at_ms":1787097601000u64 + index as u64,
                    "completed_at_ms":1787097601100u64 + index as u64,
                    "item":item
                }
            }));
        }
        rows.push(serde_json::json!({"type":"event_msg","timestamp":"2026-08-19T00:00:10Z","payload":{"type":"thread_rolled_back","num_turns":1}}));
        rows.push(serde_json::json!({"type":"inter_agent_communication","timestamp":"2026-08-19T00:00:11Z","payload":{"content":"worker done"}}));
        let data = rows
            .iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>();
        fs::write(&path, data).expect("write rollout");

        let trajectory = parse(&path).expect("parse trajectory");
        for event in [
            "Assistant",
            "Reasoning",
            "Web search",
            "Image generation",
            "Apply patch",
            "spawn_agent",
            "Review mode",
            "Compaction",
            "Agent communication",
        ] {
            assert!(
                trajectory
                    .records
                    .iter()
                    .any(|record| record.event == event),
                "missing event {event}"
            );
        }
        assert!(trajectory
            .warnings
            .iter()
            .any(|warning| warning.code == "thread_rolled_back"));

        let _ = fs::remove_dir_all(dir);
    }
}
