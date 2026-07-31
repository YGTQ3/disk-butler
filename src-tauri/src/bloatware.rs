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
/// 安全软件/杀毒软件识别清单（关键词，匹配名称或发行商）：命中单独归类 + 卸载走诚实引导。
const SECURITY_RAW: &str = include_str!("data/security-software.txt");

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

/// 安全软件关键词表（首次解析后缓存）。
fn security_list() -> &'static Vec<String> {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| parse_list(SECURITY_RAW))
}

/// 是否为安全软件/杀毒软件（关键词命中名称或发行商）。
fn is_security(name: &str, publisher: &str) -> bool {
    let n = name.to_lowercase();
    let p = publisher.to_lowercase();
    security_list().iter().any(|k| n.contains(k.as_str()) || (!p.is_empty() && p.contains(k.as_str())))
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
    /// 软件图标（PNG data URL，带透明通道）；提取失败为 None，前端用占位
    pub icon: Option<String>,
    /// 中性行为标签：开机自启 / 后台常驻 / 占用较大
    pub tags: Vec<String>,
    /// 客观行为陈述（事实，不评价好坏）
    pub behaviors: Vec<String>,
    /// 中性建议（如"如不常用，可卸载释放空间"）
    pub suggestion: String,
    /// 命中内置受信任/系统白名单（前端降权折叠为"常用/系统"）
    pub trusted: bool,
    /// 是否安全软件/杀毒软件（前端单独归类 + 卸载走诚实引导）
    pub security: bool,
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

/// 卸载后残留全景报告：文件目录 + 当前用户注册表键。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidueScanReport {
    pub dirs: Vec<ResidueDetail>,
    pub reg_keys: Vec<String>,
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

/// 枚举「自动启动」服务的 ImagePath（小写），作为持久自启信号之一。
/// 只认 Start=2（自动）；手动/按需(3)、禁用(4)、引导/系统(0/1) 都不随开机启动，
/// 排除以免误判——例如本程序的 MFT 秒扫服务是按需启动、用完即停（Start=3）。
fn service_commands() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(services) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SYSTEM\CurrentControlSet\Services") {
        for name in services.enum_keys().flatten() {
            if let Ok(k) = services.open_subkey(&name) {
                if k.get_value::<u32, _>("Start").unwrap_or(3) != 2 {
                    continue;
                }
                let img = read_str(&k, "ImagePath");
                if !img.is_empty() {
                    out.push(img.to_lowercase());
                }
            }
        }
    }
    out
}

/// 解码计划任务 XML（多为 UTF-16LE BOM）。
fn decode_task(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        encoding_rs::UTF_16LE.decode(bytes).0.into_owned()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// 枚举计划任务的 <Command> 执行路径（小写），作为持久自启信号之一。
fn scheduled_task_commands() -> Vec<String> {
    let root = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()))
        .join("System32")
        .join("Tasks");
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if let Ok(bytes) = std::fs::read(&p) {
                let low = decode_task(&bytes).to_lowercase();
                let mut idx = 0;
                while let Some(s) = low[idx..].find("<command>") {
                    let start = idx + s + "<command>".len();
                    let Some(e2) = low[start..].find("</command>") else { break };
                    let cmd = low[start..start + e2].trim().trim_matches('"').to_string();
                    if !cmd.is_empty() {
                        out.push(cmd);
                    }
                    idx = start + e2;
                }
            }
        }
    }
    walk(&root, &mut out);
    out
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
    display_icon: String,
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
                display_icon: read_str(&k, "DisplayIcon"),
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

// ---------- 软件图标来源解析（提取实现见 crate::icon） ----------

/// 从卸载命令里取出 exe 路径（去引号/参数）。
fn exe_of(cmd: &str) -> String {
    let c = cmd.trim();
    if let Some(rest) = c.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return rest[..end].to_string();
        }
    }
    c.split_whitespace().next().unwrap_or("").to_string()
}

/// 软件的"有效目录"：优先注册表 InstallLocation；为空时退回卸载器 exe 的父目录，
/// 再退回图标文件的父目录。解决豆包等 InstallLocation 为空导致后台检测/停止失效的问题。
fn effective_location(install_location: &str, uninstall: &str, display_icon: &str) -> String {
    let il = install_location.trim().trim_matches('"');
    if !il.is_empty() {
        return il.to_string();
    }
    // 卸载器 exe 的父目录（uninstall.exe 就在软件安装目录内，最可靠）
    let un = exe_of(uninstall);
    if !un.is_empty() {
        if let Some(dir) = Path::new(&un).parent() {
            let d = dir.to_string_lossy().to_string();
            if !d.trim().is_empty() {
                return d;
            }
        }
    }
    // 图标文件的父目录（去掉 ",<index>" 后缀）
    let mut ic = display_icon.trim().trim_matches('"').to_string();
    if let Some(idx) = ic.rfind(',') {
        let tail = ic[idx + 1..].trim();
        if !tail.is_empty() && tail.trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) {
            ic = ic[..idx].to_string();
        }
    }
    let ic = ic.trim().trim_matches('"');
    if !ic.is_empty() {
        if let Some(dir) = Path::new(ic).parent() {
            return dir.to_string_lossy().to_string();
        }
    }
    String::new()
}

