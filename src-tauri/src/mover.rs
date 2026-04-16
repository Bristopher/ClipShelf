use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use regex::Regex;
use crate::config::AppConfig;

// --- MoveResult ---

#[derive(Debug, Clone)]
pub struct MoveResult {
    pub original_path: PathBuf,
    pub new_path: PathBuf,
    pub tag_applied: String,
}

// --- Tag shorthand mapping ---

pub fn get_tag_shorthand(bind_folder_name: &str) -> &str {
    match bind_folder_name {
        "!! or ! (G1)" => "!!",
        "odd or checked (G2)" => "CHKD",
        "!!! (G3)" => "!!!",
        other => other,
    }
}

// --- Insert tag in filename ---

pub fn insert_tag_in_filename(filename: &str, tag: &str) -> String {
    // OBS format: "2026-04-15 12-30-00"
    let obs_re = Regex::new(r"(\d{4}-\d{2}-\d{2} \d{2}-\d{2}-\d{2})").unwrap();
    if let Some(m) = obs_re.find(filename) {
        let end = m.end();
        return format!("{} {}{}", &filename[..end], tag, &filename[end..]);
    }

    // ShadowPlay format: "2026.04.15 - 12.30.00"
    let sp_re = Regex::new(r"(\d{4}\.\d{2}\.\d{2} - \d{2}\.\d{2}\.\d{2})").unwrap();
    if let Some(m) = sp_re.find(filename) {
        let end = m.end();
        return format!("{} {}{}", &filename[..end], tag, &filename[end..]);
    }

    // Fallback: insert before extension
    if let Some(dot_pos) = filename.rfind('.') {
        let stem = &filename[..dot_pos];
        let ext = &filename[dot_pos..];
        return format!("{} {}{}", stem, tag, ext);
    }

    // No extension: append at end
    format!("{} {}", filename, tag)
}

// --- File size helpers ---

pub fn file_size_mb(path: &Path) -> f64 {
    match std::fs::metadata(path) {
        Ok(meta) => meta.len() as f64 / (1024.0 * 1024.0),
        Err(_) => 0.0,
    }
}

pub fn is_file_size_valid(path: &Path, min_size_mb: f64) -> bool {
    file_size_mb(path) >= min_size_mb
}

// --- move_or_rename_file ---

pub fn move_or_rename_file(
    file_path: &Path,
    gkey: u8,
    config: &AppConfig,
) -> Result<MoveResult, String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    let bind_folder_name = match gkey {
        1 => config.g1_bind_folder_name.as_str(),
        2 => config.g2_bind_folder_name.as_str(),
        3 => config.g3_bind_folder_name.as_str(),
        _ => return Err(format!("Invalid gkey: {}. Must be 1, 2, or 3.", gkey)),
    };

    let tag = get_tag_shorthand(bind_folder_name);

    let original_filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "File has no valid name".to_string())?;

    let new_filename = insert_tag_in_filename(original_filename, tag);

    let new_path = if config.disable_file_movesorting {
        // Rename in same directory
        let parent = file_path
            .parent()
            .ok_or_else(|| "File has no parent directory".to_string())?;
        parent.join(&new_filename)
    } else {
        // Move to sort directory
        let sort_dir = config.sort_folder_path(gkey);
        std::fs::create_dir_all(&sort_dir)
            .map_err(|e| format!("Failed to create sort directory: {}", e))?;
        sort_dir.join(&new_filename)
    };

    // Retry up to 3 times with increasing delays
    let delays = [200u64, 500, 1000];
    let mut last_err = String::new();

    for (attempt, &delay_ms) in delays.iter().enumerate() {
        match std::fs::rename(file_path, &new_path) {
            Ok(_) => {
                return Ok(MoveResult {
                    original_path: file_path.to_path_buf(),
                    new_path,
                    tag_applied: tag.to_string(),
                });
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < delays.len() - 1 {
                    thread::sleep(Duration::from_millis(delay_ms));
                }
            }
        }
    }

    Err(format!("Failed to move/rename file after 3 attempts: {}", last_err))
}

// --- rename_file_with_text ---

