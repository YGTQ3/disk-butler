//! 应用图标提取：优先用 DisplayIcon(路径,索引)，其次 MSI 产品图标，再退回安装目录/卸载器主 exe
//! （Geek Uninstaller / BCUninstaller 同款兑底思路）。统一经 GDI 转 32bpp RGBA，编码 PNG 后返回 base64 data URI。

use base64::Engine;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

/// 尝试从一个图标文件（exe/dll/ico，支持索引）提取：
/// 先 ExtractIconEx（PE/DLL），再 LoadImage 按文件加载 ICO（治后缀骗人的原始 .ico，如 MSI 的 ResolveIcon.exe），最后退 Shell 图标。
fn try_icon_file(path: &str, idx: i32) -> Option<(u32, u32, Vec<u8>)> {
    let p = expand_env(path);
    if p.is_empty() || !Path::new(&p).exists() {
        return None;
    }
    // 首选 PrivateExtractIcons：按固定尺寸(64)栅格化，对现代/大尺寸/PNG压缩图标最稳（如微信 Weixin.exe）
    if let Some(r) = extract_via_private(&p, idx, 64) {
        return Some(r);
    }
    if let Some(r) = extract_icon_rgba(&p, idx) {
        return Some(r);
    }
    if let Some(r) = load_ico_file(&p) {
        return Some(r);
    }
    shell_icon_rgba(&p)
}

/// 用 LoadImageW 按文件加载 ICO（LR_LOADFROMFILE），不看扩展名按内容解析，
/// 专治那些后缀名不对的原始 .ico（MSI 产品图标常见：*.exe / 无扩展名）。
fn load_ico_file(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, LoadImageW};
    const IMAGE_ICON: u32 = 1;
    const LR_LOADFROMFILE: u32 = 0x0000_0010;
    const LR_DEFAULTSIZE: u32 = 0x0000_0040;
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let h = LoadImageW(
            std::ptr::null_mut(),
            wide.as_ptr(),
            IMAGE_ICON,
            0,
            0,
            LR_LOADFROMFILE | LR_DEFAULTSIZE,
        );
        if h.is_null() {
            return None;
        }
        let out = hicon_to_rgba(h as _);
        DestroyIcon(h as _);
        out
    }
}

/// 按 ProductName 在 MSI 产品表里查 ProductIcon（很多 MSI 软件 DisplayIcon/InstallLocation 为空，
/// 图标只存于 HKLM\SOFTWARE\Classes\Installer\Products\<PackedGUID>\ProductIcon，指向 C:\Windows\Installer\{GUID}\*.ico|exe）。
fn msi_product_icon(display_name: &str) -> Option<(String, i32)> {
    let target = display_name.trim().to_lowercase();
    if target.is_empty() {
        return None;
    }
    let sources = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Classes\Installer\Products"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Installer\Products"),
    ];
    for (hive, path) in sources {
        let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for sub in root.enum_keys().flatten() {
            let Ok(k) = root.open_subkey_with_flags(&sub, KEY_READ) else {
                continue;
            };
            let pn: String = k.get_value("ProductName").unwrap_or_default();
            if pn.trim().to_lowercase() != target {
                continue;
            }
            let icon: String = k.get_value("ProductIcon").unwrap_or_default();
            if icon.trim().is_empty() {
                continue;
            }
            return parse_display_icon(&icon);
        }
    }
    None
}

/// 解析 DisplayIcon 并按兑底链取图标，编码为 `data:image/png;base64,...`。失败返回 None。
pub fn icon_data_uri(
    display_icon: &str,
    install_location: &str,
    uninstall_string: &str,
    display_name: &str,
) -> Option<String> {
    // UWP/直接图片：DisplayIcon 指向 png/jpg 时直接读文件返回（UWP 包内 logo 为 png，不走 GDI 图标管线）
    if let Some((p, _)) = parse_display_icon(display_icon) {
        let pe = expand_env(&p);
        let low = pe.to_lowercase();
        if (low.ends_with(".png") || low.ends_with(".jpg") || low.ends_with(".jpeg"))
            && Path::new(&pe).exists()
        {
            if let Ok(bytes) = std::fs::read(&pe) {
                let mime = if low.ends_with(".png") { "image/png" } else { "image/jpeg" };
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Some(format!("data:{};base64,{}", mime, b64));
            }
        }
    }
    let rgba = resolve_icon(display_icon, install_location, uninstall_string, display_name)?;
    let png: Vec<u8> = encode_png(rgba.0, rgba.1, &rgba.2)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{}", b64))
}

