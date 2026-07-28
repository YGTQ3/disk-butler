//! 后台扫描服务的管道客户端：主程序（普通权限）经命名管道请求 MFT 秒级扫描。
//! 服务为 OnDemand + 用完即停：每次扫描前按需拉起（安装时 ACL 已放行普通用户启动），
//! 扫完服务自行退出——系统中不存在常驻进程。

use crate::scan::TreeNode;
use crate::svc::{PIPE_NAME, SERVICE_NAME};
use serde_json::json;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

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
