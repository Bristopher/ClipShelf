# Phase 1: Game Detection + History Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every clip gets a detected game label, every clip event lands in an append-only `history.jsonl`, and game/rating/description are mirrored into Windows file properties — with hard lock-safety around files OBS may still hold.

**Architecture:** Three new backend modules with pure-testable cores: `history.rs` (JSONL store), `gamedetect.rs` (Win32 snapshot + pure classification), `props.rs` (shell property store writes gated on an exclusive-share probe). Wire-up: snapshot at save-clip-bind press, attach in `handle_file_created`, history events from the existing move/rename/undo paths.

**Tech Stack:** Rust (Tauri v2 backend), `windows-sys` (already a dep — foreground/monitor/version-info APIs), `windows` crate (new dep — COM `IPropertyStore`), serde_json JSONL, chrono.

**Spec:** `Docs/specs/2026-07-12-game-detection-history-overlay-design.md` (Phase 1 sections). External contract that MUST match: `Docs/Features/Clip-Metadata-Interop.md`.

## Global Constraints

- Never do disk IO while holding the state lock — clone what you need, `drop(s)`, then IO (existing codebase discipline).
- File ops with retries run on the blocking pool (`tauri::async_runtime::spawn_blocking`).
- TOML map keys must be strings — `game_overrides` is a `Vec<GameOverride>` struct list, NOT a `HashMap` (see `stats.rs` comment for the lesson).
- All new config fields use `#[serde(default = ...)]` fns and get added to `impl Default` (pattern in `config.rs`).
- Property writes are best-effort: failures log a warning, never an error toast, never block the clip flow.
- Contract values from the interop doc are fixed: Tags=`System.Keywords` (game), `System.Rating` 1★=1/2★=25/3★=50/4★=75/5★=99, Comment=`System.Comment` (description); history.jsonl lives next to `gkey_config.toml`; rating in JSONL is 1–5.
- Run tests with: `Set-Location 'C:\Users\cbuzi\Documents\~Documents-NzxtPc\Code\VSCode\zMisc\Gkey Mover v2\src-tauri'; cargo test` — must end 0 warnings (the codebase is warning-clean; keep it that way).
- Frontend checks: `pnpm exec tsc --noEmit` and `pnpm build` from the repo root. Always pnpm, never npm.

---

### Task 1: `history.rs` — append-only JSONL event store

**Files:**
- Create: `src-tauri/src/history.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod history;` next to `mod stats;`)

**Interfaces:**
- Produces (later tasks rely on these exact signatures):
  - `pub struct HistoryEvent` (fields below), `HistoryEvent::new(event: &str, path: &Path, source: &str) -> Self` + builder-style setters `with_game`, `with_old_path`, `with_key`, `with_rating`, `with_label`, `with_description`
  - `pub fn history_path(config_path: &Path) -> PathBuf` — sibling `history.jsonl`
  - `pub fn append(path: &Path, event: &HistoryEvent)` — best-effort, logs to stderr on failure, never panics
  - `pub fn read_all(path: &Path) -> Vec<HistoryEvent>` — skips corrupt lines, empty vec if missing

- [ ] **Step 1: Write the failing tests**

