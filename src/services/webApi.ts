import type {
  ProjectEntry,
  SessionIndexEntry,
  PaginatedMessages,
  RangeMessages,
  Trajectory,
  SearchResult,
  TokenUsageSummary,
  RequestLogPage,
  RequestLogFilter,
  ProjectCostEntry,
  SessionCostSummary,
  Bookmark,
  DeleteLevel,
  DeleteResult,
  ExportFormat,
  ScanProgress,
  RecycledItem,
  SkillsResult,
  ImportResult,
  SkillScope,
} from "../types";
import type { CliInstallation, ModelInfo, StartChatParams, ContinueChatParams, CliConfig } from "../types/chat";
import type {
  ProviderSyncStatus,
  SyncResult,
  CloneResult,
  RestoreOptions,
  RestoreResult,
} from "../types/providerSync";
import { getApiBaseUrl, getApiToken } from "./nodeConfig";

function getToken(): string | null {
  return getApiToken();
}

function notifyAuthRequired(): void {
  window.dispatchEvent(new CustomEvent("asv-auth-required"));
}

function applyAuthHeader(headers: Record<string, string>): Record<string, string> {
  const token = getToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }
  return headers;
}

/**
 * Wait until the user either updates the token (via AuthGate) or cancels
 * the prompt. Resolves with `true` if the token was saved, `false` if the
 * user dismissed the dialog. Multiple concurrent waiters share a single
 * promise so they all unblock together when the event fires.
 */
let pendingAuthRestoration: Promise<boolean> | null = null;

function awaitAuthRestoration(): Promise<boolean> {
  if (pendingAuthRestoration) return pendingAuthRestoration;
  pendingAuthRestoration = new Promise<boolean>((resolve) => {
    const settle = (value: boolean) => {
      window.removeEventListener("asv-auth-updated", onUpdated);
      window.removeEventListener("asv-auth-cancelled", onCancel);
      pendingAuthRestoration = null;
      resolve(value);
    };
    const onUpdated = () => settle(true);
    const onCancel = () => settle(false);
    window.addEventListener("asv-auth-updated", onUpdated, { once: true });
    window.addEventListener("asv-auth-cancelled", onCancel, { once: true });
  });
  return pendingAuthRestoration;
}

/**
 * Run a fetch executor and, if it returns 401, prompt the user for a token
 * (via AuthGate) and retry exactly once. Multiple in-flight requests
 * collapse onto one prompt — when the user saves the token, every waiter
 * retries with the new credentials. If the user cancels the prompt, the
 * original 401 response is returned so the caller can throw.
 *
 * The executor is called fresh each attempt so headers (including the
 * Bearer token) are rebuilt with whatever's in localStorage at the moment
 * of the request.
 */
async function withAuthRetry(execute: () => Promise<Response>): Promise<Response> {
  const resp = await execute();
  if (resp.status !== 401) return resp;
  notifyAuthRequired();
  const restored = await awaitAuthRestoration();
  if (!restored) return resp;
  return execute();
}

async function probeWebSocketAuth(): Promise<void> {
  const resp = await withAuthRetry(() =>
    fetch(new URL("/api/cli/detect", getApiBaseUrl()).toString(), {
      headers: applyAuthHeader({}),
    }),
  );

  if (resp.status === 401) {
    throw new Error("Authentication required");
  }

  if (!resp.ok) {
    throw new Error("WebSocket auth probe failed");
  }
}

/**
 * Mint a single-use WebSocket auth ticket from the server. The ticket is
 * embedded in the upgrade URL and consumed on first use, so even if it lands
 * in a reverse-proxy access log it can't be replayed (compared to passing
 * the long-lived token directly in the query string).
 *
 * Returns null when the server has no token configured (i.e. unauthenticated
 * deployment) — the caller can then connect without any query param.
 */
