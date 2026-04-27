import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "@/lib/events";
import type { CountUpTick } from "@/types";

export function useCountUp() {
  const [state, setState] = useState<CountUpTick>({
    elapsedSecs: 0,
    running: false,
  });

  useEffect(() => {
    const unlisten = listen<CountUpTick>(EVENTS.COUNT_UP_TICK, (event) => {
      setState(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return state;
}
