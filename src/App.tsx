import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  getConfig,
  updateConfig,
  setWindowOpacity,
  wipeLog,
  undoLastAction,
  openSettingsWindow,
  dropFileToGkey,
  selectDroppedFile,
} from "@/lib/commands";
import { errorMessage, toastError, toastInfo } from "@/lib/toast";
import { EVENTS } from "@/lib/events";
import { useEventLog } from "@/hooks/useEventLog";
import { useTimer } from "@/hooks/useTimer";
import { useTheme } from "@/hooks/useTheme";
import { resolveFlashTheme } from "@/lib/themes";
import { EventLog } from "@/components/EventLog";
import { Sidebar } from "@/components/Sidebar";
import { TimerDisplay } from "@/components/TimerDisplay";
import { BottomBar } from "@/components/BottomBar";
import { RenameDialog } from "@/components/RenameDialog";
import { TitleBar } from "@/components/TitleBar";
import { Toaster } from "@/components/Toaster";
import { openFirstRunWindow } from "@/lib/commands";
import type { AppConfig } from "@/types";

// Short tone played when user clicks the locked main area while first-run
// setup is still pending — analogous to the Windows dialog "ding".
function playBeep() {
  try {
    const ctx = new (window.AudioContext || (window as any).webkitAudioContext)();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.type = "sine";
    osc.frequency.value = 520;
    gain.gain.setValueAtTime(0.0001, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.2, ctx.currentTime + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.18);
    osc.start();
    osc.stop(ctx.currentTime + 0.2);
    osc.onended = () => ctx.close();
  } catch {
    /* no audio available */
  }
}

