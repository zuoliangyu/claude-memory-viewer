import { create } from "zustand";
import type {
  ProjectEntry,
  SessionIndexEntry,
  DisplayMessage,
  TokenUsageSummary,
  RequestRecord,
  ProjectCostEntry,
  SessionCostSummary,
  RequestLogFilter,
  SearchResult,
  Bookmark,
  DeleteLevel,
  RecycledItem,
} from "../types";
import { api } from "../services/api";

/**
 * Window size for the progressive message view: how many messages we
 * load on initial entry and how many we tack on per scroll-driven extension.
 * Smaller = faster first paint at the cost of more round-trips when the user
 * scrolls a lot. 30 keeps a typical viewport saturated while finishing the
 * first paint in well under a second even on long sessions.
 */
const MAIN_MESSAGES_PAGE_SIZE = 30;
/** Half-window when jumping to a specific message (e.g. via TOC). */
const JUMP_HALF_WINDOW = 15;

/**
 * In-flight `loadProjects` request, keyed by source. The Sidebar and
 * ProjectsPage both call `loadProjects` on mount; without this guard a cold
 * cache would trigger two concurrent full project scans on the backend.
 * Concurrent callers share the same promise.
 */
const projectsInFlight = new Map<string, Promise<void>>();

interface AppState {
  // Source
  source: "claude" | "codex" | "grok";
  setSource: (s: "claude" | "codex" | "grok") => void;

  // Display settings
  showTimestamp: boolean;
  showModel: boolean;
  toggleTimestamp: () => void;
  toggleModel: () => void;

  // Terminal shell (Windows only)
  terminalShell: "cmd" | "powershell";
  setTerminalShell: (shell: "cmd" | "powershell") => void;

  // Projects
  projects: ProjectEntry[];
  projectsLoading: boolean;
  selectedProject: string | null;

  // Sessions（仅 Valid，给主列表用 —— 老代码语义不变）
  sessions: SessionIndexEntry[];
  // 异常会话（Empty + Corrupt）—— 由同一次 getSessions 响应拆分而来，
  // 提供给 SessionsPage 顶部"损坏会话"banner 和"清理空会话"按钮使用。
  // **不要单独再 RPC 拿这个数据** —— 同一份缓存的拆片即可。
  invalidSessions: SessionIndexEntry[];
  sessionsLoading: boolean;
  selectedFilePath: string | null;

  // Messages — progressive (windowed) loading
  messages: DisplayMessage[];
  messagesLoading: boolean;
  messagesTotal: number;
  /** Index of the first loaded message in the full session (inclusive). */
  loadedStart: number;
  /** Exclusive index of the last loaded message + 1. */
  loadedEnd: number;
  /** True iff there are more messages older than the current window. */
  messagesHasMore: boolean;
  /** True iff there are more messages newer than the current window. */
  messagesHasNewer: boolean;

  // Search
  searchQuery: string;
  searchScope: "all" | "content" | "session" | "tags";
  searchResults: SearchResult[];
  searchLoading: boolean;

  // Stats
  tokenSummary: TokenUsageSummary | null;
  statsLoading: boolean;
  /** null = unknown (loading), true = first-time full scan, false = cache hit */
  statsIsFirstBuild: boolean | null;

  // Per-request log (档位 2)
  requestLog: RequestRecord[];
  requestLogTotal: number;
  requestLogTotalCost: number;
  requestLogLoading: boolean;
  requestLogFilter: RequestLogFilter;

  // Per-project cost ranking (档位 3)
  projectCosts: ProjectCostEntry[];
  projectCostsLoading: boolean;

  // Per-session cost (档位 4, keyed by file path)
  sessionCosts: Record<string, SessionCostSummary>;

  // Tags
  allTags: string[];
  tagFilter: string[];

  // Cross-project tags
  crossProjectTags: Record<string, string[]>;
  globalTagFilter: string[];

  // Bookmarks
  bookmarks: Bookmark[];
  bookmarksLoading: boolean;

  // Recyclebin
  recycledItems: RecycledItem[];
  recyclebinLoading: boolean;

