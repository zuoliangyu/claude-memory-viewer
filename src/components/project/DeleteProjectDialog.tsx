import { useState } from "react";
import { Loader2 } from "lucide-react";
import type { ProjectEntry } from "../../types";
import type { DeleteLevel } from "../../types";

interface DeleteProjectDialogProps {
  project: ProjectEntry;
  /** 数据源；非 claude 时隐藏「清理 CC 配置」选项。 */
  source?: string;
  onConfirm: (level: DeleteLevel) => Promise<void>;
  onCancel: () => void;
}

export function DeleteProjectDialog({
  project,
  source = "claude",
  onConfirm,
  onCancel,
}: DeleteProjectDialogProps) {
  const [deleting, setDeleting] = useState(false);
  const [withCcConfig, setWithCcConfig] = useState(false);

  const handleConfirm = async () => {
    setDeleting(true);
    try {
      await onConfirm(withCcConfig ? "withCcConfig" : "sessionOnly");
    } catch (err) {
      console.error("Failed to delete project:", err);
    } finally {
      setDeleting(false);
    }
  };

  const displayName = project.alias ?? project.shortName;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-card border border-border rounded-lg p-6 max-w-sm w-full mx-4 shadow-lg">
        <h3 className="text-lg font-semibold mb-2">确认删除工程</h3>
        <p className="text-sm text-muted-foreground mb-1">
          工程：<span className="font-medium text-foreground">{displayName}</span>
        </p>
        <p className="text-xs text-muted-foreground mb-4">
          将删除 {project.sessionCount} 个会话记录
        </p>

        {/* Level 2 复选框（仅 claude 有 CC 配置） */}
        {source === "claude" && (
        <label className="flex items-start gap-2 cursor-pointer mb-4 group">
          <input
            type="checkbox"
            checked={withCcConfig}
            onChange={(e) => setWithCcConfig(e.target.checked)}
            disabled={deleting}
            className="mt-0.5 accent-destructive"
          />
          <span className="text-xs text-muted-foreground leading-relaxed">
            同时清理 Claude Code 项目配置
            {withCcConfig && (
              <span className="block mt-1 text-yellow-600 dark:text-yellow-400">
                将从 ~/.claude.json 移除该项目配置，下次进入该目录等于全新项目
              </span>
            )}
          </span>
        </label>
        )}

        <div className="flex justify-end gap-2">
          <button
            onClick={onCancel}
            disabled={deleting}
            className="px-4 py-2 text-sm rounded-md border border-border hover:bg-accent transition-colors"
          >
            取消
          </button>
          <button
            onClick={handleConfirm}
            disabled={deleting}
            className="px-4 py-2 text-sm rounded-md bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors flex items-center gap-1.5 disabled:opacity-50"
          >
            {deleting ? (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                删除中...
              </>
            ) : (
              "删除"
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
