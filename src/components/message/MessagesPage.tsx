import { useEffect, useRef, useCallback, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import { useChatStore } from "../../stores/chatStore";
import { useChatStream } from "../../hooks/useChatStream";
import { ArrowLeft, Play, Copy, Loader2, ArrowDown, ArrowUp, Clock, Cpu, AlertCircle } from "lucide-react";
import { MessageThread } from "./MessageThread";
import { ChatInput } from "../chat/ChatInput";
import { StreamingMessage } from "../chat/StreamingMessage";
import { api } from "../../services/api";

declare const __IS_TAURI__: boolean;

export function MessagesPage() {
  const params = useParams();
  const projectId = params.projectId || "";
  const navigate = useNavigate();

  // Use React Router's wildcard param (already decoded) instead of manual pathname slicing
  const rawFilePath = params["*"] || "";
  const filePath = rawFilePath ? decodeURIComponent(rawFilePath) : "";

  const {
    source,
    messages,
    messagesLoading,
    messagesHasMore,
    messagesTotal,
    selectSession,
    loadMoreMessages,
    sessions,
    projects,
    showTimestamp,
    showModel,
    toggleTimestamp,
    toggleModel,
  } = useAppStore();

  const containerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const [showScrollDown, setShowScrollDown] = useState(false);
  const [showScrollUp, setShowScrollUp] = useState(false);
  const [initialScrollDone, setInitialScrollDone] = useState(false);
  const prevScrollHeightRef = useRef<number>(0);
  const isLoadingOlderRef = useRef(false);

  // Chat store for inline continue-chat
  const {
    messages: chatMessages,
    isStreaming: chatStreaming,
    error: chatError,
    availableClis,
    detectCli,
    continueExistingChat,
    cancelChat,
    clearChat,
    skipPermissions,
    setProjectPath: setChatProjectPath,
    setSource: setChatSource,
    setModel: setChatModel,
    model: chatModel,
  } = useChatStore();

  // Listen for chat stream events
  useChatStream();

  const session = sessions.find((s) => s.filePath === filePath);
  const project = projects.find((p) => p.id === projectId);

  const chatProjectPath = session?.projectPath || session?.cwd || project?.displayPath || "";
  const cliAvailable = availableClis.some((c) => c.cliType === source);

  // Detect CLI and set chat context on mount
  useEffect(() => {
    detectCli();
  }, [detectCli]);

  useEffect(() => {
    setChatSource(source as "claude" | "codex");
    setChatProjectPath(chatProjectPath);
    if (!chatModel) {
      setChatModel(source === "codex" ? "codex-mini-latest" : "claude-sonnet-4-20250514");
    }
  }, [source, chatProjectPath, chatModel, setChatSource, setChatProjectPath, setChatModel]);

  // Clear chat state when leaving the page / switching sessions
  useEffect(() => {
    return () => {
      clearChat();
    };
  }, [filePath, clearChat]);

  useEffect(() => {
    if (filePath) {
      setInitialScrollDone(false);
      selectSession(filePath);
    }
  }, [filePath]);

  // Auto-scroll to bottom on initial load
  useEffect(() => {
    if (!initialScrollDone && messages.length > 0 && !messagesLoading) {
      requestAnimationFrame(() => {
        if (containerRef.current) {
          containerRef.current.scrollTop = containerRef.current.scrollHeight;
        }
        setInitialScrollDone(true);
      });
    }
  }, [messages, messagesLoading, initialScrollDone]);

  // Preserve scroll position after prepending older messages
  useEffect(() => {
    if (isLoadingOlderRef.current && !messagesLoading && containerRef.current) {
      const newScrollHeight = containerRef.current.scrollHeight;
      const addedHeight = newScrollHeight - prevScrollHeightRef.current;
      containerRef.current.scrollTop += addedHeight;
      isLoadingOlderRef.current = false;
    }
  }, [messages, messagesLoading]);

  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;

    // Show scroll-to-bottom when not near bottom
    setShowScrollDown(scrollHeight - scrollTop - clientHeight > 400);
    // Show scroll-to-top when not near top
    setShowScrollUp(scrollTop > 400);

    // Load older messages when scrolling near top
    if (!messagesLoading && messagesHasMore && scrollTop < 200) {
      isLoadingOlderRef.current = true;
      prevScrollHeightRef.current = scrollHeight;
      loadMoreMessages();
    }
  }, [messagesLoading, messagesHasMore, loadMoreMessages]);

  const scrollToBottom = () => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  const scrollToTop = () => {
    containerRef.current?.scrollTo({ top: 0, behavior: "smooth" });
  };

  const [copied, setCopied] = useState(false);

  const getResumeCommand = () => {
    if (!session) return "";
    return source === "claude"
      ? `claude --resume ${session.sessionId}`
      : `codex resume ${session.sessionId}`;
  };

  const handleCopyCommand = async (e: React.MouseEvent) => {
    e.preventDefault();
    await navigator.clipboard.writeText(getResumeCommand());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleResume = async () => {
    if (!session) return;
    if (__IS_TAURI__) {
      const path = session.projectPath || session.cwd || project?.displayPath;
      if (!path) return;
      try {
        await api.resumeSession(source, session.sessionId, path, session.filePath);
      } catch (err) {
        console.error("Failed to resume session:", err);
      }
    } else {
      const cmd = getResumeCommand();
      await navigator.clipboard.writeText(cmd);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  // Auto-scroll when new chat messages arrive
  useEffect(() => {
    if (chatMessages.length > 0 || chatStreaming) {
      requestAnimationFrame(() => {
        bottomRef.current?.scrollIntoView({ behavior: "smooth" });
      });
    }
  }, [chatMessages, chatStreaming]);

  const handleSendChat = (prompt: string) => {
    if (!session) return;
    continueExistingChat(
      source as "claude" | "codex",
      session.sessionId,
      chatProjectPath,
      prompt,
      chatModel
    );
  };

  const assistantName = source === "claude" ? "Claude" : "Codex";

  return (
    <div className="flex flex-col h-full relative">
      {/* Header */}
      <div className="shrink-0 border-b border-border bg-card px-6 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3 min-w-0">
          <button
            onClick={() => navigate(`/projects/${encodeURIComponent(projectId)}`)}
            className="p-1 rounded hover:bg-accent transition-colors shrink-0"
          >
            <ArrowLeft className="w-5 h-5" />
          </button>
          <div className="min-w-0">
            <p className="text-sm font-medium truncate">
              {session?.alias || session?.firstPrompt || session?.sessionId || "Session"}
            </p>
            <p className="text-xs text-muted-foreground">
              {messagesTotal} 条消息
              {session?.gitBranch && ` · ${session.gitBranch}`}
              {` · ${assistantName}`}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          <button
            onClick={toggleTimestamp}
            className={`p-1.5 rounded transition-colors ${
              showTimestamp ? "bg-primary/15 text-primary" : "text-muted-foreground hover:text-foreground"
            }`}
            title="显示时间"
          >
            <Clock className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={toggleModel}
            className={`p-1.5 rounded transition-colors ${
              showModel ? "bg-primary/15 text-primary" : "text-muted-foreground hover:text-foreground"
            }`}
            title="显示模型"
          >
            <Cpu className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={handleResume}
            onContextMenu={handleCopyCommand}
            className="ml-1 px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded-md hover:bg-primary/90 flex items-center gap-1"
            title={__IS_TAURI__ ? "在终端中恢复此会话（右键复制命令）" : "复制恢复命令"}
          >
            {copied ? (
              <>已复制</>
            ) : __IS_TAURI__ ? (
              <><Play className="w-3 h-3" />Resume</>
            ) : (
              <><Copy className="w-3 h-3" />复制命令</>
            )}
          </button>
        </div>
      </div>

      {/* Messages */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto"
      >
        {/* Loading older indicator at top */}
        {messagesLoading && messages.length > 0 && messagesHasMore && (
          <div className="flex items-center justify-center py-4 text-muted-foreground">
            <Loader2 className="w-4 h-4 animate-spin mr-2" />
            加载更早的消息...
          </div>
        )}
        {!messagesHasMore && messages.length > 0 && (
          <div className="text-center py-4 text-xs text-muted-foreground">
            — 会话开始 —
          </div>
        )}
        {messagesLoading && messages.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground">
            <Loader2 className="w-5 h-5 animate-spin mr-2" />
            加载消息中...
          </div>
        ) : (
          <MessageThread messages={messages} source={source} showTimestamp={showTimestamp} showModel={showModel} />
        )}
        {!messagesLoading && messages.length > 0 && chatMessages.length === 0 && !chatStreaming && (
          <div className="text-center py-4 text-xs text-muted-foreground">
            — 会话结束 —
          </div>
        )}

        {/* Inline streaming messages from continue-chat */}
        {chatMessages.length > 0 && (
          <div className="max-w-4xl mx-auto px-6 py-2 space-y-1 border-t border-dashed border-border mt-2">
            {chatMessages.map((msg) => (
              <StreamingMessage key={msg.id} message={msg} source={source} />
            ))}
          </div>
        )}
        {chatStreaming && (
          <div className="max-w-4xl mx-auto px-6">
            <div className="flex items-center gap-2 py-2 text-sm text-muted-foreground">
              <div className="flex gap-1">
                <div className="w-1.5 h-1.5 bg-muted-foreground rounded-full animate-bounce [animation-delay:-0.3s]" />
                <div className="w-1.5 h-1.5 bg-muted-foreground rounded-full animate-bounce [animation-delay:-0.15s]" />
                <div className="w-1.5 h-1.5 bg-muted-foreground rounded-full animate-bounce" />
              </div>
            </div>
          </div>
        )}
        {chatError && (
          <div className="max-w-4xl mx-auto px-6">
            <div className="flex items-center gap-2 py-2 text-sm text-red-400">
              <AlertCircle className="w-4 h-4 shrink-0" />
              {chatError}
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {/* Chat input */}
      {session && cliAvailable && (
        <div className="shrink-0">
          <ChatInput
            onSend={handleSendChat}
            onCancel={cancelChat}
            isStreaming={chatStreaming}
            disabled={!chatProjectPath}
          />
        </div>
      )}

      {/* Scroll buttons */}
      <div className="absolute bottom-20 right-6 flex flex-col gap-2">
        {showScrollUp && (
          <button
            onClick={scrollToTop}
            className="p-2.5 rounded-full bg-primary text-primary-foreground shadow-lg hover:bg-primary/90 transition-all hover:scale-105"
            title="跳转到顶部"
          >
            <ArrowUp className="w-4 h-4" />
          </button>
        )}
        {showScrollDown && (
          <button
            onClick={scrollToBottom}
            className="p-2.5 rounded-full bg-primary text-primary-foreground shadow-lg hover:bg-primary/90 transition-all hover:scale-105"
            title="跳转到底部"
          >
            <ArrowDown className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  );
}