In `src-tauri/src/history.rs` (module skeleton + tests only; `pub struct HistoryEvent;` etc. not yet written — put tests first, they won't compile, that's the failing state):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_history_path_is_sibling_of_config() {
        let p = history_path(Path::new("C:/app/gkey_config.toml"));
        assert_eq!(p, PathBuf::from("C:/app/history.jsonl"));
    }

    #[test]
    fn test_append_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");

        let e1 = HistoryEvent::new("created", Path::new("C:/clips/a.mp4"), "hotkey")
            .with_game("Counter-Strike 2");
        let e2 = HistoryEvent::new("moved", Path::new("C:/clips/sorted/a.mp4"), "hotkey")
            .with_old_path(Path::new("C:/clips/a.mp4"))
            .with_key(1)
            .with_game("Counter-Strike 2");
        append(&path, &e1);
        append(&path, &e2);

        let all = read_all(&path);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].event, "created");
        assert_eq!(all[0].game.as_deref(), Some("Counter-Strike 2"));
        assert_eq!(all[1].old_path.as_deref(), Some("C:/clips/a.mp4"));
        assert_eq!(all[1].key, Some(1));
        // ts is RFC3339 with offset (parseable by chrono)
        assert!(chrono::DateTime::parse_from_rfc3339(&all[0].ts).is_ok());
    }

    #[test]
    fn test_read_skips_corrupt_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        append(&path, &HistoryEvent::new("created", Path::new("C:/a.mp4"), "app"));
        // Corrupt the file with a garbage line + a blank line
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(f, "{{not json").unwrap();
            writeln!(f).unwrap();
        }
        append(&path, &HistoryEvent::new("rated", Path::new("C:/a.mp4"), "app").with_rating(4));

        let all = read_all(&path);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].rating, Some(4));
    }

    #[test]
    fn test_read_missing_file_is_empty() {
        assert!(read_all(Path::new("C:/does/not/exist/history.jsonl")).is_empty());
    }

    #[test]
    fn test_optional_fields_omitted_from_json() {
        let e = HistoryEvent::new("created", Path::new("C:/a.mp4"), "app");
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("old_path"));
        assert!(!json.contains("rating"));
        assert!(!json.contains("label"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `Set-Location '...\src-tauri'; cargo test history` (with `mod history;` added to lib.rs)
Expected: compile FAIL — `HistoryEvent` not found.

- [ ] **Step 3: Write the implementation**

Top of `src-tauri/src/history.rs`:

```rust
//! Append-only clip event history (`history.jsonl` next to the config).
//! Source of truth for the History panel and for external consumers —
//! the schema is a public contract documented in
//! Docs/Features/Clip-Metadata-Interop.md; change both together.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    /// RFC 3339 local time with offset.
    pub ts: String,
    /// created | moved | renamed | rated | labeled | described | game_edited | undone
    pub event: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<u8>,
    /// 1-5 stars (human scale; the Windows property uses 1-99).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// hotkey | overlay | drop | app
    pub source: String,
}

impl HistoryEvent {
    pub fn new(event: &str, path: &Path, source: &str) -> Self {
        Self {
            ts: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            event: event.to_string(),
            path: path.to_string_lossy().to_string(),
            old_path: None,
            game: None,
            key: None,
            rating: None,
            label: None,
            description: None,
            source: source.to_string(),
        }
    }
    pub fn with_game(mut self, game: &str) -> Self { self.game = Some(game.to_string()); self }
    pub fn with_old_path(mut self, p: &Path) -> Self { self.old_path = Some(p.to_string_lossy().to_string()); self }
    pub fn with_key(mut self, key: u8) -> Self { self.key = Some(key); self }
    pub fn with_rating(mut self, stars: u8) -> Self { self.rating = Some(stars); self }
    pub fn with_label(mut self, label: &str) -> Self { self.label = Some(label.to_string()); self }
    pub fn with_description(mut self, d: &str) -> Self { self.description = Some(d.to_string()); self }
}

/// `history.jsonl` lives next to the config file (same folder as
/// gkey_config.toml / gkey_stats.toml — documented location for other apps).
pub fn history_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name("history.jsonl")
}

/// Append one event. Best-effort: an unwritable history file must never
/// break the clip flow, so failures only log to stderr.
pub fn append(path: &Path, event: &HistoryEvent) {
    let line = match serde_json::to_string(event) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("history: serialize failed: {}", e);
            return;
        }
    };
    let res = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| writeln!(f, "{}", line));
    if let Err(e) = res {
        eprintln!("history: append failed: {}", e);
    }
}

/// Read every event, oldest first. Corrupt/blank lines are skipped —
/// never fatal (the file may predate schema changes).
pub fn read_all(path: &Path) -> Vec<HistoryEvent> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}
```

Note: no `with_extension` for the path — `with_file_name` avoids the `.toml` stripping trap.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test history`
Expected: 5 passed, 0 warnings.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/history.rs src-tauri/src/lib.rs
git commit -m "Add append-only history.jsonl event store with corrupt-line-tolerant reader"
```

---

### Task 2: Config additions — detection toggles, overrides, rollover hour

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src/types/index.ts` (frontend `AppConfig` interface — add the same fields)

**Interfaces:**
- Produces:
  - `pub struct GameOverride { pub exe: String, pub name: String }` (Serialize/Deserialize/Clone/Debug/PartialEq)
  - `AppConfig` fields: `game_detection_enabled: bool` (default true), `write_file_properties: bool` (default true), `day_rollover_hour: u8` (default 4), `game_overrides: Vec<GameOverride>` (default empty)
  - `AppConfig::remember_game_override(&mut self, exe: &str, name: &str)` — upsert by case-insensitive exe stem

- [ ] **Step 1: Write the failing tests** (append inside `config.rs` `mod tests`)

```rust
#[test]
fn test_game_detection_defaults() {
    let cfg = AppConfig::default();
    assert!(cfg.game_detection_enabled);
    assert!(cfg.write_file_properties);
    assert_eq!(cfg.day_rollover_hour, 4);
    assert!(cfg.game_overrides.is_empty());
}

#[test]
fn test_game_overrides_toml_roundtrip() {
    let tmp = NamedTempFile::new().unwrap();
    let mut cfg = AppConfig::default();
    cfg.remember_game_override("cs2", "Counter-Strike 2");
    cfg.save_to(tmp.path()).expect("save");
    let loaded = AppConfig::load_from(tmp.path()).expect("load");
    assert_eq!(loaded.game_overrides.len(), 1);
    assert_eq!(loaded.game_overrides[0].exe, "cs2");
    assert_eq!(loaded.game_overrides[0].name, "Counter-Strike 2");
}

#[test]
fn test_remember_game_override_upserts_case_insensitive() {
    let mut cfg = AppConfig::default();
    cfg.remember_game_override("CS2", "Wrong Name");
    cfg.remember_game_override("cs2", "Counter-Strike 2");
    assert_eq!(cfg.game_overrides.len(), 1);
    assert_eq!(cfg.game_overrides[0].name, "Counter-Strike 2");
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test config` → compile FAIL (fields missing).

- [ ] **Step 3: Implement**

Add to `config.rs` (defaults section):

```rust
fn default_game_detection_enabled() -> bool { true }
fn default_write_file_properties() -> bool { true }
fn default_day_rollover_hour() -> u8 { 4 }
fn default_game_overrides() -> Vec<GameOverride> { Vec::new() }

/// exe stem → display-name override, remembered when the user corrects a
/// wrong detection. A Vec of structs, NOT a HashMap: TOML rejects non-string
/// map keys and the list form renders cleanly in the config file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameOverride {
    pub exe: String,
    pub name: String,
}
```

Struct fields (with the same doc-comment style as neighbors):

```rust
    /// Detect the focused game/app when a clip is saved and record it in
    /// history + file properties. Master switch for the whole feature.
    #[serde(default = "default_game_detection_enabled")]
    pub game_detection_enabled: bool,

    /// Mirror game/rating/description into Windows file properties
    /// (Explorer Tags/Rating/Comments). history.jsonl is written regardless.
    #[serde(default = "default_write_file_properties")]
    pub write_file_properties: bool,

    /// Hour (0-23) at which "today" starts for history and daily stats —
    /// default 4 AM so a late-night session doesn't split at midnight.
    #[serde(default = "default_day_rollover_hour")]
    pub day_rollover_hour: u8,

    /// User corrections for misdetected games, checked before heuristics.
    #[serde(default = "default_game_overrides")]
    pub game_overrides: Vec<GameOverride>,
```

Add all four to `impl Default`. Then:

```rust
    /// Upsert a detection override (case-insensitive on exe stem).
    pub fn remember_game_override(&mut self, exe: &str, name: &str) {
        let lower = exe.to_lowercase();
        self.game_overrides.retain(|o| o.exe.to_lowercase() != lower);
        self.game_overrides.push(GameOverride { exe: exe.to_string(), name: name.to_string() });
    }
```

In `src/types/index.ts`, extend the `AppConfig` interface:

```ts
  game_detection_enabled: boolean;
  write_file_properties: boolean;
  day_rollover_hour: number;
  game_overrides: { exe: string; name: string }[];
```

- [ ] **Step 4: Verify** — `cargo test config` all pass, 0 warnings; `pnpm exec tsc --noEmit` clean.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/config.rs src/types/index.ts
git commit -m "Add game-detection config: master toggle, property-write toggle, 4am rollover hour, override list"
```

---

### Task 3: `gamedetect.rs` — pure classification + Win32 foreground snapshot

**Files:**
- Create: `src-tauri/src/gamedetect.rs`
- Modify: `src-tauri/src/lib.rs` (`mod gamedetect;`)
- Modify: `src-tauri/Cargo.toml` (extend `windows-sys` features)

**Interfaces:**
- Consumes: `GameOverride` from Task 2.
- Produces:
  - `pub struct ForegroundApp { pub exe_stem: String, pub product_name: Option<String>, pub title: String, pub fullscreen: bool }`
  - `pub fn classify(app: &ForegroundApp, overrides: &[GameOverride]) -> String` — PURE, fully unit-tested
  - `pub struct GameSnapshot { pub label: String, pub exe_stem: String, pub taken_at: std::time::SystemTime }`
  - `pub fn snapshot_foreground(overrides: &[GameOverride]) -> Option<GameSnapshot>` — Win32, returns None if anything fails

- [ ] **Step 1: Add windows-sys features**

In `Cargo.toml`, extend the existing `windows-sys` feature list with:

```toml
    "Win32_Graphics_Gdi",
    "Win32_Storage_FileSystem",
```

(`Win32_UI_WindowsAndMessaging`, `Win32_Foundation`, `Win32_System_Threading` are already present.)

- [ ] **Step 2: Write the failing classification tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameOverride;

    fn app(exe: &str, product: Option<&str>, title: &str, fullscreen: bool) -> ForegroundApp {
        ForegroundApp {
            exe_stem: exe.to_string(),
            product_name: product.map(|s| s.to_string()),
            title: title.to_string(),
            fullscreen,
        }
    }

    #[test]
    fn test_override_wins_regardless_of_fullscreen() {
        let overrides = vec![GameOverride { exe: "CS2".into(), name: "Counter-Strike 2".into() }];
        assert_eq!(classify(&app("cs2", Some("Valve CS2"), "cs2", true), &overrides), "Counter-Strike 2");
        assert_eq!(classify(&app("cs2", None, "cs2", false), &overrides), "Counter-Strike 2");
    }

    #[test]
    fn test_fullscreen_prefers_product_name_then_title_then_exe() {
        assert_eq!(classify(&app("r5apex", Some("Apex Legends"), "Apex", true), &[]), "Apex Legends");
        assert_eq!(classify(&app("r5apex", None, "Apex Legends Window", true), &[]), "Apex Legends Window");
        assert_eq!(classify(&app("r5apex", None, "", true), &[]), "r5apex");
    }

    #[test]
    fn test_windowed_gets_desktop_prefix() {
        assert_eq!(classify(&app("discord", Some("Discord"), "#general", false), &[]), "Desktop-Discord");
        assert_eq!(classify(&app("weirdapp", None, "", false), &[]), "Desktop-weirdapp");
    }

    #[test]
    fn test_whitespace_product_name_is_ignored() {
        assert_eq!(classify(&app("game", Some("   "), "Real Title", true), &[]), "Real Title");
    }
}
```

- [ ] **Step 3: Run to verify failure** — `cargo test gamedetect` → compile FAIL.

- [ ] **Step 4: Implement**

```rust
//! Foreground-app detection for clip labeling. Pure classification is
//! separated from the unsafe Win32 snapshot so the naming rules are unit-
//! testable. Snapshot is taken at save-clip-bind press (user is in-game at
//! that instant) with a fallback at file-creation (see lib.rs wiring).

use crate::config::GameOverride;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ForegroundApp {
    pub exe_stem: String,
    pub product_name: Option<String>,
    pub title: String,
    pub fullscreen: bool,
}

#[derive(Debug, Clone)]
pub struct GameSnapshot {
    pub label: String,
    pub exe_stem: String,
    pub taken_at: SystemTime,
}

/// Naming rules (spec order): override map → fullscreen game name →
/// Desktop-<app>. Product name from exe version info beats window title
/// beats exe stem, skipping blank values.
pub fn classify(app: &ForegroundApp, overrides: &[GameOverride]) -> String {
    let lower = app.exe_stem.to_lowercase();
    if let Some(o) = overrides.iter().find(|o| o.exe.to_lowercase() == lower) {
        return o.name.clone();
    }
    let best = [app.product_name.as_deref(), Some(app.title.as_str()), Some(app.exe_stem.as_str())]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("Unknown")
        .to_string();
    if app.fullscreen {
        best
    } else {
        // Desktop apps prefer the app's name over its window title
        // ("Desktop-Discord", not "Desktop-#general").
        let app_name = [app.product_name.as_deref(), Some(app.exe_stem.as_str())]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|s| !s.is_empty())
            .unwrap_or("Unknown");
        format!("Desktop-{}", app_name)
    }
}

