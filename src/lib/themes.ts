import type { AppConfig, Theme, ThemeTokens, ThemeTokenKey } from "@/types";
import { THEME_TOKEN_ORDER } from "@/types";
import { getSystemMode } from "@/lib/systemTheme";

export const SYSTEM_THEME_ID = "system";

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

// Pseudo-theme that resolves to Dark or Light based on the OS setting.
// Tokens match Dark at construction time but are never used directly —
// resolveTheme() forwards to the real built-in before applying.
const SYSTEM_PSEUDO_THEME: Theme = {
  id: SYSTEM_THEME_ID,
  name: "Match Windows",
  builtin: true,
  tokens: { ...BUILTIN_THEMES[0].tokens },
};

export function allThemes(config: AppConfig | null): Theme[] {
  const customs = config?.themes ?? [];
  return [SYSTEM_PSEUDO_THEME, ...BUILTIN_THEMES, ...customs];
}

export function resolveTheme(config: AppConfig | null): Theme {
  const id = config?.active_theme_id ?? "dark";
  if (id === SYSTEM_THEME_ID) {
    const mode = getSystemMode();
    const targetId = mode === "light" ? "light" : "dark";
    return BUILTIN_THEMES.find((t) => t.id === targetId) ?? BUILTIN_THEMES[0];
  }
  const themes = [...BUILTIN_THEMES, ...(config?.themes ?? [])];
  return themes.find((t) => t.id === id) ?? BUILTIN_THEMES[0];
}

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  for (const key of THEME_TOKEN_ORDER) {
    const cssName = `--t-${key.replace(/_/g, "-")}`;
    root.style.setProperty(cssName, theme.tokens[key]);
  }
  // Cache resolved bg+text so the next app open can paint the current theme
  // immediately via the inline script in index.html, avoiding any flash.
  try {
    const bg = resolveCssColor(theme.tokens.app_bg);
    const text = resolveCssColor(theme.tokens.text);
    if (bg && text) {
      localStorage.setItem("gkey-theme-paint", JSON.stringify({ bg, text }));
      // The boot <style> is unlayered so it outranks Tailwind's layered
      // `body { bg-background }` rule forever — rewrite it on every theme
      // application or the window background stays stuck on the boot color
      // (visible as "only the edges change" during the timer flash).
      const boot = document.getElementById("theme-boot-paint");
      if (boot) {
        boot.textContent = `html,body,#root{background:${bg};color:${text};margin:0;}`;
      }
    }
  } catch {
    /* noop */
  }
}

/**
 * Resolves any valid CSS color string ("oklch(...)", "#abc", "rgba(...)") to
 * a concrete hex/rgb string via the browser's own parser. Needed because
 * the inline script in index.html must hand the color to CSS without
 * re-running through the full theming pipeline.
 */
function resolveCssColor(color: string): string | null {
  const canvas = document.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.fillStyle = "#000";
  ctx.fillStyle = color;
  const out = ctx.fillStyle.toString();
  return out || null;
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

/**
 * Theme to swap to during the ≤5s timer flash. If the user picked one
 * explicitly via `timer_flash_theme_id`, use that. Otherwise auto-pick the
 * contrasting built-in: light active theme → dark, dark → light.
 */
export function resolveFlashTheme(config: AppConfig): Theme {
  const all = [...BUILTIN_THEMES, ...(config.themes ?? [])];
  if (config.timer_flash_theme_id) {
    const explicit = all.find((t) => t.id === config.timer_flash_theme_id);
    if (explicit) return explicit;
  }
  const active = resolveTheme(config);
  const targetId = isLightTheme(active) ? "dark" : "light";
  return BUILTIN_THEMES.find((t) => t.id === targetId) ?? BUILTIN_THEMES[0];
}

/**
 * Rough light-vs-dark classification by relative luminance of the theme's
 * `app_bg`. Uses the canvas trick to normalize any CSS color (oklch, hex,
 * rgba) to an rgb() string we can parse.
 */
export function isLightTheme(theme: Theme): boolean {
  const rgb = parseColorToRgb(theme.tokens.app_bg);
  if (!rgb) return false;
  const lum = (0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b) / 255;
  return lum > 0.5;
}

function parseColorToRgb(color: string): { r: number; g: number; b: number } | null {
  const resolved = resolveCssColor(color);
  if (!resolved) return null;
  // Browser's canvas returns rgb()/rgba() form.
  const m = resolved.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  if (!m) return null;
  return { r: Number(m[1]), g: Number(m[2]), b: Number(m[3]) };
}

export function emptyTokens(): ThemeTokens {
  return { ...BUILTIN_THEMES[0].tokens };
}

export function cloneTokens(tokens: ThemeTokens): ThemeTokens {
  return { ...tokens };
}

export type { ThemeTokenKey };
