import { memo, useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode, type RefObject } from "react";
import type { DisplayMessage } from "../../types";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ToolOutputMessage } from "./ToolOutputMessage";
import { useAppStore } from "../../stores/appStore";
import { Star, GitFork, Play, Loader2, ChevronDown, ChevronRight } from "lucide-react";
import { api } from "../../services/api";
import { useExpandAllControl } from "../common/ExpandAllContext";
import {
  buildMessageTree,
  getUserMessageId,
  type ThreadDisplayNode,
} from "./threading";

declare const __IS_TAURI__: boolean;

interface MessageThreadProps {
  messages: DisplayMessage[];
  source: string;
  showTimestamp: boolean;
  showModel: boolean;
  sessionId?: string;
  projectId?: string;
  filePath?: string;
  sessionTitle?: string;
  projectName?: string;
  projectPath?: string;
  viewportRef?: RefObject<HTMLDivElement | null>;
  priorityMessageId?: string | null;
}

const DEFER_RENDER_THRESHOLD = 24;
const INITIAL_RECENT_RENDER_COUNT = 18;
const DEFER_RENDER_ROOT_MARGIN = "480px 0px 480px 0px";
const PLACEHOLDER_MIN_HEIGHT = 76;
const PLACEHOLDER_MAX_HEIGHT = 320;

interface ThreadFoldControl {
  hasChildren: boolean;
  expanded: boolean;
  setExpanded: (next: boolean) => void;
  childrenCount: number;
}

function flattenThreadNodes(nodes: ThreadDisplayNode[]): ThreadDisplayNode[] {
  const flat: ThreadDisplayNode[] = [];

  const visit = (list: ThreadDisplayNode[]) => {
    list.forEach((node) => {
      flat.push(node);
      if (node.children.length > 0) {
        visit(node.children);
      }
    });
  };

  visit(nodes);
  return flat;
}

function estimatePlaceholderHeight(node: ThreadDisplayNode): number {
  const textLength = node.message.content.reduce((total, block) => {
    switch (block.type) {
      case "text":
        return total + block.text.length;
      case "thinking":
        return total + block.thinking.length;
      case "reasoning":
        return total + block.text.length;
      case "tool_result":
        return total + block.content.length;
      case "function_call_output":
        return total + block.output.length;
      case "tool_use":
        return total + block.input.length;
      case "function_call":
        return total + block.arguments.length;
      default:
        return total;
    }
  }, 0);

  const blockWeight = node.message.content.length * 18;
  const baseHeight = node.message.role === "assistant" ? 88 : node.message.role === "tool" ? 96 : 80;
  return Math.max(
    PLACEHOLDER_MIN_HEIGHT,
    Math.min(PLACEHOLDER_MAX_HEIGHT, baseHeight + Math.ceil(textLength / 140) * 24 + blockWeight)
  );
}

function getPlaceholderLabel(node: ThreadDisplayNode) {
  switch (node.message.role) {
    case "user":
      return "用户消息";
    case "assistant":
      return "助手回复";
    case "tool":
      return "工具输出";
    default:
      return "消息";
  }
}

function DeferredMessagePlaceholder({
  node,
  estimatedHeight,
  isThreaded,
}: {
  node: ThreadDisplayNode;
  estimatedHeight: number;
  isThreaded: boolean;
}) {
  const isUser = node.message.role === "user" && !isThreaded;

  return (
    <div
      className={`flex ${isUser ? "justify-end" : "justify-start"}`}
      style={{ minHeight: `${estimatedHeight}px` }}
    >
      <div
        className={`w-full rounded-2xl border border-dashed border-border/70 bg-muted/25 px-4 py-3 ${
          !isThreaded ? "max-w-[85%]" : ""
        } ${
          isUser ? "bg-primary/5" : ""
        }`}
      >
        <div className="mb-2 flex items-center gap-2 text-[11px] text-muted-foreground">
          <span>{getPlaceholderLabel(node)}</span>
          <span className="truncate">{node.threadTitle || "正在准备内容..."}</span>
        </div>
        <div className="space-y-2">
          <div className="h-3 rounded bg-muted/70" />
          <div className="h-3 w-11/12 rounded bg-muted/60" />
          <div className="h-3 w-8/12 rounded bg-muted/50" />
        </div>
      </div>
    </div>
  );
}

