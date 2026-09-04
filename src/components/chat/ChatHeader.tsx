import { useMemo, useState } from "react";
import { DEFAULT_CHAT_PANE_ID, useChatStore } from "../../stores/chatStore";
import {
  FolderOpen,
  Bot,
  CircleDot,
  Shield,
  ShieldOff,
  Cpu,
  ChevronsUpDown,
  Rows3,
  Hash,
  Copy,
  Check,
} from "lucide-react";
import { OmpMark } from "../layout/ProviderMarks";

export function ChatHeader({
  paneId = DEFAULT_CHAT_PANE_ID,
  onExpandAll,
  onCollapseAll,
}: {
  paneId?: string;
  onExpandAll: () => void;
  onCollapseAll: () => void;
}) {
  const pane = useChatStore((state) => state.getPaneState(paneId));
  const availableClis = useChatStore((state) => state.availableClis);
  const skipPermissions = useChatStore((state) => state.skipPermissions);
  const setSkipPermissions = useChatStore((state) => state.setSkipPermissions);
  const { projectPath, messages, isStreaming, source, sessionId } = pane;
  const [copiedSessionId, setCopiedSessionId] = useState(false);

  const handleCopySessionId = () => {
    if (!sessionId) return;
    navigator.clipboard.writeText(sessionId);
    setCopiedSessionId(true);
    setTimeout(() => setCopiedSessionId(false), 1500);
  };
  const shortSessionId = sessionId ? sessionId.slice(0, 8) : "";
  const resumeHint =
    source === "codex"
      ? `codex resume ${sessionId ?? ""}`
      : source === "omp"
        ? `omp --resume ${sessionId ?? ""}`
        : `claude --resume ${sessionId ?? ""}`;

  const cliLabel = source === "codex" ? "Codex" : source === "omp" ? "Oh My Pi" : "Claude";
  const cliInfo = availableClis.find((c) => c.cliType === source);
  const canToggleExpand = messages.length > 0;

  // Aggregate token stats
  const tokenStats = useMemo(() => {
    let input = 0;
    let output = 0;
    let cacheWrite = 0;
    let cacheRead = 0;
    for (const msg of messages) {
      if (msg.usage) {
        input += msg.usage.inputTokens;
        output += msg.usage.outputTokens;
        cacheWrite += msg.usage.cacheCreationInputTokens;
        cacheRead += msg.usage.cacheReadInputTokens;
      }
    }
    return { input, output, cacheWrite, cacheRead, total: input + output };
  }, [messages]);

  return (
    <div className="flex items-center gap-3 px-4 py-2.5 border-b border-border bg-card">
      {/* Source indicator */}
      <div className="flex items-center gap-1.5">
        {source === "omp" ? <OmpMark className="w-4 h-4" /> : <Bot className={`w-4 h-4 ${source === "codex" ? "text-green-500" : "text-orange-500"}`} />}
        <span className="text-sm font-medium">{cliLabel}</span>
      </div>

      {/* CLI status */}
      <div className="flex items-center gap-1 text-xs text-muted-foreground">
        <CircleDot
          className={`w-3 h-3 ${cliInfo ? "text-green-500" : "text-red-500"}`}
        />
        <span>
          {cliInfo
            ? cliInfo.version
              ? `v${cliInfo.version}`
              : "已安装"
            : "未检测到"}
        </span>
      </div>

      {/* Project path */}
      {projectPath && (
        <div className="flex items-center gap-1 text-xs text-muted-foreground max-w-[200px]">
          <FolderOpen className="w-3 h-3 shrink-0" />
          <span className="truncate" title={projectPath}>
            {projectPath.split(/[\\/]/).pop()}
          </span>
        </div>
      )}

      {/* Session ID (for CLI resume) */}
      {sessionId && (
        <button
          type="button"
          onClick={handleCopySessionId}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
          title={`点击复制完整 ID — ${resumeHint}`}
        >
          <Hash className="w-3 h-3 shrink-0" />
          <span className="font-mono tabular-nums">{shortSessionId}</span>
          {copiedSessionId ? (
            <Check className="w-3 h-3 text-green-500" />
          ) : (
            <Copy className="w-3 h-3 opacity-60" />
          )}
        </button>
      )}

      {/* Token stats */}
      {tokenStats.total > 0 && (
        <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground tabular-nums">
          <Cpu className="w-3 h-3 shrink-0" />
          <span title="输入 tokens">入 {tokenStats.input.toLocaleString()}</span>
          <span className="opacity-30">|</span>
          <span title="输出 tokens">出 {tokenStats.output.toLocaleString()}</span>
          {tokenStats.cacheWrite > 0 && (
            <>
              <span className="opacity-30">|</span>
              <span title="写入缓存 tokens">写缓存 {tokenStats.cacheWrite.toLocaleString()}</span>
            </>
          )}
          {tokenStats.cacheRead > 0 && (
            <>
              <span className="opacity-30">|</span>
              <span title="读取缓存 tokens">读缓存 {tokenStats.cacheRead.toLocaleString()}</span>
            </>
          )}
        </div>
      )}

      <div className="flex-1" />

      {/* Skip permissions toggle */}
      <button
        type="button"
        onClick={() => setSkipPermissions(!skipPermissions)}
        disabled={isStreaming}
        className={`flex items-center gap-1 px-2 py-1 text-xs rounded border transition-colors ${
          skipPermissions
            ? "border-yellow-500/50 bg-yellow-500/10 text-yellow-500"
            : "border-border bg-muted text-muted-foreground hover:text-foreground"
        } disabled:opacity-50`}
        title={
          skipPermissions
            ? "已跳过权限确认（危险模式）"
            : "正常权限模式"
        }
      >
        {skipPermissions ? (
          <ShieldOff className="w-3 h-3" />
        ) : (
          <Shield className="w-3 h-3" />
        )}
        <span className="hidden sm:inline">
          {skipPermissions ? "跳过权限" : "正常权限"}
        </span>
      </button>

      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={onExpandAll}
          disabled={!canToggleExpand}
          className="flex items-center gap-1 px-2 py-1 text-xs rounded border border-border bg-muted text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50 disabled:hover:text-muted-foreground"
          title="全部展开"
        >
          <Rows3 className="w-3 h-3" />
          <span className="hidden sm:inline">展开</span>
        </button>
        <button
          type="button"
          onClick={onCollapseAll}
          disabled={!canToggleExpand}
          className="flex items-center gap-1 px-2 py-1 text-xs rounded border border-border bg-muted text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50 disabled:hover:text-muted-foreground"
          title="全部折叠"
        >
          <ChevronsUpDown className="w-3 h-3" />
          <span className="hidden sm:inline">折叠</span>
        </button>
      </div>

      {/* Streaming indicator */}
      {isStreaming && (
        <div className="flex items-center gap-1.5 text-xs text-blue-400">
          <div className="w-1.5 h-1.5 bg-blue-400 rounded-full animate-pulse" />
          对话中...
        </div>
      )}
    </div>
  );
}
