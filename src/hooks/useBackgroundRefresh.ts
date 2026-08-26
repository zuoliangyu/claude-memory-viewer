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
      // 首屏加载已经读取过项目和当前会话。这里只核对缓存，避免切换数据源后
      // 立刻重复深扫全部 rollout，与消息首屏请求争用磁盘和命令执行线程。
      void refreshInBackground(false, false, { reason: "startup" });
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
