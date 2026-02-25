import { useState } from "react";
import type { ChatMessage, ChatContentBlock } from "../../types/chat";
import {
  Bot,
  ChevronDown,
  ChevronRight,
  Wrench,
  Brain,
  Settings,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";

interface Props {
  message: ChatMessage;
  source: string;
  showTimestamp?: boolean;
  showModel?: boolean;
}

/**
 * Renders a streaming chat message using the same visual style
 * as the existing MessageThread components (UserMessage / AssistantMessage).
 */
export function StreamingMessage({ message, source, showTimestamp, showModel }: Props) {
  if (message.role === "system") {
    return <SystemMsg message={message} />;
  }
  if (message.role === "user") {
    return <UserMsg message={message} showTimestamp={showTimestamp} />;
  }
  // assistant + tool
  return <AssistantMsg message={message} source={source} showTimestamp={showTimestamp} showModel={showModel} />;
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

/* ── User message (same as existing UserMessage style) ──── */

function UserMsg({ message, showTimestamp }: { message: ChatMessage; showTimestamp?: boolean }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[85%]">
        {showTimestamp && message.timestamp && (
          <div className="flex items-center justify-end gap-2 mb-1">
            <span className="text-xs text-muted-foreground">
              {formatTime(message.timestamp)}
            </span>
          </div>
        )}
        {message.content.map((block, i) => {
          if (block.type === "text") {
            return (
              <div key={i} className="bg-primary/10 rounded-2xl px-4 py-2.5 text-sm leading-relaxed">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                  {block.text}
                </ReactMarkdown>
              </div>
            );
          }
          if (block.type === "tool_result") {
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

/* ── Assistant message (same as existing AssistantMessage style) ── */

function AssistantMsg({
  message,
  source,
  showTimestamp,
  showModel,
}: {
  message: ChatMessage;
  source: string;
  showTimestamp?: boolean;
  showModel?: boolean;
}) {
  const assistantName = source === "codex" ? "Codex" : "Claude";
  const iconColor = source === "codex" ? "text-green-500" : "text-orange-500";
  const iconBg = source === "codex" ? "bg-green-500/10" : "bg-orange-500/10";

  return (
    <div className="flex gap-3">
      <div className={`shrink-0 w-7 h-7 rounded-full ${iconBg} flex items-center justify-center mt-0.5`}>
        <Bot className={`w-3.5 h-3.5 ${iconColor}`} />
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
              {formatTime(message.timestamp)}
            </span>
          )}
          {message.usage && (
            <span className="text-xs text-muted-foreground">
              {message.usage.inputTokens + message.usage.outputTokens} tokens
            </span>
          )}
        </div>
        {message.content.map((block, i) => (
          <ContentBlockRenderer key={i} block={block} />
        ))}
      </div>
    </div>
  );
}

/* ── Content block renderers (same as existing AssistantMessage) ── */

function ContentBlockRenderer({ block }: { block: ChatContentBlock }) {
  if (block.type === "text") {
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
              return <div className="not-prose my-2">{children}</div>;
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
          {block.text}
        </ReactMarkdown>
      </div>
    );
  }

  if (block.type === "thinking") {
    return <ThinkingBlock text={block.text} />;
  }

  if (block.type === "tool_use") {
    return <ToolCallBlock name={block.name} input={block.input} />;
  }

  if (block.type === "tool_result") {
    return <ToolResultBlock content={block.content} isError={block.isError} />;
  }

  return null;
}

function ThinkingBlock({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
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

function ToolCallBlock({ name, input }: { name: string; input: string }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <div className="mt-2 mb-2 border border-border rounded-md overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs bg-muted/50 hover:bg-muted transition-colors"
      >
        <Wrench className="w-3.5 h-3.5 text-muted-foreground" />
        <span className="font-mono font-medium">{name}</span>
        {expanded ? <ChevronDown className="w-3 h-3 ml-auto" /> : <ChevronRight className="w-3 h-3 ml-auto" />}
      </button>
      {expanded && (
        <div className="p-3 text-xs font-mono bg-muted/20 overflow-x-auto">
          <pre className="whitespace-pre-wrap break-all">
            {input.length > 5000 ? input.slice(0, 5000) + "\n... (truncated)" : input}
          </pre>
        </div>
      )}
    </div>
  );
}

function ToolResultBlock({ content, isError }: { content: string; isError: boolean }) {
  const [expanded, setExpanded] = useState(false);
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

/* ── Helpers ── */

function formatTime(timestamp: string): string {
  try {
    const d = new Date(timestamp);
    return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  } catch {
    return timestamp;
  }
}
