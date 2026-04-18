import { useEffect } from "react";
import type { AppConfig } from "@/types";
import { applyTheme, resolveTheme } from "@/lib/themes";

export function useTheme(config: AppConfig | null) {
  useEffect(() => {
    if (!config) return;
    applyTheme(resolveTheme(config));
  }, [config?.active_theme_id, config?.themes]);
}
