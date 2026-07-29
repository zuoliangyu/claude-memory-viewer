export interface ModelInfo {
  id: string;
  name: string;
  provider: string;
  group: string;
  created: number | null;
}

export type ChatSource = "claude" | "codex";

export interface CliInstallation {
  path: string;
  version: string | null;
  cliType: string; // "claude" | "codex"
}

export interface StartChatParams {
  source: string;
  sessionId?: string;
  projectPath: string;
  prompt: string;
  model: string;
  skipPermissions: boolean;
  cliPath?: string;
  apiKey?: string;
  baseUrl?: string;
}

export interface ContinueChatParams {
  source: string;
  sessionId: string;
  projectPath: string;
  prompt: string;
  model: string;
  skipPermissions: boolean;
  cliPath?: string;
  apiKey?: string;
  baseUrl?: string;
}

// Unified chat message (parsed from stream)
export interface ChatMessage {
  id: string;
  role: "system" | "user" | "assistant" | "tool";
  content: ChatContentBlock[];
  model?: string;
  timestamp: string;
  usage?: TokenUsage;
}

export interface TokenUsage {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
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
  // Codex-specific fields (empty for Claude)
  authJsonPath: string;
  authJsonKeyMasked: string;
  authJsonHasKey: boolean;
  configTomlKeyMasked: string;
  configTomlHasKey: boolean;
  configTomlUrl: string;
  apiKeySource: string;
  baseUrlSource: string;
}
