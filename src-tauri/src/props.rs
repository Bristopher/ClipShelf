//! Best-effort Windows property mirror (Explorer Tags / Rating / Comments).
//! history.jsonl is the source of truth; these writes exist so other apps
//! and Explorer can see game/rating/description on the file itself.
//! HARD RULE: never touch a file something else still holds — probe with
//! exclusive share access first, retry, then skip with a warning.
//! Contract: Docs/Features/Clip-Metadata-Interop.md.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// One property to mirror onto the file.
pub enum PropValue {
    Game(String),
    // consumed in Phase 2/3 (rating mirror)
    #[allow(dead_code)]
    Stars(u8),
    // consumed in Phase 2/3 (description mirror)
    #[allow(dead_code)]
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
/// "is OBS done writing?" check. Missing file -> false.
pub fn probe_exclusive(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
        .is_ok()
}

pub const PROBE_ATTEMPTS: u32 = 5;
pub const PROBE_DELAY_MS: u64 = 1700; // same cadence as the mover's move retries

/// Probe-then-write with retries, RE-RESOLVING the target path before every
/// probe. BLOCKING (sleeps up to `attempts * delay`) -- only call from the
/// blocking pool. Err(msg) is for a warning log, never a toast.
///
/// The path is re-fetched via `get_path` each attempt so the write follows a
/// clip that gets moved/renamed (e.g. a G-key sort) while OBS still holds the
/// original file: without this, every retry would probe the now-moved
/// creation path and the write would land on nothing. `attempts`/`delay` are
/// parameters purely so tests can drive tiny counts; production passes
/// `PROBE_ATTEMPTS` / `PROBE_DELAY_MS`.
pub fn write_with_retry_resolving(
    get_path: impl Fn() -> Option<PathBuf>,
    values: &[PropValue],
    attempts: u32,
    delay: Duration,
) -> Result<(), String> {
    let mut attempt = 0;
    loop {
        let path = get_path()
            .ok_or_else(|| "clip path no longer resolvable — skipped property write".to_string())?;
        if probe_exclusive(&path) {
            return write_properties(&path, values);
        }
        attempt += 1;
        if attempt >= attempts {
            return Err(format!(
                "file still locked after {} attempts — skipped property write (history.jsonl has the data)",
                attempts
            ));
        }
        std::thread::sleep(delay);
    }
}

/// Build a plain `VT_LPWSTR` PROPVARIANT from a single string (System.Comment).
/// Contrast with `InitPropVariantFromStringAsVector`, which produces a
/// `VT_VECTOR|VT_LPWSTR` PROPVARIANT (used for System.Keywords). The pinned
/// `windows` crate version doesn't expose a single-string init helper, so
/// this is built by hand: allocate a CoTaskMem'd wide string (the property
/// store / `PropVariantClear` expect COM-owned memory) and set `pwszVal`.
///
/// SAFETY: caller owns the returned PROPVARIANT and must eventually pass it
/// to `PropVariantClear` (or otherwise free `pwszVal` via `CoTaskMemFree`) —
/// this function does not do that itself.
unsafe fn propvariant_from_string(
    text: &str,
) -> windows::core::Result<windows::Win32::System::Com::StructuredStorage::PROPVARIANT> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::E_OUTOFMEMORY;
    use windows::Win32::System::Com::CoTaskMemAlloc;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Variant::VT_LPWSTR;

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = std::mem::size_of_val(wide.as_slice());
    let buf = CoTaskMemAlloc(byte_len) as *mut u16;
    if buf.is_null() {
        return Err(windows::core::Error::from(E_OUTOFMEMORY));
    }
    std::ptr::copy_nonoverlapping(wide.as_ptr(), buf, wide.len());

    let mut pv = PROPVARIANT::default();
    (*pv.Anonymous.Anonymous).vt = VT_LPWSTR;
    (*pv.Anonymous.Anonymous).Anonymous.pwszVal = PWSTR(buf);
    Ok(pv)
}

