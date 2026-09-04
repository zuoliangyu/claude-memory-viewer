import { memo } from "react";
import type { ChatMessage, ChatContentBlock } from "../../types/chat";
import {
  Bot,
  ChevronDown,
  ChevronRight,
  Brain,
  Settings,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { ToolViewer, normalizeToolName } from "./tool-viewers/ToolViewers";
import { cleanMessageText, wrapAsciiArt } from "../message/utils";
import { useExpandAllControl } from "../common/ExpandAllContext";
import { useAppStore } from "../../stores/appStore";
import { formatDateTime } from "../../utils/dateTime";
import { OmpMark } from "../layout/ProviderMarks";

interface Props {
  message: ChatMessage;
  source?: string;
  showTimestamp?: boolean;
  showModel?: boolean;
  /** Map from tool_use id → matching tool_result (for linked rendering) */
  toolResultMap?: Map<string, { content: string; isError: boolean }>;
  /** Set of tool_result IDs already rendered via a visible linked tool_use (to skip) */
  linkedToolUseIds?: Set<string>;
  onSubmitAnswers?: (answers: string) => void;
  interactiveQuestions?: boolean;
}

type ToolResultData = { content: string; isError: boolean };

function getRelevantToolResultIds(message: ChatMessage): string[] {
  const ids: string[] = [];

  for (const block of message.content) {
    if (block.type === "tool_use" && block.id) {
      ids.push(block.id);
      continue;
    }

    if (block.type === "tool_result" && block.toolUseId) {
      ids.push(block.toolUseId);
    }
  }

  return ids;
}

function areToolResultMapsEqualForMessage(
  message: ChatMessage,
  prevMap?: Map<string, ToolResultData>,
  nextMap?: Map<string, ToolResultData>
) {
  const relevantIds = getRelevantToolResultIds(message);
  if (relevantIds.length === 0) {
    return true;
  }

  for (const id of relevantIds) {
    const prevResult = prevMap?.get(id);
    const nextResult = nextMap?.get(id);
    if (
      prevResult?.content !== nextResult?.content ||
      prevResult?.isError !== nextResult?.isError
    ) {
      return false;
    }
  }

  return true;
}

function areLinkedToolUseIdsEqualForMessage(
  message: ChatMessage,
  prevLinkedIds?: Set<string>,
  nextLinkedIds?: Set<string>
) {
  if (message.role !== "user") {
    return true;
  }

  for (const block of message.content) {
    if (block.type !== "tool_result") {
      continue;
    }

    if (prevLinkedIds?.has(block.toolUseId) !== nextLinkedIds?.has(block.toolUseId)) {
      return false;
    }
  }

  return true;
}

function areStreamingMessagePropsEqual(prevProps: Props, nextProps: Props) {
  if (prevProps.message !== nextProps.message) {
    return false;
  }

  if (
    prevProps.source !== nextProps.source ||
    prevProps.showTimestamp !== nextProps.showTimestamp ||
    prevProps.showModel !== nextProps.showModel ||
    prevProps.onSubmitAnswers !== nextProps.onSubmitAnswers ||
    prevProps.interactiveQuestions !== nextProps.interactiveQuestions
  ) {
    return false;
  }

  if (!areToolResultMapsEqualForMessage(prevProps.message, prevProps.toolResultMap, nextProps.toolResultMap)) {
    return false;
  }

  if (!areLinkedToolUseIdsEqualForMessage(prevProps.message, prevProps.linkedToolUseIds, nextProps.linkedToolUseIds)) {
    return false;
  }

  return true;
}

/**
 * Renders a streaming chat message with specialized tool viewers.
 * When toolResultMap is provided, tool_use blocks are rendered with their
 * matching results in a unified view (Read→code, Edit→diff, Bash→terminal).
 */
export const StreamingMessage = memo(function StreamingMessage({
  message,
  source = "claude",
  showTimestamp,
  showModel,
  toolResultMap,
  linkedToolUseIds,
  onSubmitAnswers,
  interactiveQuestions,
}: Props) {
  const timeZone = useAppStore((state) => state.timeZone);
  if (message.role === "system") {
    return <SystemMsg message={message} />;
  }
  if (message.role === "user") {
    return (
      <UserMsg
        message={message}
        showTimestamp={showTimestamp}
        timeZone={timeZone}
        linkedToolUseIds={linkedToolUseIds}
      />
    );
  }
  // assistant + tool
  return (
    <AssistantMsg
      message={message}
      source={source}
      showTimestamp={showTimestamp}
      timeZone={timeZone}
      showModel={showModel}
      toolResultMap={toolResultMap}
      onSubmitAnswers={onSubmitAnswers}
      interactiveQuestions={interactiveQuestions}
    />
  );
}, areStreamingMessagePropsEqual);

function isBashToolName(name: string): boolean {
  return normalizeToolName(name) === "Bash";
}

function getToolDisplayInput(block: Extract<ChatContentBlock, { type: "tool_use" }>): string {
  if (!isBashToolName(block.name)) {
    return block.input.trim();
  }

  try {
    const parsed = JSON.parse(block.input) as { command?: unknown };
    if (typeof parsed?.command === "string") {
      return parsed.command.trim();
    }
  } catch {
    // Some command events already pass plain text input.
  }

  return block.input.trim();
}

function getToolDisplayState(
  blocks: ChatContentBlock[],
  toolResultMap?: Map<string, ToolResultData>
) {
  const deduped: ChatContentBlock[] = [];
  const linkedIds = new Set<string>();

  for (const block of blocks) {
    if (block.type !== "tool_use") {
      deduped.push(block);
      continue;
    }

    const previous = deduped[deduped.length - 1];
    if (previous?.type !== "tool_use") {
      deduped.push(block);
      continue;
    }

    if (!isBashToolName(block.name) || !isBashToolName(previous.name)) {
      deduped.push(block);
      continue;
    }

    const currentInput = getToolDisplayInput(block);
    const previousInput = getToolDisplayInput(previous);
    if (currentInput !== previousInput) {
      deduped.push(block);
      continue;
    }

    const currentResult = toolResultMap?.get(block.id);
    const previousResult = toolResultMap?.get(previous.id);
    const currentHasResult = currentResult !== undefined;
    const previousHasResult = previousResult !== undefined;
    const sameResult =
      currentResult?.content === previousResult?.content &&
      currentResult?.isError === previousResult?.isError;

    if (sameResult) {
      if (previousHasResult) {
        linkedIds.add(previous.id);
      }
      if (currentHasResult) {
        linkedIds.add(block.id);
      }
      continue;
    }

    if (!currentHasResult && !previousHasResult) {
      continue;
    }

    if (!currentHasResult && previousHasResult) {
      linkedIds.add(previous.id);
      continue;
    }

    if (currentHasResult && !previousHasResult) {
      deduped[deduped.length - 1] = block;
      linkedIds.add(block.id);
      continue;
    }

    deduped.push(block);
  }

  if (toolResultMap) {
    for (const block of deduped) {
      if (block.type === "tool_use" && toolResultMap.has(block.id)) {
        linkedIds.add(block.id);
      }
    }
  }

  return { blocks: deduped, linkedToolUseIds: linkedIds };
}

export function getLinkedToolUseIds(
  message: ChatMessage,
  toolResultMap?: Map<string, ToolResultData>
): string[] {
  if (message.role !== "assistant" || !toolResultMap) {
    return [];
  }

  return Array.from(getToolDisplayState(message.content, toolResultMap).linkedToolUseIds);
}

/* ── System (result / info) ─────────────────────────────── */

function SystemMsg({ message }: { message: ChatMessage }) {
  const text = message.content.map((b) => (b.type === "text" ? b.text : "")).join("");
  return (
    <div className="flex items-center justify-center gap-2 py-2">
      <Settings className="w-3 h-3 text-muted-foreground" />
      <span className="text-xs text-muted-foreground">{text}</span>
    </div>
  );
}

/* ── User message ──── */

function UserMsg({
  message,
  showTimestamp,
  linkedToolUseIds,
  timeZone,
}: {
  message: ChatMessage;
  showTimestamp?: boolean;
  linkedToolUseIds?: Set<string>;
  timeZone: string;
}) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[85%]">
        {showTimestamp && message.timestamp && (
          <div className="flex items-center justify-end gap-2 mb-1">
            <span className="text-xs text-muted-foreground">
              {formatDateTime(message.timestamp, timeZone)}
            </span>
          </div>
        )}
        {message.content.map((block, i) => {
          if (block.type === "text") {
            return (
              <div key={i} className="bg-primary/10 rounded-2xl px-4 py-2.5 text-sm leading-relaxed">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                  {wrapAsciiArt(cleanMessageText(block.text))}
                </ReactMarkdown>
              </div>
            );
          }
          if (block.type === "tool_result") {
            // Skip if this result is already rendered via linked tool_use
            if (linkedToolUseIds?.has(block.toolUseId)) return null;

            return (
              <div
                key={i}
                className={`mt-2 text-xs rounded-md p-3 font-mono overflow-x-auto ${
                  block.isError
                    ? "bg-destructive/10 text-destructive border border-destructive/20"
                    : "bg-muted text-muted-foreground"
                }`}
              >
                <pre className="whitespace-pre-wrap break-all">
                  {block.content.length > 2000
                    ? block.content.slice(0, 2000) + "\n... (truncated)"
                    : block.content}
                </pre>
              </div>
            );
          }
          return null;
        })}
      </div>
    </div>
  );
}
function AssistantMsg({
  message,
  source,
  showTimestamp,
  showModel,
  toolResultMap,
  onSubmitAnswers,
  interactiveQuestions,
  timeZone,
}: {
  message: ChatMessage;
  source: string;
  showTimestamp?: boolean;
  showModel?: boolean;
  toolResultMap?: Map<string, ToolResultData>;
  onSubmitAnswers?: (answers: string) => void;
  interactiveQuestions?: boolean;
  timeZone: string;
}) {
  const { blocks: displayBlocks } = getToolDisplayState(message.content, toolResultMap);
  const assistantName = source === "codex" ? "Codex" : source === "omp" ? "Oh My Pi" : "Claude";
  const iconColor = source === "codex" ? "text-green-500" : source === "omp" ? "text-fuchsia-500" : "text-orange-500";
  const iconBg = source === "codex" ? "bg-green-500/10" : source === "omp" ? "bg-fuchsia-500/10" : "bg-orange-500/10";

  return (
    <div className="flex gap-3">
      <div className={`shrink-0 w-7 h-7 rounded-full ${iconBg} flex items-center justify-center mt-0.5`}>
        {source === "omp" ? <OmpMark className="w-4 h-4" /> : <Bot className={`w-3.5 h-3.5 ${iconColor}`} />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2 mb-1">
          <span className="text-sm font-medium">{assistantName}</span>
          {showModel && message.model && (
            <span className="text-xs text-muted-foreground bg-muted/50 px-1.5 py-0.5 rounded">
              {message.model}
            </span>
          )}
          {showTimestamp && message.timestamp && (
            <span className="text-xs text-muted-foreground">
              {formatDateTime(message.timestamp, timeZone)}
            </span>
          )}
          {message.usage && (
            <span
              className="text-xs text-muted-foreground tabular-nums"
              title={[
                `输入: ${message.usage.inputTokens.toLocaleString()}`,
                `输出: ${message.usage.outputTokens.toLocaleString()}`,
                message.usage.cacheCreationInputTokens > 0
                  ? `写入缓存: ${message.usage.cacheCreationInputTokens.toLocaleString()}`
                  : "",
                message.usage.cacheReadInputTokens > 0
                  ? `读取缓存: ${message.usage.cacheReadInputTokens.toLocaleString()}`
                  : "",
              ].filter(Boolean).join(" · ")}
            >
              入{message.usage.inputTokens.toLocaleString()} 出{message.usage.outputTokens.toLocaleString()}
              {message.usage.cacheReadInputTokens > 0 && ` 缓存${message.usage.cacheReadInputTokens.toLocaleString()}`}
            </span>
          )}
        </div>
        {displayBlocks.map((block, i) => (
          <ContentBlockRenderer
            key={i}
            block={block}
            toolResultMap={toolResultMap}
            onSubmitAnswers={onSubmitAnswers}
            interactiveQuestions={interactiveQuestions}
          />
        ))}
      </div>
    </div>
  );
}

/* ── Content block renderers ── */

function ContentBlockRenderer({
  block,
  toolResultMap,
  onSubmitAnswers,
  interactiveQuestions,
}: {
  block: ChatContentBlock;
  toolResultMap?: Map<string, ToolResultData>;
  onSubmitAnswers?: (answers: string) => void;
  interactiveQuestions?: boolean;
}) {
  if (block.type === "text") {
    return <TextBlock text={block.text} />;
  }

  if (block.type === "thinking") {
    return <ThinkingBlock text={block.text} />;
  }

  if (block.type === "tool_use") {
    const result = toolResultMap?.get(block.id) ?? null;
    return (
      <ToolViewer
        name={normalizeToolName(block.name)}
        input={block.input}
        result={result}
        onSubmitAnswers={onSubmitAnswers}
        interactive={interactiveQuestions}
      />
    );
  }

  if (block.type === "tool_result") {
    // In assistant messages, tool_results are rare but possible
    return <FallbackToolResult content={block.content} isError={block.isError} />;
  }

  return null;
}

/* ── Text block with markdown ── */

function TextBlock({ text }: { text: string }) {
  return (
    <div className="prose prose-sm max-w-none text-sm leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          code({ className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || "");
            const codeStr = String(children).replace(/\n$/, "");
            if (match) {
              return (
                <SyntaxHighlighter
                  style={oneDark}
                  language={match[1]}
                  PreTag="div"
                  className="rounded-md text-xs !mt-2 !mb-2"
                >
                  {codeStr}
                </SyntaxHighlighter>
              );
            }
            return (
              <code className="bg-muted px-1.5 py-0.5 rounded text-xs font-mono" {...props}>
                {children}
              </code>
            );
          },
          pre({ children }) {
            return (
              <div className="not-prose my-2">
                <pre className="rounded-md bg-muted border border-border p-3 text-xs font-mono overflow-x-auto [&>code]:bg-transparent [&>code]:p-0 [&>code]:rounded-none">
                  {children}
                </pre>
              </div>
            );
          },
          a({ href, children }) {
            return (
              <a href={href} target="_blank" rel="noopener noreferrer" className="text-blue-400 hover:text-blue-300 underline underline-offset-2">
                {children}
              </a>
            );
          },
          ul({ children }) {
            return <ul className="list-disc pl-5 my-2 space-y-0.5">{children}</ul>;
          },
          ol({ children }) {
            return <ol className="list-decimal pl-5 my-2 space-y-0.5">{children}</ol>;
          },
          li({ children }) {
            return <li className="text-sm">{children}</li>;
          },
          h1({ children }) {
            return <h1 className="text-lg font-bold mt-4 mb-2">{children}</h1>;
          },
          h2({ children }) {
            return <h2 className="text-base font-bold mt-3 mb-2">{children}</h2>;
          },
          h3({ children }) {
            return <h3 className="text-sm font-bold mt-3 mb-1">{children}</h3>;
          },
          blockquote({ children }) {
            return (
              <blockquote className="border-l-2 border-border pl-3 my-2 text-muted-foreground italic">
                {children}
              </blockquote>
            );
          },
          hr() {
            return <hr className="border-border my-4" />;
          },
          p({ children }) {
            return <p className="my-2 leading-relaxed">{children}</p>;
          },
        }}
      >
        {wrapAsciiArt(cleanMessageText(text))}
      </ReactMarkdown>
    </div>
  );
}

