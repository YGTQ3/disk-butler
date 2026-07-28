// =====================================================================
// DiskButler MFT 全盘秒级透视采集器（独立版）
//   用 MFT 直读引擎（WizTree 同款原理）秒级枚举所有 NTFS 盘的大目录，
//   输出与绿色采集器一致的 md/json 报告（collector 标记为 "mft"），
//   直接进 samples 规则评估流水线。
//   需要管理员权限（打开裸卷句柄是系统硬性要求）——启动时自动请求 UAC。
//   只读、不删除不修改任何东西、不联网。路径脱敏为 %USERPROFILE%。
// =====================================================================

mod console;
mod mft;

use console::{cprint, cprintln};
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::Instant;
use windows_sys::Win32::Foundation::{CloseHandle, SYSTEMTIME};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::{
    SHGetKnownFolderPath, ShellExecuteW, FOLDERID_Desktop,
};
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

const GB: f64 = 1024.0 * 1024.0 * 1024.0;
const DRIVE_FIXED: u32 = 3;
/// 报告里收录的目录/文件大小阈值（GB）——低于此不列，保持报告聚焦。
const ROW_MIN_GB: f64 = 1.0;

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

// ---------- 管理员权限 ----------

fn is_elevated() -> bool {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// 以管理员身份重新启动自身（UAC）。返回 true 表示已成功拉起提权进程。
fn relaunch_as_admin() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe_w: Vec<u16> = exe.as_os_str().encode_wide().chain(once(0)).collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain(once(0)).collect();
    let params: Vec<u16> = "--elevated".encode_utf16().chain(once(0)).collect();
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe_w.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        )
    };
    (r as isize) > 32
}

// ---------- 磁盘 ----------

fn fixed_ntfs_drives() -> Vec<String> {
    let mask = unsafe { GetLogicalDrives() };
    let mut v = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root: Vec<u16> = format!("{}:\\", letter).encode_utf16().chain(once(0)).collect();
        if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
            continue;
        }
        if is_ntfs(&root) {
            v.push(format!("{}:", letter));
        }
    }
    v
}

fn is_ntfs(root_w: &[u16]) -> bool {
    let mut fsname = [0u16; 32];
    let ok = unsafe {
        GetVolumeInformationW(
            root_w.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fsname.as_mut_ptr(),
            fsname.len() as u32,
        )
    };
    if ok == 0 {
        return false;
    }
    let n = fsname.iter().position(|&c| c == 0).unwrap_or(fsname.len());
    String::from_utf16_lossy(&fsname[..n]).eq_ignore_ascii_case("NTFS")
}

fn disk_space(drive: &str) -> (f64, f64) {
    let root: Vec<u16> = format!("{}\\", drive).encode_utf16().chain(once(0)).collect();
    let mut avail = 0u64;
    let mut total = 0u64;
    let mut free = 0u64;
    unsafe {
        GetDiskFreeSpaceExW(root.as_ptr(), &mut avail, &mut total, &mut free);
    }
    (round1(total as f64 / GB), round1(free as f64 / GB))
}

// ---------- 报告辅助 ----------