/// 在安装目录顶层挑一个"主程序 exe"（跳过卸载器/更新器，取体积最大的）。
fn main_exe_in(dir: &str) -> Option<String> {
    if dir.is_empty() {
        return None;
    }
    let mut best: Option<(u64, String)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if !p.extension().map(|x| x.eq_ignore_ascii_case("exe")).unwrap_or(false) {
            continue;
        }
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        if ["unins", "uninst", "update", "setup", "crash", "helper", "report"]
            .iter()
            .any(|k| name.contains(k))
        {
            continue;
        }
        let sz = e.metadata().map(|m| m.len()).unwrap_or(0);
        if best.as_ref().map(|(b, _)| sz > *b).unwrap_or(true) {
            best = Some((sz, p.to_string_lossy().to_string()));
        }
    }
    best.map(|(_, p)| p)
}

/// 解析图标来源文件：DisplayIcon → 卸载器 exe → 安装目录主 exe（三级回退，尽量抠到图标）。
fn resolve_icon_path(display_icon: &str, uninstall: &str, install_location: &str) -> Option<String> {
    let mut s = display_icon.trim().trim_matches('"').to_string();
    if let Some(idx) = s.rfind(',') {
        let tail = s[idx + 1..].trim();
        if !tail.is_empty() && tail.trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) {
            s = s[..idx].to_string();
        }
    }
    let s = s.trim().trim_matches('"').to_string();
    if !s.is_empty() && Path::new(&s).exists() {
        return Some(s);
    }
    let exe = exe_of(uninstall);
    if !exe.is_empty() && !exe.to_lowercase().contains("msiexec") && Path::new(&exe).exists() {
        return Some(exe);
    }
    main_exe_in(install_location)
}

// ---------- 扫描 ----------

