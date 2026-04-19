import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, LogEntry, Theme } from "../types";

export const getConfig = () => invoke<AppConfig>("get_config");
export const updateConfig = (partial: Partial<AppConfig>) => invoke<AppConfig>("update_config", { partial });
export const pressGkey = (key: number) => invoke<void>("press_gkey", { key });
export const renameFile = (text: string) => invoke<void>("rename_file", { text });
export const wipeLog = () => invoke<void>("wipe_log");
export const restoreLog = () => invoke<LogEntry[]>("restore_log");
export const restartWatcher = () => invoke<void>("restart_watcher");
export const openFolder = (path: string) => invoke<void>("open_folder", { path });
export const setWindowOpacity = (opacity: number) => invoke<void>("set_window_opacity", { opacity });
export const resetWindow = () => invoke<void>("reset_window");
export const importTheme = (path: string) => invoke<Theme>("import_theme", { path });
export const exportTheme = (path: string, themeId: string) =>
  invoke<void>("export_theme", { path, themeId });
export const openSettingsWindow = () => invoke<void>("open_settings_window");
export const openFirstRunWindow = () => invoke<void>("open_first_run_window");
export const startUserTimer = (durationSecs?: number) =>
  invoke<void>("start_user_timer", { durationSecs });
export const resetUserTimer = (durationSecs?: number) =>
  invoke<void>("reset_user_timer", { durationSecs });