pub fn rename_file_with_text(file_path: &Path, text: &str) -> Result<MoveResult, String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    let parent = file_path
        .parent()
        .ok_or_else(|| "File has no parent directory".to_string())?;

    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "File has no valid stem".to_string())?;

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let new_filename = format!("{} - {}{}", stem, text, ext);
    let new_path = parent.join(&new_filename);

    std::fs::rename(file_path, &new_path)
        .map_err(|e| format!("Failed to rename file: {}", e))?;

    Ok(MoveResult {
        original_path: file_path.to_path_buf(),
        new_path,
        tag_applied: text.to_string(),
    })
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_tag_shorthand_mapping() {
        assert_eq!(get_tag_shorthand("!! or ! (G1)"), "!!");
        assert_eq!(get_tag_shorthand("odd or checked (G2)"), "CHKD");
        assert_eq!(get_tag_shorthand("!!! (G3)"), "!!!");
        assert_eq!(get_tag_shorthand("custom folder"), "custom folder");
        assert_eq!(get_tag_shorthand("my_folder"), "my_folder");
    }

    #[test]
    fn test_insert_tag_obs_format() {
        let result = insert_tag_in_filename("Replay 2026-04-15 12-30-00.mp4", "!!");
        assert_eq!(result, "Replay 2026-04-15 12-30-00 !!.mp4");
    }

    #[test]
    fn test_insert_tag_shadowplay_format() {
        let result = insert_tag_in_filename("Overwatch 2026.04.15 - 12.30.00.mp4", "CHKD");
        assert_eq!(result, "Overwatch 2026.04.15 - 12.30.00 CHKD.mp4");
    }

    #[test]
    fn test_insert_tag_no_timestamp_fallback() {
        let result = insert_tag_in_filename("myclip.mp4", "!!");
        assert_eq!(result, "myclip !!.mp4");
    }

    #[test]
    fn test_insert_tag_no_extension() {
        let result = insert_tag_in_filename("myclip", "!!!");
        assert_eq!(result, "myclip !!!");
    }

    #[test]
    fn test_file_size_check() {
        let dir = tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("test_1kb.mp4");

        // Write 1 KB of data
        let mut f = std::fs::File::create(&file_path).expect("failed to create file");
        f.write_all(&[0u8; 1024]).expect("failed to write");
        drop(f);

        let size_mb = file_size_mb(&file_path);
        // 1 KB is not >= 6.5 MB
        assert!(!is_file_size_valid(&file_path, 6.5));
        // 1 KB is >= 0 MB
        assert!(is_file_size_valid(&file_path, 0.0));
        // Size should be approximately 1/1024 MB
        assert!(size_mb > 0.0 && size_mb < 0.01);
    }

    #[test]
    fn test_rename_only_mode() {
        let dir = tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("Replay 2026-04-15 12-30-00.mp4");
        std::fs::File::create(&file_path).expect("failed to create file");

        let mut config = AppConfig::default();
        config.disable_file_movesorting = true;
        config.g1_bind_folder_name = "!! or ! (G1)".to_string();

        let result = move_or_rename_file(&file_path, 1, &config).expect("move_or_rename_file failed");

        assert!(!result.original_path.exists(), "original file should be gone");
        assert!(result.new_path.exists(), "new file should exist");
        assert_eq!(result.tag_applied, "!!");

        // New file should be in same directory as original
        assert_eq!(result.new_path.parent(), file_path.parent());
        assert!(result.new_path.file_name().unwrap().to_str().unwrap().contains("!!"));
    }

    #[test]
    fn test_folder_sort_mode() {
        let dir = tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("Replay 2026-04-15 12-30-00.mp4");
        std::fs::File::create(&file_path).expect("failed to create file");

        let mut config = AppConfig::default();
        config.disable_file_movesorting = false;
        config.videos_folder = dir.path().to_str().unwrap().to_string();
        config.g2_bind_folder_name = "odd or checked (G2)".to_string();

        let result = move_or_rename_file(&file_path, 2, &config).expect("move_or_rename_file failed");

        assert!(!result.original_path.exists(), "original file should be gone");
        assert!(result.new_path.exists(), "new file should exist in sort dir");
        assert_eq!(result.tag_applied, "CHKD");

        // New file should be under videos_folder/sort/AHK sort/odd or checked (G2)/
        let expected_parent = dir.path().join("sort").join("AHK sort").join("odd or checked (G2)");
        assert_eq!(result.new_path.parent().unwrap(), expected_parent.as_path());
    }

    #[test]
    fn test_rename_with_text() {
        let dir = tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("myclip.mp4");
        std::fs::File::create(&file_path).expect("failed to create file");

        let result = rename_file_with_text(&file_path, "highlight").expect("rename_file_with_text failed");

        assert!(!result.original_path.exists(), "original file should be gone");
        assert!(result.new_path.exists(), "renamed file should exist");
        assert_eq!(
            result.new_path.file_name().unwrap().to_str().unwrap(),
            "myclip - highlight.mp4"
        );
        assert_eq!(result.tag_applied, "highlight");
    }

    #[test]
    fn test_move_nonexistent_file_returns_error() {
        let config = AppConfig::default();
        let fake_path = Path::new("/nonexistent/path/clip.mp4");
        let result = move_or_rename_file(fake_path, 1, &config);
        assert!(result.is_err(), "expected error for nonexistent file");
        assert!(result.unwrap_err().contains("does not exist"));
    }
}
