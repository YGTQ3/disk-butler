//! 软件卸载引擎（类 Geek Uninstaller）：枚举已装软件、调用其原生卸载器、
//! 扫描并清理卸载后的残留（文件夹 + 注册表项）。
//!
//! 安全原则与全项目一致：前端只传回软件 id 与「后端扫出的」残留路径，
//! 实际操作前后端都会重新解析校验；文件删入回收站可恢复，注册表删除前先导出 .reg 备份。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 三个标准卸载注册表位置（64 位机器软件分散在这三处）。
/// 元组：(hive, 子路径, hive 文本标签用于 reg export)
const UNINSTALL_HIVES: [(winreg::HKEY, &str, &str); 3] = [
    (
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM",
    ),
    (
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM",
    ),
    (
        HKEY_CURRENT_USER,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKCU",
    ),
];

/// 一个已安装软件（序列化给前端）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    /// 注册表子键名（如 GUID 或应用名），用于后端重新定位
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub install_location: String,
    /// 图标来源（DisplayIcon，去掉资源索引后缀），前端可选用
    pub icon_path: String,
    /// EstimatedSize 换算的字节数（注册表以 KB 记，可能为 0）
    pub size: u64,
    /// 是否具备静默卸载串（前端仅作提示，本版一律走向导）
    pub has_quiet: bool,
    /// UninstallString（图标兑底时用于定位卸载器/主程序目录）
    pub uninstall_string: String,
}

/// 卸载后扫描出的单个残留项。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverItem {
    /// "dir" 文件夹 | "reg" 注册表项
    pub kind: String,
    /// 文件夹绝对路径，或注册表全路径（如 HKCU\Software\Foo）
    pub path: String,
    /// 文件夹字节数；注册表项为 0
    pub size: u64,
    /// "high" 高置信度（安装目录残留/名称精确匹配）| "low" 需谨慎
    pub confidence: String,
}

/// 前端回传的待删残留（与 LeftoverItem 的 kind/path 对应）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverInput {
    pub kind: String,
    pub path: String,
}

/// 单项残留删除结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveResult {
    pub path: String,
    pub ok: bool,
    pub error: Option<String>,
}

// ---------- 工具 ----------

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

/// 目录递归字节数（残留项通常不大，直接走 std 遍历即可）。
fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(read) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in read.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            total += dir_size(&entry.path());
        } else if let Ok(m) = entry.metadata() {
            total += m.len();
        }
    }
    total
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// 过于宽泛、匹配到会误伤共享目录的名字（厂商大类/系统词）。
fn is_generic(name: &str) -> bool {
    matches!(
        norm(name).as_str(),
        "" | "microsoft"
            | "windows"
            | "common files"
            | "common"
            | "google"
            | "intel"
            | "nvidia"
            | "amd"
            | "realtek"
            | "temp"
            | "cache"
            | "data"
    )
}

/// 去掉软件名尾部的版本号（如 "ocs desktop 2.8.5" -> "ocs desktop"），用于匹配不带版本的注册表键。
fn strip_version_suffix(s: &str) -> String {
    let mut parts: Vec<&str> = s.split_whitespace().collect();
    while let Some(last) = parts.last() {
        let is_ver = last.chars().all(|c| c.is_ascii_digit() || c == '.' || c == 'v')
            && last.chars().any(|c| c.is_ascii_digit());
        if is_ver && parts.len() > 1 {
            parts.pop();
        } else {
            break;
        }
    }
    parts.join(" ")
}

/// 发行商标准化：去掉 "llc/inc/ltd/corporation/gmbh/co" 等后缀与标点，得到用于匹配注册表文件夹的核心名。
/// 如 "Google LLC" -> "google"，"JetBrains s.r.o." -> "jetbrains"。
fn norm_publisher(pubname: &str) -> String {
    let mut s = norm(pubname);
    for junk in [",", ".", "(", ")"] {
        s = s.replace(junk, " ");
    }
    let stop = [
        "llc", "inc", "ltd", "limited", "corporation", "corp", "co", "company", "gmbh", "srl",
        "sro", "ab", "bv", "kg", "llp", "pte", "technologies", "technology", "software",
    ];
    let kept: Vec<&str> = s.split_whitespace().filter(|w| !stop.contains(w)).collect();
    kept.join(" ")
}

