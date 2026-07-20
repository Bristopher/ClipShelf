import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";
import type { LogEntry } from "@/types";

// Keep the rendered log bounded — with autostart the app runs for days and
// an ever-growing array means unbounded memory + render cost. The backend
// keeps a larger history; Restore re-fetches from there.
const MAX_ENTRIES = 500;

/**
 * Append a live log entry, collapsing repeated watcher-status lines.
 * The watcher restarts on every sleep/resume (>10s wall-clock gap), logging
 * "Watcher running" after each wake — the LIVE log keeps only the latest
 * line per status message (fresh timestamp, moves to the bottom). Restore
 * still shows every occurrence from the backend history.
 */
export function appendLogEntry(
  prev: LogEntry[],
  e: LogEntry,
  max = MAX_ENTRIES,
): LogEntry[] {
  const base =
    e.category === "watcher-status"
      ? prev.filter(
          (p) => !(p.category === "watcher-status" && p.message === e.message),
        )
      : prev;
  return [...base, e].slice(-max);
}

export function useEventLog() {
  const [entries, setEntries] = useState<LogEntry[]>([]);

  useEffect(() => {
    const unlisten = listen<LogEntry>(EVENTS.LOG_ENTRY, (event) => {
      setEntries((prev) => appendLogEntry(prev, event.payload));
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
