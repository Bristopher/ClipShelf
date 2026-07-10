use chrono::Local;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::events::{LogCategory, LogEntryPayload, LogLevel};

/// Cap on the in-memory buffers. With autostart the app can run for days;
/// unbounded Vecs would grow forever. Oldest entries are evicted — the
/// on-disk daily log keeps the full record.
const MAX_ENTRIES: usize = 5000;

pub struct AppLogger {
    history: Vec<LogEntryPayload>,
    display_buffer: Vec<LogEntryPayload>,
    log_dir: Option<PathBuf>,
    log_enabled: bool,
}

impl AppLogger {
    pub fn new(videos_folder: &str, log_enabled: bool) -> Self {
        let log_dir = if !videos_folder.is_empty() {
            let dir = PathBuf::from(videos_folder).join("logs");
            if let Err(e) = fs::create_dir_all(&dir) {
                eprintln!("Failed to create log dir: {}", e);
                None
            } else {
                Some(dir)
            }
        } else {
            None
        };

        Self {
            history: Vec::new(),
            display_buffer: Vec::new(),
            log_dir,
            log_enabled,
        }
    }

    /// Re-derive the log directory and enabled flag after a config change,
    /// keeping the in-memory history/display buffers intact. Without this,
    /// changing the videos folder in Settings left daily file logs writing
    /// to the old folder until an app restart.
    pub fn reconfigure(&mut self, videos_folder: &str, log_enabled: bool) {
        self.log_enabled = log_enabled;
        self.log_dir = if !videos_folder.is_empty() {
            let dir = PathBuf::from(videos_folder).join("logs");
            if let Err(e) = fs::create_dir_all(&dir) {
                eprintln!("Failed to create log dir: {}", e);
                None
            } else {
                Some(dir)
            }
        } else {
            None
        };
    }

    pub fn log(
        &mut self,
        level: LogLevel,
        message: String,
        category: LogCategory,
    ) -> LogEntryPayload {
        self.log_with_path(level, message, category, None)
    }

    /// Like `log`, but attaches the file path the entry refers to so the UI
    /// can make it clickable (reveal in Explorer / play).
    pub fn log_with_path(
        &mut self,
        level: LogLevel,
        message: String,
        category: LogCategory,
        path: Option<String>,
    ) -> LogEntryPayload {
        let timestamp = Local::now().format("%I:%M:%S %p").to_string();
        let entry = LogEntryPayload {
            timestamp,
            level,
            message,
            category,
            path,
        };
        self.history.push(entry.clone());
        self.display_buffer.push(entry.clone());
        if self.history.len() > MAX_ENTRIES {
            let excess = self.history.len() - MAX_ENTRIES;
            self.history.drain(..excess);
        }
        if self.display_buffer.len() > MAX_ENTRIES {
            let excess = self.display_buffer.len() - MAX_ENTRIES;
            self.display_buffer.drain(..excess);
        }
        entry
    }