/// 注册表子键名与软件名的匹配判定（cand 已小写）。宁缺勿误，不命中返回 None。
fn reg_name_match(cand: &str, name_l: &str, name_core: &str) -> Option<&'static str> {
    if is_generic(cand) {
        return None;
    }
    if cand == name_l {
        return Some("high");
    }
    if !name_core.is_empty() && name_core.len() >= 4 && cand == name_core {
        return Some("high");
    }
    // 软件名以该键名开头（键名足够长）：如 "ocs desktop 2.8.5" 匹配键名 "ocs desktop"
    if cand.len() >= 5 && name_l.starts_with(cand) {
        return Some("high");
    }
    None
}

// ---------- 枚举已装软件 ----------

/// 一条注册表卸载项里执行卸载所需的原始字段（内部用，不序列化）。
struct AppRaw {
    uninstall_string: String,
    quiet_string: String,
    msi: bool,
}

/// 枚举全部已安装的桌面软件，按名称排序、同名去重。
pub fn list_installed() -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (idx, (hive, path, _tag)) in UNINSTALL_HIVES.iter().enumerate() {
        let Ok(root) = RegKey::predef(*hive).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for sub in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&sub, KEY_READ) else {
                continue;
            };
            let name: String = match key.get_value("DisplayName") {
                Ok(n) => {
                    let n: String = n;
                    n.trim().to_string()
                }
                Err(_) => continue,
            };
            if name.is_empty() {
                continue;
            }
            if key.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
                continue;
            }
            if key.get_value::<String, _>("ParentKeyName").is_ok() {
                continue;
            }
            let release_type: String = key.get_value("ReleaseType").unwrap_or_default();
            if release_type.contains("Update") || release_type.contains("Hotfix") {
                continue;
            }
            let uninstall: String = key.get_value("UninstallString").unwrap_or_default();
            let quiet: String = key.get_value("QuietUninstallString").unwrap_or_default();
            if uninstall.trim().is_empty() && quiet.trim().is_empty() {
                continue;
            }

            let version: String = key.get_value("DisplayVersion").unwrap_or_default();
            let publisher: String = key.get_value("Publisher").unwrap_or_default();
            let install_location: String = key.get_value("InstallLocation").unwrap_or_default();
            let icon: String = key.get_value("DisplayIcon").unwrap_or_default();
            let est_kb = key.get_value::<u32, _>("EstimatedSize").unwrap_or(0);

            // 去重键：名称 + 版本（WOW6432Node 与主键常重复登记同一软件）
            let dedup = format!("{}\u{1}{}", norm(&name), norm(&version));
            if !seen.insert(dedup) {
                continue;
            }

            apps.push(InstalledApp {
                // id 编码 hive 序号，保证全局唯一（同一子键名可能出现在多个 hive）
                id: format!("{}|{}", idx, sub),
                name,
                version: version.trim().to_string(),
                publisher: publisher.trim().to_string(),
                install_location: install_location.trim().to_string(),
                icon_path: icon.trim().to_string(),
                size: (est_kb as u64) * 1024,
                has_quiet: !quiet.trim().is_empty(),
                uninstall_string: uninstall.trim().to_string(),
            });
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// 枚举可卸载的 UWP/商店应用（Get-AppxPackage），解析友好名与包内 logo。
/// 过滤框架包/系统包/不可卸载项；DisplayName 为 ms-resource 时退回包名末段。
pub fn list_uwp() -> Vec<InstalledApp> {
    use std::os::windows::process::CommandExt;
    let script = r#"[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
$ErrorActionPreference='SilentlyContinue'
$apps = Get-AppxPackage | Where-Object { -not $_.IsFramework -and -not $_.NonRemovable -and $_.SignatureKind -ne 'System' }
$list = foreach ($p in $apps) {
  $dn=''; $logo=''; $pub=''
  $mf = Join-Path $p.InstallLocation 'AppxManifest.xml'
  if ($p.InstallLocation -and (Test-Path -LiteralPath $mf)) {
    try {
      [xml]$x = Get-Content -LiteralPath $mf -Raw -Encoding UTF8
      $dn = [string]$x.Package.Properties.DisplayName
      $pub = [string]$x.Package.Properties.PublisherDisplayName
      $logo = [string]$x.Package.Properties.Logo
      $vdn = [string]$x.Package.Applications.Application.VisualElements.DisplayName
      if (($dn -eq '' -or $dn -like 'ms-resource:*') -and $vdn -and $vdn -notlike 'ms-resource:*') { $dn = $vdn }
    } catch {}
  }
  if (-not $dn -or $dn -like 'ms-resource:*') { $dn = ($p.Name -split '\.')[-1] }
  $logoPath=''
  if ($p.InstallLocation -and $logo) {
    $ldir = Join-Path $p.InstallLocation (Split-Path $logo -Parent)
    $lbase = [System.IO.Path]::GetFileNameWithoutExtension($logo)
    $c = Get-ChildItem -LiteralPath $ldir -Filter "$lbase*.png" | Sort-Object Length -Descending | Select-Object -First 1
    if ($c) { $logoPath = $c.FullName }
  }
  if (-not $logoPath -and $p.InstallLocation) {
    $c = Get-ChildItem -LiteralPath $p.InstallLocation -Recurse -Depth 2 -Filter '*Square44x44Logo*.png' | Sort-Object Length -Descending | Select-Object -First 1
    if ($c) { $logoPath = $c.FullName }
  }
  [PSCustomObject]@{ id=$p.PackageFullName; name=$dn; publisher=$pub; install=$p.InstallLocation; logo=$logoPath }
}
$list | ConvertTo-Json -Compress"#;

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let Ok(out) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    // ConvertTo-Json：单个对象为 {}，多个为 []
    let val: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match val {
        serde_json::Value::Array(a) => a,
        v @ serde_json::Value::Object(_) => vec![v],
        _ => return Vec::new(),
    };
    let mut result = Vec::new();
    for item in arr {
        let get = |k: &str| {
            item.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let pfn = get("id");
        if pfn.is_empty() {
            continue;
        }
        let name = get("name");
        // 从 PackageFullName 抽版本：Name_Version_arch__hash
        let version = pfn.split('_').nth(1).unwrap_or("").to_string();
        result.push(InstalledApp {
            id: format!("uwp|{}", pfn),
            name: if name.is_empty() { pfn.clone() } else { name },
            version,
            publisher: get("publisher"),
            install_location: get("install"),
            icon_path: get("logo"),
            size: 0,
            has_quiet: false,
            uninstall_string: String::new(),
        });
    }
    result
}

/// 按 id 定位软件，返回执行卸载所需字段。
/// id 形如 "1|{GUID}"：剩 hive 序号 + 子键名；旧格式（无 "|"）则遗历三个 hive 兼容。
fn find_app(id: &str) -> Option<AppRaw> {
    let read = |hive: winreg::HKEY, path: &str, sub: &str| -> Option<AppRaw> {
        let root = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ).ok()?;
        let key = root.open_subkey_with_flags(sub, KEY_READ).ok()?;
        let name: String = key.get_value("DisplayName").unwrap_or_default();
        if name.trim().is_empty() {
            return None;
        }
        Some(AppRaw {
            uninstall_string: key.get_value("UninstallString").unwrap_or_default(),
            quiet_string: key.get_value("QuietUninstallString").unwrap_or_default(),
            msi: key.get_value::<u32, _>("WindowsInstaller").unwrap_or(0) == 1,
        })
    };

    // 新格式：hive 序号 | 子键名，直接定位
    if let Some((idx_str, sub)) = id.split_once('|') {
        if let Ok(idx) = idx_str.parse::<usize>() {
            if let Some((hive, path, _)) = UNINSTALL_HIVES.get(idx) {
                return read(*hive, path, sub);
            }
        }
    }
    // 旧格式兜底：按子键名遍历三个 hive
    for (hive, path, _tag) in UNINSTALL_HIVES {
        if let Some(app) = read(hive, path, id) {
            return Some(app);
        }
    }
    None
}

