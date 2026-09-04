import { create } from "zustand";
import type {
  CliInstallation,
  CliConfig,
  ModelInfo,
  ChatMessage,
  ChatContentBlock,
} from "../types/chat";
import { api } from "../services/api";

type ChatSource = "claude" | "codex" | "omp";

export const DEFAULT_CHAT_PANE_ID = "default";

interface ChatPaneModelListState {
  modelList: ModelInfo[];
  modelListLoading: boolean;
  modelListError: string | null;
}

export interface ChatPaneState {
  isActive: boolean;
  /**
   * The session id displayed to the user (URL, resume, etc). Updated mid-stream
   * when the CLI's system init / codex thread/start announces the real id.
   */
  sessionId: string | null;
  /**
   * Stable routing key for the active stream. Set when a turn starts and used
   * to filter incoming WS frames / Tauri events. Never updated mid-stream so
   * listeners don't have to re-subscribe and miss data.
   */
  streamId: string | null;
  projectPath: string;
  model: string;
  source: ChatSource;
  messages: ChatMessage[];
  rawOutput: string[];
  isStreaming: boolean;
  error: string | null;
}

type ChatPaneStateMap = Record<string, ChatPaneState>;
type ChatPaneModelListStateMap = Record<string, ChatPaneModelListState>;
type ChatPaneStateUpdater =
  | Partial<ChatPaneState>
  | ((pane: ChatPaneState) => ChatPaneState);

interface ChatState {
  activePaneId: string;
  panes: ChatPaneStateMap;
  paneModelLists: ChatPaneModelListStateMap;

  // Chat status
  isActive: boolean;
  sessionId: string | null;
  projectPath: string;
  model: string;
  source: ChatSource;

  // Messages
  messages: ChatMessage[];
  rawOutput: string[];
  isStreaming: boolean;
  error: string | null;

  // CLI info
  availableClis: CliInstallation[];

  // Model list
  modelList: ModelInfo[];
  modelListLoading: boolean;
  modelListError: string | null;

  // CLI config (auto-detected)
  cliConfig: CliConfig | null;
  cliConfigLoading: boolean;
  cliConfigError: string | null;
  codexCliConfig: CliConfig | null;
  codexCliConfigLoading: boolean;
  codexCliConfigError: string | null;

  // Manual API overrides (persisted to localStorage)
  claudeApiKeyOverride: string;
  claudeBaseUrlOverride: string;
  codexApiKeyOverride: string;
  codexBaseUrlOverride: string;

  // Settings (persisted to localStorage)
  skipPermissions: boolean;
  defaultModel: string;
  cliPath: string;

  // Actions
  detectCli: () => Promise<void>;
  fetchCliConfig: () => Promise<void>;
  fetchCodexCliConfig: () => Promise<void>;
  setClaudeApiKeyOverride: (v: string) => void;
  setClaudeBaseUrlOverride: (v: string) => void;
  setCodexApiKeyOverride: (v: string) => void;
  setCodexBaseUrlOverride: (v: string) => void;
  fetchModelList: (paneId?: string) => Promise<void>;
  getPaneState: (paneId?: string) => ChatPaneState;
  getPaneModelListState: (paneId?: string) => ChatPaneModelListState;
  setActivePane: (paneId: string) => void;
  setPaneState: (paneId: string, updater: ChatPaneStateUpdater) => void;
  clearPane: (paneId: string) => void;
  cancelPane: (paneId: string) => Promise<void>;
  startNewChatInPane: (
    paneId: string,
    projectPath: string,
    prompt: string,
    model: string
  ) => Promise<void>;
  continueExistingChatInPane: (
    paneId: string,
    sessionId: string,
    projectPath: string,
    prompt: string,
    model: string
  ) => Promise<void>;
  addStreamLineToPane: (paneId: string, line: string) => void;
  setPaneStreaming: (paneId: string, value: boolean) => void;
  setPaneSessionId: (paneId: string, id: string | null) => void;
  setPaneError: (paneId: string, error: string | null) => void;
  setPaneProjectPath: (paneId: string, path: string) => void;
  setPaneModel: (paneId: string, model: string) => void;
  setPaneSource: (paneId: string, source: ChatSource) => void;
  startNewChat: (
    projectPath: string,
    prompt: string,
    model: string
  ) => Promise<void>;
  continueExistingChat: (
    sessionId: string,
    projectPath: string,
    prompt: string,
    model: string
  ) => Promise<void>;
  cancelChat: () => Promise<void>;
  clearChat: () => void;
  setSkipPermissions: (v: boolean) => void;
  setDefaultModel: (m: string) => void;
  setCliPath: (p: string) => void;
  addCustomModel: (modelId: string, source?: ChatSource, paneId?: string) => void;
  removeCustomModel: (modelId: string, source?: ChatSource, paneId?: string) => void;
  addStreamLine: (line: string) => void;
  setStreaming: (v: boolean) => void;
  setSessionId: (id: string) => void;
  setError: (e: string | null) => void;
  setProjectPath: (p: string) => void;
  setModel: (m: string) => void;
  setSource: (s: ChatSource) => void;
}

function createChatPaneState(
  overrides: Partial<ChatPaneState> = {}
): ChatPaneState {
  return {
    isActive: false,
    sessionId: null,
    streamId: null,
    projectPath: "",
    model: localStorage.getItem("chat_lastUsedModel") || "",
    source: "claude",
    messages: [],
    rawOutput: [],
    isStreaming: false,
    error: null,
    ...overrides,
  };
}

function createChatPaneModelListState(
  overrides: Partial<ChatPaneModelListState> = {}
): ChatPaneModelListState {
  return {
    modelList: [],
    modelListLoading: false,
    modelListError: null,
    ...overrides,
  };
}

function toLegacyPaneFields(pane: ChatPaneState): Pick<
  ChatState,
  | "isActive"
  | "sessionId"
  | "projectPath"
  | "model"
  | "source"
  | "messages"
  | "rawOutput"
  | "isStreaming"
  | "error"
> {
  return {
    isActive: pane.isActive,
    sessionId: pane.sessionId,
    projectPath: pane.projectPath,
    model: pane.model,
    source: pane.source,
    messages: pane.messages,
    rawOutput: pane.rawOutput,
    isStreaming: pane.isStreaming,
    error: pane.error,
  };
}

function toLegacyModelListFields(modelListState: ChatPaneModelListState): Pick<
  ChatState,
  "modelList" | "modelListLoading" | "modelListError"
> {
  return {
    modelList: modelListState.modelList,
    modelListLoading: modelListState.modelListLoading,
    modelListError: modelListState.modelListError,
  };
}

function getOrCreatePaneState(
  panes: ChatPaneStateMap,
  paneId: string,
  fallback?: Partial<ChatPaneState>
): ChatPaneState {
  return panes[paneId] ?? createChatPaneState(fallback);
}

