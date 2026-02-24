import { create } from "zustand";
import type {
  ProjectEntry,
  SessionIndexEntry,
  DisplayMessage,
  TokenUsageSummary,
  SearchResult,
} from "../types";
import { api } from "../services/api";

interface AppState {
  // Source
  source: "claude" | "codex";
  setSource: (s: "claude" | "codex") => void;

  // Display settings
  showTimestamp: boolean;
  showModel: boolean;
  toggleTimestamp: () => void;
  toggleModel: () => void;

  // Projects
  projects: ProjectEntry[];
  projectsLoading: boolean;
  selectedProject: string | null;

  // Sessions
  sessions: SessionIndexEntry[];
  sessionsLoading: boolean;
  selectedFilePath: string | null;

  // Messages
  messages: DisplayMessage[];
  messagesLoading: boolean;
  messagesTotal: number;
  messagesPage: number;
  messagesHasMore: boolean;

  // Search
  searchQuery: string;
  searchResults: SearchResult[];
  searchLoading: boolean;

  // Stats
  tokenSummary: TokenUsageSummary | null;
  statsLoading: boolean;

  // Actions
  loadProjects: () => Promise<void>;
  selectProject: (projectId: string) => Promise<void>;
  selectSession: (filePath: string) => Promise<void>;
  deleteSession: (filePath: string) => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  search: (query: string) => Promise<void>;
  loadStats: () => Promise<void>;
  clearSelection: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  source: "claude",
  setSource: (s) => {
    set({
      source: s,
      projects: [],
      sessions: [],
      messages: [],
      selectedProject: null,
      selectedFilePath: null,
      searchResults: [],
      searchQuery: "",
      tokenSummary: null,
    });
  },

  showTimestamp: localStorage.getItem("showTimestamp") !== "false",
  showModel: localStorage.getItem("showModel") !== "false",
  toggleTimestamp: () => {
    const next = !get().showTimestamp;
    localStorage.setItem("showTimestamp", String(next));
    set({ showTimestamp: next });
  },
  toggleModel: () => {
    const next = !get().showModel;
    localStorage.setItem("showModel", String(next));
    set({ showModel: next });
  },

  projects: [],
  projectsLoading: false,
  selectedProject: null,

  sessions: [],
  sessionsLoading: false,
  selectedFilePath: null,

  messages: [],
  messagesLoading: false,
  messagesTotal: 0,
  messagesPage: 0,
  messagesHasMore: false,

  searchQuery: "",
  searchResults: [],
  searchLoading: false,

  tokenSummary: null,
  statsLoading: false,

  loadProjects: async () => {
    set({ projectsLoading: true });
    try {
      const projects = await api.getProjects(get().source);
      set({ projects, projectsLoading: false });
    } catch (e) {
      console.error("Failed to load projects:", e);
      set({ projectsLoading: false });
    }
  },

  selectProject: async (projectId: string) => {
    set({
      selectedProject: projectId,
      sessionsLoading: true,
      selectedFilePath: null,
      messages: [],
      messagesTotal: 0,
      messagesPage: 0,
    });
    try {
      const sessions = await api.getSessions(get().source, projectId);
      set((state) => ({
        sessions,
        sessionsLoading: false,
        projects: state.projects.map((p) =>
          p.id === projectId ? { ...p, sessionCount: sessions.length } : p
        ),
      }));
    } catch (e) {
      console.error("Failed to load sessions:", e);
      set({ sessionsLoading: false });
    }
  },

  selectSession: async (filePath: string) => {
    set({
      selectedFilePath: filePath,
      messagesLoading: true,
      messages: [],
      messagesTotal: 0,
      messagesPage: 0,
    });
    try {
      const result = await api.getMessages(get().source, filePath, 0, 50, true);
      set({
        messages: result.messages,
        messagesTotal: result.total,
        messagesPage: 0,
        messagesHasMore: result.hasMore,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to load messages:", e);
      set({ messagesLoading: false });
    }
  },

  deleteSession: async (filePath: string) => {
    await api.deleteSession(filePath);
    set((state) => ({
      sessions: state.sessions.filter((s) => s.filePath !== filePath),
    }));
  },

  loadMoreMessages: async () => {
    const state = get();
    if (
      !state.selectedFilePath ||
      !state.messagesHasMore ||
      state.messagesLoading
    ) {
      return;
    }

    const nextPage = state.messagesPage + 1;
    set({ messagesLoading: true });
    try {
      const result = await api.getMessages(
        state.source,
        state.selectedFilePath,
        nextPage,
        50,
        true
      );
      set({
        messages: [...result.messages, ...state.messages],
        messagesPage: nextPage,
        messagesHasMore: result.hasMore,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to load more messages:", e);
      set({ messagesLoading: false });
    }
  },

  search: async (query: string) => {
    set({ searchQuery: query, searchLoading: true });
    if (!query.trim()) {
      set({ searchResults: [], searchLoading: false });
      return;
    }
    try {
      const results = await api.globalSearch(get().source, query, 50);
      set({ searchResults: results, searchLoading: false });
    } catch (e) {
      console.error("Failed to search:", e);
      set({ searchLoading: false });
    }
  },

  loadStats: async () => {
    set({ statsLoading: true });
    try {
      const tokenSummary = await api.getStats(get().source);
      set({ tokenSummary, statsLoading: false });
    } catch (e) {
      console.error("Failed to load stats:", e);
      set({ statsLoading: false });
    }
  },

  clearSelection: () => {
    set({
      selectedProject: null,
      selectedFilePath: null,
      sessions: [],
      messages: [],
    });
  },
}));
