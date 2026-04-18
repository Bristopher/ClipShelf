import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getConfig, updateConfig, pressGkey, setWindowOpacity } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { useEventLog } from "@/hooks/useEventLog";
import { useTimer } from "@/hooks/useTimer";
import { useTheme } from "@/hooks/useTheme";
import { EventLog } from "@/components/EventLog";
import { Sidebar } from "@/components/Sidebar";
import { TimerDisplay } from "@/components/TimerDisplay";
import { BottomBar } from "@/components/BottomBar";
import { RenameDialog } from "@/components/RenameDialog";
import { TitleBar } from "@/components/TitleBar";
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

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const { entries, clear, restore } = useEventLog();
  useTheme(config);

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

  // Handle hotkey-triggered events for G1-G3 (call press_gkey command)
  useEffect(() => {
    const unlisten = listen<{ key: number }>(EVENTS.HOTKEY_TRIGGERED, (event) => {
      const key = event.payload.key;
      if (key >= 1 && key <= 3) {
        pressGkey(key);
      }
      // key=4 (rename) is handled by RenameDialog
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Auto-wipe on timer expiry
  const clearRef = useCallback(clear, [clear]);
  useEffect(() => {
    const unlisten = listen(EVENTS.TIMER_EXPIRED, () => {
      if (config?.auto_wipe_enabled) {
        clearRef();
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [config?.auto_wipe_enabled, clearRef]);

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
        <Sidebar />
        <main className="flex-1 flex flex-col min-w-0">
          <EventLog entries={entries} />
          <BottomBar
            mode={config.disable_file_movesorting ? "rename" : "sort"}
            autoWipe={config.auto_wipe_enabled}
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
      <RenameDialog />
    </div>
  );
}

export default App;