/* ── Thinking block ── */

function ThinkingBlock({ text }: { text: string }) {
  const { expanded, setExpanded } = useExpandAllControl(true);
  return (
    <div className="mt-2 mb-2">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        <Brain className="w-3.5 h-3.5 shrink-0" />
        思考过程
        {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
      </button>
      {expanded && (
        <div className="mt-1 pl-5 text-xs text-muted-foreground whitespace-pre-wrap border-l-2 border-muted">
          {text}
        </div>
      )}
    </div>
  );
}

/* ── Fallback tool result (for edge cases) ── */

function FallbackToolResult({ content, isError }: { content: string; isError: boolean }) {
  const { expanded, setExpanded } = useExpandAllControl(false);
  const isLong = content.length > 300;

  if (!isLong) {
    return (
      <div
        className={`mt-2 text-xs rounded-md p-3 font-mono overflow-x-auto ${
          isError
            ? "bg-destructive/10 text-destructive border border-destructive/20"
            : "bg-muted text-muted-foreground"
        }`}
      >
        <pre className="whitespace-pre-wrap break-all">{content}</pre>
      </div>
    );
  }

  return (
    <div className={`mt-2 border rounded-md overflow-hidden ${isError ? "border-destructive/20" : "border-border"}`}>
      <button
        onClick={() => setExpanded(!expanded)}
        className={`w-full flex items-center gap-2 px-3 py-1.5 text-xs ${isError ? "bg-destructive/5" : "bg-muted/30"} hover:bg-muted/50 transition-colors`}
      >
        <span className={isError ? "text-destructive" : "text-muted-foreground"}>
          {isError ? "Error" : "Result"} ({content.length} chars)
        </span>
        {expanded ? <ChevronDown className="w-3 h-3 ml-auto" /> : <ChevronRight className="w-3 h-3 ml-auto" />}
      </button>
      {expanded && (
        <div className="p-3 text-xs font-mono bg-muted/10 overflow-x-auto max-h-96 overflow-y-auto">
          <pre className="whitespace-pre-wrap break-all">
            {content.length > 10000 ? content.slice(0, 10000) + "\n... (truncated)" : content}
          </pre>
        </div>
      )}
    </div>
  );
}
