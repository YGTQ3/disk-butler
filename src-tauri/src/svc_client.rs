//! 后台扫描服务的管道客户端：主程序（普通权限）经命名管道请求 MFT 秒级扫描。

use crate::scan::TreeNode;
use crate::svc::PIPE_NAME;
use serde_json::json;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

/// 通过后台服务扫描。服务不在/协议异常时返回 Err，由调用方回退其他引擎。
/// progress(已发现文件数, 已统计字节数, 精确百分比 0~100, 阶段文案)
pub fn scan_via_service<F: FnMut(u64, u64, f32, &str)>(
    root: &str,
    mut progress: F,
) -> Result<TreeNode, String> {
    // 客户端侧用 std File 打开命名管道即可，无需额外依赖
    let mut pipe = File::options()
        .read(true)
        .write(true)
        .open(PIPE_NAME)
        .map_err(|e| format!("后台扫描服务未运行：{}", e))?;

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
