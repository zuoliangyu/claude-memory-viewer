export interface ProjectEntry {
  source: string;
  id: string;
  displayPath: string;
  shortName: string;
  sessionCount: number;
  lastModified: string | null;
  modelProvider: string | null;
  alias: string | null;
  pathExists: boolean;
  /** Codex only: synthetic "unrooted" project that buckets sessions with no
   *  cwd by date. Real cwd projects keep this falsy. */
  isVirtual?: boolean;
}

/** A discovered skill (one `SKILL.md` directory). Mirrors the Rust
 *  `SkillEntry` in session-core. */
export interface SkillEntry {
  name: string;
  description: string;
  /** Absolute path to the skill's SKILL.md. */
  path: string;
  scope: "global" | "project" | "plugin";
  /** For plugin skills: the marketplace / source dir name. */
  sourceLabel: string | null;
  slug: string;
  /** True when the skill dir is a symlink (deleting only removes the link). */
  isSymlink: boolean;
}

/** Grouped result of a skills scan. Mirrors Rust `SkillsResult`. */
export interface SkillsResult {
  global: SkillEntry[];
  plugin: SkillEntry[];
  project: SkillEntry[];
  projectPath: string | null;
}

/** Outcome of importing skills from an archive. Mirrors Rust `ImportResult`. */
export interface ImportResult {
  imported: string[];
  skipped: string[];
  errors: string[];
}

/** Writable skill scopes (plugin skills are read-only here). */
export type SkillScope = "global" | "project";

/** Scan-time health classification of a session file.
 *  - `valid`   — has messages and JSONL parsed cleanly.
 *  - `empty`   — file exists but has no user/assistant messages.
 *  - `corrupt` — has messages but a non-last line failed to parse
 *               (typically mid-file NUL bytes from a crashed CC writer). */
export type SessionStatus = "valid" | "empty" | "corrupt";

export interface SessionIndexEntry {
  source: string;
  sessionId: string;
  filePath: string;
  firstPrompt: string | null;
  /** Codex only: Codex Desktop's human-readable thread title from
   *  session_index.jsonl. Preferred over firstPrompt for display. */
  threadName: string | null;
  messageCount: number;
  created: string | null;
  modified: string | null;
  gitBranch: string | null;
  projectPath: string | null;
  // Claude-specific
  isSidechain: boolean | null;
  // Codex-specific
  cwd: string | null;
  modelProvider: string | null;
  cliVersion: string | null;
  // User metadata
  alias: string | null;
  tags: string[] | null;
  /** Optional for backward compat with older API responses. Treat
   *  missing value as `"valid"` for main-list rows and `"empty"` for
   *  rows returned by `getInvalidSessions`. */
  status?: SessionStatus;
}

export type DisplayContentBlock =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string }
  | { type: "tool_use"; id: string; name: string; input: string }
  | { type: "tool_result"; toolUseId: string; content: string; isError: boolean }
  | { type: "reasoning"; text: string }
  | { type: "function_call"; name: string; arguments: string; callId: string }
  | { type: "function_call_output"; callId: string; output: string };

export interface DisplayMessage {
  uuid: string | null;
  parentUuid: string | null;
  role: string;
  timestamp: string | null;
  model: string | null;
  content: DisplayContentBlock[];
}

/** Session-wide navigation metadata for a user question. */
export interface QuestionIndexEntry {
  messageIndex: number;
  messageId: string;
  preview: string;
  timestamp: string | null;
  parentMessageIndex: number | null;
  replyPreview: string;
  replyModel: string | null;
  replyTimestamp: string | null;
  hasTool: boolean;
}