function DeferredThreadMessage({
  node,
  eager,
  viewportRef,
  renderContent,
  isThreaded,
}: {
  node: ThreadDisplayNode;
  eager: boolean;
  viewportRef?: RefObject<HTMLDivElement | null>;
  renderContent: () => ReactNode;
  isThreaded: boolean;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const userMessageId =
    node.message.role === "user" ? getUserMessageId(node.message, node.originalIndex) : undefined;
  const estimatedHeight = useMemo(() => estimatePlaceholderHeight(node), [node]);
  const [activated, setActivated] = useState(eager);

  useEffect(() => {
    if (eager) setActivated(true);
  }, [node.id, eager]);

  useEffect(() => {
    if (activated) {
      return;
    }

    const element = containerRef.current;
    if (!element || typeof IntersectionObserver === "undefined") {
      setActivated(true);
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting || entry.intersectionRatio > 0)) {
          setActivated(true);
          observer.disconnect();
        }
      },
      {
        root: viewportRef?.current ?? null,
        rootMargin: DEFER_RENDER_ROOT_MARGIN,
      }
    );

    observer.observe(element);
    return () => observer.disconnect();
  }, [activated, viewportRef]);

  return (
    <div
      ref={containerRef}
      data-user-msg-id={userMessageId}
    >
      {activated ? renderContent() : <DeferredMessagePlaceholder node={node} estimatedHeight={estimatedHeight} isThreaded={isThreaded} />}
    </div>
  );
}

function getThreadLineText(node: ThreadDisplayNode, source: string): string {
  const title = node.threadTitle.trim();

  if (node.message.role === "assistant") {
    const assistantName = source === "claude" ? "Claude" : "Codex";
    return title ? `${assistantName} · ${title}` : assistantName;
  }

  if (node.message.role === "user") {
    return title || "（用户问题）";
  }

  return title || "工具输出";
}

function getThreadLineTone(node: ThreadDisplayNode) {
  switch (node.message.role) {
    case "assistant":
      return {
        label: "text-foreground",
        meta: "text-sky-600 dark:text-sky-400",
      };
    case "user":
      return {
        label: "text-foreground",
        meta: "text-emerald-600 dark:text-emerald-400",
      };
    default:
      return {
        label: "text-muted-foreground",
        meta: "text-muted-foreground",
      };
  }
}

