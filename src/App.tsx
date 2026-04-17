import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getConfig, updateConfig, pressGkey, setWindowOpacity } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { useEventLog } from "@/hooks/useEventLog";
import { useTimer } from "@/hooks/useTimer";
import { EventLog } from "@/components/EventLog";
import { Sidebar } from "@/components/Sidebar";
import { TimerDisplay } from "@/components/TimerDisplay";
import { BottomBar } from "@/components/BottomBar";
import { SettingsSheet } from "@/components/SettingsSheet";
import { RenameDialog } from "@/components/RenameDialog";
import { TitleBar } from "@/components/TitleBar";
import type { AppConfig } from "@/types";

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { entries, clear, restore } = useEventLog();

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
      <div className="flex flex-1 min-h-0">
        <Sidebar onSettingsClick={() => setSettingsOpen(true)} />
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
      </div>
      <SettingsSheet
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        config={config}
        onConfigChange={setConfig}
      />
      <RenameDialog />
    </div>
  );
}

export default App;
