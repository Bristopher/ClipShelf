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
export const startCalibration = (targetSamples: number) =>
  invoke<void>("start_calibration", { targetSamples });
export const cancelCalibration = () => invoke<void>("cancel_calibration");
export const toggleCountUp = () => invoke<void>("toggle_count_up");
export const fullQuit = () => invoke<void>("full_quit");
export const undoLastAction = () => invoke<void>("undo_last_action");
export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });
export const setWatchPaused = (paused: boolean) =>
  invoke<void>("set_watch_paused", { paused });
export const getMonitorCount = () => invoke<number>("get_monitor_count");
