import { forwardRef, useImperativeHandle, useState, useRef, useEffect } from "react";
import { Send, Square, ChevronDown, Bot } from "lucide-react";
import { DEFAULT_CHAT_PANE_ID, useChatStore } from "../../stores/chatStore";
import { useAppStore } from "../../stores/appStore";
import { ModelSelector } from "./ModelSelector";
import { api } from "../../services/api";
import { OmpMark } from "../layout/ProviderMarks";
export interface ChatInputHandle {
  /** Insert the given text as a markdown blockquote at the current cursor position, focusing the textarea. */
  insertQuote: (text: string) => void;
  focus: () => void;
}

interface Props {
  paneId?: string;
  onSend: (prompt: string) => void;
  onCancel: () => void;
  isStreaming: boolean;
  disabled: boolean;
}

/** Derive a short display name from a full model ID. */
function shortModelName(id: string): string {
  return id
    .replace(/-\d{8}$/, "")
    .replace(/^claude-/, "");
}

export const ChatInput = forwardRef<ChatInputHandle, Props>(function ChatInput({
  paneId = DEFAULT_CHAT_PANE_ID,
  onSend,
  onCancel,
  isStreaming,
  disabled,
}, ref) {
  const [text, setText] = useState("");
  const [modelSelectorOpen, setModelSelectorOpen] = useState(false);
  const [hint, setHint] = useState<{ kind: "info" | "error"; text: string } | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const hintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const model = useChatStore((state) => state.getPaneState(paneId).model);
  const source = useChatStore((state) => state.getPaneState(paneId).source);
  const setPaneModel = useChatStore((state) => state.setPaneModel);
  const setActivePane = useChatStore((state) => state.setActivePane);

  const showHint = (kind: "info" | "error", message: string) => {
    setHint({ kind, text: message });
    if (hintTimerRef.current) clearTimeout(hintTimerRef.current);
    hintTimerRef.current = setTimeout(() => setHint(null), 2500);
  };

  useEffect(() => {
    return () => {
      if (hintTimerRef.current) clearTimeout(hintTimerRef.current);
    };
  }, []);

  useImperativeHandle(ref, () => ({
    focus: () => {
      textareaRef.current?.focus();
    },
    insertQuote: (quote: string) => {
      if (!quote) return;
      const trimmed = quote.replace(/\r\n?/g, "\n").trim();
      if (!trimmed) return;
      const quoted = trimmed
        .split("\n")
        .map((line) => (line ? `> ${line}` : ">"))
        .join("\n");
      setText((prev) => {
        const prefix = prev && !prev.endsWith("\n") ? `${prev}\n\n` : prev;
        return `${prefix}${quoted}\n\n`;
      });
      setActivePane(paneId);
      requestAnimationFrame(() => {
        const el = textareaRef.current;
        if (!el) return;
        el.focus();
        const end = el.value.length;
        el.setSelectionRange(end, end);
        el.scrollTop = el.scrollHeight;
      });
    },
  }), [paneId, setActivePane]);

  useEffect(() => {
    if (!isStreaming && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isStreaming]);

  const openModelSelector = () => {
    setActivePane(paneId);
    setModelSelectorOpen(true);
  };

  const handleSubmit = () => {
    const trimmed = text.trim();
    if (!trimmed || isStreaming) return;

    // Slash command: /model [id]
    if (trimmed.startsWith("/model")) {
      const arg = trimmed.slice(6).trim();
      if (arg) {
        setPaneModel(paneId, arg);
      } else {
        setModelSelectorOpen(true);
      }
      setText("");
      return;
    }

    // Slash command: /rename <new alias>  (empty arg = clear alias)
    if (trimmed === "/rename" || trimmed.startsWith("/rename ")) {
      const newAlias = trimmed.slice(7).trim();
      const pane = useChatStore.getState().getPaneState(paneId);
      if (!pane.sessionId) {
        showHint("error", "/rename 需要先有活动的 session");
        return;
      }
      if (!pane.projectPath) {
        showHint("error", "/rename 需要工作目录");
        return;
      }
      const aliasArg = newAlias.length > 0 ? newAlias : null;
      api
        .renameChatSession(pane.source, pane.projectPath, pane.sessionId, aliasArg)
        .then(() => {
          showHint(
            "info",
            aliasArg ? `已重命名为：${aliasArg}` : "已清空别名",
          );
          // Trigger silent refresh so sessions list reflects new alias
          void useAppStore.getState().refreshInBackground(true);
        })
        .catch((e: unknown) => {
          showHint("error", e instanceof Error ? e.message : String(e));
        });
      setText("");
      return;
    }

    if (disabled) return;
    onSend(trimmed);
    setText("");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      openModelSelector();
      return;
    }

    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 200) + "px";
  }, [text]);

  const modelDisplay = model ? shortModelName(model) : "选择模型";

  return (
    <>
      <div className="border-t border-border bg-card px-4 py-3">
        {/* Model selector row */}
        <div className="flex items-center gap-2 mb-2">
          <button
            onClick={openModelSelector}
            disabled={isStreaming}
            className="flex items-center gap-1.5 px-2 py-1 text-xs rounded-md border border-border bg-muted hover:bg-accent/50 transition-colors disabled:opacity-50"
            title={model || "选择模型 (Ctrl+K)"}
          >
            {source === "omp" ? <OmpMark className="w-3.5 h-3.5" /> : <Bot className={`w-3 h-3 ${source === "codex" ? "text-green-500" : "text-orange-500"}`} />}
            <span className="max-w-[12rem] truncate text-foreground">{modelDisplay}</span>
            <ChevronDown className="w-3 h-3 text-muted-foreground" />
          </button>
          <span className="text-[10px] text-muted-foreground">
            Ctrl+K 或 /model 切换 · /rename &lt;名字&gt; 改别名
          </span>
          {hint && (
            <span
              className={`ml-auto text-[10px] truncate max-w-[40%] ${
                hint.kind === "error" ? "text-red-400" : "text-emerald-500"
              }`}
              title={hint.text}
            >
              {hint.text}
            </span>
          )}
        </div>

        {/* Input row */}
        <div className="flex items-end gap-2">
          <textarea
            ref={textareaRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={handleKeyDown}
            onFocus={() => setActivePane(paneId)}
            placeholder={
              isStreaming
                ? "等待响应中..."
                : disabled
                  ? "请先选择工作目录"
                  : "输入消息... (Enter 发送, Shift+Enter 换行)"
            }
            disabled={isStreaming || disabled}
            rows={1}
            className="flex-1 bg-muted border border-border rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground resize-none focus:outline-none focus:ring-1 focus:ring-primary disabled:opacity-50"
          />
          {isStreaming ? (
            <button
              onClick={onCancel}
              className="shrink-0 p-2 rounded-lg bg-red-500 text-white hover:bg-red-600 transition-colors"
              title="停止生成"
            >
              <Square className="w-4 h-4" />
            </button>
          ) : (
            <button
              onClick={handleSubmit}
              disabled={!text.trim() || disabled}
              className="shrink-0 p-2 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              title="发送消息"
            >
              <Send className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Model Selector modal */}
      <ModelSelector
        paneId={paneId}
        open={modelSelectorOpen}
        onClose={() => setModelSelectorOpen(false)}
        onSelect={(m) => setPaneModel(paneId, m)}
      />
    </>
  );
});