  // Actions
  loadProjects: () => Promise<void>;
  selectProject: (projectId: string) => Promise<void>;
  selectSession: (filePath: string) => Promise<void>;
  deleteSession: (filePath: string, sessionId?: string) => Promise<void>;
  deleteProject: (projectId: string, level?: DeleteLevel) => Promise<void>;
  setProjectAlias: (projectId: string, alias: string | null) => Promise<void>;
  /** Extend the loaded window backwards by one page. */
  loadMoreMessages: () => Promise<void>;
  /** Extend the loaded window forwards by one page (only meaningful when loadedEnd < total). */
  loadNewerMessages: () => Promise<void>;
  /**
   * Replace the loaded window with one centered on `targetIndex`. Used when
   * the user clicks a TOC entry whose message isn't currently in the
   * window.
   */
  jumpToMessageIndex: (targetIndex: number) => Promise<void>;
  search: (query: string) => Promise<void>;
  setSearchScope: (scope: "all" | "content" | "session" | "tags") => void;
  loadStats: () => Promise<void>;
  /** Load (or reload) the per-request log with the current filter. */
  loadRequestLog: (filter?: RequestLogFilter) => Promise<void>;
  setRequestLogFilter: (filter: RequestLogFilter) => void;
  loadProjectCosts: () => Promise<void>;
  /** Fetch the cost summary for a session, cached per file path. */
  loadSessionCost: (filePath: string, force?: boolean) => Promise<SessionCostSummary | null>;
  clearSelection: () => void;
  /** Silently refresh projects and the current session list. `rebuildProjects`
   * is reserved for the explicit manual cache refresh. */
  refreshInBackground: (forceReload?: boolean, rebuildProjects?: boolean) => Promise<void>;
  /** Force-reload page 0 of the currently selected session's messages. Preserves user's scroll position in the viewport but rewinds state to the latest page. */
  reloadLatestMessages: () => Promise<void>;
  updateSessionMeta: (
    sessionId: string,
    alias: string | null,
    tags: string[]
  ) => Promise<void>;
  loadAllTags: () => Promise<void>;
  setTagFilter: (tags: string[]) => void;
  loadCrossProjectTags: () => Promise<void>;
  setGlobalTagFilter: (tags: string[]) => void;
  loadBookmarks: () => Promise<void>;
  addBookmark: (bookmark: Omit<Bookmark, "id" | "createdAt">) => Promise<void>;
  removeBookmark: (id: string) => Promise<void>;
  isBookmarked: (sessionId: string, messageId?: string | null) => boolean;

  loadRecycledItems: () => Promise<void>;
  restoreItem: (id: string) => Promise<void>;
  permanentlyDeleteItem: (id: string) => Promise<void>;
  emptyRecyclebin: () => Promise<void>;
  cleanupOrphanDirs: () => Promise<number>;
}

