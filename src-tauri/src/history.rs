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
