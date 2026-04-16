use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::thread;

/// Play a sound file asynchronously (fire and forget on a new thread).
pub fn play_sound(path: &Path) {
    let path = path.to_path_buf();
    thread::spawn(move || {
        if let Err(e) = play_sound_blocking(&path) {
            log::warn!("Failed to play sound {}: {}", path.display(), e);
        }
    });
}

fn play_sound_blocking(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let file = File::open(path)?;
    let source = Decoder::new(BufReader::new(file))?;
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}

/// Resolve a sound path: if custom path is set and exists, use it. Otherwise use the bundled default.
pub fn resolve_sound_path(
    custom: &Option<String>,
    default_filename: &str,
    resource_dir: &Path,
) -> PathBuf {
    if let Some(ref custom_path) = custom {
        let p = PathBuf::from(custom_path);
        if !custom_path.is_empty() && p.exists() {
            return p;
        }
    }
    resource_dir.join("sounds").join(default_filename)
}

/// Play the clip-saved notification sound
pub fn play_clip_saved(custom: &Option<String>, resource_dir: &Path) {
    let path = resolve_sound_path(custom, "CrimewaveTone.wav", resource_dir);
    play_sound(&path);
}

/// Play the error sound (no current file / black screen warning)
pub fn play_error(custom: &Option<String>, resource_dir: &Path) {
    let path = resolve_sound_path(custom, "Microsoft Windows XP Error.mp3", resource_dir);
    play_sound(&path);
}

/// Play the move notification beep
pub fn play_move_beep(resource_dir: &Path) {
    let path = resource_dir.join("sounds").join("audiocheck.net_sin_523Hz_-21dBFS_.15s.wav");
    play_sound(&path);
}

/// Play the low error beep
pub fn play_error_beep(resource_dir: &Path) {
    let path = resource_dir.join("sounds").join("audiocheck.net_sin_150Hz_-21dBFS_.75s.wav");
    play_sound(&path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_sound_path_with_nonexistent_custom() {
        let resource_dir = Path::new("resources");
        let result = resolve_sound_path(&Some("/nonexistent/sound.wav".to_string()), "default.wav", resource_dir);
        assert_eq!(result, resource_dir.join("sounds").join("default.wav"));
    }

    #[test]
    fn test_resolve_sound_path_no_custom() {
        let resource_dir = Path::new("resources");
        let result = resolve_sound_path(&None, "default.wav", resource_dir);
        assert_eq!(result, resource_dir.join("sounds").join("default.wav"));
    }

    #[test]
    fn test_resolve_sound_path_empty_custom() {
        let resource_dir = Path::new("resources");
        let result = resolve_sound_path(&Some(String::new()), "default.wav", resource_dir);
        assert_eq!(result, resource_dir.join("sounds").join("default.wav"));
    }
}
