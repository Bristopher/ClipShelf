import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ExternalLink,
  FolderOpen,
  HelpCircle,
  Pause,
  Play,
  Power,
  ScrollText,
} from "lucide-react";
import {
  fullQuit,
  getConfig,
  getDiagnostics,
  hideTrayMenu,
  openFolder,
  setWatchPaused,
  showMainWindow,
} from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { useTheme } from "@/hooks/useTheme";
import logoUrl from "@/assets/gkey-logo.png";
import type { AppConfig, Diagnostics } from "@/types";

const appWindow = getCurrentWindow();

/**
 * Custom themed context menu for the tray icon — rendered in the frameless
 * transparent "traymenu" window that Rust positions at the cursor on tray
 * right-click. Refreshes its snapshot on every `traymenu-visible` emit and
 * hides itself on blur, Esc, or after any action.
 */
export function TrayMenuApp() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  useTheme(config, null);

  // The window is transparent; the boot <style> and index.css both paint
  // opaque backgrounds — override all three surfaces (same as OverlayApp)
  // so only the rounded panel below is visible.
  useEffect(() => {
    const root = document.getElementById("root");
    document.body.style.background = "transparent";
    document.documentElement.style.background = "transparent";
    if (root) root.style.background = "transparent";
  }, []);

  useEffect(() => {
    getConfig().then(setConfig).catch(console.error);
    const unlisten = listen<AppConfig>(EVENTS.CONFIG_CHANGED, (e) => {
      setConfig(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Fresh snapshot each time the menu is shown (pause state can change from
  // the main window or the watcher dying between opens).
  useEffect(() => {
    const refresh = () => {
      getDiagnostics().then(setDiag).catch(console.error);
    };
    refresh();
    const unlisten = listen("traymenu-visible", refresh);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Dismiss like a native menu: blur or Esc.
  useEffect(() => {
    const unlisten = appWindow.onFocusChanged(({ payload: focused }) => {
      if (!focused) hideTrayMenu().catch(() => {});
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") hideTrayMenu().catch(() => {});
    };
    document.addEventListener("keydown", onKey);
    return () => {
      unlisten.then((fn) => fn());
      document.removeEventListener("keydown", onKey);
    };
  }, []);

  const close = () => hideTrayMenu().catch(() => {});
  const run = (action: () => Promise<unknown>) => {
    action().catch(console.error);
    close();
  };

  const paused = diag?.watchPaused ?? false;
  const folder = diag?.videosFolder ?? "";

  return (
    <div className="p-1.5 select-none" onContextMenu={(e) => e.preventDefault()}>
      <div className="rounded-lg border border-t-border bg-panel shadow-xl overflow-hidden">
        <div className="flex items-center gap-2 px-2.5 pt-2 pb-1.5">
          <img src={logoUrl} alt="" className="h-4 w-4 rounded-sm" />
          <span className="text-[11px] font-semibold text-t-text">GKey Mover</span>
          {diag && <span className="ml-auto text-[10px] text-t-muted">v{diag.version}</span>}
        </div>
        <div className="h-px bg-t-border mx-1.5" />
        <div className="p-1">
          <MenuItem
            icon={<ExternalLink className="h-3.5 w-3.5" />}
            label="Open GKey Mover"
            onClick={() => run(showMainWindow)}
          />
          <MenuItem
            icon={
              paused ? <Play className="h-3.5 w-3.5" /> : <Pause className="h-3.5 w-3.5" />
            }
            label={paused ? "Resume Watching" : "Pause Watching"}
            onClick={() => run(() => setWatchPaused(!paused))}
          />
          <MenuItem
            icon={<FolderOpen className="h-3.5 w-3.5" />}
            label="Video Folder"
            disabled={!folder}
            onClick={() => run(() => openFolder(folder))}
          />
          <MenuItem
            icon={<ScrollText className="h-3.5 w-3.5" />}
            label="Log Folder"
            disabled={!folder}
            onClick={() => run(() => openFolder(`${folder}\\logs`))}
          />
          <MenuItem
            icon={<HelpCircle className="h-3.5 w-3.5" />}
            label="Help"
            onClick={() => run(() => openFolder("https://github.com"))}
          />
        </div>
        <div className="h-px bg-t-border mx-1.5" />
        <div className="p-1">
          <MenuItem
            icon={<Power className="h-3.5 w-3.5" />}
            label="Exit"
            danger
            onClick={() => run(fullQuit)}
          />
        </div>
      </div>
    </div>
  );
}

function MenuItem({
  icon,
  label,
  onClick,
  disabled,
  danger,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`w-full flex items-center gap-2 px-2 h-7 rounded text-xs text-left ${
        disabled
          ? "text-t-muted/50 cursor-default"
          : danger
            ? "text-red-400 hover:bg-hover"
            : "text-t-text hover:bg-hover"
      }`}
    >
      <span className={disabled ? "" : danger ? "text-red-400" : "text-t-muted"}>{icon}</span>
      {label}
    </button>
  );
}