/// Build a `VT_UI4` PROPVARIANT from a plain u32 (System.Rating). No helper
/// for scalar ints exists in the pinned crate version's StructuredStorage
/// module (only vector variants), so this is a trivial manual union fill —
/// no heap allocation, so no cleanup obligation beyond the usual
/// `PropVariantClear`.
fn propvariant_from_u32(
    value: u32,
) -> windows::Win32::System::Com::StructuredStorage::PROPVARIANT {
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Variant::VT_UI4;

    let mut pv = PROPVARIANT::default();
    unsafe {
        (*pv.Anonymous.Anonymous).vt = VT_UI4;
        (*pv.Anonymous.Anonymous).Anonymous.ulVal = value;
    }
    pv
}

fn write_properties(path: &Path, values: &[PropValue]) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::StructuredStorage::{
        InitPropVariantFromStringAsVector, PropVariantClear,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IPropertyStore, PSGetPropertyKeyFromName, SHGetPropertyStoreFromParsingName,
        GPS_READWRITE,
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
                // Resolve the property key BEFORE constructing the
                // PROPVARIANT: if resolution fails we return while no
                // allocation exists yet, so a `?` here can't leak the
                // CoTaskMem'd string buffer (the Clear below only runs
                // after SetValue).
                let key_name = match v {
                    PropValue::Game(_) => "System.Keywords",
                    PropValue::Stars(_) => "System.Rating",
                    PropValue::Description(_) => "System.Comment",
                };
                let mut key = windows::Win32::Foundation::PROPERTYKEY::default();
                PSGetPropertyKeyFromName(&HSTRING::from(key_name), &mut key)
                    .map_err(|e| format!("resolve {key_name}: {e}"))?;

                let mut var = match v {
                    PropValue::Game(name) => {
                        InitPropVariantFromStringAsVector(&HSTRING::from(name.as_str()))
                            .map_err(|e| format!("keywords propvariant: {e}"))?
                    }
                    PropValue::Stars(stars) => {
                        propvariant_from_u32(stars_to_system_rating(*stars))
                    }
                    PropValue::Description(text) => {
                        // Single VT_LPWSTR string — NOT a vector.
                        propvariant_from_string(text)
                            .map_err(|e| format!("comment propvariant: {e}"))?
                    }
                };
                let set_result = store
                    .SetValue(&key, &var)
                    .map_err(|e| format!("set {key_name}: {e}"));
                // SetValue copies the value internally; we still own `var`
                // and must release it (frees the CoTaskMem'd string buffer
                // for Keywords/Comment; a no-op for the scalar Rating).
                let _ = PropVariantClear(&mut var);
                set_result?;
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

    // The fast-sort race: a clip is created, OBS still holds the file, and the
    // user G-key-sorts it during the retry window. The resolving helper must
    // re-fetch the path each attempt and follow the moved clip instead of
    // burning all its retries probing the now-locked creation path.
    #[test]
    fn test_write_with_retry_resolving_follows_moved_path() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = tempfile::tempdir().unwrap();
        let creation = dir.path().join("creation.mp4");
        let moved = dir.path().join("moved.mp4");
        std::fs::write(&creation, b"stub").unwrap();
        std::fs::write(&moved, b"stub").unwrap();

        // Hold the creation path with NO sharing (OBS mid-write) so its probe
        // always fails; the moved path is free and probes exclusive.
        use std::os::windows::fs::OpenOptionsExt;
        let _hold = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&creation)
            .unwrap();

        let calls = AtomicUsize::new(0);
        let cp = creation.clone();
        let mp = moved.clone();
        let resolve = || {
            // First two attempts still see the locked creation path; then the
            // clip is "sorted" and the closure returns the freed moved path.
            let n = calls.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                Some(cp.clone())
            } else {
                Some(mp.clone())
            }
        };

        let res = write_with_retry_resolving(
            resolve,
            &[PropValue::Game("Test".into())],
            6,
            Duration::from_millis(1),
        );

        // Must have re-resolved past the locked creation path (>=3 fetches).
        assert!(
            calls.load(Ordering::SeqCst) >= 3,
            "helper must re-resolve the path on every attempt"
        );
        // It must NOT have exhausted its retries on the locked creation path —
        // it followed the moved clip. (write_properties itself may still Err on
        // a stub file that has no real property handler; that's fine, we only
        // assert the resolving loop reached the moved path.)
        if let Err(e) = &res {
            assert!(
                !e.contains("still locked"),
                "should have followed the moved path, got: {e}"
            );
        }
    }
}
