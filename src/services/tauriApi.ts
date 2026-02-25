import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectEntry,
  SessionIndexEntry,
  PaginatedMessages,
  SearchResult,
  TokenUsageSummary,
} from "../types";
import type { CliInstallation, ModelInfo, StartChatParams, ContinueChatParams, CliConfig, QuickChatMessage } from "../types/chat";

export async function getProjects(source: string): Promise<ProjectEntry[]> {
  return invoke<ProjectEntry[]>("get_projects", { source });
}

export async function getSessions(
  source: string,
  projectId: string
): Promise<SessionIndexEntry[]> {
  return invoke<SessionIndexEntry[]>("get_sessions", { source, projectId });
}

export async function getMessages(
  source: string,
  filePath: string,
  page: number = 0,
  pageSize: number = 50,
  fromEnd: boolean = false
): Promise<PaginatedMessages> {
  return invoke<PaginatedMessages>("get_messages", {
    source,
    filePath,
    page,
    pageSize,
    fromEnd,
  });
}

export async function globalSearch(
  source: string,
  query: string,
  maxResults: number = 50
): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("global_search", { source, query, maxResults });
}

export async function getStats(source: string): Promise<TokenUsageSummary> {
  return invoke<TokenUsageSummary>("get_stats", { source });
}

export async function deleteSession(
  filePath: string,
  source?: string,
  projectId?: string,
  sessionId?: string
): Promise<void> {
  return invoke<void>("delete_session", {
    filePath,
    source: source || "",
    projectId: projectId || "",
    sessionId: sessionId || "",
  });
}

export async function updateSessionMeta(
  source: string,
  projectId: string,
  sessionId: string,
  alias: string | null,
  tags: string[]
): Promise<void> {
  return invoke<void>("update_session_meta", {
    source,
    projectId,
    sessionId,
    alias,
    tags,
  });
}

export async function getAllTags(
  source: string,
  projectId: string
): Promise<string[]> {
  return invoke<string[]>("get_all_tags", { source, projectId });
}

export async function getCrossProjectTags(
  source: string
): Promise<Record<string, string[]>> {
  return invoke<Record<string, string[]>>("get_cross_project_tags", { source });
}

export async function resumeSession(
  source: string,
  sessionId: string,
  projectPath: string,
  filePath?: string
): Promise<void> {
  return invoke<void>("resume_session", { source, sessionId, projectPath, filePath });
}

export async function getInstallType(): Promise<"installed" | "portable"> {
  return invoke<"installed" | "portable">("get_install_type");
}

// Chat API
export async function detectCli(): Promise<CliInstallation[]> {
  return invoke<CliInstallation[]>("detect_cli");
}

export async function getCliConfig(source: string): Promise<CliConfig> {
  return invoke<CliConfig>("get_cli_config", { source });
}

export async function startQuickChat(
  source: string,
  messages: QuickChatMessage[],
  model: string,
  onChunk: (text: string) => void,
  onError: (err: string) => void,
  onDone: () => void,
): Promise<() => void> {
  // Invoke the quick_chat command — it streams via Tauri events
  invoke("quick_chat", { source, messages, model }).catch((e) => {
    onError(String(e));
  });

  // We need to listen for the events. The session ID is not known in advance,
  // so we use a global listener pattern based on event prefix.
  const { listen } = await import("@tauri-apps/api/event");

  let cancelled = false;
  const cleanups: (() => void)[] = [];

  // Listen for chunk events on any quick-chat-chunk:* channel
  // Since we don't know the session ID, use a global event listener
  const unlistenChunk = await listen<string>("quick-chat-chunk", (event) => {
    if (!cancelled) onChunk(event.payload);
  });
  cleanups.push(unlistenChunk);

  const unlistenError = await listen<string>("quick-chat-error", (event) => {
    if (!cancelled) onError(event.payload);
  });
  cleanups.push(unlistenError);

  const unlistenDone = await listen<string>("quick-chat-done", () => {
    if (!cancelled) onDone();
  });
  cleanups.push(unlistenDone);

  return () => {
    cancelled = true;
    cleanups.forEach((fn) => fn());
  };
}

export async function listModels(
  source: string,
  apiKey: string = "",
  baseUrl: string = ""
): Promise<ModelInfo[]> {
  return invoke<ModelInfo[]>("list_models", { source, apiKey, baseUrl });
}

export async function startChat(params: StartChatParams): Promise<string> {
  return invoke<string>("start_chat", { ...params });
}

export async function continueChat(params: ContinueChatParams): Promise<string> {
  return invoke<string>("continue_chat", { ...params });
}

export async function cancelChat(sessionId: string): Promise<void> {
  return invoke<void>("cancel_chat", { sessionId });
}
