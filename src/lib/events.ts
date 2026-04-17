export const EVENTS = {
  FILE_CREATED: "file-created",
  FILE_MOVED: "file-moved",
  FILE_RENAMED: "file-renamed",
  HOTKEY_PRESSED: "hotkey-pressed",
  HOTKEY_TRIGGERED: "hotkey-triggered",
  TIMER_TICK: "timer-tick",
  TIMER_EXPIRED: "timer-expired",
  LOG_ENTRY: "log-entry",
  WATCHER_STATUS: "watcher-status",
  OBS_WS_STATUS: "obs-ws-status",
  CONFIG_CHANGED: "config-changed",
  ERROR: "error",
} as const;
