import { invoke } from "@tauri-apps/api/core";

export type SystemMode = "light" | "dark" | null;

let current: SystemMode = null;
let inFlight: Promise<SystemMode> | null = null;
const subs = new Set<(m: SystemMode) => void>();

export function getSystemMode(): SystemMode {
  return current;
}

export function subscribeSystemMode(fn: (m: SystemMode) => void): () => void {
  subs.add(fn);
  return () => {
    subs.delete(fn);
  };
}

function raceWithTimeout<T>(p: Promise<T>, ms: number, fallback: T): Promise<T> {
  return new Promise((resolve) => {
    let done = false;
    const timer = setTimeout(() => {
      if (!done) {
        done = true;
        resolve(fallback);
      }
    }, ms);
    p.then((v) => {
      if (!done) {
        done = true;
        clearTimeout(timer);
        resolve(v);
      }
    }).catch(() => {
      if (!done) {
        done = true;
        clearTimeout(timer);
        resolve(fallback);
      }
    });
  });
}

/**
 * Reads the OS theme (Windows only). Subsequent calls while one is in flight
 * share the same promise. Falls back to `null` if the backend is unreachable
 * or takes longer than 500ms — the settings sheet must never hang on this.
 */
export async function refreshSystemMode(): Promise<SystemMode> {
  if (inFlight) return inFlight;
  inFlight = raceWithTimeout<SystemMode>(
    invoke<SystemMode>("get_system_theme_mode"),
    500,
    null,
  )
    .then((mode) => {
      if (mode !== current) {
        current = mode;
        subs.forEach((fn) => fn(current));
      }
      return current;
    })
    .finally(() => {
      inFlight = null;
    });
  return inFlight;
}
