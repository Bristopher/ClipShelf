import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";
import type { LogEntry } from "@/types";

export function useEventLog() {
  const [entries, setEntries] = useState<LogEntry[]>([]);

  useEffect(() => {
    const unlisten = listen<LogEntry>(EVENTS.LOG_ENTRY, (event) => {
      setEntries((prev) => [...prev, event.payload]);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const clear = () => setEntries([]);
  const restore = (restored: LogEntry[]) => setEntries(restored);

  return { entries, clear, restore };
}
