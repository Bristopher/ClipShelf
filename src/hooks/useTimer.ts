import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";
import type { TimerTick } from "@/types";

export function useTimer(initialTotalSecs: number) {
  const [state, setState] = useState({
    remainingSecs: initialTotalSecs,
    totalSecs: initialTotalSecs,
    running: false,
  });

  useEffect(() => {
    const unlistenTick = listen<TimerTick>(EVENTS.TIMER_TICK, (event) => {
      setState({
        remainingSecs: event.payload.remainingSecs,
        totalSecs: event.payload.totalSecs,
        running: event.payload.remainingSecs > 0,
      });
    });
    const unlistenExpired = listen(EVENTS.TIMER_EXPIRED, () => {
      setState((prev) => ({ ...prev, remainingSecs: prev.totalSecs, running: false }));
    });
    return () => {
      unlistenTick.then((fn) => fn());
      unlistenExpired.then((fn) => fn());
    };
  }, []);

  return state;
}
