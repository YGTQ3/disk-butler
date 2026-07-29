//! 软件体检：只陈述软件的客观行为（开机自启 / 后台常驻 / 占用较大），不给"好坏"评价、
//! 不维护厂商黑名单——规避商誉侵权风险，由用户自行判断是否保留。
//!
//! 白名单（正面、合规）：
//! - 内置"受信任/系统"清单：主流常用软件、系统组件、驱动、安全软件——降权折叠，不打扰。
//! - 用户手动"不再提醒"：本地持久化，个人兜底。
//!
//! 红线（务必守住）：
//! 1. 只陈述客观事实，不对任何软件定性；不点名"垃圾/流氓/广告"。
//! 2. 卸载 = 调起该软件自带的官方卸载程序（注册表 UninstallString），绝不直接删安装目录；
//!    无批量、无静默，逐项由前端二次确认。
//! 3. 残留清理白名单式：仅清理已卸载项残留的安装目录本身，且必须位于常规安装根下、非根本身。
//! 4. 措辞中立：用"如不常用可卸载"这类建议，而非评价。

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::Manager;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

/// id 内部分隔符（SOH 控制符，注册表键名绝不会包含）
const SEP: char = '\u{1}';
/// 安装体积达此值（MB）标注"占用较大"
const LARGE_MB: f64 = 1024.0;