function ThreadBranch({
  node,
  renderMessage,
  source,
}: {
  node: ThreadDisplayNode;
  renderMessage: (node: ThreadDisplayNode, threadFold?: ThreadFoldControl) => ReactNode;
  source: string;
}) {
  const hasChildren = node.children.length > 0;
  const { expanded, setExpanded } = useExpandAllControl(true);
  const lineText = getThreadLineText(node, source);
  const tone = getThreadLineTone(node);
  const collapsedText = lineText.trim() || "当前分支";

  // For user nodes the bubble itself owns the unified fold control
  // (collapsing the bubble + all of its replies together), so we hide
  // the duplicate fold button that ThreadBranch would otherwise render.
  const isUserNode = node.message.role === "user";
  const showOwnFoldButton = hasChildren && !isUserNode;
  const threadFold: ThreadFoldControl | undefined = hasChildren
    ? { hasChildren, expanded, setExpanded, childrenCount: node.children.length }
    : undefined;

  return (
    <div className="min-w-0">
      <div className="relative min-w-0 rounded-md text-left">
        {showOwnFoldButton && (
          <button
            onClick={() => setExpanded(!expanded)}
            className={`absolute right-full top-0 mr-1 rounded-full border p-0.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground ${
              expanded ? "border-border/80 bg-background" : "border-border/60 bg-muted/40"
            }`}
            title={expanded ? `折叠 ${collapsedText}` : `展开 ${collapsedText}`}
          >
            {expanded ? (
              <ChevronDown className="h-3.5 w-3.5" />
            ) : (
              <ChevronRight className="h-3.5 w-3.5" />
            )}
          </button>
        )}

        <div className="min-w-0">
          {!isUserNode && (
            <div className="flex min-w-0 items-center gap-2 py-0.5 text-left">
              <span
                className={`min-w-0 truncate text-[13px] leading-5 ${tone.label}`}
                title={lineText}
              >
                {lineText}
              </span>
              {hasChildren && (
                <span className={`shrink-0 text-[11px] ${tone.meta}`}>
                  {node.children.length} 条回复
                </span>
              )}
            </div>
          )}

          <div className={isUserNode ? "min-w-0" : "mt-2 min-w-0"}>{renderMessage(node, threadFold)}</div>

          {hasChildren && expanded && (
            <div className="mt-4 space-y-4">
              {node.children.map((child) => (
                <ThreadBranch
                  key={child.id}
                  node={child}
                  renderMessage={renderMessage}
                  source={source}
                />
              ))}
            </div>
          )}

          {hasChildren && !expanded && !isUserNode && (
            <div className="mt-2 text-xs text-muted-foreground">
              已折叠 {collapsedText}（{node.children.length} 条后续消息）
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export const MessageThread = memo(function MessageThread({
  messages,
  source,
  showTimestamp,
  showModel,
  sessionId,
  projectId,
  filePath,
  sessionTitle,
  projectName,
  projectPath,
  viewportRef,
  priorityMessageId,
}: MessageThreadProps) {
  const addBookmark = useAppStore((state) => state.addBookmark);
  const removeBookmark = useAppStore((state) => state.removeBookmark);
  const isBookmarked = useAppStore((state) => state.isBookmarked);
  const bookmarks = useAppStore((state) => state.bookmarks);
  const terminalShell = useAppStore((state) => state.terminalShell);
  const refreshInBackground = useAppStore((state) => state.refreshInBackground);
  const [forkingMsgId, setForkingMsgId] = useState<string | null>(null);
  const [forkSuccessMsgId, setForkSuccessMsgId] = useState<string | null>(null);
  const [assistantContextMenu, setAssistantContextMenu] = useState<{
    x: number;
    y: number;
    userMsgId: string;
  } | null>(null);

  useEffect(() => {
    if (!assistantContextMenu) return;

    const closeMenu = () => setAssistantContextMenu(null);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setAssistantContextMenu(null);
      }
    };

    window.addEventListener("click", closeMenu);
    window.addEventListener("resize", closeMenu);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("keydown", closeOnEscape);

    return () => {
      window.removeEventListener("click", closeMenu);
      window.removeEventListener("resize", closeMenu);
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [assistantContextMenu]);

  const handleToggleBookmark = (msg: DisplayMessage, msgId: string) => {
    if (!sessionId || !projectId || !filePath) return;
    const bookmarked = isBookmarked(sessionId, msgId);
    if (bookmarked) {
      const bm = bookmarks.find(
        (b) => b.sessionId === sessionId && b.messageId === msgId
      );
      if (bm) removeBookmark(bm.id);
    } else {
      const textContent = msg.content
        .filter((b): b is { type: "text"; text: string } => b.type === "text")
        .map((b) => b.text)
        .join(" ")
        .trim();
      const preview = textContent.slice(0, 100) + (textContent.length > 100 ? "..." : "");
      addBookmark({
        source,
        projectId,
        sessionId,
        filePath,
        messageId: msgId,
        preview,
        sessionTitle: sessionTitle || "",
        projectName: projectName || "",
      });
    }
  };

  const handleFork = async (msgId: string) => {
    if (!filePath || !projectPath || !msgId) return;
    setForkingMsgId(msgId);
    try {
      await api.forkAndResume(source, filePath, msgId, projectPath, terminalShell);
      setForkSuccessMsgId(msgId);
      setTimeout(() => setForkSuccessMsgId(null), 2000);
      refreshInBackground();
    } catch (err) {
      console.error("Failed to fork session:", err);
    } finally {
      setForkingMsgId(null);
    }
  };

  const handleAssistantContextMenu = (
    event: MouseEvent<HTMLDivElement>,
    node: ThreadDisplayNode
  ) => {
    if (!showActionButtons || !node.forkUserMessageId) {
      return;
    }

    event.preventDefault();
    setAssistantContextMenu({
      x: event.clientX,
      y: event.clientY,
      userMsgId: node.forkUserMessageId,
    });
  };

  const handleAssistantFork = async () => {
    if (!assistantContextMenu) {
      return;
    }

    setAssistantContextMenu(null);
    await handleFork(assistantContextMenu.userMsgId);
  };

  const handleResumeFromMessage = async () => {
    if (!sessionId || !projectPath) return;
    try {
      await api.resumeSession(source, sessionId, projectPath, filePath, terminalShell);
    } catch (err) {
      console.error("Failed to resume session:", err);
    }
  };

  const showActionButtons = __IS_TAURI__ && source === "claude";
  const { roots, isThreaded } = useMemo(() => buildMessageTree(messages), [messages]);
  // Map each user-message id (uuid or fallback) to its 0-based ordinal so
  // UserMessage can paint a question-specific hue. Index is computed from the
  // raw `messages` order, which matches what the TOC sidebar shows, keeping
  // colors in sync across surfaces.
  const userQuestionIndexMap = useMemo(() => {
    const map = new Map<string, number>();
    let i = 0;
    messages.forEach((msg, idx) => {
      if (msg.role !== "user") return;
      map.set(getUserMessageId(msg, idx), i);
      i += 1;
    });
    return map;
  }, [messages]);
  const flatNodes = useMemo(() => flattenThreadNodes(roots), [roots]);
  const shouldDefer = flatNodes.length > DEFER_RENDER_THRESHOLD;
  const eagerNodeIds = useMemo(() => {
    if (!shouldDefer) {
      return null;
    }

    const ids = new Set(
      flatNodes
        .slice(Math.max(0, flatNodes.length - INITIAL_RECENT_RENDER_COUNT))
        .map((node) => node.id),
    );
    if (priorityMessageId) {
      ids.add(priorityMessageId);
    }
    return ids;
  }, [flatNodes, priorityMessageId, shouldDefer]);

  const renderMessageContent = (node: ThreadDisplayNode, threadFold?: ThreadFoldControl) => {
    const msg = node.message;
    const messageLayout = isThreaded ? "thread" : "default";

    if (msg.role === "user") {
      const msgId = getUserMessageId(msg, node.originalIndex);
      const bookmarked = sessionId ? isBookmarked(sessionId, msgId) : false;
      const canFork = showActionButtons && !!msg.uuid;
      const canResume = showActionButtons && !!sessionId;
      const isForking = forkingMsgId === msgId;
      const isForkSuccess = forkSuccessMsgId === msgId;

      return (
        <div
          key={msgId}
          data-user-msg-id={msgId}
          className={`group/bookmark flex items-start gap-1.5 ${isThreaded ? "w-full justify-start" : "justify-end"}`}
        >
          <div className={`min-w-0 ${isThreaded ? "flex-1" : ""}`}>
            <UserMessage
              message={msg}
              showTimestamp={showTimestamp}
              layout={messageLayout}
              questionIndex={userQuestionIndexMap.get(msgId)}
              threadHint={
                node.parentSource === "mention" && node.mentionAnchors[0]
                  ? `通过 ${node.mentionAnchors[0]} 挂到该回复`
                  : null
              }
              replyCount={threadFold?.childrenCount ?? 0}
              repliesExpanded={threadFold?.expanded}
              onToggleReplies={threadFold?.setExpanded}
            />
          </div>
          <div className="flex shrink-0 flex-col gap-0.5">
            {canResume && (
              <button
                onClick={handleResumeFromMessage}
                className="mt-1 rounded p-1 text-muted-foreground opacity-0 transition-all group-hover/bookmark:opacity-100 hover:text-primary"
                title="在终端中恢复此会话"
              >
                <Play className="h-3.5 w-3.5" />
              </button>
            )}
            {canFork && (
              <button
                onClick={() => handleFork(msgId)}
                disabled={isForking}
                className={`rounded p-1 transition-all ${
                  isForkSuccess
                    ? "text-green-500 opacity-100"
                    : isForking
                      ? "text-muted-foreground opacity-100"
                      : "text-muted-foreground opacity-0 group-hover/bookmark:opacity-100 hover:text-primary"
                }`}
                title="从此处分叉新会话"
              >
                {isForking ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <GitFork className="h-3.5 w-3.5" />
                )}
              </button>
            )}
            {sessionId && (
              <button
                onClick={() => handleToggleBookmark(msg, msgId)}
                className={`rounded p-1 transition-all ${
                  bookmarked
                    ? "text-yellow-500 opacity-100"
                    : "text-muted-foreground opacity-0 group-hover/bookmark:opacity-100 hover:text-yellow-500"
                }`}
                title={bookmarked ? "取消收藏" : "收藏此消息"}
              >
                <Star className={`h-3.5 w-3.5 ${bookmarked ? "fill-current" : ""}`} />
              </button>
            )}
          </div>
        </div>
      );
    }

    if (msg.role === "tool") {
      return (
        <div key={node.id}>
          <ToolOutputMessage message={msg} showTimestamp={showTimestamp} layout={messageLayout} />
        </div>
      );
    }

    return (
      <div key={node.id}>
        <div onContextMenu={(event) => handleAssistantContextMenu(event, node)}>
          <AssistantMessage
            message={msg}
            source={source}
            showTimestamp={showTimestamp}
            showModel={showModel}
            layout={messageLayout}
            threadAnchor={node.threadAnchor}
            threadHint={showActionButtons && node.forkUserMessageId ? "右击可从此回复分叉" : null}
          />
        </div>
      </div>
    );
  };

  const renderMessage = (node: ThreadDisplayNode, threadFold?: ThreadFoldControl) => (
    <DeferredThreadMessage
      key={node.id}
      node={node}
      eager={!shouldDefer || eagerNodeIds?.has(node.id) === true}
      viewportRef={viewportRef}
      isThreaded={isThreaded}
      renderContent={() => renderMessageContent(node, threadFold)}
    />
  );

  const renderAssistantContextMenu = () => {
    if (!assistantContextMenu) {
      return null;
    }

    return (
      <div
        className="fixed z-50 min-w-[11rem] rounded-lg border border-border bg-popover p-1 shadow-lg"
        style={{
          left: Math.min(assistantContextMenu.x, window.innerWidth - 220),
          top: Math.min(assistantContextMenu.y, window.innerHeight - 80),
        }}
        onClick={(event) => event.stopPropagation()}
      >
        <button
          onClick={handleAssistantFork}
          className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm text-foreground transition-colors hover:bg-accent"
        >
          <GitFork className="h-4 w-4 text-primary" />
          从此回复继续分叉
        </button>
      </div>
    );
  };

  if (!isThreaded) {
    return (
      <div className="mx-auto max-w-3xl space-y-6 px-6 py-6">
        {roots.map((node) => renderMessage(node))}
        {renderAssistantContextMenu()}
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6 px-2 py-6 sm:px-3">
      {roots.map((node) => (
        <ThreadBranch key={node.id} node={node} renderMessage={renderMessage} source={source} />
      ))}
      {renderAssistantContextMenu()}
    </div>
  );
}, (prevProps, nextProps) => (
  prevProps.messages === nextProps.messages &&
  prevProps.source === nextProps.source &&
  prevProps.showTimestamp === nextProps.showTimestamp &&
  prevProps.showModel === nextProps.showModel &&
  prevProps.sessionId === nextProps.sessionId &&
  prevProps.projectId === nextProps.projectId &&
  prevProps.filePath === nextProps.filePath &&
  prevProps.sessionTitle === nextProps.sessionTitle &&
  prevProps.projectName === nextProps.projectName &&
  prevProps.projectPath === nextProps.projectPath &&
  prevProps.viewportRef === nextProps.viewportRef &&
  prevProps.priorityMessageId === nextProps.priorityMessageId
));