function getOrCreatePaneModelListState(
  paneModelLists: ChatPaneModelListStateMap,
  paneId: string
): ChatPaneModelListState {
  return paneModelLists[paneId] ?? createChatPaneModelListState();
}

function getPaneFallbackFromState(
  state: Pick<ChatState, "model" | "source" | "projectPath">
): Partial<ChatPaneState> {
  return {
    model: state.model,
    source: state.source,
    projectPath: state.projectPath,
  };
}

function applyPaneStateUpdate(
  state: ChatState,
  paneId: string,
  updater: ChatPaneStateUpdater
): Partial<ChatState> {
  const currentPane = getOrCreatePaneState(
    state.panes,
    paneId,
    getPaneFallbackFromState(state)
  );
  const nextPane =
    typeof updater === "function"
      ? updater(currentPane)
      : { ...currentPane, ...updater };
  const panes = {
    ...state.panes,
    [paneId]: nextPane,
  };

  if (paneId === state.activePaneId) {
    return {
      panes,
      ...toLegacyPaneFields(nextPane),
    };
  }

  return { panes };
}

function generateUUID(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  // Fallback for non-secure contexts (HTTP + non-localhost)
  // crypto.getRandomValues() is still available in non-secure contexts
  return "10000000-1000-4000-8000-100000000000".replace(/[018]/g, (c) =>
    (
      +c ^
      (crypto.getRandomValues(new Uint8Array(1))[0] & (15 >> (+c / 4)))
    ).toString(16)
  );
}

function waitForNextPaint(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(() => resolve());
      return;
    }
    setTimeout(resolve, 0);
  });
}

function getSourceOverrides(
  state: Pick<
    ChatState,
    | "claudeApiKeyOverride"
    | "claudeBaseUrlOverride"
    | "codexApiKeyOverride"
    | "codexBaseUrlOverride"
  >,
  source: ChatSource
): { apiKey?: string; baseUrl?: string } {
  const apiKey =
    source === "codex" ? state.codexApiKeyOverride : state.claudeApiKeyOverride;
  const baseUrl =
    source === "codex" ? state.codexBaseUrlOverride : state.claudeBaseUrlOverride;

  return {
    apiKey: apiKey || undefined,
    baseUrl: baseUrl || undefined,
  };
}

/** Result of parsing a single stream line */
type ParseResult =
  | { action: "add"; message: ChatMessage }
  | { action: "delta"; delta: string; blockIndex: number }
  | { action: "block_start"; blockType: string; blockIndex: number }
  | null;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function parseClaudeStreamLine(line: string): ParseResult {
  let data: any;
  try {
    data = JSON.parse(line);
  } catch {
    return null;
  }

  if (!data || typeof data !== "object" || !data.type) return null;

  const recordType: string = data.type;

  // System init — extract session_id only, no visible message
  if (recordType === "system" && data.subtype === "init") {
    return null;
  }

  // Stream events — incremental content updates
  if (recordType === "stream_event" && data.event) {
    const evt = data.event;
    if (evt.type === "content_block_start" && typeof evt.index === "number") {
      const blockType = evt.content_block?.type || "text";
      return { action: "block_start", blockType, blockIndex: evt.index };
    }
    if (evt.type === "content_block_delta" && typeof evt.index === "number") {
      const delta = evt.delta;
      if (delta?.type === "text_delta" && delta.text) {
        return { action: "delta", delta: delta.text, blockIndex: evt.index };
      }
      if (delta?.type === "thinking_delta" && delta.thinking) {
        return { action: "delta", delta: delta.thinking, blockIndex: evt.index };
      }
    }
    // message_start: create a placeholder assistant message
    if (evt.type === "message_start" && evt.message) {
      const msg = evt.message;
      return {
        action: "add",
        message: {
          id: generateUUID(),
          role: "assistant",
          content: [],
          model: msg.model,
          timestamp: new Date().toISOString(),
          usage: msg.usage
            ? {
                inputTokens: msg.usage.input_tokens ?? 0,
                outputTokens: msg.usage.output_tokens ?? 0,
                cacheCreationInputTokens: msg.usage.cache_creation_input_tokens ?? 0,
                cacheReadInputTokens: msg.usage.cache_read_input_tokens ?? 0,
              }
            : undefined,
        },
      };
    }
    return null;
  }

  // Complete assistant message (final, replaces streaming placeholder)
  if (recordType === "assistant" && data.message) {
    const msg = data.message;
    const content = parseContentValue(msg.content);
    if (content.length === 0) return null;

    const usage = msg.usage || data.usage;
    if (usage && (usage.output_tokens ?? 0) === 0) return null;

    return {
      action: "add",
      message: {
        id: generateUUID(),
        role: "assistant",
        content,
        model: msg.model || data.model,
        timestamp: data.timestamp || new Date().toISOString(),
        usage: usage
          ? {
              inputTokens: usage.input_tokens ?? 0,
              outputTokens: usage.output_tokens ?? 0,
              cacheCreationInputTokens: usage.cache_creation_input_tokens ?? 0,
              cacheReadInputTokens: usage.cache_read_input_tokens ?? 0,
            }
          : undefined,
      },
    };
  }

  // User message (including tool_result content blocks)
  if (recordType === "user" && data.message) {
    const msg = data.message;
    const content = parseContentValue(msg.content);
    if (content.length === 0) return null;

    return {
      action: "add",
      message: {
        id: generateUUID(),
        role: "user",
        content,
        timestamp: data.timestamp || new Date().toISOString(),
      },
    };
  }

  // Result message — final summary with token breakdown
  if (recordType === "result") {
    const text =
      data.result ||
      data.error ||
      (data.is_error ? "Error" : "Done");
    const durationInfo = data.duration_ms
      ? ` (${(data.duration_ms / 1000).toFixed(1)}s)`
      : "";

    const usage = data.usage;
    let tokenInfo = "";
    if (usage) {
      const parts: string[] = [];
      if (usage.input_tokens) parts.push(`输入: ${usage.input_tokens.toLocaleString()}`);
      if (usage.output_tokens) parts.push(`输出: ${usage.output_tokens.toLocaleString()}`);
      if (usage.cache_creation_input_tokens) parts.push(`写入缓存: ${usage.cache_creation_input_tokens.toLocaleString()}`);
      if (usage.cache_read_input_tokens) parts.push(`读取缓存: ${usage.cache_read_input_tokens.toLocaleString()}`);
      if (parts.length > 0) tokenInfo = ` [${parts.join(" · ")}]`;
    }

    return {
      action: "add",
      message: {
        id: generateUUID(),
        role: "system",
        content: [{ type: "text", text: `${text}${durationInfo}${tokenInfo}` }],
        timestamp: new Date().toISOString(),
      },
    };
  }

  return null;
}