/// 展开路径中的 %ENV% 环境变量（DisplayIcon 偶尔写成 %ProgramFiles%\...）。
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        if let Some(end_rel) = rest[start + 1..].find('%') {
            let var = &rest[start + 1..start + 1 + end_rel];
            match std::env::var(var) {
                Ok(val) => out.push_str(&val),
                Err(_) => {
                    out.push('%');
                    out.push_str(var);
                    out.push('%');
                }
            }
            rest = &rest[start + 1 + end_rel + 1..];
        } else {
            out.push_str(&rest[start..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// 从 UninstallString 里抽出卸载器 exe 路径（取首个 .exe token，支持引号包裹）。
fn uninstaller_exe(uninstall_string: &str) -> Option<String> {
    let s = expand_env(uninstall_string.trim());
    if s.is_empty() {
        return None;
    }
    // 引号包裹：取首对引号内容
    if let Some(rest) = s.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    // 无引号：取到 ".exe" 为止（大小写不敏感）
    let lower = s.to_lowercase();
    if let Some(pos) = lower.find(".exe") {
        return Some(s[..pos + 4].to_string());
    }
    None
}

/// 兑底链（按优先级依次尝试，命中即返，避免不必要的注册表枚举）：
/// DisplayIcon 及其同目录主 exe → MSI 产品图标 → 安装目录主 exe → 卸载器目录主 exe / 卸载器本体。
fn resolve_icon(
    display_icon: &str,
    install_location: &str,
    uninstall_string: &str,
    display_name: &str,
) -> Option<(u32, u32, Vec<u8>)> {
    // 1) DisplayIcon 本体 + 其所在目录的主 exe
    if let Some((path, index)) = parse_display_icon(display_icon) {
        let path = expand_env(&path);
        if let Some(r) = try_icon_file(&path, index) {
            return Some(r);
        }
        if let Some(dir) = Path::new(&path).parent() {
            if let Some(exe) = main_exe_in(&dir.to_string_lossy()) {
                if let Some(r) = try_icon_file(&exe, 0) {
                    return Some(r);
                }
            }
        }
    }
    // 2) MSI 产品图标（多数 DisplayIcon 为空的 MSI 软件靠这一步）
    if let Some((p, idx)) = msi_product_icon(display_name) {
        if let Some(r) = try_icon_file(&p, idx) {
            return Some(r);
        }
    }
    // 3) 安装目录主 exe
    let inst = expand_env(install_location.trim());
    if !inst.is_empty() {
        if let Some(exe) = main_exe_in(&inst) {
            if let Some(r) = try_icon_file(&exe, 0) {
                return Some(r);
            }
        }
    }
    // 4) 卸载器所在目录主 exe，以及卸载器本体（很多软件只有 UninstallString）
    if let Some(u) = uninstaller_exe(uninstall_string) {
        if let Some(dir) = Path::new(&u).parent() {
            if let Some(exe) = main_exe_in(&dir.to_string_lossy()) {
                if let Some(r) = try_icon_file(&exe, 0) {
                    return Some(r);
                }
            }
        }
        if let Some(r) = try_icon_file(&u, 0) {
            return Some(r);
        }
    }
    None
}

/// 解析 `"C:\App\a.exe",0` → (路径, 索引)。无逗号索引则为 0。
fn parse_display_icon(raw: &str) -> Option<(String, i32)> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // 从右找逗号，且逗号后是整数才当索引（避免误伤路径里的逗号）
    if let Some(i) = s.rfind(',') {
        let tail = s[i + 1..].trim();
        if let Ok(idx) = tail.parse::<i32>() {
            let path = s[..i].trim().trim_matches('"').to_string();
            return Some((path, idx));
        }
    }
    Some((s.trim_matches('"').to_string(), 0))
}

/// 共享/系统目录（C:\Windows、System32、Program Files 根、盘符根等）：不得在其中“猜主程序”，
/// 否则会把 explorer.exe 等系统程序的图标误当成软件图标。
fn is_shared_root(dir: &str) -> bool {
    let d = dir.trim().trim_end_matches(['\\', '/']).to_lowercase();
    if d.is_empty() {
        return true;
    }
    // 盘符根，如 c: 或 c:\
    if d.len() <= 3 && d.as_bytes().get(1) == Some(&b':') {
        return true;
    }
    let norm_var = |var: &str| {
        std::env::var_os(var)
            .map(|v| v.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase())
            .unwrap_or_default()
    };
    for var in ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData"] {
        let r = norm_var(var);
        if !r.is_empty() && d == r {
            return true;
        }
    }
    let sr = norm_var("SystemRoot");
    if !sr.is_empty() && (d == format!("{}\\system32", sr) || d == format!("{}\\syswow64", sr)) {
        return true;
    }
    false
}

/// 在安装目录里挑主程序 exe：扫目录本身、bin 子目录、以及每个一级子目录
/// （很多软件把主 exe 嵌在子目录，如 Adobe：InstallLocation\Acrobat\Acrobat.exe）。
/// 优先名字与目录名相关的 exe，其次体积最大。
fn main_exe_in(dir: &str) -> Option<String> {
    if is_shared_root(dir) {
        return None;
    }
    let base = Path::new(dir);
    let folder = base
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let first_word = folder.split_whitespace().next().unwrap_or("").to_string();

    // 待扫目录：自身 + bin + 一级子目录（限量，避免病态大目录）
    let mut dirs: Vec<PathBuf> = vec![base.to_path_buf(), base.join("bin")];
    if let Ok(rd) = std::fs::read_dir(base) {
        for e in rd.flatten().take(80) {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(e.path());
            }
        }
    }

    let mut best: Option<(u64, PathBuf)> = None;
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let is_exe = p
                .extension()
                .map(|x| x.eq_ignore_ascii_case("exe"))
                .unwrap_or(false);
            if !is_exe {
                continue;
            }
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            // 跳过明显的辅助程序
            if stem.contains("unins")
                || stem.contains("update")
                || stem.contains("crash")
                || stem.contains("setup")
                || stem.contains("helper")
            {
                continue;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            // 名字与目录相关的 exe 强烈优先（加一个很大的基数）
            let name_match = (!first_word.is_empty()
                && (stem.contains(&first_word) || first_word.contains(&stem)))
                || folder.contains(&stem);
            let score = if name_match { size + (1 << 40) } else { size };
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, p));
            }
        }
    }
    best.map(|(_, p)| p.to_string_lossy().to_string())
}

