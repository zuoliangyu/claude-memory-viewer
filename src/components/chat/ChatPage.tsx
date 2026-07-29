import { useEffect, useLayoutEffect, useMemo, useRef, useCallback, useState } from "react";
import { useParams } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { DEFAULT_CHAT_PANE_ID, useChatStore } from "../../stores/chatStore";
import { useAppStore } from "../../stores/appStore";
import { ChatHeader } from "./ChatHeader";
import { ChatInput } from "./ChatInput";
import { StreamingMessage, getLinkedToolUseIds } from "./StreamingMessage";
import { FolderSelector } from "./FolderSelector";
import { MessageSquarePlus, AlertCircle, Bot } from "lucide-react";
import type { ChatMessage } from "../../types/chat";
import { ExpandAllProvider } from "../common/ExpandAllContext";
import { useReplyNotification } from "../../hooks/useReplyNotification";
import { normalizeToolName } from "./tool-viewers/ToolViewers";
import { ScrollArea } from "../ScrollArea";
import { subscribeToChatWebSocketMessages } from "../../services/webApi";
import { isActualChatError } from "./chatError";

declare const __IS_TAURI__: boolean;

/** Enable virtual scrolling when there are more turns than this */
const VIRTUAL_THRESHOLD = 30;

interface Turn {
  turnIndex: number;
  displayMessages: ChatMessage[];
  tokens: number;
}

interface ToolResultSummary {
  content: string;
  isError: boolean;
}

interface LatestAssistantMessagePreview {
  key: string;
  preview: string;
}

interface ChatDerivedState {
  toolResultMap: Map<string, ToolResultSummary>;
  turns: Turn[];
  linkedToolUseIds: Set<string>;
  latestAssistantMessage: LatestAssistantMessagePreview | null;
}

function usePaneChatStream(paneId: string, streamIdOverride: string | null) {
  const paneStreamId = useChatStore(
    useCallback((state) => state.panes[paneId]?.streamId ?? null, [paneId])
  );
  // Prefer the pane's live streamId; fall back to the URL-derived override
  // so a freshly navigated pane can still receive its first frames.
  const targetStreamId = paneStreamId ?? streamIdOverride;
  const cleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    if (!targetStreamId) return;

    if (__IS_TAURI__) {
      const { addStreamLineToPane, setPaneStreaming, setPaneError } = useChatStore.getState();
      let cancelled = false;

      const setupListeners = async () => {
        const { listen } = await import("@tauri-apps/api/event");
        if (cancelled) return;

        const unlistenOutput = await listen<string>(
          `chat-output:${targetStreamId}`,
          (event) => {
            if (!cancelled) {
              addStreamLineToPane(paneId, event.payload);
            }
          }
        );
        if (cancelled) {
          unlistenOutput();
          return;
        }

        const unlistenError = await listen<string>(
          `chat-error:${targetStreamId}`,
          (event) => {
            if (!cancelled && isActualChatError(event.payload)) {
              setPaneError(paneId, event.payload);
            }
          }
        );
        if (cancelled) {
          unlistenOutput();
          unlistenError();
          return;
        }

        const unlistenComplete = await listen<string>(
          `chat-complete:${targetStreamId}`,
          () => {
            if (!cancelled) {
              setPaneStreaming(paneId, false);
            }
          }
        );
        if (cancelled) {
          unlistenOutput();
          unlistenError();
          unlistenComplete();
          return;
        }

        cleanupRef.current = () => {
          unlistenOutput();
          unlistenError();
          unlistenComplete();
        };
      };

      setupListeners();

      return () => {
        cancelled = true;
        if (cleanupRef.current) {
          cleanupRef.current();
          cleanupRef.current = null;
        }
      };
    }

    const unsubscribe = subscribeToChatWebSocketMessages((rawMessage) => {
      const { addStreamLineToPane, setPaneStreaming, setPaneError } = useChatStore.getState();

      try {
        const data = JSON.parse(rawMessage);
        const type = typeof data?.type === "string" ? data.type : "";
        const payload =
          typeof data?.data === "string"
            ? data.data
            : typeof data?.payload === "string"
              ? data.payload
              : "";
        const eventSessionId =
          typeof data?.sessionId === "string"
            ? data.sessionId
            : typeof data?.session_id === "string"
              ? data.session_id
              : null;

        // Strict per-stream routing: untagged frames or other panes' frames
        // are dropped here so multiple chat panes on the same socket can run
        // concurrently without crosstalk.
        if (!eventSessionId || eventSessionId !== targetStreamId) {
          return;
        }

        if (type === "output" || type === "chunk") {
          if (payload) {
            addStreamLineToPane(paneId, payload);
          }
        } else if (type === "error" || type === "auth_required") {
          if (type === "auth_required") {
            setPaneStreaming(paneId, false);
            window.dispatchEvent(new CustomEvent("asv-auth-required"));
          }
          if (payload && isActualChatError(payload)) {
            setPaneError(paneId, payload);
          }
        } else if (type === "complete" || type === "done") {
          setPaneStreaming(paneId, false);
        }
      } catch {
        // ignore non-JSON messages
      }
    });

    return () => {
      unsubscribe();
    };
  }, [paneId, targetStreamId]);
}

