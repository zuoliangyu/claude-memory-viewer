import { useEffect, useRef, useState } from "react";
import ReactDOM from "react-dom";
import { Copy, Check, Pencil, Trash2 } from "lucide-react";
import type { ProjectEntry } from "../../types";

interface ProjectActionsMenuProps {
  project: ProjectEntry;
  source: string;
  anchorRect: DOMRect;
  onClose: () => void;
  onRename: (project: ProjectEntry) => void;
  onDelete: (project: ProjectEntry) => void;
}

export function ProjectActionsMenu({
  project,
  source,
  anchorRect,
  onClose,
  onRename,
  onDelete,
}: ProjectActionsMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);

  // 关闭：点击外部或 Escape
  useEffect(() => {
    const handleMouseDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", handleMouseDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleMouseDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  // 定位：基于 anchorRect，自动翻转防止溢出视口
  const menuWidth = 220;
  const menuHeight = 120;
  let left = anchorRect.right - menuWidth;
  let top = anchorRect.bottom + 4;
  if (left < 8) left = 8;
  if (top + menuHeight > window.innerHeight - 8) {
    top = anchorRect.top - menuHeight - 4;
  }

  const handleCopy = () => {
    if (!navigator.clipboard) return;
    navigator.clipboard.writeText(project.displayPath).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 300);
    }).catch(() => {/* 静默降级 */});
  };

  return ReactDOM.createPortal(
    <div
      ref={menuRef}
      className="fixed z-50 bg-card border border-border rounded-lg shadow-lg py-1"
      style={{ left, top, width: menuWidth }}
    >
      {/* 路径行 */}
      <div className="px-3 py-2 border-b border-border flex items-start gap-2">
        <p className="text-xs text-muted-foreground break-all max-h-20 overflow-y-auto flex-1">
          {project.displayPath}
        </p>
        <button
          onClick={handleCopy}
          className="shrink-0 p-0.5 rounded text-muted-foreground hover:text-foreground transition-colors"
          title="复制路径"
        >
          {copied
            ? <Check className="w-3.5 h-3.5 text-green-500" />
            : <Copy className="w-3.5 h-3.5" />}
        </button>
      </div>

      {/* 重命名（仅 claude） */}
      {source === "claude" && (
        <button
          onClick={() => { onRename(project); onClose(); }}
          className="w-full text-left px-3 py-2 text-sm text-foreground hover:bg-accent/50 transition-colors flex items-center gap-2"
        >
          <Pencil className="w-3.5 h-3.5" />
          设置别名
        </button>
      )}

      {/* 所有本地会话源均支持删除会话数据 */}
      <button
        onClick={() => { onDelete(project); onClose(); }}
        className="w-full text-left px-3 py-2 text-sm text-destructive hover:bg-destructive/10 transition-colors flex items-center gap-2"
      >
        <Trash2 className="w-3.5 h-3.5" />
        删除会话数据
      </button>

    </div>,
    document.body
  );
}
