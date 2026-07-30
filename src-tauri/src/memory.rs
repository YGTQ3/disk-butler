//! 内存体检：物理内存/页面文件水位 + 按进程名分组的内存大户排行 + 进程人话知识库。

use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessGroup {
    pub name: String,
    pub friendly_name: String,
    pub description: String,
    /// "closable" 可退出 | "system" 系统必需 | "unknown"
    pub kind: String,
    pub count: u32,
    pub memory: u64,
    /// 程序图标（PNG data URL），提取失败为 None，前端用占位
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReport {
    pub overview: MemoryOverview,
    pub groups: Vec<ProcessGroup>,
}

// ---------- 页面文件一致性核验（源自一次真实排查：配置正确但系统拒绝启用） ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagefileEntry {
    /// 如 "c:\pagefile.sys"
    pub path: String,
    /// 盘符大写，如 "C"
    pub drive: String,
    pub init_mb: u64,
    pub max_mb: u64,
    /// init/max 均为 0 = 该盘"系统托管大小"
    pub system_managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePagefile {
    pub path: String,
    pub drive: String,
    /// 本次开机实际分配大小（MB）
    pub allocated_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagefileCheck {
    /// 是否勾选了"自动管理所有驱动器的分页文件大小"
    pub auto_managed: bool,
    pub configured: Vec<PagefileEntry>,
    pub active: Vec<ActivePagefile>,
    /// 人话问题列表；为空 = 配置与实际一致，不打扰用户
    pub issues: Vec<String>,
}

/// 解析注册表 PagingFiles 多字符串：
/// "?:\pagefile.sys" = 自动管理；"c:\pagefile.sys 800 800" = 自定义；init/max 为 0 = 托管
fn parse_config(vals: &[String]) -> (Vec<PagefileEntry>, bool) {
    let mut auto = false;
    let mut out = Vec::new();
    for v in vals {
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        if v.starts_with('?') {
            auto = true;
            continue;
        }
        let mut parts = v.split_whitespace();
        let path = parts.next().unwrap_or("").to_string();
        let init_mb: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let max_mb: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let drive = path
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase().to_string())
            .unwrap_or_default();
        if drive.is_empty() {
            continue;
        }
        out.push(PagefileEntry {
            system_managed: init_mb == 0 && max_mb == 0,
            path,
            drive,
            init_mb,
            max_mb,
        });
    }
    (out, auto)
}

/// 一致性判定（纯函数，可测）：配置了却没启用 = 问题
fn evaluate(configured: &[PagefileEntry], auto: bool, active: &[ActivePagefile]) -> Vec<String> {
    let mut issues = Vec::new();
    if active.is_empty() {
        if auto || !configured.is_empty() {
            issues.push(
                "已配置页面文件，但本次开机系统没有启用任何一个——虚拟内存完全没在工作，大型程序可能报“内存不足”。"
                    .to_string(),
            );
        } else {
            issues.push(
                "当前没有配置任何页面文件，物理内存一旦占满，程序会直接崩溃或被系统终止。"
                    .to_string(),
            );
        }
        return issues;
    }
    // 自动管理模式下有活动页面文件即视为正常
    if auto {
        return issues;
    }
    for c in configured {
        if !active.iter().any(|a| a.drive == c.drive) {
            issues.push(format!(
                "已配置 {} 盘页面文件，但本次开机系统没有启用它（Windows 对非系统盘页面文件偶发此问题）。建议改为 C 盘或勾选“自动管理”。",
                c.drive
            ));
        }
    }
    issues
}

fn read_config() -> (Vec<PagefileEntry>, bool) {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let vals: Vec<String> = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management")
        .and_then(|k| k.get_value("PagingFiles"))
        .unwrap_or_default();
    parse_config(&vals)
}

/// Win32_PageFileUsage = 本次开机实际启用的页面文件（权威口径；文件存在≠已启用）
fn read_active() -> Vec<ActivePagefile> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_PageFileUsage | ForEach-Object { $_.Name + '|' + $_.AllocatedBaseSize }",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (p, s) = l.trim().split_once('|')?;
            Some(ActivePagefile {
                drive: p.chars().next()?.to_ascii_uppercase().to_string(),
                path: p.to_string(),
                allocated_mb: s.trim().parse().ok()?,
            })
        })
        .collect()
}

pub fn pagefile_check() -> PagefileCheck {
    let (configured, auto_managed) = read_config();
    let active = read_active();
    let issues = evaluate(&configured, auto_managed, &active);
    PagefileCheck {
        auto_managed,
        configured,
        active,
        issues,
    }
}

struct ProcInfo {
    needle: &'static str,
    friendly: &'static str,
    desc: &'static str,
    kind: &'static str,
}