/// Win32 snapshot of the current foreground window. Any failure → None
/// (caller records the clip without a game label). Never panics.
pub fn snapshot_foreground(overrides: &[GameOverride]) -> Option<GameSnapshot> {
    let raw = read_foreground()?;
    Some(GameSnapshot {
        label: classify(&raw, overrides),
        exe_stem: raw.exe_stem,
        taken_at: SystemTime::now(),
    })
}
```

`read_foreground()` (private, `#[cfg(windows)]`) — full implementation required, no stubs:

```rust
fn read_foreground() -> Option<ForegroundApp> {
    use windows_sys::Win32::Foundation::{CloseHandle, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 { return None; }

        // Window title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = String::from_utf16_lossy(&title_buf[..title_len.max(0) as usize]);

        // Exe path via the owning process
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 { return None; }
        let hproc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if hproc == 0 { return None; }
        let mut path_buf = [0u16; 1024];
        let mut path_len = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(hproc, 0, path_buf.as_mut_ptr(), &mut path_len);
        CloseHandle(hproc);
        if ok == 0 { return None; }
        let exe_path = String::from_utf16_lossy(&path_buf[..path_len as usize]);
        let exe_stem = std::path::Path::new(&exe_path)
            .file_stem()?
            .to_string_lossy()
            .to_string();

        // Fullscreen test: window rect covers its monitor's full rect —
        // catches exclusive fullscreen AND borderless-windowed in one check.
        let mut wrect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(hwnd, &mut wrect) == 0 { return None; }
        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmon, &mut mi) == 0 { return None; }
        let m = mi.rcMonitor;
        let fullscreen = wrect.left <= m.left && wrect.top <= m.top
            && wrect.right >= m.right && wrect.bottom >= m.bottom;

        Some(ForegroundApp {
            exe_stem,
            product_name: product_name_of(&exe_path),
            title,
            fullscreen,
        })
    }
}

/// Read ProductName (fallback FileDescription) from the exe's version info.
fn product_name_of(exe_path: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle);
        if size == 0 { return None; }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, data.as_mut_ptr() as _) == 0 {
            return None;
        }

        // Translation table → first language, then the two string values.
        let mut cp_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut cp_len = 0u32;
        let trans_key: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain(std::iter::once(0)).collect();
        if VerQueryValueW(data.as_ptr() as _, trans_key.as_ptr(), &mut cp_ptr, &mut cp_len) == 0
            || cp_len < 4 { return None; }
        let lang = *(cp_ptr as *const u16);
        let code = *(cp_ptr as *const u16).add(1);

        for field in ["ProductName", "FileDescription"] {
            let key: Vec<u16> = format!("\\StringFileInfo\\{:04x}{:04x}\\{}", lang, code, field)
                .encode_utf16().chain(std::iter::once(0)).collect();
            let mut val_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut val_len = 0u32;
            if VerQueryValueW(data.as_ptr() as _, key.as_ptr(), &mut val_ptr, &mut val_len) != 0
                && val_len > 1
            {
                let slice = std::slice::from_raw_parts(val_ptr as *const u16, (val_len - 1) as usize);
                let s = String::from_utf16_lossy(slice).trim().to_string();
                if !s.is_empty() { return Some(s); }
            }
        }
        None
    }
}
```

