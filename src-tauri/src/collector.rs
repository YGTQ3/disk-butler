// 规则采集（主程序内置版）：与 tools/collector 的 DiskButlerCollector.exe 逻辑与报告格式完全一致。
// 只采集目录名+大小+缓存模式命中+软件清单，永不采集文件名/文件内容/用户名（路径脱敏）。
// 零网络、不删除不修改任何东西。报告写到桌面，由用户自主决定是否分享。

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;
use windows_sys::Win32::Foundation::SYSTEMTIME;
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, SHGetKnownFolderPath};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

const MB: f64 = 1024.0 * 1024.0;
const GB: f64 = 1024.0 * 1024.0 * 1024.0;
/// GetDriveTypeW 返回值：本地固定磁盘（winbase.h DRIVE_FIXED）
const DRIVE_FIXED: u32 = 3;

const CACHE_PATTERNS: &[&str] = &[
    "cache", "code cache", "gpucache", "cacheddata", "cachestorage", "shadercache",
    "dawncache", "crashpad", "blob_storage", "service worker", "logs", "log", "temp",
    "tmp", "crashdumps", "dumps", "pending", "staging",
];
const PERSONAL_PATTERNS: &[&str] = &[
    "download", "document", "desktop", "picture", "photo", "video", "music",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectResult {
    pub md_path: String,
    pub json_path: String,
    pub software_count: usize,
    pub dir_rows: usize,
    pub drive_rows: usize,
    pub elapsed_secs: u64,
}

struct TopDir {
    name: String,
    size_mb: f64,
    cache_hits: String,
    flags: String,
}

struct DriveRow {
    drive: String,
    depth: u32,
    path: String,
    size_gb: f64,
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn dir_size(path: &Path) -> u64 {
    jwalk::WalkDir::new(path)
        .skip_hidden(false)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

fn replace_ci(hay: &str, needle: &str, repl: &str) -> String {
    if needle.is_empty() {
        return hay.to_string();
    }
    let hl = hay.to_lowercase();
    let nl = needle.to_lowercase();
    if hl.len() != hay.len() {
        return hay.replace(needle, repl);
    }
    let mut out = String::new();
    let mut i = 0;
    while let Some(pos) = hl[i..].find(&nl) {
        let abs = i + pos;
        out.push_str(&hay[i..abs]);
        out.push_str(repl);
        i = abs + needle.len();
    }
    out.push_str(&hay[i..]);
    out
}

fn mask(path: &str, profile: &str, user: &str) -> String {
    let s = replace_ci(path, profile, "%USERPROFILE%");
    if user.is_empty() {
        s
    } else {
        replace_ci(&s, user, "%USER%")
    }
}

/// 版本号命名的目录（如 12.1.0.26895 / app-1.2.3）——升级残留探测的基本单元
fn is_version_dir_name(name: &str) -> bool {
    let n = name.strip_prefix("app-").unwrap_or(name);
    !n.is_empty()
        && n.split('.').count() >= 2
        && n.split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// 顶层目录自身或其一级子目录下，是否存在 ≥2 个版本号命名的兄弟目录。
/// 命中说明该软件采用「多版本并存」布局（如 WPS/Squirrel 系），旧版本极可能是升级残留，
/// 打 VERSION-SIBLINGS 标记供规则评估时人工确认（探测只报线索，不做清理判断）。
fn has_version_siblings(top: &Path) -> bool {
    fn version_dirs_at(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| {
                        e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                            && is_version_dir_name(&e.file_name().to_string_lossy())
                    })
                    .count()
            })
            .unwrap_or(0)
    }
    if version_dirs_at(top) >= 2 {
        return true;
    }
    if let Ok(rd) = std::fs::read_dir(top) {
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) && version_dirs_at(&e.path()) >= 2
            {
                return true;
            }
        }
    }
    false
}

/// 顶层目录下两级子目录中缓存类命名（先一级后二级，与独立版一致）
fn collect_cache_hits(top: &Path, hits: &mut Vec<String>) {
    let is_hit = |lname: &str| -> bool {
        CACHE_PATTERNS.iter().any(|c| lname == *c) || lname.contains("updater")
    };
    let mut children: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(top) {
        for e in rd.flatten() {
            if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if is_hit(&name.to_lowercase()) && !hits.contains(&name) {
                hits.push(name.clone());
            }
            children.push(e.path());
        }
    }
    for c in children {
        let Ok(rd) = std::fs::read_dir(&c) else { continue };
        for e in rd.flatten() {
            if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if is_hit(&name.to_lowercase()) {
                let rel = e
                    .path()
                    .strip_prefix(top)
                    .map(|r| r.to_string_lossy().to_string())
                    .unwrap_or(name);
                if !hits.contains(&rel) {
                    hits.push(rel);
                }
            }
        }
    }
}