/// 把卸载命令行拆成 (可执行文件, 参数串)。支持引号包裹的带空格路径。
fn split_command(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let exe = rest[..end].to_string();
            let args = rest[end + 1..].trim_start().to_string();
            return (exe, args);
        }
    }
    match s.find(' ') {
        Some(i) => (s[..i].to_string(), s[i + 1..].trim_start().to_string()),
        None => (s.to_string(), String::new()),
    }
}

// ---------- 执行原生卸载 ----------

/// 卸载 UWP/商店应用（Remove-AppxPackage，每用户无需提权）。
fn remove_appx(pfn: &str) -> Result<i32, String> {
    use std::os::windows::process::CommandExt;
    // 仅允许合法 PackageFullName 字符，防注入
    if pfn.is_empty()
        || !pfn
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err("非法的应用包名。".to_string());
    }
    let ps = format!("Remove-AppxPackage -Package '{}'", pfn);
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("卸载商店应用失败：{}", e))?;
    Ok(status.code().unwrap_or(-1))
}

/// 调用软件自带卸载器（向导模式），阻塞等待其结束。返回卸载器退出码。
/// 手动拆解可执行文件与参数、用 raw_arg 原样传递，避开 `cmd /c` 对引号路径的错误拆分。
pub fn run_uninstaller(id: &str) -> Result<i32, String> {
    use std::os::windows::process::CommandExt;

    // UWP/商店应用：走 Remove-AppxPackage
    if let Some(pfn) = id.strip_prefix("uwp|") {
        return remove_appx(pfn);
    }

    let app = find_app(id).ok_or("找不到该软件的卸载信息（可能已被卸载）")?;
    let mut cmdline = app.uninstall_string.trim().to_string();
    if cmdline.is_empty() {
        cmdline = app.quiet_string.trim().to_string();
    }
    if cmdline.is_empty() {
        return Err("该软件未提供卸载命令。".to_string());
    }
    // MSI 型：UninstallString 常是 MsiExec /I{GUID}，改成 /X 才是卸载
    if app.msi {
        cmdline = cmdline.replacen("/I", "/X", 1).replacen("/i", "/x", 1);
    }

    let (exe, args) = split_command(&cmdline);
    let mut cmd = std::process::Command::new(&exe);
    if !args.is_empty() {
        cmd.raw_arg(&args);
    }
    let status = cmd
        .status()
        .map_err(|e| format!("启动卸载程序失败：{}", e))?;
    Ok(status.code().unwrap_or(-1))
}