async function fetchWebSocketTicket(): Promise<string | null> {
  const resp = await withAuthRetry(() =>
    fetch(new URL("/api/auth/ws-ticket", getApiBaseUrl()).toString(), {
      method: "POST",
      headers: applyAuthHeader({ "Content-Type": "application/json" }),
      body: "{}",
    }),
  );

  if (resp.status === 401) {
    throw new Error("Authentication required");
  }
  if (resp.status === 404) {
    // Older server build without ticket support — caller should fall back to
    // the unauthenticated codepath.
    return null;
  }
  if (!resp.ok) {
    throw new Error(`Failed to obtain WebSocket ticket: ${resp.statusText}`);
  }

  const body = (await resp.json()) as { ticket?: string };
  return typeof body.ticket === "string" && body.ticket ? body.ticket : null;
}

export async function buildAuthenticatedWebSocketUrl(path: string): Promise<string> {
  const url = new URL(path, getApiBaseUrl());
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";

  // Only attempt to mint a ticket when a token is configured client-side.
  // Otherwise the server is in unauthenticated mode and a missing ticket
  // is fine.
  if (getToken()) {
    const ticket = await fetchWebSocketTicket();
    if (ticket) {
      url.searchParams.set("ticket", ticket);
    }
  }

  return url.toString();
}

async function apiFetch<T>(path: string, params?: Record<string, string>): Promise<T> {
  const url = new URL(path, getApiBaseUrl());
  if (params) {
    for (const [key, value] of Object.entries(params)) {
      url.searchParams.set(key, value);
    }
  }

  const resp = await withAuthRetry(() =>
    fetch(url.toString(), { headers: applyAuthHeader({}) }),
  );

  if (resp.status === 401) {
    throw new Error("Authentication required");
  }

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }

  return resp.json();
}

async function apiDelete<T>(path: string, params?: Record<string, string>): Promise<T> {
  const url = new URL(path, getApiBaseUrl());
  if (params) {
    for (const [key, value] of Object.entries(params)) {
      url.searchParams.set(key, value);
    }
  }

  const resp = await withAuthRetry(() =>
    fetch(url.toString(), { method: "DELETE", headers: applyAuthHeader({}) }),
  );

  if (resp.status === 401) {
    throw new Error("Authentication required");
  }

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }

  return resp.json();
}

export async function getProjects(source: string): Promise<ProjectEntry[]> {
  return apiFetch("/api/projects", { source });
}

export async function refreshProjectsCache(source: string): Promise<ProjectEntry[]> {
  return getProjects(source);
}

export async function rebuildProjectsCache(source: string): Promise<ProjectEntry[]> {
  return apiFetch("/api/projects", { source, rebuild: "true" });
}

export async function getSessions(
  source: string,
  projectId: string
): Promise<SessionIndexEntry[]> {
  return apiFetch("/api/sessions", { source, projectId });
}

export async function refreshSessionsCache(
  source: string,
  projectId: string
): Promise<SessionIndexEntry[]> {
  return getSessions(source, projectId);
}

export async function getInvalidSessions(
  source: string,
  projectId: string
): Promise<SessionIndexEntry[]> {
  return apiFetch("/api/sessions/invalid", { source, projectId });
}

export async function getMessages(
  source: string,
  filePath: string,
  page: number = 0,
  pageSize: number = 50,
  fromEnd: boolean = false
): Promise<PaginatedMessages> {
  return apiFetch("/api/messages", {
    source,
    filePath,
    page: String(page),
    pageSize: String(pageSize),
    fromEnd: String(fromEnd),
  });
}

/** Load `[start, end)` slice for the windowed (progressive) message view. */
export async function getMessagesRange(
  source: string,
  filePath: string,
  start: number,
  end: number,
): Promise<RangeMessages> {
  return apiFetch("/api/messages/range", {
    source,
    filePath,
    start: String(start),
    end: String(end),
  });
}

