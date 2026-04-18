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
  windows_notification_enabled: boolean;
  timer_enabled: boolean;
  timer_duration_ms: number;
  auto_wipe_enabled: boolean;
  disable_file_movesorting: boolean;
  obs_websocket_enabled: boolean;
  obs_websocket_password: string;
  shadowplay_folder: string | null;
  prompt_capture_software: boolean;
  window_opacity: number;
  hover_full_opacity: boolean;
  active_theme_id: string;
  themes: Theme[];
}

export interface LogEntry {
  timestamp: string;
  level: "info" | "warning" | "error" | "success";
  message: string;
  category: string;
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
}

export interface FileMovedEvent {
  original: string;
  destination: string;
  tag: string;
  mode: string;
}
