export interface ModelInfo {
  id: string;
  name: string;
  provider: string;
  group: string;
  created: number | null;
}

export interface CliInstallation {
  path: string;
  version: string | null;
  cliType: string; // "claude" | "codex"
}

export interface StartChatParams {
  source: string;
  projectPath: string;
  prompt: string;
  model: string;
  skipPermissions: boolean;
}

export interface ContinueChatParams {
  source: string;
  sessionId: string;
  projectPath: string;
  prompt: string;
  model: string;
  skipPermissions: boolean;
}

// Unified chat message (parsed from stream)
export interface ChatMessage {
  id: string;
  role: "system" | "user" | "assistant" | "tool";
  content: ChatContentBlock[];
  model?: string;
  timestamp: string;
  usage?: { inputTokens: number; outputTokens: number };
}

export type ChatContentBlock =
  | { type: "text"; text: string }
  | { type: "thinking"; text: string }
  | { type: "tool_use"; id: string; name: string; input: string }
  | { type: "tool_result"; toolUseId: string; content: string; isError: boolean };

export interface CliConfig {
  source: string;
  apiKeyMasked: string;
  hasApiKey: boolean;
  baseUrl: string;
  defaultModel: string;
  configPath: string;
}

export interface QuickChatMessage {
  role: "user" | "assistant";
  content: string;
}
