# GKey Mover

Sort and rename OBS / ShadowPlay clips the moment you save them — with
G-key hotkeys, automatic game detection, a clip history, and an in-game
overlay that never steals focus from your game.

Windows desktop app built with Tauri v2 (Rust backend, React frontend).

## Features

- **G-key sorting** — press G1–G3 to move the latest clip into your named
  folders, G4 to rename it; drag-and-drop works too
- **Game detection** — every clip records which game (or desktop app) was
  focused when you saved it, written into Explorer-visible file properties
  (Tags / Rating / Comments)
- **Clip history** — today's clips grouped by game with a configurable
  4 AM day rollover, full history by day, right-click actions, one-click
  game fixes with "remember" overrides
- **In-game overlay** (default `Shift+F1`) — CS:GO-buy-menu-style panel to
  sort, star-rate, label, describe, or re-tag the current clip with number
  keys, without your game losing focus or minimizing
- **Themes** — dark/light/custom, applied everywhere including the tray menu
- **In-app updates** — checks GitHub releases on launch (consent-based,
  never silent; can be disabled in Settings)

## Install

Grab the latest release from
[Releases](https://github.com/Bristopher/GKeyMover/releases/latest):

- `GKeyMover_x.y.z_x64-setup.exe` — installer (recommended; enables
  in-place delta updates)
- `GKeyMover_x.y.z_x64-Portable.exe` — single-file portable build
  (update checks open the releases page instead of self-updating)

## Develop

```
pnpm install
pnpm tauri dev           # hot-reload frontend, rebuilds Rust
cd src-tauri && cargo test
```

Architecture and feature docs live in `Docs/` (see `CLAUDE.md` for the
map). Releases are published with `.\build-release.ps1` — see
[RELEASING.md](RELEASING.md).

## Reading clip metadata from other apps

`Docs/Features/Clip-Metadata-Interop.md` documents the label/rating/game
contract (filename suffix, Windows property IDs, `history.jsonl` schema)
with PowerShell/Python/C#/Node reader examples.
