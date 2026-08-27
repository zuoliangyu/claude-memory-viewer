import { useEffect } from "react";
import { useAppStore } from "../stores/appStore";
import { isRemoteNodeActive } from "../services/nodeConfig";

declare const __IS_TAURI__: boolean;

const REFRESH_INTERVAL_MS = 10 * 60 * 1000;

export function useBackgroundRefresh() {
  const refreshInBackground = useAppStore((state) => state.refreshInBackground);

  useEffect(() => {
    if (!__IS_TAURI__ || isRemoteNodeActive()) return;

    // Initial mounting and source changes already load their visible lists.
    // Keep only the periodic safety refresh; file changes are handled by the
    // watcher and should not be accompanied by another delayed request.
    const interval = setInterval(() => {
      void refreshInBackground(true, false, { reason: "interval" });
    }, REFRESH_INTERVAL_MS);

    return () => {
      clearInterval(interval);
    };
  }, [refreshInBackground]);
}
