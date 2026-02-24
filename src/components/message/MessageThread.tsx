import type { DisplayMessage } from "../../types";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ToolOutputMessage } from "./ToolOutputMessage";

interface MessageThreadProps {
  messages: DisplayMessage[];
  source: string;
  showTimestamp: boolean;
  showModel: boolean;
}

export function MessageThread({ messages, source, showTimestamp, showModel }: MessageThreadProps) {
  return (
    <div className="max-w-3xl mx-auto py-6 px-6 space-y-6">
      {messages.map((msg, i) => {
        if (msg.role === "user") {
          return <UserMessage key={msg.uuid || i} message={msg} showTimestamp={showTimestamp} />;
        }
        if (msg.role === "tool") {
          return <ToolOutputMessage key={msg.uuid || i} message={msg} showTimestamp={showTimestamp} />;
        }
        return <AssistantMessage key={msg.uuid || i} message={msg} source={source} showTimestamp={showTimestamp} showModel={showModel} />;
      })}
    </div>
  );
}