pub fn scan(ignored: &HashSet<String>, include_all: bool) -> BloatwareScan {
    let procs = running_processes();
    // 自启命令一次性枚举（小写）：Run键/启动文件夹 + 服务 ImagePath + 计划任务 Command。
    // 服务/计划任务是"持久信号"——软件当时跑不跑都能被发现，保证体检结果一致可信。
    let mut starts: Vec<String> = crate::startup::list_items()
        .into_iter()
        .map(|it| it.command.to_lowercase())
        .collect();
    starts.extend(service_commands());
    starts.extend(scheduled_task_commands());

    let mut entries: Vec<BloatwareEntry> = Vec::new();

    for app in list_installed() {
        // 有效目录：InstallLocation 为空时退回卸载器/图标父目录，保证后台检测不漏（豆包等）
        let loc = effective_location(&app.install_location, &app.uninstall_string, &app.display_icon);
        let autostart_count = autostart_count_for(&loc, &starts);
        let resident = resident_mem_mb(&loc, &procs);
        let large = app.size_mb.map(|m| m >= LARGE_MB).unwrap_or(false);
        let security = is_security(&app.name, &app.publisher);

        // 默认只列有值得关注行为的软件；安全软件即使当时无行为也列出（单独归类）；include_all 时全列
        if !include_all && !security && autostart_count == 0 && resident == 0 && !large {
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
        let icon_src = resolve_icon_path(&app.display_icon, &app.uninstall_string, &loc);
        let icon = icon_src.as_deref().and_then(crate::icon::from_file);
        entries.push(BloatwareEntry {
            id: app.id,
            trusted: is_trusted(&app.name, &app.publisher),
            security,
            dismissed: ignored.contains(&key),
            key,
            name: app.name,
            publisher: app.publisher,
            version: app.version,
            install_location: loc,
            size_mb: app.size_mb,
            icon,
            tags,
            behaviors,
            suggestion: "如果你不常用它，卸载可减少开机负担、释放空间；常用则保留即可。".to_string(),
            uninstallable: !app.uninstall_string.is_empty(),
            autostart_count,
            resident_mem_mb: resident,
        });
    }

    // 展示排序：优先让用户"认得出"的软件浮上来，避免主流常用软件沉到最底才看到。
    // 分层：① 有图标的在前；② 名称含中文的在前；③ 同层内再按负担降序（自启/常驻/体积）。
    entries.sort_by(|a, b| {
        let has_cjk = |s: &str| s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c));
        let burden = |e: &BloatwareEntry| {
            e.autostart_count as f64 * 200.0 + e.resident_mem_mb as f64 + e.size_mb.unwrap_or(0.0)
        };
        b.icon.is_some()
            .cmp(&a.icon.is_some())
            .then_with(|| has_cjk(&b.name).cmp(&has_cjk(&a.name)))
            .then_with(|| {
                burden(b)
                    .partial_cmp(&burden(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
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

/// 规整卸载命令：给"未加引号但含空格的 exe 路径"补引号，避免 cmd 在空格处截断
/// （鲁大师等软件的 UninstallString 常不带引号，是"点了没反应"的元凶）。
fn normalize_cmd(cmd: &str) -> String {
    let c = cmd.trim();
    if c.starts_with('"') {
        return c.to_string();
    }
    if let Some(pos) = c.to_lowercase().find(".exe") {
        let end = pos + 4;
        let (path, args) = c.split_at(end);
        if path.contains(' ') {
            return format!("\"{}\"{}", path, args);
        }
    }
    c.to_string()
}

/// 据 id 读取该软件的"有效目录"（用于停止后台运行/强力卸载/残留定位）。
/// InstallLocation 为空时退回卸载器/图标的父目录，兜住豆包等目录缺失的情况。
fn install_loc_of(id: &str) -> String {
    let Some((tag, sub)) = id.split_once(SEP) else { return String::new() };
    let Some((_, hive, path)) = HIVES.iter().find(|(t, _, _)| *t == tag) else { return String::new() };
    let Some(k) = RegKey::predef(*hive)
        .open_subkey(path)
        .ok()
        .and_then(|key| key.open_subkey(sub).ok())
    else {
        return String::new();
    };
    let install = read_str(&k, "InstallLocation");
    let uninstall = {
        let s = read_str(&k, "QuietUninstallString");
        if s.is_empty() { read_str(&k, "UninstallString") } else { s }
    };
    let icon = read_str(&k, "DisplayIcon");
    effective_location(&install, &uninstall, &icon)
}

/// 停止某安装目录下的相关服务与进程（**提权**执行，破服务常驻/自我保护）。
pub fn stop_software(install_location: &str) -> Result<(), String> {
    if install_location.trim().is_empty() {
        return Ok(());
    }
    let script = format!("$ErrorActionPreference='SilentlyContinue'\n{}", ps_stop_block(install_location));
    run_elevated_ps(&script)
}

/// 静默参数（方法A）：MSI→/qn /norestart；其余追加 NSIS(/S)+Inno(/VERYSILENT...) 常见静默开关
/// （互不冲突，卸载器会忽略不认识的）。厂商自研卸载器可能都不认——那时走「强力卸载」。
fn silent_uninstall_cmd(cmd: &str) -> String {
    let base = normalize_cmd(cmd);
    let low = base.to_lowercase();
    if low.contains("msiexec") {
        let mut s = base;
        if !low.contains("/qn") && !low.contains("/quiet") {
            s.push_str(" /qn");
        }
        if !low.contains("/norestart") {
            s.push_str(" /norestart");
        }
        s
    } else {
        format!("{} /S /VERYSILENT /SUPPRESSMSGBOXES /NORESTART", base)
    }
}

/// 在提权环境中执行一条 Windows 命令行（原走独立 .bat，现改为内联，消除 TOCTOU）。
/// 关键点：卸载命令经**环境变量**传给 cmd，由 cmd 自行展开解析——
/// 1）不落地任何可被同账户进程篡改的临时文件；
/// 2）绕开 PowerShell 5.1 原生传参对内嵌引号的破坏；
/// 3）环境变量在 UTF-16 的提权 PowerShell 中赋值，中文/含空格路径均无损（替代原 GBK bat）。
fn run_cmd_block(cmd: &str) -> String {
    format!(
        "$env:DB_UNINST_CMD='{}'\n& cmd.exe /c '%DB_UNINST_CMD%'\n",
        ps_quote(cmd)
    )
}

/// 进度标记文件：提权授权通过、脚本真正开始执行后才会被创建，供前端轮询判断"已授权"。
fn op_marker() -> std::path::PathBuf {
    std::env::temp_dir().join("diskbutler-op-started.flag")
}

/// 提权操作是否已真正开始（用户已在 UAC 点"是"）。
pub fn op_started() -> bool {
    op_marker().exists()
}

fn run_elevated_ps(script: &str) -> Result<(), String> {
    let marker = op_marker();
    let _ = std::fs::remove_file(&marker);
    // 首行创建标记文件——它只会在 UAC 授权通过、脚本真正开始执行后产生
    let full = format!(
        "New-Item -ItemType File -Force -Path '{}' | Out-Null\n{}",
        op_marker().to_string_lossy().replace('\'', "''"),
        script
    );
    // 安全修复（docs/17 · TOCTOU 本地提权）：脚本经 UTF-16LE+Base64 编码后以 -EncodedCommand
    // **内联进提权进程命令行**，不再向 %TEMP% 落地可被同账户进程篡改的 .ps1；命令行由本进程
    // 直接构造传参，UAC 窗口期内无任何中间文件可改 → TOCTOU 窗口消失。
    let encoded = encode_ps_command(&full);
    let outer = format!(
        "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-EncodedCommand','{}'",
        encoded
    );
    let r = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &outer])
        .status();
    let _ = std::fs::remove_file(&marker);
    r.map(|_| ()).map_err(|e| format!("提权失败：{}", e))
}

/// 将脚本编码为 PowerShell `-EncodedCommand` 参数（UTF-16LE 后 Base64）。
/// 内联传参，避免落地脚本文件被篡改，同时天然承载中文（UTF-16）。
fn encode_ps_command(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    base64_encode(&utf16)
}

/// 标准 Base64 编码（无外部依赖，供 -EncodedCommand 内联提权脚本用）。
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// PowerShell 单引号转义。
fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

/// 停止安装目录下相关服务、结束进程的 PS 片段（提权环境执行）。
fn ps_stop_block(loc: &str) -> String {
    if loc.trim().is_empty() {
        return String::new();
    }
    format!(
        "$loc='{}'\nGet-CimInstance Win32_Service | ? {{ $_.PathName -and $_.PathName.ToLower().Contains($loc.ToLower()) }} | % {{ Stop-Service $_.Name -Force -EA SilentlyContinue }}\nGet-Process | ? {{ $_.Path -and $_.Path.ToLower().StartsWith($loc.ToLower()) }} | Stop-Process -Force -EA SilentlyContinue\nStart-Sleep -Milliseconds 300\n",
        ps_quote(loc)
    )
}

/// 方法A卸载：单次提权脚本 = 停服务 + 杀进程 + 静默运行官方卸载器。用户只授权一次。
pub fn uninstall(id: &str) -> Result<(), String> {
    let (_name, cmd) = uninstall_string_of(id).ok_or_else(|| "找不到该软件的卸载程序".to_string())?;
    let loc = install_loc_of(id);
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'\n{}{}",
        ps_stop_block(&loc),
        run_cmd_block(&silent_uninstall_cmd(&cmd))
    );
    run_elevated_ps(&script)?;
    if still_installed(id) {
        Err("卸载未完成（可能取消了授权，或该软件不支持静默卸载）。可试试「强力卸载」。".to_string())
    } else {
        Ok(())
    }
}

// ---------- 强力卸载（方法C：自己停/删服务、任务、目录、注册表） ----------

/// 前台打开软件自带的官方卸载程序（不提权、不静默、不置顶）——用于安全软件等
/// 有内核级自我保护、必须由用户在其自带流程里亲自走完的情况。卸载器会自行按需弹 UAC。
pub fn open_official_uninstaller(id: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let (_name, cmd) = uninstall_string_of(id).ok_or_else(|| "找不到该软件的卸载程序".to_string())?;
    std::process::Command::new("cmd")
        .arg("/C")
        .raw_arg(normalize_cmd(&cmd))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开卸载程序失败：{}", e))
}

/// 强力卸载预览：将删除的内容清单，供前端红色二次确认。
#[derive(serde::Serialize)]
pub struct ForcePlan {
    pub name: String,
    pub install_dir: String,
    pub dir_deletable: bool,
    pub services: Vec<String>,
    pub tasks: Vec<String>,
    pub reg_path: String,
}

/// ImagePath 位于安装目录下的服务名。
fn services_under(loc: &str) -> Vec<String> {
    let mut out = Vec::new();
    if loc.trim().is_empty() {
        return out;
    }
    let l = loc.trim_end_matches('\\').to_lowercase();
    if let Ok(services) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SYSTEM\CurrentControlSet\Services") {
        for name in services.enum_keys().flatten() {
            if let Ok(k) = services.open_subkey(&name) {
                if read_str(&k, "ImagePath").to_lowercase().contains(&l) {
                    out.push(name);
                }
            }
        }
    }
    out
}

/// 执行命令位于安装目录下的计划任务名（\路径\名）。
fn tasks_under(loc: &str) -> Vec<String> {
    if loc.trim().is_empty() {
        return Vec::new();
    }
    let l = loc.trim_end_matches('\\').to_lowercase();
    let root = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()))
        .join("System32")
        .join("Tasks");
    let mut out = Vec::new();
    fn walk(dir: &Path, root: &Path, l: &str, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, l, out);
            } else if let Ok(bytes) = std::fs::read(&p) {
                if decode_task(&bytes).to_lowercase().contains(l) {
                    if let Ok(rel) = p.strip_prefix(root) {
                        out.push(format!("\\{}", rel.to_string_lossy()));
                    }
                }
            }
        }
    }
    walk(&root, &root, &l, &mut out);
    out
}

