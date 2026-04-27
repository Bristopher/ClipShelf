import { useEffect, useState } from "react";
import type { AppConfig, Theme } from "@/types";
import { applyTheme, resolveTheme } from "@/lib/themes";
import {
  getSystemMode,
  refreshSystemMode,
  subscribeSystemMode,
  type SystemMode,
} from "@/lib/systemTheme";

/**
 * Applies the active theme on every config change. Also watches the OS
 * theme so that if `active_theme_id === "system"` and the OS theme flips
 * (detected on settings-open or app mount), the app repaints to match.
 *
 * `flashOverride` lets a caller temporarily swap themes without mutating
 * `active_theme_id` — used by the ≤5s timer flash to alternate between the
 * user's theme and a contrasting one each second. When it goes back to
 * null, the active theme is reapplied.
 */
export function useTheme(config: AppConfig | null, flashOverride: Theme | null) {
  const [systemMode, setSystemMode] = useState<SystemMode>(getSystemMode());

  // Refresh once on mount — fire-and-forget; the timeout inside
  // refreshSystemMode() guarantees we never hang.
  useEffect(() => {
    refreshSystemMode();
    const unsub = subscribeSystemMode((m) => setSystemMode(m));
    return () => unsub();
  }, []);

  useEffect(() => {
    if (!config) return;
    applyTheme(flashOverride ?? resolveTheme(config));
  }, [config?.active_theme_id, config?.themes, systemMode, flashOverride]);
}
