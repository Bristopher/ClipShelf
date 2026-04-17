import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";

export function useObsStatus() {
  const [status, setStatus] = useState({ status: "disconnected", attempt: 0 });

  useEffect(() => {
    const unlisten = listen<{ status: string; attempt?: number }>(
      EVENTS.OBS_WS_STATUS,
      (e) => {
        setStatus({ status: e.payload.status, attempt: e.payload.attempt ?? 0 });
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return status;
}
