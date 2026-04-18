import type { AppConfig, Theme, ThemeTokens, ThemeTokenKey } from "@/types";
import { THEME_TOKEN_ORDER } from "@/types";

// Mirror of src-tauri/src/theme.rs `builtin_themes`. Must stay in sync.
export const BUILTIN_THEMES: Theme[] = [
  {
    id: "dark",
    name: "Dark",
    builtin: true,
    tokens: {
      title_bar: "oklch(0.269 0 0)",
      app_bg: "oklch(0.145 0 0)",
      panel_bg: "oklch(0.205 0 0)",
      text: "oklch(0.985 0 0)",
      text_muted: "oklch(0.708 0 0)",
      border: "oklch(1 0 0 / 10%)",
      hover_bg: "oklch(1 0 0 / 15%)",
      g1_accent: "#2563eb",
      g2_accent: "#16a34a",
      g3_accent: "#ea580c",
      g4_accent: "#9333ea",
    },
  },
  {
    id: "light",
    name: "Light",
    builtin: true,
    tokens: {
      title_bar: "#f3f4f6",
      app_bg: "#ffffff",
      panel_bg: "#f9fafb",
      text: "#111827",
      text_muted: "#6b7280",
      border: "#e5e7eb",
      hover_bg: "rgba(0,0,0,0.06)",
      g1_accent: "#2563eb",
      g2_accent: "#16a34a",
      g3_accent: "#ea580c",
      g4_accent: "#9333ea",
    },
  },
  {
    id: "pink",
    name: "Pink",
    builtin: true,
    tokens: {
      title_bar: "#f9a8d4",
      app_bg: "#fdf2f8",
      panel_bg: "#fce7f3",
      text: "#500724",
      text_muted: "#9d174d",
      border: "#f9a8d4",
      hover_bg: "rgba(236,72,153,0.15)",
      g1_accent: "#ec4899",
      g2_accent: "#f472b6",
      g3_accent: "#db2777",
      g4_accent: "#be185d",
    },
  },
];

export function allThemes(config: AppConfig | null): Theme[] {
  const customs = config?.themes ?? [];
  return [...BUILTIN_THEMES, ...customs];
}

export function resolveTheme(config: AppConfig | null): Theme {
  const id = config?.active_theme_id ?? "dark";
  const themes = allThemes(config);
  return themes.find((t) => t.id === id) ?? BUILTIN_THEMES[0];
}

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  for (const key of THEME_TOKEN_ORDER) {
    const cssName = `--t-${key.replace(/_/g, "-")}`;
    root.style.setProperty(cssName, theme.tokens[key]);
  }
}

export function slugify(name: string): string {
  const s = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return s || "custom";
}

export function uniqueId(base: string, existing: Theme[]): string {
  const used = new Set(existing.map((t) => t.id));
  if (!used.has(base)) return base;
  for (let i = 2; i < 100; i++) {
    const cand = `${base}-${i}`;
    if (!used.has(cand)) return cand;
  }
  return `${base}-${Date.now()}`;
}

export function uniqueName(base: string, existing: Theme[]): string {
  const used = new Set(existing.map((t) => t.name));
  if (!used.has(base)) return base;
  for (let i = 2; i < 100; i++) {
    const cand = `${base} ${i}`;
    if (!used.has(cand)) return cand;
  }
  return `${base} ${Date.now()}`;
}

export function isValidCssColor(value: string): boolean {
  if (!value.trim()) return false;
  const probe = document.createElement("span");
  probe.style.color = "";
  probe.style.color = value;
  return probe.style.color !== "";
}

export function emptyTokens(): ThemeTokens {
  return { ...BUILTIN_THEMES[0].tokens };
}

export function cloneTokens(tokens: ThemeTokens): ThemeTokens {
  return { ...tokens };
}

export type { ThemeTokenKey };
