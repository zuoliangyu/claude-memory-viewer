import { useEffect, useRef, useCallback } from "react";
import { useAppStore } from "../stores/appStore";

declare const __IS_TAURI__: boolean;

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
  const { loadProjects, selectProject } = useAppStore();

  const handleChange = useCallback(() => {
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      loadProjects();
      const project = useAppStore.getState().selectedProject;
      if (project) selectProject(project);
    }, 500);
  }, [loadProjects, selectProject]);

  useEffect(() => {
    if (__IS_TAURI__) {
      let unlisten: (() => void) | undefined;
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<string[]>("fs-change", handleChange).then((fn) => {
          unlisten = fn;
        });
      });
      return () => {
        unlisten?.();
        clearTimeout(debounceRef.current);
      };
    }

    // Web mode: connect to WebSocket
    const connect = () => {
      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const wsUrl = `${protocol}//${window.location.host}/ws`;

      const token = localStorage.getItem("asv_token");
      const url = token ? `${wsUrl}?token=${encodeURIComponent(token)}` : wsUrl;

      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onmessage = handleChange;

      ws.onclose = () => {
        reconnectRef.current = setTimeout(connect, 5000);
      };

      ws.onerror = () => {
        ws.close();
      };
    };

    connect();

    return () => {
      clearTimeout(reconnectRef.current);
      clearTimeout(debounceRef.current);
      wsRef.current?.close();
    };
  }, [handleChange]);
}