- [ ] **Step 5: Verify** — `cargo test gamedetect` → 4 passed; full `cargo test` still green, **0 warnings** (if `snapshot_foreground` is not yet called anywhere, add `#[allow(dead_code)]` is FORBIDDEN — instead wire nothing yet but mark the module `pub` and expect the warning to disappear in Task 5; if the build warns now, gate with `pub` use in Task 5 within the same PR — acceptable interim: add a `#[cfg(test)]` no-op reference is NOT needed since Tasks 3–5 land in one session; run the 0-warning check at Task 5's end instead).

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/gamedetect.rs src-tauri/src/lib.rs
git commit -m "Add foreground game detection: pure classifier + Win32 snapshot with version-info naming"
```

---

### Task 4: `props.rs` — Windows property writes with exclusive-access probe

**Files:**
- Create: `src-tauri/src/props.rs`
- Modify: `src-tauri/src/lib.rs` (`mod props;`)
- Modify: `src-tauri/Cargo.toml` (add `windows` crate)

**Interfaces:**
- Produces:
  - `pub enum PropValue { Game(String), Stars(u8), Description(String) }`
  - `pub fn stars_to_system_rating(stars: u8) -> u32` — 1→1, 2→25, 3→50, 4→75, 5→99 (clamped 1–5)
  - `pub fn probe_exclusive(path: &Path) -> bool` — true if we can open read+write with share_mode(0)
  - `pub fn write_with_retry(path: &Path, values: &[PropValue]) -> Result<(), String>` — probe loop (5 attempts × 1.7 s) then COM write; call ONLY from the blocking pool

- [ ] **Step 1: Add the `windows` crate**

```toml
windows = { version = "0.61", features = [
    "Win32_UI_Shell_PropertiesSystem",
    "Win32_System_Com",
    "Win32_System_Com_StructuredStorage",
    "Win32_System_Variant",
    "Win32_Foundation",
] }
```

(Coexists fine with `windows-sys` — different crates. If 0.61 isn't the current version, use the latest and adjust paths per its docs; property-store APIs live in `windows::Win32::UI::Shell::PropertiesSystem`.)

- [ ] **Step 2: Write the failing tests** (pure + probe; the COM write itself is manual-verify only)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stars_to_system_rating_explorer_scale() {
        assert_eq!(stars_to_system_rating(1), 1);
        assert_eq!(stars_to_system_rating(2), 25);
        assert_eq!(stars_to_system_rating(3), 50);
        assert_eq!(stars_to_system_rating(4), 75);
        assert_eq!(stars_to_system_rating(5), 99);
        // Clamped, never panics
        assert_eq!(stars_to_system_rating(0), 1);
        assert_eq!(stars_to_system_rating(9), 99);
    }

    #[test]
    fn test_probe_exclusive_free_vs_held_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mp4");
        std::fs::write(&path, b"stub").unwrap();

        assert!(probe_exclusive(&path), "free file should probe ok");

        // Hold the file with NO sharing (like OBS mid-write) — probe must fail.
        use std::os::windows::fs::OpenOptionsExt;
        let _hold = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        assert!(!probe_exclusive(&path), "held file must fail the probe");
    }

    #[test]
    fn test_probe_missing_file_is_false() {
        assert!(!probe_exclusive(std::path::Path::new("C:/nope/missing.mp4")));
    }
}
```

- [ ] **Step 3: Run to verify failure** — `cargo test props` → compile FAIL.

- [ ] **Step 4: Implement**

```rust
//! Best-effort Windows property mirror (Explorer Tags / Rating / Comments).
//! history.jsonl is the source of truth; these writes exist so other apps
//! and Explorer can see game/rating/description on the file itself.
//! HARD RULE: never touch a file something else still holds — probe with
//! exclusive share access first, retry, then skip with a warning.
//! Contract: Docs/Features/Clip-Metadata-Interop.md.

use std::path::Path;

/// One property to mirror onto the file.
pub enum PropValue {
    Game(String),
    Stars(u8),
    Description(String),
}

/// Explorer's star buckets for System.Rating (1-99).
pub fn stars_to_system_rating(stars: u8) -> u32 {
    match stars.clamp(1, 5) {
        1 => 1,
        2 => 25,
        3 => 50,
        4 => 75,
        _ => 99,
    }
}

/// Can we open the file exclusively (no other handle open)? Mirrors the
/// "is OBS done writing?" check. Missing file → false.
pub fn probe_exclusive(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
        .is_ok()
}

const PROBE_ATTEMPTS: u32 = 5;
const PROBE_DELAY_MS: u64 = 1700; // same cadence as the mover's move retries

/// Probe-then-write with retries. BLOCKING (sleeps up to ~8.5 s) — only
/// call from the blocking pool. Err(msg) is for a warning log, never a toast.
pub fn write_with_retry(path: &Path, values: &[PropValue]) -> Result<(), String> {
    let mut attempt = 0;
    loop {
        if probe_exclusive(path) {
            break;
        }
        attempt += 1;
        if attempt >= PROBE_ATTEMPTS {
            return Err(format!(
                "file still locked after {} attempts — skipped property write (history.jsonl has the data)",
                PROBE_ATTEMPTS
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(PROBE_DELAY_MS));
    }
    write_properties(path, values)
}
```

`write_properties` (the COM part — full skeleton; the implementer should consult the `windows` crate docs for exact PROPVARIANT constructor names in the pinned version):

```rust
fn write_properties(path: &Path, values: &[PropValue]) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IPropertyStore, PSGetPropertyKeyFromName, SHGetPropertyStoreFromParsingName, GPS_READWRITE,
    };
    use windows::Win32::System::Com::StructuredStorage::{
        InitPropVariantFromStringAsVector, InitPropVariantFromStringVector,
        InitPropVariantFromUInt32,
    };

    unsafe {
        // Property handlers want an STA; init per-call on this blocking thread.
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| -> Result<(), String> {
            let store: IPropertyStore = SHGetPropertyStoreFromParsingName(
                &HSTRING::from(path.to_string_lossy().as_ref()),
                None,
                GPS_READWRITE,
            )
            .map_err(|e| format!("open property store: {e}"))?;

            for v in values {
                let (key_name, var) = match v {
                    PropValue::Game(name) => (
                        "System.Keywords",
                        InitPropVariantFromStringAsVector(&HSTRING::from(name.as_str()))
                            .map_err(|e| format!("keywords propvariant: {e}"))?,
                    ),
                    PropValue::Stars(stars) => (
                        "System.Rating",
                        InitPropVariantFromUInt32(stars_to_system_rating(*stars))
                            .map_err(|e| format!("rating propvariant: {e}"))?,
                    ),
                    PropValue::Description(text) => (
                        "System.Comment",
                        // Single VT_LPWSTR string — NOT a vector (see note below).
                        propvariant_from_string(text)
                            .map_err(|e| format!("comment propvariant: {e}"))?,
                    ),
                };
                let mut key = Default::default();
                PSGetPropertyKeyFromName(&HSTRING::from(key_name), &mut key)
                    .map_err(|e| format!("resolve {key_name}: {e}"))?;
                store.SetValue(&key, &var).map_err(|e| format!("set {key_name}: {e}"))?;
            }
            store.Commit().map_err(|e| format!("commit: {e}"))?;
            Ok(())
        })();
        if com.is_ok() {
            CoUninitialize();
        }
        result
    }
}
```

**IMPORTANT implementation note:** `propvariant_from_string` is a small private helper the implementer writes against the pinned `windows` version: `System.Comment` needs a plain `VT_LPWSTR` PROPVARIANT (in recent versions `PROPVARIANT::from(text.as_str())` or `InitPropVariantFromString`-equivalent — check the crate docs; do NOT use the vector constructor). The Keywords value MUST be a string vector (`VT_VECTOR|VT_LPWSTR`) — that's what `InitPropVariantFromStringAsVector` produces from a single string. The unit-test gate covers the pure parts only; the COM write is verified manually (Explorer shows Tags/Rating/Comments — checklist §15).

- [ ] **Step 5: Verify** — `cargo test props` → 3 passed. Full `cargo test` green.

- [ ] **Step 6: Manual smoke check (dev box, one file)**

Temporarily add to a test-only `#[test] #[ignore]` fn or run via the app later in Task 5 — acceptable to defer to the master verification checklist. Explorer → clip Properties → Details must show Tags/Rating/Comments after a wired run.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/props.rs src-tauri/src/lib.rs
git commit -m "Add Windows property mirror with exclusive-access probe and mover-cadence retry"
```

---

### Task 5: Wire-up — snapshot at save-press, attach at file-created, history events on move/rename/undo

**Files:**
- Modify: `src-tauri/src/state.rs` (pending snapshot + accessor)
- Modify: `src-tauri/src/lib.rs` (`HotkeyAction::SaveClipHealthCheck` arm ~line 344; `handle_file_created` ~line 684)
- Modify: `src-tauri/src/commands.rs` (`move_file_with_key`, `do_rename_file`, `do_undo`, `drop_files_to_gkey`)

**Interfaces:**
- Consumes: `gamedetect::{snapshot_foreground, GameSnapshot}`, `history::{HistoryEvent, history_path, append}`, `props::{PropValue, write_with_retry}`.
- Produces:
  - `AppStateInner.pending_game: Option<GameSnapshot>` and `AppStateInner.take_pending_game(&mut self, max_age: Duration) -> Option<GameSnapshot>`
  - `AppStateInner.clip_games: HashMap<PathBuf, String>` — session map current-path → game label, updated on move/rename so later events keep their game

- [ ] **Step 1: Write the failing state test** (in `state.rs` `mod tests`)

```rust
#[test]
fn test_take_pending_game_respects_age() {
    use std::time::{Duration, SystemTime};
    let mut s = AppStateInner::new(AppConfig::default(), PathBuf::new());
    assert!(s.take_pending_game(Duration::from_secs(30)).is_none());

    s.pending_game = Some(crate::gamedetect::GameSnapshot {
        label: "Counter-Strike 2".into(),
        exe_stem: "cs2".into(),
        taken_at: SystemTime::now(),
    });
    let snap = s.take_pending_game(Duration::from_secs(30)).expect("fresh snapshot");
    assert_eq!(snap.label, "Counter-Strike 2");
    assert!(s.pending_game.is_none(), "take consumes");

    s.pending_game = Some(crate::gamedetect::GameSnapshot {
        label: "Old".into(),
        exe_stem: "old".into(),
        taken_at: SystemTime::now() - Duration::from_secs(120),
    });
    assert!(s.take_pending_game(Duration::from_secs(30)).is_none(), "stale is discarded");
}
```

- [ ] **Step 2: Run to verify failure**, then implement in `state.rs`:

```rust
    /// Game snapshot captured at the save-clip-bind press, consumed by the
    /// next FileCreated. Age-gated so a stale press can't mislabel a later
    /// clip that arrived by other means.
    pub pending_game: Option<crate::gamedetect::GameSnapshot>,

    /// Session map: clip's CURRENT path → detected game. Kept in sync on
    /// move/rename so rate/label events after sorting still carry the game.
    pub clip_games: HashMap<PathBuf, String>,
```

(init both in `new()`: `pending_game: None`, `clip_games: HashMap::new()`), plus:

```rust
    pub fn take_pending_game(
        &mut self,
        max_age: std::time::Duration,
    ) -> Option<crate::gamedetect::GameSnapshot> {
        let snap = self.pending_game.take()?;
        if snap.taken_at.elapsed().unwrap_or_default() <= max_age {
            Some(snap)
        } else {
            None
        }
    }
```

- [ ] **Step 3: Snapshot at save-press** — in the `HotkeyAction::SaveClipHealthCheck` arm in `lib.rs` (before `spawn_save_clip_health_check`), add:

```rust
                                // Game detection: the user is in-game at this
                                // exact instant — snapshot the foreground app
                                // for the clip about to arrive. Win32 calls are
                                // cheap but not free; do them off the async loop.
                                {
                                    let st = state.clone();
                                    tauri::async_runtime::spawn_blocking(move || {
                                        let (enabled, overrides) = {
                                            let s = st.lock().unwrap();
                                            (s.config.game_detection_enabled, s.config.game_overrides.clone())
                                        };
                                        if enabled {
                                            let snap = gamedetect::snapshot_foreground(&overrides);
                                            let mut s = st.lock().unwrap();
                                            s.pending_game = snap;
                                        }
                                    });
                                }
```

- [ ] **Step 4: Attach at file-created** — in `handle_file_created` (`lib.rs`), inside the existing state-update critical section (~line 684 where `current_file` is set), take the pending snapshot; after the section, fall back to a fresh snapshot; then append history + spawn the property write:

```rust
    // Game detection: prefer the snapshot from the save-press instant;
    // fall back to "what's focused right now" for clips that arrived
    // without a hotkey press (watcher-only / OBS event).
    let game: Option<String> = {
        let taken = {
            let mut s = state.lock().unwrap();
            s.take_pending_game(std::time::Duration::from_secs(30))
        };
        match taken {
            Some(snap) => Some(snap.label),
            None if config.game_detection_enabled => {
                let overrides = config.game_overrides.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    gamedetect::snapshot_foreground(&overrides).map(|s| s.label)
                })
                .await
                .ok()
                .flatten()
            }
            None => None,
        }
    };
    let (config_path, hist_event) = {
        let mut s = state.lock().unwrap();
        if let Some(g) = &game {
            s.clip_games.insert(path.clone(), g.clone());
        }
        let mut e = history::HistoryEvent::new("created", &path, "app");
        if let Some(g) = &game { e = e.with_game(g); }
        (s.config_path.clone(), e)
    };
    let hist_path = history::history_path(&config_path);
    let write_props = config.write_file_properties;
    {
        let path_for_props = path.clone();
        let game_for_props = game.clone();
        tauri::async_runtime::spawn_blocking(move || {
            history::append(&hist_path, &hist_event);
            if write_props {
                if let Some(g) = game_for_props {
                    if let Err(msg) =
                        props::write_with_retry(&path_for_props, &[props::PropValue::Game(g)])
                    {
                        eprintln!("props: {}", msg);
                    }
                }
            }
        });
    }