function getNormalizedCommandInput(input: string): string {
  try {
    const parsed = JSON.parse(input) as { command?: unknown };
    if (typeof parsed?.command === "string") {
      return parsed.command.trim();
    }
  } catch {
    // Some command events already provide plain text input.
  }

  return input.trim();
}

function getDedupableAssistantSignature(
  message: ChatMessage,
  toolResultMap: Map<string, { content: string; isError: boolean }>
): string | null {
  if (message.role !== "assistant" || message.content.length === 0) {
    return null;
  }

  const parts: string[] = [];
  for (const block of message.content) {
    if (block.type === "tool_use") {
      if (normalizeToolName(block.name) !== "Bash") {
        return null;
      }

      const linkedResult = toolResultMap.get(block.id);
      parts.push([
        "tool",
        "bash",
        getNormalizedCommandInput(block.input),
        linkedResult?.isError ? "1" : "0",
        linkedResult?.content ?? "",
      ].join(":"));
      continue;
    }

    if (block.type === "tool_result") {
      parts.push([
        "result",
        block.toolUseId,
        block.isError ? "1" : "0",
        block.content,
      ].join(":"));
      continue;
    }

    return null;
  }

  return parts.length > 0 ? parts.join("\n") : null;
}

function dedupeTurnMessages(
  messages: ChatMessage[],
  toolResultMap: Map<string, { content: string; isError: boolean }>
): ChatMessage[] {
  const deduped: ChatMessage[] = [];
  let previousSignature: string | null = null;

  for (const message of messages) {
    const signature = getDedupableAssistantSignature(message, toolResultMap);
    if (signature && signature === previousSignature) {
      continue;
    }

    deduped.push(message);
    previousSignature = signature;
  }

  return deduped;
}

function buildChatDerivedState(messages: ChatMessage[]): ChatDerivedState {
  const toolResultMap = new Map<string, ToolResultSummary>();
  const rawTurns: Array<{ turnIndex: number; messages: ChatMessage[]; tokens: number }> = [];
  const linkedToolUseIds = new Set<string>();
  let currentMessages: ChatMessage[] = [];
  let turnIndex = 0;
  let tokens = 0;
  let latestAssistantMessage: LatestAssistantMessagePreview | null = null;

  for (const message of messages) {
    for (const block of message.content) {
      if (block.type === "tool_result") {
        toolResultMap.set(block.toolUseId, {
          content: block.content,
          isError: block.isError,
        });
      }
    }

    if (message.role === "assistant") {
      const text = message.content.find((block) => block.type === "text");
      latestAssistantMessage = {
        key: `${message.id}:${message.timestamp}`,
        preview: text && "text" in text ? text.text.slice(0, 80) : "有新回复",
      };
    }

    const isUserText = message.role === "user" && message.content.some((block) => block.type === "text");
    if (isUserText && currentMessages.length > 0) {
      rawTurns.push({ turnIndex, messages: currentMessages, tokens });
      turnIndex += 1;
      currentMessages = [];
      tokens = 0;
    }

    currentMessages.push(message);
    if (message.usage) {
      tokens += message.usage.inputTokens + message.usage.outputTokens;
    }
  }

  if (currentMessages.length > 0) {
    rawTurns.push({ turnIndex, messages: currentMessages, tokens });
  }

  const turns = rawTurns.map((turn) => {
    const displayMessages = dedupeTurnMessages(turn.messages, toolResultMap);
    for (const message of displayMessages) {
      for (const toolUseId of getLinkedToolUseIds(message, toolResultMap)) {
        linkedToolUseIds.add(toolUseId);
      }
    }
    return {
      turnIndex: turn.turnIndex,
      displayMessages,
      tokens: turn.tokens,
    };
  });

  return {
    toolResultMap,
    turns,
    linkedToolUseIds,
    latestAssistantMessage,
  };
}

