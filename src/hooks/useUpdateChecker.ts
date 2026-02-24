import { useEffect, useRef } from "react";
import { useUpdateStore } from "../stores/updateStore";

declare const __IS_TAURI__: boolean;

export function useUpdateChecker() {
  const hasChecked = useRef(false);
  const { detectInstallType, loadCurrentVersion, checkForUpdate } =
    useUpdateStore();

  useEffect(() => {
    if (!__IS_TAURI__) return;
    if (hasChecked.current) return;
    hasChecked.current = true;

    detectInstallType();
    loadCurrentVersion();

    const timer = setTimeout(() => {
      checkForUpdate();
    }, 5000);

    return () => clearTimeout(timer);
  }, []);
}