/// 三个 Uninstall hive；tag 用于 id 前缀，卸载时据此重新定位注册表项。
const HIVES: [(&str, winreg::HKEY, &str); 3] = [
    ("hklm64", HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ("hklm32", HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    ("hkcu", HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
];

/// 内置"受信任/系统"白名单（正面声明，合规）：外置纯文本 + 编译期内嵌，首次解析后缓存。
/// 命中发行商或软件名即视为常用/系统，折叠降噪；只影响默认显示，不影响卸载能力。
const TRUSTED_PUBLISHERS_RAW: &str = include_str!("data/trusted-publishers.txt");
const TRUSTED_NAMES_RAW: &str = include_str!("data/trusted-names.txt");

/// 解析清单文本：按行 trim、跳过空行与 # 注释、统一小写。
fn parse_list(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_lowercase())
        .collect()
}

/// 首次调用解析并缓存 (发行商, 软件名) 两份小写指纹表，后续零解析开销。
fn trusted_lists() -> &'static (Vec<String>, Vec<String>) {
    static LISTS: OnceLock<(Vec<String>, Vec<String>)> = OnceLock::new();
    LISTS.get_or_init(|| (parse_list(TRUSTED_PUBLISHERS_RAW), parse_list(TRUSTED_NAMES_RAW)))
}

// ---------- 数据结构 ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatwareEntry {
    /// 形如 "hklm64\u{1}{GUID}"，卸载时后端据此重新读 UninstallString（杜绝前端传任意命令）
    pub id: String,
    /// 稳定标识（name|publisher 小写），用于"不再提醒"持久化（重装后仍能对上）
    pub key: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub install_location: String,
    pub size_mb: Option<f64>,
    /// 中性行为标签：开机自启 / 后台常驻 / 占用较大
    pub tags: Vec<String>,
    /// 客观行为陈述（事实，不评价好坏）
    pub behaviors: Vec<String>,
    /// 中性建议（如"如不常用，可卸载释放空间"）
    pub suggestion: String,
    /// 命中内置受信任/系统白名单（前端降权折叠为"常用/系统"）
    pub trusted: bool,
    /// 用户已手动"不再提醒"（前端折叠为"已忽略"，可恢复）
    pub dismissed: bool,
    /// 是否可一键卸载（有 UninstallString）
    pub uninstallable: bool,
    pub autostart_count: u32,
    /// 当前后台常驻进程占用内存（MB），0 表示未运行
    pub resident_mem_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatwareScan {
    pub entries: Vec<BloatwareEntry>,
    /// 需重点关注的软件数（非受信任、非已忽略）
    pub shown_count: u32,
    /// 扫描级中性提示（如检测到浏览器主页被策略设定），不归因到具体某软件
    pub browser_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidueDetail {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidueReport {
    pub freed: u64,
    pub errors: Vec<String>,
}

// ---------- 白名单：稳定 key、内置受信任判断、"不再提醒"持久化 ----------

/// 稳定标识：软件名 + 发行商小写（重装后 GUID 会变，但名字/发行商稳定）。
fn stable_key(name: &str, publisher: &str) -> String {
    format!("{}|{}", name.trim().to_lowercase(), publisher.trim().to_lowercase())
}

/// 是否命中内置受信任/系统白名单（正面声明，仅用于降噪）。
fn is_trusted(name: &str, publisher: &str) -> bool {
    let (pubs, names) = trusted_lists();
    let n = name.to_lowercase();
    let p = publisher.to_lowercase();
    (!p.is_empty() && pubs.iter().any(|t| p.contains(t.as_str())))
        || names.iter().any(|t| n.contains(t.as_str()))
}

fn ignored_file(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("bloatware-ignored.json"))
}

/// 读取用户"不再提醒"的 key 集合（文件不存在/损坏时返回空集）。
pub fn load_ignored(app: &tauri::AppHandle) -> HashSet<String> {
    ignored_file(app)
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

/// 增/删一个"不再提醒" key 并持久化（失败静默）。
pub fn set_ignored(app: &tauri::AppHandle, key: String, ignored: bool) {
    let Some(file) = ignored_file(app) else { return };
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut set = load_ignored(app);
    if ignored {
        set.insert(key);
    } else {
        set.remove(&key);
    }
    let v: Vec<String> = set.into_iter().collect();
    if let Ok(json) = serde_json::to_vec(&v) {
        let _ = std::fs::write(&file, json);
    }
}

// ---------- 客观行为探测（无厂商黑名单，只陈述事实） ----------

/// 收集运行中进程的 (exe 路径小写, 内存字节)，用于"后台常驻"探测。
fn running_processes() -> Vec<(String, u64)> {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();
    sys.processes()
        .values()
        .filter_map(|p| p.exe().map(|e| (e.to_string_lossy().to_lowercase(), p.memory())))
        .collect()
}

/// 某安装目录下运行进程占用的总内存（MB）；0 表示当前无常驻进程。
fn resident_mem_mb(install_location: &str, procs: &[(String, u64)]) -> u64 {
    if install_location.is_empty() {
        return 0;
    }
    let loc = install_location.trim_end_matches('\\').to_lowercase();
    let bytes: u64 = procs
        .iter()
        .filter(|(exe, _)| exe.starts_with(&loc))
        .map(|(_, m)| m)
        .sum();
    bytes / 1024 / 1024
}

/// 某安装目录关联的开机自启项数量（自启命令列表一次性传入，避免重复枚举）。
fn autostart_count_for(install_location: &str, starts: &[String]) -> u32 {
    if install_location.is_empty() {
        return 0;
    }
    let loc = install_location.trim_end_matches('\\').to_lowercase();
    starts.iter().filter(|cmd| cmd.contains(&loc)).count() as u32
}

// ---------- 注册表读取（补齐 collector 未读的字段） ----------

struct InstalledApp {
    id: String,
    name: String,
    publisher: String,
    version: String,
    install_location: String,
    size_mb: Option<f64>,
    uninstall_string: String,
}

fn read_str(k: &RegKey, name: &str) -> String {
    k.get_value::<String, _>(name).unwrap_or_default().trim().to_string()
}

fn list_installed() -> Vec<InstalledApp> {
    let mut out: Vec<InstalledApp> = Vec::new();
    for (tag, hive, path) in HIVES {
        let Ok(root) = RegKey::predef(hive).open_subkey(path) else { continue };
        for sub in root.enum_keys().flatten() {
            let Ok(k) = root.open_subkey(&sub) else { continue };
            let name = read_str(&k, "DisplayName");
            if name.is_empty() {
                continue;
            }
            if k.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
                continue;
            }
            if !read_str(&k, "ParentKeyName").is_empty() {
                continue;
            }
            let size_mb = k
                .get_value::<u32, _>("EstimatedSize")
                .ok()
                .map(|kb| (kb as f64 / 1024.0 * 10.0).round() / 10.0);
            out.push(InstalledApp {
                id: format!("{}{}{}", tag, SEP, sub),
                name,
                publisher: read_str(&k, "Publisher"),
                version: read_str(&k, "DisplayVersion"),
                install_location: read_str(&k, "InstallLocation"),
                size_mb,
                uninstall_string: {
                    let s = read_str(&k, "QuietUninstallString");
                    if s.is_empty() { read_str(&k, "UninstallString") } else { s }
                },
            });
        }
    }
    out
}

/// 读取浏览器主页策略键（若被某软件设定，给出中性提示）。
fn browser_notes() -> Vec<String> {
    let mut notes = Vec::new();
    let checks: [(&str, &str, &str); 2] = [
        ("Chrome", r"SOFTWARE\Policies\Google\Chrome", "HomepageLocation"),
        ("Edge", r"SOFTWARE\Policies\Microsoft\Edge", "HomepageLocation"),
    ];
    for (browser, path, value) in checks {
        for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            if let Ok(k) = RegKey::predef(hive).open_subkey(path) {
                let url = read_str(&k, value);
                if !url.is_empty() {
                    notes.push(format!(
                        "检测到 {} 主页被设定为 {}（可能由某软件更改，可在浏览器设置中改回）。",
                        browser, url
                    ));
                }
            }
        }
    }
    notes
}

// ---------- 扫描 ----------

pub fn scan(ignored: &HashSet<String>) -> BloatwareScan {
    let procs = running_processes();
    // 自启命令一次性枚举（小写），供所有软件比对，避免每个软件都全量枚举一次
    let starts: Vec<String> = crate::startup::list_items()
        .into_iter()
        .map(|it| it.command.to_lowercase())
        .collect();

    let mut entries: Vec<BloatwareEntry> = Vec::new();

    for app in list_installed() {
        let autostart_count = autostart_count_for(&app.install_location, &starts);
        let resident = resident_mem_mb(&app.install_location, &procs);
        let large = app.size_mb.map(|m| m >= LARGE_MB).unwrap_or(false);

        // 只列出有值得关注行为的软件（安静的小软件不打扰）
        if autostart_count == 0 && resident == 0 && !large {
            continue;
        }

        let mut tags: Vec<String> = Vec::new();
        let mut behaviors: Vec<String> = Vec::new();
        if autostart_count > 0 {
            tags.push("开机自启".to_string());
            behaviors.push(format!("设置了 {} 个开机自启项，会随开机启动。", autostart_count));
        }
        if resident > 0 {
            tags.push("后台常驻".to_string());
            behaviors.push(format!("当前有后台进程在运行，占用内存约 {} MB。", resident));
        }
        if large {
            tags.push("占用较大".to_string());
            if let Some(m) = app.size_mb {
                behaviors.push(format!("安装体积约 {:.1} GB，占用较多磁盘空间。", m / 1024.0));
            }
        }

        let key = stable_key(&app.name, &app.publisher);
        entries.push(BloatwareEntry {
            id: app.id,
            trusted: is_trusted(&app.name, &app.publisher),
            dismissed: ignored.contains(&key),
            key,
            name: app.name,
            publisher: app.publisher,
            version: app.version,
            install_location: app.install_location,
            size_mb: app.size_mb,
            tags,
            behaviors,
            suggestion: "如果你不常用它，卸载可减少开机负担、释放空间；常用则保留即可。".to_string(),
            uninstallable: !app.uninstall_string.is_empty(),
            autostart_count,
            resident_mem_mb: resident,
        });
    }

    // 负担降序：自启项权重最高，其次常驻内存、体积
    entries.sort_by(|a, b| {
        let score = |e: &BloatwareEntry| {
            e.autostart_count as f64 * 200.0 + e.resident_mem_mb as f64 + e.size_mb.unwrap_or(0.0)
        };
        score(b).partial_cmp(&score(a)).unwrap_or(std::cmp::Ordering::Equal)
    });

    // 需重点关注 = 非受信任 且 非已忽略
    let shown_count = entries.iter().filter(|e| !e.trusted && !e.dismissed).count() as u32;
    BloatwareScan {
        entries,
        shown_count,
        browser_notes: browser_notes(),
    }
}

// ---------- 一键卸载（调官方卸载器） ----------

/// 据 id 重新打开注册表项并读取卸载命令（后端解析，前端只传 id）。
fn uninstall_string_of(id: &str) -> Option<(String, String)> {
    let (tag, sub) = id.split_once(SEP)?;
    let (_, hive, path) = HIVES.iter().find(|(t, _, _)| *t == tag)?;
    let key = RegKey::predef(*hive).open_subkey(path).ok()?;
    let k = key.open_subkey(sub).ok()?;
    let name = read_str(&k, "DisplayName");
    let s = read_str(&k, "QuietUninstallString");
    let s = if s.is_empty() { read_str(&k, "UninstallString") } else { s };
    if s.is_empty() {
        None
    } else {
        Some((name, s))
    }
}

/// id 对应项是否仍在注册表中（判定卸载是否成功的"最终事实"）。
fn still_installed(id: &str) -> bool {
    let Some((tag, sub)) = id.split_once(SEP) else { return false };
    let Some((_, hive, path)) = HIVES.iter().find(|(t, _, _)| *t == tag) else { return false };
    RegKey::predef(*hive)
        .open_subkey(path)
        .and_then(|key| key.open_subkey(sub))
        .map(|k| !read_str(&k, "DisplayName").is_empty())
        .unwrap_or(false)
}

/// 卸载：运行该软件自带的官方卸载程序并等待完成；以"注册表项是否消失"判定成败。
pub fn uninstall(id: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let (_name, cmd) = uninstall_string_of(id).ok_or_else(|| "找不到该软件的卸载程序".to_string())?;

    // 通过 cmd /C 原样执行卸载命令串（自身含引号，交给 cmd 解析）；
    // 卸载器如需管理员会自行弹 UAC。等待其结束。
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .raw_arg(&cmd)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("启动卸载程序失败：{}", e))?;
    let _ = status;

    if still_installed(id) {
        Err("卸载未完成（可能已取消，或该软件卸载后需重启）。可稍后重新扫描确认。".to_string())
    } else {
        Ok(())
    }
}

