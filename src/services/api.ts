declare const __IS_TAURI__: boolean;

// Dynamically import the appropriate API module based on build target
const apiModule = __IS_TAURI__
  ? import("./tauriApi")
  : import("./webApi");

// Re-export all API functions through the promise
export const api = await apiModule;
