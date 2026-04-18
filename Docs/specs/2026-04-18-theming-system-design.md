# Theming System

**Status:** ✅ Production
**Author:** Bristopher
**Date:** 2026-04-18
**Version:** 1.0.0
**Last Updated:** 2026-04-18

---

## 📋 Table of Contents

1. [Overview](#overview)
2. [Design Philosophy](#design-philosophy)
3. [Architecture](#architecture)
4. [Features](#features)
5. [Config Schema](#config-schema)
6. [API Reference](#api-reference)
7. [Configuration](#configuration)
8. [Implementation Guide](#implementation-guide)
9. [Testing](#testing)
10. [Performance](#performance)
11. [Troubleshooting](#troubleshooting)
12. [References](#references)

---

## Overview

**Purpose:** Let users pick, edit, save, import, and export visual themes for GKey Mover — controlling the title bar, main app surface, events/log panel, text, borders, and per-G-key accent colors.

**Problem it solves:** The app currently ships with a single hardcoded dark theme defined in `src/index.css`. There is no way for a user to tweak colors, try a light variant, or express a preference (e.g., a pink theme for fun). Re-skinning today requires editing CSS and rebuilding.

**Key benefits:**
- ✅ Ship multiple polished built-in themes (Dark / Light / Pink) users can switch between with zero friction.
- ✅ Let users craft custom themes with live preview, via hex input, and persist them in the existing TOML config.
- ✅ Portable themes — JSON export/import enables sharing between machines or communities.
- ✅ Per-G-key accent colors surface in the BottomBar G1/G2/G3 buttons so theming is visible at a glance, not just a chrome change.

**Use cases:**
- User prefers a cozy light-pink aesthetic while editing clips late at night.
- User has an OBS/stream scene using a specific accent palette and wants GKey Mover to match.
- Community member shares a "Synthwave" theme JSON in Discord; recipient imports it in one click.
- User builds several themes for different moods and toggles between them without restarting the app.

---

## Design Philosophy

### 1. Config is source of truth
Theme state lives in the Rust-owned `AppConfig` (same TOML file that already persists settings). The React layer never stores theme data outside of the event-driven config mirror. This preserves the project rule stated in `CLAUDE.md`: *"Rust backend owns all state. React is a pure view layer."*

**Example:**
```
Principle: Config is source of truth
Why: Prevents theme drift if the user closes/reopens the window, and reuses the existing config-changed event pipe.
Impact: Theme edits go through updateConfig() like every other setting.
```

### 2. CSS custom properties as the only runtime surface
The frontend never hardcodes hex strings once the system ships. All themable surfaces read from a fixed set of CSS variables. Switching themes means swapping the values on `document.documentElement` — no component re-render orchestration needed.

### 3. Built-ins are read-only; customs clone-on-edit
Users cannot overwrite `Dark`, `Light`, or `Pink` — attempts to edit auto-clone the theme to a custom first. This keeps a working fallback even if a user saves an unreadable theme, and makes the reset path trivial (pick a built-in).

### 4. Tri-tone by default, di-tone by convention
The token set supports three distinct surface colors (`title_bar`, `app_bg`, `panel_bg`) but users who want a 2-tone look just set `app_bg === panel_bg`. No separate "mode" flag is needed.

### 5. Narrow, named token vocabulary
Rather than exposing every shadcn/Tailwind variable, the theme schema exposes a short, stable vocabulary (~10 tokens) meant for this app specifically. That keeps export/import JSON tiny and forward-compatible, and keeps the settings form from sprawling.

### Trade-offs considered

- **Full shadcn token override vs. curated short list:** We chose the curated list. Exposing every token (`--ring`, `--chart-1`..`5`, etc.) would overwhelm the settings form and make exports brittle across Tailwind version bumps.
- **Live edit on the committed theme vs. scratch buffer:** We chose live edit of a scratch buffer that only writes to config on "Save". This avoids thrashing the TOML file on every color-picker tick and lets users abandon a bad experiment cleanly.
- **JSON export format with schema version vs. raw token dump:** We chose a versioned envelope so future migrations (added/removed tokens) can be handled without breaking old exports.
- **Theme storage inside AppConfig vs. separate file:** We chose inside AppConfig. Keeps a single round-tripped TOML file, reuses existing `merge_partial` + `config-changed` event wiring, and avoids a second filesystem watcher.

---

## Architecture

### High-Level Diagram

```
┌─────────────────────┐      ┌──────────────────────┐      ┌──────────────────────┐
│  SettingsSheet      │      │  Tauri commands      │      │  AppConfig (Rust)    │
│  (Appearance panel) │─────▶│  update_config /     │─────▶│  themes: Vec<Theme>  │
│  - theme picker     │      │  import_theme /      │      │  active_theme_id     │
│  - token editors    │      │  export_theme        │      │  persisted to TOML   │
│  - import/export    │      └──────────┬───────────┘      └──────────┬───────────┘
└─────────┬───────────┘                 │ emits "config-changed"      │
          │                             ▼                             │
          │                   ┌──────────────────────┐                │
          │                   │  React ThemeEffect   │◀───────────────┘
          │                   │  - resolves active   │
          │                   │    theme             │
          │                   │  - writes CSS vars   │
          │                   │    to :root          │
          │                   └──────────┬───────────┘
          │                              ▼
          │                   ┌──────────────────────┐
          └─────── preview ──▶│  DOM (bg-title-bar,  │
                              │  bg-panel, text-*,   │
                              │  g1/g2/g3 utilities) │
                              └──────────────────────┘
```

### Data Flow

```
1. User clicks a theme / edits a color in SettingsSheet
      ↓
2. React calls updateConfig({ themes?, active_theme_id? })
      ↓
3. Rust merges into AppConfig, saves TOML, emits "config-changed"
      ↓
4. All windows receive the new config; ThemeEffect reapplies CSS vars
      ↓
5. Styled surfaces (title bar, panels, text, G-key buttons) repaint
```

### Component Responsibilities

| Component | Responsibility | Technology |
|-----------|---------------|------------|
| `config.rs` (Rust) | Owns `Theme`, `themes: Vec<Theme>`, `active_theme_id`. Serializes to TOML. Ships built-ins. | Rust + serde + toml |
| `commands.rs` (Rust) | Exposes `import_theme`, `export_theme` commands. `update_config` already handles theme CRUD. | Tauri commands |
| `lib/themes.ts` (TS) | Built-in theme catalog mirror, `Theme` type, `applyTheme(tokens)` that writes CSS vars. | TypeScript |
| `hooks/useTheme.ts` | Watches `config.active_theme_id` + `config.themes` and applies on change. | React hook |
| `components/SettingsSheet.tsx` | Hosts the new **Appearance** section. | React + shadcn |
| `components/TitleBar.tsx` / `EventLog.tsx` / `BottomBar.tsx` | Consume CSS vars via new Tailwind utility classes. | React + Tailwind |
| `index.css` | Defines the theme-var layer; existing shadcn tokens either alias onto the new vars or stay untouched for non-themable chrome. | Tailwind v4 CSS |

---

## Features

### ✅ Implemented

*None — feature is in design. See Planned below.*

---

### 🔜 Planned

#### Feature 1: Built-in themes (Dark / Light / Pink)
**Description:** Three read-only themes shipped in code, always available in the picker.
**Status:** 🔜 Planned
**Priority:** High

**Why we need it:**
- Immediate user value without requiring theme creation.
- Fallback target if a user's custom theme becomes unreadable.
- Sets the visual reference for custom theme authors.

**Token values (shipped):**

| Token | Dark | Light | Pink |
|-------|------|-------|------|
| `title_bar` | `oklch(0.269 0 0)` | `#f3f4f6` | `#f9a8d4` |
| `app_bg` | `oklch(0.145 0 0)` | `#ffffff` | `#fdf2f8` |
| `panel_bg` | `oklch(0.205 0 0)` | `#f9fafb` | `#fce7f3` |
| `text` | `oklch(0.985 0 0)` | `#111827` | `#500724` |
| `text_muted` | `oklch(0.708 0 0)` | `#6b7280` | `#9d174d` |
| `border` | `oklch(1 0 0 / 10%)` | `#e5e7eb` | `#f9a8d4` |
| `hover_bg` | `oklch(1 0 0 / 15%)` | `rgba(0,0,0,0.06)` | `rgba(236,72,153,0.15)` |
| `g1_accent` | `#2563eb` | `#2563eb` | `#ec4899` |
| `g2_accent` | `#16a34a` | `#16a34a` | `#f472b6` |
| `g3_accent` | `#ea580c` | `#ea580c` | `#db2777` |
| `g4_accent` | `#9333ea` | `#9333ea` | `#be185d` |

#### Feature 2: Custom theme CRUD
**Description:** Users can clone the active theme, rename it, edit any token via hex input + swatch, and delete.
**Status:** 🔜 Planned
**Priority:** High

**Behavior:**
- "Edit" on a built-in → clones to a new custom named "Dark (Custom)" before accepting edits.
- "Save as new" → prompts for name, snapshots current draft tokens.
- "Delete" disabled for built-ins; confirmation required for customs.
- "Set as default" = sets `active_theme_id` (the config is already persisted, so this is effectively "apply and save").

#### Feature 3: Import / Export JSON
**Description:** Export active theme to a `.json` file via the existing `tauri-plugin-dialog` save dialog. Import reads a `.json`, validates, and adds to `themes`.
**Status:** 🔜 Planned
**Priority:** Medium

**Format:**
```json
{
  "schema": "gkey-theme-v1",
  "name": "My Theme",
  "tokens": {
    "title_bar": "#1a1a1a",
    "app_bg": "#0d0d0d",
    "panel_bg": "#151515",
    "text": "#f5f5f5",
    "text_muted": "#888",
    "border": "#2a2a2a",
    "g1_accent": "#3b82f6",
    "g2_accent": "#a855f7",
    "g3_accent": "#f59e0b"
  }
}
```

**Validation:**
- `schema` must equal `"gkey-theme-v1"` (reject otherwise with a surfaced error).
- `name` non-empty, ≤ 40 chars.
- Every token must parse as a valid CSS color (hex `#rgb`/`#rrggbb`/`#rrggbbaa`, named, `rgb()`, `oklch()`, etc.) — validated with a hidden `<span>.style.color = ...; computedStyle !== ""` check on the frontend side.
- Duplicate-name imports get a ` (imported)` suffix.

#### Feature 4: Per-G-key accent coloring
**Description:** Sidebar G1/G2/G3/G4 buttons derive their background color from the theme tokens `g1_accent` / `g2_accent` / `g3_accent` / `g4_accent`. Hover brightens the accent by 10% via `filter: brightness(1.1)` rather than a separate hover token — keeps the palette concise.
**Status:** 🔜 Planned
**Priority:** Medium

**Why we need it:**
- Theming should be visible in the primary interaction surface, not just chrome.
- Users already associate a "vibe" with each G-key (sort targets); accent colors reinforce muscle memory.

---

### 🚫 Out of Scope

#### Overriding every shadcn/Tailwind token
**Why not:** Explodes the settings form, couples exports to Tailwind's internal variable names, and gives users a thousand ways to make the app unreadable. The curated ~10-token vocabulary is deliberately narrow.

#### Per-window or per-component theme overrides
**Why not:** GKey Mover has a single always-on-top window. Adds complexity with no user benefit in this app.

#### Auto light/dark switching from OS
**Why not:** Users who want that can pick Light or Dark manually. Following OS theme would fight the "set a default" feature users explicitly asked for.

#### Animated theme transitions
**Why not:** CSS variable swaps are instant on this app's surface area; fading would require coordinating transitions across every component and offers no functional value.

---

## Config Schema

### TOML (AppConfig additions)

```toml
active_theme_id = "dark"

[[themes]]
id = "my-pink"
name = "My Pink"
builtin = false
tokens.title_bar   = "#f9a8d4"
tokens.app_bg      = "#fdf2f8"
tokens.panel_bg    = "#fce7f3"
tokens.text        = "#4a044e"
tokens.text_muted  = "#9d174d"
tokens.border      = "#f472b6"
tokens.g1_accent   = "#ec4899"
tokens.g2_accent   = "#f472b6"
tokens.g3_accent   = "#fbcfe8"
```

**Purpose:** Persists every user-created theme alongside the built-in selection. Built-in themes are **not** written to TOML — they live in code and are merged into the picker at read time.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `active_theme_id` | `String` | ID of the currently applied theme. Must match a built-in or an entry in `themes`. Defaults to `"dark"`. |
| `themes[].id` | `String` | Stable identifier. Generated as a slug from `name` at create time. |
| `themes[].name` | `String` | User-visible name. |
| `themes[].builtin` | `bool` | Always `false` for config entries. Field exists for schema symmetry with the runtime theme union. |
| `themes[].tokens` | `Tokens` | Map of token name → CSS color string. |

**Built-in enumeration (code-only):**

```rust
// src-tauri/src/theme.rs
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        Theme { id: "dark".into(),  name: "Dark".into(),  builtin: true, tokens: dark_tokens()  },
        Theme { id: "light".into(), name: "Light".into(), builtin: true, tokens: light_tokens() },
        Theme { id: "pink".into(),  name: "Pink".into(),  builtin: true, tokens: pink_tokens()  },
    ]
}
```

### Defaults

`active_theme_id` defaults to `"dark"` and `themes` defaults to `vec![]` — built-ins are always present regardless of config state.

### Data Model Diagram

```
AppConfig
 ├── active_theme_id: String   ──▶ resolves to one of:
 │                                  - builtin_themes()[0..3]  (code)
 │                                  - themes[*]               (TOML)
 └── themes: Vec<Theme>
      └── Theme
           ├── id
           ├── name
           ├── builtin  (false for TOML-stored customs)
           └── tokens: { title_bar, app_bg, panel_bg,
                         text, text_muted, border,
                         g1_accent, g2_accent, g3_accent }
```

---

## API Reference

### Tauri Commands

#### `get_config` (existing — unchanged)
Returns the full config including `themes` and `active_theme_id`. Frontend combines this with the built-in list at runtime.

#### `update_config` (existing — covers theme CRUD)
Already supports arbitrary partial updates via `merge_partial`. Used to add/modify/delete custom themes and to change `active_theme_id`.

**Example — apply a theme:**
```ts
await updateConfig({ active_theme_id: "my-pink" });
```

**Example — save a new custom theme:**
```ts
await updateConfig({
  themes: [
    ...existingCustoms,
    { id: "my-pink", name: "My Pink", builtin: false, tokens: {...} },
  ],
});
```

#### `import_theme` (new)

**Description:** Reads a theme JSON file at the given path, validates, and returns the parsed `Theme` ready to be saved via `update_config`.

**Arguments:**
- `path` (String): absolute path to a `.json` file selected via `tauri-plugin-dialog`.

**Response:**
```json
{
  "id": "my-pink-imported",
  "name": "My Pink (imported)",
  "builtin": false,
  "tokens": { "title_bar": "#f9a8d4", ... }
}
```

**Errors:**
- `"invalid schema"` — `schema` field missing or not `"gkey-theme-v1"`.
- `"invalid color at {token}"` — any token fails color parsing.
- `"io: {msg}"` — file read failure.

#### `export_theme` (new)

**Description:** Writes a theme (resolved by ID) as JSON to the given path.

**Arguments:**
- `path` (String): absolute path to the target `.json` file.
- `theme_id` (String): the theme to export.

**Response:** `()` on success, `String` error otherwise.

### Event Bus

No new events. The existing `config-changed` event carries the full updated config, and the `useTheme` hook derives the active theme from that.

---

## Configuration

### Runtime state
The full theme system is runtime-driven via the existing config — no environment variables, build flags, or CLI arguments.

### Disabling the feature
Not supported. Dark remains the default and behaves identically to today's hardcoded theme if the user never touches the setting.

### Migration from pre-theming configs
Configs written before this feature lack `themes` and `active_theme_id`. serde `#[serde(default)]` attributes on both fields let old configs load cleanly with:
- `themes = []`
- `active_theme_id = "dark"` (via `default_active_theme_id()`)

---

## Implementation Guide

### Prerequisites
- Node 20+, pnpm
- Rust 1.80+ with Tauri v2 toolchain
- No new dependencies — uses existing `serde`, `serde_json`, `toml`, `@tauri-apps/plugin-dialog`.

### Step-by-step

**1. Backend — new theme module**

Create `src-tauri/src/theme.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    pub tokens: BTreeMap<String, String>,
}

pub fn builtin_themes() -> Vec<Theme> { /* ... */ }
```

**2. Extend `AppConfig`**

In `config.rs`:
```rust
#[serde(default = "default_active_theme_id")]
pub active_theme_id: String,

#[serde(default)]
pub themes: Vec<Theme>,

fn default_active_theme_id() -> String { "dark".to_string() }
```

Update `Default` impl. No changes needed to `merge_partial` — it's already generic.

**3. New commands**

In `commands.rs`, add `import_theme(path)` and `export_theme(path, theme_id)`. Register in `lib.rs` `generate_handler!`.

**4. Frontend types**

In `src/types/index.ts`:
```ts
export type ThemeTokens = {
  title_bar: string; app_bg: string; panel_bg: string;
  text: string; text_muted: string; border: string;
  g1_accent: string; g2_accent: string; g3_accent: string;
};
export type Theme = { id: string; name: string; builtin: boolean; tokens: ThemeTokens };
```

Add `themes: Theme[]` and `active_theme_id: string` to `AppConfig`.

**5. Built-in catalog mirror**

Create `src/lib/themes.ts`:
```ts
export const BUILTIN_THEMES: Theme[] = [ /* same values as Rust builtin_themes */ ];
export function resolveTheme(cfg: AppConfig): Theme {
  const all = [...BUILTIN_THEMES, ...cfg.themes];
  return all.find(t => t.id === cfg.active_theme_id) ?? BUILTIN_THEMES[0];
}
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  for (const [k, v] of Object.entries(theme.tokens)) {
    root.style.setProperty(`--t-${k.replace(/_/g, "-")}`, v);
  }
}
```

**6. Wire into CSS**

In `src/index.css`, add a base layer that introduces the theme vars and bridges them to shadcn tokens:
```css
@layer base {
  :root {
    --t-title-bar: oklch(0.205 0 0);
    --t-app-bg:    oklch(0.145 0 0);
    --t-panel-bg:  oklch(0.20 0 0);
    --t-text:      oklch(0.985 0 0);
    --t-text-muted:oklch(0.708 0 0);
    --t-border:    oklch(1 0 0 / 10%);
    --t-g1-accent: #3b82f6;
    --t-g2-accent: #a855f7;
    --t-g3-accent: #f59e0b;
  }
  html, body { background: var(--t-app-bg); color: var(--t-text); }
}
```

Add Tailwind utility mappings (`bg-title-bar`, `bg-panel`, `border-themed`, `text-muted-themed`, `bg-g1`, `bg-g2`, `bg-g3`) in the Tailwind v4 `@theme` block so classes like `bg-title-bar` emit `background: var(--t-title-bar)`.

**7. Replace hardcoded classes**

- `TitleBar.tsx`: `bg-secondary/80` → `bg-title-bar` (keep `/80` via `/80` modifier or fold into the token as `hsla` if needed).
- `EventLog.tsx`: panel surface → `bg-panel`.
- `BottomBar.tsx` G-key buttons → `bg-g1` / `bg-g2` / `bg-g3` (with hover states derived from the accent).
- Muted-text `text-muted-foreground` stays on shadcn's token but we alias `--muted-foreground` → `var(--t-text-muted)` in `:root`.

**8. React theme hook**

Create `src/hooks/useTheme.ts`:
```ts
export function useTheme(config: AppConfig | null) {
  useEffect(() => {
    if (!config) return;
    applyTheme(resolveTheme(config));
  }, [config?.active_theme_id, config?.themes]);
}
```

Call it from `App.tsx` right after the config loads.

**9. Appearance panel in SettingsSheet**

New section with:
- `Select` for theme picker (built-ins + customs, with a `(built-in)` suffix).
- Token grid: for each token, `Label` + hex `Input` + a color swatch `<div style={{ background }}>`.
- Actions row: `Edit` (disabled on built-ins), `Save as new`, `Delete`, `Import`, `Export`.
- Preview of the current draft is always live against `:root` (so every keystroke in a hex box updates the app).

Use `tauri-plugin-dialog`'s `save` / `open` with `defaultPath` and `filters: [{ name: "Theme", extensions: ["json"] }]`.

**10. Capability additions**

No new Tauri permissions required — `update_config` already runs, and file reads go through the dialog plugin already allowed in `capabilities/default.json` (`opener:default` is unrelated; dialog is implicitly allowed when the plugin is registered). Confirm by a quick run.

---

## Testing

### Rust unit tests (`src-tauri/src/theme.rs`)

```
cargo test -p gkey_mover_v2_lib theme
```

Cover:
- `builtin_themes()` returns 3 themes with expected IDs.
- `Theme` round-trips through TOML with only custom themes written.
- `import_theme` rejects wrong schema, rejects bogus color values, accepts well-formed JSON.
- `export_theme` writes valid JSON that re-imports to an equal struct.

### Frontend unit tests

```
pnpm test
```

Cover:
- `resolveTheme` returns the built-in when `active_theme_id` is unknown.
- `applyTheme` writes the expected `--t-*` variables onto `document.documentElement`.
- Hex-input validation accepts `#fff`, `#ffffff`, `#ffffffff`, rejects `#ggg`.

### Manual testing

1. `pnpm tauri dev`.
2. Open Settings → Appearance. Confirm three built-ins appear.
3. Switch to Light — title bar, panel, and text update instantly.
4. Switch to Pink — G-key button backgrounds in the BottomBar update.
5. Click Edit on Pink — expect a new `Pink (Custom)` entry auto-selected.
6. Tweak `title_bar` to `#00ff00`. The bar turns green live.
7. Save as new → name it "Lime". Switch to Dark, then back to Lime — preserves.
8. Export Lime to `lime.json`. Inspect file; confirm `schema: "gkey-theme-v1"` envelope.
9. Delete Lime. Re-import from `lime.json`. Theme returns.
10. Restart app. Active theme persists across restart.
11. Hand-edit TOML to set `active_theme_id = "nonexistent"`. Restart. App falls back to Dark without crashing.

---

## Performance

### Expected Metrics

| Metric | Value | Notes |
|--------|-------|-------|
| Theme-switch repaint | < 16 ms | CSS variable swap; no React re-render cascade. |
| Config save latency | < 10 ms | Existing TOML write; themes add ~1 KB per custom. |
| Import validation | < 5 ms | Pure JSON parse + color-string check. |
| Memory overhead | < 50 KB | Themes are strings; negligible even at 100 custom themes. |

### Optimizations Applied

1. **CSS-var mechanism** — single DOM write per switch; zero component-tree churn.
2. **Debounced color input** — hex-input edits are applied to `:root` immediately (cheap) but only committed to config on blur or explicit "Save" to avoid TOML-thrashing on every keystroke.
3. **No runtime theme generation** — built-ins are static `const` tables on both Rust and TS sides.

### Benchmarks

Not instrumented. If a user reports lag, capture a Chromium DevTools performance trace while switching themes and share — expect the cost to live entirely in the paint phase, not JS.

---

## Troubleshooting

#### Issue: Theme changes don't persist across restart

**Symptoms:** Selected theme reverts to Dark after restarting the app.

**Cause:** `active_theme_id` not being saved — usually because the new `AppConfig` field wasn't added to the `Default` impl or serde derives.

**Solution:**
1. Inspect `gkey_config.toml` next to the executable — confirm `active_theme_id` is present.
2. `cargo test` to ensure serde round-trips work.
3. If `themes` is missing for a custom theme, verify the `Vec<Theme>` field on `AppConfig` has `#[serde(default)]`.

---

#### Issue: Imported theme looks broken / unreadable

**Symptoms:** After import, text blends into background.

**Cause:** Hex values are valid CSS but produce no contrast (e.g., white text on white).

**Solution:**
1. Switch back to a built-in via the dropdown.
2. Open the custom theme and adjust `text` vs `app_bg` / `panel_bg`.
3. (Future enhancement:) add a WCAG contrast warning on save.

---

#### Issue: Built-in theme got modified

**Symptoms:** Dark looks off after an edit session.

**Cause:** Should be impossible — built-ins are code-side and not written to TOML. If it happens, something in SettingsSheet bypassed the clone-on-edit rule.

**Solution:**
1. Confirm by inspecting TOML: no entry with `id = "dark"` should exist.
2. If present, delete it manually; `resolveTheme` will fall back to the code-side built-in.
3. File a bug — `Edit` should clone before accepting changes.

---

#### Issue: Color picker updates every token on first open

**Symptoms:** First color change flashes every token.

**Cause:** Usually a React effect dependency missing `config.themes` — applies stale tokens before re-rendering.

**Solution:** Ensure `useTheme(config)` depends on both `active_theme_id` and `themes`, and that `applyTheme` reads from the resolved theme freshly each call.

---

## References

### Internal Documentation

- [`Docs/specs/2026-04-15-gkey-mover-v2-design.md`](./2026-04-15-gkey-mover-v2-design.md) — overall app architecture this theme system plugs into.
- `CLAUDE.md` — project rules (Rust owns state, React is pure view layer).
- `src/index.css` — current single-theme definition that this system generalizes.

### External Resources

- [Tauri v2 — Commands & State](https://v2.tauri.app/develop/calling-rust/)
- [Tailwind v4 `@theme` directive](https://tailwindcss.com/docs/v4)
- [CSS `oklch()` color function](https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/oklch)
- [MDN — Using CSS custom properties](https://developer.mozilla.org/en-US/docs/Web/CSS/Using_CSS_custom_properties)

### Related Features

- **Window opacity** (`set_window_opacity` command) — orthogonal but often tuned together with theme choice; no shared state.
- **Config round-trip (`merge_partial`)** — the generic mechanism that enables theme CRUD with zero new commands.

---

## Changelog

### v1.0.0 — 2026-04-18
- Shipped. 11-token vocabulary (added `hover_bg` for button hover surface, and `g4_accent` for the Rename button). Dark / Light / Pink built-ins, custom theme CRUD, import/export.

### v0.1.0 — 2026-04-18
- Initial design doc.

---

**Maintainer:** Bristopher
**Status:** 🔜 Design complete — awaiting implementation sign-off.