export const useAppStore = create<AppState>((set, get) => ({
  source: "claude",
  setSource: (s) => {
    set({
      source: s,
      projects: [],
      projectsLoading: false,
      sessions: [],
      invalidSessions: [],
      sessionsLoading: false,
      messages: [],
      messagesLoading: false,
      selectedProject: null,
      selectedFilePath: null,
      searchResults: [],
      searchQuery: "",
      searchScope: "all",
      searchLoading: false,
      tokenSummary: null,
      statsLoading: false,
      statsIsFirstBuild: null,
      requestLog: [],
      requestLogTotal: 0,
      requestLogTotalCost: 0,
      requestLogLoading: false,
      projectCosts: [],
      projectCostsLoading: false,
      sessionCosts: {},
      allTags: [],
      tagFilter: [],
      crossProjectTags: {},
      globalTagFilter: [],
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

  terminalShell: (localStorage.getItem("terminalShell") === "powershell" ? "powershell" : "cmd") as "cmd" | "powershell",
  setTerminalShell: (shell) => {
    localStorage.setItem("terminalShell", shell);
    set({ terminalShell: shell });
  },

  projects: [],
  projectsLoading: false,
  selectedProject: null,

  sessions: [],
  invalidSessions: [],
  sessionsLoading: false,
  selectedFilePath: null,

  messages: [],
  messagesLoading: false,
  messagesTotal: 0,
  loadedStart: 0,
  loadedEnd: 0,
  messagesHasMore: false,
  messagesHasNewer: false,

  searchQuery: "",
  searchScope: "all",
  searchResults: [],
  searchLoading: false,

  tokenSummary: null,
  statsLoading: false,
  statsIsFirstBuild: null,

  requestLog: [],
  requestLogTotal: 0,
  requestLogTotalCost: 0,
  requestLogLoading: false,
  requestLogFilter: { page: 0, pageSize: 500 },

  projectCosts: [],
  projectCostsLoading: false,

  sessionCosts: {},

  allTags: [],
  tagFilter: [],

  crossProjectTags: {},
  globalTagFilter: [],

  bookmarks: [],
  bookmarksLoading: false,

  recycledItems: [],
  recyclebinLoading: false,

  loadProjects: async () => {
    const requestSource = get().source;
    // Share a single in-flight request per source so duplicate mount-time
    // callers (Sidebar + ProjectsPage) don't each kick off a full scan.
    const existing = projectsInFlight.get(requestSource);
    if (existing) return existing;

    const run = (async () => {
      set({ projectsLoading: true });
      try {
        const projects = await api.getProjects(requestSource);
        if (get().source !== requestSource) return;
        set({ projects, projectsLoading: false });
      } catch (e) {
        console.error("Failed to load projects:", e);
        if (get().source === requestSource) {
          set({ projectsLoading: false });
        }
      } finally {
        projectsInFlight.delete(requestSource);
      }
    })();

    projectsInFlight.set(requestSource, run);
    return run;
  },

  selectProject: async (projectId: string) => {
    const requestSource = get().source;
    set({
      selectedProject: projectId,
      sessions: [],
      invalidSessions: [],
      sessionsLoading: true,
      selectedFilePath: null,
      messages: [],
      messagesTotal: 0,
      loadedStart: 0,
      loadedEnd: 0,
      messagesHasMore: false,
      messagesHasNewer: false,
      tagFilter: [],
    });
    try {
      // 后端 v3 起 getSessions 返回全分类（含 Empty + Corrupt），由前端按
      // status 拆分。一次 RPC 同时填两个数组，避免 SessionsPage 再单独拉一次
      // getInvalidSessions —— 冷缓存时两个调用会各自启动一次完整扫描，是
      // 用户报 "侧边栏黄色项目点进去卡死" 的根因。
      const all = await api.getSessions(requestSource, projectId);
      // Stale check: ignore result if user already navigated to another project
      if (
        get().source !== requestSource ||
        get().selectedProject !== projectId
      ) {
        return;
      }
      const validSessions = all.filter(
        (s) => (s.status ?? "valid") === "valid",
      );
      const invalidSessions = all.filter(
        (s) => (s.status ?? "valid") !== "valid",
      );
      set((state) => ({
        sessions: validSessions,
        invalidSessions,
        sessionsLoading: false,
        projects: state.projects.map((p) =>
          p.id === projectId ? { ...p, sessionCount: validSessions.length } : p
        ),
      }));
      // Load all tags for this project
      if (get().selectedProject === projectId) {
        get().loadAllTags();
      }
    } catch (e) {
      console.error("Failed to load sessions:", e);
      if (
        get().source === requestSource &&
        get().selectedProject === projectId
      ) {
        set({ sessions: [], invalidSessions: [], sessionsLoading: false });
      }
    }
  },

  selectSession: async (filePath: string) => {
    const requestSource = get().source;
    set({
      selectedFilePath: filePath,
      messagesLoading: true,
      messages: [],
      messagesTotal: 0,
      loadedStart: 0,
      loadedEnd: 0,
      messagesHasMore: false,
      messagesHasNewer: false,
    });
    try {
      // Initial load: the tail window — same code path as before, just
      // with a smaller page size. The result tells us `total`, which we
      // use to seed loadedStart / loadedEnd.
      const result = await api.getMessages(requestSource, filePath, 0, MAIN_MESSAGES_PAGE_SIZE, true);
      if (
        get().source !== requestSource ||
        get().selectedFilePath !== filePath
      ) {
        return;
      }
      const loadedStart = Math.max(0, result.total - result.messages.length);
      const loadedEnd = result.total;
      set({
        messages: result.messages,
        messagesTotal: result.total,
        loadedStart,
        loadedEnd,
        messagesHasMore: loadedStart > 0,
        messagesHasNewer: false,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to load messages:", e);
      if (
        get().source === requestSource &&
        get().selectedFilePath === filePath
      ) {
        set({ messagesLoading: false });
      }
    }
  },

  deleteSession: async (filePath: string, sessionId?: string) => {
    const { source, selectedProject } = get();
    await api.deleteSession(filePath, source, selectedProject || undefined, sessionId);
    set((state) => ({
      sessions: state.sessions.filter((s) => s.filePath !== filePath),
      // 损坏 / 空会话也可能在这里被删（清理对话框走这条路径），同步清掉。
      invalidSessions: state.invalidSessions.filter(
        (s) => s.filePath !== filePath,
      ),
    }));
  },

  deleteProject: async (projectId: string, level?: DeleteLevel) => {
    const { source } = get();
    await api.deleteProject(source, projectId, level || "sessionOnly");
    // 在 await 完成后重新读取 selectedProject，避免竞态
    set((state) => {
      const filtered = state.projects.filter((p) => p.id !== projectId);
      if (state.selectedProject === projectId) {
        return {
          projects: filtered,
          selectedProject: null,
          selectedFilePath: null,
          sessions: [],
          invalidSessions: [],
          messages: [],
          messagesTotal: 0,
          loadedStart: 0,
          loadedEnd: 0,
          messagesHasMore: false,
          messagesHasNewer: false,
        };
      }
      return { projects: filtered };
    });
  },

  setProjectAlias: async (projectId: string, alias: string | null) => {
    const { source, projects } = get();
    const prev = projects;
    // 乐观更新
    set({ projects: projects.map(p =>
      p.id === projectId ? { ...p, alias } : p
    )});
    try {
      await api.setProjectAlias(source, projectId, alias);
    } catch (e) {
      set({ projects: prev }); // 回滚
      throw e;
    }
  },

  loadMoreMessages: async () => {
    // Extend the loaded window backwards (towards older messages).
    const state = get();
    if (
      !state.selectedFilePath ||
      !state.messagesHasMore ||
      state.messagesLoading ||
      state.loadedStart <= 0
    ) {
      return;
    }

    const requestEnd = state.loadedStart;
    const requestStart = Math.max(0, requestEnd - MAIN_MESSAGES_PAGE_SIZE);
    set({ messagesLoading: true });
    try {
      const result = await api.getMessagesRange(
        state.source,
        state.selectedFilePath,
        requestStart,
        requestEnd,
      );
      const current = get();
      if (
        current.source !== state.source ||
        current.selectedFilePath !== state.selectedFilePath
      ) {
        set({ messagesLoading: false });
        return;
      }
      // Defensive: if the file grew under us, the backend's `total` is the
      // source of truth.
      const total = result.total;
      const newStart = result.start;
      // No-progress guard: if the backend didn't actually move our window
      // backwards (empty slice, or start >= where we already were), force
      // messagesHasMore off so an outer loop ("加载全部") doesn't spin.
      const madeProgress =
        result.messages.length > 0 && newStart < current.loadedStart;
      if (!madeProgress) {
        console.warn(
          "loadMoreMessages: backend returned no progress",
          { requestStart, requestEnd, resultStart: newStart, resultLen: result.messages.length },
        );
        set({
          messagesTotal: total,
          messagesHasMore: false,
          messagesHasNewer: current.loadedEnd < total,
          messagesLoading: false,
        });
        return;
      }
      set({
        messages: [...result.messages, ...current.messages],
        messagesTotal: total,
        loadedStart: newStart,
        messagesHasMore: newStart > 0,
        messagesHasNewer: current.loadedEnd < total,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to load more messages:", e);
      // Treat hard failure as "no more" so an outer loop doesn't retry forever.
      set({ messagesHasMore: false, messagesLoading: false });
    }
  },

  loadNewerMessages: async () => {
    // Extend the loaded window forwards (towards newer messages). Only
    // meaningful after a `jumpToMessageIndex` since selectSession starts
    // at the tail.
    const state = get();
    if (
      !state.selectedFilePath ||
      !state.messagesHasNewer ||
      state.messagesLoading ||
      state.loadedEnd >= state.messagesTotal
    ) {
      return;
    }

    const requestStart = state.loadedEnd;
    const requestEnd = Math.min(
      state.messagesTotal,
      state.loadedEnd + MAIN_MESSAGES_PAGE_SIZE,
    );
    set({ messagesLoading: true });
    try {
      const result = await api.getMessagesRange(
        state.source,
        state.selectedFilePath,
        requestStart,
        requestEnd,
      );
      const current = get();
      if (
        current.source !== state.source ||
        current.selectedFilePath !== state.selectedFilePath
      ) {
        set({ messagesLoading: false });
        return;
      }
      const total = result.total;
      const newEnd = result.end;
      const madeProgress =
        result.messages.length > 0 && newEnd > current.loadedEnd;
      if (!madeProgress) {
        console.warn(
          "loadNewerMessages: backend returned no progress",
          { requestStart, requestEnd, resultEnd: newEnd, resultLen: result.messages.length },
        );
        set({
          messagesTotal: total,
          messagesHasMore: current.loadedStart > 0,
          messagesHasNewer: false,
          messagesLoading: false,
        });
        return;
      }
      set({
        messages: [...current.messages, ...result.messages],
        messagesTotal: total,
        loadedEnd: newEnd,
        messagesHasMore: current.loadedStart > 0,
        messagesHasNewer: newEnd < total,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to load newer messages:", e);
      set({ messagesHasNewer: false, messagesLoading: false });
    }
  },

  jumpToMessageIndex: async (targetIndex: number) => {
    // Replace the loaded window with one centered on `targetIndex`. Used
    // by TOC navigation when the target message is currently outside the
    // loaded range.
    const state = get();
    if (!state.selectedFilePath) return;

    const total = state.messagesTotal;
    if (total <= 0) return;

    const clampedTarget = Math.max(0, Math.min(targetIndex, total - 1));
    const requestStart = Math.max(0, clampedTarget - JUMP_HALF_WINDOW);
    const requestEnd = Math.min(total, clampedTarget + JUMP_HALF_WINDOW + 1);

    // Already covered? Nothing to do.
    if (state.loadedStart <= requestStart && state.loadedEnd >= requestEnd) {
      return;
    }

    set({ messagesLoading: true });
    try {
      const result = await api.getMessagesRange(
        state.source,
        state.selectedFilePath,
        requestStart,
        requestEnd,
      );
      const current = get();
      if (
        current.source !== state.source ||
        current.selectedFilePath !== state.selectedFilePath
      ) {
        return;
      }
      const newTotal = result.total;
      set({
        messages: result.messages,
        messagesTotal: newTotal,
        loadedStart: result.start,
        loadedEnd: result.end,
        messagesHasMore: result.start > 0,
        messagesHasNewer: result.end < newTotal,
        messagesLoading: false,
      });
    } catch (e) {
      console.error("Failed to jump to message:", e);
      set({ messagesLoading: false });
    }
  },

  search: async (query: string) => {
    const scope = get().searchScope;
    set({ searchQuery: query, searchLoading: true });
    if (!query.trim()) {
      set({ searchResults: [], searchLoading: false });
      return;
    }
    try {
      const results = await api.globalSearch(get().source, query, 50, scope);
      set({ searchResults: results, searchLoading: false });
    } catch (e) {
      console.error("Failed to search:", e);
      set({ searchLoading: false });
    }
  },

  setSearchScope: (scope) => {
    set({ searchScope: scope });
    const query = get().searchQuery;
    if (query.trim()) {
      void get().search(query);
    }
  },

  loadStats: async () => {
    set({ statsLoading: true, statsIsFirstBuild: null });
    try {
      const tokenSummary = await api.getStats(get().source);
      set({ tokenSummary, statsLoading: false, statsIsFirstBuild: tokenSummary.isFirstBuild });
    } catch (e) {
      console.error("Failed to load stats:", e);
      set({ statsLoading: false });
    }
  },

  setRequestLogFilter: (filter: RequestLogFilter) => {
    set((state) => ({ requestLogFilter: { ...state.requestLogFilter, ...filter } }));
  },

  loadRequestLog: async (filterOverride?: RequestLogFilter) => {
    const requestSource = get().source;
    const filter: RequestLogFilter = {
      ...get().requestLogFilter,
      ...(filterOverride ?? {}),
    };
    if (filterOverride) {
      set({ requestLogFilter: filter });
    }
    set({ requestLogLoading: true });
    try {
      const page = await api.getRequestLog(requestSource, filter);
      if (get().source !== requestSource) return;
      set({
        requestLog: page.records,
        requestLogTotal: page.total,
        requestLogTotalCost: page.totalCostUsd,
        requestLogLoading: false,
      });
    } catch (e) {
      console.error("Failed to load request log:", e);
      if (get().source === requestSource) {
        set({ requestLogLoading: false });
      }
    }
  },

  loadProjectCosts: async () => {
    const requestSource = get().source;
    set({ projectCostsLoading: true });
    try {
      const projectCosts = await api.getProjectCosts(requestSource);
      if (get().source !== requestSource) return;
      set({ projectCosts, projectCostsLoading: false });
    } catch (e) {
      console.error("Failed to load project costs:", e);
      if (get().source === requestSource) {
        set({ projectCostsLoading: false });
      }
    }
  },

  loadSessionCost: async (filePath: string, force = false) => {
    const cached = get().sessionCosts[filePath];
    if (cached && !force) return cached;
    const requestSource = get().source;
    try {
      const summary = await api.getSessionCost(requestSource, filePath);
      if (get().source !== requestSource) return null;
      set((state) => ({
        sessionCosts: { ...state.sessionCosts, [filePath]: summary },
      }));
      return summary;
    } catch (e) {
      console.error("Failed to load session cost:", e);
      return null;
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

  reloadLatestMessages: async () => {
    const { source: currentSource, selectedFilePath } = get();
    if (!selectedFilePath) return;
    try {
      const result = await api.getMessages(currentSource, selectedFilePath, 0, MAIN_MESSAGES_PAGE_SIZE, true);
      if (
        get().source !== currentSource ||
        get().selectedFilePath !== selectedFilePath
      ) {
        return;
      }
      const loadedStart = Math.max(0, result.total - result.messages.length);
      set({
        messages: result.messages,
        messagesTotal: result.total,
        loadedStart,
        loadedEnd: result.total,
        messagesHasMore: loadedStart > 0,
        messagesHasNewer: false,
      });
    } catch {
      // silent
    }
  },

  refreshInBackground: async (forceReload = false, rebuildProjects = false) => {
    const { source, selectedProject } = get();
    try {
      const loadProjects = rebuildProjects
        ? api.rebuildProjectsCache
        : forceReload
          ? api.refreshProjectsCache
          : api.getProjects;
      const loadSessions = forceReload
        ? api.refreshSessionsCache
        : api.getSessions;
      const projects = await loadProjects(source);
      if (get().source !== source) return;

      if (selectedProject) {
        // Fetch sessions first, then update projects + sessions atomically
        // to avoid sessionCount flashing between raw file count and filtered count
        const all = await loadSessions(source, selectedProject);
        if (
          get().source !== source ||
          get().selectedProject !== selectedProject
        ) {
          return;
        }
        // 后端 v3 起返回全分类（含 Empty + Corrupt），拆成 sessions / invalidSessions。
        const validSessions = all.filter(
          (s) => (s.status ?? "valid") === "valid",
        );
        const invalidSessions = all.filter(
          (s) => (s.status ?? "valid") !== "valid",
        );
        set({
          sessions: validSessions,
          invalidSessions,
          projects: projects.map((p) =>
            p.id === selectedProject
              ? { ...p, sessionCount: validSessions.length }
              : p
          ),
        });
      } else {
        set({ projects });
      }
    } catch (e) {
      console.error("Background refresh failed:", e);
    }

    // 静默刷新当前会话消息（仅当窗口贴在尾部，不打断用户上翻历史/跳转后正在浏览中段）
    const { selectedFilePath, loadedEnd, messagesTotal, messagesLoading, source: currentSource } = get();
    const atTail = loadedEnd >= messagesTotal && messagesTotal > 0;
    // `messagesLoading` 守卫：用户正在点"加载全部"时 handleLoadAll 在循环
    // 调 loadMoreMessages，这里不能再发请求把 messages 覆盖回尾部窗口
    // ——否则刚加载的历史被擦掉、外层循环又重新加载，进度数字就会循环波动。
    if (!selectedFilePath || !atTail || messagesLoading) return;
    try {
      const result = await api.getMessages(currentSource, selectedFilePath, 0, MAIN_MESSAGES_PAGE_SIZE, true);
      const after = get();
      if (
        after.source !== currentSource ||
        after.selectedFilePath !== selectedFilePath ||
        after.loadedEnd < after.messagesTotal ||
        after.messagesLoading
      ) {
        return;
      }
      // 文件未增长——保持现状，特别是别动 loadedStart，避免把用户已展开的窗口擦回尾部。
      if (result.total === after.messagesTotal) return;
      // 用户已上翻历史超出默认尾部页：只更新总数 + 标记有新消息，不替换 messages
      // ——替换会把已加载历史擦掉，又把 handleLoadAll 的循环踢回起点（进度循环波动）。
      const userExpanded = after.loadedStart < Math.max(0, after.messagesTotal - MAIN_MESSAGES_PAGE_SIZE);
      if (userExpanded) {
        set({
          messagesTotal: result.total,
          messagesHasNewer: after.loadedEnd < result.total,
        });
        return;
      }
      // 普通的尾部追加场景（窗口本来就只有最后一页）：照旧整段替换。
      const newStart = Math.max(0, result.total - result.messages.length);
      set({
        messages: result.messages,
        messagesTotal: result.total,
        loadedStart: newStart,
        loadedEnd: result.total,
        messagesHasMore: newStart > 0,
        messagesHasNewer: false,
      });
    } catch {
      // 静默失败
    }
  },

  updateSessionMeta: async (
    sessionId: string,
    alias: string | null,
    tags: string[]
  ) => {
    const { source, selectedProject, sessions } = get();
    if (!selectedProject) return;
    const session = sessions.find(s => s.sessionId === sessionId);
    const filePath = session?.filePath ?? null;
    await api.updateSessionMeta(source, selectedProject, sessionId, alias, tags, filePath);
    // Update local state
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.sessionId === sessionId
          ? { ...s, alias, tags: tags.length > 0 ? tags : null }
          : s
      ),
    }));
    // Refresh tags
    get().loadAllTags();
  },

  loadAllTags: async () => {
    const { source, selectedProject } = get();
    if (!selectedProject) return;
    try {
      const allTags = await api.getAllTags(source, selectedProject);
      // Drop the result if the user switched source/project while we were
      // waiting — otherwise stale tags would land on the new view.
      const latest = get();
      if (latest.source !== source || latest.selectedProject !== selectedProject) {
        return;
      }
      set({ allTags });
    } catch (e) {
      console.error("Failed to load tags:", e);
    }
  },

  setTagFilter: (tags: string[]) => {
    set({ tagFilter: tags });
  },

  loadCrossProjectTags: async () => {
    const source = get().source;
    try {
      const crossProjectTags = await api.getCrossProjectTags(source);
      // Drop stale result if the user switched source while waiting.
      if (get().source !== source) return;
      set({ crossProjectTags });
    } catch (e) {
      console.error("Failed to load cross-project tags:", e);
    }
  },

  setGlobalTagFilter: (tags: string[]) => {
    set({ globalTagFilter: tags });
  },

  loadBookmarks: async () => {
    set({ bookmarksLoading: true });
    try {
      const bookmarks = await api.listBookmarks();
      set({ bookmarks, bookmarksLoading: false });
    } catch (e) {
      console.error("Failed to load bookmarks:", e);
      set({ bookmarksLoading: false });
    }
  },

  addBookmark: async (bookmark) => {
    try {
      const created = await api.addBookmark(bookmark);
      set((state) => ({ bookmarks: [...state.bookmarks, created] }));
    } catch (e) {
      console.error("Failed to add bookmark:", e);
    }
  },

  removeBookmark: async (id) => {
    try {
      await api.removeBookmark(id);
      set((state) => ({
        bookmarks: state.bookmarks.filter((b) => b.id !== id),
      }));
    } catch (e) {
      console.error("Failed to remove bookmark:", e);
    }
  },

  isBookmarked: (sessionId, messageId) => {
    return get().bookmarks.some(
      (b) => b.sessionId === sessionId && b.messageId === (messageId ?? null)
    );
  },

  loadRecycledItems: async () => {
    set({ recyclebinLoading: true });
    try {
      const recycledItems = await api.listRecycledItems();
      set({ recycledItems, recyclebinLoading: false });
    } catch (e) {
      console.error("Failed to load recycled items:", e);
      set({ recyclebinLoading: false });
    }
  },

  restoreItem: async (id: string) => {
    await api.restoreRecycledItem(id);
    set((state) => ({
      recycledItems: state.recycledItems.filter((item) => item.id !== id),
    }));
    // Refresh projects/sessions list so the restored item shows up
    void get().refreshInBackground(true);
  },

  permanentlyDeleteItem: async (id: string) => {
    await api.permanentlyDeleteRecycledItem(id);
    set((state) => ({
      recycledItems: state.recycledItems.filter((item) => item.id !== id),
    }));
  },

  emptyRecyclebin: async () => {
    await api.emptyRecyclebin();
    set({ recycledItems: [] });
  },

  cleanupOrphanDirs: async () => {
    const { source } = get();
    const count = await api.cleanupOrphanDirs(source);
    // 刷新回收站列表
    if (count > 0) {
      const recycledItems = await api.listRecycledItems();
      set({ recycledItems });
    }
    return count;
  },
}));