/**
 * Parse Claude's `content` field which can be:
 * - A plain string (user prompt text)
 * - An array of content blocks (text, thinking, tool_use, tool_result, etc.)
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function parseContentValue(content: any): ChatContentBlock[] {
  if (!content) return [];

  // Plain string content
  if (typeof content === "string") {
    return content.trim() ? [{ type: "text", text: content }] : [];
  }

  // Array of content blocks
  if (!Array.isArray(content)) return [];

  const results: ChatContentBlock[] = [];
  for (const block of content) {
    if (!block || typeof block !== "object") continue;
    const blockType: string = block.type;

    if (blockType === "text" && block.text) {
      results.push({ type: "text", text: block.text });
    } else if (blockType === "thinking" && block.thinking) {
      results.push({ type: "thinking", text: block.thinking });
    } else if (blockType === "tool_use" && block.name) {
      results.push({
        type: "tool_use",
        id: block.id || "",
        name: block.name,
        input:
          typeof block.input === "string"
            ? block.input
            : JSON.stringify(block.input, null, 2),
      });
    } else if (blockType === "tool_result") {
      let resultContent: string;
      if (typeof block.content === "string") {
        resultContent = block.content;
      } else if (Array.isArray(block.content)) {
        // tool_result content can be an array of {type:"text", text:"..."} blocks
        resultContent = block.content
          .map((c: any) => (typeof c === "string" ? c : c?.text || JSON.stringify(c)))
          .join("\n");
      } else if (block.content) {
        resultContent = JSON.stringify(block.content, null, 2);
      } else {
        resultContent = "";
      }
      results.push({
        type: "tool_result",
        toolUseId: block.tool_use_id || "",
        content: resultContent,
        isError: block.is_error || false,
      });
    }
  }
  return results;
}

/**
 * Parse one line emitted by the Rust backend on the codex chat stream. The
 * line is either an envelope tagged by `type` (session_id / history) or a
 * `notification` envelope wrapping a raw `codex app-server` JSON-RPC method.
 *
 * Supported app-server notification methods (translated):
 *   thread/started, thread/tokenUsage/updated,
 *   item/agentMessage/delta, item/completed, item/started,
 *   turn/started, turn/completed, turn/failed, error.
 */
interface CodexUsage {
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
}

type CodexParseResult =
  | { action: "session_id"; id: string }
  | { action: "replace_messages"; messages: ChatMessage[] }
  | { action: "add"; message: ChatMessage }
  | { action: "delta"; itemId: string; delta: string }
  | { action: "token_usage"; usage: CodexUsage }
  | { action: "done" }
  | { action: "error"; message: string }
  | null;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function buildCodexMessageFromItem(item: any): ChatMessage | null {
  if (!item || typeof item !== "object") return null;
  const itemType: string = item.type || "";
  const id: string = typeof item.id === "string" && item.id ? item.id : generateUUID();
  const ts = new Date().toISOString();

  if (itemType === "userMessage") {
    // app-server format: content: [{type:"text", text}|{type:"image",url}|...]
    const parts: ChatContentBlock[] = [];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const content = Array.isArray(item.content) ? item.content : [];
    for (const c of content) {
      if (c?.type === "text" && typeof c.text === "string" && c.text) {
        parts.push({ type: "text", text: c.text });
      }
    }
    if (parts.length === 0) return null;
    return { id, role: "user", content: parts, timestamp: ts };
  }

  if (itemType === "agentMessage") {
    const text = typeof item.text === "string" ? item.text : "";
    if (!text) return null;
    return { id, role: "assistant", content: [{ type: "text", text }], timestamp: ts };
  }

  if (itemType === "reasoning") {
    // summary: string[] (concise) — prefer; fall back to content (full)
    const summaryArr: string[] = Array.isArray(item.summary) ? item.summary : [];
    const contentArr: string[] = Array.isArray(item.content) ? item.content : [];
    const text = (summaryArr.length > 0 ? summaryArr : contentArr).join("\n").trim();
    if (!text) return null;
    return { id, role: "assistant", content: [{ type: "thinking", text }], timestamp: ts };
  }

  if (itemType === "commandExecution") {
    const cmd: string = typeof item.command === "string" ? item.command : "";
    const output: string = typeof item.aggregatedOutput === "string" ? item.aggregatedOutput : "";
    const exitCode: number | null = typeof item.exitCode === "number" ? item.exitCode : null;
    const isError = exitCode !== null && exitCode !== 0;
    return {
      id,
      role: "assistant",
      content: [
        { type: "tool_use", id, name: "shell", input: cmd },
        ...(output ? [{ type: "tool_result" as const, toolUseId: id, content: output, isError }] : []),
      ],
      timestamp: ts,
    };
  }

  if (itemType === "fileChange") {
    const changes = Array.isArray(item.changes) ? item.changes : [];
    return {
      id,
      role: "assistant",
      content: [
        { type: "tool_use", id, name: "fileChange", input: JSON.stringify(changes, null, 2) },
      ],
      timestamp: ts,
    };
  }

  if (itemType === "webSearch") {
    const query = typeof item.query === "string" ? item.query : "";
    return {
      id,
      role: "assistant",
      content: [{ type: "tool_use", id, name: "webSearch", input: query }],
      timestamp: ts,
    };
  }

  if (itemType === "mcpToolCall") {
    const server = typeof item.server === "string" ? item.server : "";
    const tool = typeof item.tool === "string" ? item.tool : "";
    const args = item.arguments;
    const inputStr =
      typeof args === "string" ? args : JSON.stringify(args ?? null, null, 2);
    const resultText: string =
      item.result && typeof item.result === "object"
        ? JSON.stringify(item.result, null, 2)
        : "";
    const errText: string =
      item.error && typeof item.error === "object"
        ? JSON.stringify(item.error, null, 2)
        : "";
    return {
      id,
      role: "assistant",
      content: [
        { type: "tool_use", id, name: `mcp:${server}/${tool}`, input: inputStr },
        ...(errText
          ? [{ type: "tool_result" as const, toolUseId: id, content: errText, isError: true }]
          : resultText
          ? [{ type: "tool_result" as const, toolUseId: id, content: resultText, isError: false }]
          : []),
      ],
      timestamp: ts,
    };
  }

  // Unknown item type — skip
  return null;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function turnsToMessages(turns: any[]): ChatMessage[] {
  const out: ChatMessage[] = [];
  for (const turn of turns) {
    const items = Array.isArray(turn?.items) ? turn.items : [];
    for (const item of items) {
      const msg = buildCodexMessageFromItem(item);
      if (msg) out.push(msg);
    }
  }
  return out;
}

function parseCodexStreamLine(line: string): CodexParseResult {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let data: any;
  try { data = JSON.parse(line); } catch { return null; }
  if (!data || typeof data !== "object") return null;

  const envType: string = typeof data.type === "string" ? data.type : "";

  if (envType === "session_id" && typeof data.data === "string") {
    return { action: "session_id", id: data.data };
  }

  if (envType === "history" && Array.isArray(data.data)) {
    return { action: "replace_messages", messages: turnsToMessages(data.data) };
  }

  if (envType !== "notification" || typeof data.method !== "string") {
    return null;
  }

  const method: string = data.method;
  const params = data.params ?? {};

  if (method === "thread/started") {
    const id = params?.thread?.id;
    if (typeof id === "string") return { action: "session_id", id };
    return null;
  }

  if (method === "item/agentMessage/delta") {
    const itemId = typeof params?.itemId === "string" ? params.itemId : "";
    const delta = typeof params?.delta === "string" ? params.delta : "";
    if (!itemId || !delta) return null;
    return { action: "delta", itemId, delta };
  }

  if (method === "item/completed") {
    const message = buildCodexMessageFromItem(params?.item);
    if (message) return { action: "add", message };
    return null;
  }

  if (method === "thread/tokenUsage/updated") {
    const last = params?.tokenUsage?.last;
    if (last && typeof last === "object") {
      return {
        action: "token_usage",
        usage: {
          inputTokens: typeof last.inputTokens === "number" ? last.inputTokens : 0,
          outputTokens: typeof last.outputTokens === "number" ? last.outputTokens : 0,
          cachedInputTokens:
            typeof last.cachedInputTokens === "number" ? last.cachedInputTokens : 0,
        },
      };
    }
    return null;
  }

  if (method === "turn/completed") {
    return { action: "done" };
  }

  if (method === "turn/failed") {
    const err =
      params?.turn?.error?.message ||
      params?.error?.message ||
      "Codex turn failed";
    return { action: "error", message: typeof err === "string" ? err : "Codex turn failed" };
  }

  if (method === "error") {
    const msg = typeof params?.message === "string" ? params.message : "Codex error";
    return { action: "error", message: msg };
  }

  // Quietly ignore other notifications (mcpServer status, thread/tokenUsage,
  // hook/*, turn/diff, turn/plan, account/*, fs/*, model/rerouted, etc.).
  return null;
}
type OmpParseResult =
  | { action: "session_id"; id: string }
  | { action: "add"; message: ChatMessage }
  | { action: "delta"; delta: string }
  | { action: "done" }
  | { action: "error"; message: string }
  | null;

type OmpRecord = Record<string, unknown>;

function ompRecord(value: unknown): OmpRecord | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as OmpRecord)
    : null;
}

