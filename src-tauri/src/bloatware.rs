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

/// 枚举所有 Windows 服务的 ImagePath（小写），作为持久自启信号之一。
fn service_commands() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(services) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SYSTEM\CurrentControlSet\Services") {
        for name in services.enum_keys().flatten() {
            if let Ok(k) = services.open_subkey(&name) {
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
        let autostart_count = autostart_count_for(&app.install_location, &starts);
        let resident = resident_mem_mb(&app.install_location, &procs);
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
        let icon_src = resolve_icon_path(&app.display_icon, &app.uninstall_string, &app.install_location);
        let icon = icon_src.as_deref().and_then(crate::icon::from_file);
        // 打开位置：优先 InstallLocation，缺失则用图标来源文件所在目录兜底
        let open_dir = if !app.install_location.trim().is_empty() {
            app.install_location.clone()
        } else {
            icon_src
                .as_deref()
                .and_then(|p| Path::new(p).parent())
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default()
        };
        entries.push(BloatwareEntry {
            id: app.id,
            trusted: is_trusted(&app.name, &app.publisher),
            security,
            dismissed: ignored.contains(&key),
            key,
            name: app.name,
            publisher: app.publisher,
            version: app.version,
            install_location: open_dir,
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

/// 据 id 读取该软件的安装目录（用于停止后台运行）。
fn install_loc_of(id: &str) -> String {
    let Some((tag, sub)) = id.split_once(SEP) else { return String::new() };
    let Some((_, hive, path)) = HIVES.iter().find(|(t, _, _)| *t == tag) else { return String::new() };
    RegKey::predef(*hive)
        .open_subkey(path)
        .ok()
        .and_then(|key| key.open_subkey(sub).ok())
        .map(|k| read_str(&k, "InstallLocation"))
        .unwrap_or_default()
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

/// 把卸载命令写入 GBK 批处理（正确处理中文路径），返回路径。
fn write_uninstall_bat(cmd: &str) -> Result<std::path::PathBuf, String> {
    let bat = std::env::temp_dir().join("diskbutler-uninstall.bat");
    let content = format!("@echo off\r\n{}\r\n", cmd);
    let (bytes, _, _) = encoding_rs::GBK.encode(&content);
    std::fs::write(&bat, &bytes).map_err(|e| format!("准备卸载失败：{}", e))?;
    Ok(bat)
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
    let ps1 = std::env::temp_dir().join("diskbutler-op.ps1");
    // 首行创建标记文件——它只会在 UAC 授权通过、脚本真正开始执行后产生
    let full = format!(
        "New-Item -ItemType File -Force -Path '{}' | Out-Null\n{}",
        op_marker().to_string_lossy().replace('\'', "''"),
        script
    );
    let mut bytes = vec![0xEFu8, 0xBB, 0xBF];
    bytes.extend_from_slice(full.as_bytes());
    std::fs::write(&ps1, &bytes).map_err(|e| format!("准备脚本失败：{}", e))?;
    let outer = format!(
        "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        ps1.display()
    );
    let r = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &outer])
        .status();
    let _ = std::fs::remove_file(&ps1);
    let _ = std::fs::remove_file(&marker);
    r.map(|_| ()).map_err(|e| format!("提权失败：{}", e))
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
    let bat = write_uninstall_bat(&silent_uninstall_cmd(&cmd))?;
    let bat_path = bat.to_string_lossy().to_string();
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'\n{}& cmd.exe /c '{}'\n",
        ps_stop_block(&loc),
        ps_quote(&bat_path)
    );
    run_elevated_ps(&script)?;
    let _ = std::fs::remove_file(&bat);
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
        let bat = write_uninstall_bat(&silent_uninstall_cmd(&cmd))?;
        let bat_path = bat.to_string_lossy().to_string();
        format!("& cmd.exe /c '{}'\nStart-Sleep -Milliseconds 500\n", ps_quote(&bat_path))
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
