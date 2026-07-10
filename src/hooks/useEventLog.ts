import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";
import type { LogEntry } from "@/types";

// Keep the rendered log bounded — with autostart the app runs for days and
// an ever-growing array means unbounded memory + render cost. The backend
// keeps a larger history; Restore re-fetches from there.
const MAX_ENTRIES = 500;

export function useEventLog() {
  const [entries, setEntries] = useState<LogEntry[]>([]);

  useEffect(() => {
    const unlisten = listen<LogEntry>(EVENTS.LOG_ENTRY, (event) => {
      setEntries((prev) => [...prev, event.payload].slice(-MAX_ENTRIES));
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Stable references so effects that depend on these don't re-register
  // their listeners on every App re-render — that's what was making
  // timer-expired sometimes drop and auto-wipe feel broken.
  const clear = useCallback(() => setEntries([]), []);
  const restore = useCallback(
    (restored: LogEntry[]) => setEntries(restored),
    [],
  );

  return { entries, clear, restore };
}