function ompString(record: OmpRecord, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" ? value : undefined;
}

function parseOmpMessage(data: OmpRecord): ChatMessage | null {
  const message = ompRecord(data.message);
  if (!message) return null;
  const role = ompString(message, "role") === "user" ? "user" : ompString(message, "role") === "toolResult" ? "tool" : "assistant";
  const rawContent = message.content;
  const content: unknown[] = Array.isArray(rawContent) ? rawContent : [{ type: "text", text: rawContent }];
  const blocks: ChatContentBlock[] = [];
  for (const rawBlock of content) {
    if (typeof rawBlock === "string") {
      if (rawBlock) blocks.push({ type: "text", text: rawBlock });
      continue;
    }
    const block = ompRecord(rawBlock);
    if (!block) continue;
    const blockType = ompString(block, "type");
    if (blockType === "text" && ompString(block, "text")) {
      blocks.push({ type: "text", text: ompString(block, "text") || "" });
    } else if (blockType === "thinking" && ompString(block, "thinking")) {
      blocks.push({ type: "thinking", text: ompString(block, "thinking") || "" });
    } else if (blockType === "toolCall" && ompString(block, "name")) {
      const args = block.arguments;
      blocks.push({
        type: "tool_use",
        id: ompString(block, "id") || generateUUID(),
        name: ompString(block, "name") || "tool",
        input: typeof args === "string" ? args : JSON.stringify(args ?? {}, null, 2),
      });
    } else if (blockType === "toolResult") {
      const result = block.content;
      blocks.push({
        type: "tool_result",
        toolUseId: ompString(block, "toolCallId") || "",
        content: typeof result === "string" ? result : JSON.stringify(result ?? "", null, 2),
        isError: block.isError === true,
      });
    }
  }
  if (blocks.length === 0) return null;
  return {
    id: ompString(data, "id") || ompString(message, "id") || generateUUID(),
    role,
    content: blocks,
    model: ompString(message, "model"),
    timestamp: ompString(message, "timestamp") || new Date().toISOString(),
  };
}

function parseOmpStreamLine(line: string): OmpParseResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(line);
  } catch {
    return null;
  }
  const data = ompRecord(parsed);
  if (!data) return null;
  if (ompString(data, "type") === "session" && ompString(data, "id")) {
    return { action: "session_id", id: ompString(data, "id")! };
  }
  const type = ompString(data, "type");
  const message = ompRecord(data.message);
  if ((type === "message_start" || type === "message_end") && message) {
    const converted = parseOmpMessage(data);
    if (converted) return { action: "add", message: converted };
    if (type === "message_start" && ompString(message, "role") === "assistant") {
      return {
        action: "add",
        message: {
          id: ompString(data, "id") || generateUUID(),
          role: "assistant",
          content: [],
          model: ompString(message, "model"),
          timestamp: new Date().toISOString(),
        },
      };
    }
    return null;
  }
  if (type === "message_update") {
    const event = ompRecord(data.assistantMessageEvent);
    const eventType = event ? ompString(event, "type") : undefined;
    const delta = event ? ompString(event, "delta") || ompString(event, "text") : undefined;
    if ((eventType === "text_delta" || eventType === "thinking_delta") && delta) {
      return { action: "delta", delta };
    }
    return null;
  }
  if (type === "agent_end") return { action: "done" };
  if (type === "notice" && ompString(data, "level") === "error") {
    return { action: "error", message: ompString(data, "message") || "OMP error" };
  }
  return null;
}