/// 安装目录是否可安全删除（存在、是目录、位于常规安装根下、非盘根）。
fn dir_safe_to_delete(dir: &str) -> bool {
    if dir.trim().is_empty() {
        return false;
    }
    let path = Path::new(dir);
    if !path.is_dir() {
        return false;
    }
    let Ok(canon) = path.canonicalize() else { return false };
    install_roots().iter().any(|root| {
        root.canonicalize().map(|rc| canon.starts_with(&rc) && canon != rc).unwrap_or(false)
    })
}

/// 注册表卸载项的 PowerShell 路径（用于删除）。
fn reg_ps_path(id: &str) -> String {
    let Some((tag, sub)) = id.split_once(SEP) else { return String::new() };
    let (prefix, base) = match tag {
        "hklm64" => ("HKLM:", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        "hklm32" => ("HKLM:", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        "hkcu" => ("HKCU:", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        _ => return String::new(),
    };
    format!("{}\\{}\\{}", prefix, base, sub)
}

/// 强力卸载预览：列出将删除的目录/服务/计划任务/注册表项。
pub fn force_preview(id: &str) -> ForcePlan {
    let name = uninstall_string_of(id).map(|(n, _)| n).unwrap_or_default();
    let dir = install_loc_of(id);
    ForcePlan {
        name,
        dir_deletable: dir_safe_to_delete(&dir),
        services: services_under(&dir),
        tasks: tasks_under(&dir),
        reg_path: reg_ps_path(id),
        install_dir: dir,
    }
}

/// 强力卸载执行：单次提权脚本——停/删服务→删计划任务→杀进程→(先尝试官方静默卸载)→删目录→删注册表项。
pub fn force_uninstall(id: &str) -> Result<(), String> {
    let loc = install_loc_of(id);
    let reg = reg_ps_path(id);
    let bat_line = if let Some((_, cmd)) = uninstall_string_of(id) {
        format!("{}Start-Sleep -Milliseconds 500\n", run_cmd_block(&silent_uninstall_cmd(&cmd)))
    } else {
        String::new()
    };
    let mut s = String::from("$ErrorActionPreference='SilentlyContinue'\n");
    if !loc.trim().is_empty() {
        s.push_str(&format!("$loc='{}'\n", ps_quote(&loc)));
        s.push_str("Get-CimInstance Win32_Service | ? { $_.PathName -and $_.PathName.ToLower().Contains($loc.ToLower()) } | % { Stop-Service $_.Name -Force -EA SilentlyContinue; sc.exe delete $_.Name | Out-Null }\n");
        s.push_str("Get-ScheduledTask | ? { (($_.Actions | % { $_.Execute }) -join ' ') -match [Regex]::Escape($loc) } | % { Unregister-ScheduledTask -TaskName $_.TaskName -TaskPath $_.TaskPath -Confirm:$false }\n");
        s.push_str("Get-Process | ? { $_.Path -and $_.Path.ToLower().StartsWith($loc.ToLower()) } | Stop-Process -Force -EA SilentlyContinue\nStart-Sleep -Milliseconds 300\n");
    }
    s.push_str(&bat_line);
    if dir_safe_to_delete(&loc) {
        s.push_str("if (Test-Path -LiteralPath $loc) { Remove-Item -LiteralPath $loc -Recurse -Force -EA SilentlyContinue }\n");
    }
    if !reg.is_empty() {
        s.push_str(&format!("Remove-Item -LiteralPath '{}' -Recurse -Force -EA SilentlyContinue\n", ps_quote(&reg)));
    }
    run_elevated_ps(&s)?;
    if still_installed(id) {
        Err("强力卸载可能未完全清除（部分文件被占用或需重启）。重启后可再扫描确认。".to_string())
    } else {
        Ok(())
    }
}

// ---------- 卸载后残留清理（白名单式，严格路径校验） ----------
//
// 参考 Geek Uninstaller 的"卸载后深度清理"：官方卸载器跑完后，自动扫描
// 文件残留 + 注册表残留，列成清单让用户确认后清除。
// 本实现的安全边界（宁可漏不可误伤）：
// - 文件残留只认「安装根下与软件名/安装目录名**完全同名**的一级目录」，不做子串匹配；
// - 排除共享厂商目录（Microsoft/Tencent 等多产品共用，删了殃及在装软件）；
// - 排除仍被其他在装软件的 InstallLocation 引用的目录；
// - 注册表只清当前用户 HKCU\Software 下的精确同名键（用户可写、无需二次提权；HKLM 不碰）。

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

/// 容错删除整个目录（含目录本身）：逐个删除文件/子目录，被占用或拒绝访问的自动跳过，
/// 尽最大努力删掉能删的部分。与 remove_dir_all 的"全有或全无"不同——避免单个被占用文件
/// 导致整目录清理失败（如刚卸载完 WPS，云同步后台仍锁着个别文件）。
fn robust_remove_dir(path: &Path) {
    let Ok(read) = std::fs::read_dir(path) else { return };
    for entry in read.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if std::fs::remove_dir_all(&p).is_err() {
                robust_remove_dir(&p);
                let _ = std::fs::remove_dir(&p);
            }
        } else {
            let _ = std::fs::remove_file(&p);
        }
    }
    let _ = std::fs::remove_dir(path);
}

/// 多产品共用的厂商/系统目录名：与软件同名也绝不作为残留候选。
fn shared_vendor_name(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "microsoft"
            | "google"
            | "tencent"
            | "kingsoft"
            | "adobe"
            | "intel"
            | "nvidia"
            | "amd"
            | "common files"
            | "windows"
            | "packages"
            | "programs"
            | "temp"
            | "classes"
            | "policies"
            | "wow6432node"
            | "clients"
    )
}

