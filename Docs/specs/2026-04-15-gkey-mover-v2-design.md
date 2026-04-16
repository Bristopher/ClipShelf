# Gkey Mover v2 — Tauri Conversion & Modern Redesign

**Status:** 🏗️ In Development
**Author:** Chris
**Date:** 2026-04-15
**Version:** 2.0.0
**Last Updated:** 2026-04-15

---

## Table of Contents

1.  [Overview](#1-overview)
2.  [Design Philosophy](#2-design-philosophy)
3.  [Tech Stack](#3-tech-stack)
4.  [Architecture](#4-architecture)
5.  [Rust Backend Modules](#5-rust-backend-modules)
6.  [React Frontend](#6-react-frontend)
7.  [IPC Contract](#7-ipc-contract)
8.  [Configuration](#8-configuration)
9.  [Error Handling & Resilience](#9-error-handling--resilience)
10. [Project Structure](#10-project-structure)
11. [Migration from v1](#11-migration-from-v1)
12. [Testing](#12-testing)
13. [Performance](#13-performance)
14. [Troubleshooting](#14-troubleshooting)
15. [References](#15-references)

---

## 1. Overview

**Purpose:** Gkey Mover v2 is a complete rewrite of the original Python/Tkinter clip sorting utility as a Tauri desktop app with a Rust backend and React frontend.

**Problem it solves:** The v1 Python app suffers from:
- Threading lag — Tkinter mainloop + PySide6 tray + watchdog + keyboard hooks all fighting for the GIL
- Race conditions — 30+ mutable globals accessed from multiple threads with inconsistent locking
- Consistency bugs — command queue polling at 100ms intervals, timer drift, missed events after sleep/resume
- Fragile stdout redirect — all logging routed through a `StdoutRedirector` that modifies Tkinter widgets from worker threads

**Key benefits:**
- Zero-lag event handling via Rust's async runtime (`tokio`) and native channels
- Single binary, ~5MB (vs Python + PySide6 + venv = 200MB+)
- Modern UI with dark theme, settings panel, and visual G-key sidebar
- No GIL, no threading spaghetti, no command queue polling
- Native Windows hotkey support including F13-F24 via Raw Input API

**Use cases:**
- OBS/ShadowPlay clip sorting after saving a replay buffer
- Quick rename and categorization of game recordings via G-keys
- Hands-free clip management during gaming sessions

---

## 2. Design Philosophy

### A. Event-Driven, Not Poll-Driven

v1 polls a command queue every 100ms and checks file watcher liveness every 1s. v2 is fully event-driven: file system events, hotkey presses, and timer ticks flow through `tokio` channels. The frontend subscribes to Tauri events — no polling, no `after()` scheduling, no sleep loops.

**Impact:** Eliminates all timer drift, reduces CPU usage to near-zero when idle, and removes the entire class of "event lost between polls" bugs.

### B. Rust Owns All State

The React frontend is a pure view layer. It renders what Rust tells it and sends commands back. There is no client-side state that the backend doesn't know about. This eliminates the v1 pattern where `current_filename`, `renamed`, `bind_chosen`, and other globals could desync between threads.

**Impact:** Single source of truth. No race conditions. State changes are atomic and sequential in the Rust event loop.

### C. Configuration as a First-Class UI

v1 requires manually editing a TOML file and restarting. v2 exposes all settings through a slide-out panel with immediate effect. Changes write to TOML on disk and take effect without restart.

**Impact:** Users can change hotkeys, folders, sounds, and toggles without leaving the app.

### Trade-offs considered:
- **Tauri vs Electron:** Tauri chosen for smaller binary, Rust backend performance, and native system tray. Trade-off: smaller ecosystem, less documentation.
- **Full Rust backend vs hybrid:** Full Rust chosen to eliminate the GIL and threading issues that motivated the rewrite. Trade-off: more Rust code upfront, but the domain logic is simple enough that this is manageable.
- **React vs Svelte:** React chosen for ecosystem size and developer familiarity. Trade-off: slightly larger bundle, but negligible for a desktop app.

---

## 3. Tech Stack

### Rust Backend

| Crate | Category | Usage |
| :--- | :--- | :--- |
| **tauri 2.x** | Framework | Desktop app shell, window management, system tray, IPC |
| **tokio** | Runtime | Async runtime for all backend operations |
| **notify** | File Watching | Cross-platform filesystem event monitoring (replaces Python `watchdog`) |
| **winapi / windows-sys** | Hotkeys | Windows Raw Input API for F13-F24 key support |
| **rodio** | Audio | Async sound playback for notification sounds |
| **tokio-tungstenite** | WebSocket | OBS WebSocket 5.x protocol client |
| **toml** | Config | TOML config file read/write (serde integration) |
| **serde / serde_json** | Serialization | Type-safe serialization for config, events, IPC |
| **chrono** | Time | Timestamp parsing and formatting from filenames |
| **sha2 / base64** | Auth | OBS WebSocket authentication (SHA256 challenge) |
| **log / env_logger** | Logging | Structured logging throughout backend |

### React Frontend

| Technology | Category | Usage |
| :--- | :--- | :--- |
| **React 19 + TypeScript** | Framework | UI component framework |
| **Vite** | Bundler | Fast dev server and build tool (Tauri default) |
| **Tailwind CSS v4** | Styling | Utility-first CSS |
| **shadcn/ui** | Components | Pre-built component library (Button, Dialog, Sheet, ScrollArea, Switch) |
| **@tauri-apps/api** | IPC | Frontend bindings for Tauri commands and events |
| **lucide-react** | Icons | Icon library for UI elements |

---

## 4. Architecture

### High-Level Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Tauri Shell (v2)                         │
│                                                                 │
│  ┌─────────────────────────┐    ┌────────────────────────────┐  │
│  │      Rust Backend       │    │     React Frontend         │  │
│  │                         │    │                            │  │
│  │  ┌───────────────────┐  │    │  ┌──────────────────────┐  │  │
│  │  │   AppState         │  │    │  │   Sidebar            │  │  │
│  │  │   (Arc<Mutex<_>>)  │  │    │  │   G1/G2/G3/G4 cards │  │  │
│  │  └────────┬──────────┘  │    │  └──────────────────────┘  │  │
│  │           │              │    │  ┌──────────────────────┐  │  │
│  │  ┌────────▼──────────┐  │    │  │   EventLog           │  │  │
│  │  │ Module Registry   │  │    │  │   Color-coded entries │  │  │
│  │  │                   │  │    │  └──────────────────────┘  │  │
│  │  │ • FileWatcher     │──┼────│  ┌──────────────────────┐  │  │
│  │  │ • HotkeyManager   │  │    │  │   Timer              │  │  │
│  │  │ • FileMover       │  │ E  │  │   Countdown + pulse  │  │  │
│  │  │ • SoundPlayer     │  │ v  │  └──────────────────────┘  │  │
│  │  │ • ObsWebSocket    │  │ e  │  ┌──────────────────────┐  │  │
│  │  │ • Logger          │  │ n  │  │   Settings (Sheet)   │  │  │
│  │  │ • Timer           │  │ t  │  │   All config fields  │  │  │
│  │  │ • ConfigManager   │  │ s  │  └──────────────────────┘  │  │
│  │  └───────────────────┘  │    │  ┌──────────────────────┐  │  │
│  │                         │    │  │   BottomBar           │  │  │
│  │  ┌───────────────────┐  │    │  │   Wipe/Restore/Mode  │  │  │
│  │  │ System Tray       │  │    │  └──────────────────────┘  │  │
│  │  │ (Tauri built-in)  │  │    │                            │  │
│  │  └───────────────────┘  │    │  shadcn/ui + Tailwind      │  │
│  └─────────────────────────┘    └────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Communication Flow

```
                    ┌──────────────┐
                    │  User Action │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ Hotkey   │ │ UI Click │ │ File     │
        │ (F13+)  │ │ (G1 btn) │ │ Created  │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │            │            │
             ▼            ▼            ▼
        ┌─────────────────────────────────────┐
        │         Rust Event Loop             │
        │         (tokio channels)            │
        │                                     │
        │  match event {                      │
        │    Hotkey(G1)  => move_file(g1),    │
        │    Hotkey(G4)  => request_rename(), │
        │    FileNew(p)  => track_file(p),    │
        │    TimerTick   => emit_to_frontend, │
        │  }                                  │
        └──────────────┬──────────────────────┘
                       │
                       ▼
        ┌─────────────────────────────────────┐
        │     Tauri Event Emission            │
        │                                     │
        │  emit("file-created", payload)      │
        │  emit("file-moved", payload)        │
        │  emit("timer-tick", payload)        │
        │  emit("log-entry", payload)         │
        └──────────────┬──────────────────────┘
                       │
                       ▼
        ┌─────────────────────────────────────┐
        │     React Event Listeners           │
        │                                     │
        │  listen("file-created", handler)    │
        │  listen("timer-tick", handler)      │
        │  → setState → re-render            │
        └─────────────────────────────────────┘
```

### State Ownership

| State | Owner | Access Pattern |
|-------|-------|---------------|
| `current_filename` | Rust `AppState` | Mutex-protected, single writer |
| `config` | Rust `ConfigManager` | Read via command, write via command (saves to TOML) |
| `event_log` | Rust `Logger` | Append-only Vec, emitted to frontend |
| `timer_state` | Rust `Timer` | Tokio interval, emits ticks |
| `watcher_status` | Rust `FileWatcher` | Enum: Running/Stopped/Error |
| `obs_ws_status` | Rust `ObsWebSocket` | Enum: Connected/Disconnected/Reconnecting |
| UI display state | React | Derived from Rust events. Ephemeral UI toggles (e.g., auto-wipe switch position) are local React state but backed by config — toggling invokes `update_config` which persists to Rust/TOML |

---

## 5. Rust Backend Modules

### Module Overview

| Module | File | Responsibility | Priority |
| :--- | :--- | :--- | :--- |
| **Config** | `config.rs` | TOML read/write, strongly typed config struct | **[T1]** |
| **File Watcher** | `watcher.rs` | `notify` crate, debounce, sleep/resume detection | **[T1]** |
| **Hotkey Manager** | `hotkeys.rs` | Windows Raw Input API, F13-F24 support | **[T1]** |
| **File Mover** | `mover.rs` | Move/rename files, directory creation, size validation | **[T1]** |
| **Sound Player** | `sound.rs` | `rodio` async playback, custom sound files | **[T1]** |
| **Logger** | `logger.rs` | In-memory history, daily log files, event emission | **[T1]** |
| **Timer** | `timer.rs` | Tokio interval countdown, auto-reset | **[T1]** |
| **System Tray** | `tray.rs` | Tauri tray API, menu items | **[T1]** |
| **OBS WebSocket** | `obs_ws.rs` | `tokio-tungstenite`, auth, replay buffer events, auto-failover to file watcher | **[T1]** |

### Module Details

#### 5.1 Config Manager (`src-tauri/src/config.rs`)

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    // General
    pub screen_capture_software: String,        // "obs" | "shadowplay"
    pub videos_folder: String,                  // Watch directory path
    pub log_file_enabled: bool,

    // Hotkeys
    pub g1_bind: String,                        // Default: "ctrl+F13"
    pub g2_bind: String,                        // Default: "ctrl+F14"
    pub g3_bind: String,                        // Default: "ctrl+F15"
    pub rename_bind: String,                    // Default: "alt+F13"
    pub restart_watcher_bind: String,           // Default: "ctrl+shift+F12"

    // Folder Names (for sort mode)
    pub g1_bind_folder_name: String,            // Default: "!! or ! (G1)"
    pub g2_bind_folder_name: String,            // Default: "odd or checked (G2)"
    pub g3_bind_folder_name: String,            // Default: "!!! (G3)"

    // Sounds
    pub clip_save_sound_enabled: bool,
    pub clip_save_sound_custom: Option<String>,  // None = use bundled default
    pub move_sound_enabled: bool,
    pub error_sound_enabled: bool,
    pub error_sound_custom: Option<String>,

    // Notifications
    pub windows_notification_enabled: bool,

    // Timer
    pub timer_enabled: bool,
    pub timer_duration_ms: u64,                 // Default: 70000 (70s)
    pub auto_wipe_enabled: bool,

    // Mode
    pub disable_file_movesorting: bool,         // true = rename only, false = folder sort

    // OBS WebSocket
    pub obs_websocket_enabled: bool,
    pub obs_websocket_password: String,

    // ShadowPlay — config-only support. User sets shadowplay_folder as an
    // alternate videos_folder path. No ShadowPlay-specific API integration;
    // file watcher works the same regardless of capture software.
    pub shadowplay_folder: Option<String>,
    pub prompt_capture_software: bool,
}
```

**Key behaviors:**
- Loads from `config.toml` next to executable
- Falls back to embedded defaults if file missing
- Legacy `options.txt` auto-migration on first run
- `update_config` command writes changes immediately and emits `config-changed` event
- All fields validated on load (paths checked, durations clamped)

#### 5.2 File Watcher (`src-tauri/src/watcher.rs`)

```rust
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    tx: mpsc::Sender<WatcherEvent>,
    status: WatcherStatus,
    last_check: Instant,
}

pub enum WatcherEvent {
    FileCreated { path: PathBuf, timestamp: DateTime<Local> },
    WatcherError { message: String },
    WatcherRestarted { restart_count: u32 },
}

pub enum WatcherStatus {
    Running,
    Stopped,
    Error(String),
}
```

**Key behaviors:**
- `notify::RecommendedWatcher` with 200ms debounce (fixes v1's "file size too fast" bug)
- Filters: only `.mp4`, `.mov`, `.avi`, `.mkv` extensions
- Sleep/resume detection: compares wall-clock time between 1-second health check ticks. If the delta between two consecutive ticks exceeds 10 seconds, the system likely resumed from sleep — watcher is stopped and restarted. This is the same logic as v1's `ResumableObserver.check_and_restart_if_needed()`
- Manual restart via hotkey or UI button
- Emits `WatcherEvent::FileCreated` through tokio channel to main event loop

#### 5.3 Hotkey Manager (`src-tauri/src/hotkeys.rs`)

```rust
pub struct HotkeyManager {
    bindings: HashMap<KeyCombo, HotkeyAction>,
    tx: mpsc::Sender<HotkeyEvent>,
}

pub enum HotkeyAction {
    MoveG1,
    MoveG2,
    MoveG3,
    Rename,
    RestartWatcher,
}

pub struct KeyCombo {
    pub modifiers: Vec<Modifier>,   // Ctrl, Alt, Shift
    pub key: VirtualKey,            // F13, F14, F15, etc.
}
```

**Key behaviors:**
- Uses Windows Raw Input API (`RegisterRawInputDevices` + `WM_INPUT` messages)
- Supports F13-F24 keys that Tauri's built-in shortcut API cannot handle
- Runs on a dedicated thread with a message pump, sends events via channel
- Hotkeys registered/unregistered dynamically when config changes
- Fallback on registration failure: logs warning in event log with the specific key that failed, shows yellow badge next to the failed key in Sidebar. Sidebar click-buttons always work regardless of hotkey registration status. User can rebind to different keys in Settings to recover.

#### 5.4 File Mover (`src-tauri/src/mover.rs`)

```rust
pub struct FileMover;

pub enum MoveMode {
    FolderSort {
        base_path: PathBuf,           // videos_folder/sort/AHK sort/
        bind_folder: String,          // e.g., "!! or ! (G1)"
    },
    RenameOnly {
        tag: String,                  // e.g., "!!", "CHKD", "!!!"
    },
}

pub struct MoveResult {
    pub original_path: PathBuf,
    pub new_path: PathBuf,
    pub tag_applied: String,
}
```

**Key behaviors:**
- **Folder sort mode:** Moves file to `{videos_folder}/sort/AHK sort/{bind_folder_name}/{tagged_filename}`
- **Rename only mode:** Inserts tag after timestamp in filename (e.g., `Replay 2026-04-15 12-30-00 !! .mp4`). Uses regex `(\d{4}-\d{2}-\d{2} \d{2}-\d{2}-\d{2})` to find the OBS/ShadowPlay timestamp pattern and inserts the tag after it. If no timestamp found, appends tag before the file extension. Handles both OBS format (`Replay YYYY-MM-DD HH-MM-SS.mp4`) and ShadowPlay format (`Game Name YYYY.MM.DD - HH.MM.SS.mp4`)
- Tag shorthand map: `"!! or ! (G1)" → "!!"`, `"odd or checked (G2)" → "CHKD"`, `"!!! (G3)" → "!!!"`
- Creates sort directories on startup if they don't exist (`std::fs::create_dir_all`)
- File size validation: warns if < 6.5MB (probable black screen recording)
- Retry logic: 3 attempts with 200ms/500ms/1s backoff if file is locked by OBS

#### 5.5 Sound Player (`src-tauri/src/sound.rs`)

```rust
pub struct SoundPlayer {
    sink: rodio::Sink,
    device: rodio::Device,
}

pub enum SoundEvent {
    ClipSaved,          // CrimewaveTone.wav (default)
    FileMoved(u8),      // 523Hz beep × gkey number
    Error,              // XP Error sound
    Warning,            // XP Error sound (black screen)
}
```

**Key behaviors:**
- `rodio` for non-blocking audio playback
- Default sounds bundled in `resources/sounds/`
- Custom sound paths from config (validated on load)
- `SND_ASYNC` equivalent — fire and forget, doesn't block event loop
- Sound events dispatched through the same channel as other events

#### 5.6 Logger (`src-tauri/src/logger.rs`)

```rust
pub struct AppLogger {
    history: Vec<LogEntry>,
    log_dir: PathBuf,
}

pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
    pub category: LogCategory,
}

pub enum LogLevel { Info, Warning, Error, Success }

pub enum LogCategory {
    FileCreated,
    FileMoved,
    FileRenamed,
    HotkeyPressed,
    WatcherStatus,
    ObsWebSocket,
    System,
}
```

**Key behaviors:**
- In-memory `Vec<LogEntry>` for history (replaces `TerminalHistory`)
- Daily log files: `ObsMoveLog YYYY-MM-DD.txt` in `{videos_folder}/logs/`
- Log format: `{bind_chosen} | {filename}` for moves, `Renamed: {old} → {new}` for renames
- Wipe: clears in-memory display buffer, keeps history for restore
- Restore: re-emits full history to frontend
- Each new entry emits `log-entry` Tauri event

#### 5.7 Timer (`src-tauri/src/timer.rs`)

```rust
pub struct CountdownTimer {
    duration: Duration,
    remaining: Duration,
    running: bool,
    tx: mpsc::Sender<TimerEvent>,
}

pub enum TimerEvent {
    Tick { remaining_secs: u32, total_secs: u32 },
    Expired,
    Reset,
}
```

**Key behaviors:**
- `tokio::time::interval(Duration::from_secs(1))` for 1-second ticks
- Starts when new file detected, duration from config
- Emits `TimerEvent::Tick` every second with remaining time
- On expiry: emits `TimerEvent::Expired`, triggers auto-wipe + state reset
- New file resets the timer (cancels old, starts new)
- No drift — tokio intervals are clock-based, not sleep-based

#### 5.8 OBS WebSocket (`src-tauri/src/obs_ws.rs`)

```rust
pub struct ObsWebSocketClient {
    status: ObsWsStatus,
    retry_count: u32,
    max_retries: u32,           // Default: 5
    tx: mpsc::Sender<WatcherEvent>,
}

pub enum ObsWsStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
    FailedOver,                 // Switched to file watcher
}
```

**Key behaviors:**
- `tokio-tungstenite` for async WebSocket client
- OBS WebSocket 5.x protocol: `op:0` challenge → `op:1` auth → `op:2` success
- Auth: SHA256(`password + salt`) then SHA256(`secret + challenge`), base64 encoded
- Listens for `op:5` / `eventType: "ReplayBufferSaved"` → extracts `savedReplayPath`
- Converts forward slashes to backslashes in paths from OBS
- Auto-reconnect with 5s delay between attempts, max 5 retries
- On max retries: switches to file watcher mode, emits status event
- Enabled/disabled via config toggle (default: disabled)

#### 5.9 System Tray (`src-tauri/src/tray.rs`)

Uses Tauri's built-in system tray API (replaces PySide6/Qt entirely):

**Menu items:**
- Video Folder → opens `videos_folder` in Explorer
- Log Folder → opens `{videos_folder}/logs` in Explorer
- Help → opens help URL in browser
- Exit → clean shutdown (stop watcher, close WS, save config, exit)

**Tooltip:** `Gkey Mover v2.0.0`
**Icon:** `obsicon.ico` from resources

---

## 6. React Frontend

### Component Tree

```
<App>
  <ConfigProvider>
    <div className="flex h-screen bg-background">
      <Sidebar />
      <main className="flex-1 flex flex-col">
        <EventLog />
        <BottomBar />
      </main>
      <TimerDisplay />
    </div>
    <SettingsSheet />
    <RenameDialog />
  </ConfigProvider>
</App>
```

### Component Details

#### 6.1 Sidebar (`src/components/Sidebar.tsx`)

```
┌──────────┐
│  G1  !!  │  ← Accent color: blue
├──────────┤
│  G2 CHKD │  ← Accent color: green
├──────────┤
│  G3  !!! │  ← Accent color: orange
├──────────┤
│  G4  REN │  ← Accent color: purple
├──────────┤
│    ⚙     │  ← Opens settings sheet
└──────────┘
```

- Each G-key is a `Card` component with large text and distinct accent color
- Click invokes the same Tauri command as the physical hotkey
- Brief highlight animation on press (100ms scale pulse)
- Shows bound folder name / action label under the key identifier
- Settings gear button at bottom opens `SettingsSheet`

#### 6.2 Event Log (`src/components/EventLog.tsx`)

- shadcn `ScrollArea` with virtual scrolling for performance
- Each entry: `[timestamp] [icon] [message]`
- Color coding via Tailwind classes:
  - `text-green-400` — New file detected
  - `text-purple-400` — File moved / renamed
  - `text-red-400` — Warning (black screen) / "No current file"
  - `text-yellow-400` — Watcher restart / OBS WebSocket events
  - `text-muted-foreground` — System messages
- Auto-scrolls to bottom on new entries
- Listens to `log-entry` Tauri events

#### 6.3 Timer Display (`src/components/TimerDisplay.tsx`)

- Large monospace countdown: `MM:SS` format
- Position: top-right corner of the main area
- States:
  - **Idle:** Shows initial time in muted color
  - **Running:** Counts down, neutral color
  - **Warning (< 10s):** Amber text
  - **Critical (< 5s):** Red text with CSS `animate-pulse` glow (replaces v1's theme blinking)
  - **Expired:** Resets to initial time
- Listens to `timer-tick` and `timer-expired` Tauri events

#### 6.4 Bottom Bar (`src/components/BottomBar.tsx`)

```
┌──────────────────────────────────────────────────────┐
│  Mode: Rename Only  │ [Wipe] [Restore] │ Auto-Wipe ◉ │
└──────────────────────────────────────────────────────┘
```

- **Mode indicator:** "Rename Only" or "Folder Sort" based on config
- **Wipe button:** Clears displayed log (shadcn `Button variant="outline"`)
- **Restore button:** Brings back full history (shadcn `Button variant="outline"`)
- **Auto-Wipe toggle:** shadcn `Switch` component

#### 6.5 Settings Sheet (`src/components/SettingsSheet.tsx`)

shadcn `Sheet` that slides from the right. Sections:

| Section | Fields | Components |
|---------|--------|------------|
| **General** | Videos folder, capture software | Input + folder picker dialog, Select |
| **Hotkeys** | G1/G2/G3/Rename/Restart binds | Hotkey capture input fields |
| **Folders** | G1/G2/G3 folder names, sort mode | Input fields, Switch toggle |
| **Sounds** | Clip save, move, error toggles + custom paths | Switch + file picker per sound |
| **Notifications** | Windows toast toggle | Switch |
| **OBS WebSocket** | Enable toggle, password, status | Switch, password Input, status Badge |
| **Timer** | Duration (seconds input), auto-wipe | Number Input, Switch |

- Changes invoke `update_config` Tauri command immediately
- Config changes take effect without restart
- Connection status for OBS WebSocket shown as a live badge (green/red/yellow)

#### 6.6 Rename Dialog (`src/components/RenameDialog.tsx`)

- shadcn `Dialog` component
- Shows current filename in muted text
- Text input with `" - "` prefix auto-prepended
- Enter to confirm → invokes `rename_file` Tauri command
- Escape to cancel
- Window focused on open via Tauri `set_focus()`

### State Management

```typescript
// src/contexts/ConfigContext.tsx
interface ConfigState {
  config: AppConfig;
  updateConfig: (partial: Partial<AppConfig>) => Promise<void>;
}

// src/hooks/useEvents.ts
function useEventLog(): LogEntry[] {
  // Listens to "log-entry" events, maintains display buffer
}

function useTimer(): TimerState {
  // Listens to "timer-tick" and "timer-expired" events
}

function useWatcherStatus(): WatcherStatus {
  // Listens to "watcher-status" events
}
```

- React Context for config (loaded once on mount via `get_config` command)
- Custom hooks for each event stream
- No Redux, no Zustand — Rust is the source of truth
- `@tauri-apps/api/event::listen()` for all real-time data

### Theme

- Dark mode by default (shadcn dark theme CSS variables)
- G-key accent colors: Blue (G1), Green (G2), Orange (G3), Purple (G4)
- Font: system default (Segoe UI on Windows)
- Default window size: 900x300px, resizable
- Always-on-top toggle via Tauri window API (default: on)
- Multi-monitor: positions on secondary monitor if available
- App icon: `obsicon.ico` from original project

---

## 7. IPC Contract

### Tauri Commands (Frontend → Rust)

| Command | Params | Returns | Description |
|---------|--------|---------|-------------|
| `get_config` | — | `AppConfig` | Load current configuration |
| `update_config` | `Partial<AppConfig>` | `AppConfig` | Update and save config, returns new state |
| `reset_config` | — | `AppConfig` | Reset to defaults |
| `press_gkey` | `{ key: 1\|2\|3 }` | `MoveResult \| Error` | Trigger G-key action (same as hotkey) |
| `rename_file` | `{ text: string }` | `RenameResult \| Error` | Append text to current filename |
| `wipe_log` | — | — | Clear displayed log entries |
| `restore_log` | — | `LogEntry[]` | Get full history |
| `restart_watcher` | — | — | Restart file observer |
| `get_watcher_status` | — | `WatcherStatus` | Current watcher state |
| `get_obs_status` | — | `ObsWsStatus` | Current OBS WebSocket state |
| `open_folder` | `{ path: string }` | — | Open folder in Explorer |
| `pick_folder` | — | `string \| null` | Native folder picker dialog |
| `pick_file` | `{ filters: string[] }` | `string \| null` | Native file picker dialog |

### Tauri Events (Rust → Frontend)

| Event | Payload | Description |
|-------|---------|-------------|
| `file-created` | `{ path, filename, timestamp, size_mb, is_warning }` | New video file detected |
| `file-moved` | `{ original, destination, tag, mode }` | File moved/renamed successfully |
| `file-renamed` | `{ original, new_name }` | File renamed via G4 dialog |
| `hotkey-pressed` | `{ key: "G1"\|"G2"\|"G3"\|"G4" }` | Physical hotkey triggered |
| `timer-tick` | `{ remaining_secs, total_secs }` | Countdown update (1/sec) |
| `timer-expired` | — | Timer reached zero |
| `log-entry` | `{ timestamp, level, message, category }` | New log entry to display |
| `watcher-status` | `{ status, restart_count? }` | Watcher state changed |
| `obs-ws-status` | `{ status, attempt? }` | OBS WebSocket state changed |
| `config-changed` | `AppConfig` | Config was updated |
| `error` | `{ message, context }` | Backend error notification |

---

## 8. Configuration

### Config File Location

Config file: `config.toml` in the same directory as the executable (portable) or `%APPDATA%/gkey-mover-v2/config.toml` (installed).

### Default Configuration

```toml
# Gkey Mover v2 Configuration

# General
screen_capture_software = "obs"
videos_folder = ""
log_file_enabled = true

# Hotkeys (use Windows virtual key names)
g1_bind = "ctrl+F13"
g2_bind = "ctrl+F14"
g3_bind = "ctrl+F15"
rename_bind = "alt+F13"
restart_watcher_bind = "ctrl+shift+F12"

# Folder Names (used in folder sort mode)
g1_bind_folder_name = "!! or ! (G1)"
g2_bind_folder_name = "odd or checked (G2)"
g3_bind_folder_name = "!!! (G3)"

# Sounds
clip_save_sound_enabled = false
clip_save_sound_custom = ""
move_sound_enabled = false
error_sound_enabled = true
error_sound_custom = ""

# Notifications
windows_notification_enabled = false

# Timer
timer_enabled = true
timer_duration_ms = 70000
auto_wipe_enabled = true

# Mode
disable_file_movesorting = true

# OBS WebSocket
obs_websocket_enabled = false
obs_websocket_password = ""

# ShadowPlay
shadowplay_folder = ""
prompt_capture_software = false
```

### Legacy Migration

On first launch, if `config.toml` is missing but `options.txt` exists:
1. Parse `options.txt` line-by-line using the v1 key order
2. Convert boolean strings to actual booleans
3. Write as `config.toml`
4. Log migration success

---

## 9. Error Handling & Resilience

### File Operations

| Scenario | Handling |
|----------|---------|
| File locked by OBS during write | Retry 3x with exponential backoff (200ms → 500ms → 1s) |
| File already moved by another process | Log warning, reset `current_filename`, emit error event |
| Sort directories deleted while running | Recreate with `create_dir_all` on next move attempt |
| Videos folder doesn't exist on startup | Show error in UI, prompt folder picker |
| File < 6.5MB | Warning sound + red log entry ("probable black screen") |

### Hotkeys

| Scenario | Handling |
|----------|---------|
| F13-F15 registration fails | Log warning, show in event log, sidebar buttons still work |
| Hotkey pressed with no current file | Error sound + red "No current file" log entry |
| Config changes hotkey binds | Unregister old, register new, log any failures |

### OBS WebSocket

| Scenario | Handling |
|----------|---------|
| Connection refused | Retry up to 5 times with 5s delay |
| Auth failure | Show error in settings panel with status badge |
| Connection drops mid-session | Auto-reconnect in background |
| Max retries exceeded | Switch to file watcher mode, emit status event |

### Timer / State

| Scenario | Handling |
|----------|---------|
| Multiple files created rapidly | Each new file resets timer and updates current file atomically |
| App resumed from sleep | `notify` polling fallback + system time delta check |
| Window closed | Minimize to tray (not exit) |
| Tray "Exit" | Stop watcher → close WebSocket → save config → exit |

### Multi-Monitor

| Scenario | Handling |
|----------|---------|
| Two monitors | Position window on secondary monitor |
| One monitor | Position at default (0, 0) |
| Monitor disconnected while running | Window repositions to primary automatically (OS handles) |

---

## 10. Project Structure

```
gkey-mover-v2/
├── Docs/
│   ├── Examples/                      # Reference docs from other project
│   │   ├── AI-Development-Guide.md
│   │   ├── Architecture.md
│   │   ├── Feature-Template.md
│   │   ├── OptionsMania-Main-Doc-Index.md
│   │   └── Testing.md
│   ├── specs/
│   │   └── 2026-04-15-gkey-mover-v2-design.md    # This file
│   ├── AI-Development-Guide.md        # Rules for AI assistants on this project
│   └── Architecture.md                # Condensed architecture reference
│
├── src-tauri/                         # Rust backend
│   ├── Cargo.toml                     # Rust dependencies
│   ├── tauri.conf.json                # Tauri config (window, tray, permissions)
│   ├── build.rs                       # Build script
│   ├── icons/                         # App icons (obsicon.ico + generated sizes)
│   └── src/
│       ├── main.rs                    # Entry point, Tauri builder setup
│       ├── lib.rs                     # Module declarations
│       ├── state.rs                   # AppState (Arc<Mutex<_>>) shared state
│       ├── config.rs                  # Config struct, TOML read/write, migration
│       ├── watcher.rs                 # File watcher (notify crate)
│       ├── hotkeys.rs                 # Windows Raw Input API hotkey manager
│       ├── mover.rs                   # File move/rename logic
│       ├── sound.rs                   # Audio playback (rodio)
│       ├── logger.rs                  # In-memory + file logging
│       ├── timer.rs                   # Countdown timer (tokio interval)
│       ├── tray.rs                    # System tray setup
│       ├── obs_ws.rs                  # OBS WebSocket client
│       ├── events.rs                  # Event type definitions
│       └── commands.rs                # Tauri command handlers (IPC)
│
├── src/                               # React frontend
│   ├── main.tsx                       # Entry point
│   ├── App.tsx                        # Root component
│   ├── index.css                      # Tailwind imports + shadcn CSS vars
│   ├── components/
│   │   ├── ui/                        # shadcn/ui components (auto-generated)
│   │   │   ├── button.tsx
│   │   │   ├── card.tsx
│   │   │   ├── dialog.tsx
│   │   │   ├── input.tsx
│   │   │   ├── scroll-area.tsx
│   │   │   ├── sheet.tsx
│   │   │   ├── switch.tsx
│   │   │   ├── badge.tsx
│   │   │   ├── select.tsx
│   │   │   ├── separator.tsx
│   │   │   └── label.tsx
│   │   ├── Sidebar.tsx                # G-key cards + settings button
│   │   ├── EventLog.tsx               # Scrollable color-coded log
│   │   ├── TimerDisplay.tsx           # Countdown with pulse animation
│   │   ├── BottomBar.tsx              # Wipe/Restore/Mode/Auto-wipe
│   │   ├── SettingsSheet.tsx          # Config panel (slide-out)
│   │   └── RenameDialog.tsx           # File rename modal
│   ├── contexts/
│   │   └── ConfigContext.tsx          # Config state provider
│   ├── hooks/
│   │   ├── useEventLog.ts            # Log entry event listener
│   │   ├── useTimer.ts               # Timer tick event listener
│   │   ├── useWatcherStatus.ts       # Watcher status listener
│   │   └── useObsStatus.ts           # OBS WebSocket status listener
│   ├── lib/
│   │   ├── commands.ts               # Typed Tauri command wrappers
│   │   ├── events.ts                 # Event type definitions (mirrors Rust)
│   │   └── utils.ts                  # Formatting helpers
│   └── types/
│       └── index.ts                   # Shared TypeScript types
│
├── resources/                         # Bundled assets
│   ├── sounds/
│   │   ├── CrimewaveTone.wav
│   │   ├── Microsoft Windows XP Error.mp3
│   │   ├── audiocheck.net_sin_523Hz_-21dBFS_.15s.wav
│   │   └── audiocheck.net_sin_150Hz_-21dBFS_.75s.wav
│   └── icons/
│       └── obsicon.ico
│
├── index.html                         # Vite entry HTML
├── package.json                       # Node dependencies
├── tsconfig.json                      # TypeScript config
├── tailwind.config.ts                 # Tailwind config (shadcn preset)
├── vite.config.ts                     # Vite config (Tauri plugin)
├── components.json                    # shadcn/ui config
├── CLAUDE.md                          # AI assistant instructions
├── .gitignore
└── README.md
```

---

## 11. Migration from v1

### Feature Parity Checklist

| v1 Feature | v2 Module | Status |
|:---|:---|:---|
| File watching (watchdog) | `watcher.rs` (notify) | Planned |
| Sleep/resume detection | `watcher.rs` (built-in) | Planned |
| Manual watcher restart (Ctrl+Shift+F12) | `hotkeys.rs` + `watcher.rs` | Planned |
| G1/G2/G3 hotkeys (Ctrl+F13/14/15) | `hotkeys.rs` (Raw Input) | Planned |
| G4 rename (Alt+F13) | `hotkeys.rs` + `RenameDialog.tsx` | Planned |
| Folder sort mode | `mover.rs` (FolderSort) | Planned |
| Rename only mode | `mover.rs` (RenameOnly) | Planned |
| Tag insertion after timestamp | `mover.rs` (regex) | Planned |
| File size validation (6.5MB) | `mover.rs` | Planned |
| Black screen warning sound | `sound.rs` | Planned |
| Clip save notification sound | `sound.rs` | Planned |
| Move notification sound | `sound.rs` | Planned |
| Error sound (no current file) | `sound.rs` | Planned |
| Custom sound paths | `config.rs` + `sound.rs` | Planned |
| Windows toast notifications | Tauri notification plugin | Planned |
| TOML config | `config.rs` | Planned |
| Legacy options.txt migration | `config.rs` | Planned |
| System tray icon | `tray.rs` (Tauri API) | Planned |
| Tray menu (Video/Log/Help/Exit) | `tray.rs` | Planned |
| Terminal output GUI | `EventLog.tsx` | Planned |
| Color-coded log entries | `EventLog.tsx` (Tailwind) | Planned |
| Wipe / Restore buttons | `BottomBar.tsx` | Planned |
| Auto-wipe timer | `timer.rs` + `BottomBar.tsx` | Planned |
| Countdown display | `TimerDisplay.tsx` | Planned |
| Timer blinking (< 5s) | `TimerDisplay.tsx` (CSS pulse) | Planned |
| Daily log files | `logger.rs` | Planned |
| OBS WebSocket file detection | `obs_ws.rs` | Planned |
| OBS WebSocket auth | `obs_ws.rs` (SHA256) | Planned |
| OBS WebSocket auto-reconnect | `obs_ws.rs` (5 retries) | Planned |
| OBS → file watcher fallback | `obs_ws.rs` → `watcher.rs` | Planned |
| Always-on-top window | Tauri window API | Planned |
| Multi-monitor positioning | Tauri window API | Planned |
| G-key button panel (debug UI) | `Sidebar.tsx` | Planned |
| Mode indicator (Rename/Sort) | `BottomBar.tsx` | Planned |
| ShadowPlay folder option | `config.rs` | Planned |
| Prompt OBS/ShadowPlay on start | `config.rs` flag | Planned |

### v2 Improvements (Not in v1)

| Improvement | Description |
|:---|:---|
| Settings UI | Built-in settings panel (no more editing TOML manually) |
| Live config changes | Config changes take effect without restart |
| Dark mode UI | Modern dark theme with shadcn/ui components |
| No threading bugs | Single-threaded event loop (tokio), no GIL |
| No polling | Event-driven architecture, zero CPU when idle |
| Smaller binary | ~5MB vs 200MB+ (Python + PySide6 + venv) |
| Proper error handling | Typed Results, no silent failures |
| Type safety | Rust + TypeScript, no `None` globals |

---

## 12. Testing

### Rust Backend Tests

```bash
# Run all tests
cd src-tauri && cargo test

# Run specific module tests
cargo test config::tests
cargo test mover::tests
cargo test watcher::tests

# Run with output
cargo test -- --nocapture
```

**Test structure mirrors source:**
```
src-tauri/src/
  config.rs         → #[cfg(test)] mod tests { ... }
  mover.rs          → #[cfg(test)] mod tests { ... }
  logger.rs         → #[cfg(test)] mod tests { ... }
```

**Key test areas:**
- Config: TOML parsing, defaults, legacy migration, validation
- Mover: folder sort paths, rename-only paths, tag insertion regex, edge cases
- Logger: entry formatting, daily log file naming, history wipe/restore
- Timer: countdown math, reset behavior

### Frontend Tests

**Framework:** Vitest + React Testing Library (consistent with Vite-based Tauri frontend).

```bash
# Run all tests
pnpm test

# Watch mode
pnpm test:watch

# Coverage
pnpm test:coverage
```

**Test areas:**
- Component rendering (EventLog entries, Timer display, Sidebar buttons)
- Event hook behavior (mock `@tauri-apps/api/event::listen` calls)
- Config context updates (mock `invoke` for `get_config` / `update_config`)
- Formatting helpers (timestamp display, file size formatting)

### Manual Testing Checklist

- [ ] New video file detected and logged
- [ ] G1/G2/G3 hotkeys move/rename file correctly
- [ ] G4 rename dialog appears, renames file
- [ ] Timer starts on new file, counts down, auto-wipes
- [ ] Timer blinks/pulses under 5 seconds
- [ ] Wipe clears log, Restore brings it back
- [ ] Settings panel opens, changes save and take effect
- [ ] Tray icon appears with correct menu items
- [ ] Close window minimizes to tray
- [ ] Tray Exit cleanly shuts down
- [ ] Sleep/resume restarts watcher
- [ ] OBS WebSocket connects and detects replays (when enabled)
- [ ] Error sound plays when pressing G-key with no current file
- [ ] Black screen warning for small files
- [ ] Multi-monitor window positioning
- [ ] Legacy options.txt migration works

---

## 13. Performance

### Expected Metrics

| Metric | v1 (Python) | v2 (Tauri) | Notes |
|--------|-------------|------------|-------|
| **Binary size** | ~200MB (with venv) | ~5MB | Tauri + Rust |
| **RAM usage (idle)** | ~80MB | ~15MB | No Python runtime |
| **CPU usage (idle)** | 1-3% (polling) | <0.1% | Event-driven |
| **Event latency** | 100-200ms (queue poll) | <1ms | Direct channel |
| **Startup time** | 3-5s | <1s | No interpreter |
| **File move latency** | 300ms (sleep + GIL) | <10ms | Native fs ops |

### Why v1 is Laggy

1. **100ms command queue poll** — Tk `after(100, check_queue)` introduces up to 100ms latency on every cross-thread operation
2. **0.3s sleep after every G-key** — `time.sleep(0.3)` in `action_a/b/c/d` blocks the thread
3. **GIL contention** — watchdog thread, keyboard thread, Qt tray thread, and Tk mainloop all compete
4. **Stdout redirect overhead** — every `print()` goes through `StdoutRedirector.write()` which does string matching, tag insertion, and history diffing in the Tk thread
5. **1s watcher poll** — `while True: time.sleep(1)` loop for sleep detection

---

## 14. Troubleshooting

### Hotkeys Not Registering

**Symptoms:** G-key presses don't trigger actions, no log entries

**Possible causes:**
1. F13-F24 keys not mapped on keyboard → check Logitech G Hub / keyboard software
2. Another app captured the hotkey → check for conflicts
3. App not running as expected privilege level

**Solution:**
1. Check event log for "Hotkey registration failed" warnings
2. Try remapping to standard keys (Ctrl+1/2/3) in Settings
3. Use sidebar buttons as fallback

### File Watcher Not Detecting Files

**Symptoms:** New clips don't appear in log

**Possible causes:**
1. Videos folder path incorrect in config
2. Watcher stopped after sleep/resume (should auto-restart)
3. File extension not in filter list

**Solution:**
1. Check Settings → General → Videos folder path
2. Press Ctrl+Shift+F12 to manually restart watcher
3. Check event log for watcher status messages

### OBS WebSocket Not Connecting

**Symptoms:** Status shows "Disconnected" in settings

**Possible causes:**
1. OBS not running or WebSocket server not enabled
2. Wrong password in config
3. Port conflict (default: 4455)

**Solution:**
1. OBS → Tools → WebSocket Server Settings → Enable
2. Check password matches in Settings → OBS WebSocket
3. Check OBS WebSocket port is 4455

---

## 15. References

### Internal Documentation

- [AI Development Guide](../AI-Development-Guide.md) — Rules for AI assistants on this project
- [Architecture](../Architecture.md) — Condensed architecture reference

### External Resources

- [Tauri v2 Documentation](https://v2.tauri.app/)
- [notify crate](https://docs.rs/notify/) — Cross-platform file watching
- [rodio crate](https://docs.rs/rodio/) — Audio playback
- [tokio-tungstenite](https://docs.rs/tokio-tungstenite/) — Async WebSocket
- [shadcn/ui](https://ui.shadcn.com/) — React component library
- [OBS WebSocket Protocol 5.x](https://github.com/obsproject/obs-websocket/blob/master/docs/generated/protocol.md)

### Original v1 Source

- `C:\Users\cbuzi\Documents\~Documents-NzxtPc\Code\VSCode\zMisc\Gkey Mover\Gkey Mover.py` — 2084-line Python source

---

## Changelog

### v2.0.0 (2026-04-15)
- Complete rewrite from Python/Tkinter to Tauri/Rust/React
- Modern dark UI with shadcn/ui components
- Built-in settings panel (no manual TOML editing)
- Event-driven architecture (no polling, no threading bugs)
- Native F13-F24 hotkey support via Windows Raw Input API
- OBS WebSocket as secondary file detection method

---

**Maintainer:** Chris
**Status:** 🏗️ Design phase — awaiting approval for implementation
