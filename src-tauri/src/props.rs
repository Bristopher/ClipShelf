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

const PROBE_ATTEMPTS: u32 = 5;
const PROBE_DELAY_MS: u64 = 1700; // same cadence as the mover's move retries

/// Probe-then-write with retries. BLOCKING (sleeps up to ~8.5 s) -- only
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
}