export async function getTrajectory(
  source: string,
  filePath: string,
  maxRecords: number = 500,
  beforeRecord?: number,
  fast: boolean = false,
): Promise<Trajectory> {
  return apiFetch("/api/trajectory", {
    source,
    filePath,
    maxRecords: String(maxRecords),
    ...(beforeRecord == null ? {} : { beforeRecord: String(beforeRecord) }),
    ...(fast ? { fast: "true" } : {}),
  });
}

export async function globalSearch(
  source: string,
  query: string,
  maxResults: number = 50,
  scope: string = "all",
): Promise<SearchResult[]> {
  return apiFetch("/api/search", { source, query, maxResults: String(maxResults), scope });
}

export async function getStats(source: string, timeZone: string): Promise<TokenUsageSummary> {
  return apiFetch("/api/stats", { source, timeZone });
}

export async function getRequestLog(
  source: string,
  filter: RequestLogFilter = {},
): Promise<RequestLogPage> {
  const params: Record<string, string> = { source };
  if (filter.projectId) params.projectId = filter.projectId;
  if (filter.sessionId) params.sessionId = filter.sessionId;
  if (filter.startDate) params.startDate = filter.startDate;
  if (filter.endDate) params.endDate = filter.endDate;
  if (filter.model) params.model = filter.model;
  if (filter.page !== undefined) params.page = String(filter.page);
  if (filter.pageSize !== undefined) params.pageSize = String(filter.pageSize);
  if (filter.timeZone) params.timeZone = filter.timeZone;
  return apiFetch("/api/stats/requests", params);
}

export async function getProjectCosts(source: string): Promise<ProjectCostEntry[]> {
  return apiFetch("/api/stats/projects", { source });
}

export async function getSessionCost(
  source: string,
  filePath: string,
): Promise<SessionCostSummary> {
  return apiFetch("/api/stats/session", { source, filePath });
}

export async function deleteSession(
  filePath: string,
  source?: string,
  projectId?: string,
  sessionId?: string
): Promise<void> {
  const params: Record<string, string> = { filePath };
  if (source) params.source = source;
  if (projectId) params.projectId = projectId;
  if (sessionId) params.sessionId = sessionId;
  await apiDelete("/api/sessions", params);
}

export async function deleteProject(
  source: string,
  projectId: string,
  level: DeleteLevel = "sessionOnly"
): Promise<DeleteResult> {
  return await apiDelete<DeleteResult>("/api/projects", { source, projectId, level });
}

export async function exportSession(
  source: string,
  filePath: string,
  format: ExportFormat
): Promise<string> {
  // 导出端点返回 text/plain 正文（非 JSON），单独处理。
  const url = new URL("/api/export", getApiBaseUrl());
  url.searchParams.set("source", source);
  url.searchParams.set("filePath", filePath);
  url.searchParams.set("format", format);

  const resp = await withAuthRetry(() =>
    fetch(url.toString(), { headers: applyAuthHeader({}) }),
  );

  if (resp.status === 401) {
    throw new Error("Authentication required");
  }
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }
  return resp.text();
}

// Web 模式不写本地文件——导出通过浏览器 Blob 下载完成。保留此导出仅为与
// tauriApi 的类型对齐（api.ts 把 webApi 结构性当作 tauriApi）；不应被调用。
export async function writeExportFile(_path: string, _content: string): Promise<void> {
  throw new Error("writeExportFile is not supported in web mode");
}

export async function getScanProgress(): Promise<ScanProgress> {
  return apiFetch<ScanProgress>("/api/scan-progress");
}

// Skills API
export async function listSkills(projectPath?: string | null): Promise<SkillsResult> {
  const params: Record<string, string> = {};
  if (projectPath) params.projectPath = projectPath;
  return apiFetch("/api/skills", params);
}

