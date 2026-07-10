import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";
import { getWatcherStatus } from "@/lib/commands";

export function useWatcherStatus() {
  const [status, setStatus] = useState({ status: "stopped", restartCount: 0 });

  useEffect(() => {
    // Status events usually fire during backend setup, before this listener
    // exists — fetch the current value so the UI doesn't show a stale guess.
    getWatcherStatus()
      .then((s) => setStatus({ status: s.status, restartCount: s.restartCount ?? 0 }))
      .catch(() => {});
    const unlisten = listen<{ status: string; restartCount?: number }>(
      EVENTS.WATCHER_STATUS,
      (e) => {
        setStatus({ status: e.payload.status, restartCount: e.payload.restartCount ?? 0 });
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return status;
}