/// 残留匹配候选名：软件显示名 + 安装目录叶名。
/// 过滤：太短的（≥3 字符才够独特）、共享厂商名、含路径非法字符的。
fn residue_name_candidates(name: &str, install_location: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        let s = s.trim();
        if s.chars().count() < 3 || shared_vendor_name(s) {
            return;
        }
        if s.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
            return;
        }
        if !out.iter().any(|x| x.eq_ignore_ascii_case(s)) {
            out.push(s.to_string());
        }
    };
    push(name);
    if let Some(leaf) = Path::new(install_location.trim_end_matches('\\')).file_name() {
        push(&leaf.to_string_lossy());
    }
    out
}

/// 目录是否仍被某个在装软件的 InstallLocation 引用（是其本身或祖先）——删了会伤及在装软件。
fn referenced_by_installed(dir: &Path) -> bool {
    let d = dir.to_string_lossy().to_lowercase().trim_end_matches('\\').to_string();
    list_installed().iter().any(|a| {
        let loc = a.install_location.to_lowercase();
        let loc = loc.trim_end_matches('\\');
        !loc.is_empty() && (loc == d || loc.starts_with(&format!("{}\\", d)))
    })
}

/// HKCU\Software 下的残留注册表键（返回展示用完整路径，如 `HKCU\Software\XX`）。
/// 只认两种精确形态：`Software\<候选名>`、`Software\<发行商>\<候选名>`（二级删叶保父）。
fn residue_reg_keys(name: &str, publisher: &str, install_location: &str) -> Vec<String> {
    let cands = residue_name_candidates(name, install_location);
    let Ok(software) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software") else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for c in &cands {
        if software.open_subkey(c).is_ok() {
            out.push(format!(r"HKCU\Software\{}", c));
        }
    }
    let publisher = publisher.trim();
    if !publisher.is_empty() && !publisher.contains('\\') {
        if let Ok(vendor) = software.open_subkey(publisher) {
            for c in &cands {
                if vendor.open_subkey(c).is_ok() {
                    out.push(format!(r"HKCU\Software\{}\{}", publisher, c));
                }
            }
        }
    }
    out
}

