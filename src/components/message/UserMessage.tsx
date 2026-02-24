import type { DisplayMessage } from "../../types";
import { User } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { formatTime } from "./utils";

interface Props {
  message: DisplayMessage;
  showTimestamp: boolean;
}

export function UserMessage({ message, showTimestamp }: Props) {
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