export async function getSkillContent(path: string): Promise<string> {
  // 内容端点返回 text/plain 正文（非 JSON），单独处理。
  const url = new URL("/api/skills/content", getApiBaseUrl());
  url.searchParams.set("path", path);
  const resp = await withAuthRetry(() =>
    fetch(url.toString(), { headers: applyAuthHeader({}) }),
  );
  if (resp.status === 401) {
    throw new Error("Authentication required");
  }
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }
  return resp.text();
}

export async function deleteSkill(
  scope: SkillScope,
  projectPath: string | null,
  slug: string,
): Promise<void> {
  const params: Record<string, string> = { scope, slug };
  if (projectPath) params.projectPath = projectPath;
  await apiDelete("/api/skills", params);
}

/** In web mode `archive` must be a File; its raw bytes are POSTed as the body. */
export async function importSkills(
  archive: File | string,
  scope: SkillScope,
  projectPath: string | null,
  overwrite: boolean,
): Promise<ImportResult> {
  if (typeof archive === "string") {
    throw new Error("Web 模式导入需要选择文件");
  }
  const url = new URL("/api/skills/import", getApiBaseUrl());
  url.searchParams.set("scope", scope);
  if (projectPath) url.searchParams.set("projectPath", projectPath);
  url.searchParams.set("overwrite", String(overwrite));
  url.searchParams.set("archiveName", archive.name);

  const resp = await withAuthRetry(() =>
    fetch(url.toString(), {
      method: "POST",
      headers: applyAuthHeader({ "Content-Type": "application/octet-stream" }),
      body: archive,
    }),
  );
  if (resp.status === 401) {
    throw new Error("Authentication required");
  }
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }
  return resp.json();
}

async function apiPut<T>(path: string, body: unknown): Promise<T> {
  const url = new URL(path, getApiBaseUrl()).toString();
  const payload = JSON.stringify(body);
  const resp = await withAuthRetry(() =>
    fetch(url, {
      method: "PUT",
      headers: applyAuthHeader({ "Content-Type": "application/json" }),
      body: payload,
    }),
  );

  if (resp.status === 401) {
    throw new Error("Authentication required");
  }

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }

  return resp.json();
}

export async function updateSessionMeta(
  source: string,
  projectId: string,
  sessionId: string,
  alias: string | null,
  tags: string[],
  filePath: string | null
): Promise<void> {
  await apiPut("/api/sessions/meta", { source, projectId, sessionId, alias, tags, filePath });
}

export async function renameChatSession(
  source: string,
  projectPath: string,
  sessionId: string,
  alias: string | null
): Promise<void> {
  await apiPost("/api/sessions/rename", { source, projectPath, sessionId, alias });
}

export async function getAllTags(
  source: string,
  projectId: string
): Promise<string[]> {
  return apiFetch("/api/tags", { source, projectId });
}

export async function getCrossProjectTags(
  source: string
): Promise<Record<string, string[]>> {
  return apiFetch("/api/cross-tags", { source });
}

export interface ForkResult {
  newSessionId: string;
  newFilePath: string;
  messageCount: number;
  firstPrompt: string | null;
}

export async function forkAndResume(
  _source: string,
  _originalFilePath: string,
  _userMsgUuid: string,
  _projectPath: string,
  _shell?: string
): Promise<ForkResult> {
  throw new Error("Fork is not available in web mode");
}

// Web mode: resume not available, use clipboard instead
export async function resumeSession(
  _source: string,
  _sessionId: string,
  _projectPath: string,
  _filePath?: string,
  _shell?: string
): Promise<void> {
  // No-op in web mode; handled by UI directly
}

export async function getInstallType(): Promise<"installed" | "portable"> {
  return "installed"; // Not applicable in web mode
}

async function apiPost<T>(path: string, body: unknown): Promise<T> {
  const url = new URL(path, getApiBaseUrl()).toString();
  const payload = JSON.stringify(body);
  const resp = await withAuthRetry(() =>
    fetch(url, {
      method: "POST",
      headers: applyAuthHeader({ "Content-Type": "application/json" }),
      body: payload,
    }),
  );

  if (resp.status === 401) {
    throw new Error("Authentication required");
  }

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }

  return resp.json();
}

