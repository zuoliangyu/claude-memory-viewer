import { useEffect } from "react";
import { useAppStore } from "../stores/appStore";
import { isRemoteNodeActive } from "../services/nodeConfig";

declare const __IS_TAURI__: boolean;

const INITIAL_REFRESH_DELAY_MS = 2000;
const REFRESH_INTERVAL_MS = 10 * 60 * 1000;

export function useBackgroundRefresh() {
  const source = useAppStore((state) => state.source);
  const selectedProject = useAppStore((state) => state.selectedProject);
  const refreshInBackground = useAppStore((state) => state.refreshInBackground);

  useEffect(() => {
    if (!__IS_TAURI__ || isRemoteNodeActive()) return;

    const initialTimer = setTimeout(() => {
      void refreshInBackground(true, false, { reason: "startup" });
    }, INITIAL_REFRESH_DELAY_MS);

    const interval = setInterval(() => {
      void refreshInBackground(true, false, { reason: "interval" });
    }, REFRESH_INTERVAL_MS);

    return () => {
      clearTimeout(initialTimer);
      clearInterval(interval);
    };
  }, [source, selectedProject, refreshInBackground]);
}
