# ClipShelf (formerly Gkey Mover v2)

## What This Is
Tauri v2 desktop app (Rust backend + React frontend) for sorting/renaming OBS/ShadowPlay video clips via G-key hotkeys. Rewrite of the Python/Tkinter v1. Renamed to ClipShelf 2026-07-17; internal IDs deliberately keep the old name — the Velopack packId / Tauri identifier `com.cbuzi.gkey-mover-v2`, the `gkey-mover-v2` crate/exe name, and the `%APPDATA%\com.cbuzi.gkey-mover-v2` config dir must NEVER change (installed apps update and locate config by them).

## Architecture
- Rust backend owns all state. React is a pure view layer.
- Communication: Tauri commands (frontend->Rust) and Tauri events (Rust->frontend).
- No polling. Event-driven via tokio channels.

## Dev Commands
```
pnpm tauri dev          # Run in dev mode (hot reload frontend, rebuilds Rust)
pnpm tauri build        # Production build
cd src-tauri && cargo test  # Run Rust tests
pnpm test               # Run frontend tests
```

## Key Files
- `src-tauri/src/lib.rs` — Tauri builder, registers all commands
- `src-tauri/src/state.rs` — AppState (Arc<Mutex<_>>)
- `src-tauri/src/config.rs` — TOML config struct
- `src-tauri/src/commands.rs` — All IPC command handlers
- `src/App.tsx` — Root React component

## Design Spec
See `Docs/specs/2026-04-15-gkey-mover-v2-design.md` for full architecture.