export const useChatStore = create<ChatState>((set, get) => {
  const initialDefaultPane = createChatPaneState();
  const initialDefaultPaneModelList = createChatPaneModelListState();

  return {
    activePaneId: DEFAULT_CHAT_PANE_ID,
    panes: {
      [DEFAULT_CHAT_PANE_ID]: initialDefaultPane,
    },
    paneModelLists: {
      [DEFAULT_CHAT_PANE_ID]: initialDefaultPaneModelList,
    },
    ...toLegacyPaneFields(initialDefaultPane),
    ...toLegacyModelListFields(initialDefaultPaneModelList),

    availableClis: [],

    cliConfig: null,
    cliConfigLoading: false,
    cliConfigError: null,
    codexCliConfig: null,
    codexCliConfigLoading: false,
    codexCliConfigError: null,

    claudeApiKeyOverride: localStorage.getItem("chat_claudeApiKey") || "",
    claudeBaseUrlOverride: localStorage.getItem("chat_claudeBaseUrl") || "",
    codexApiKeyOverride: localStorage.getItem("chat_codexApiKey") || "",
    codexBaseUrlOverride: localStorage.getItem("chat_codexBaseUrl") || "",

    skipPermissions: localStorage.getItem("chat_skipPermissions") === "true",
    defaultModel: localStorage.getItem("chat_defaultModel") || "",
    cliPath: localStorage.getItem("chat_cliPath") || "",

  detectCli: async () => {
    try {
      const clis = await api.detectCli();
      set({ availableClis: clis });
    } catch (e) {
      console.error("Failed to detect CLI:", e);
    }
  },

  fetchCliConfig: async () => {
    set({ cliConfigLoading: true, cliConfigError: null });
    try {
      const config = await api.getCliConfig("claude");
      set({ cliConfig: config, cliConfigLoading: false });
    } catch (e) {
      set({
        cliConfigLoading: false,
        cliConfigError: e instanceof Error ? e.message : String(e),
      });
    }
  },

  fetchCodexCliConfig: async () => {
    set({ codexCliConfigLoading: true, codexCliConfigError: null });
    try {
      const config = await api.getCliConfig("codex");
      set({ codexCliConfig: config, codexCliConfigLoading: false });
    } catch (e) {
      set({
        codexCliConfigLoading: false,
        codexCliConfigError: e instanceof Error ? e.message : String(e),
      });
    }
  },

  getPaneState: (paneId = DEFAULT_CHAT_PANE_ID) => {
    const state = get();
    return getOrCreatePaneState(
      state.panes,
      paneId,
      getPaneFallbackFromState(state)
    );
  },

  getPaneModelListState: (paneId = DEFAULT_CHAT_PANE_ID) => {
    const state = get();
    return getOrCreatePaneModelListState(state.paneModelLists, paneId);
  },

  setActivePane: (paneId) => {
    set((state) => {
      const nextPane = getOrCreatePaneState(
        state.panes,
        paneId,
        getPaneFallbackFromState(state)
      );
      const nextPaneModelList = getOrCreatePaneModelListState(
        state.paneModelLists,
        paneId
      );
      return {
        activePaneId: paneId,
        panes: {
          ...state.panes,
          [paneId]: nextPane,
        },
        paneModelLists: {
          ...state.paneModelLists,
          [paneId]: nextPaneModelList,
        },
        ...toLegacyPaneFields(nextPane),
        ...toLegacyModelListFields(nextPaneModelList),
      };
    });
  },

  setPaneState: (paneId, updater) => {
    set((state) => applyPaneStateUpdate(state, paneId, updater));
  },

  clearPane: (paneId) => {
    set((state) => {
      const currentPane = getOrCreatePaneState(
        state.panes,
        paneId,
        getPaneFallbackFromState(state)
      );
      return applyPaneStateUpdate(state, paneId, () =>
        createChatPaneState({
          projectPath: currentPane.projectPath,
          model: currentPane.model || state.defaultModel || state.model,
          source: currentPane.source,
        })
      );
    });
  },

  cancelPane: async (paneId) => {
    const pane = get().getPaneState(paneId);
    // Cancel by streamId — the routing key the backend uses for this turn.
    // Falls back to sessionId only if streamId hasn't been set yet (e.g. very
    // early failure before the start handshake).
    const target = pane.streamId ?? pane.sessionId;
    if (target) {
      try {
        await api.cancelChat(target);
      } catch (e) {
        console.error("Failed to cancel chat:", e);
      }
    }
    get().setPaneStreaming(paneId, false);
  },

  fetchModelList: async (paneId) => {
    const state = get();
    const targetPaneId = paneId ?? state.activePaneId;
    const pane = state.getPaneState(targetPaneId);
    const {
      claudeApiKeyOverride,
      claudeBaseUrlOverride,
      codexApiKeyOverride,
      codexBaseUrlOverride,
    } = state;
    const { source } = pane;
    const apiKey = source === "codex" ? codexApiKeyOverride : claudeApiKeyOverride;
    const baseUrl = source === "codex" ? codexBaseUrlOverride : claudeBaseUrlOverride;
    set((currentState) => {
      const currentPaneModelList = getOrCreatePaneModelListState(
        currentState.paneModelLists,
        targetPaneId
      );
      const nextPaneModelList = {
        ...currentPaneModelList,
        modelListLoading: true,
        modelListError: null,
      };
      return {
        paneModelLists: {
          ...currentState.paneModelLists,
          [targetPaneId]: nextPaneModelList,
        },
        ...(targetPaneId === currentState.activePaneId
          ? toLegacyModelListFields(nextPaneModelList)
          : {}),
      };
    });
    try {
      let activeCliConfig = source === "codex" ? get().codexCliConfig : get().cliConfig;
      if (!activeCliConfig) {
        try {
          const config = await api.getCliConfig(source);
          activeCliConfig = config;
          if (source === "codex") {
            set({ codexCliConfig: config, codexCliConfigError: null });
          } else {
            set({ cliConfig: config, cliConfigError: null });
          }
        } catch (configError) {
          const message = configError instanceof Error ? configError.message : String(configError);
          if (source === "codex") {
            set({ codexCliConfigError: message });
          } else {
            set({ cliConfigError: message });
          }
        }
      }

      const models = await api.listModels(source, apiKey, baseUrl);
      const customKey = `chat_customModels_${source}`;
      const customIds: string[] = JSON.parse(localStorage.getItem(customKey) || "[]");
      const existingIds = new Set(models.map((m) => m.id));
      const provider = source === "codex" ? "openai" : source === "omp" ? "omp" : "anthropic";
      const customModels: ModelInfo[] = customIds
        .filter((id) => !existingIds.has(id))
        .map((id) => ({ id, name: id, provider, group: "自定义", created: null }));
      const allModels = [...customModels, ...models];
      set((currentState) => {
        const nextPaneModelList = createChatPaneModelListState({
          modelList: allModels,
        });
        return {
          paneModelLists: {
            ...currentState.paneModelLists,
            [targetPaneId]: nextPaneModelList,
          },
          ...(targetPaneId === currentState.activePaneId
            ? toLegacyModelListFields(nextPaneModelList)
            : {}),
        };
      });

      // Auto-select first model if current model is empty or not in this source's list
      const latestState = get();
      const latestPane = latestState.getPaneState(targetPaneId);
      const modelIds = new Set(allModels.map((m) => m.id));
      if (!latestPane.model || !modelIds.has(latestPane.model)) {
        const resolve = (name: string): string => {
          if (!name) return "";
          if (modelIds.has(name)) return name;
          const lower = name.toLowerCase();
          return allModels.find((m) => m.id.toLowerCase().includes(lower))?.id || "";
        };
        const candidates = [
          resolve(latestPane.model),
          resolve(latestState.defaultModel),
          resolve(activeCliConfig?.defaultModel || ""),
        ].filter(Boolean);
        const selected = candidates[0] || (allModels.length > 0 ? allModels[0].id : "");
        if (selected && selected !== latestPane.model) {
          get().setPaneModel(targetPaneId, selected);
        }
      }
    } catch (e) {
      set((currentState) => {
        const currentPaneModelList = getOrCreatePaneModelListState(
          currentState.paneModelLists,
          targetPaneId
        );
        const nextPaneModelList = {
          ...currentPaneModelList,
          modelListLoading: false,
          modelListError: e instanceof Error ? e.message : String(e),
        };
        return {
          paneModelLists: {
            ...currentState.paneModelLists,
            [targetPaneId]: nextPaneModelList,
          },
          ...(targetPaneId === currentState.activePaneId
            ? toLegacyModelListFields(nextPaneModelList)
            : {}),
        };
      });
    }
  },

  startNewChatInPane: async (paneId, projectPath, prompt, model) => {
    const state = get();
    const pane = state.getPaneState(paneId);
    const overrides = getSourceOverrides(state, pane.source);
    const pendingSessionId = generateUUID();
    localStorage.setItem("chat_lastUsedModel", model);
    // streamId is the stable routing key for this turn — listeners subscribe
    // by streamId, not sessionId, so the mid-stream sessionId swap (Claude
    // system init / Codex thread/start) never reroutes the live stream.
    get().setPaneState(paneId, {
      isActive: true,
      sessionId: pendingSessionId,
      streamId: pendingSessionId,
      isStreaming: true,
      projectPath,
      model,
      error: null,
      messages: [
        {
          id: generateUUID(),
          role: "user",
          content: [{ type: "text", text: prompt }],
          timestamp: new Date().toISOString(),
        },
      ],
      rawOutput: [],
    });

    try {
      await waitForNextPaint();
      const sessionId = await api.startChat({
        source: pane.source,
        sessionId: pendingSessionId,
        projectPath,
        prompt,
        model,
        skipPermissions: state.skipPermissions,
        cliPath: state.cliPath || undefined,
        ...overrides,
      });
      if (sessionId !== pendingSessionId) {
        get().setPaneSessionId(paneId, sessionId);
      }
    } catch (e) {
      get().setPaneState(paneId, {
        sessionId: null,
        streamId: null,
        isStreaming: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  continueExistingChatInPane: async (
    paneId,
    sessionId,
    projectPath,
    prompt,
    model
  ) => {
    const state = get();
    const pane = state.getPaneState(paneId);
    const overrides = getSourceOverrides(state, pane.source);
    localStorage.setItem("chat_lastUsedModel", model);
    get().setPaneState(paneId, {
      isActive: true,
      isStreaming: true,
      sessionId,
      streamId: sessionId,
      projectPath,
      model,
      error: null,
      messages: [
        ...pane.messages,
        {
          id: generateUUID(),
          role: "user",
          content: [{ type: "text", text: prompt }],
          timestamp: new Date().toISOString(),
        },
      ],
      rawOutput: [],
    });

    try {
      await api.continueChat({
        source: pane.source,
        sessionId,
        projectPath,
        prompt,
        model,
        skipPermissions: state.skipPermissions,
        cliPath: state.cliPath || undefined,
        ...overrides,
      });
    } catch (e) {
      get().setPaneState(paneId, {
        isStreaming: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  startNewChat: async (projectPath, prompt, model) => {
    await get().startNewChatInPane(
      DEFAULT_CHAT_PANE_ID,
      projectPath,
      prompt,
      model
    );
  },

  continueExistingChat: async (sessionId, projectPath, prompt, model) => {
    await get().continueExistingChatInPane(
      DEFAULT_CHAT_PANE_ID,
      sessionId,
      projectPath,
      prompt,
      model
    );
  },

  cancelChat: async () => {
    await get().cancelPane(DEFAULT_CHAT_PANE_ID);
  },

  clearChat: () => {
    get().clearPane(DEFAULT_CHAT_PANE_ID);
  },

  setSkipPermissions: (v) => {
    localStorage.setItem("chat_skipPermissions", String(v));
    set({ skipPermissions: v });
  },

  setDefaultModel: (m) => {
    localStorage.setItem("chat_defaultModel", m);
    set({ defaultModel: m });
  },

  setCliPath: (p) => {
    localStorage.setItem("chat_cliPath", p);
    set({ cliPath: p });
  },

  addCustomModel: (modelId, sourceOverride, paneId) => {
    const state = get();
    const src = sourceOverride ?? state.source;
    const customKey = `chat_customModels_${src}`;
    const existing: string[] = JSON.parse(localStorage.getItem(customKey) || "[]");
    if (!existing.includes(modelId)) {
      localStorage.setItem(customKey, JSON.stringify([...existing, modelId]));
    }
    const customModel = {
      id: modelId,
      name: modelId,
      provider: src === "codex" ? "openai" : "anthropic",
      group: "自定义",
      created: null,
    } satisfies ModelInfo;
    set((currentState) => {
      const targetPaneIds = paneId
        ? [paneId]
        : Array.from(new Set([
            ...Object.keys(currentState.panes),
            ...Object.keys(currentState.paneModelLists),
          ]));
      const nextPaneModelLists = { ...currentState.paneModelLists };
      let activePaneModelList: ChatPaneModelListState | null = null;

      for (const targetPaneId of targetPaneIds) {
        const targetPane = getOrCreatePaneState(
          currentState.panes,
          targetPaneId,
          getPaneFallbackFromState(currentState)
        );
        if (targetPane.source !== src) continue;

        const currentPaneModelList = getOrCreatePaneModelListState(
          nextPaneModelLists,
          targetPaneId
        );
        if (currentPaneModelList.modelList.some((m) => m.id === modelId)) continue;

        const nextPaneModelList = {
          ...currentPaneModelList,
          modelList: [customModel, ...currentPaneModelList.modelList],
        };
        nextPaneModelLists[targetPaneId] = nextPaneModelList;
        if (targetPaneId === currentState.activePaneId) {
          activePaneModelList = nextPaneModelList;
        }
      }

      return {
        paneModelLists: nextPaneModelLists,
        ...(activePaneModelList
          ? toLegacyModelListFields(activePaneModelList)
          : {}),
      };
    });
  },

  removeCustomModel: (modelId, sourceOverride, paneId) => {
    const state = get();
    const src = sourceOverride ?? state.source;
    const customKey = `chat_customModels_${src}`;
    const existing: string[] = JSON.parse(localStorage.getItem(customKey) || "[]");
    localStorage.setItem(customKey, JSON.stringify(existing.filter((id) => id !== modelId)));
    set((currentState) => {
      const targetPaneIds = paneId
        ? [paneId]
        : Array.from(new Set([
            ...Object.keys(currentState.panes),
            ...Object.keys(currentState.paneModelLists),
          ]));
      const nextPaneModelLists = { ...currentState.paneModelLists };
      let activePaneModelList: ChatPaneModelListState | null = null;

      for (const targetPaneId of targetPaneIds) {
        const targetPane = getOrCreatePaneState(
          currentState.panes,
          targetPaneId,
          getPaneFallbackFromState(currentState)
        );
        if (targetPane.source !== src) continue;

        const currentPaneModelList = getOrCreatePaneModelListState(
          nextPaneModelLists,
          targetPaneId
        );
        const nextPaneModelList = {
          ...currentPaneModelList,
          modelList: currentPaneModelList.modelList.filter((m) => m.id !== modelId),
        };
        nextPaneModelLists[targetPaneId] = nextPaneModelList;
        if (targetPaneId === currentState.activePaneId) {
          activePaneModelList = nextPaneModelList;
        }
      }

      return {
        paneModelLists: nextPaneModelLists,
        ...(activePaneModelList
          ? toLegacyModelListFields(activePaneModelList)
          : {}),
      };
    });
  },

  addStreamLineToPane: (paneId, line) => {
    const pane = get().getPaneState(paneId);

    if (pane.source === "omp") {
      const parsed = parseOmpStreamLine(line);
      get().setPaneState(paneId, (currentPane) => ({
        ...currentPane,
        rawOutput: [...currentPane.rawOutput, line],
      }));
      if (!parsed) return;
      if (parsed.action === "session_id") {
        get().setPaneSessionId(paneId, parsed.id);
      } else if (parsed.action === "delta") {
        get().setPaneState(paneId, (currentPane) => {
          const messages = [...currentPane.messages];
          const last = messages[messages.length - 1];
          if (last?.role === "assistant") {
            const textBlock = last.content.find((block) => block.type === "text");
            if (textBlock?.type === "text") {
              const index = last.content.indexOf(textBlock);
              const content = [...last.content];
              content[index] = { ...textBlock, text: textBlock.text + parsed.delta };
              messages[messages.length - 1] = { ...last, content };
            } else {
              messages[messages.length - 1] = {
                ...last,
                content: [...last.content, { type: "text", text: parsed.delta }],
              };
            }
          } else {
            messages.push({
              id: generateUUID(),
              role: "assistant",
              content: [{ type: "text", text: parsed.delta }],
              timestamp: new Date().toISOString(),
            });
          }
          return { ...currentPane, messages };
        });
      } else if (parsed.action === "add") {
        get().setPaneState(paneId, (currentPane) => {
          const messages = [...currentPane.messages];
          const index = messages.findIndex((message) => message.id === parsed.message.id);
          if (index >= 0) {
            messages[index] = parsed.message;
          } else if (
            (parsed.message.role === "assistant" || parsed.message.role === "user") &&
            messages[messages.length - 1]?.role === parsed.message.role
          ) {
            messages[messages.length - 1] = parsed.message;
          } else {
            messages.push(parsed.message);
          }
          return { ...currentPane, messages };
        });
      } else if (parsed.action === "error") {
        get().setPaneState(paneId, { isStreaming: false, error: parsed.message });
      } else if (parsed.action === "done") {
        get().setPaneStreaming(paneId, false);
      }
      return;
    }

    if (pane.source === "codex") {
      const parsed = parseCodexStreamLine(line);
      if (!parsed) {
        get().setPaneState(paneId, (currentPane) => ({
          ...currentPane,
          rawOutput: [...currentPane.rawOutput, line],
        }));
        return;
      }
      if (parsed.action === "session_id") {
        get().setPaneSessionId(paneId, parsed.id);
      } else if (parsed.action === "replace_messages") {
        // Resume: prepend history before the user's pending prompt (which
        // was optimistically pushed by continueExistingChatInPane and is
        // always the last message in the pane at this point).
        get().setPaneState(paneId, (currentPane) => {
          const last = currentPane.messages[currentPane.messages.length - 1];
          const pending =
            last && last.role === "user" && last.content.some((c) => c.type === "text")
              ? last
              : null;
          return {
            ...currentPane,
            rawOutput: [...currentPane.rawOutput, line],
            messages: pending ? [...parsed.messages, pending] : parsed.messages,
          };
        });
      } else if (parsed.action === "delta") {
        get().setPaneState(paneId, (currentPane) => {
          const messages = [...currentPane.messages];
          const lastIdx = messages.length - 1;
          const last = lastIdx >= 0 ? messages[lastIdx] : null;
          // Only fold deltas into a streaming assistant message we own.
          if (last && last.role === "assistant" && last.id === parsed.itemId) {
            const blocks = [...last.content];
            const tIdx = blocks.findIndex((b) => b.type === "text");
            if (tIdx >= 0) {
              const block = blocks[tIdx];
              if (block.type === "text") {
                blocks[tIdx] = { ...block, text: block.text + parsed.delta };
              }
            } else {
              blocks.push({ type: "text", text: parsed.delta });
            }
            messages[lastIdx] = { ...last, content: blocks };
          } else {
            // Start a new streaming assistant message keyed on the itemId.
            messages.push({
              id: parsed.itemId,
              role: "assistant",
              content: [{ type: "text", text: parsed.delta }],
              timestamp: new Date().toISOString(),
            });
          }
          return {
            ...currentPane,
            rawOutput: [...currentPane.rawOutput, line],
            messages,
          };
        });
      } else if (parsed.action === "add") {
        get().setPaneState(paneId, (currentPane) => {
          const incoming = parsed.message;
          // If we've been streaming this message via deltas, replace the
          // placeholder with the finalized version (matched by id).
          const messages = [...currentPane.messages];
          const idx = messages.findIndex((m) => m.id === incoming.id);
          if (idx >= 0) {
            messages[idx] = incoming;
          } else {
            messages.push(incoming);
          }
          return {
            ...currentPane,
            rawOutput: [...currentPane.rawOutput, line],
            messages,
          };
        });
      } else if (parsed.action === "error") {
        get().setPaneState(paneId, (currentPane) => ({
          ...currentPane,
          isStreaming: false,
          rawOutput: [...currentPane.rawOutput, line],
          error: parsed.message,
        }));
      } else if (parsed.action === "token_usage") {
        // Stash for the upcoming "done" event. Encoded into rawOutput so we
        // don't have to grow ChatPaneState.
        const usageMarker = `__codex_usage__:${JSON.stringify(parsed.usage)}`;
        get().setPaneState(paneId, (currentPane) => ({
          ...currentPane,
          rawOutput: [...currentPane.rawOutput, usageMarker],
        }));
      } else if (parsed.action === "done") {
        // Pull the last usage marker stashed via token_usage above.
        const stash = (() => {
          for (let i = pane.rawOutput.length - 1; i >= 0; i -= 1) {
            const r = pane.rawOutput[i];
            if (r.startsWith("__codex_usage__:")) {
              try {
                return JSON.parse(r.slice("__codex_usage__:".length)) as CodexUsage;
              } catch {
                return null;
              }
            }
          }
          return null;
        })();
        if (stash) {
          const parts: string[] = [];
          if (stash.inputTokens) parts.push(`输入: ${stash.inputTokens.toLocaleString()}`);
          if (stash.outputTokens) parts.push(`输出: ${stash.outputTokens.toLocaleString()}`);
          if (stash.cachedInputTokens) parts.push(`缓存命中: ${stash.cachedInputTokens.toLocaleString()}`);
          if (parts.length > 0) {
            get().setPaneState(paneId, (currentPane) => ({
              ...currentPane,
              isStreaming: false,
              rawOutput: [...currentPane.rawOutput, line],
              messages: [
                ...currentPane.messages,
                {
                  id: generateUUID(),
                  role: "system",
                  content: [{ type: "text", text: `完成 [${parts.join(" · ")}]` }],
                  timestamp: new Date().toISOString(),
                },
              ],
            }));
            return;
          }
        }
        get().setPaneState(paneId, (currentPane) => ({
          ...currentPane,
          isStreaming: false,
          rawOutput: [...currentPane.rawOutput, line],
        }));
      }
      return;
    }

    const parsed = parseClaudeStreamLine(line);

    try {
      const data = JSON.parse(line);
      if (data.type === "system" && data.subtype === "init" && data.session_id) {
        get().setPaneSessionId(paneId, data.session_id);
      }
      if (data.type === "result") {
        const extras: Partial<ChatPaneState> = { isStreaming: false };
        if (data.is_error || data.error) {
          extras.error = data.error || data.result || "Unknown error";
        }
        get().setPaneState(paneId, extras);
      }
    } catch {
      // ignore non-JSON
    }

    if (!parsed) {
      get().setPaneState(paneId, (currentPane) => ({
        ...currentPane,
        rawOutput: [...currentPane.rawOutput, line],
      }));
      return;
    }

    get().setPaneState(paneId, (currentPane) => {
      let newMessages = currentPane.messages;

      if (parsed.action === "add") {
        const lastIdx = newMessages.length - 1;
        if (
          parsed.message.role === "assistant" &&
          lastIdx >= 0 &&
          newMessages[lastIdx].role === "assistant"
        ) {
          newMessages = [...newMessages.slice(0, lastIdx), parsed.message];
        } else {
          newMessages = [...newMessages, parsed.message];
        }
      } else if (parsed.action === "block_start") {
        const lastIdx = newMessages.length - 1;
        if (lastIdx >= 0 && newMessages[lastIdx].role === "assistant") {
          const lastMsg = newMessages[lastIdx];
          const newBlock: ChatContentBlock =
            parsed.blockType === "thinking"
              ? { type: "thinking", text: "" }
              : { type: "text", text: "" };
          const updated = {
            ...lastMsg,
            content: [...lastMsg.content, newBlock],
          };
          newMessages = [...newMessages.slice(0, lastIdx), updated];
        }
      } else if (parsed.action === "delta") {
        const lastIdx = newMessages.length - 1;
        if (lastIdx >= 0 && newMessages[lastIdx].role === "assistant") {
          const lastMsg = newMessages[lastIdx];
          const blocks = [...lastMsg.content];
          const bi = parsed.blockIndex;
          if (bi < blocks.length) {
            const block = blocks[bi];
            if (block.type === "text" || block.type === "thinking") {
              blocks[bi] = { ...block, text: block.text + parsed.delta };
            }
          }
          newMessages = [
            ...newMessages.slice(0, lastIdx),
            { ...lastMsg, content: blocks },
          ];
        }
      }

      return {
        ...currentPane,
        rawOutput: [...currentPane.rawOutput, line],
        messages: newMessages,
      };
    });
  },

  addStreamLine: (line) => {
    get().addStreamLineToPane(DEFAULT_CHAT_PANE_ID, line);
  },

  setClaudeApiKeyOverride: (v) => { localStorage.setItem("chat_claudeApiKey", v); set({ claudeApiKeyOverride: v }); },
  setClaudeBaseUrlOverride: (v) => { localStorage.setItem("chat_claudeBaseUrl", v); set({ claudeBaseUrlOverride: v }); },
  setCodexApiKeyOverride: (v) => { localStorage.setItem("chat_codexApiKey", v); set({ codexApiKeyOverride: v }); },
  setCodexBaseUrlOverride: (v) => { localStorage.setItem("chat_codexBaseUrl", v); set({ codexBaseUrlOverride: v }); },

  setPaneStreaming: (paneId, value) => {
    get().setPaneState(paneId, { isStreaming: value });
  },
  setPaneSessionId: (paneId, id) => {
    get().setPaneState(paneId, { sessionId: id });
  },
  setPaneError: (paneId, error) => {
    get().setPaneState(paneId, { error });
  },
  setPaneProjectPath: (paneId, path) => {
    get().setPaneState(paneId, { projectPath: path });
  },
  setPaneModel: (paneId, model) => {
    localStorage.setItem("chat_lastUsedModel", model);
    get().setPaneState(paneId, { model });
  },
  setPaneSource: (paneId, source) => {
    const pane = get().getPaneState(paneId);
    if (pane.source === source) return;
    // Cancel any in-flight turn first — the new source uses a different
    // stream parser, so leftover events from the old turn would be parsed
    // wrong (e.g. Codex notifications fed to the Claude parser produce
    // garbled messages).
    if (pane.isStreaming) {
      void get().cancelPane(paneId);
    }
    // Reset the chat history along with the source change. Previous messages
    // were produced by the old CLI and don't belong to the new one's view.
    set((state) => {
      const updates = applyPaneStateUpdate(state, paneId, () =>
        createChatPaneState({
          projectPath: pane.projectPath,
          model: "",
          source,
        })
      );
      const nextPaneModelList = createChatPaneModelListState();
      return {
        ...updates,
        paneModelLists: {
          ...state.paneModelLists,
          [paneId]: nextPaneModelList,
        },
        ...(paneId === state.activePaneId
          ? toLegacyModelListFields(nextPaneModelList)
          : {}),
      };
    });
  },

  setStreaming: (v) => {
    get().setPaneStreaming(DEFAULT_CHAT_PANE_ID, v);
  },
  setSessionId: (id) => {
    get().setPaneSessionId(DEFAULT_CHAT_PANE_ID, id);
  },
  setError: (e) => {
    get().setPaneError(DEFAULT_CHAT_PANE_ID, e);
  },
  setProjectPath: (p) => {
    get().setPaneProjectPath(DEFAULT_CHAT_PANE_ID, p);
  },
  setModel: (m) => {
    get().setPaneModel(DEFAULT_CHAT_PANE_ID, m);
  },
  setSource: (s) => {
    get().setPaneSource(DEFAULT_CHAT_PANE_ID, s);
  },
  };
});