/// 进程知识库：常见进程的人话解释（来自真实内存分析实战）
const PROC_KNOWLEDGE: &[ProcInfo] = &[
    ProcInfo { needle: "svchost", friendly: "系统服务宿主", desc: "Windows 各种后台服务的载体，数量多是正常现象，不要动它。", kind: "system" },
    ProcInfo { needle: "memory compression", friendly: "内存压缩", desc: "系统在压缩内存腾空间。这个值越大，说明物理内存越吃紧。", kind: "system" },
    ProcInfo { needle: "dwm", friendly: "桌面窗口管理器", desc: "负责窗口绘制与特效，系统必需。关闭透明特效可略微降低占用。", kind: "system" },
    ProcInfo { needle: "explorer", friendly: "文件资源管理器", desc: "桌面和文件窗口本体，系统必需。", kind: "system" },
    ProcInfo { needle: "searchhost", friendly: "Windows 搜索", desc: "开始菜单搜索服务，系统组件。", kind: "system" },
    ProcInfo { needle: "startmenuexperiencehost", friendly: "开始菜单", desc: "开始菜单界面进程，系统组件。", kind: "system" },
    ProcInfo { needle: "runtimebroker", friendly: "运行时代理", desc: "UWP 应用的权限中介，系统组件。", kind: "system" },
    ProcInfo { needle: "textinputhost", friendly: "输入体验", desc: "输入法候选界面，系统组件。", kind: "system" },
    ProcInfo { needle: "msedgewebview2", friendly: "网页组件 (WebView2)", desc: "各种桌面应用内嵌的网页引擎（本应用也在用）。随宿主应用关闭而退出。", kind: "system" },
    ProcInfo { needle: "msedge", friendly: "Edge 浏览器", desc: "标签页开得越多占用越大，不用时关闭多余标签页。", kind: "closable" },
    ProcInfo { needle: "chrome", friendly: "Chrome 浏览器", desc: "标签页开得越多占用越大，不用时关闭多余标签页。", kind: "closable" },
    ProcInfo { needle: "weixin", friendly: "微信", desc: "聊天数据和小程序缓存都在内存里，不用时彻底退出可省 1~2 GB。", kind: "closable" },
    ProcInfo { needle: "wechatappex", friendly: "微信小程序", desc: "微信内嵌浏览器/小程序进程，退出微信即释放。", kind: "closable" },
    ProcInfo { needle: "qq", friendly: "QQ", desc: "不用时退出可释放数百 MB。", kind: "closable" },
    ProcInfo { needle: "dingtalk", friendly: "钉钉", desc: "不用时退出可释放数百 MB。", kind: "closable" },
    ProcInfo { needle: "qoder", friendly: "Qoder IDE", desc: "AI 编程工具，工作时的主力应用。", kind: "closable" },
    ProcInfo { needle: "code", friendly: "VS Code", desc: "代码编辑器，多窗口多插件会增加占用。", kind: "closable" },
    ProcInfo { needle: "onedrive", friendly: "OneDrive", desc: "云盘同步客户端，同步空闲时占用会回落。", kind: "closable" },
    ProcInfo { needle: "sogou", friendly: "搜狗输入法云服务", desc: "云输入/推荐模块，被主程序守护，难以单独关闭。", kind: "system" },
    ProcInfo { needle: "hipsdaemon", friendly: "火绒安全", desc: "安全防护核心，建议保留。", kind: "system" },
    ProcInfo { needle: "everything", friendly: "Everything", desc: "文件秒搜索引，常驻占用换来的是瞬时搜索。", kind: "closable" },
    ProcInfo { needle: "tailscale", friendly: "Tailscale 组网", desc: "异地组网服务，需要远程访问时保留。", kind: "closable" },
    ProcInfo { needle: "node", friendly: "Node.js", desc: "前端开发服务或某些应用的后台进程。", kind: "closable" },
    ProcInfo { needle: "powershell", friendly: "PowerShell", desc: "终端/脚本进程。", kind: "closable" },
    ProcInfo { needle: "system", friendly: "系统内核", desc: "Windows 内核进程，不要动它。", kind: "system" },
];

fn lookup(name: &str) -> (&'static str, &'static str, &'static str) {
    let lower = name.to_lowercase();
    for info in PROC_KNOWLEDGE {
        if lower.contains(info.needle) {
            return (info.friendly, info.desc, info.kind);
        }
    }
    ("", "未收录的进程。可搜索进程名了解用途；陌生且占用异常大时值得留意。", "unknown")
}

