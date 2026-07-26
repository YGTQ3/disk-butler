//! 启动项管理：枚举注册表 Run 键与启动文件夹，附带「建议禁用/保留」知识引擎，
//! 通过 StartupApproved 二进制值切换启用/禁用（与任务管理器"禁用"完全等效、可逆）。

use serde::Serialize;
use std::path::PathBuf;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
use winreg::{RegKey, RegValue};

const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_RUN: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const APPROVED_FOLDER: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    /// 形如 "hkcu-run::DingTalk"，切换时传回
    pub id: String,
    pub name: String,
    /// 启动命令或快捷方式路径（真身）
    pub command: String,
    /// 人话位置：用户注册表 / 系统注册表 / 启动文件夹
    pub location: String,
    pub enabled: bool,
    /// 系统级(HKLM/公共文件夹)项修改需要管理员
    pub needs_admin: bool,
    /// 对应进程当前内存占用（MB，未运行为 0）
    pub mem_mb: u64,
    /// "disable" 建议禁用 | "keep" 建议保留 | "neutral" 看使用习惯
    pub advice: String,
    /// 建议理由（人话）
    pub reason: String,
}

// ---------- 建议知识引擎（来自真实优化实战） ----------

fn advise(name: &str, command: &str) -> (&'static str, &'static str) {
    let hay = format!("{} {}", name.to_lowercase(), command.to_lowercase());
    let has = |k: &str| hay.contains(k);

    // 建议保留：系统功能 / 安全 / 驱动 / 轻量常用工具
    if has("ctfmon") {
        return ("keep", "输入法核心组件，禁用后可能无法打字。");
    }
    if has("securityhealth") {
        return ("keep", "Windows 安全中心，安全防护相关，建议保留。");
    }
    if has("rtkaud") || has("realtek") {
        return ("keep", "声卡驱动服务，禁用可能影响音频功能。");
    }
    if has("hips") || has("sysdiag") || has("huorong") || has("360") {
        return ("keep", "安全软件组件，安全防护建议保留。");
    }
    if has("everything") {
        return ("keep", "文件秒搜工具，常驻占用很小，常用建议保留。");
    }
    if has("snipaste") || has("ditto") {
        return ("keep", "轻量常用小工具，占用很小，保留不亏。");
    }

    // 建议禁用：预加载 / 更新器 / 明显不需要常驻的
    if has("autolaunch") || has("--no-startup-window") {
        return (
            "disable",
            "浏览器开机预加载，纯为抢首次启动速度，禁用后首次打开慢 1~2 秒，可省数百 MB 内存。",
        );
    }
    if has("pdf24") {
        return ("disable", "PDF 工具托盘，用的时候手动打开即可。");
    }
    if has("baomihua") {
        return ("disable", "网易爆米花视频软件，一般无需开机自启。");
    }
    if has("idman") || has("idm") && has("onboot") {
        return (
            "disable",
            "IDM 下载器。浏览器点下载时会通过插件自动唤起，无需开机自启。",
        );
    }
    if has("-updater") || has("update.exe") {
        return ("disable", "软件更新器，软件启动时会自行检查更新，无需常驻。");
    }

    // 看习惯：聊天 / 同步 / 组网
    if has("dingtalk") || has("dingding") {
        return (
            "neutral",
            "钉钉。上班依赖它就保留；否则禁用后用时手动打开，开机可省 300~500 MB。",
        );
    }
    if has("qqnt") || has("qq.exe") {
        return ("neutral", "QQ。不需要开机自动登录可禁用，随时手动打开。");
    }
    if has("weixin") || has("wechat") {
        return ("neutral", "微信。依赖开机即收消息就保留，否则可禁用。");
    }
    if has("onedrive") {
        return (
            "neutral",
            "OneDrive 云同步。依赖它自动备份文件就保留，否则可禁用改手动。",
        );
    }
    if has("synology") {
        return ("neutral", "群晖同步客户端。依赖 NAS 自动备份则保留。");
    }
    if has("tailscale") {
        return ("neutral", "异地组网工具。常用远程访问则保留，否则用时手动开。");
    }

    (
        "neutral",
        "未收录的启动项。若不认识且路径可疑，建议先查证再决定；禁用是可逆的。",
    )
}

