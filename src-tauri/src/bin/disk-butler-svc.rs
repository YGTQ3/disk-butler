//! C盘管家后台扫描服务（DiskButlerScanSvc）。
//!
//! 以 LocalSystem 运行，仅提供只读的 MFT 磁盘扫描（命名管道 IPC），
//! 让主程序无需管理员权限即可秒级扫描。同 Everything 服务模式。
//!
//! 子命令：
//!   install      创建并启动服务（需管理员，由安装器调用）
//!   uninstall    停止并删除服务（需管理员，由卸载器调用）
//!   run-console  控制台前台运行（开发调试用，需管理员终端）
//!   scan-test C:\  直接跑一次 MFT 扫描并打印摘要（引擎验证用，需管理员终端）
//!   （无参数）    由 SCM 作为服务启动
//!
//! 保持控制台子系统：服务运行在会话 0 不会弹窗，而 run-console/install 的输出对调试和 NSIS 日志可见。

use disk_butler_lib::svc;
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

const SERVICE_NAME: &str = "DiskButlerScanSvc";
const DISPLAY_NAME: &str = "C盘管家 磁盘扫描服务";
const DESCRIPTION: &str =
    "为 C盘管家 提供只读的 MFT 秒级磁盘扫描。不联网、不写盘；禁用后主程序自动回退慢速扫描。";

define_windows_service!(ffi_service_main, service_main);

fn service_main(_args: Vec<OsString>) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_handler = stop.clone();

    let status_handle = match service_control_handler::register(SERVICE_NAME, move |control| {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stop_handler.store(true, Ordering::SeqCst);
                svc::nudge_server();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    }) {
        Ok(h) => h,
        Err(_) => return,
    };

    let running = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };
    let _ = status_handle.set_service_status(running);

    // 用完即停：只接待一个扫描请求，完成后服务立即退出（配合 OnDemand 实现无常驻进程）
    svc::run_server_once(stop);

    let stopped = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };
    let _ = status_handle.set_service_status(stopped);
}

fn install() -> Result<(), String> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|e| format!("连接服务管理器失败（需要管理员权限）：{}", e))?;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        // OnDemand + 用完即停：平时系统里只有注册项、没有进程；
        // 主程序扫描时按需拉起（ACL 放行普通用户 start），扫完服务自行退出
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };

    // 已存在（覆盖安装）则只更新配置，不存在则创建
    let service = match manager.create_service(
        &info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START,
    ) {
        Ok(s) => s,
        Err(_) => {
            let s = manager
                .open_service(
                    SERVICE_NAME,
                    ServiceAccess::CHANGE_CONFIG | ServiceAccess::START,
                )
                .map_err(|e| format!("打开已有服务失败：{}", e))?;
            s.change_config(&info)
                .map_err(|e| format!("更新服务配置失败：{}", e))?;
            s
        }
    };
    let _ = service.set_description(DESCRIPTION);

    // OnDemand 模式：安装时不启动（服务由主程序扫描时按需拉起，用完即停）。
    // 设置服务 DACL：在默认权限外授予 Authenticated Users 仅「启动(RP)+查询」权限
    // （不给停止权 WP——客户端不需要，最小授权），使普通权限主程序免 UAC 拉起只读扫描。
    const SERVICE_SDDL: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)(A;;RPCCLCSWLOCRRC;;;AU)";
    let out = std::process::Command::new("sc.exe")
        .args(["sdset", SERVICE_NAME, SERVICE_SDDL])
        .output()
        .map_err(|e| format!("设置服务权限失败（sc.exe 不可用）：{}", e))?;
    if !out.status.success() {
        return Err(format!(
            "设置服务权限失败：{}",
            String::from_utf8_lossy(&out.stdout)
        ));
    }
    Ok(())
}

fn uninstall() -> Result<(), String> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .map_err(|e| format!("连接服务管理器失败（需要管理员权限）：{}", e))?;
    let service = match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(s) => s,
        Err(_) => return Ok(()), // 本就不存在
    };
    let _ = service.stop();
    // 等待停止，最多 5 秒
    for _ in 0..50 {
        match service.query_status() {
            Ok(st) if st.current_state == ServiceState::Stopped => break,
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    service.delete().map_err(|e| format!("删除服务失败：{}", e))
}

fn run_console() {
    println!("[disk-butler-svc] 控制台模式，监听 {}，Ctrl+C 退出", svc::PIPE_NAME);
    svc::run_server(Arc::new(AtomicBool::new(false)));
}

/// 引擎验证：扫一次盘，打印耗时、总大小与顶层目录，不走管道。
fn scan_test(root: &str) {
    use std::time::Instant;
    let t0 = Instant::now();
    let result = disk_butler_lib::mft_scan::scan_mft(root, |files, bytes, percent, phase| {
        println!("  [{}] {:.1}%  files={} bytes={:.2} GB", phase, percent, files, bytes as f64 / (1 << 30) as f64);
    });
    match result {
        Ok(tree) => {
            println!("耗时 {:.2}s", t0.elapsed().as_secs_f32());
            println!("根：{}  总大小 {:.2} GB  直接子项 {} 个", tree.path, tree.size as f64 / (1 << 30) as f64, tree.children.len());
            for c in tree.children.iter().take(10) {
                println!("  {:>8.2} GB  {}", c.size as f64 / (1 << 30) as f64, c.name);
            }
            if tree.children.is_empty() || tree.size == 0 {
                eprintln!("⚠ 扫出空树，引擎存在问题！");
                std::process::exit(2);
            }
        }
        Err(e) => {
            eprintln!("扫描失败：{}", e);
            std::process::exit(1);
        }
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("install") => {
            if let Err(e) = install() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some("uninstall") => {
            if let Err(e) = uninstall() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some("run-console") => run_console(),
        Some("scan-test") => {
            let root = std::env::args().nth(2).unwrap_or_else(|| "C:\\".to_string());
            scan_test(&root);
        }
        _ => {
            // 由 SCM 调起；直接双击运行会立刻报错退出
            if service_dispatcher::start(SERVICE_NAME, ffi_service_main).is_err() {
                eprintln!("本程序是 Windows 服务，请用 install / uninstall / run-console 子命令。");
                std::process::exit(1);
            }
        }
    }
}