pub fn report() -> MemoryReport {
    let mut sys = System::new_all();
    sys.refresh_all();

    let overview = MemoryOverview {
        total: sys.total_memory(),
        used: sys.total_memory().saturating_sub(sys.available_memory()),
        available: sys.available_memory(),
        swap_total: sys.total_swap(),
        swap_used: sys.used_swap(),
    };

    // 按进程名分组聚合；同时记录每组一个代表性 exe 路径（供抠图标）
    let mut map: std::collections::HashMap<String, (u32, u64)> = std::collections::HashMap::new();
    let mut paths: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for proc in sys.processes().values() {
        let name = proc.name().to_string_lossy().to_string();
        let key = name.trim_end_matches(".exe").to_string();
        let entry = map.entry(key.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += proc.memory();
        if !paths.contains_key(&key) {
            if let Some(p) = proc.exe() {
                let s = p.to_string_lossy().to_string();
                if !s.is_empty() {
                    paths.insert(key, s);
                }
            }
        }
    }

    let mut groups: Vec<ProcessGroup> = map
        .into_iter()
        .map(|(name, (count, memory))| {
            let (friendly, desc, kind) = lookup(&name);
            ProcessGroup {
                friendly_name: if friendly.is_empty() {
                    name.clone()
                } else {
                    friendly.to_string()
                },
                description: desc.to_string(),
                kind: kind.to_string(),
                name,
                count,
                memory,
                icon: None,
            }
        })
        .collect();

    groups.sort_by(|a, b| b.memory.cmp(&a.memory));
    groups.truncate(20);

    // 仅对排行前 20 抠图标（GDI 提取有开销，无需对全部进程做）
    for g in &mut groups {
        if let Some(p) = paths.get(&g.name) {
            g.icon = crate::icon::from_file(p);
        }
    }

    MemoryReport { overview, groups }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_process() {
        let (friendly, _, kind) = lookup("svchost");
        assert_eq!(friendly, "系统服务宿主");
        assert_eq!(kind, "system");
    }

    #[test]
    fn lookup_wechat_is_closable() {
        let (_, _, kind) = lookup("Weixin");
        assert_eq!(kind, "closable");
    }

    #[test]
    fn lookup_unknown_returns_hint() {
        let (friendly, desc, kind) = lookup("some_totally_unknown_proc_xyz");
        assert!(friendly.is_empty());
        assert!(!desc.is_empty());
        assert_eq!(kind, "unknown");
    }

    #[test]
    fn report_has_overview_and_groups() {
        let r = report();
        assert!(r.overview.total > 0);
        assert!(!r.groups.is_empty());
        assert!(r.groups.len() <= 20);
        // 排序校验：降序
        for w in r.groups.windows(2) {
            assert!(w[0].memory >= w[1].memory);
        }
    }

    // ---------- 页面文件核验 ----------

    fn active(drive: &str, mb: u64) -> ActivePagefile {
        ActivePagefile {
            path: format!("{}:\\pagefile.sys", drive),
            drive: drive.to_string(),
            allocated_mb: mb,
        }
    }

    #[test]
    fn parse_config_custom_and_auto() {
        let (list, auto) = parse_config(&[
            r"c:\pagefile.sys 800 800".to_string(),
            r"d:\pagefile.sys 0 0".to_string(),
        ]);
        assert!(!auto);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].drive, "C");
        assert_eq!(list[0].init_mb, 800);
        assert!(!list[0].system_managed);
        assert!(list[1].system_managed);

        let (list2, auto2) = parse_config(&[r"?:\pagefile.sys".to_string()]);
        assert!(auto2);
        assert!(list2.is_empty());
    }

    #[test]
    fn evaluate_detects_configured_but_inactive_drive() {
        // 真实事故场景：C 800MB 生效，D 盘配置了却没启用
        let (cfg, auto) = parse_config(&[
            r"c:\pagefile.sys 800 800".to_string(),
            r"d:\pagefile.sys 8192 16384".to_string(),
        ]);
        let issues = evaluate(&cfg, auto, &[active("C", 800)]);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("D 盘"));
    }

    #[test]
    fn evaluate_ok_when_all_active() {
        let (cfg, auto) = parse_config(&[r"c:\pagefile.sys 800 800".to_string()]);
        assert!(evaluate(&cfg, auto, &[active("C", 800)]).is_empty());
        // 自动管理 + 有活动文件 = 正常
        let (cfg2, auto2) = parse_config(&[r"?:\pagefile.sys".to_string()]);
        assert!(evaluate(&cfg2, auto2, &[active("C", 14848)]).is_empty());
    }

    #[test]
    fn evaluate_warns_when_nothing_active() {
        let (cfg, auto) = parse_config(&[r"d:\pagefile.sys 8192 16384".to_string()]);
        let issues = evaluate(&cfg, auto, &[]);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("没有启用"));
        // 完全没配置也要提醒
        let issues2 = evaluate(&[], false, &[]);
        assert_eq!(issues2.len(), 1);
    }
}