// ---------- StartupApproved 启用状态读写 ----------

/// 首字节 bit0 = 1 表示禁用；无记录默认启用
fn read_enabled(hive: &RegKey, approved_path: &str, value_name: &str) -> bool {
    let Ok(key) = hive.open_subkey_with_flags(approved_path, KEY_READ) else {
        return true;
    };
    match key.get_raw_value(value_name) {
        Ok(v) if !v.bytes.is_empty() => v.bytes[0] & 1 == 0,
        _ => true,
    }
}

fn write_enabled(root: winreg::HKEY, approved_path: &str, value_name: &str, enabled: bool) -> Result<(), String> {
    let hive = RegKey::predef(root);
    let key = hive
        .create_subkey(approved_path)
        .map_err(|e| perm_hint(&e))?
        .0;
    let mut bytes = vec![0u8; 12];
    bytes[0] = if enabled { 2 } else { 3 };
    key.set_raw_value(
        value_name,
        &RegValue {
            bytes,
            vtype: winreg::enums::RegType::REG_BINARY,
        },
    )
    .map_err(|e| perm_hint(&e))
}

fn perm_hint(e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        "需要管理员权限：请右键以管理员身份运行本程序后再修改系统级启动项。".to_string()
    } else {
        format!("修改失败：{}", e)
    }
}

// ---------- 进程内存匹配 ----------

/// 从启动命令中提取 exe 名（小写、去扩展名）
fn exe_stem(command: &str) -> Option<String> {
    let cmd = command.trim().trim_start_matches('"');
    let end = cmd.find('"').unwrap_or(cmd.len());
    let path_part = &cmd[..end];
    // 无引号时截到第一个 " /" 或 " -" 参数前
    let path_part = path_part
        .split(" /")
        .next()
        .unwrap_or(path_part)
        .split(" -")
        .next()
        .unwrap_or(path_part);
    let p = PathBuf::from(path_part.trim());
    p.file_stem().map(|s| s.to_string_lossy().to_lowercase())
}

fn process_mem_map() -> std::collections::HashMap<String, u64> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut map: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for proc in sys.processes().values() {
        let name = proc.name().to_string_lossy().to_lowercase();
        let stem = name.trim_end_matches(".exe").to_string();
        *map.entry(stem).or_insert(0) += proc.memory();
    }
    map
}

// ---------- 枚举 ----------

pub fn list_items() -> Vec<StartupItem> {
    let mem = process_mem_map();
    let mut out: Vec<StartupItem> = Vec::new();

    // 注册表 Run 键（HKCU + HKLM）
    for (root, prefix, location, needs_admin) in [
        (HKEY_CURRENT_USER, "hkcu-run", "用户注册表", false),
        (HKEY_LOCAL_MACHINE, "hklm-run", "系统注册表", true),
    ] {
        let hive = RegKey::predef(root);
        if let Ok(run) = hive.open_subkey_with_flags(RUN_KEY, KEY_READ) {
            for item in run.enum_values().flatten() {
                let (name, value) = item;
                let command: String = String::from_utf16_lossy(
                    &value
                        .bytes
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .take_while(|&c| c != 0)
                        .collect::<Vec<u16>>(),
                );
                let enabled = read_enabled(&hive, APPROVED_RUN, &name);
                let (advice, reason) = advise(&name, &command);
                let mem_mb = exe_stem(&command)
                    .and_then(|s| mem.get(&s).copied())
                    .unwrap_or(0)
                    / 1024
                    / 1024;
                out.push(StartupItem {
                    id: format!("{}::{}", prefix, name),
                    name,
                    command,
                    location: location.to_string(),
                    enabled,
                    needs_admin,
                    mem_mb,
                    advice: advice.to_string(),
                    reason: reason.to_string(),
                });
            }
        }
    }

    // 启动文件夹（用户 + 公共）
    let folders = [
        (
            std::env::var_os("APPDATA").map(|p| {
                PathBuf::from(p).join(r"Microsoft\Windows\Start Menu\Programs\Startup")
            }),
            "user-folder",
            "启动文件夹",
            HKEY_CURRENT_USER,
            false,
        ),
        (
            std::env::var_os("ProgramData").map(|p| {
                PathBuf::from(p).join(r"Microsoft\Windows\Start Menu\Programs\StartUp")
            }),
            "common-folder",
            "公共启动文件夹",
            HKEY_LOCAL_MACHINE,
            true,
        ),
    ];
    for (dir, prefix, location, root, needs_admin) in folders {
        let Some(dir) = dir else { continue };
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let hive = RegKey::predef(root);
        for entry in read.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            let display = fname.trim_end_matches(".lnk").to_string();
            let enabled = read_enabled(&hive, APPROVED_FOLDER, &fname);
            let (advice, reason) = advise(&display, &fname);
            let mem_mb = mem
                .get(&display.to_lowercase().replace(' ', ""))
                .or_else(|| mem.get(&display.to_lowercase()))
                .copied()
                .unwrap_or(0)
                / 1024
                / 1024;
            out.push(StartupItem {
                id: format!("{}::{}", prefix, fname),
                name: display,
                command: entry.path().to_string_lossy().to_string(),
                location: location.to_string(),
                enabled,
                needs_admin,
                mem_mb,
                advice: advice.to_string(),
                reason: reason.to_string(),
            });
        }
    }

    // 排序：建议禁用在前，其次按内存降序
    out.sort_by(|a, b| {
        let rank = |s: &str| match s {
            "disable" => 0,
            "neutral" => 1,
            _ => 2,
        };
        rank(&a.advice)
            .cmp(&rank(&b.advice))
            .then(b.mem_mb.cmp(&a.mem_mb))
    });
    out
}