/// user32 导出的 PrivateExtractIconsW（windows-sys 未在 Shell 模块暴露，自行声明）。
#[link(name = "user32")]
extern "system" {
    fn PrivateExtractIconsW(
        szfilename: *const u16,
        niconindex: i32,
        cxicon: i32,
        cyicon: i32,
        phicon: *mut *mut core::ffi::c_void,
        piconid: *mut u32,
        nicons: u32,
        flags: u32,
    ) -> u32;
}

/// 用 PrivateExtractIconsW 按指定像素尺寸栅格化取图标（对现代/PNG压缩/大尺寸图标最可靠）。
fn extract_via_private(path: &str, index: i32, size: i32) -> Option<(u32, u32, Vec<u8>)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let idx = if index < 0 { 0 } else { index };
    unsafe {
        let mut hicon: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut id: u32 = 0;
        let n = PrivateExtractIconsW(wide.as_ptr(), idx, size, size, &mut hicon, &mut id, 1, 0);
        if n == 0 || n == u32::MAX || hicon.is_null() {
            return None;
        }
        let out = hicon_to_rgba(hicon as _);
        DestroyIcon(hicon as _);
        out
    }
}

/// 用 ExtractIconExW 按索引取图标。
fn extract_icon_rgba(path: &str, index: i32) -> Option<(u32, u32, Vec<u8>)> {
    use windows_sys::Win32::UI::Shell::ExtractIconExW;
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    // 负索引在 DisplayIcon 里表示资源 ID，ExtractIconEx 不支持，退回 0
    let idx = if index < 0 { 0 } else { index };
    unsafe {
        let mut large = std::ptr::null_mut();
        let mut small = std::ptr::null_mut();
        let n = ExtractIconExW(wide.as_ptr(), idx, &mut large, &mut small, 1);
        if n == 0 || n == u32::MAX {
            return None;
        }
        let hicon = if !large.is_null() { large } else { small };
        let out = if hicon.is_null() {
            None
        } else {
            hicon_to_rgba(hicon)
        };
        if !large.is_null() {
            DestroyIcon(large);
        }
        if !small.is_null() {
            DestroyIcon(small);
        }
        out
    }
}

