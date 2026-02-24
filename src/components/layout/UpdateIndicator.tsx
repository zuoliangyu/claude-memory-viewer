import { useState } from "react";
import { useUpdateStore } from "../../stores/updateStore";
import {
  ArrowDownToLine,
  ExternalLink,
  X,
  RefreshCw,
  CheckCircle2,
  ChevronUp,
  ChevronDown,
} from "lucide-react";

export function UpdateIndicator() {
  const [expanded, setExpanded] = useState(false);
  const {
    installType,
    status,
    currentVersion,
    newVersion,
    releaseNotes,
    downloadProgress,
    dismissed,
    errorMessage,
    downloadAndInstall,
    openDownloadPage,
    dismiss,
    checkForUpdate,
  } = useUpdateStore();

  const hasUpdate = status === "available" && !dismissed;
  const isChecking = status === "checking";
  const isWorking = status === "downloading" || status === "installing";

  return (
    <div className="space-y-2">
      {/* Top row: version + check button */}
      <div className="flex items-center gap-2">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <span>v{currentVersion || "..."}</span>
          {hasUpdate && (
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-blue-500" />
            </span>
          )}
          {isWorking && <RefreshCw className="w-3 h-3 animate-spin" />}
          {expanded ? (
            <ChevronDown className="w-3 h-3" />
          ) : (
            <ChevronUp className="w-3 h-3" />
          )}
        </button>

        {/* Manual check button */}
        <button
          onClick={() => checkForUpdate()}
          disabled={isChecking || isWorking}
          className="p-0.5 text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40"
          title="检查更新"
        >
          <RefreshCw
            className={`w-3 h-3 ${isChecking ? "animate-spin" : ""}`}
          />
        </button>
      </div>

      {/* Expanded panel (inline, not absolute) */}
      {expanded && (
        <div className="bg-muted/50 border border-border rounded-md p-2.5 space-y-2">
          {/* No update */}
          {status === "idle" && !newVersion && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <CheckCircle2 className="w-3.5 h-3.5 text-green-500 shrink-0" />
              <span>已是最新版本</span>
            </div>
          )}

          {/* Checking */}
          {isChecking && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <RefreshCw className="w-3.5 h-3.5 animate-spin shrink-0" />
              <span>检查更新中...</span>
            </div>
          )}

          {/* Update available */}
          {(status === "available" || (status === "idle" && newVersion)) && (
            <>
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-foreground">
                  新版本可用
                </span>
                <button
                  onClick={() => {
                    dismiss();
                    setExpanded(false);
                  }}
                  className="text-muted-foreground hover:text-foreground"
                  title="忽略此版本"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
              <div className="text-xs text-muted-foreground">
                v{currentVersion} → v{newVersion}
              </div>
              {releaseNotes && (
                <div className="text-xs text-muted-foreground max-h-20 overflow-y-auto border-t border-border pt-2 whitespace-pre-wrap">
                  {releaseNotes}
                </div>
              )}
              {installType === "installed" ? (
                <button
                  onClick={() => downloadAndInstall()}
                  className="w-full flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium bg-blue-600 text-white hover:bg-blue-700 transition-colors"
                >
                  <ArrowDownToLine className="w-3.5 h-3.5" />
                  更新并重启
                </button>
              ) : (
                <button
                  onClick={() => openDownloadPage()}
                  className="w-full flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium bg-blue-600 text-white hover:bg-blue-700 transition-colors"
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  前往下载新版本
                </button>
              )}
            </>
          )}

          {/* Downloading */}
          {status === "downloading" && (
            <>
              <div className="text-xs font-medium text-foreground">
                正在下载更新...
              </div>
              <div className="w-full bg-muted rounded-full h-1.5">
                <div
                  className="bg-blue-500 h-1.5 rounded-full transition-all duration-300"
                  style={{ width: `${downloadProgress}%` }}
                />
              </div>
              <div className="text-xs text-muted-foreground text-right">
                {downloadProgress}%
              </div>
            </>
          )}

          {/* Installing */}
          {status === "installing" && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <RefreshCw className="w-3.5 h-3.5 animate-spin shrink-0" />
              <span>正在安装，即将重启...</span>
            </div>
          )}

          {/* Error */}
          {status === "error" && (
            <>
              <div className="text-xs text-destructive">更新检查失败</div>
              {errorMessage && (
                <div className="text-[10px] text-muted-foreground break-all line-clamp-2">
                  {errorMessage}
                </div>
              )}
              <button
                onClick={() => checkForUpdate()}
                className="text-xs text-blue-500 hover:text-blue-400"
              >
                重试
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
