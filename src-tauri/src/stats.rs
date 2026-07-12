//! Persistent per-G-key "today" counters.
//!
//! Lives in its own `gkey_stats.toml` next to the config file — same
//! rationale as window_layout.toml: the Settings draft/save model writes the
//! whole config back on Save, so frequently-auto-written state must not live
//! inside `AppConfig`. Counts roll over at the configurable
//! `config.day_rollover_hour` (default 4 AM), not calendar midnight, so a
//! late-night session's clips still count toward "today"; the recent-clips
//! lists stay session-only in AppState.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Named fields rather than a keyed map — TOML only allows string map keys,
/// so a `HashMap<u8, _>` silently fails to serialize.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyStats {
    /// Local calendar date the counts belong to ("YYYY-MM-DD").
    pub date: String,
    #[serde(default)]
    pub g1: u32,
    #[serde(default)]
    pub g2: u32,
    #[serde(default)]
    pub g3: u32,
}

/// The "logical" date a timestamp belongs to when the day starts at
/// `rollover_hour` instead of midnight — 3 AM clips count as yesterday for
/// a 4 AM rollover. Pure so it's testable; clamps the hour to 0-23.
pub fn logical_date_of(dt: chrono::DateTime<chrono::Local>, rollover_hour: u8) -> String {
    let shifted = dt - chrono::Duration::hours(rollover_hour.min(23) as i64);
    shifted.format("%Y-%m-%d").to_string()
}

pub fn logical_today(rollover_hour: u8) -> String {
    logical_date_of(chrono::Local::now(), rollover_hour)
}

pub fn stats_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(|p| p.join("gkey_stats.toml"))
        .unwrap_or_else(|| PathBuf::from("gkey_stats.toml"))
}

/// Load today's counts; a missing/unreadable file or one from a previous
/// day yields a fresh zeroed set for today.
pub fn load(path: &Path, rollover_hour: u8) -> DailyStats {
    let today = logical_today(rollover_hour);
    let loaded: Option<DailyStats> = std::fs::read_to_string(path)
        .ok()
        .and_then(|c| toml::from_str(&c).ok());
    match loaded {
        Some(s) if s.date == today => s,
        _ => DailyStats {
            date: today,
            ..Default::default()
        },
    }
}

pub fn save(path: &Path, stats: &DailyStats) {
    if let Ok(contents) = toml::to_string(stats) {
        // Temp-then-rename so a crash mid-write can't corrupt the file.
        let tmp = path.with_extension("toml.tmp");
        let result = std::fs::write(&tmp, contents).and_then(|_| std::fs::rename(&tmp, path));
        if let Err(e) = result {
            log::warn!("Failed to save gkey stats: {}", e);
        }
    }
}

impl DailyStats {
    pub fn count(&self, key: u8) -> u32 {
        match key {
            1 => self.g1,
            2 => self.g2,
            3 => self.g3,
            _ => 0,
        }
    }

    /// Bump a key's count, resetting everything first if the logical date
    /// has rolled over since the counts were loaded (app running past the
    /// rollover hour).
    pub fn increment(&mut self, key: u8, rollover_hour: u8) {
        let today = logical_today(rollover_hour);
        if self.date != today {
            *self = DailyStats {
                date: today,
                ..Default::default()
            };
        }
        match key {
            1 => self.g1 += 1,
            2 => self.g2 += 1,
            3 => self.g3 += 1,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_logical_date_respects_rollover_hour() {
        use chrono::TimeZone;
        let d = |y, mo, d, h, mi| chrono::Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
        // 23:59 belongs to its own calendar day
        assert_eq!(logical_date_of(d(2026, 7, 12, 23, 59), 4), "2026-07-12");
        // 03:59 still belongs to the PREVIOUS day (late-night session)
        assert_eq!(logical_date_of(d(2026, 7, 13, 3, 59), 4), "2026-07-12");
        // 04:00 exactly starts the new day
        assert_eq!(logical_date_of(d(2026, 7, 13, 4, 0), 4), "2026-07-13");
        // rollover 0 = plain calendar midnight
        assert_eq!(logical_date_of(d(2026, 7, 13, 0, 0), 0), "2026-07-13");
        // hour clamped: 25 treated as 23
        assert_eq!(logical_date_of(d(2026, 7, 13, 22, 59), 25), "2026-07-12");
    }

    #[test]
    fn test_roundtrip_and_stale_date_resets() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("gkey_stats.toml");

        let mut stats = DailyStats {
            date: logical_today(4),
            ..Default::default()
        };
        stats.increment(1, 4);
        stats.increment(1, 4);
        stats.increment(3, 4);
        save(&path, &stats);

        let loaded = load(&path, 4);
        assert_eq!(loaded.date, logical_today(4));
        assert_eq!(loaded.count(1), 2);
        assert_eq!(loaded.count(3), 1);

        // A file from a previous day loads as fresh zeroed counts.
        let stale = DailyStats {
            date: "2000-01-01".to_string(),
            g1: 99,
            ..Default::default()
        };
        save(&path, &stale);
        let loaded = load(&path, 4);
        assert_eq!(loaded.date, logical_today(4));
        assert_eq!(loaded.count(1), 0);
    }

    #[test]
    fn test_load_missing_file_is_fresh() {
        let loaded = load(Path::new("/nonexistent/gkey_stats.toml"), 4);
        assert_eq!(loaded.date, logical_today(4));
        assert_eq!(loaded.count(1), 0);
    }

    #[test]
    fn test_increment_rolls_over_stale_date() {
        let mut stats = DailyStats {
            date: "2000-01-01".to_string(),
            g2: 50,
            ..Default::default()
        };
        stats.increment(2, 4);
        assert_eq!(stats.date, logical_today(4));
        assert_eq!(stats.count(2), 1);
    }
}
