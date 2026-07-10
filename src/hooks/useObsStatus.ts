import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";
import { getObsStatus } from "@/lib/commands";

export function useObsStatus() {
  const [status, setStatus] = useState({ status: "disconnected", attempt: 0 });

  useEffect(() => {
    // The actor connects during backend setup — fetch the current status so
    // a connection made before the webview loaded still shows green.
    getObsStatus()
      .then((s) => setStatus({ status: s.status, attempt: s.attempt ?? 0 }))
      .catch(() => {});
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
