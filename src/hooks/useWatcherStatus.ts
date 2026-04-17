import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";

export function useWatcherStatus() {
  const [status, setStatus] = useState({ status: "stopped", restartCount: 0 });

  useEffect(() => {
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
