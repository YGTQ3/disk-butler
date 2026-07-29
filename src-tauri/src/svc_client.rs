//! 后台扫描服务的管道客户端：主程序（普通权限）经命名管道请求 MFT 秒级扫描。
//! 服务为 OnDemand + 用完即停：每次扫描前按需拉起（安装时 ACL 已放行普通用户启动），
//! 扫完服务自行退出——系统中不存在常驻进程。

use crate::scan::TreeNode;
use crate::svc::{PIPE_NAME, SERVICE_NAME};
use serde_json::json;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

/// 查询扫描服务是否已注册。用于主程序自检"秒级加速是否可用"：
/// 服务若被安全软件等移除，扫描会静默回退慢速遍历，主程序据此引导用户一键修复。
pub fn service_installed() -> bool {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    let Ok(manager) =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
    else {
        return false;
    };
    manager
        .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .is_ok()
}

/// 重装扫描服务：提权运行安装目录下的 disk-butler-svc.exe install（弹一次 UAC）。
/// 服务注册需管理员；成败以「重装后服务是否已注册」为准（UAC 被取消 → 仍未注册 → 报错）。
pub fn repair_service() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("定位程序路径失败：{}", e))?
        .parent()
        .ok_or_else(|| "无法定位安装目录".to_string())?
        .join("disk-butler-svc.exe");
    if !exe.exists() {
        return Err("找不到扫描服务程序（disk-butler-svc.exe）".to_string());
    }
    // 经 PowerShell 提权运行并等待完成（-Verb RunAs 弹 UAC；-Wait 等安装结束）
    let ps = format!(
        "Start-Process -FilePath '{}' -ArgumentList 'install' -Verb RunAs -Wait -WindowStyle Hidden",
        exe.display()
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .status()
        .map_err(|e| format!("启动提权进程失败：{}", e))?;
    // 以最终事实为准：服务是否已注册
    if service_installed() {
        Ok(())
    } else {
        Err("修复未完成（可能取消了管理员授权），请重试".to_string())
    }
}

/// 按需拉起扫描服务（普通权限；ACL 未放行/服务未安装时返回 Err，由调用方回退）。
fn ensure_service_started() -> Result<(), String> {
    use windows_service::service::{ServiceAccess, ServiceState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("连接服务管理器失败：{}", e))?;
    let service = manager
        .open_service(SERVICE_NAME, ServiceAccess::START | ServiceAccess::QUERY_STATUS)
        .map_err(|e| format!("扫描服务未安装：{}", e))?;
    if service.start::<&str>(&[]).is_err() {
        // 启动失败：可能已在运行（上一个请求刚拉起），查状态确认
        let running = service
            .query_status()
            .map(|s| {
                s.current_state == ServiceState::Running
                    || s.current_state == ServiceState::StartPending
            })
            .unwrap_or(false);
        if !running {
            return Err("扫描服务启动失败".to_string());
        }
    }
    Ok(())
}

/// 连接服务管道，必要时先拉起服务并短暂重试等待管道就绪。
fn open_pipe_on_demand() -> Result<File, String> {
    // 服务可能恰好在跑（并发请求），先直接连一次
    if let Ok(f) = File::options().read(true).write(true).open(PIPE_NAME) {
        return Ok(f);
    }
    ensure_service_started()?;
    for _ in 0..15 {
        std::thread::sleep(Duration::from_millis(200));
        if let Ok(f) = File::options().read(true).write(true).open(PIPE_NAME) {
            return Ok(f);
        }
    }
    Err("扫描服务已拉起但管道未就绪".to_string())
}

/// 通过后台服务扫描。服务不在/协议异常时返回 Err，由调用方回退其他引擎。
/// progress(已发现文件数, 已统计字节数, 精确百分比 0~100, 阶段文案)
pub fn scan_via_service<F: FnMut(u64, u64, f32, &str)>(
    root: &str,
    mut progress: F,
) -> Result<TreeNode, String> {
    let mut pipe = open_pipe_on_demand()?;

    writeln!(pipe, "{}", json!({"cmd": "scan", "root": root}))
        .map_err(|e| format!("发送扫描请求失败：{}", e))?;
    pipe.flush().ok();

    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        let line = line.map_err(|e| format!("读取服务响应失败：{}", e))?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: serde_json::Value =
            serde_json::from_str(&line).map_err(|e| format!("服务响应格式错误：{}", e))?;
        match msg["type"].as_str() {
            Some("progress") => progress(
                msg["filesScanned"].as_u64().unwrap_or(0),
                msg["bytesScanned"].as_u64().unwrap_or(0),
                msg["percent"].as_f64().unwrap_or(0.0) as f32,
                msg["phase"].as_str().unwrap_or(""),
            ),
            Some("done") => {
                return serde_json::from_value(msg["tree"].clone())
                    .map_err(|e| format!("解析扫描结果失败：{}", e));
            }
            Some("error") => {
                return Err(msg["message"].as_str().unwrap_or("服务扫描失败").to_string());
            }
            _ => {}
        }
    }
    Err("服务连接中断".to_string())
}