fn analyze_root(root: &str, min_mb: f64) -> Vec<TopDir> {
    let mut rows: Vec<TopDir> = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return rows };
    let dirs: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    for d in &dirs {
        let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
        let size_mb = round1(dir_size(d) as f64 / MB);
        if size_mb < min_mb {
            continue;
        }
        let mut hits: Vec<String> = Vec::new();
        collect_cache_hits(d, &mut hits);
        let cache_hits = hits.iter().take(8).cloned().collect::<Vec<_>>().join("; ");
        let lname = name.to_lowercase();
        let mut flags: Vec<&str> = Vec::new();
        if PERSONAL_PATTERNS.iter().any(|p| lname.contains(p)) {
            flags.push("PERSONAL-DO-NOT-ADD");
        }
        if lname.contains("updater") {
            flags.push("UPDATER-RESIDUE");
        }
        if has_version_siblings(d) {
            flags.push("VERSION-SIBLINGS");
        }
        rows.push(TopDir {
            name,
            size_mb,
            cache_hits,
            flags: flags.join("; "),
        });
    }
    rows.sort_by(|a, b| b.size_mb.partial_cmp(&a.size_mb).unwrap_or(std::cmp::Ordering::Equal));
    rows
}

fn installed_software() -> Vec<(String, Option<f64>)> {
    let mut list: Vec<(String, Option<f64>)> = Vec::new();
    let paths: [(winreg::HKEY, &str); 3] = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    for (hive, path) in paths {
        let Ok(key) = RegKey::predef(hive).open_subkey(path) else { continue };
        for sub in key.enum_keys().flatten() {
            let Ok(k) = key.open_subkey(&sub) else { continue };
            let Ok(name) = k.get_value::<String, _>("DisplayName") else { continue };
            if name.trim().is_empty() {
                continue;
            }
            let size_mb = k
                .get_value::<u32, _>("EstimatedSize")
                .ok()
                .map(|kb| round1(kb as f64 / 1024.0));
            list.push((name, size_mb));
        }
    }
    list.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    list.dedup_by(|a, b| a.0.to_lowercase() == b.0.to_lowercase());
    list
}

fn fixed_drives() -> Vec<String> {
    let mask = unsafe { GetLogicalDrives() };
    let mut v = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root: Vec<u16> = format!("{}:\\", letter)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        if unsafe { GetDriveTypeW(root.as_ptr()) } == DRIVE_FIXED {
            v.push(format!("{}:", letter));
        }
    }
    v
}

fn disk_space(drive: &str) -> (f64, f64) {
    let root: Vec<u16> = format!("{}\\", drive)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut avail = 0u64;
    let mut total = 0u64;
    let mut free = 0u64;
    unsafe {
        GetDiskFreeSpaceExW(root.as_ptr(), &mut avail, &mut total, &mut free);
    }
    (round1(total as f64 / GB), round1(free as f64 / GB))
}

fn scan_drives(profile: &str, user: &str) -> Vec<DriveRow> {
    let mut rows = Vec::new();
    for dv in fixed_drives() {
        let root = format!("{}\\", dv);
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        let dirs: Vec<PathBuf> = rd
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect();
        for d in &dirs {
            let sz = dir_size(d);
            if (sz as f64) < GB {
                continue;
            }
            rows.push(DriveRow {
                drive: dv.clone(),
                depth: 1,
                path: mask(&d.to_string_lossy(), profile, user),
                size_gb: round2(sz as f64 / GB),
            });
            if let Ok(rd2) = std::fs::read_dir(d) {
                for e in rd2.flatten() {
                    if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    let ssz = dir_size(&e.path());
                    if (ssz as f64) >= GB {
                        rows.push(DriveRow {
                            drive: dv.clone(),
                            depth: 2,
                            path: mask(&e.path().to_string_lossy(), profile, user),
                            size_gb: round2(ssz as f64 / GB),
                        });
                    }
                }
            }
        }
    }
    rows
}