// Provider Sync API
export async function providerSyncStatus(): Promise<ProviderSyncStatus> {
  return apiFetch("/api/provider-sync/status");
}

export async function providerSyncRun(
  provider: string | null,
  keep: number = 5,
): Promise<SyncResult> {
  return apiPost("/api/provider-sync/sync", { provider, keep });
}

export async function providerSyncSwitch(
  provider: string,
  keep: number = 5,
): Promise<SyncResult> {
  return apiPost("/api/provider-sync/switch", { provider, keep });
}

export async function providerSyncClone(
  filePaths: string[],
  provider: string,
  keep: number = 5,
): Promise<CloneResult> {
  return apiPost("/api/provider-sync/clone", { filePaths, provider, keep });
}

export async function providerSyncRestore(
  backupDir: string,
  options?: RestoreOptions,
): Promise<RestoreResult> {
  return apiPost("/api/provider-sync/restore", { backupDir, options });
}

export async function providerSyncPrune(keep: number = 5): Promise<number> {
  return apiPost(`/api/provider-sync/prune?keep=${keep}`, {});
}

// Chat API
export async function detectCli(): Promise<CliInstallation[]> {
  return apiFetch("/api/cli/detect");
}

export async function getCliConfig(source: string): Promise<CliConfig> {
  return apiFetch("/api/cli/config", { source });
}

export async function listModels(
  source: string,
  apiKey: string = "",
  baseUrl: string = ""
): Promise<ModelInfo[]> {
  return apiPost("/api/models", { source, apiKey, baseUrl });
}

// Chat WebSocket connection — managed externally by useChatStream
let chatWs: WebSocket | null = null;
const chatWsSubscribers = new Set<(rawMessage: string) => void>();
let chatWsOpenPromise: Promise<WebSocket> | null = null;

// Per-routing-id pending start/continue handshakes. The handshake completes
// when the server echoes `{ type: "session_id", sessionId: <routingId> }`,
// at which point we resolve with the real session id from `data` (which
// equals the routingId for Claude, and the codex thread_id for Codex).
type PendingChatHandshake = {
  resolve: (sessionId: string) => void;
  reject: (error: Error) => void;
};
const pendingChatStarts = new Map<string, PendingChatHandshake>();

function rejectAllPendingHandshakes(reason: string): void {
  if (pendingChatStarts.size === 0) return;
  const err = new Error(reason);
  for (const handshake of pendingChatStarts.values()) {
    handshake.reject(err);
  }
  pendingChatStarts.clear();
}

function dispatchHandshakeFrame(rawMessage: string): void {
  try {
    const data = JSON.parse(rawMessage);
    const type = typeof data?.type === "string" ? data.type : "";
    const routingId =
      typeof data?.sessionId === "string"
        ? data.sessionId
        : typeof data?.session_id === "string"
          ? data.session_id
          : null;

    if (type === "auth_required") {
      // The server hasn't tagged auth_required with a sessionId. We can't
      // tell which pending start this belongs to, so fail every outstanding
      // handshake and let each pane re-enter auth.
      notifyAuthRequired();
      rejectAllPendingHandshakes("Authentication required");
      return;
    }

    if (!routingId) return;

    const handshake = pendingChatStarts.get(routingId);
    if (!handshake) return;

    if (type === "session_id") {
      pendingChatStarts.delete(routingId);
      const resolved =
        typeof data.data === "string" && data.data ? data.data : routingId;
      handshake.resolve(resolved);
    } else if (type === "error") {
      pendingChatStarts.delete(routingId);
      const message =
        typeof data.data === "string" && data.data ? data.data : "Chat stream error";
      handshake.reject(new Error(message));
    }
  } catch {
    // not JSON; ignore
  }
}