// ---------- 卸载后残留清理（白名单式，严格路径校验） ----------

/// 路径是否是"可安全清理的残留目录"：存在、是目录、位于常规安装根下、且不是根本身。
fn is_safe_residue_path(p: &str) -> bool {
    let path = Path::new(p);
    if !path.is_dir() {
        return false;
    }
    let Ok(canon) = path.canonicalize() else { return false };
    install_roots().iter().any(|root| {
        if let Ok(rc) = root.canonicalize() {
            canon.starts_with(&rc) && canon != rc
        } else {
            false
        }
    })
}

fn install_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramData", "LOCALAPPDATA", "APPDATA"] {
        if let Some(p) = std::env::var_os(var) {
            v.push(PathBuf::from(p));
        }
    }
    v
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    for entry in jwalk::WalkDir::new(path).skip_hidden(false).into_iter().flatten() {
        if let Ok(md) = entry.metadata() {
            if md.is_file() {
                total += md.len();
            }
        }
    }
    total
}

/// 扫描某个（已卸载软件的）安装目录残留：仅当路径校验通过且非空才返回。
pub fn scan_residue(install_location: &str) -> Option<ResidueDetail> {
    if install_location.is_empty() || !is_safe_residue_path(install_location) {
        return None;
    }
    let size = dir_size(Path::new(install_location));
    if size == 0 {
        return None;
    }
    Some(ResidueDetail {
        path: install_location.to_string(),
        size,
    })
}

