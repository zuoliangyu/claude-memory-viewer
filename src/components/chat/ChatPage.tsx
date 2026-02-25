import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { useChatStore } from "../../stores/chatStore";
import { useAppStore } from "../../stores/appStore";
import { useChatStream } from "../../hooks/useChatStream";
import { ChatHeader } from "./ChatHeader";
import { ChatInput } from "./ChatInput";
import { StreamingMessage } from "./StreamingMessage";
import { FolderSelector } from "./FolderSelector";
import { MessageSquarePlus, AlertCircle, Bot, Terminal } from "lucide-react";

const DEFAULT_CLAUDE_MODEL = "claude-sonnet-4-6";
const DEFAULT_CODEX_MODEL = "codex-mini-latest";

export function ChatPage() {
  const { sessionId: urlSessionId } = useParams<{ sessionId?: string }>();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const {
    isActive,
    sessionId,
    source,
    projectPath,
    model,
    messages,
    isStreaming,
    error,
    availableClis,
    defaultModel,
    cliConfig,
    detectCli,
    fetchCliConfig,
    startNewChat,
    continueExistingChat,
    clearChat,
    cancelChat,
    setProjectPath,
    setSource,
    setModel,
  } = useChatStore();

  const appSource = useAppStore((s) => s.source);
  const [initialized, setInitialized] = useState(false);

  // Detect CLI on mount
  useEffect(() => {
    detectCli();
  }, [detectCli]);

  // Initialize source from app store and fetch CLI config
  useEffect(() => {
    if (!initialized) {
      setSource(appSource);
      fetchCliConfig(appSource);
      setInitialized(true);
    }
  }, [appSource, initialized, setSource, fetchCliConfig]);

  // Set default model: user override > CLI config > hardcoded fallback
  // For CLI chat mode, prefer CLI's own model since it handles aliases natively.
  useEffect(() => {
    if (initialized && !model) {
      const defModel =
        defaultModel ||
        cliConfig?.defaultModel ||
        (source === "codex" ? DEFAULT_CODEX_MODEL : DEFAULT_CLAUDE_MODEL);
      setModel(defModel);
    }
  }, [initialized, model, defaultModel, cliConfig, source, setModel]);

  // If navigating with a sessionId (continue mode), set it up
  useEffect(() => {
    if (urlSessionId && !isActive) {
      // We're continuing an existing session - the session_id comes from URL
      // The user still needs to provide a prompt to continue
    }
  }, [urlSessionId, isActive]);

  // Listen for stream events
  useChatStream();

  // Auto-scroll to bottom
  useEffect(() => {
    if (messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages]);

  const handleSend = (prompt: string) => {
    if (!projectPath) return;

    if (sessionId || urlSessionId) {
      const sid = sessionId || urlSessionId!;
      continueExistingChat(
        source as "claude" | "codex",
        sid,
        projectPath,
        prompt,
        model
      );
    } else {
      startNewChat(source as "claude" | "codex", projectPath, prompt, model);
    }
  };

  const handleSourceToggle = (s: "claude" | "codex") => {
    if (isStreaming) return;
    setSource(s);
    fetchCliConfig(s);
    setModel(""); // Will be set by the effect using CLI config
    clearChat();
  };

  const cliAvailable = availableClis.some((c) => c.cliType === source);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <ChatHeader />

      {/* Messages area */}
      <div
        ref={scrollContainerRef}
        className="flex-1 overflow-y-auto"
      >
        {!isActive && messages.length === 0 ? (
          <EmptyState
            source={source as "claude" | "codex"}
            projectPath={projectPath}
            onProjectPathChange={setProjectPath}
            onSourceChange={handleSourceToggle}
            cliAvailable={cliAvailable}
          />
        ) : (
          <div className="max-w-4xl mx-auto px-4 py-4 space-y-1">
            {messages.map((msg) => (
              <StreamingMessage
                key={msg.id}
                message={msg}
                source={source}
              />
            ))}
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
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* Input area */}
      <div className="max-w-4xl mx-auto w-full">
        <ChatInput
          onSend={handleSend}
          onCancel={cancelChat}
          isStreaming={isStreaming}
          disabled={!projectPath || !cliAvailable}
        />
      </div>
    </div>
  );
}

function EmptyState({
  source,
  projectPath,
  onProjectPathChange,
  onSourceChange,
  cliAvailable,
}: {
  source: "claude" | "codex";
  projectPath: string;
  onProjectPathChange: (p: string) => void;
  onSourceChange: (s: "claude" | "codex") => void;
  cliAvailable: boolean;
}) {
  return (
    <div className="flex items-center justify-center h-full">
      <div className="max-w-md w-full px-6 space-y-6">
        <div className="text-center space-y-2">
          <MessageSquarePlus className="w-10 h-10 mx-auto text-muted-foreground" />
          <h2 className="text-lg font-semibold">新建对话</h2>
          <p className="text-sm text-muted-foreground">
            选择 CLI 工具和工作目录，开始与 AI 对话
          </p>
        </div>

        {/* Source selector */}
        <div>
          <label className="block text-xs font-medium text-muted-foreground mb-1.5">
            CLI 工具
          </label>
          <div className="flex rounded-lg bg-muted p-0.5">
            <button
              onClick={() => onSourceChange("claude")}
              className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-all ${
                source === "claude"
                  ? "bg-orange-500/20 text-orange-400 shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <Bot className="w-4 h-4" />
              Claude
            </button>
            <button
              onClick={() => onSourceChange("codex")}
              className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-all ${
                source === "codex"
                  ? "bg-green-500/20 text-green-400 shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <Terminal className="w-4 h-4" />
              Codex
            </button>
          </div>
          {!cliAvailable && (
            <p className="mt-1.5 text-xs text-red-400">
              未检测到 {source} CLI。请先安装后再试。
            </p>
          )}
        </div>

        {/* Folder selector */}
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