function dispatchChatWsMessage(rawMessage: string): void {
  // Resolve pending start/continue handshakes first so callers see the
  // session id before any output frame fan-out to subscribers.
  dispatchHandshakeFrame(rawMessage);
  for (const subscriber of chatWsSubscribers) {
    subscriber(rawMessage);
  }
}

function attachChatWebSocketListeners(ws: WebSocket): void {
  ws.addEventListener("message", (event) => {
    if (typeof event.data === "string") {
      dispatchChatWsMessage(event.data);
    }
  });

  ws.addEventListener("close", () => {
    if (chatWs === ws) {
      chatWs = null;
      chatWsOpenPromise = null;
      rejectAllPendingHandshakes("WebSocket closed");
    }
  });

  ws.addEventListener("error", () => {
    if (chatWs === ws && ws.readyState !== WebSocket.OPEN) {
      chatWs = null;
      chatWsOpenPromise = null;
      rejectAllPendingHandshakes("WebSocket error");
    }
  });
}

async function openChatWebSocket(): Promise<WebSocket> {
  if (chatWs && chatWs.readyState === WebSocket.OPEN) {
    return chatWs;
  }

  if (chatWsOpenPromise) {
    return chatWsOpenPromise;
  }

  chatWsOpenPromise = (async () => {
    await probeWebSocketAuth();

    if (chatWs && chatWs.readyState === WebSocket.OPEN) {
      return chatWs;
    }

    const wsUrl = await buildAuthenticatedWebSocketUrl("/ws/chat");
    const ws = new WebSocket(wsUrl);
    attachChatWebSocketListeners(ws);
    chatWs = ws;

    await new Promise<void>((resolve, reject) => {
      const handleOpen = () => {
        cleanup();
        resolve();
      };

      const handleError = () => {
        cleanup();
        reject(new Error("WebSocket connection failed"));
      };

      const handleClose = () => {
        cleanup();
        reject(new Error("WebSocket connection closed before opening"));
      };

      const cleanup = () => {
        ws.removeEventListener("open", handleOpen);
        ws.removeEventListener("error", handleError);
        ws.removeEventListener("close", handleClose);
      };

      ws.addEventListener("open", handleOpen, { once: true });
      ws.addEventListener("error", handleError, { once: true });
      ws.addEventListener("close", handleClose, { once: true });
    });

    return ws;
  })();

  try {
    return await chatWsOpenPromise;
  } catch (error) {
    chatWsOpenPromise = null;
    throw error;
  }
}

export function subscribeToChatWebSocketMessages(listener: (rawMessage: string) => void): () => void {
  chatWsSubscribers.add(listener);

  return () => {
    chatWsSubscribers.delete(listener);
  };
}

export async function connectFileWatcherWebSocket(): Promise<WebSocket> {
  await probeWebSocketAuth();
  const url = await buildAuthenticatedWebSocketUrl("/ws");
  return new WebSocket(url);
}

export function closeChatWebSocket(): void {
  if (chatWs) {
    chatWs.close();
    chatWs = null;
  }
  chatWsOpenPromise = null;
  rejectAllPendingHandshakes("WebSocket closed");
}

function generateRoutingId(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return "10000000-1000-4000-8000-100000000000".replace(/[018]/g, (c) =>
    (
      Number(c) ^
      (crypto.getRandomValues(new Uint8Array(1))[0] & (15 >> (Number(c) / 4)))
    ).toString(16)
  );
}

