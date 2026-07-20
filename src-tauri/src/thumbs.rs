//! Explorer-thumbnail extraction for the overlay's clip strip.
//!
//! Uses the Windows Shell's own thumbnail pipeline (`IShellItemImageFactory`)
//! — the same cache Explorer shows — so any video format the system can
//! thumbnail works with zero decoding dependencies. The HBITMAP the shell
//! hands back is converted to an in-memory 32bpp BMP and returned as a
//! `data:` URL the overlay webview can drop straight into an `<img>`.
//!
//! Results are cached per path (clips are never rewritten in place — a
//! rename/sort produces a NEW path, which naturally gets its own entry).
//! Failures are NOT cached: a just-recorded clip often has no shell
//! thumbnail for a second or two, and the next overlay open should retry.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use base64::Engine;

/// Pixel size requested from the shell (16:9). The strip renders smaller;
/// asking a bit big keeps it sharp on high-DPI monitors.
const THUMB_W: i32 = 168;
const THUMB_H: i32 = 94;

/// path → data URL. Bounded: wholesale-cleared past the cap (simple and
/// fine — regenerating a screenful of thumbs is cheap).
static CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const CACHE_MAX: usize = 128;

/// Fetch (or serve from cache) the shell thumbnail for `path` as a
/// `data:image/bmp;base64,...` URL.
#[tauri::command]
pub async fn clip_thumbnail(path: String) -> Result<String, String> {
    if let Some(hit) = CACHE.lock().unwrap().get(&path).cloned() {
        return Ok(hit);
    }
    let p = path.clone();
    let data = tauri::async_runtime::spawn_blocking(move || shell_thumbnail(&p))
        .await
        .map_err(|e| e.to_string())??;
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= CACHE_MAX {
        cache.clear();
    }
    cache.insert(path, data.clone());
    Ok(data)
}

fn shell_thumbnail(path: &str) -> Result<String, String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::DeleteObject;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_RESIZETOFIT,
    };

    unsafe {
        // Shell item factories want an STA; init per-call on this blocking
        // thread (same pattern as props.rs).
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| -> Result<String, String> {
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(&HSTRING::from(path), None)
                    .map_err(|e| format!("open shell item: {e}"))?;
            let hbmp = factory
                .GetImage(
                    SIZE {
                        cx: THUMB_W,
                        cy: THUMB_H,
                    },
                    SIIGBF_RESIZETOFIT,
                )
                .map_err(|e| format!("shell thumbnail: {e}"))?;
            let bmp = hbitmap_to_bmp(hbmp);
            let _ = DeleteObject(hbmp.into());
            let bytes = bmp?;
            Ok(format!(
                "data:image/bmp;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ))
        })();
        if com.is_ok() {
            CoUninitialize();
        }
        result
    }
}

/// Read the HBITMAP's pixels via `GetDIBits` and wrap them in a complete BMP
/// file image (BITMAPFILEHEADER + BITMAPINFOHEADER + bottom-up BGRA rows).
unsafe fn hbitmap_to_bmp(
    hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Result<Vec<u8>, String> {
    use windows::Win32::Graphics::Gdi::{
        GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };

    let mut bm = BITMAP::default();
    if GetObjectW(
        hbmp.into(),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        return Err("GetObject on shell thumbnail failed".to_string());
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if w <= 0 || h <= 0 {
        return Err(format!("shell thumbnail has degenerate size {w}x{h}"));
    }

    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = w;
    info.bmiHeader.biHeight = h; // positive = bottom-up, matching BMP layout
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB.0;

    let mut pixels = vec![0u8; (w as usize) * 4 * (h as usize)];
    let dc = GetDC(None);
    let lines = GetDIBits(
        dc,
        hbmp,
        0,
        h as u32,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut info,
        DIB_RGB_COLORS,
    );
    ReleaseDC(None, dc);
    if lines == 0 {
        return Err("GetDIBits on shell thumbnail failed".to_string());
    }

    // Shell thumbnails come back premultiplied-alpha; a 32bpp BI_RGB BMP's
    // 4th byte is officially padding, but some decoders honor it — force
    // fully opaque so nothing renders ghosted.
    for px in pixels.chunks_exact_mut(4) {
        px[3] = 0xFF;
    }

    let file_size = 14 + 40 + pixels.len();
    let mut out = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    out.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset
    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes()); // ~72 DPI
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // palette colors
    out.extend_from_slice(&0u32.to_le_bytes()); // important colors
    out.extend_from_slice(&pixels);
    Ok(out)
}