interface ChatPageProps {
  paneId?: string;
}

export function ChatPage({ paneId = DEFAULT_CHAT_PANE_ID }: ChatPageProps) {
  const { sessionId: urlSessionId } = useParams<{ sessionId?: string }>();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const pane = useChatStore((state) => state.getPaneState(paneId));
  const availableClis = useChatStore((state) => state.availableClis);
  const detectCli = useChatStore((state) => state.detectCli);
  const fetchCliConfig = useChatStore((state) => state.fetchCliConfig);
  const fetchCodexCliConfig = useChatStore((state) => state.fetchCodexCliConfig);
  const fetchModelList = useChatStore((state) => state.fetchModelList);
  const setActivePane = useChatStore((state) => state.setActivePane);
  const startNewChatInPane = useChatStore((state) => state.startNewChatInPane);
  const continueExistingChatInPane = useChatStore((state) => state.continueExistingChatInPane);
  const cancelPane = useChatStore((state) => state.cancelPane);
  const clearPane = useChatStore((state) => state.clearPane);
  const setPaneProjectPath = useChatStore((state) => state.setPaneProjectPath);
  const setPaneSessionId = useChatStore((state) => state.setPaneSessionId);
  const setPaneSource = useChatStore((state) => state.setPaneSource);
  const [expandVersion, setExpandVersion] = useState(0);
  const [allExpanded, setAllExpanded] = useState(true);
  const { isActive, sessionId, projectPath, model, messages, isStreaming, error, source } = pane;

  const appSource = useAppStore((s) => s.source);
  const cliLabel = source === "codex" ? "Codex" : "Claude";
  const activeSessionId = urlSessionId ?? sessionId ?? null;

  useEffect(() => {
    setActivePane(paneId);
  }, [paneId, setActivePane]);

  // Sync source from appStore into the target pane
  useEffect(() => {
    if (appSource !== "grok") {
      setPaneSource(paneId, appSource);
    }
  }, [appSource, paneId, setPaneSource]);

  // Detect CLI on mount + fetch config & model list
  useEffect(() => { detectCli(); }, [detectCli]);
  useLayoutEffect(() => {
    if (urlSessionId) {
      clearPane(paneId);
      setPaneSessionId(paneId, urlSessionId);
      return;
    }
    clearPane(paneId);
  }, [clearPane, paneId, setPaneSessionId, urlSessionId]);
  useEffect(() => {
    if (appSource === "grok") return;
    if (appSource === "codex") {
      fetchCodexCliConfig();
      return;
    }
    fetchCliConfig();
  }, [appSource, fetchCliConfig, fetchCodexCliConfig]);
  // Re-fetch model list whenever source changes (setSource clears modelList first)
  useEffect(() => {
    if (appSource !== "grok") fetchModelList();
  }, [appSource, fetchModelList]);

  // Listen for stream events in the target pane. The hook falls back to the
  // URL session id only until a live turn sets the pane's own streamId.
  usePaneChatStream(paneId, urlSessionId ?? null);

  const handleSend = useCallback((prompt: string) => {
    if (!projectPath) return;
    if (activeSessionId) {
      continueExistingChatInPane(paneId, activeSessionId, projectPath, prompt, model);
    } else {
      startNewChatInPane(paneId, projectPath, prompt, model);
    }
  }, [activeSessionId, continueExistingChatInPane, model, paneId, projectPath, startNewChatInPane]);
  const handleExpandAll = useCallback(() => {
    setAllExpanded(true);
    setExpandVersion((v) => v + 1);
  }, []);
  const handleCollapseAll = useCallback(() => {
    setAllExpanded(false);
    setExpandVersion((v) => v + 1);
  }, []);
  const handleProjectPathChange = useCallback((path: string) => {
    setPaneProjectPath(paneId, path);
  }, [paneId, setPaneProjectPath]);
  const handleCancel = useCallback(() => {
    void cancelPane(paneId);
  }, [cancelPane, paneId]);

  const cliAvailable = availableClis.some((c) => c.cliType === source);

  const { toolResultMap, turns, linkedToolUseIds, latestAssistantMessage } = useMemo(
    () => buildChatDerivedState(messages),
    [messages]
  );

  const useVirtual = turns.length > VIRTUAL_THRESHOLD;

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    // Scroll to bottom
    el.scrollTop = el.scrollHeight;
  }, [messages.length, isStreaming]);

  useReplyNotification(
    latestAssistantMessage?.key ?? null,
    `${cliLabel} 有新回复`,
    latestAssistantMessage?.preview || "点击查看最新对话"
  );

  const handleSubmitAnswers = useCallback(async (answers: string) => {
    const trimmedAnswers = answers.trim();
    if (!trimmedAnswers || !projectPath || !activeSessionId) return;

    if (isStreaming) {
      await cancelPane(paneId);
      setTimeout(() => {
        continueExistingChatInPane(paneId, activeSessionId, projectPath, trimmedAnswers, model);
      }, 150);
      return;
    }
    continueExistingChatInPane(paneId, activeSessionId, projectPath, trimmedAnswers, model);
  }, [activeSessionId, cancelPane, continueExistingChatInPane, isStreaming, model, paneId, projectPath]);

  return (
    <div className="flex flex-col h-full">
      <ChatHeader
        paneId={paneId}
        onExpandAll={handleExpandAll}
        onCollapseAll={handleCollapseAll}
      />

      <ScrollArea
        className="flex-1 min-h-0"
        viewportRef={scrollContainerRef}
        viewportClassName="h-full"
      >
        <ExpandAllProvider value={{ expanded: allExpanded, version: expandVersion }}>
          {!isActive && messages.length === 0 ? (
          <EmptyState
            projectPath={projectPath}
            onProjectPathChange={handleProjectPathChange}
            cliAvailable={cliAvailable}
            cliLabel={cliLabel}
          />
        ) : useVirtual ? (
          <VirtualizedTurns
            turns={turns}
            toolResultMap={toolResultMap}
            linkedToolUseIds={linkedToolUseIds}
            isStreaming={isStreaming}
            error={error}
            scrollContainer={scrollContainerRef}
            onSubmitAnswers={handleSubmitAnswers}
          />
        ) : (
          <div className="max-w-4xl mx-auto px-4 py-4">
            {turns.map((turn) => (
              <TurnBlock
                key={turn.turnIndex}
                turn={turn}
                toolResultMap={toolResultMap}
                linkedToolUseIds={linkedToolUseIds}
                onSubmitAnswers={handleSubmitAnswers}
              />
            ))}
            <StreamingAndError isStreaming={isStreaming} error={error} />
          </div>
        )}
        </ExpandAllProvider>
      </ScrollArea>

      <div className="max-w-4xl mx-auto w-full">
        <ChatInput
          paneId={paneId}
          onSend={handleSend}
          onCancel={handleCancel}
          isStreaming={isStreaming}
          disabled={!projectPath || !cliAvailable}
        />
      </div>
    </div>
  );
}