/// 卸载后残留全景扫描：文件目录（安装目录 + 各安装根下同名数据目录）+ HKCU 注册表键。
pub fn scan_residue(name: &str, publisher: &str, install_location: &str) -> ResidueScanReport {
    let mut dirs: Vec<ResidueDetail> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // 1) 安装目录本身
    if !install_location.is_empty() && is_safe_residue_path(install_location) {
        let size = dir_size(Path::new(install_location));
        if size > 0 {
            seen.insert(install_location.to_lowercase());
            dirs.push(ResidueDetail { path: install_location.to_string(), size });
        }
    }
    // 2) 各安装根下与候选名完全同名的一级目录（数据/配置残留）
    for root in install_roots() {
        for c in residue_name_candidates(name, install_location) {
            let p = root.join(&c);
            let disp = p.to_string_lossy().to_string();
            if seen.contains(&disp.to_lowercase()) || !is_safe_residue_path(&disp) {
                continue;
            }
            if referenced_by_installed(&p) {
                continue;
            }
            let size = dir_size(&p);
            if size == 0 {
                continue;
            }
            seen.insert(disp.to_lowercase());
            dirs.push(ResidueDetail { path: disp, size });
        }
    }
    dirs.sort_by(|a, b| b.size.cmp(&a.size));
    ResidueScanReport {
        dirs,
        reg_keys: residue_reg_keys(name, publisher, install_location),
    }
}

/// 清理残留：目录与注册表键都必须命中「重新扫描得到的白名单」才执行——
/// 前端只回传选中项，实际允许集合由本函数当场重新推导，杜绝任意路径/键删除。
pub fn clean_residue(
    name: &str,
    publisher: &str,
    install_location: &str,
    dirs: Vec<String>,
    reg_keys: Vec<String>,
) -> ResidueReport {
    let allowed = scan_residue(name, publisher, install_location);
    let allowed_dirs: HashSet<String> = allowed.dirs.iter().map(|d| d.path.to_lowercase()).collect();
    let allowed_regs: HashSet<String> = allowed.reg_keys.iter().map(|k| k.to_lowercase()).collect();

    let mut freed = 0u64;
    let mut errors = Vec::new();
    for p in dirs {
        if !allowed_dirs.contains(&p.to_lowercase()) || !is_safe_residue_path(&p) {
            errors.push(format!("已跳过（路径不在允许范围）：{}", p));
            continue;
        }
        // 容错删除：能删的先删，被占用/拒绝访问的单个文件跳过，不让整目录一票否决。
        let before = dir_size(Path::new(&p));
        robust_remove_dir(Path::new(&p));
        let after = dir_size(Path::new(&p));
        freed += before.saturating_sub(after);
        if after > 0 {
            errors.push(format!(
                "「{}」还有部分文件正被占用没删掉——请先彻底退出该软件（含托盘/后台服务），或重启电脑后再清理一次。",
                p
            ));
        }
    }
    for k in reg_keys {
        if !allowed_regs.contains(&k.to_lowercase()) {
            errors.push(format!("已跳过（注册表键不在允许范围）：{}", k));
            continue;
        }
        let Some(sub) = k.strip_prefix(r"HKCU\Software\") else {
            errors.push(format!("已跳过（非 HKCU\\Software 键）：{}", k));
            continue;
        };
        let r = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software")
            .and_then(|s| s.delete_subkey_all(sub));
        if let Err(e) = r {
            errors.push(format!("删除注册表键失败 {}：{}", k, e));
        }
    }
    ResidueReport { freed, errors }
}

// ---------- AppData 孤儿残留检测（已卸载软件遗留目录，"回头清存量"） ----------
//
// 与"卸载后残留清理"共用一套安全边界（install_roots/shared_vendor_name/容错删除）。
// 红线（宁漏报不误报）：
// - 只有命中「目录名→软件」确证映射知识库、且无任何在装软件认领的目录才可清理；
// - 在装判定从宽：目录名/别名与任何在装软件的名称、发行商、安装路径有包含关系即视为有主；
// - 厂商级共享目录永不判；未知大目录只列出、不提供删除；
// - 已被磁盘透视知识库专项规则认识的目录（npm/pip/JetBrains 等）不当"未知"列出，降噪。

const ORPHAN_DIRS_RAW: &str = include_str!("data/orphan-dirs.txt");

struct OrphanRule {
    dir: String,
    app: String,
    aliases: Vec<String>,
    /// 警示语（可选）：含云端同步用户资产的条目必须带，UI 醒目展示引导先确认再删
    note: String,
}

