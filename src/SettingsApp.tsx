import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getConfig } from "@/lib/commands";
import { EVENTS } from "@/lib/events";
import { refreshSystemMode } from "@/lib/systemTheme";
import { useTheme } from "@/hooks/useTheme";
import { SettingsForm } from "@/components/SettingsForm";
import type { AppConfig } from "@/types";

export function SettingsApp() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  useTheme(config);

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
      <div className="flex h-screen items-center justify-center bg-app-bg text-t-text">
        <p className="text-t-muted">Loading...</p>
      </div>
    );
  }

  return (
    <div className="h-screen overflow-y-auto bg-app-bg text-t-text">
      <div className="max-w-xl mx-auto px-6 py-6">
        <h1 className="text-lg font-semibold mb-4">Settings</h1>
        <SettingsForm config={config} onConfigChange={setConfig} />
      </div>
    </div>
  );
}