    pub fn write_to_file(&self, line: &str) {
        if !self.log_enabled {
            return;
        }
        let Some(ref dir) = self.log_dir else {
            return;
        };
        let filename = format!("ObsMoveLog {}.txt", Local::now().format("%Y-%m-%d"));
        let path = dir.join(filename);
        match fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "{}", line) {
                    eprintln!("Failed to write to log file: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to open log file: {}", e);
            }
        }
    }

    pub fn wipe_display(&mut self) {
        self.display_buffer.clear();
    }

    /// Rebuilds display_buffer from history, filtering out HotkeyPressed entries
    /// and entries whose message contains "No current_file".
    pub fn restore_display(&mut self) -> Vec<LogEntryPayload> {
        self.display_buffer = self
            .history
            .iter()
            .filter(|entry| {
                !matches!(entry.category, LogCategory::HotkeyPressed)
                    && !entry.message.contains("No current_file")
            })
            .cloned()
            .collect();
        self.display_buffer.clone()
    }

    pub fn display_entries(&self) -> &[LogEntryPayload] {
        &self.display_buffer
    }

    pub fn history(&self) -> &[LogEntryPayload] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_logger() -> AppLogger {
        AppLogger::new("", false)
    }

    #[test]
    fn test_log_adds_to_history_and_display() {
        let mut logger = make_logger();
        logger.log(LogLevel::Info, "hello".to_string(), LogCategory::System);

        assert_eq!(logger.history().len(), 1);
        assert_eq!(logger.display_entries().len(), 1);
        assert_eq!(logger.history()[0].message, "hello");
        assert_eq!(logger.display_entries()[0].message, "hello");
    }

    #[test]
    fn test_wipe_clears_display_but_keeps_history() {
        let mut logger = make_logger();
        logger.log(LogLevel::Info, "first".to_string(), LogCategory::System);
        logger.log(LogLevel::Info, "second".to_string(), LogCategory::System);

        logger.wipe_display();

        assert_eq!(logger.history().len(), 2);
        assert_eq!(logger.display_entries().len(), 0);
    }

    #[test]
    fn test_restore_filters_noise() {
        let mut logger = make_logger();

        // Should survive restore
        logger.log(LogLevel::Info, "file created".to_string(), LogCategory::FileCreated);

        // Should be filtered: HotkeyPressed
        logger.log(LogLevel::Info, "hotkey G1".to_string(), LogCategory::HotkeyPressed);

        // Should be filtered: "No current_file" system message
        logger.log(
            LogLevel::Warning,
            "No current_file set".to_string(),
            LogCategory::System,
        );

        // Should survive restore
        logger.log(LogLevel::Success, "file moved".to_string(), LogCategory::FileMoved);

        logger.wipe_display();
        assert_eq!(logger.display_entries().len(), 0);

        let restored = logger.restore_display();

        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].message, "file created");
        assert_eq!(restored[1].message, "file moved");
    }

    #[test]
    fn test_buffers_capped_evicting_oldest() {
        let mut logger = make_logger();
        for i in 0..(MAX_ENTRIES + 10) {
            logger.log(LogLevel::Info, format!("entry {}", i), LogCategory::System);
        }
        assert_eq!(logger.history().len(), MAX_ENTRIES);
        assert_eq!(logger.display_entries().len(), MAX_ENTRIES);
        // Oldest evicted, newest kept.
        assert_eq!(logger.history()[0].message, "entry 10");
        assert_eq!(
            logger.history().last().unwrap().message,
            format!("entry {}", MAX_ENTRIES + 9)
        );
    }

    #[test]
    fn test_write_to_file_with_logging_disabled() {
        // log_enabled = false, no log_dir — must not panic
        let logger = AppLogger::new("", false);
        logger.write_to_file("test line"); // should be a no-op
    }

    #[test]
    fn test_write_to_file_creates_daily_log() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let videos_folder = tmp.path().to_str().unwrap();

        let mut logger = AppLogger {
            history: Vec::new(),
            display_buffer: Vec::new(),
            log_dir: Some(PathBuf::from(videos_folder).join("logs")),
            log_enabled: true,
        };

        // Ensure the log dir exists
        fs::create_dir_all(logger.log_dir.as_ref().unwrap()).unwrap();

        let test_line = "test log entry";
        logger.write_to_file(test_line);

        let expected_filename = format!("ObsMoveLog {}.txt", Local::now().format("%Y-%m-%d"));
        let log_path = logger.log_dir.as_ref().unwrap().join(&expected_filename);

        assert!(log_path.exists(), "log file should exist at {:?}", log_path);

        let contents = fs::read_to_string(&log_path).unwrap();
        assert!(
            contents.contains(test_line),
            "log file should contain the written line"
        );

        // suppress unused warning — logger is intentionally used above
        let _ = logger.history();
    }
}