/// 解析目录映射知识库（首次调用后缓存）。行格式：目录名|软件名|别名1,别名2|警示语(可选)
fn orphan_rules() -> &'static Vec<OrphanRule> {
    static RULES: OnceLock<Vec<OrphanRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        ORPHAN_DIRS_RAW
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| {
                let mut parts = l.split('|');
                let dir = parts.next()?.trim().to_string();
                let app = parts.next()?.trim().to_string();
                let aliases: Vec<String> = parts
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(|a| a.trim().to_lowercase())
                    .filter(|a| !a.is_empty())
                    .collect();
                let note = parts.next().unwrap_or("").trim().to_string();
                if dir.is_empty() || app.is_empty() || shared_vendor_name(&dir) {
                    return None;
                }
                Some(OrphanRule { dir, app, aliases, note })
            })
            .collect()
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanEntry {
    pub path: String,
    /// 归属软件人话名（未知目录为空字符串）
    pub app_name: String,
    pub size: u64,
    /// 警示语（空 = 普通缓存/配置残留；非空 = 含云端同步资产，删前需用户确认备份）
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanScan {
    /// 知识库确证 + 无在装软件认领：可勾选清理（本功能只显示确证残留，不再猜"归属不明"）
    pub confirmed: Vec<OrphanEntry>,
}

/// 在装软件是否认领该目录（从宽匹配，命中即"有主"——放过是安全方向）。
fn owner_installed(dir_name: &str, aliases: &[String], installed: &[(String, String, String)]) -> bool {
    let d = dir_name.to_lowercase();
    installed.iter().any(|(name, publisher, loc)| {
        (name.len() >= 3 && d.len() >= 3 && (name.contains(&d) || d.contains(name.as_str())))
            || (!publisher.is_empty() && publisher.contains(&d))
            || (!loc.is_empty() && loc.contains(&d))
            || aliases.iter().any(|a| {
                name.contains(a.as_str())
                    || (!publisher.is_empty() && publisher.contains(a.as_str()))
                    || (!loc.is_empty() && loc.contains(a.as_str()))
            })
    })
}

/// 扫描孤儿残留：遍历三大安装根的一级目录，只报告"知识库确证 + 主人已卸载"的真残留。
pub fn scan_orphans() -> OrphanScan {
    // 在装软件三元组（名称/发行商/有效目录，全小写）一次性备好
    let installed: Vec<(String, String, String)> = list_installed()
        .into_iter()
        .map(|a| {
            let loc = effective_location(&a.install_location, &a.uninstall_string, &a.display_icon);
            (a.name.to_lowercase(), a.publisher.to_lowercase(), loc.to_lowercase())
        })
        .collect();
    // 运行中进程 exe 路径（小写），作为"绿色/免安装软件不进注册表"的兜底：进程在跑 = 有主
    let running: Vec<String> = running_processes().into_iter().map(|(exe, _)| exe).collect();

    let mut confirmed: Vec<OrphanEntry> = Vec::new();

    for root in install_roots() {
        // 只看 AppData/ProgramData 三大根；Program Files 归安装目录残留管，不在此扫
        let root_str = root.to_string_lossy().to_lowercase();
        if root_str.contains("program files") {
            continue;
        }
        let Ok(read) = std::fs::read_dir(&root) else { continue };
        for e in read.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let dir_name = e.file_name().to_string_lossy().to_string();
            if shared_vendor_name(&dir_name) {
                continue;
            }
            // 只报告知识库确证的目录；其余（在装/缓存/未知）一律不进本功能
            let Some(rule) = orphan_rules().iter().find(|r| r.dir.eq_ignore_ascii_case(&dir_name)) else {
                continue;
            };
            if owner_installed(&dir_name, &rule.aliases, &installed) {
                continue; // 注册表在装，绝不判孤儿
            }
            // 兜底：绿色/免安装软件不进注册表——若有运行进程路径命中目录名或别名，视为有主
            let dl = dir_name.to_lowercase();
            let alias_hit = |exe: &str| exe.contains(&dl) || rule.aliases.iter().any(|a| exe.contains(a.as_str()));
            if running.iter().any(|exe| alias_hit(exe)) {
                continue;
            }
            let disp = p.to_string_lossy().to_string();
            let size = dir_size(&p);
            if size == 0 {
                continue;
            }
            confirmed.push(OrphanEntry {
                path: disp,
                app_name: rule.app.clone(),
                size,
                note: rule.note.clone(),
            });
        }
    }
    confirmed.sort_by(|a, b| b.size.cmp(&a.size));
    OrphanScan { confirmed }
}

