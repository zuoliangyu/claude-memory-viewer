import type {
  ProjectEntry,
  SessionIndexEntry,
  PaginatedMessages,
  SearchResult,
  TokenUsageSummary,
} from "../types";
import type { CliInstallation, ModelInfo, StartChatParams, ContinueChatParams, CliConfig, QuickChatMessage } from "../types/chat";

function getToken(): string | null {
  return localStorage.getItem("asv_token");
}

async function apiFetch<T>(path: string, params?: Record<string, string>): Promise<T> {
  const url = new URL(path, window.location.origin);
  if (params) {
    for (const [key, value] of Object.entries(params)) {
      url.searchParams.set(key, value);
    }
  }

  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const resp = await fetch(url.toString(), { headers });

  if (resp.status === 401) {
    // Trigger auth prompt
    window.dispatchEvent(new CustomEvent("asv-auth-required"));
    throw new Error("Authentication required");
  }

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }

  return resp.json();
}

async function apiDelete<T>(path: string, params?: Record<string, string>): Promise<T> {
  const url = new URL(path, window.location.origin);
  if (params) {
    for (const [key, value] of Object.entries(params)) {
      url.searchParams.set(key, value);
    }
  }

  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const resp = await fetch(url.toString(), { method: "DELETE", headers });

  if (resp.status === 401) {
    window.dispatchEvent(new CustomEvent("asv-auth-required"));
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

export async function getSessions(
  source: string,
  projectId: string
): Promise<SessionIndexEntry[]> {
  return apiFetch("/api/sessions", { source, projectId });
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

export async function globalSearch(
  source: string,
  query: string,
  maxResults: number = 50
): Promise<SearchResult[]> {
  return apiFetch("/api/search", { source, query, maxResults: String(maxResults) });
}

export async function getStats(source: string): Promise<TokenUsageSummary> {
  return apiFetch("/api/stats", { source });
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

async function apiPut<T>(path: string, body: unknown): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const resp = await fetch(new URL(path, window.location.origin).toString(), {
    method: "PUT",
    headers,
    body: JSON.stringify(body),
  });

  if (resp.status === 401) {
    window.dispatchEvent(new CustomEvent("asv-auth-required"));
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
  tags: string[]
): Promise<void> {
  await apiPut("/api/sessions/meta", { source, projectId, sessionId, alias, tags });
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

// Web mode: resume not available, use clipboard instead
export async function resumeSession(
  _source: string,
  _sessionId: string,
  _projectPath: string,
  _filePath?: string
): Promise<void> {
  // No-op in web mode; handled by UI directly
}

export async function getInstallType(): Promise<"installed" | "portable"> {
  return "installed"; // Not applicable in web mode
}

async function apiPost<T>(path: string, body: unknown): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const resp = await fetch(new URL(path, window.location.origin).toString(), {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });

  if (resp.status === 401) {
    window.dispatchEvent(new CustomEvent("asv-auth-required"));
    throw new Error("Authentication required");
  }

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(text || resp.statusText);
  }

  return resp.json();
}

// Chat API
export async function detectCli(): Promise<CliInstallation[]> {
  return apiFetch("/api/cli/detect");
}

export async function getCliConfig(source: string): Promise<CliConfig> {
  return apiFetch("/api/cli/config", { source });
}

export async function startQuickChat(
  source: string,
  messages: QuickChatMessage[],
  model: string,
  onChunk: (text: string) => void,
  onError: (err: string) => void,
  onDone: () => void,
): Promise<() => void> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const resp = await fetch(new URL("/api/quick-chat", window.location.origin).toString(), {
    method: "POST",
    headers,
    body: JSON.stringify({ source, messages, model }),
  });

  if (!resp.ok) {
    const text = await resp.text();
    onError(text || resp.statusText);
    onDone();
    return () => {};
  }

  const reader = resp.body?.getReader();
  if (!reader) {
    onError("No response body");
    onDone();
    return () => {};
  }

  let cancelled = false;
  const decoder = new TextDecoder();
  let buffer = "";

  const readLoop = async () => {
    try {
      while (!cancelled) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed || !trimmed.startsWith("data:")) continue;
          const data = trimmed.slice(5).trim();
          if (data === "[DONE]") {
            onDone();
            return;
          }
          onChunk(data);
        }
      }
    } catch (e) {
      if (!cancelled) {
        onError(String(e));
      }
    }
    if (!cancelled) onDone();
  };

  readLoop();

  return () => {
    cancelled = true;
    reader.cancel().catch(() => {});
  };
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
let chatWsResolve: ((sessionId: string) => void) | null = null;

export function getChatWebSocket(): WebSocket {
  if (chatWs && chatWs.readyState === WebSocket.OPEN) {
    return chatWs;
  }

  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const wsUrl = `${protocol}//${window.location.host}/ws/chat`;
  chatWs = new WebSocket(wsUrl);
  return chatWs;
}

export function closeChatWebSocket(): void {
  if (chatWs) {
    chatWs.close();
    chatWs = null;
  }
}

export async function startChat(params: StartChatParams): Promise<string> {
  const ws = getChatWebSocket();

  return new Promise((resolve, reject) => {
    const onOpen = () => {
      ws.send(
        JSON.stringify({
          action: "start",
          source: params.source,
          projectPath: params.projectPath,
          prompt: params.prompt,
          model: params.model,
          skipPermissions: params.skipPermissions,
        })
      );
    };

    chatWsResolve = resolve;

    const onMessage = (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data);
        if (data.type === "session_id") {
          ws.removeEventListener("message", onMessage);
          if (chatWsResolve) {
            chatWsResolve(data.data);
            chatWsResolve = null;
          }
        }
      } catch {
        // not JSON, ignore
      }
    };

    ws.addEventListener("message", onMessage);

    if (ws.readyState === WebSocket.OPEN) {
      onOpen();
    } else {
      ws.addEventListener("open", onOpen, { once: true });
      ws.addEventListener(
        "error",
        () => reject(new Error("WebSocket connection failed")),
        { once: true }
      );
    }
  });
}

export async function continueChat(params: ContinueChatParams): Promise<string> {
  const ws = getChatWebSocket();

  return new Promise((resolve, reject) => {
    const sendMsg = () => {
      ws.send(
        JSON.stringify({
          action: "continue",
          source: params.source,
          sessionId: params.sessionId,
          projectPath: params.projectPath,
          prompt: params.prompt,
          model: params.model,
          skipPermissions: params.skipPermissions,
        })
      );
    };

    chatWsResolve = resolve;

    const onMessage = (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data);
        if (data.type === "session_id") {
          ws.removeEventListener("message", onMessage);
          if (chatWsResolve) {
            chatWsResolve(data.data);
            chatWsResolve = null;
          }
        }
      } catch {
        // ignore
      }
    };

    ws.addEventListener("message", onMessage);

    if (ws.readyState === WebSocket.OPEN) {
      sendMsg();
    } else {
      ws.addEventListener("open", sendMsg, { once: true });
      ws.addEventListener(
        "error",
        () => reject(new Error("WebSocket connection failed")),
        { once: true }
      );
    }
  });
}

export async function cancelChat(_sessionId: string): Promise<void> {
  if (chatWs && chatWs.readyState === WebSocket.OPEN) {
    chatWs.send(JSON.stringify({ action: "cancel" }));
  }
}
