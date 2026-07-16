export interface ThemeTokens {
  title_bar: string;
  app_bg: string;
  panel_bg: string;
  text: string;
  text_muted: string;
  border: string;
  hover_bg: string;
  g1_accent: string;
  g2_accent: string;
  g3_accent: string;
  g4_accent: string;
}

export interface Theme {
  id: string;
  name: string;
  builtin: boolean;
  tokens: ThemeTokens;
}

export type ThemeTokenKey = keyof ThemeTokens;

export const THEME_TOKEN_ORDER: readonly ThemeTokenKey[] = [
  "title_bar",
  "app_bg",
  "panel_bg",
  "text",
  "text_muted",
  "border",
  "hover_bg",
  "g1_accent",
  "g2_accent",
  "g3_accent",
  "g4_accent",
] as const;

export const THEME_TOKEN_LABELS: Record<ThemeTokenKey, string> = {
  title_bar: "Title Bar",
  app_bg: "App Background",
  panel_bg: "Panel Background",
  text: "Text",
  text_muted: "Muted Text",
  border: "Border",
  hover_bg: "Hover",
  g1_accent: "G1 Button",
  g2_accent: "G2 Button",
  g3_accent: "G3 Button",
  g4_accent: "G4 Button",
};

export interface AppConfig {
  screen_capture_software: string;
  videos_folder: string;
  log_file_enabled: boolean;
  g1_bind: string;
  g2_bind: string;
  g3_bind: string;
  rename_bind: string;
  restart_watcher_bind: string;
  g1_bind_folder_name: string;
  g2_bind_folder_name: string;
  g3_bind_folder_name: string;
  clip_save_sound_enabled: boolean;
  clip_save_sound_custom: string | null;
  move_sound_enabled: boolean;
  error_sound_enabled: boolean;
  error_sound_custom: string | null;
  timer_enabled: boolean;
  timer_duration_ms: number;
  auto_wipe_enabled: boolean;
  disable_file_movesorting: boolean;
  obs_websocket_enabled: boolean;
  obs_websocket_password: string;
  window_opacity: number;
  hover_full_opacity: boolean;
  active_theme_id: string;
  themes: Theme[];
  save_clip_bind: string;
  timer_flash_enabled: boolean;
  save_clip_health_check_timeout_secs: number;
  /** null = auto-pick contrasting built-in theme; otherwise theme id to swap to during ≤5s flash. */
  timer_flash_theme_id: string | null;
  count_up_bind: string;
  /** Clips smaller than this (MB) get the "possible black screen" warning. */
  small_file_warn_mb: number;
  /** Hotkey to undo the last move/rename. Empty = not registered. */
  undo_bind: string;
  /** Launch GKey Mover automatically at Windows login. */
  autostart_enabled: boolean;
  /** Restore last window position/size on launch. */
  remember_window_layout: boolean;
  /** 1-based monitor for the default open position. */
  default_monitor: number;
  /** Anchor corner for the default open position. */
  default_anchor: string;
  /** Most-recently-used rename texts, newest first (backend-maintained). */
  rename_mru: string[];
  game_detection_enabled: boolean;
  check_updates: boolean;
  write_file_properties: boolean;
  day_rollover_hour: number;
  game_overrides: { exe: string; name: string }[];
  overlay_enabled: boolean;
  overlay_bind: string;
  overlay_typing_enabled: boolean;
  label_presets: string[];
  description_presets: string[];
}

/** G-key binds + folder names + the overlay toggle bind — mirrors Rust `OverlayBinds`. */
export interface OverlayBinds {
  g1: string;
  g2: string;
  g3: string;
  g1Name: string;
  g2Name: string;
  g3Name: string;
  overlay: string;
}

/** Snapshot the overlay UI renders from — mirrors Rust `OverlayContext`. */
export interface OverlayContext {
  filename: string;
  path: string;
  game: string | null;
  exe: string | null;
  labelPresets: string[];
  descriptionPresets: string[];
  typingEnabled: boolean;
  binds: OverlayBinds;
}

export interface RecentClip {
  name: string;
  path: string;
}

/** Session move stats for one G-key (sidebar badge + flyout). */
export interface GKeyStat {
  key: number;
  count: number;
  recent: RecentClip[];
}

/** Snapshot for the diagnostics popover. */
export interface Diagnostics {
  version: string;
  configPath: string;
  videosFolder: string;
  watcherStatus: string;
  watcherRestartCount: number;
  watchPaused: boolean;
  obsEnabled: boolean;
  obsStatus: string;
}

export interface CountUpTick {
  elapsedSecs: number;
  running: boolean;
}

export interface CalibrationSampleEvent {
  kind: "sample" | "complete";
  filename: string;
  deltaMs: number;
  index: number;
  target: number;
  averageMs?: number;
  worstMs?: number;
  bestMs?: number;
}

export interface LogEntry {
  timestamp: string;
  level: "info" | "warning" | "error" | "success";
  message: string;
  category: string;
  /** File this entry refers to — makes the entry clickable (reveal/play). */
  path?: string;
}

export interface TimerTick {
  remainingSecs: number;
  totalSecs: number;
}

export interface FileCreatedEvent {
  path: string;
  filename: string;
  timestamp: string;
  sizeMb: number;
  isWarning: boolean;
  game?: string | null;
}

export interface FileMovedEvent {
  original: string;
  destination: string;
  tag: string;
  mode: string;
}

/** One row in the History panel — mirrors the Rust HistoryEntryPayload. */
export interface HistoryEntry {
  ts: string;
  /** created | moved | renamed | rated | labeled | described | game_edited | undone */
  event: string;
  path: string;
  oldPath?: string;
  game?: string;
  exe?: string;
  key?: number;
  rating?: number;
  label?: string;
  description?: string;
  /** hotkey | overlay | drop | app */
  source: string;
  /** Logical day bucket ("YYYY-MM-DD"), precomputed by the backend. */
  day: string;
  filename: string;
  /** Distinct-clip identity from backend reconciliation. All events of one
   * physical clip share it, so group headers count DISTINCT clips, not rows. */
  clipId: number;
}