fn os_version() -> String {
    if let Ok(k) =
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
    {
        let product: String = k.get_value("ProductName").unwrap_or_else(|_| "Windows".into());
        let disp: String = k.get_value("DisplayVersion").unwrap_or_default();
        let major: u32 = k.get_value("CurrentMajorVersionNumber").unwrap_or(10);
        let minor: u32 = k.get_value("CurrentMinorVersionNumber").unwrap_or(0);
        let build: String = k.get_value("CurrentBuildNumber").unwrap_or_default();
        // 注册表 ProductName 在 Win11 上仍写 "Windows 10"（微软已知问题），按 build 号修正
        let product = if build.parse::<u32>().unwrap_or(0) >= 22000 {
            product.replace("Windows 10", "Windows 11")
        } else {
            product
        };
        return format!("Microsoft {} {} {}.{}.{}", product, disp, major, minor, build);
    }
    "Windows".into()
}

fn local_time() -> (String, String) {
    let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut st) };
    (
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute
        ),
        format!(
            "{:04}{:02}{:02}-{:02}{:02}",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute
        ),
    )
}

fn desktop_dir() -> PathBuf {
    unsafe {
        let mut psz: *mut u16 = std::ptr::null_mut();
        if SHGetKnownFolderPath(&FOLDERID_Desktop, 0, std::ptr::null_mut(), &mut psz) == 0
            && !psz.is_null()
        {
            let mut len = 0usize;
            while *psz.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(psz, len));
            CoTaskMemFree(psz as *const _);
            return PathBuf::from(s);
        }
    }
    PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Desktop")
}

