import { useState } from "react";
import type { DisplayMessage } from "../../types";
import { Terminal, ChevronDown, ChevronRight } from "lucide-react";
import { formatTime } from "./utils";

interface Props {
  message: DisplayMessage;
  showTimestamp: boolean;
}

export function ToolOutputMessage({ message, showTimestamp }: Props) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="flex gap-3 ml-10">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-1">
          <Terminal className="w-3 h-3 text-muted-foreground" />
          <span className="text-xs font-medium text-muted-foreground">Tool Output</span>
          {showTimestamp && message.timestamp && (
            <span className="text-xs text-muted-foreground">
              {formatTime(message.timestamp)}
            </span>
          )}
        </div>
        {message.content.map((block, i) => {
          if (block.type === "function_call_output") {
            const output = block.output;
            const isLong = output.length > 300;

            return (
              <div key={i} className="mt-1">
                {isLong ? (
                  <div className="border border-border rounded-md overflow-hidden">
                    <button
                      onClick={() => setExpanded(!expanded)}
                      className="w-full flex items-center gap-2 px-3 py-1.5 text-xs bg-muted/30 hover:bg-muted/50 transition-colors"
                    >
                      <span className="text-muted-foreground">
                        {output.length} chars
                      </span>
                      {expanded ? (
                        <ChevronDown className="w-3 h-3 ml-auto" />
                      ) : (
                        <ChevronRight className="w-3 h-3 ml-auto" />
                      )}
                    </button>
                    {expanded && (
                      <div className="p-3 text-xs font-mono bg-muted/10 overflow-x-auto max-h-96 overflow-y-auto">
                        <pre className="whitespace-pre-wrap break-all">
                          {output.length > 10000
                            ? output.slice(0, 10000) + "\n... (truncated)"
                            : output}
                        </pre>
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="text-xs font-mono bg-muted/20 rounded-md p-3 overflow-x-auto">
                    <pre className="whitespace-pre-wrap break-all">
                      {output}
                    </pre>
                  </div>
                )}
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
