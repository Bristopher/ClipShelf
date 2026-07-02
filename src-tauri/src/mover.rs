use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;
use regex::Regex;
use crate::config::AppConfig;

static OBS_TIMESTAMP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4}-\d{2}-\d{2} \d{2}-\d{2}-\d{2})").unwrap());
static SP_TIMESTAMP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4}\.\d{2}\.\d{2} - \d{2}\.\d{2}\.\d{2})").unwrap());

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
    if let Some(m) = OBS_TIMESTAMP_RE.find(filename) {
        let end = m.end();
        return format!("{} {}{}", &filename[..end], tag, &filename[end..]);
    }

    // ShadowPlay format: "2026.04.15 - 12.30.00"
    if let Some(m) = SP_TIMESTAMP_RE.find(filename) {
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

// --- Destination helpers ---

/// `std::fs::rename` on Windows silently REPLACES an existing destination —
/// for a clip mover that means data loss on a name collision. This returns
/// the path unchanged if free, otherwise appends " (2)", " (3)", … before
/// the extension until a free name is found.
pub fn unique_destination(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    for n in 2u32.. {
        let candidate = parent.join(format!("{} ({}){}", stem, n, ext));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// Rename with retries — OBS/ShadowPlay may still hold the file handle for
/// a moment after the create event fires.
fn rename_with_retry(from: &Path, to: &Path) -> Result<(), String> {
    let delays = [200u64, 500, 1000];
    let mut last_err = String::new();
    for (attempt, &delay_ms) in delays.iter().enumerate() {
        match std::fs::rename(from, to) {
            Ok(_) => return Ok(()),
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
    let new_path = unique_destination(&new_path);

    rename_with_retry(file_path, &new_path)?;
    Ok(MoveResult {
        original_path: file_path.to_path_buf(),
        new_path,
        tag_applied: tag.to_string(),
    })
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
    let new_path = unique_destination(&parent.join(&new_filename));

    rename_with_retry(file_path, &new_path)?;

    Ok(MoveResult {
        original_path: file_path.to_path_buf(),
        new_path,
        tag_applied: text.to_string(),
    })
}

/// Restore a previously moved/renamed file back to where it came from.
/// Used by undo. Collision-safe: if the original name has been re-taken
/// (e.g. OBS saved a new clip with the same timestamp), the restored file
/// gets a " (2)" suffix instead of clobbering it.
pub fn restore_file(from: &Path, to: &Path) -> Result<PathBuf, String> {
    if !from.exists() {
        return Err(format!("File no longer exists: {}", from.display()));
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    let target = unique_destination(to);
    rename_with_retry(from, &target)?;
    Ok(target)
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
    fn test_unique_destination_no_collision_returns_same_path() {
        let dir = tempdir().expect("tempdir");
        let p = dir.path().join("free.mp4");
        assert_eq!(unique_destination(&p), p);
    }

    #[test]
    fn test_unique_destination_appends_counter() {
        let dir = tempdir().expect("tempdir");
        let p = dir.path().join("clip.mp4");
        std::fs::File::create(&p).unwrap();
        assert_eq!(unique_destination(&p), dir.path().join("clip (2).mp4"));

        std::fs::File::create(dir.path().join("clip (2).mp4")).unwrap();
        assert_eq!(unique_destination(&p), dir.path().join("clip (3).mp4"));
    }

    #[test]
    fn test_move_does_not_overwrite_existing() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("Replay 2026-04-15 12-30-00.mp4");
        std::fs::File::create(&file_path).unwrap();

        // A previous clip already sorted to the tagged name.
        let taken = dir.path().join("Replay 2026-04-15 12-30-00 !!.mp4");
        std::fs::write(&taken, b"precious").unwrap();

        let mut config = AppConfig::default();
        config.disable_file_movesorting = true;
        config.g1_bind_folder_name = "!! or ! (G1)".to_string();

        let result = move_or_rename_file(&file_path, 1, &config).expect("move failed");

        assert_eq!(
            result.new_path,
            dir.path().join("Replay 2026-04-15 12-30-00 !! (2).mp4")
        );
        // Original tagged file untouched.
        assert_eq!(std::fs::read(&taken).unwrap(), b"precious");
    }

    #[test]
    fn test_restore_file_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let original = dir.path().join("clip.mp4");
        let moved = dir.path().join("clip !!.mp4");
        std::fs::File::create(&moved).unwrap();

        let restored = restore_file(&moved, &original).expect("restore failed");
        assert_eq!(restored, original);
        assert!(original.exists());
        assert!(!moved.exists());
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
