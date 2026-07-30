//! 从 exe / ico / lnk 提取图标为 PNG data URL（带透明通道），供软件体检、启动管理、内存体检等复用。
//! 流程：GDI 抠 HICON → GetDIBits 取像素 → BGRA→RGBA → image 编 PNG → base64 data URL。
//! 结果按路径进程内缓存（同一 exe 图标基本不变），刷新时命中缓存、免重复抠图。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// 标准 base64 编码（自实现，避免额外依赖）。
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity((data.len() + 2) / 3 * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | (*c.get(2).unwrap_or(&0) as u32);
        s.push(T[(n >> 18 & 63) as usize] as char);
        s.push(T[(n >> 12 & 63) as usize] as char);
        s.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        s.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    s
}

/// 用 GDI 从文件（exe/ico/lnk）抠出图标位图并编码为 PNG。失败返回 None。
fn extract_png(path: &str) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut shfi: SHFILEINFOW = std::mem::zeroed();
        let r = SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut shfi,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if r == 0 || shfi.hIcon.is_null() {
            return None;
        }
        let hicon = shfi.hIcon;
        let mut ii: ICONINFO = std::mem::zeroed();
        if GetIconInfo(hicon, &mut ii) == 0 {
            DestroyIcon(hicon);
            return None;
        }
        let mut bm: BITMAP = std::mem::zeroed();
        let ok = GetObjectW(
            ii.hbmColor as _,
            std::mem::size_of::<BITMAP>() as i32,
            &mut bm as *mut _ as *mut _,
        );
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        let mut result = None;
        if ok != 0 && w > 0 && h > 0 && w <= 512 && h <= 512 {
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h; // 负高度 = 自上而下
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = 0; // BI_RGB
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let hdc = GetDC(std::ptr::null_mut::<core::ffi::c_void>() as HWND);
            let got = GetDIBits(hdc, ii.hbmColor, 0, h as u32, buf.as_mut_ptr() as *mut _, &mut bmi, DIB_RGB_COLORS);
            ReleaseDC(std::ptr::null_mut::<core::ffi::c_void>() as HWND, hdc);
            if got != 0 {
                let any_alpha = buf.chunks_exact(4).any(|p| p[3] != 0);
                for p in buf.chunks_exact_mut(4) {
                    p.swap(0, 2); // B<->R
                    if !any_alpha {
                        p[3] = 255;
                    }
                }
                let mut png: Vec<u8> = Vec::new();
                let enc = image::codecs::png::PngEncoder::new(&mut png);
                if enc
                    .write_image(&buf, w as u32, h as u32, image::ExtendedColorType::Rgba8)
                    .is_ok()
                {
                    result = Some(png);
                }
            }
        }
        DeleteObject(ii.hbmColor as _);
        DeleteObject(ii.hbmMask as _);
        DestroyIcon(hicon);
        result
    }
}

/// 提取指定文件（exe/ico/lnk）的图标为 PNG data URL；文件不存在或提取失败返回 None。
/// 结果按路径缓存（含 None）——同一进程内重复刷新直接命中，不再重复抠图。
pub fn from_file(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let key = path.to_lowercase();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock() {
        if let Some(hit) = map.get(&key) {
            return hit.clone();
        }
    }
    let result = if Path::new(path).exists() {
        extract_png(path).map(|png| format!("data:image/png;base64,{}", base64(&png)))
    } else {
        None
    };
    if let Ok(mut map) = cache.lock() {
        map.insert(key, result.clone());
    }
    result
}

/// 图标缓存：路径(小写) → data URL 或 None。
static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_standard_vectors() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
    }
}