export async function startChat(params: StartChatParams): Promise<string> {
  const ws = await openChatWebSocket();
  const routingId = params.sessionId || generateRoutingId();

  return new Promise<string>((resolve, reject) => {
    if (pendingChatStarts.has(routingId)) {
      // Two concurrent starts for the same routing id would race; the caller
      // should never do this, but fail loudly rather than silently overwrite.
      reject(new Error(`Duplicate chat start for sessionId ${routingId}`));
      return;
    }
    pendingChatStarts.set(routingId, { resolve, reject });

    try {
      ws.send(
        JSON.stringify({
          action: "start",
          source: params.source,
          sessionId: routingId,
          projectPath: params.projectPath,
          prompt: params.prompt,
          model: params.model,
          skipPermissions: params.skipPermissions,
          apiKey: params.apiKey || "",
          baseUrl: params.baseUrl || "",
        })
      );
    } catch (err) {
      pendingChatStarts.delete(routingId);
      reject(err instanceof Error ? err : new Error(String(err)));
    }
  });
}

export async function continueChat(params: ContinueChatParams): Promise<string> {
  const ws = await openChatWebSocket();
  const routingId = params.sessionId;
  if (!routingId) {
    throw new Error("continueChat requires a sessionId");
  }

  return new Promise<string>((resolve, reject) => {
    if (pendingChatStarts.has(routingId)) {
      reject(new Error(`Duplicate chat continue for sessionId ${routingId}`));
      return;
    }
    pendingChatStarts.set(routingId, { resolve, reject });

    try {
      ws.send(
        JSON.stringify({
          action: "continue",
          source: params.source,
          sessionId: routingId,
          projectPath: params.projectPath,
          prompt: params.prompt,
          model: params.model,
          skipPermissions: params.skipPermissions,
          apiKey: params.apiKey || "",
          baseUrl: params.baseUrl || "",
        })
      );
    } catch (err) {
      pendingChatStarts.delete(routingId);
      reject(err instanceof Error ? err : new Error(String(err)));
    }
  });
}

export async function cancelChat(sessionId: string): Promise<void> {
  if (!sessionId) return;
  if (chatWs && chatWs.readyState === WebSocket.OPEN) {
    chatWs.send(JSON.stringify({ action: "cancel", sessionId }));
  }
}

// Bookmarks API
export async function listBookmarks(source?: string): Promise<Bookmark[]> {
  const params: Record<string, string> = {};
  if (source) params.source = source;
  return apiFetch("/api/bookmarks", params);
}

export async function addBookmark(bookmark: Omit<Bookmark, "id" | "createdAt"> & { id?: string; createdAt?: string }): Promise<Bookmark> {
  return apiPost("/api/bookmarks", { id: "", createdAt: "", ...bookmark });
}

export async function removeBookmark(id: string): Promise<void> {
  await apiDelete(`/api/bookmarks/${encodeURIComponent(id)}`);
}

export async function setProjectAlias(
  source: string,
  projectId: string,
  alias: string | null
): Promise<void> {
  await apiPut("/api/projects/alias", { source, projectId, alias });
}

// Recyclebin API
export async function listRecycledItems(): Promise<RecycledItem[]> {
  return apiFetch("/api/recyclebin");
}

export async function restoreRecycledItem(id: string): Promise<void> {
  await apiPost(`/api/recyclebin/${encodeURIComponent(id)}/restore`, {});
}

export async function permanentlyDeleteRecycledItem(id: string): Promise<void> {
  const url = new URL(
    `/api/recyclebin/${encodeURIComponent(id)}`,
    getApiBaseUrl(),
  ).toString();
  const resp = await withAuthRetry(() =>
    fetch(url, { method: "DELETE", headers: applyAuthHeader({}) }),
  );
  if (resp.status === 401) {
    throw new Error("Authentication required");
  }
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }
}

export async function emptyRecyclebin(): Promise<number> {
  const result = await apiPost<{ deleted: number }>("/api/recyclebin/empty", {});
  return result.deleted;
}

export async function cleanupOrphanDirs(source: string): Promise<number> {
  const result = await apiPost<{ deleted: number }>(
    `/api/recyclebin/cleanup-orphans?source=${encodeURIComponent(source)}`,
    {},
  );
  return result.deleted;
}