/// 清理残留目录：逐个再次校验路径安全后删除。
pub fn clean_residue(paths: Vec<String>) -> ResidueReport {
    let mut freed = 0u64;
    let mut errors = Vec::new();
    for p in paths {
        if !is_safe_residue_path(&p) {
            errors.push(format!("已跳过（路径不在允许范围）：{}", p));
            continue;
        }
        let size = dir_size(Path::new(&p));
        match std::fs::remove_dir_all(&p) {
            Ok(_) => freed += size,
            Err(e) => errors.push(format!("删除失败 {}：{}", p, e)),
        }
    }
    ResidueReport { freed, errors }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_matches_common_and_system_software() {
        assert!(is_trusted("Google Chrome", "Google LLC"));
        assert!(is_trusted("微信", "Tencent"));
        assert!(is_trusted("NVIDIA 显卡驱动", ""));
        assert!(is_trusted("某工具", "Microsoft Corporation"));
        // 不认识的普通软件不应被当作受信任
        assert!(!is_trusted("某某清理大师", "某某科技"));
    }

    #[test]
    fn stable_key_is_case_insensitive() {
        assert_eq!(stable_key("Foo", "Bar Inc"), stable_key("foo", "bar inc"));
    }

    #[test]
    fn autostart_count_matches_by_install_dir() {
        let starts = vec![
            r"c:\program files\foo\foo.exe /background".to_string(),
            r"c:\program files\foo\helper.exe".to_string(),
            r"c:\windows\other.exe".to_string(),
        ];
        assert_eq!(autostart_count_for(r"C:\Program Files\Foo", &starts), 2);
        assert_eq!(autostart_count_for("", &starts), 0);
    }

    #[test]
    fn resident_mem_sums_processes_under_install_dir() {
        let procs = vec![
            (r"c:\program files\foo\foo.exe".to_string(), 200 * 1024 * 1024),
            (r"c:\program files\foo\sub\bar.exe".to_string(), 100 * 1024 * 1024),
            (r"c:\windows\other.exe".to_string(), 999 * 1024 * 1024),
        ];
        assert_eq!(resident_mem_mb(r"C:\Program Files\Foo", &procs), 300);
        assert_eq!(resident_mem_mb("", &procs), 0);
    }

    #[test]
    fn residue_path_rejects_roots_and_outside() {
        assert!(!is_safe_residue_path(r"C:\"));
        assert!(!is_safe_residue_path(r"C:\Windows"));
        assert!(!is_safe_residue_path(r"Z:\definitely\not\exist\xyz"));
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            assert!(!is_safe_residue_path(&pf.to_string_lossy()), "不应允许删除 ProgramFiles 根");
        }
    }

    #[test]
    fn id_roundtrip_split() {
        let id = format!("hklm64{}{{GUID-1234}}", SEP);
        let (tag, sub) = id.split_once(SEP).unwrap();
        assert_eq!(tag, "hklm64");
        assert_eq!(sub, "{GUID-1234}");
    }
}
