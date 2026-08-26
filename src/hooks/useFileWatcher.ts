import { useEffect, useRef, useCallback } from "react";
import { useAppStore } from "../stores/appStore";
import { isRemoteNodeActive } from "../services/nodeConfig";
import { recordPerfDiagnostic } from "../utils/perfDiagnostics";

declare const __IS_TAURI__: boolean;
const FILE_CHANGE_DEBOUNCE_MS = 2_000;

/**
 * In Tauri mode: use Tauri's event system (already handled by existing watcher).
 * In Web mode: connect to WebSocket at /ws for file change notifications.
 *
 * Debounces rapid file changes (e.g. multiple session deletions) to avoid
 * triggering excessive reloads.
 */
export function useFileWatcher() {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pendingPathsRef = useRef(new Set<string>());
  const hasUnknownPathsRef = useRef(false);
  const closingRef = useRef(false);
  const refreshInBackground = useAppStore((state) => state.refreshInBackground);

  const handleChange = useCallback((paths?: string[]) => {
    if (paths) {
      for (const path of paths) pendingPathsRef.current.add(path);
    } else {
      hasUnknownPathsRef.current = true;
    }
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      const changedPaths = hasUnknownPathsRef.current
        ? undefined
        : Array.from(pendingPathsRef.current);
      pendingPathsRef.current.clear();
      hasUnknownPathsRef.current = false;
      recordPerfDiagnostic("watcher.refresh_dispatched", undefined, {
        knownPaths: changedPaths != null,
        changedPaths: changedPaths?.length ?? null,
      });
      void refreshInBackground(false, false, {
        reason: "file-watcher",
        changedPaths,
      });
    }, FILE_CHANGE_DEBOUNCE_MS);
  }, [refreshInBackground]);

  useEffect(() => {
    if (__IS_TAURI__ && !isRemoteNodeActive()) {
      let unlisten: (() => void) | undefined;
      let cancelled = false;
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<string[]>("fs-change", (event) => handleChange(event.payload)).then((fn) => {
          if (cancelled) {
            fn();
            return;
          }
          unlisten = fn;
        });
      });
      return () => {
        cancelled = true;
        unlisten?.();
        clearTimeout(debounceRef.current);
        pendingPathsRef.current.clear();
        hasUnknownPathsRef.current = false;
      };
    }

    closingRef.current = false;

    // Web mode: connect to WebSocket
    const connect = () => {
      import("../services/webApi")
        .then(async ({ connectFileWatcherWebSocket }) => {
          if (closingRef.current) {
            return;
          }

          const ws = await connectFileWatcherWebSocket();
          if (closingRef.current) {
            ws.close();
            return;
          }
          wsRef.current = ws;

          ws.onmessage = (event) => {
            try {
              const payload = JSON.parse(String(event.data)) as {
                type?: string;
                paths?: unknown;
              };
              const paths = Array.isArray(payload.paths)
                ? payload.paths.filter((path): path is string => typeof path === "string")
                : undefined;
              handleChange(paths);
            } catch {
              handleChange();
            }
          };

          ws.onclose = () => {
            if (closingRef.current) {
              return;
            }
            reconnectRef.current = setTimeout(connect, 5000);
          };

          ws.onerror = () => {
            ws.close();
          };
        })
        .catch((error: unknown) => {
          if (closingRef.current) {
            return;
          }
          if (error instanceof Error && error.message === "Authentication required") {
            return;
          }
          reconnectRef.current = setTimeout(connect, 5000);
        });
    };

    connect();

    return () => {
      closingRef.current = true;
      clearTimeout(reconnectRef.current);
      clearTimeout(debounceRef.current);
      pendingPathsRef.current.clear();
      hasUnknownPathsRef.current = false;
      wsRef.current?.close();
    };
  }, [handleChange]);
}