```

Also add the game to the `file-created` log line when present: change the `msg` construction to append `" — {game}"` when `game.is_some()` (e.g. `New file: clip.mp4 (42.3MB) — Counter-Strike 2`), and add `game: Option<String>` to `FileCreatedPayload` in `events.rs` (camelCase serde like its siblings) so the UI can show it later.

- [ ] **Step 5: History events from moves/renames/undo** — in `commands.rs`:
  - `move_file_with_key(...)`: after a successful move (where `record_gkey_move` is called), look up `clip_games` under the SAME lock: remove the old-path entry and re-insert under the new path; clone `config_path` + game; after the lock drops, `history::append` a `"moved"` event with `old_path`, `key`, and game. Source: pass through a `source: &str` argument — `do_press_gkey` passes `"hotkey"`, `drop_files_to_gkey` passes `"drop"` (add the parameter; both callers are in this file plus lib.rs's blocking spawns which go through these fns).
  - `do_rename_file`: same pattern — `"renamed"` event with `old_path`, game from `clip_games` (re-keyed), source `"app"`.
  - `do_undo`: for each restored `UndoMove`, append `"undone"` with `path` = restored-to (`from`), `old_path` = undone-from (`to`), re-key `clip_games`, source `"app"`.
  - All appends happen AFTER locks are dropped, on the already-blocking thread these fns run on.

- [ ] **Step 6: Verify** — full `cargo test` green, **0 warnings** (Tasks 3–4 modules are now all referenced). `pnpm exec tsc --noEmit` + `pnpm build` clean (FileCreatedPayload change touches `types/index.ts`: add `game?: string | null` to the FileCreated payload type).

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src src/types
git commit -m "Wire game detection into clip flow: snapshot at save-press, history events on create/move/rename/undo, property mirror"
```

