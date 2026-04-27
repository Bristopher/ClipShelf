import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getConfig } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { refreshSystemMode } from "@/lib/systemTheme";
import { useTheme } from "@/hooks/useTheme";
import { SettingsForm } from "@/components/SettingsForm";
import { WindowChrome } from "@/components/WindowChrome";
import type { AppConfig } from "@/types";

export function SettingsApp() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  useTheme(config, null);

  useEffect(() => {
    getConfig().then(setConfig).catch(console.error);
    refreshSystemMode().catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<AppConfig>(EVENTS.CONFIG_CHANGED, (event) => {
      setConfig(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (!config) {
    return (
      <div className="flex flex-col h-screen bg-app-bg text-t-text">
        <WindowChrome title="Settings" />
        <div className="flex-1 flex items-center justify-center">
          <p className="text-t-muted">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-app-bg text-t-text">
      <WindowChrome title="Settings" />
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-xl mx-auto px-6 py-6">
          <SettingsForm config={config} onConfigChange={setConfig} />
        </div>
      </div>
    </div>
  );
}