// Match the backend watcher's accepted extensions (watcher::is_video_file).
const VIDEO_EXT_RE = /\.(mp4|mov|avi|mkv)$/i;

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const { entries, clear, restore } = useEventLog();
  const [filterOpen, setFilterOpen] = useState(false);
  // Drag-drop hover state: which G-key button the file is over (1-4), or
  // null; dragActive drives the "drop to rename" hint over the log.
  const [dropKey, setDropKey] = useState<number | null>(null);
  const [dragActive, setDragActive] = useState(false);

  useEffect(() => {
    getConfig().then(setConfig).catch(console.error);
  }, []);

  useEffect(() => {
    const unlisten = listen<AppConfig>(EVENTS.CONFIG_CHANGED, (event) => {
      setConfig(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // G1-G3 hotkeys are handled entirely in Rust now (no webview round-trip).
  // The hotkey-triggered event only remains for key=4 (rename), which the
  // RenameDialog listens for since it needs UI.

  // Auto-wipe on timer expiry — listener mounts once; config + clear are
  // read via refs so we don't re-register on every render.
  const autoWipeRef = useRef(false);
  autoWipeRef.current = !!config?.auto_wipe_enabled;
  const clearRef = useRef(clear);
  clearRef.current = clear;
  useEffect(() => {
    const unlisten = listen(EVENTS.TIMER_EXPIRED, () => {
      if (autoWipeRef.current) {
        clearRef.current();
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // In-app (non-global) shortcuts on the main window. Skipped while typing
  // in an input (covers the rename dialog and the log filter box).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) {
        return;
      }
      if (e.key === "Delete" && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        e.preventDefault();
        wipeLog()
          .then(() => clearRef.current())
          .catch((err) => toastError(errorMessage(err)));
      } else if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "z") {
        e.preventDefault();
        undoLastAction().catch((err) => toastError(errorMessage(err)));
      } else if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        openSettingsWindow().catch((err) => toastError(errorMessage(err)));
      } else if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setFilterOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // Drag-and-drop from Explorer. Tauri's dragDrop swallows HTML5 drag
  // events, so hover targets are hit-tested from the event's physical
  // position. G1-G3 buttons sort the dropped file; anywhere else selects it
  // as the current clip and opens the rename dialog.
  useEffect(() => {
    const targetKeyAt = (pos: { x: number; y: number }): number | null => {
      const dpr = window.devicePixelRatio || 1;
      const el = document.elementFromPoint(pos.x / dpr, pos.y / dpr);
      const btn = el?.closest("[data-drop-key]");
      const key = btn ? Number(btn.getAttribute("data-drop-key")) : NaN;
      return Number.isInteger(key) ? key : null;
    };

    const handleDrop = (paths: string[], pos: { x: number; y: number }) => {
      const videos = paths.filter((p) => VIDEO_EXT_RE.test(p));
      if (videos.length === 0) {
        toastError("Only video files (.mp4 .mov .avi .mkv) can be dropped here");
        return;
      }
      if (videos.length > 1) {
        toastInfo("Multiple files dropped — using the first video");
      }
      const path = videos[0];
      const key = targetKeyAt(pos);
      if (key !== null && key >= 1 && key <= 3) {
        dropFileToGkey(path, key).catch((err) => toastError(errorMessage(err)));
      } else {
        // G4 / log / anywhere else: make it the current clip and rename it.
        selectDroppedFile(path)
          .then((filename) => emit(EVENTS.HOTKEY_TRIGGERED, { key: 4, filename }))
          .catch((err) => toastError(errorMessage(err)));
      }
    };

    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") {
        setDragActive(true);
        setDropKey(targetKeyAt(p.position));
      } else if (p.type === "drop") {
        setDragActive(false);
        setDropKey(null);
        handleDrop(p.paths, p.position);
      } else {
        setDragActive(false);
        setDropKey(null);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Apply window opacity when config loads or changes
  useEffect(() => {
    if (config?.window_opacity != null) {
      setWindowOpacity(config.window_opacity).catch(console.error);
    }
  }, [config?.window_opacity]);

  // First-run flow — open the setup window if no videos folder is set yet.
  useEffect(() => {
    if (config && !config.videos_folder) {
      openFirstRunWindow().catch(console.error);
    }
  }, [config?.videos_folder]);

  // Hover-to-full-opacity
  const handleMouseEnter = useCallback(() => {
    if (config?.hover_full_opacity && config.window_opacity < 1) {
      setWindowOpacity(1).catch(console.error);
    }
  }, [config?.hover_full_opacity, config?.window_opacity]);

  const handleMouseLeave = useCallback(() => {
    if (config?.hover_full_opacity && config.window_opacity < 1) {
      setWindowOpacity(config.window_opacity).catch(console.error);
    }
  }, [config?.hover_full_opacity, config?.window_opacity]);

  const initialSecs = config ? Math.floor(config.timer_duration_ms / 1000) : 70;
  const timer = useTimer(initialSecs);
  // (The old manual "user timer" countdown was never reachable from the UI —
  // its frontend wiring was removed. The backend actor still exists if a
  // future feature wants a second countdown.)

  // Flash the whole window each second once the countdown is at 5s or
  // fewer — opt-out via `timer_flash_enabled`. Parity of the remaining-
  // seconds integer drives the toggle (ticks arrive every 1s so the class
  // flips every tick). When on, useTheme swaps to the contrasting/override
  // theme; when off, it reapplies the active theme.
  const flashOn = (() => {
    if (!config?.timer_flash_enabled) return false;
    const t =
      timer.running && timer.remainingSecs > 0 && timer.remainingSecs <= 5
        ? timer.remainingSecs
        : 0;
    return t > 0 && t % 2 === 1;
  })();

  const flashOverride = useMemo(
    () => (flashOn && config ? resolveFlashTheme(config) : null),
    [flashOn, config?.timer_flash_theme_id, config?.active_theme_id, config?.themes],
  );
  useTheme(config, flashOverride);

  if (!config) {
    return (
      <div className="flex h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    );
  }

  return (
    <div
      className="flex flex-col h-screen"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      <TitleBar />
      <div className="flex flex-1 min-h-0 relative">
        <Sidebar config={config} dropKey={dropKey} />
        <main className="flex-1 flex flex-col min-w-0 relative">
          <EventLog
            entries={entries}
            config={config}
            filterOpen={filterOpen}
            onCloseFilter={() => setFilterOpen(false)}
          />
          {dragActive && dropKey === null && (
            <div className="absolute inset-0 z-30 pointer-events-none flex items-center justify-center bg-black/30 border-2 border-dashed border-t-border rounded-sm m-1">
              <span className="text-xs font-semibold text-t-text bg-panel/90 px-3 py-1.5 rounded border border-t-border shadow">
                Drop to select &amp; rename — or drop on a G-key to sort
              </span>
            </div>
          )}
          <BottomBar
            mode={config.disable_file_movesorting ? "rename" : "sort"}
            autoWipe={config.auto_wipe_enabled}
            obsEnabled={config.obs_websocket_enabled}
            onAutoWipeChange={(v) => updateConfig({ auto_wipe_enabled: v }).then(setConfig)}
            onWipe={clear}
            onRestore={restore}
          />
        </main>
        <TimerDisplay
          remainingSecs={timer.remainingSecs}
          totalSecs={timer.totalSecs}
          running={timer.running}
        />
        {!config.videos_folder && (
          <div
            className="absolute inset-0 z-40 bg-black/55 backdrop-blur-[1px] cursor-not-allowed flex items-center justify-center"
            onMouseDownCapture={(e) => {
              e.preventDefault();
              e.stopPropagation();
              playBeep();
              openFirstRunWindow().catch(console.error);
            }}
          >
            <div className="pointer-events-none text-t-text text-xs font-semibold px-3 py-1.5 rounded bg-panel/80 border border-t-border shadow-lg">
              Finish setup first →
            </div>
          </div>
        )}
      </div>
      <RenameDialog mru={config.rename_mru} />
      <Toaster listenBackendErrors />
    </div>
  );
}

export default App;
