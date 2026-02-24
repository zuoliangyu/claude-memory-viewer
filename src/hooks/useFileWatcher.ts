import { useEffect, useRef } from "react";
import { useAppStore } from "../stores/appStore";

declare const __IS_TAURI__: boolean;

/**
 * In Tauri mode: use Tauri's event system (already handled by existing watcher).
 * In Web mode: connect to WebSocket at /ws for file change notifications.
 */
export function useFileWatcher() {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const { loadProjects, selectedProject, selectProject } = useAppStore();

  useEffect(() => {
    if (__IS_TAURI__) {
      // Tauri mode: listen to fs-change events from the Rust backend
      let unlisten: (() => void) | undefined;
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<string[]>("fs-change", () => {
          loadProjects();
          const project = useAppStore.getState().selectedProject;
          if (project) selectProject(project);
        }).then((fn) => {
          unlisten = fn;
        });
      });
      return () => {
        unlisten?.();
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

      ws.onmessage = () => {
        // Any file change: refresh data
        loadProjects();
        const project = useAppStore.getState().selectedProject;
        if (project) selectProject(project);
      };

      ws.onclose = () => {
        // Reconnect after 5 seconds
        reconnectRef.current = setTimeout(connect, 5000);
      };

      ws.onerror = () => {
        ws.close();
      };
    };

    connect();

    return () => {
      clearTimeout(reconnectRef.current);
      wsRef.current?.close();
    };
  }, []);
}
