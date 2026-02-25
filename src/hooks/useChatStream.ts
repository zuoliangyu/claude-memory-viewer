import { useEffect, useRef } from "react";
import { useChatStore } from "../stores/chatStore";

declare const __IS_TAURI__: boolean;

/** Filter out non-error stderr lines (progress/info output from CLI). */
function isActualError(line: string): boolean {
  const lower = line.toLowerCase().trim();
  if (!lower) return false;
  // Ignore common non-error stderr lines
  if (lower.startsWith("[request interrupted")) return false;
  if (lower.startsWith("warning:")) return false;
  if (lower.startsWith("info:")) return false;
  if (lower.startsWith("debug:")) return false;
  // Lines containing "error" or "fatal" are likely real errors
  if (lower.includes("error") || lower.includes("fatal") || lower.includes("panic")) return true;
  // Other stderr lines — show them (could be relevant)
  return true;
}

/**
 * Hook to listen for chat stream events.
 * In Tauri mode: listens to Tauri events.
 * In Web mode: listens to WebSocket messages.
 */
export function useChatStream() {
  const sessionId = useChatStore((s) => s.sessionId);
  const addStreamLine = useChatStore((s) => s.addStreamLine);
  const setStreaming = useChatStore((s) => s.setStreaming);
  const setError = useChatStore((s) => s.setError);
  const cleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    if (!sessionId) return;

    if (__IS_TAURI__) {
      // Tauri event listeners
      let cancelled = false;

      const setupListeners = async () => {
        const { listen } = await import("@tauri-apps/api/event");

        const unlistenOutput = await listen<string>(
          `chat-output:${sessionId}`,
          (event) => {
            if (!cancelled) {
              addStreamLine(event.payload);
            }
          }
        );

        const unlistenError = await listen<string>(
          `chat-error:${sessionId}`,
          (event) => {
            if (!cancelled) {
              // stderr from CLI — only show actual errors, not progress/info lines
              const line = event.payload;
              if (line && isActualError(line)) {
                setError(line);
              }
            }
          }
        );

        const unlistenComplete = await listen<string>(
          `chat-complete:${sessionId}`,
          () => {
            if (!cancelled) {
              setStreaming(false);
            }
          }
        );

        cleanupRef.current = () => {
          cancelled = true;
          unlistenOutput();
          unlistenError();
          unlistenComplete();
        };
      };

      setupListeners();
    } else {
      // Web mode: listen to WebSocket messages
      const setupWebSocket = async () => {
        const { getChatWebSocket } = await import("../services/webApi");
        const ws = getChatWebSocket();

        const handleMessage = (event: MessageEvent) => {
          try {
            const data = JSON.parse(event.data);
            if (data.type === "output") {
              addStreamLine(data.data);
            } else if (data.type === "error") {
              if (data.data && isActualError(data.data)) {
                setError(data.data);
              }
            } else if (data.type === "complete") {
              setStreaming(false);
            }
            // session_id type is already handled in webApi.ts
          } catch {
            // non-JSON message, ignore
          }
        };

        ws.addEventListener("message", handleMessage);

        cleanupRef.current = () => {
          ws.removeEventListener("message", handleMessage);
        };
      };

      setupWebSocket();
    }

    return () => {
      if (cleanupRef.current) {
        cleanupRef.current();
        cleanupRef.current = null;
      }
    };
  }, [sessionId, addStreamLine, setStreaming, setError]);
}