fn os_version() -> String {
    if let Ok(k) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
    {
        let product: String = k.get_value("ProductName").unwrap_or_else(|_| "Windows".into());
        let disp: String = k.get_value("DisplayVersion").unwrap_or_default();
        let major: u32 = k.get_value("CurrentMajorVersionNumber").unwrap_or(10);
        let minor: u32 = k.get_value("CurrentMinorVersionNumber").unwrap_or(0);
        let build: String = k.get_value("CurrentBuildNumber").unwrap_or_default();
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

/// 大小写不敏感替换（路径脱敏）
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

/// 报告行：一条大目录/大文件线索。
struct DriveRow {
    drive: String,
    depth: u32,
    path: String,
    size_gb: f64,
    is_dir: bool,
}

/// 递归展开 MFT 树为报告行（跳过合并的「其他」占位节点；只收 >=ROW_MIN_GB）。
fn flatten(
    node: &mft::TreeNode,
    depth: u32,
    drive: &str,
    profile: &str,
    user: &str,
    out: &mut Vec<DriveRow>,
) {
    for child in &node.children {
        if child.path.contains("::__others__") {
            continue;
        }
        let gb = child.size as f64 / GB;
        if gb >= ROW_MIN_GB {
            out.push(DriveRow {
                drive: drive.to_string(),
                depth,
                path: mask(&child.path, profile, user),
                size_gb: round2(gb),
                is_dir: child.is_dir,
            });
        }
        if child.is_dir {
            flatten(child, depth + 1, drive, profile, user, out);
        }
    }
}

// ---------- 每盘扫描进度 ----------

fn scan_progress(drive: &str, files: u64, _bytes: u64, percent: f32, phase: &str) {
    cprint(&format!(
        "\r  {} {} {:.0}%（已发现 {} 文件）        ",
        drive, phase, percent, files
    ));
}
fn clear_line() {
    cprint(&format!("\r{:<64}\r", ""));
}

fn main() {
    let elevated_flag = std::env::args().any(|a| a == "--elevated");

    cprintln("==============================================");
    cprintln("  C盘管家 · MFT 全盘秒级透视采集器（独立版）");
    cprintln("  只读、不删除不修改、不联网。报告生成在桌面。");
    cprintln("  需要管理员权限（直读磁盘文件表的系统要求）。");
    cprintln("==============================================");
    cprintln("");

    if !is_elevated() {
        cprintln("本工具需要管理员权限才能直读磁盘文件表（仅只读，不做任何修改）。");
        cprintln("正在请求管理员权限，请在弹出的 UAC 窗口点击「是」……");
        if relaunch_as_admin() {
            // 已拉起提权进程，本普通权限进程退出
            return;
        }
        cprintln("");
        cprintln("[提示] 未获得管理员权限，无法直读文件表。");
        cprintln("       请右键本程序选择「以管理员身份运行」，或在弹窗时点「是」。");
        if console::is_console() {
            cprint("按回车键退出……");
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
        }
        return;
    }
    let _ = elevated_flag;

    let started = Instant::now();
    let profile = std::env::var("USERPROFILE").unwrap_or_default();
    let user = std::env::var("USERNAME").unwrap_or_default();
    let (generated_at, stamp) = local_time();
    let osv = os_version();

    let drives = fixed_ntfs_drives();
    cprintln(&format!("发现 {} 个 NTFS 固定磁盘：{}", drives.len(), drives.join(" ")));
    cprintln("开始秒级扫描……");
    cprintln("");

    let disks: Vec<(String, f64, f64)> = drives
        .iter()
        .map(|d| {
            let (t, f) = disk_space(d);
            (d.clone(), t, f)
        })
        .collect();

    let mut all_rows: Vec<DriveRow> = Vec::new();
    // 每盘扫描统计：drive, 耗时秒, 文件数, 总GB
    let mut scan_stats: Vec<(String, f64, u64, f64)> = Vec::new();

    for drive in &drives {
        let root = format!("{}\\", drive);
        let t0 = Instant::now();
        let mut last_files = 0u64;
        let mut last_bytes = 0u64;
        let mut last_emit = Instant::now();
        let result = mft::scan_mft(&root, |f, b, p, ph| {
            last_files = f;
            last_bytes = b;
            if last_emit.elapsed().as_millis() >= 100 || p >= 100.0 {
                last_emit = Instant::now();
                scan_progress(drive, f, b, p, ph);
            }
        });
        clear_line();
        let secs = round2(t0.elapsed().as_secs_f64());
        match result {
            Ok(tree) => {
                let total_gb = round2(tree.size as f64 / GB);
                flatten(&tree, 1, drive, &profile, &user, &mut all_rows);
                scan_stats.push((drive.clone(), secs, last_files, total_gb));
                cprintln(&format!(
                    "  {} 完成：{:.2}s，{} 文件，合计 {:.2} GB",
                    drive, secs, last_files, total_gb
                ));
            }
            Err(e) => {
                scan_stats.push((drive.clone(), secs, 0, 0.0));
                cprintln(&format!("  {} 扫描失败：{}", drive, e));
            }
        }
    }
    cprintln("");

    // ---------- 组装报告 ----------
    let desktop = desktop_dir();
    let md_path = desktop.join(format!("diskbutler-mft-report-{}.md", stamp));
    let json_path = desktop.join(format!("diskbutler-mft-report-{}.json", stamp));

    let report = serde_json::json!({
        "summary": {
            "reportVersion": "1.0",
            "generatedAt": generated_at,
            "osVersion": osv,
            "collector": "mft",
            "disks": disks.iter().map(|(d, t, f)| serde_json::json!({
                "drive": d, "totalGB": t, "freeGB": f
            })).collect::<Vec<_>>(),
            "mftScan": scan_stats.iter().map(|(d, s, f, g)| serde_json::json!({
                "drive": d, "elapsedSec": s, "files": f, "totalGB": g
            })).collect::<Vec<_>>(),
        },
        "software": serde_json::Value::Array(vec![]),
        "localAppData": serde_json::Value::Array(vec![]),
        "roamingAppData": serde_json::Value::Array(vec![]),
        "programData": serde_json::Value::Array(vec![]),
        "packageCaches": serde_json::Value::Array(vec![]),
        "driveTopDirs": all_rows.iter().map(|r| serde_json::json!({
            "drive": r.drive, "depth": r.depth, "path": r.path,
            "sizeGB": r.size_gb, "isDir": r.is_dir
        })).collect::<Vec<_>>(),
    });
    if let Err(e) = std::fs::write(&json_path, serde_json::to_string_pretty(&report).unwrap_or_default()) {
        cprintln(&format!("[错误] 写入 JSON 报告失败：{}", e));
    }

    let mut md: Vec<String> = Vec::new();
    md.push("# DiskButler MFT Full-Disk Report".into());
    md.push("".into());
    md.push("> Privacy: no file contents, user name masked as %USERPROFILE%. Review before sharing.".into());
    md.push("> Engine: MFT direct-read (WizTree-style). Read-only.".into());
    md.push("".into());
    md.push("## Machine Summary".into());
    md.push(format!("- Generated: {}", generated_at));
    md.push(format!("- OS: {}", osv));
    for (d, t, f) in &disks {
        md.push(format!("- Disk {} total {} GB, free {} GB", d, t, f));
    }
    md.push("".into());
    md.push("## MFT Scan Stats".into());
    md.push("".into());
    md.push("| Drive | Elapsed (s) | Files | Total (GB) |".into());
    md.push("|---|---|---|---|".into());
    for (d, s, f, g) in &scan_stats {
        md.push(format!("| {} | {} | {} | {} |", d, s, f, g));
    }
    md.push("".into());
    md.push(format!("## Drive Top Directories (MFT, up to 4 levels, >={}GB)", ROW_MIN_GB));
    md.push("".into());
    md.push("| Path | Type | Size (GB) |".into());
    md.push("|---|---|---|".into());
    for r in &all_rows {
        let indent = "&nbsp;".repeat(((r.depth.saturating_sub(1)) * 4) as usize);
        let kind = if r.is_dir { "dir" } else { "file" };
        md.push(format!("| {}{} | {} | {} |", indent, r.path, kind, r.size_gb));
    }
    md.push("".into());
    let secs = round1(started.elapsed().as_secs_f64());
    md.push(format!("_Scan took {} seconds (MFT direct-read)._", secs));
    let md_text = format!("\u{FEFF}{}\r\n", md.join("\r\n"));
    if let Err(e) = std::fs::write(&md_path, md_text) {
        cprintln(&format!("[错误] 写入 MD 报告失败：{}", e));
    }

    cprintln(&format!("完成，总用时 {} 秒。", secs));
    cprintln(&format!("报告（人看的）：{}", md_path.display()));
    cprintln(&format!("报告（机器版）：{}", json_path.display()));
    cprintln("");
    cprintln("请先打开 .md 报告自己审查一遍，确认没有不想分享的内容，再发送给收集人。");
    cprintln("");
    if console::is_console() {
        cprint("按回车键退出……");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }
}
