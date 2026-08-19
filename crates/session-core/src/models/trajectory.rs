use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Trajectory {
    pub schema_version: u32,
    pub generated_at: String,
    pub session: TrajectorySession,
    pub stats: TrajectoryStats,
    pub pagination: TrajectoryPagination,
    pub turns: Vec<TrajectoryTurn>,
    pub records: Vec<TrajectoryRecord>,
    pub warnings: Vec<TrajectoryWarning>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryPagination {
    pub first_record: Option<usize>,
    pub last_record: Option<usize>,
    pub earlier_records: usize,
    pub later_records: usize,
    pub has_earlier: bool,
    pub has_later: bool,
    pub next_before_record: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrajectorySession {
    pub id: String,
    pub title: String,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub originator: Option<String>,
    pub source_kind: Option<String>,
    pub started_at: Option<String>,
    pub updated_at: Option<String>,
    pub archived: bool,
    pub parent_thread_id: Option<String>,
    pub agent_path: Option<String>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryStats {
    pub turns: usize,
    pub records: usize,
    pub visible_records: usize,
    pub tool_calls: usize,
    pub failed_tools: usize,
    pub compactions: usize,
    pub tokens: Option<TokenUsage>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryTurn {
    pub index: usize,
    pub id: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub time_to_first_token_ms: Option<u64>,
    pub status: String,
    pub error: Option<String>,
    pub records: usize,
    pub steps: usize,
    pub model_calls: usize,
    pub usage: Option<TokenUsage>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryRecord {
    pub index: usize,
    pub turn: usize,
    pub step: Option<usize>,
    pub kind: String,
    pub event: String,
    pub summary: String,
    pub timestamp: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub status: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub call_id: Option<String>,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryWarning {
    pub code: String,
    pub line: usize,
    pub message: String,
}
