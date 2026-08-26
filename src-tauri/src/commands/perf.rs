use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MAX_EVENTS_PER_BATCH: usize = 50;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfDiagnosticEvent {
    timestamp: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<f64>,
    #[serde(default)]
    fields: Value,
}

pub fn enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("ASV_PERF_DIAGNOSTICS")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes"))
}

pub fn emit_backend(name: &str, duration_ms: f64, fields: Value) {
    if !enabled() {
        return;
    }

    let event = json!({
        "timestamp": Utc::now().to_rfc3339(),
        "name": name,
        "durationMs": (duration_ms * 100.0).round() / 100.0,
        "fields": fields,
    });
    eprintln!("[ASV-PERF] {event}");
}

#[tauri::command]
pub fn report_perf_events(events: Vec<PerfDiagnosticEvent>) -> Result<(), String> {
    if !enabled() {
        return Ok(());
    }

    for event in events.into_iter().take(MAX_EVENTS_PER_BATCH) {
        let serialized = serde_json::to_string(&event)
            .map_err(|error| format!("性能诊断事件序列化失败: {error}"))?;
        eprintln!("[ASV-PERF] {serialized}");
    }
    Ok(())
}