---

### Task 6: Settings UI — Game Detection section

**Files:**
- Modify: `src/SettingsApp.tsx` (new section; follow the existing section/row component patterns in that file)

**Interfaces:**
- Consumes: config fields from Task 2 (already in `types/index.ts` and flowing through the existing draft/save model — `game_detection_enabled`, `write_file_properties`, `day_rollover_hour`, `game_overrides`).

- [ ] **Step 1: Add the section** (match the file's existing Switch/Input/row idioms — read neighboring sections first):
  - **"Game detection"** header with master `Switch` bound to `draft.game_detection_enabled`.
  - `Switch` "Write game/rating/description into file properties (visible in Explorer)" → `write_file_properties`, disabled when master is off.
  - Number input "Day starts at (hour, 0–23)" → `day_rollover_hour`, clamped 0–23 on change, helper text "History and daily stats roll over at this hour — default 4 AM for late-night sessions."
  - **Overrides table**: one row per `game_overrides` entry — read-only `exe` cell, editable `name` input, remove (✕) button; plus an "Add override" row with exe + name inputs and an Add button (trim both, reject empty, upsert case-insensitively on exe like the backend). Helper text: "When a game is detected wrong, corrections you save here (or via Remember) always win."
  - All edits mutate the draft only; the existing Save flow persists (no new commands needed — `game_overrides` round-trips through `updateConfig` since it's a plain config field; do NOT strip it in `handleSave` — only `rename_mru` gets stripped).

- [ ] **Step 2: Verify** — `pnpm exec tsc --noEmit` clean; `pnpm build` clean.

- [ ] **Step 3: Commit**

```powershell
git add src/SettingsApp.tsx
git commit -m "Add Game detection settings section: toggles, rollover hour, override editor"
```

---

### Task 7: Docs + verification entries

**Files:**
- Modify: `Docs/Verify/2026-07-10-master-verification-checklist.md` (new §15 "Game detection + history store", update the **Updated:** line)
- Modify: `Docs/Features/Clip-Metadata-Interop.md` (flip status line for the shipped parts: history.jsonl + game/property writes → "Implemented"; label suffix/rating/description writes remain "designed" until Phase 3)
- Modify: `Docs/future/feature-ideas.md` only if anything got consciously deferred during implementation

- [ ] **Step 1: Add §15 to the checklist** — commit range, ship date, what automation covered (unit tests listed by name), and the human items:
  - Clip saved while a fullscreen game focused → log shows `— <game>`; `history.jsonl` gains a `created` line with the right game; Explorer Details shows the game in Tags after OBS releases the file.
  - Borderless-windowed game → same result.
  - Clip saved with only Discord focused (windowed) → `Desktop-Discord`.
  - Wrong detection → add override in Settings → next clip uses the corrected name.
  - Hold the file open in another program → property write retries then skips with a warning log line; history still has the game.
  - G1 move / rename / undo each append their history line (open the JSONL and eyeball).
  - Detection toggle off → no game anywhere, everything else unaffected.

- [ ] **Step 2: Commit**

```powershell
git add Docs
git commit -m "Add game-detection verification section; mark interop contract parts implemented"
```

---

## Self-Review Notes

- Spec coverage: detection moment (T5 steps 3–4), classification + overrides (T3), Desktop- fallback (T3), enabled-by-default (T2), history.jsonl schema/location (T1), property mapping + lock probe + retry-skip (T4), config additions (T2), Settings editability of overrides (T6), tests (each task) + manual items (T7). `day_rollover_hour` ships in config now (T2) but is CONSUMED in Phase 2 (History panel + stats switch) — intentional, documented here.
- Deferred to Phase 2 plan: `get_history` command, History button/panel, stats rollover switch. Deferred to Phase 3 plan: rate/label/describe writers (props.rs already supports Stars/Description so Phase 3 only adds callers), overlay.
- Type consistency: `GameSnapshot.label`/`exe_stem` (T3) match T5's usage; `HistoryEvent` builder names match all call sites; `PropValue::Game` used in T5.
- Known judgment call for the implementer: exact `windows`-crate PROPVARIANT constructor names vary by version — Task 4 flags this explicitly with the type requirements (Keywords = string VECTOR, Comment = single string, Rating = UInt32).
