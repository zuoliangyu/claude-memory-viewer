import { memo, useMemo, useState } from "react";
import type { DisplayMessage } from "../../types";
import { Bot, ChevronDown, ChevronRight, Wrench, Brain, Copy, Check } from "lucide-react";
import { formatTime, cleanMessageText, stripAnsi, copyTextToClipboard } from "./utils";
import { ToolViewer } from "../chat/tool-viewers/ToolViewers";
import { MarkdownContent } from "./MarkdownContent";
import { useExpandAllControl } from "../common/ExpandAllContext";
import { useAppStore } from "../../stores/appStore";
import { OmpMark } from "../layout/ProviderMarks";

interface Props {
  message: DisplayMessage;
  source: string;
  showTimestamp: boolean;
  showModel: boolean;
  threadAnchor?: string | null;
  threadHint?: string | null;
  layout?: "default" | "thread";
}

const MARKDOWN_CLASS_NAME = "prose prose-sm max-w-none p-0 text-sm leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0";

export const AssistantMessage = memo(function AssistantMessage({
  message,
  source,
  showTimestamp,
  showModel,
  threadAnchor,
  threadHint,
  layout = "default",
}: Props) {
  const timeZone = useAppStore((state) => state.timeZone);
  const assistantName = source === "codex" ? "Codex" : source === "omp" ? "Oh My Pi" : "Claude";
  const iconColor = source === "codex" ? "text-green-500" : source === "omp" ? "text-fuchsia-500" : "text-orange-500";
  const iconBg = source === "codex" ? "bg-green-500/10" : source === "omp" ? "bg-fuchsia-500/10" : "bg-orange-500/10";
  const [copied, setCopied] = useState(false);
  const { expanded: messageExpanded, setExpanded: setMessageExpanded } = useExpandAllControl(true, { followGlobal: true });
  const textContent = useMemo(
    () => message.content
      .filter((block): block is { type: "text"; text: string } => block.type === "text")
      .map((block) => cleanMessageText(block.text))
      .filter(Boolean),
    [message.content]
  );
  const hasTextContent = textContent.length > 0;
  const copyText = useMemo(
    () => textContent.join("\n\n").trim(),
    [textContent]
  );
  const previewText = useMemo(() => {
    const raw = copyText.replace(/\s+/g, " ").trim();
    if (raw) {
      return raw.length > 140 ? `${raw.slice(0, 140)}…` : raw;
    }
    const hasTool = message.content.some((b) => b.type === "tool_use" || b.type === "function_call");
    if (hasTool) return "（工具调用）";
    const hasThinking = message.content.some((b) => b.type === "thinking" || b.type === "reasoning");
    if (hasThinking) return "（思考过程）";
    return "（回复内容）";
  }, [copyText, message.content]);

  const handleCopy = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (!copyText) return;
    const ok = await copyTextToClipboard(copyText);
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const isThreadLayout = layout === "thread";
  const metaContent = (
    <>
      <button
        onClick={() => setMessageExpanded(!messageExpanded)}
        className="inline-flex items-center gap-1 rounded-md px-1 py-0.5 text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
        title={messageExpanded ? "折叠此回复" : "展开此回复"}
      >
        {messageExpanded ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
      </button>
      <span className="text-sm font-medium">{assistantName}</span>
      {threadAnchor && (
        <span className="rounded-full border border-border bg-muted/60 px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
          {threadAnchor}
        </span>
      )}
      {showModel && message.model && (
        <span className="rounded bg-muted/50 px-1.5 py-0.5 text-xs text-muted-foreground">
          {message.model}
        </span>
      )}
      {showTimestamp && message.timestamp && (
        <span className="text-xs text-muted-foreground">
          {formatTime(message.timestamp, timeZone)}
        </span>
      )}
    </>
  );
  const copyButton = hasTextContent ? (
    <button
      onClick={handleCopy}
      className="ml-auto inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      title="复制消息"
    >
      {copied ? (
        <>
          <Check className="h-3.5 w-3.5 text-green-500" />
          已复制
        </>
      ) : (
        <>
          <Copy className="h-3.5 w-3.5" />
          复制文本
        </>
      )}
    </button>
  ) : null;

  return (
    <div className={`group/assistant ${isThreadLayout ? "w-full" : "flex gap-3"}`}>
      {!isThreadLayout && (
        <div className={`mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full ${iconBg}`}>
          {source === "omp" ? <OmpMark className="h-4 w-4" /> : <Bot className={`h-3.5 w-3.5 ${iconColor}`} />}
        </div>
      )}
      <div className="min-w-0 flex-1">
        <div className={`mb-1 flex gap-2 ${isThreadLayout ? "items-center flex-wrap" : "items-baseline"}`}>
          {metaContent}
          {copyButton}
        </div>
        {threadHint && (
          <div className="mb-2 text-[11px] text-muted-foreground">
            {threadHint}
          </div>
        )}
        {!messageExpanded ? (
          <button
            onClick={() => setMessageExpanded(true)}
            className="block w-full rounded-xl border border-dashed border-border bg-muted/30 px-3 py-1.5 text-left text-xs text-muted-foreground hover:bg-muted/60 transition-colors"
            title="展开此回复"
          >
            <span className="line-clamp-2">{previewText}</span>
          </button>
        ) : message.content.map((block, i) => {
          if (block.type === "text") {
            const cleaned = cleanMessageText(block.text);
            if (!cleaned) return null;
            return (
              <MarkdownContent
                key={i}
                content={cleaned}
                className={MARKDOWN_CLASS_NAME}
              />
            );
          }
          if (block.type === "thinking") {
            return <ThinkingBlock key={i} thinking={block.thinking} layout={layout} />;
          }
          if (block.type === "reasoning") {
            return <ReasoningBlock key={i} text={block.text} layout={layout} />;
          }
          if (block.type === "tool_use") {
            return (
              <ToolViewer
                key={i}
                name={block.name}
                input={block.input}
              />
            );
          }
          if (block.type === "function_call") {
            return (
              <FunctionCallBlock
                key={i}
                name={block.name}
                arguments={block.arguments}
              />
            );
          }
          return null;
        })}
      </div>
    </div>
  );
}, (prevProps, nextProps) => (
  prevProps.message === nextProps.message &&
  prevProps.source === nextProps.source &&
  prevProps.showTimestamp === nextProps.showTimestamp &&
  prevProps.showModel === nextProps.showModel &&
  prevProps.threadAnchor === nextProps.threadAnchor &&
  prevProps.threadHint === nextProps.threadHint &&
  prevProps.layout === nextProps.layout
));

function ThinkingBlock({ thinking, layout = "default" }: { thinking: string; layout?: "default" | "thread" }) {
  const { expanded, setExpanded } = useExpandAllControl(false);
  const isThreadLayout = layout === "thread";

  return (
    <div className="mt-2 mb-2">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        <Brain className="w-3.5 h-3.5 shrink-0" />
        思考过程
        {expanded ? (
          <ChevronDown className="w-3 h-3" />
        ) : (
          <ChevronRight className="w-3 h-3" />
        )}
      </button>
      {expanded && (
        <div className={`mt-1 whitespace-pre-wrap text-xs text-muted-foreground ${
          isThreadLayout
            ? "rounded-md border border-border/60 bg-muted/15 px-2.5 py-2"
            : "border-l-2 border-muted pl-5"
        }`}>
          {thinking}
        </div>
      )}
    </div>
  );
}

function ReasoningBlock({ text, layout = "default" }: { text: string; layout?: "default" | "thread" }) {
  const { expanded, setExpanded } = useExpandAllControl(true);
  const isThreadLayout = layout === "thread";

  return (
    <div className="mt-2 mb-2">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        <Brain className="w-3.5 h-3.5 shrink-0" />
        推理过程
        {expanded ? (
          <ChevronDown className="w-3 h-3" />
        ) : (
          <ChevronRight className="w-3 h-3" />
        )}
      </button>
      {expanded && (
        <div className={`mt-1 whitespace-pre-wrap text-xs text-muted-foreground ${
          isThreadLayout
            ? "rounded-md border border-border/60 bg-muted/15 px-2.5 py-2"
            : "border-l-2 border-muted pl-5"
        }`}>
          {text}
        </div>
      )}
    </div>
  );
}

function FunctionCallBlock({ name, arguments: args }: { name: string; arguments: string }) {
  const { expanded, setExpanded } = useExpandAllControl(true);
  const cleanedArgs = useMemo(() => {
    const cleaned = stripAnsi(args);
    return cleaned.length > 5000
      ? cleaned.slice(0, 5000) + "\n... (truncated)"
      : cleaned;
  }, [args]);

  return (
    <div className="mt-2 mb-2 border border-border rounded-md overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs bg-muted/50 hover:bg-muted transition-colors"
      >
        <Wrench className="w-3.5 h-3.5 text-green-500" />
        <span className="font-mono font-medium">{name}</span>
        {expanded ? (
          <ChevronDown className="w-3 h-3 ml-auto" />
        ) : (
          <ChevronRight className="w-3 h-3 ml-auto" />
        )}
      </button>
      {expanded && (
        <div className="p-3 text-xs font-mono bg-muted/20 overflow-x-auto">
          <pre className="whitespace-pre-wrap break-all">{cleanedArgs}</pre>
        </div>
      )}
    </div>
  );
}
