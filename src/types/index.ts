export interface ProjectEntry {
  source: string;
  id: string;
  displayPath: string;
  shortName: string;
  sessionCount: number;
  lastModified: string | null;
  modelProvider: string | null;
}

export interface SessionIndexEntry {
  source: string;
  sessionId: string;
  filePath: string;
  firstPrompt: string | null;
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
  role: string;
  timestamp: string | null;
  model: string | null;
  content: DisplayContentBlock[];
}

export interface PaginatedMessages {
  messages: DisplayMessage[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
}

export interface TokenUsageSummary {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  tokensByModel: Record<string, number>;
  dailyTokens: DailyTokenEntry[];
  sessionCount: number;
  messageCount: number;
}

export interface DailyTokenEntry {
  date: string;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
}

export interface SearchResult {
  source: string;
  projectId: string;
  projectName: string;
  sessionId: string;
  firstPrompt: string | null;
  matchedText: string;
  role: string;
  timestamp: string | null;
  filePath: string;
}