pub fn collect(include_drives: bool) -> Result<CollectResult, String> {
    let started = Instant::now();
    let profile = std::env::var("USERPROFILE").unwrap_or_default();
    let user = std::env::var("USERNAME").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let roaming = std::env::var("APPDATA").unwrap_or_default();
    let progdata = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());

    let (generated_at, stamp) = local_time();
    let osv = os_version();
    let disks: Vec<(String, f64, f64)> = fixed_drives()
        .into_iter()
        .map(|d| {
            let (t, f) = disk_space(&d);
            (d, t, f)
        })
        .collect();

    let software = installed_software();
    let local_rows = analyze_root(&local, 10.0);
    let roaming_rows = analyze_root(&roaming, 10.0);
    let prog_rows = analyze_root(&progdata, 50.0);

    let pkg_candidates: [(&str, PathBuf); 12] = [
        ("npm", PathBuf::from(&local).join("npm-cache")),
        ("npm2", PathBuf::from(&roaming).join("npm-cache")),
        ("pip", PathBuf::from(&local).join("pip").join("cache")),
        ("yarn", PathBuf::from(&local).join("Yarn").join("Cache")),
        ("pnpm", PathBuf::from(&local).join("pnpm").join("store")),
        ("nuget", PathBuf::from(&profile).join(".nuget").join("packages")),
        ("gradle", PathBuf::from(&profile).join(".gradle").join("caches")),
        ("maven", PathBuf::from(&profile).join(".m2").join("repository")),
        ("cargo", PathBuf::from(&profile).join(".cargo").join("registry")),
        ("go", PathBuf::from(&profile).join("go").join("pkg").join("mod").join("cache")),
        ("conda", PathBuf::from(&profile).join("miniconda3").join("pkgs")),
        ("conda2", PathBuf::from(&profile).join("anaconda3").join("pkgs")),
    ];
    let mut pkg_rows: Vec<(String, String, f64)> = Vec::new();
    for (name, p) in &pkg_candidates {
        if p.exists() {
            pkg_rows.push((
                name.to_string(),
                mask(&p.to_string_lossy(), &profile, &user),
                round1(dir_size(p) as f64 / MB),
            ));
        }
    }

    let drive_rows = if include_drives {
        scan_drives(&profile, &user)
    } else {
        Vec::new()
    };

    let desktop = desktop_dir();
    let md_path = desktop.join(format!("diskbutler-rule-report-{}.md", stamp));
    let json_path = desktop.join(format!("diskbutler-rule-report-{}.json", stamp));

    let rows_json = |rows: &Vec<TopDir>, label: &str| -> serde_json::Value {
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "root": label, "name": r.name, "sizeMB": r.size_mb,
                    "cacheHits": r.cache_hits, "flags": r.flags
                })
            })
            .collect()
    };
    let report = serde_json::json!({
        "summary": {
            "reportVersion": "1.0",
            "generatedAt": generated_at,
            "osVersion": osv,
            "collector": "app",
            "disks": disks.iter().map(|(d, t, f)| serde_json::json!({
                "drive": d, "totalGB": t, "freeGB": f
            })).collect::<Vec<_>>(),
        },
        "software": software.iter().map(|(n, s)| serde_json::json!({
            "name": n, "sizeMB": s
        })).collect::<Vec<_>>(),
        "localAppData": rows_json(&local_rows, "%LOCALAPPDATA%"),
        "roamingAppData": rows_json(&roaming_rows, "%APPDATA%"),
        "programData": rows_json(&prog_rows, "%ProgramData%"),
        "packageCaches": pkg_rows.iter().map(|(m, p, s)| serde_json::json!({
            "manager": m, "path": p, "sizeMB": s
        })).collect::<Vec<_>>(),
        "driveTopDirs": drive_rows.iter().map(|r| serde_json::json!({
            "drive": r.drive, "depth": r.depth, "path": r.path, "sizeGB": r.size_gb
        })).collect::<Vec<_>>(),
    });
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入 JSON 报告失败：{}", e))?;

    let mut md: Vec<String> = Vec::new();
    md.push("# DiskButler Rule Collection Report".into());
    md.push("".into());
    md.push("> Privacy: no file names, no file contents, no user name. Review before sharing.".into());
    md.push("".into());
    md.push("## Machine Summary".into());
    md.push(format!("- Generated: {}", generated_at));
    md.push(format!("- OS: {}", osv));
    for (d, t, f) in &disks {
        md.push(format!("- Disk {} total {} GB, free {} GB", d, t, f));
    }
    md.push("".into());
    md.push(format!("## Installed Software ({})", software.len()));
    md.push("".into());
    for (n, s) in &software {
        match s {
            Some(mb) => md.push(format!("- {} ({} MB)", n, mb)),
            None => md.push(format!("- {}", n)),
        }
    }
    let sections: [(&str, &Vec<TopDir>); 3] = [
        ("%LOCALAPPDATA% Top-Level Directories (>=10MB)", &local_rows),
        ("%APPDATA% Top-Level Directories (>=10MB)", &roaming_rows),
        ("%ProgramData% Top-Level Directories (>=50MB)", &prog_rows),
    ];
    for (title, rows) in sections {
        md.push("".into());
        md.push(format!("## {}", title));
        md.push("".into());
        md.push("| Directory | Size (MB) | Cache-like children | Flags |".into());
        md.push("|---|---|---|---|".into());
        for r in rows {
            md.push(format!(
                "| {} | {} | {} | {} |",
                r.name, r.size_mb, r.cache_hits, r.flags
            ));
        }
    }
    md.push("".into());
    md.push("## Package Manager Caches".into());
    md.push("".into());
    md.push("| Manager | Path | Size (MB) |".into());
    md.push("|---|---|---|".into());
    for (m, p, s) in &pkg_rows {
        md.push(format!("| {} | {} | {} |", m, p, s));
    }
    if include_drives {
        md.push("".into());
        md.push("## Drive Top Directories (2 levels, >=1GB) - knowledge-base clues".into());
        md.push("".into());
        md.push("| Path | Size (GB) |".into());
        md.push("|---|---|".into());
        for r in &drive_rows {
            let indent = if r.depth == 2 { "&nbsp;&nbsp;&nbsp;&nbsp;" } else { "" };
            md.push(format!("| {}{} | {} |", indent, r.path, r.size_gb));
        }
    }
    md.push("".into());
    let mins = round1(started.elapsed().as_secs_f64() / 60.0);
    md.push(format!("_Scan took {} minutes._", mins));
    let md_text = format!("\u{FEFF}{}\r\n", md.join("\r\n"));
    std::fs::write(&md_path, md_text).map_err(|e| format!("写入 MD 报告失败：{}", e))?;

    Ok(CollectResult {
        md_path: md_path.to_string_lossy().to_string(),
        json_path: json_path.to_string_lossy().to_string(),
        software_count: software.len(),
        dir_rows: local_rows.len() + roaming_rows.len() + prog_rows.len(),
        drive_rows: drive_rows.len(),
        elapsed_secs: started.elapsed().as_secs(),
    })
}
