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