// ---------- 扫描残留 ----------

/// 卸载后扫描残留：安装目录残余 + 常见数据目录下的同名目录 + 相关注册表项。
/// 保守收录（宁漏勿误）：只认安装目录与「名称精确匹配」的目录，厂商级泛目录一律跳过。
///
/// 卸载器执行后软件的原注册表键通常已被删除，因此按前端缓存的字段（而非 id）扫描。
pub fn scan_leftovers(name: &str, publisher: &str, install_location: &str) -> Vec<LeftoverItem> {
    let mut out: Vec<LeftoverItem> = Vec::new();
    let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
    let name_l = norm(name);

    // 1) 安装目录本身仍残留 —— 高置信度
    if !install_location.is_empty() {
        let p = PathBuf::from(install_location);
        if p.is_dir() && seen_dirs.insert(norm(install_location)) {
            out.push(LeftoverItem {
                kind: "dir".into(),
                path: install_location.to_string(),
                size: dir_size(&p),
                confidence: "high".into(),
            });
        }
    }

    // 2) 常见数据目录下「目录名精确等于软件名」的残留
    let roots: Vec<PathBuf> = [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "LOCALAPPDATA",
        "APPDATA",
        "ProgramData",
    ]
    .iter()
    .filter_map(|v| env_path(v))
    .collect();

    for root in &roots {
        let Ok(read) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in read.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let dl = norm(&dir_name);
            if is_generic(&dl) {
                continue;
            }
            let path = entry.path();
            let key = norm(&path.to_string_lossy());
            if seen_dirs.contains(&key) {
                continue;
            }
            // 精确匹配软件名（高）；等于发行商且发行商非泛词（低）
            let confidence = if dl == name_l {
                "high"
            } else if !publisher.is_empty() && dl == norm(publisher) && !is_generic(publisher) {
                "low"
            } else {
                continue;
            };
            seen_dirs.insert(key);
            out.push(LeftoverItem {
                kind: "dir".into(),
                path: path.to_string_lossy().to_string(),
                size: dir_size(&path),
                confidence: confidence.into(),
            });
        }
    }

    // 3) 注册表残留：直接命中 Software\<产品>、嵌套 Software\<发行商>\<产品>、以及残留的卸载登记键
    let reg_targets: [(winreg::HKEY, &str, &str); 4] = [
        (HKEY_CURRENT_USER, "HKCU", r"Software"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE"),
        (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\WOW6432Node"),
        (HKEY_CURRENT_USER, "HKCU", r"Software\WOW6432Node"),
    ];
    let name_core = strip_version_suffix(&name_l);
    let pub_l = norm(publisher);
    let pub_core = norm_publisher(publisher);
    let mut seen_reg: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut push_reg = |out: &mut Vec<LeftoverItem>, full: String, conf: &str| {
        if seen_reg.insert(norm(&full)) {
            out.push(LeftoverItem {
                kind: "reg".into(),
                path: full,
                size: 0,
                confidence: conf.into(),
            });
        }
    };

    for (hive, tag, base) in reg_targets {
        let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(base, KEY_READ) else {
            continue;
        };
        for sub in root.enum_keys().flatten() {
            let sl = norm(&sub);
            if is_generic(&sl) {
                continue;
            }
            // 直接命中 Software\<产品>
            if let Some(conf) = reg_name_match(&sl, &name_l, &name_core) {
                push_reg(&mut out, format!("{}\\{}\\{}", tag, base, sub), conf);
                continue;
            }
            // 嵌套：Software\<发行商>\<产品>——命中发行商文件夹后，只挑其中与软件名匹配的子键
            // 发行商文件夹名常与 DisplayName 中的发行商不一致（如 "Google LLC" vs 文件夹 "Google"），故用标准化名匹配
            let is_pub_folder =
                (sl == pub_l || (!pub_core.is_empty() && sl == pub_core)) && !is_generic(&sl);
            if is_pub_folder {
                if let Ok(pubkey) = root.open_subkey_with_flags(&sub, KEY_READ) {
                    for q in pubkey.enum_keys().flatten() {
                        let ql = norm(&q);
                        if reg_name_match(&ql, &name_l, &name_core).is_some() {
                            push_reg(
                                &mut out,
                                format!("{}\\{}\\{}\\{}", tag, base, sub, q),
                                "high",
                            );
                        }
                    }
                }
            }
        }
    }

    // 3b) 残留的卸载登记键：卸载器有时不清自己的 Uninstall 键，按 DisplayName 匹配收回
    for (hive, path, tag) in UNINSTALL_HIVES {
        let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for sub in root.enum_keys().flatten() {
            let Ok(k) = root.open_subkey_with_flags(&sub, KEY_READ) else {
                continue;
            };
            let dn: String = k.get_value("DisplayName").unwrap_or_default();
            if !dn.trim().is_empty() && norm(dn.trim()) == name_l {
                push_reg(&mut out, format!("{}\\{}\\{}", tag, path, sub), "high");
            }
        }
    }

    // 3c) App Paths：按安装目录前缀精确匹配（默认值为 exe 全路径，在安装目录下即属于本软件）
    if !install_location.trim().is_empty() {
        let inst = norm(install_location.trim());
        let app_paths: [(winreg::HKEY, &str, &str); 3] = [
            (HKEY_LOCAL_MACHINE, "HKLM", r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths"),
            (
                HKEY_LOCAL_MACHINE,
                "HKLM",
                r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths",
            ),
            (HKEY_CURRENT_USER, "HKCU", r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths"),
        ];
        for (hive, tag, base) in app_paths {
            let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(base, KEY_READ) else {
                continue;
            };
            for sub in root.enum_keys().flatten() {
                let Ok(k) = root.open_subkey_with_flags(&sub, KEY_READ) else {
                    continue;
                };
                let def: String = k.get_value("").unwrap_or_default();
                let def_n = norm(def.trim().trim_matches('"'));
                if !def_n.is_empty() && def_n.starts_with(&inst) {
                    push_reg(&mut out, format!("{}\\{}\\{}", tag, base, sub), "high");
                }
            }
        }
    }

    out
}

// ---------- 删除残留 ----------

/// 把指定文件夹删入回收站（可恢复）。path 必须是已存在的绝对目录。
fn send_dir_to_recycle(path: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW};

    // wFunc / fFlags 常量（手写以避免 features 命名差异）
    const FO_DELETE: u32 = 0x0003;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;

    // pFrom 需以双 null 结尾
    let mut from: Vec<u16> = path.encode_utf16().collect();
    from.push(0);
    from.push(0);

    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    let ret = unsafe { SHFileOperationW(&mut op) };
    if ret == 0 && op.fAnyOperationsAborted == 0 {
        Ok(())
    } else {
        Err(format!("移入回收站失败 (code={})", ret))
    }
}

/// reg export 备份目录：%LOCALAPPDATA%\DiskButler\reg-backup\
fn reg_backup_dir() -> Option<PathBuf> {
    let dir = env_path("LOCALAPPDATA")?.join("DiskButler").join("reg-backup");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// 把注册表全路径（HKCU\... / HKLM\...）拆成 (hive, 子路径)。
fn split_reg(full: &str) -> Option<(winreg::HKEY, String)> {
    let (tag, rest) = full.split_once('\\')?;
    let hive = match tag.to_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        _ => return None,
    };
    Some((hive, rest.to_string()))
}

/// 校验注册表目标只在 Software 分支（防误删系统关键键），返回 (hive, 子路径)。
fn ensure_software_branch(full: &str) -> Result<(winreg::HKEY, String), String> {
    let (hive, subpath) = split_reg(full).ok_or("注册表路径格式无法识别")?;
    if !subpath.to_uppercase().starts_with("SOFTWARE") {
        return Err("出于安全，只允许清理 Software 分支下的残留项。".to_string());
    }
    Ok((hive, subpath))
}

/// reg export 导出 .reg 备份（读操作，普通权限即可）。失败返回 Err。
fn backup_reg(full: &str, subpath: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let dir = reg_backup_dir().ok_or("无法创建注册表备份目录")?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let safe: String = subpath
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let backup = dir.join(format!("{}_{}.reg", safe, stamp));
    let status = std::process::Command::new("reg")
        .args(["export", full, &backup.to_string_lossy(), "/y"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err("导出注册表备份失败，已中止删除以保安全。".to_string()),
    }
}

/// 注册表键是否仍存在（HKLM 普通用户可读，用于验证提权删除结果）。
fn reg_exists(hive: winreg::HKEY, subpath: &str) -> bool {
    RegKey::predef(hive)
        .open_subkey_with_flags(subpath, KEY_READ)
        .is_ok()
}

/// 判断文件夹删除是否需要管理员权限（位于 ProgramData / Program Files / Windows 等受保护位置）。
fn dir_needs_elevation(path: &str) -> bool {
    let p = norm(path);
    for var in [
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "SystemRoot",
    ] {
        if let Some(root) = std::env::var_os(var) {
            let r = norm(&root.to_string_lossy());
            if !r.is_empty() && p.starts_with(&r) {
                return true;
            }
        }
    }
    false
}

/// 一次性提权执行一批命令（单次 UAC）：写 GBK 编码临时 bat，用 Start-Process RunAs 执行。
/// 用于删除受保护文件夹（rmdir）与 HKLM 注册表项（reg delete）；中文路径用 GBK 规避 cmd 乱码。
fn run_elevated_batch(lines: &[String]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let bat = std::env::temp_dir().join("diskbutler-cleanup.bat");
    let mut content = String::from("@echo off\r\n");
    for l in lines {
        content.push_str(l);
        content.push_str("\r\n");
    }
    content.push_str("exit /b 0\r\n");
    let (gbk, _, _) = encoding_rs::GBK.encode(&content);
    std::fs::write(&bat, &gbk).map_err(|e| format!("写入清理脚本失败：{}", e))?;

    let ps = format!(
        r#"$p = Start-Process -Verb RunAs -WindowStyle Hidden -Wait -PassThru -FilePath cmd.exe -ArgumentList '/c','"{}"'; exit $p.ExitCode"#,
        bat.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("提权操作失败：{}", e))?;
    let _ = std::fs::remove_file(&bat);
    match status.code().unwrap_or(-1) {
        0 => Ok(()),
        1 => Err("已取消管理员授权，需要权限的项未清理。".to_string()),
        c => Err(format!("提权清理未完成（退出码 {}）。", c)),
    }
}

/// 待提权处理的目标（批处理后逐项验证成败）。
struct ElevTarget {
    path: String,
    is_dir: bool,
    reg_subpath: String,
}

/// 删除前端勾选的残留项。
/// 普通文件夹入回收站、HKCU 注册表直删；受保护文件夹与 HKLM 注册表合并为一次提权批处理（单次 UAC，不会隐藏阻塞）。
pub fn remove_leftovers(items: Vec<LeftoverInput>) -> Vec<RemoveResult> {
    let mut results = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut targets: Vec<ElevTarget> = Vec::new();

    for item in items {
        match item.kind.as_str() {
            "dir" => {
                let p = PathBuf::from(&item.path);
                if !p.is_dir() {
                    results.push(RemoveResult {
                        path: item.path,
                        ok: false,
                        error: Some("目录不存在或已删除".to_string()),
                    });
                    continue;
                }
                // 命令行路径去掉尾部反斜杠，避免 cmd 中 `\"` 转义闭合引号
                let cmd_path = item.path.trim_end_matches('\\').to_string();
                if dir_needs_elevation(&item.path) {
                    lines.push(format!(r#"rmdir /s /q "{}""#, cmd_path));
                    targets.push(ElevTarget {
                        path: item.path,
                        is_dir: true,
                        reg_subpath: String::new(),
                    });
                } else {
                    match send_dir_to_recycle(&item.path) {
                        Ok(()) => results.push(RemoveResult {
                            path: item.path,
                            ok: true,
                            error: None,
                        }),
                        // 用户目录无需提权，回收失败通常是文件被占用（如正在运行的程序）；
                        // 不再升级为提权永久删除（避免删坏运行中程序的数据），如实报告
                        Err(_) => results.push(RemoveResult {
                            path: item.path,
                            ok: false,
                            error: Some(
                                "删除失败（文件可能正被占用，请先关闭相关程序后重试）".to_string(),
                            ),
                        }),
                    }
                }
            }
            "reg" => match ensure_software_branch(&item.path) {
                Err(e) => results.push(RemoveResult {
                    path: item.path,
                    ok: false,
                    error: Some(e),
                }),
                Ok((hive, subpath)) => {
                    if let Err(e) = backup_reg(&item.path, &subpath) {
                        results.push(RemoveResult {
                            path: item.path,
                            ok: false,
                            error: Some(e),
                        });
                        continue;
                    }
                    if hive == HKEY_CURRENT_USER {
                        let r = RegKey::predef(hive)
                            .delete_subkey_all(&subpath)
                            .map_err(|e| format!("删除注册表项失败：{}", e));
                        results.push(RemoveResult {
                            path: item.path,
                            ok: r.is_ok(),
                            error: r.err(),
                        });
                    } else {
                        lines.push(format!(r#"reg delete "{}" /f"#, item.path));
                        targets.push(ElevTarget {
                            path: item.path,
                            is_dir: false,
                            reg_subpath: subpath,
                        });
                    }
                }
            },
            _ => results.push(RemoveResult {
                path: item.path,
                ok: false,
                error: Some("未知的残留类型".to_string()),
            }),
        }
    }

    // 一次性提权批处理受保护文件夹 + HKLM 注册表，再逐项按“是否已消失”验证成败
    if !lines.is_empty() {
        let batch = run_elevated_batch(&lines);
        for t in targets {
            let gone = if t.is_dir {
                !PathBuf::from(&t.path).exists()
            } else {
                !reg_exists(HKEY_LOCAL_MACHINE, &t.reg_subpath)
            };
            let error = if gone {
                None
            } else {
                Some(
                    batch
                        .as_ref()
                        .err()
                        .cloned()
                        .unwrap_or_else(|| "清理失败（可能需要管理员权限）".to_string()),
                )
            };
            results.push(RemoveResult {
                path: t.path,
                ok: gone,
                error,
            });
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_names_are_rejected() {
        assert!(is_generic("Microsoft"));
        assert!(is_generic("common files"));
        assert!(is_generic(""));
        assert!(!is_generic("Everything"));
        assert!(!is_generic("剪映"));
    }

    #[test]
    fn split_command_handles_quoted_and_plain() {
        let (e, a) = split_command("\"C:\\Program Files\\App\\uninstall.exe\" /S");
        assert_eq!(e, "C:\\Program Files\\App\\uninstall.exe");
        assert_eq!(a, "/S");
        let (e2, a2) = split_command("C:\\App\\unins000.exe /SILENT");
        assert_eq!(e2, "C:\\App\\unins000.exe");
        assert_eq!(a2, "/SILENT");
        let (e3, a3) = split_command("MsiExec.exe");
        assert_eq!(e3, "MsiExec.exe");
        assert_eq!(a3, "");
    }

    #[test]
    fn split_reg_parses_hive_prefix() {
        let (h, p) = split_reg(r"HKCU\Software\Foo").unwrap();
        assert_eq!(h, HKEY_CURRENT_USER);
        assert_eq!(p, r"Software\Foo");
        assert!(split_reg("BADHIVE\\x").is_none());
    }

    #[test]
    fn delete_reg_rejects_non_software_branch() {
        // 系统关键分支必须被拒绝，绝不允许误删
        assert!(ensure_software_branch(r"HKLM\SYSTEM\CurrentControlSet").is_err());
        assert!(ensure_software_branch(r"HKCU\Software\Foo").is_ok());
    }

    #[test]
    fn strip_version_suffix_trims_trailing_version() {
        assert_eq!(strip_version_suffix("ocs desktop 2.8.5"), "ocs desktop");
        assert_eq!(strip_version_suffix("everything"), "everything");
        // 纯版本名不能被删空
        assert_eq!(strip_version_suffix("1.2.3"), "1.2.3");
    }

    #[test]
    fn reg_name_match_matches_exact_and_versioned() {
        assert_eq!(reg_name_match("ocs desktop", "ocs desktop 2.8.5", "ocs desktop"), Some("high"));
        assert_eq!(reg_name_match("everything", "everything", "everything"), Some("high"));
        // 泛词与不相关名不命中
        assert!(reg_name_match("microsoft", "microsoft office", "microsoft office").is_none());
        assert!(reg_name_match("chrome", "firefox", "firefox").is_none());
    }
}
