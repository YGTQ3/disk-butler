//! 后台扫描服务的命名管道服务端与 IPC 协议。
//!
//! 协议（NDJSON，每行一个 JSON）：
//!   请求：{"cmd":"scan","root":"C:\\"}
//!   响应：{"type":"progress","filesScanned":..,"bytesScanned":..,"percent":..,"phase":".."} × N
//!         {"type":"done","tree":{...}} 或 {"type":"error","message":"..."}
//!
//! 服务只做只读的 MFT 扫描，不接受任何写操作指令。

use crate::mft_scan;
use serde_json::json;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 管道名（客户端用 std::fs::File 直接打开即可）。
pub const PIPE_NAME: &str = r"\\.\pipe\disk-butler-scan";

/// 进度节流：至少间隔多久才往管道写一条 progress。
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// 创建一个管道实例并阻塞等待客户端连接。
/// SDDL 允许 SYSTEM/管理员完全控制、已认证用户读写（普通权限主程序要能连上）。
fn wait_for_client() -> Result<File, String> {
    use windows_sys::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;

    const SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;AU)";
    const SDDL_REVISION_1: u32 = 1;
    const ERROR_PIPE_CONNECTED: u32 = 535;

    let sddl_w: Vec<u16> = SDDL.encode_utf16().chain(std::iter::once(0)).collect();
    let pipe_w: Vec<u16> = PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut sd = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_w.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(format!("构建管道安全描述符失败：{}", GetLastError()));
        }
        // sd 由 LocalAlloc 分配；服务生命周期内每个实例一次、量极小，不主动释放
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };

        let handle = CreateNamedPipeW(
            pipe_w.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,          // 单实例：一次只服务一个扫描
            64 * 1024,  // 出方向缓冲
            4 * 1024,   // 入方向缓冲
            0,
            &sa,
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("创建命名管道失败：{}", GetLastError()));
        }

        // 阻塞等待客户端；ERROR_PIPE_CONNECTED 表示客户端已抢先连上，同样算成功
        if ConnectNamedPipe(handle, std::ptr::null_mut()) == 0 {
            let err = GetLastError();
            if err != ERROR_PIPE_CONNECTED {
                windows_sys::Win32::Foundation::CloseHandle(handle);
                return Err(format!("等待客户端连接失败：{}", err));
            }
        }
        // 交给 File 接管句柄（drop 时自动断开并关闭）
        Ok(File::from_raw_handle(handle as _))
    }
}

/// 处理一个客户端：读一行请求，流式回写进度与结果。
fn serve_client(client: File) {
    let mut reader = BufReader::new(match client.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });
    let mut writer = client;

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }

    let req: serde_json::Value = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(_) => {
            let _ = writeln!(writer, "{}", json!({"type":"error","message":"请求格式错误"}));
            return;
        }
    };

    if req["cmd"] != "scan" {
        let _ = writeln!(writer, "{}", json!({"type":"error","message":"不支持的指令"}));
        return;
    }
    let root = req["root"].as_str().unwrap_or_default().to_string();

    // 扫描并流式推进度（阶段变化立即推，同阶段内节流；客户端中途断开则静默忽略写失败，扫描继续跑完）
    let mut last_emit = Instant::now() - PROGRESS_INTERVAL;
    let mut last_phase = String::new();
    let result = mft_scan::scan_mft(&root, |files, bytes, percent, phase| {
        if phase != last_phase || last_emit.elapsed() >= PROGRESS_INTERVAL {
            last_phase = phase.to_string();
            last_emit = Instant::now();
            let _ = writeln!(
                writer,
                "{}",
                json!({"type":"progress","filesScanned":files,"bytesScanned":bytes,"percent":percent,"phase":phase})
            );
        }
    });

    let final_msg = match result {
        Ok(tree) => json!({"type":"done","tree":tree}),
        Err(e) => json!({"type":"error","message":e}),
    };
    let _ = writeln!(writer, "{}", final_msg);
    let _ = writer.flush();
}

/// 管道服务主循环：顺序接待客户端，直到 stop 置位。
pub fn run_server(stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match wait_for_client() {
            Ok(client) => {
                if stop.load(Ordering::SeqCst) {
                    break; // 被停止指令用哑客户端唤醒
                }
                serve_client(client);
            }
            Err(_) => {
                // 创建/等待失败（如句柄资源异常），稍等重试避免空转
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

/// 唤醒阻塞在 ConnectNamedPipe 上的服务循环（停止服务时用）。
pub fn nudge_server() {
    let _ = File::options().read(true).write(true).open(PIPE_NAME);
}