/// 用 SHGetFileInfoW 取 Shell 图标（处理 .ico / 默认关联 / exe 首图标等）。
fn shell_icon_rgba(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;

    const SHGFI_ICON: u32 = 0x0000_0100;
    const SHGFI_LARGEICON: u32 = 0x0000_0000;

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut fi: SHFILEINFOW = std::mem::zeroed();
        let ok = SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut fi,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ok == 0 || fi.hIcon.is_null() {
            return None;
        }
        let out = hicon_to_rgba(fi.hIcon);
        DestroyIcon(fi.hIcon);
        out
    }
}

/// HICON → (宽, 高, RGBA)。调用方负责销毁 HICON。
unsafe fn hicon_to_rgba(
    hicon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<(u32, u32, Vec<u8>)> {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut ii: ICONINFO = std::mem::zeroed();
    if GetIconInfo(hicon, &mut ii) == 0 {
        return None;
    }

    let mut bmp: BITMAP = std::mem::zeroed();
    GetObjectW(
        ii.hbmColor as _,
        std::mem::size_of::<BITMAP>() as i32,
        &mut bmp as *mut _ as *mut _,
    );
    let w = bmp.bmWidth.max(0) as u32;
    let h = bmp.bmHeight.max(0) as u32;
    let hdc = CreateCompatibleDC(std::ptr::null_mut());

    let result = if w == 0 || h == 0 {
        None
    } else {
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB as u32,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let got = GetDIBits(
            hdc,
            ii.hbmColor as _,
            0,
            h,
            buf.as_mut_ptr() as *mut _,
            &mut bi,
            DIB_RGB_COLORS,
        );
        if got == 0 {
            None
        } else {
            let mut any_alpha = false;
            for px in buf.chunks_exact_mut(4) {
                px.swap(0, 2); // BGRA -> RGBA
                if px[3] != 0 {
                    any_alpha = true;
                }
            }
            if !any_alpha {
                apply_mask_alpha(hdc, ii.hbmMask as _, w, h, &mut buf);
            }
            Some((w, h, buf))
        }
    };

    if !hdc.is_null() {
        DeleteDC(hdc);
    }
    if !ii.hbmColor.is_null() {
        DeleteObject(ii.hbmColor as _);
    }
    if !ii.hbmMask.is_null() {
        DeleteObject(ii.hbmMask as _);
    }
    result
}

/// 从 1bpp 掩码位图导出 alpha：掩码位为 1 处透明、为 0 处不透明。
unsafe fn apply_mask_alpha(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    hmask: windows_sys::Win32::Graphics::Gdi::HBITMAP,
    w: u32,
    h: u32,
    rgba: &mut [u8],
) {
    use windows_sys::Win32::Graphics::Gdi::{
        GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    let row_bytes = (((w + 31) / 32) * 4) as usize;
    let mut mask = vec![0u8; row_bytes * h as usize];
    let mut bi: BITMAPINFO = std::mem::zeroed();
    bi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w as i32,
        biHeight: -(h as i32),
        biPlanes: 1,
        biBitCount: 1,
        biCompression: BI_RGB as u32,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    let got = GetDIBits(
        hdc,
        hmask,
        0,
        h,
        mask.as_mut_ptr() as *mut _,
        &mut bi,
        DIB_RGB_COLORS,
    );
    if got == 0 {
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        return;
    }
    for y in 0..h as usize {
        for x in 0..w as usize {
            let bit = (mask[y * row_bytes + (x / 8)] >> (7 - (x % 8))) & 1;
            rgba[(y * w as usize + x) * 4 + 3] = if bit == 0 { 255 } else { 0 };
        }
    }
}

/// RGBA 像素编码为 PNG 字节。
fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut out), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_display_icon_handles_index_and_quotes() {
        assert_eq!(
            parse_display_icon("\"C:\\App\\a.exe\",0"),
            Some(("C:\\App\\a.exe".to_string(), 0))
        );
        assert_eq!(
            parse_display_icon("C:\\App\\a.exe,3"),
            Some(("C:\\App\\a.exe".to_string(), 3))
        );
        assert_eq!(
            parse_display_icon("\"C:\\App\\a.ico\""),
            Some(("C:\\App\\a.ico".to_string(), 0))
        );
        assert_eq!(parse_display_icon("  "), None);
    }
}
