import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { TimerTick } from "@/types";

interface UseTimerOpts {
  tickEvent?: string;
  expiredEvent?: string;
}

/**
 * Subscribes to a single Rust-side timer instance. Two are running:
 *   - default (`timer-tick` / `timer-expired`) → auto-wipe on file arrival
 *   - user (`user-timer-tick` / `user-timer-expired`) → manual countdown
 */
export function useTimer(
  initialTotalSecs: number,
  opts: UseTimerOpts = {},
) {
  const tickEvent = opts.tickEvent ?? "timer-tick";
  const expiredEvent = opts.expiredEvent ?? "timer-expired";

  const [state, setState] = useState({
    remainingSecs: initialTotalSecs,
    totalSecs: initialTotalSecs,
    running: false,
  });

  useEffect(() => {
    const unlistenTick = listen<TimerTick>(tickEvent, (event) => {
      setState({
        remainingSecs: event.payload.remainingSecs,
        totalSecs: event.payload.totalSecs,
        running: event.payload.remainingSecs > 0,
      });
    });
    const unlistenExpired = listen(expiredEvent, () => {
      setState((prev) => ({ ...prev, remainingSecs: 0, running: false }));
    });
    return () => {
      unlistenTick.then((fn) => fn());
      unlistenExpired.then((fn) => fn());
    };
  }, [tickEvent, expiredEvent]);

  return state;
}
