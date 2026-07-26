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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReport {
    pub overview: MemoryOverview,
    pub groups: Vec<ProcessGroup>,
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

    // 按进程名分组聚合
    let mut map: std::collections::HashMap<String, (u32, u64)> = std::collections::HashMap::new();
    for proc in sys.processes().values() {
        let name = proc.name().to_string_lossy().to_string();
        let key = name.trim_end_matches(".exe").to_string();
        let entry = map.entry(key).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += proc.memory();
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
            }
        })
        .collect();

    groups.sort_by(|a, b| b.memory.cmp(&a.memory));
    groups.truncate(20);

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
}