/* ── Virtualized turn list ─────────────────────────── */

function VirtualizedTurns({
  turns,
  toolResultMap,
  linkedToolUseIds,
  isStreaming,
  error,
  scrollContainer,
  onSubmitAnswers,
}: {
  turns: Turn[];
  toolResultMap: Map<string, { content: string; isError: boolean }>;
  linkedToolUseIds: Set<string>;
  isStreaming: boolean;
  error: string | null;
  scrollContainer: React.RefObject<HTMLDivElement | null>;
  onSubmitAnswers: (answers: string) => void;
}) {
  // +1 for the streaming/error footer row
  const count = turns.length + 1;

  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => scrollContainer.current,
    estimateSize: () => 120,
    overscan: 5,
  });

  return (
    <div className="max-w-4xl mx-auto px-4 py-4">
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((item) => {
          if (item.index === turns.length) {
            // Footer: streaming indicator + error
            return (
              <div
                key="footer"
                ref={virtualizer.measureElement}
                data-index={item.index}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${item.start}px)`,
                }}
              >
                <StreamingAndError isStreaming={isStreaming} error={error} />
              </div>
            );
          }

          const turn = turns[item.index];
          return (
            <div
              key={turn.turnIndex}
              ref={virtualizer.measureElement}
              data-index={item.index}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${item.start}px)`,
              }}
            >
              <TurnBlock
                turn={turn}
                toolResultMap={toolResultMap}
                linkedToolUseIds={linkedToolUseIds}
                onSubmitAnswers={onSubmitAnswers}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}

/* ── Single turn ───────────────────────────────────── */

function TurnBlock({
  turn,
  toolResultMap,
  linkedToolUseIds,
  onSubmitAnswers,
}: {
  turn: Turn;
  toolResultMap: Map<string, { content: string; isError: boolean }>;
  linkedToolUseIds: Set<string>;
  onSubmitAnswers: (answers: string) => void;
}) {
  return (
    <div>
      {turn.turnIndex > 0 && (
        <div className="flex items-center gap-3 my-4">
          <div className="flex-1 border-t border-border/50" />
          <span className="text-[10px] text-muted-foreground/60 tabular-nums">
            Turn {turn.turnIndex + 1}
            {turn.tokens > 0 && ` · ${turn.tokens.toLocaleString()} tokens`}
          </span>
          <div className="flex-1 border-t border-border/50" />
        </div>
      )}
      <div className="space-y-1">
        {turn.displayMessages.map((msg) => (
          <StreamingMessage
            key={msg.id}
            message={msg}
            toolResultMap={toolResultMap}
            linkedToolUseIds={linkedToolUseIds}
            onSubmitAnswers={onSubmitAnswers}
            interactiveQuestions
          />
        ))}
      </div>
    </div>
  );
}

/* ── Streaming / error footer ──────────────────────── */

function StreamingAndError({
  isStreaming,
  error,
}: {
  isStreaming: boolean;
  error: string | null;
}) {
  return (
    <>
      {isStreaming && (
        <div className="flex items-center gap-2 py-2 text-sm text-muted-foreground">
          <div className="flex gap-1">
            <div className="w-1.5 h-1.5 bg-muted-foreground rounded-full animate-bounce [animation-delay:-0.3s]" />
            <div className="w-1.5 h-1.5 bg-muted-foreground rounded-full animate-bounce [animation-delay:-0.15s]" />
            <div className="w-1.5 h-1.5 bg-muted-foreground rounded-full animate-bounce" />
          </div>
        </div>
      )}
      {error && (
        <div className="flex items-center gap-2 py-2 text-sm text-red-400">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {error}
        </div>
      )}
    </>
  );
}

/* ── Empty state ───────────────────────────────────── */

function EmptyState({
  projectPath,
  onProjectPathChange,
  cliAvailable,
  cliLabel,
}: {
  projectPath: string;
  onProjectPathChange: (p: string) => void;
  cliAvailable: boolean;
  cliLabel: string;
}) {
  return (
    <div className="flex items-center justify-center h-full">
      <div className="max-w-md w-full px-6 space-y-6">
        <div className="text-center space-y-2">
          <MessageSquarePlus className="w-10 h-10 mx-auto text-muted-foreground" />
          <h2 className="text-lg font-semibold">新建对话</h2>
          <p className="text-sm text-muted-foreground">
            选择工作目录，开始与 {cliLabel} 对话
          </p>
        </div>

        <div>
          <label className="block text-xs font-medium text-muted-foreground mb-1.5">
            {cliLabel} CLI
          </label>
          <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-muted border border-border">
            <Bot className="w-4 h-4 text-orange-500" />
            <span className="text-sm font-medium">{cliLabel}</span>
            <span className={`ml-auto text-xs ${cliAvailable ? "text-green-500" : "text-red-400"}`}>
              {cliAvailable ? "已安装" : "未检测到"}
            </span>
          </div>
          {!cliAvailable && (
            <p className="mt-1.5 text-xs text-red-400">
              未检测到 {cliLabel} CLI。请先安装后再试。
            </p>
          )}
        </div>

        <div>
          <label className="block text-xs font-medium text-muted-foreground mb-1.5">
            工作目录
          </label>
          <FolderSelector value={projectPath} onChange={onProjectPathChange} />
        </div>

        <p className="text-xs text-center text-muted-foreground">
          选择工作目录后，在下方输入框输入提示词开始对话
        </p>
      </div>
    </div>
  );
}
