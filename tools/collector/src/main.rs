// =====================================================================
// DiskButler 规则采集器（绿色单文件版）
// 与 tools/collect-rules.ps1 逻辑与报告格式完全对齐：
//   采集：AppData/ProgramData 顶层目录名+大小、缓存模式命中、软件清单、
//         包管理器缓存；完整模式另加各磁盘根两级 >=1GB 大目录线索。
//   永不采集：文件名、文件内容、用户名（路径脱敏为 %USERPROFILE%）。
//   零网络、不删除不修改任何东西、无需管理员权限。
// 报告输出到桌面：diskbutler-rule-report-<时间戳>.md / .json
// =====================================================================

mod console;

use console::{cprint, cprintln};
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

// 与 ps1 版一致的缓存类子目录名（小写、精确匹配）
const CACHE_PATTERNS: &[&str] = &[
    "cache", "code cache", "gpucache", "cacheddata", "cachestorage", "shadercache",
    "dawncache", "crashpad", "blob_storage", "service worker", "logs", "log", "temp",
    "tmp", "crashdumps", "dumps", "pending", "staging",
];
// 个人数据红旗：顶层目录名包含即标记，永不成为清理规则
const PERSONAL_PATTERNS: &[&str] = &[
    "download", "document", "desktop", "picture", "photo", "video", "music",
];

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

/// 递归目录大小（jwalk 并行遍历，只读元数据不读内容）
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

/// 大小写不敏感替换（用于路径脱敏）
fn replace_ci(hay: &str, needle: &str, repl: &str) -> String {
    if needle.is_empty() {
        return hay.to_string();
    }
    let hl = hay.to_lowercase();
    let nl = needle.to_lowercase();
    // 仅当 lowercase 不改变字节长度时才能安全按索引切片（Windows 路径几乎总是如此）
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

fn progress(label: &str, i: usize, total: usize, name: &str) {
    let short: String = name.chars().take(26).collect();
    cprint(&format!("\r  正在扫描 {} ({}/{})：{:<28}", label, i, total, short));
}
fn clear_progress() {
    cprint(&format!("\r{:<76}\r", ""));
}

/// 收集顶层目录下两级子目录中缓存类命名的相对路径
/// （与 ps1 的 Get-ChildItem -Depth 1 枚举顺序一致：先列完一级，再逐个下钻二级）
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

/// 分析一个根目录（LOCALAPPDATA / APPDATA / ProgramData）的顶层目录
fn analyze_root(root: &str, label: &str, min_mb: f64) -> Vec<TopDir> {
    let mut rows: Vec<TopDir> = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return rows };
    let dirs: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    let total = dirs.len();
    for (i, d) in dirs.iter().enumerate() {
        let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
        progress(label, i + 1, total, &name);
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
        rows.push(TopDir {
            name,
            size_mb,
            cache_hits,
            flags: flags.join("; "),
        });
    }
    clear_progress();
    rows.sort_by(|a, b| b.size_mb.partial_cmp(&a.size_mb).unwrap_or(std::cmp::Ordering::Equal));
    rows
}

/// 注册表三处卸载位置 → 全部已安装软件（名称+估算大小MB），按名去重排序
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

/// 固定磁盘盘符列表（"C:" 形式）
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

/// 完整模式：各固定盘根两级、>=1GB 目录（知识库线索）
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
        let total = dirs.len();
        for (i, d) in dirs.iter().enumerate() {
            let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
            progress(&format!("磁盘 {}", dv), i + 1, total, &name);
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
        clear_progress();
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let interactive = args.is_empty();
    let mut include_drives = args.iter().any(|a| {
        let a = a.to_lowercase();
        a == "--full" || a == "-includedrives" || a == "--includedrives"
    });

    cprintln("==============================================");
    cprintln("  C盘管家 · 规则采集器（绿色单文件版）");
    cprintln("  不删除、不修改任何东西，也不联网。");
    cprintln("  报告会生成在你的桌面上。");
    cprintln("==============================================");
    cprintln("");

    if interactive {
        cprintln("  [1] 基础模式（推荐）：扫描软件缓存区，约 1 分钟");
        cprintln("  [2] 完整模式：另外收集各磁盘的大目录线索，约 5~10 分钟");
        cprintln("      （注意：完整模式会列出 D 盘等根目录下大文件夹的名字，发送前请自行审查）");
        cprintln("");
        cprint("请输入 1 或 2 后按回车（直接回车 = 1）：");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        if line.trim() == "2" {
            include_drives = true;
        }
        cprintln("");
    }
    cprintln(if include_drives {
        "模式：完整（含磁盘大目录线索）"
    } else {
        "模式：基础"
    });
    cprintln("开始采集，请稍候……");
    cprintln("");

    let started = Instant::now();
    let profile = std::env::var("USERPROFILE").unwrap_or_default();
    let user = std::env::var("USERNAME").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let roaming = std::env::var("APPDATA").unwrap_or_default();
    let progdata = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());

    // 1. 机器概要（无主机名、无用户名）
    let (generated_at, stamp) = local_time();
    let osv = os_version();
    let disks: Vec<(String, f64, f64)> = fixed_drives()
        .into_iter()
        .map(|d| {
            let (t, f) = disk_space(&d);
            (d, t, f)
        })
        .collect();

    // 2. 已安装软件
    let software = installed_software();
    cprintln(&format!("  已安装软件清单：{} 项", software.len()));

    // 3. 三个根目录顶层分析
    let local_rows = analyze_root(&local, "%LOCALAPPDATA%", 10.0);
    cprintln(&format!("  %LOCALAPPDATA% 完成：{} 项（≥10MB）", local_rows.len()));
    let roaming_rows = analyze_root(&roaming, "%APPDATA%", 10.0);
    cprintln(&format!("  %APPDATA% 完成：{} 项（≥10MB）", roaming_rows.len()));
    let prog_rows = analyze_root(&progdata, "%ProgramData%", 50.0);
    cprintln(&format!("  %ProgramData% 完成：{} 项（≥50MB）", prog_rows.len()));

    // 4. 包管理器缓存
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
    cprintln(&format!("  包管理器缓存：{} 处", pkg_rows.len()));

    // 4b. 完整模式：磁盘根两级大目录
    let drive_rows = if include_drives {
        cprintln("  完整模式：扫描各磁盘根目录（会多花几分钟）……");
        scan_drives(&profile, &user)
    } else {
        Vec::new()
    };
    if include_drives {
        cprintln(&format!("  磁盘大目录线索：{} 条", drive_rows.len()));
    }

    // 5. 组装报告
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
            "collector": "exe",
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
    if let Err(e) = std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&report).unwrap_or_default(),
    ) {
        cprintln(&format!("[错误] 写入 JSON 报告失败：{}", e));
    }

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
    // 与 ps1 版一致：UTF-8 BOM + CRLF，记事本双击直接可读
    let md_text = format!("\u{FEFF}{}\r\n", md.join("\r\n"));
    if let Err(e) = std::fs::write(&md_path, md_text) {
        cprintln(&format!("[错误] 写入 MD 报告失败：{}", e));
    }

    cprintln("");
    cprintln(&format!("完成，用时 {} 分钟。", mins));
    cprintln(&format!("报告（人看的）：{}", md_path.display()));
    cprintln(&format!("报告（机器版）：{}", json_path.display()));
    cprintln("");
    cprintln("请先打开 .md 报告自己审查一遍，确认没有不想分享的内容，再发送给收集人。");

    if interactive {
        cprintln("");
        cprint("按回车键退出……");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }
}