/// 清理孤儿目录：只接受「当场重扫得到的确证白名单」内的路径，容错删除。
pub fn clean_orphans(paths: Vec<String>) -> ResidueReport {
    let allowed: HashSet<String> = scan_orphans()
        .confirmed
        .iter()
        .map(|o| o.path.to_lowercase())
        .collect();
    let mut freed = 0u64;
    let mut errors = Vec::new();
    for p in paths {
        if !allowed.contains(&p.to_lowercase()) || !is_safe_residue_path(&p) {
            errors.push(format!("已跳过（不在可清理名单）：{}", p));
            continue;
        }
        let before = dir_size(Path::new(&p));
        robust_remove_dir(Path::new(&p));
        let after = dir_size(Path::new(&p));
        freed += before.saturating_sub(after);
        if after > 0 {
            errors.push(format!("「{}」还有部分文件正被占用没删掉——可重启电脑后再清理一次。", p));
        }
    }
    ResidueReport { freed, errors }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residue_candidates_exclude_shared_and_short_names() {
        // 共享厂商名绝不作为候选（与 Tencent 同名的目录删了殃及全家桶）
        assert!(residue_name_candidates("Tencent", r"C:\Program Files\Tencent").is_empty());
        // 太短的名字不够独特，不作为候选
        assert!(residue_name_candidates("QQ", "").is_empty());
        // 正常软件：显示名 + 安装目录叶名去重后各留一份
        let c = residue_name_candidates("Doubao", r"C:\Users\x\AppData\Local\Doubao");
        assert_eq!(c, vec!["Doubao".to_string()]);
        // 两字 CJK 显示名（如"豆包"）同样低于长度阈值被过滤——短名误伤风险大，
        // 此时只认安装目录叶名
        let c = residue_name_candidates("豆包", r"C:\Users\x\AppData\Local\Doubao");
        assert_eq!(c, vec!["Doubao".to_string()]);
        // 含路径非法字符的显示名（如带斜杠的版本描述）被过滤
        assert!(residue_name_candidates("A/B: Tool?", "").is_empty());
    }

    #[test]
    fn residue_scan_rejects_paths_outside_roots() {
        // 不在 5 个安装根之下的目录（如用户主目录/盘根）永不列为残留
        assert!(!is_safe_residue_path(r"C:\"));
        assert!(!is_safe_residue_path(r"C:\Windows\System32"));
        let home = std::env::var("USERPROFILE").unwrap();
        assert!(!is_safe_residue_path(&format!(r"{}\Documents", home)));
    }

    #[test]
    fn clean_residue_only_accepts_rescanned_whitelist() {
        // 前端回传任意路径/注册表键时，必须因不在重扫白名单内而被跳过
        let r = clean_residue(
            "NonexistentApp-DiskButlerTest",
            "",
            "",
            vec![r"C:\Windows\System32".to_string()],
            vec![r"HKCU\Software\Microsoft".to_string(), r"HKLM\SOFTWARE\Whatever".to_string()],
        );
        assert_eq!(r.freed, 0);
        assert_eq!(r.errors.len(), 3, "三个非法项都应被跳过并记录：{:?}", r.errors);
        assert!(r.errors.iter().all(|e| e.starts_with("已跳过")));
    }

    #[test]
    fn residue_reg_keys_only_under_hkcu_software() {
        // 返回的键必须全部锚定在 HKCU\Software\ 前缀下
        for k in residue_reg_keys("SomeApp", "SomeVendor", "") {
            assert!(k.starts_with(r"HKCU\Software\"), "越界键：{}", k);
        }
    }

    #[test]
    fn orphan_rules_parse_and_exclude_shared_vendors() {
        let rules = orphan_rules();
        assert!(!rules.is_empty(), "孤儿映射知识库不应为空");
        for r in rules.iter() {
            // 知识库条目本身绝不允许收厂商级共享目录名
            assert!(!shared_vendor_name(&r.dir), "共享厂商目录混入知识库：{}", r.dir);
            assert!(!r.app.is_empty());
        }
        // 抽查：豆包条目应带中英双别名（防止中文显示名对不上英文目录名）
        let doubao = rules.iter().find(|r| r.dir == "Doubao").expect("应有 Doubao 条目");
        assert!(doubao.aliases.iter().any(|a| a == "豆包"));
        // pcsuite（vivo 办公套件）：含云端同步用户文档，警示语字段绝不允许为空
        let pcsuite = rules.iter().find(|r| r.dir == "pcsuite").expect("应有 pcsuite 条目");
        assert!(!pcsuite.note.is_empty(), "pcsuite 必须带警示语（含云笔记同步文档）");
        assert!(pcsuite.aliases.iter().any(|a| a == "vivo"));
    }

    #[test]
    fn owner_matching_is_generous() {
        // 在装软件"豆包"（中文名）+ 有效目录含 doubao：目录 Doubao 必须被认领，绝不判孤儿
        let installed = vec![(
            "豆包".to_string(),
            "beijing chuntian zhixin".to_string(),
            r"c:\users\x\appdata\local\doubao\application".to_string(),
        )];
        let aliases = vec!["doubao".to_string(), "豆包".to_string()];
        assert!(owner_installed("Doubao", &aliases, &installed));
        // 路径命中：即使别名为空，安装路径含目录名也算有主
        assert!(owner_installed("Doubao", &[], &installed));
        // 无任何关联的软件不认领
        let other = vec![("zoom".to_string(), "zoom".to_string(), String::new())];
        assert!(!owner_installed("Doubao", &aliases, &other));
    }

    #[test]
    fn clean_orphans_rejects_paths_outside_whitelist() {
        // 任意路径必须因不在"当场重扫确证名单"而被跳过
        let r = clean_orphans(vec![r"C:\Windows\System32".to_string(), r"C:\".to_string()]);
        assert_eq!(r.freed, 0);
        assert_eq!(r.errors.len(), 2);
        assert!(r.errors.iter().all(|e| e.starts_with("已跳过")));
    }

    /// 诊断用：真机打印 scan_orphans 实际结果（结果依赖本机环境，不做断言）。
    /// 运行：cargo test debug_print_scan_orphans -- --ignored --nocapture
    #[test]
    #[ignore]
    fn debug_print_scan_orphans() {
        let s = scan_orphans();
        println!("== confirmed ({}) ==", s.confirmed.len());
        for o in &s.confirmed {
            println!("  {} <- {} ({}MB)", o.path, o.app_name, o.size / 1024 / 1024);
        }
    }

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
    fn base64_encode_matches_known_vectors() {
        // 标准 RFC 4648 向量，覆盖三种补位（无补、单补、双补）
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encoded_ps_command_is_utf16le_base64() {
        // -EncodedCommand 期望 UTF-16LE 后 Base64；此处校验中文脚本编码正确、可被 PowerShell 解回。
        // "A中" => UTF-16LE 字节 41 00 2D 4E => Base64 "QQAtTg=="
        assert_eq!(encode_ps_command("A中"), "QQAtTg==");
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

    #[test]
    fn exe_of_handles_quoted_and_msi() {
        assert_eq!(exe_of("\"C:\\Program Files\\Foo\\uninst.exe\" /S"), "C:\\Program Files\\Foo\\uninst.exe");
        assert_eq!(exe_of("MsiExec.exe /X{ABC}"), "MsiExec.exe");
    }

    #[test]
    fn normalize_cmd_quotes_unquoted_spaced_exe() {
        assert_eq!(
            normalize_cmd(r"C:\Program Files\Foo\uninst.exe /S"),
            "\"C:\\Program Files\\Foo\\uninst.exe\" /S"
        );
        assert_eq!(normalize_cmd("\"C:\\Program Files\\Foo\\u.exe\""), "\"C:\\Program Files\\Foo\\u.exe\"");
        assert_eq!(normalize_cmd("MsiExec.exe /X{ABC}"), "MsiExec.exe /X{ABC}");
    }
}
