import { useState } from "react";
import type { DisplayMessage } from "../../types";
import { Bot, ChevronDown, ChevronRight, Wrench, Brain } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { formatTime } from "./utils";

interface Props {
  message: DisplayMessage;
  source: string;
  showTimestamp: boolean;
  showModel: boolean;
}

export function AssistantMessage({ message, source, showTimestamp, showModel }: Props) {
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
        </div>
        {message.content.map((block, i) => {
          if (block.type === "text") {
            return (
              <div key={i} className="prose prose-sm max-w-none text-sm leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
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
                        <code
                          className="bg-muted px-1.5 py-0.5 rounded text-xs font-mono"
                          {...props}
                        >
                          {children}
                        </code>
                      );
                    },
                    pre({ children }) {
                      return <div className="not-prose my-2">{children}</div>;
                    },
                    table({ children }) {
                      return (
                        <div className="overflow-x-auto my-3">
                          <table className="min-w-full text-xs border-collapse border border-border rounded">
                            {children}
                          </table>
                        </div>
                      );
                    },
                    th({ children }) {
                      return (
                        <th className="bg-muted/50 px-3 py-1.5 text-left text-xs font-medium border border-border">
                          {children}
                        </th>
                      );
                    },
                    td({ children }) {
                      return (
                        <td className="px-3 py-1.5 text-xs border border-border">
                          {children}
                        </td>
                      );
                    },
                    a({ href, children }) {
                      return (
                        <a
                          href={href}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-blue-400 hover:text-blue-300 underline underline-offset-2"
                        >
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
            return <ThinkingBlock key={i} thinking={block.thinking} />;
          }
          if (block.type === "reasoning") {
            return <ReasoningBlock key={i} text={block.text} />;
          }
          if (block.type === "tool_use") {
            return (
              <ToolCallBlock
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
}

function ThinkingBlock({ thinking }: { thinking: string }) {
  const [expanded, setExpanded] = useState(false);

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
        <div className="mt-1 pl-5 text-xs text-muted-foreground whitespace-pre-wrap border-l-2 border-muted">
          {thinking}
        </div>
      )}
    </div>
  );
}

function ReasoningBlock({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);

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
        {expanded ? (
          <ChevronDown className="w-3 h-3 ml-auto" />
        ) : (
          <ChevronRight className="w-3 h-3 ml-auto" />
        )}
      </button>
      {expanded && (
        <div className="p-3 text-xs font-mono bg-muted/20 overflow-x-auto">
          <pre className="whitespace-pre-wrap break-all">
            {input.length > 5000
              ? input.slice(0, 5000) + "\n... (truncated)"
              : input}
          </pre>
        </div>
      )}
    </div>
  );
}

function FunctionCallBlock({ name, arguments: args }: { name: string; arguments: string }) {
  const [expanded, setExpanded] = useState(false);

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
          <pre className="whitespace-pre-wrap break-all">
            {args.length > 5000
              ? args.slice(0, 5000) + "\n... (truncated)"
              : args}
          </pre>
        </div>
      )}
    </div>
  );
}
