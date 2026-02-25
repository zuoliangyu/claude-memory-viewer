import { useChatStore } from "../../stores/chatStore";
import {
  FolderOpen,
  Bot,
  Terminal,
  CircleDot,
  Shield,
  ShieldOff,
} from "lucide-react";

export function ChatHeader() {
  const {
    source,
    projectPath,
    isStreaming,
    availableClis,
    skipPermissions,
    setSkipPermissions,
  } = useChatStore();

  const cliInfo = availableClis.find((c) => c.cliType === source);
  const sourceColor = source === "codex" ? "text-green-500" : "text-orange-500";
  const SourceIcon = source === "codex" ? Terminal : Bot;

  return (
    <div className="flex items-center gap-3 px-4 py-2.5 border-b border-border bg-card">
      {/* Source indicator */}
      <div className="flex items-center gap-1.5">
        <SourceIcon className={`w-4 h-4 ${sourceColor}`} />
        <span className="text-sm font-medium capitalize">{source}</span>
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

      <div className="flex-1" />

      {/* Skip permissions toggle */}
      <button
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