/// 切换启用状态。id 由 list_items 提供，非法 id 直接拒绝。
pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
    let Some((kind, name)) = id.split_once("::") else {
        return Err("无效的启动项标识".to_string());
    };
    match kind {
        "hkcu-run" => write_enabled(HKEY_CURRENT_USER, APPROVED_RUN, name, enabled),
        "hklm-run" => write_enabled(HKEY_LOCAL_MACHINE, APPROVED_RUN, name, enabled),
        "user-folder" => write_enabled(HKEY_CURRENT_USER, APPROVED_FOLDER, name, enabled),
        "common-folder" => write_enabled(HKEY_LOCAL_MACHINE, APPROVED_FOLDER, name, enabled),
        _ => Err("无效的启动项类型".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advise_keep_for_input_method() {
        assert_eq!(advise("ctfmon", r"C:\WINDOWS\system32\ctfmon.exe").0, "keep");
    }

    #[test]
    fn advise_disable_for_edge_prelaunch() {
        let (a, _) = advise(
            "MicrosoftEdgeAutoLaunch_ABC",
            r#""C:\...\msedge.exe" --no-startup-window"#,
        );
        assert_eq!(a, "disable");
    }

    #[test]
    fn advise_neutral_for_dingtalk() {
        let (a, _) = advise("DingTalk", r"C:\Program Files (x86)\DingDing\DingtalkLauncher.exe /autorun");
        assert_eq!(a, "neutral");
    }

    #[test]
    fn advise_disable_for_updater() {
        // 火绒的 CrossUpgrade 真实路径含 Huorong，安全软件规则优先命中 keep
        assert_eq!(
            advise("HRCrossUpgrade", r"C:\Program Files (x86)\Huorong\Sysdiag\CrossUpgrade.exe -a").0,
            "keep"
        );
        assert_eq!(advise("SomeApp-Updater", r"C:\x\some-updater\update.exe").0, "disable");
    }

    #[test]
    fn exe_stem_parses_quoted_command() {
        assert_eq!(
            exe_stem(r#""C:\Program Files\Tencent\QQNT\QQ.exe" /background"#),
            Some("qq".to_string())
        );
    }

    #[test]
    fn exe_stem_parses_unquoted_command() {
        assert_eq!(
            exe_stem(r"C:\Program Files (x86)\DingDing\DingtalkLauncher.exe /autorun"),
            Some("dingtalklauncher".to_string())
        );
    }

    #[test]
    fn set_enabled_rejects_bad_id() {
        assert!(set_enabled("evil-path", true).is_err());
        assert!(set_enabled("unknown::x", true).is_err());
    }
}
