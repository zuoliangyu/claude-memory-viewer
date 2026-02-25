import { create } from "zustand";
import type { QuickChatMessage, ModelInfo } from "../types/chat";
import { api } from "../services/api";

interface QuickChatState {
  source: "claude" | "codex";
  model: string;
  messages: QuickChatMessage[];
  isStreaming: boolean;
  error: string | null;

  // Model list
  modelList: ModelInfo[];
  modelListLoading: boolean;

  // Actions
  sendMessage: (prompt: string) => Promise<void>;
  clearMessages: () => void;
  setSource: (s: "claude" | "codex") => void;
  setModel: (m: string) => void;
  fetchModelList: (source: "claude" | "codex") => Promise<void>;
  cancelStream: () => void;
}

let cancelFn: (() => void) | null = null;

export const useQuickChatStore = create<QuickChatState>((set, get) => ({
  source: "claude",
  model: "",
  messages: [],
  isStreaming: false,
  error: null,
  modelList: [],
  modelListLoading: false,

  sendMessage: async (prompt: string) => {
    const { source, model, messages } = get();
    if (!model) {
      set({ error: "请先选择模型" });
      return;
    }

    const userMsg: QuickChatMessage = { role: "user", content: prompt };
    const newMessages = [...messages, userMsg];
    // Add placeholder for assistant
    const assistantMsg: QuickChatMessage = { role: "assistant", content: "" };

    set({
      messages: [...newMessages, assistantMsg],
      isStreaming: true,
      error: null,
    });

    try {
      const cleanup = await api.startQuickChat(
        source,
        newMessages,
        model,
        // onChunk
        (chunk: string) => {
          set((state) => {
            const msgs = [...state.messages];
            const last = msgs[msgs.length - 1];
            if (last && last.role === "assistant") {
              msgs[msgs.length - 1] = { ...last, content: last.content + chunk };
            }
            return { messages: msgs };
          });
        },
        // onError
        (err: string) => {
          set({ error: err, isStreaming: false });
        },
        // onDone
        () => {
          set({ isStreaming: false });
          cancelFn = null;
        },
      );
      cancelFn = cleanup;
    } catch (e) {
      set({
        isStreaming: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  clearMessages: () => {
    if (cancelFn) {
      cancelFn();
      cancelFn = null;
    }
    set({ messages: [], isStreaming: false, error: null });
  },

  setSource: (s) => set({ source: s }),
  setModel: (m) => set({ model: m }),

  fetchModelList: async (source) => {
    set({ modelListLoading: true });
    try {
      const models = await api.listModels(source, "", "");
      set({ modelList: models, modelListLoading: false });
    } catch {
      set({ modelListLoading: false });
    }
  },

  cancelStream: () => {
    if (cancelFn) {
      cancelFn();
      cancelFn = null;
    }
    set({ isStreaming: false });
  },
}));
