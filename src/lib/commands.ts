import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, LogEntry } from "../types";

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