export interface PaginatedMessages {
  messages: DisplayMessage[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
}

/** Result of a range-based message load `[start, end)`. */
export interface RangeMessages {
  messages: DisplayMessage[];
  total: number;
  start: number;
  end: number;
}

export interface TrajectoryTokenUsage {
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
}

export interface TrajectorySession {
  id: string;
  title: string;
  cwd: string | null;
  model: string | null;
  effort: string | null;
  originator: string | null;
  sourceKind: string | null;
  startedAt: string | null;
  updatedAt: string | null;
  archived: boolean;
  parentThreadId: string | null;
  agentPath: string | null;
  gitBranch: string | null;
}

export interface TrajectoryStats {
  turns: number;
  records: number;
  visibleRecords: number;
  toolCalls: number;
  failedTools: number;
  compactions: number;
  tokens: TrajectoryTokenUsage | null;
  durationMs: number | null;
}

export interface TrajectoryTurn {
  index: number;
  id: string | null;
  startedAt: string | null;
  completedAt: string | null;
  durationMs: number | null;
  timeToFirstTokenMs: number | null;
  status: "running" | "complete" | "error" | "aborted" | string;
  error: string | null;
  records: number;
  steps: number;
  modelCalls: number;
  usage: TrajectoryTokenUsage | null;
  model: string | null;
}

export interface TrajectoryRecord {
  index: number;
  turn: number;
  step: number | null;
  kind: "user" | "assistant" | "reasoning" | "tool" | "subagent" | "compaction" | string;
  event: string;
  summary: string;
  timestamp: string | null;
  startedAt: string | null;
  completedAt: string | null;
  durationMs: number | null;
  status: "running" | "complete" | "error" | "aborted" | string;
  input: string | null;
  output: string | null;
  callId: string | null;
  tokenUsage: TrajectoryTokenUsage | null;
}

export interface TrajectoryWarning {
  code: string;
  line: number;
  message: string;
}

export interface TrajectoryPagination {
  complete: boolean;
  firstRecord: number | null;
  lastRecord: number | null;
  earlierRecords: number;
  laterRecords: number;
  hasEarlier: boolean;
  hasLater: boolean;
  nextBeforeRecord: number | null;
}

export interface Trajectory {
  schemaVersion: number;
  generatedAt: string;
  session: TrajectorySession;
  stats: TrajectoryStats;
  pagination: TrajectoryPagination;
  turns: TrajectoryTurn[];
  records: TrajectoryRecord[];
  warnings: TrajectoryWarning[];
}

export interface TokenUsageSummary {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheCreationTokens: number;
  totalTokens: number;
  totalCostUsd: number;
  tokensByModel: Record<string, number>;
  costByModel: Record<string, number>;
  unpricedModels: string[];
  dailyTokens: DailyTokenEntry[];
  sessionCount: number;
  messageCount: number;
  isFirstBuild: boolean;
}

export interface DailyTokenEntry {
  date: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalTokens: number;
  costUsd: number;
  tokensByModel: Record<string, number>;
  costByModel: Record<string, number>;
  unpricedModels: string[];
  messageCount: number;
  /** Per-model cache hit ratio for this day. */
  cacheHitRatioByModel: Record<string, number>;
}

/** Single assistant request as recorded in a JSONL file. */
export interface RequestRecord {
  timestamp: string;
  source: string;
  projectId: string;
  sessionId: string;
  filePath: string;
  model: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalTokens: number;
  costUsd: number;
  isPriced: boolean;
  /** Milliseconds between the preceding user message and this assistant message. */
  durationMs: number | null;
  messageUuid: string | null;
}

export interface RequestLogPage {
  records: RequestRecord[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
  totalCostUsd: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheCreationTokens: number;
  hasUnpricedUsage: boolean;
}

export interface ProjectCostEntry {
  source: string;
  projectId: string;
  displayName: string;
  requestCount: number;
  totalTokens: number;
  cacheReadTokens: number;
  costUsd: number;
  hasUnpricedUsage: boolean;
}

export interface SessionCostSummary {
  source: string;
  sessionId: string;
  filePath: string;
  requestCount: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalTokens: number;
  costUsd: number;
  avgCostUsd: number | null;
  requests: RequestRecord[];
}

export interface RequestLogFilter {
  projectId?: string | null;
  sessionId?: string | null;
  startDate?: string | null;
  endDate?: string | null;
  model?: string | null;
  page?: number;
  pageSize?: number;
  timeZone?: string;
}

export interface SearchResult {
  source: string;
  projectId: string;
  projectName: string;
  sessionId: string;
  firstPrompt: string | null;
  /** Codex only: Codex Desktop thread title. Preferred over firstPrompt. */
  threadName: string | null;
  alias: string | null;
  tags: string[] | null;
  matchedText: string;
  role: string;
  timestamp: string | null;
  filePath: string;
  totalMessageCount: number;
  matchedMessageId: string | null;
}

export interface Bookmark {
  id: string;
  source: string;
  projectId: string;
  sessionId: string;
  filePath: string;
  messageId: string | null;
  preview: string;
  sessionTitle: string;
  projectName: string;
  createdAt: string;
}

export type DeleteLevel = "sessionOnly" | "withCcConfig";

/** 会话导出格式。 */
export type ExportFormat = "json" | "markdown" | "html";

/** 冷启动扫描进度快照。 */
export interface ScanProgress {
  active: boolean;
  scanned: number;
  total: number;
  phase: string;
}

export interface DeleteResult {
  sessionsDeleted: number;
  configCleaned: boolean;
  bookmarksRemoved: number;
}

export interface RecycledItem {
  id: string;
  itemType: string;
  reason: string;
  source: string;
  projectId: string;
  sessionTitle: string | null;
  projectName: string | null;
  originalPath: string;
  storedName: string;
  companionOriginalPath?: string | null;
  companionStoredName?: string | null;
  movedAt: string;
}
