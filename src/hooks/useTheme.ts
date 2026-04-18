import { useEffect, useState } from "react";
import type { AppConfig } from "@/types";
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
 */
export function useTheme(config: AppConfig | null) {
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
    applyTheme(resolveTheme(config));
  }, [config?.active_theme_id, config?.themes, systemMode]);
}
