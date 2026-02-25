import { create } from "zustand";
import type {
  CliInstallation,
  CliConfig,
  ModelInfo,
  ChatMessage,
  ChatContentBlock,
} from "../types/chat";
import { api } from "../services/api";

interface ChatState {
  // Chat status
  isActive: boolean;
  sessionId: string | null;
  source: "claude" | "codex";
  projectPath: string;
  model: string;

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

  // Settings (persisted to localStorage)
  skipPermissions: boolean;
  defaultModel: string;

  // Actions
  detectCli: () => Promise<void>;
  fetchCliConfig: (source: "claude" | "codex") => Promise<void>;
  fetchModelList: (source: "claude" | "codex") => Promise<void>;
  startNewChat: (
    source: "claude" | "codex",
    projectPath: string,
    prompt: string,
    model: string
  ) => Promise<void>;
  continueExistingChat: (
    source: "claude" | "codex",
    sessionId: string,
    projectPath: string,
    prompt: string,
    model: string
  ) => Promise<void>;
  cancelChat: () => Promise<void>;
  clearChat: () => void;
  setSkipPermissions: (v: boolean) => void;
  setDefaultModel: (m: string) => void;
  addCustomModel: (modelId: string) => void;
  removeCustomModel: (modelId: string) => void;
  addStreamLine: (line: string) => void;
  setStreaming: (v: boolean) => void;
  setSessionId: (id: string) => void;
  setError: (e: string | null) => void;
  setProjectPath: (p: string) => void;
  setSource: (s: "claude" | "codex") => void;
  setModel: (m: string) => void;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function parseClaudeStreamLine(line: string): ChatMessage | null {
  let data: any;
  try {
    data = JSON.parse(line);
  } catch {
    // Not JSON — ignore (e.g. stderr text, blank lines)
    return null;
  }

  if (!data || typeof data !== "object" || !data.type) return null;

  const recordType: string = data.type;

  // System init — extract session_id only, no visible message
  if (recordType === "system" && data.subtype === "init") {
    return null;
  }

  // Assistant message
  if (recordType === "assistant" && data.message) {
    const msg = data.message;
    const content = parseContentValue(msg.content);
    if (content.length === 0) return null;

    const usage = msg.usage || data.usage;
    // Skip empty error responses (API error → 0 output tokens, content is just error text)
    if (usage && (usage.output_tokens ?? 0) === 0) return null;

    return {
      id: crypto.randomUUID(),
      role: "assistant",
      content,
      model: msg.model || data.model,
      timestamp: data.timestamp || new Date().toISOString(),
      usage: usage
        ? { inputTokens: usage.input_tokens ?? 0, outputTokens: usage.output_tokens ?? 0 }
        : undefined,
    };
  }

  // User message (including tool_result content blocks)
  if (recordType === "user" && data.message) {
    const msg = data.message;
    const content = parseContentValue(msg.content);
    if (content.length === 0) return null;

    return {
      id: crypto.randomUUID(),
      role: "user",
      content,
      timestamp: data.timestamp || new Date().toISOString(),
    };
  }

  // Result message — final summary
  if (recordType === "result") {
    const text =
      data.result ||
      data.error ||
      (data.is_error ? "Error" : "Done");
    const costInfo = data.total_cost_usd
      ? ` (cost: $${Number(data.total_cost_usd).toFixed(4)})`
      : "";
    const durationInfo = data.duration_ms
      ? ` (${(data.duration_ms / 1000).toFixed(1)}s)`
      : "";
    return {
      id: crypto.randomUUID(),
      role: "system",
      content: [{ type: "text", text: `${text}${durationInfo}${costInfo}` }],
      timestamp: new Date().toISOString(),
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

export const useChatStore = create<ChatState>((set, get) => ({
  isActive: false,
  sessionId: null,
  source: "claude",
  projectPath: "",
  model: "",

  messages: [],
  rawOutput: [],
  isStreaming: false,
  error: null,

  availableClis: [],

  modelList: [],
  modelListLoading: false,
  modelListError: null,

  cliConfig: null,
  cliConfigLoading: false,
  cliConfigError: null,

  skipPermissions: localStorage.getItem("chat_skipPermissions") === "true",
  defaultModel: localStorage.getItem("chat_defaultModel") || "",

  detectCli: async () => {
    try {
      const clis = await api.detectCli();
      set({ availableClis: clis });
    } catch (e) {
      console.error("Failed to detect CLI:", e);
    }
  },

  fetchCliConfig: async (source) => {
    set({ cliConfigLoading: true, cliConfigError: null });
    try {
      const config = await api.getCliConfig(source);
      set({ cliConfig: config, cliConfigLoading: false });
    } catch (e) {
      set({
        cliConfigLoading: false,
        cliConfigError: e instanceof Error ? e.message : String(e),
      });
    }
  },

  fetchModelList: async (source) => {
    set({ modelListLoading: true, modelListError: null });
    try {
      // Always pass empty strings — backend auto-reads CLI config
      const models = await api.listModels(source, "", "");
      // Merge custom models (localStorage) that aren't already in the list
      const customKey = `chat_customModels_${source}`;
      const customIds: string[] = JSON.parse(localStorage.getItem(customKey) || "[]");
      const existingIds = new Set(models.map((m) => m.id));
      const customModels: ModelInfo[] = customIds
        .filter((id) => !existingIds.has(id))
        .map((id) => ({
          id,
          name: id,
          provider: source === "claude" ? "anthropic" : "openai",
          group: "自定义",
          created: null,
        }));
      set({ modelList: [...customModels, ...models], modelListLoading: false });
    } catch (e) {
      set({
        modelListLoading: false,
        modelListError: e instanceof Error ? e.message : String(e),
      });
    }
  },

  startNewChat: async (source, projectPath, prompt, model) => {
    const state = get();
    set({
      isActive: true,
      isStreaming: true,
      source,
      projectPath,
      model,
      error: null,
      messages: [
        {
          id: crypto.randomUUID(),
          role: "user",
          content: [{ type: "text", text: prompt }],
          timestamp: new Date().toISOString(),
        },
      ],
      rawOutput: [],
    });

    try {
      const sessionId = await api.startChat({
        source,
        projectPath,
        prompt,
        model,
        skipPermissions: state.skipPermissions,
      });
      set({ sessionId });
    } catch (e) {
      set({
        isStreaming: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  continueExistingChat: async (source, sessionId, projectPath, prompt, model) => {
    const state = get();
    set({
      isActive: true,
      isStreaming: true,
      sessionId,
      source,
      projectPath,
      model,
      error: null,
      messages: [
        ...state.messages,
        {
          id: crypto.randomUUID(),
          role: "user",
          content: [{ type: "text", text: prompt }],
          timestamp: new Date().toISOString(),
        },
      ],
      rawOutput: [],
    });

    try {
      await api.continueChat({
        source,
        sessionId,
        projectPath,
        prompt,
        model,
        skipPermissions: state.skipPermissions,
      });
    } catch (e) {
      set({
        isStreaming: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  cancelChat: async () => {
    const { sessionId } = get();
    if (sessionId) {
      try {
        await api.cancelChat(sessionId);
      } catch (e) {
        console.error("Failed to cancel chat:", e);
      }
    }
    set({ isStreaming: false });
  },

  clearChat: () => {
    set({
      isActive: false,
      sessionId: null,
      messages: [],
      rawOutput: [],
      isStreaming: false,
      error: null,
    });
  },

  setSkipPermissions: (v) => {
    localStorage.setItem("chat_skipPermissions", String(v));
    set({ skipPermissions: v });
  },

  setDefaultModel: (m) => {
    localStorage.setItem("chat_defaultModel", m);
    set({ defaultModel: m });
  },

  addCustomModel: (modelId) => {
    const state = get();
    const customKey = `chat_customModels_${state.source}`;
    const existing: string[] = JSON.parse(localStorage.getItem(customKey) || "[]");
    if (!existing.includes(modelId)) {
      const updated = [...existing, modelId];
      localStorage.setItem(customKey, JSON.stringify(updated));
    }
    // Add to current modelList if not present
    if (!state.modelList.some((m) => m.id === modelId)) {
      set({
        modelList: [
          {
            id: modelId,
            name: modelId,
            provider: state.source === "claude" ? "anthropic" : "openai",
            group: "自定义",
            created: null,
          },
          ...state.modelList,
        ],
      });
    }
  },

  removeCustomModel: (modelId) => {
    const state = get();
    const customKey = `chat_customModels_${state.source}`;
    const existing: string[] = JSON.parse(localStorage.getItem(customKey) || "[]");
    const updated = existing.filter((id) => id !== modelId);
    localStorage.setItem(customKey, JSON.stringify(updated));
    set({ modelList: state.modelList.filter((m) => m.id !== modelId) });
  },

  addStreamLine: (line: string) => {
    const parsed = parseClaudeStreamLine(line);

    try {
      const data = JSON.parse(line);
      // Extract session_id from init messages
      if (data.type === "system" && data.subtype === "init" && data.session_id) {
        set({ sessionId: data.session_id });
      }
      // Result message means CLI is done — stop streaming
      if (data.type === "result") {
        const extras: Partial<ChatState> = { isStreaming: false };
        // If it's an error result, show in error banner
        if (data.is_error || data.error) {
          extras.error = data.error || data.result || "Unknown error";
        }
        set(extras as Partial<ChatState>);
      }
    } catch {
      // ignore non-JSON
    }

    set((state) => ({
      rawOutput: [...state.rawOutput, line],
      messages: parsed ? [...state.messages, parsed] : state.messages,
    }));
  },

  setStreaming: (v) => set({ isStreaming: v }),
  setSessionId: (id) => set({ sessionId: id }),
  setError: (e) => set({ error: e }),
  setProjectPath: (p) => set({ projectPath: p }),
  setSource: (s) => set({ source: s }),
  setModel: (m) => set({ model: m }),
}));
