import { useEffect, useState } from "react";
import { getConfig } from "./lib/commands";
import type { AppConfig } from "./types";

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);

  useEffect(() => {
    getConfig().then(setConfig).catch(console.error);
  }, []);

  if (!config) {
    return (
      <div className="flex h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    );
  }

  return (
    <div className="flex h-screen">
      {/* Sidebar placeholder */}
      <aside className="w-20 border-r border-border flex flex-col gap-2 p-2">
        <div className="text-xs text-muted-foreground text-center">G-Keys</div>
      </aside>

      {/* Main content */}
      <main className="flex-1 flex flex-col">
        <div className="flex-1 p-2">
          <p className="text-sm text-muted-foreground">Event log will go here</p>
        </div>
        <div className="border-t border-border p-2 text-xs text-muted-foreground">
          Bottom bar
        </div>
      </main>

      {/* Timer placeholder */}
      <div className="w-24 border-l border-border flex items-center justify-center">
        <span className="text-2xl font-bold font-mono">
          {config.timer_enabled
            ? `${String(Math.floor(config.timer_duration_ms / 60000)).padStart(2, "0")}:${String(Math.floor((config.timer_duration_ms % 60000) / 1000)).padStart(2, "0")}`
            : "--:--"}
        </span>
      </div>
    </div>
  );
}

export default App;
