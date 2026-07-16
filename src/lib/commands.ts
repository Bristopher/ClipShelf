import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  Diagnostics,
  GKeyStat,
  HistoryEntry,
  LogEntry,
  OverlayContext,
  Theme,
} from "../types";

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
export const startCalibration = (targetSamples: number) =>
  invoke<void>("start_calibration", { targetSamples });
export const cancelCalibration = () => invoke<void>("cancel_calibration");
export const toggleCountUp = () => invoke<void>("toggle_count_up");
export const fullQuit = () => invoke<void>("full_quit");
export const showMainWindow = () => invoke<void>("show_main_window");
export const hideTrayMenu = () => invoke<void>("hide_tray_menu");
export const manualUpdateCheck = () => invoke<void>("manual_update_check");
export const undoLastAction = () => invoke<void>("undo_last_action");
export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });
export const setWatchPaused = (paused: boolean) =>
  invoke<void>("set_watch_paused", { paused });
export const getMonitorCount = () => invoke<number>("get_monitor_count");
export const getWatcherStatus = () =>
  invoke<{ status: string; restartCount?: number }>("get_watcher_status");
export const getObsStatus = () =>
  invoke<{ status: string; attempt?: number }>("get_obs_status");
export const dropFilesToGkey = (paths: string[], key: number) =>
  invoke<{ moved: number; failed: number }>("drop_files_to_gkey", { paths, key });
export const selectDroppedFile = (path: string) =>
  invoke<string>("select_dropped_file", { path });
export const getGkeyStats = () => invoke<GKeyStat[]>("get_gkey_stats");
export const getDiagnostics = () => invoke<Diagnostics>("get_diagnostics");
export const testObsConnection = (password: string) =>
  invoke<void>("test_obs_connection", { password });
export const getHistory = (full: boolean) => invoke<HistoryEntry[]>("get_history", { full });
export const editHistoryGame = (path: string, game: string, exe: string | null, remember: boolean) =>
  invoke<void>("edit_history_game", { path, game, exe, remember });

// --- Overlay (Task 6) ---
export const overlayGetContext = () => invoke<OverlayContext>("overlay_get_context");
export const overlaySort = (key: number) => invoke<void>("overlay_sort", { key });
export const overlayRate = (stars: number) => invoke<void>("overlay_rate", { stars });
export const overlayLabel = (label: string) => invoke<void>("overlay_label", { label });
export const overlayDescribe = (text: string) => invoke<void>("overlay_describe", { text });
export const overlaySetGame = (game: string, remember: boolean) =>
  invoke<void>("overlay_set_game", { game, remember });
export const overlayTimerToggle = () => invoke<void>("overlay_timer_toggle");
export const overlayNeedsLabel = () => invoke<void>("overlay_needs_label");
export const startTypeMode = () => invoke<void>("start_type_mode");
export const stopTypeMode = () => invoke<void>("stop_type_mode");
export const hideOverlay = () => invoke<void>("hide_overlay");
