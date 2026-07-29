mod cache;
mod cleanup;
mod collector;
mod icon;
pub mod knowledge;
mod memory;
pub mod mft_scan;
pub mod scan;
mod startup;
mod stats;
pub mod svc;
mod svc_client;
mod uninstall;

use cache::ScanCache;
use cleanup::{CleanupReport, CleanupScan, DeepAnalyzeReport, DeepCleanReport};
use memory::MemoryReport;
use scan::{DriveInfo, TreeNode};
use startup::StartupItem;
use stats::CleanupStats;
use uninstall::{InstalledApp, LeftoverInput, LeftoverItem, RemoveResult};

/// 枚举系统盘符及容量。
#[tauri::command]
fn get_drives() -> Vec<DriveInfo> {
    scan::list_drives()
}

/// 扫描指定根目录（如 "C:\\"），后台线程执行并推送进度事件，完成后自动写入本地缓存。
#[tauri::command]
async fn scan_drive(app: tauri::AppHandle, root: String) -> Result<TreeNode, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let tree = scan::scan(&root, Some(&app))?;
        cache::save(&app, &root, &tree);
        Ok(tree)
    })
    .await
    .map_err(|e| format!("扫描任务失败：{}", e))?
}

/// 读取某盘上次扫描的本地缓存（无则返回 null）。
#[tauri::command]
async fn load_scan_cache(app: tauri::AppHandle, root: String) -> Option<ScanCache> {
    tauri::async_runtime::spawn_blocking(move || cache::load(&app, &root))
        .await
        .ok()
        .flatten()
}

/// 按需下钻：扫描更深层目录（不推送进度，用于 TreeMap 展开下级）。
#[tauri::command]
async fn scan_dir(path: String) -> Result<TreeNode, String> {
    tauri::async_runtime::spawn_blocking(move || scan::scan(&path, None))
        .await
        .map_err(|e| format!("扫描任务失败：{}", e))?
}

/// 枚举白名单清理项及当前大小 + 磁盘空间水位（含磁盘扫描，后台线程执行）。
#[tauri::command]
async fn list_cleanup_items() -> Result<CleanupScan, String> {
    tauri::async_runtime::spawn_blocking(cleanup::list_items)
        .await
        .map_err(|e| format!("扫描清理项失败：{}", e))
}

/// 高级：DISM 只读分析（需 UAC 授权，约 1~3 分钟，不做任何更改）。
#[tauri::command]
async fn run_deep_analyze() -> Result<DeepAnalyzeReport, String> {
    tauri::async_runtime::spawn_blocking(cleanup::deep_analyze)
        .await
        .map_err(|e| format!("分析任务失败：{}", e))?
}

/// 高级：系统深度清理（DISM 组件存储，需 UAC 授权，耗时 5~20 分钟）。
#[tauri::command]
async fn run_deep_clean(app: tauri::AppHandle) -> Result<DeepCleanReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let r = cleanup::deep_clean()?;
        stats::record(&app, r.freed);
        Ok(r)
    })
    .await
    .map_err(|e| format!("深度清理任务失败：{}", e))?
}

/// 执行清理（只接受白名单 id，路径由后端重新解析）。
#[tauri::command]
async fn run_cleanup(app: tauri::AppHandle, ids: Vec<String>) -> Result<CleanupReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let r = cleanup::run(ids);
        stats::record(&app, r.total_freed);
        r
    })
    .await
    .map_err(|e| format!("清理任务失败：{}", e))
}

/// 累计清理统计（本地持久化，仅用于成就感展示）。
#[tauri::command]
fn get_cleanup_stats(app: tauri::AppHandle) -> CleanupStats {
    stats::load(&app)
}

/// 枚举启动项（注册表 Run + 启动文件夹，含建议标签与运行内存）。
#[tauri::command]
async fn list_startup_items() -> Result<Vec<StartupItem>, String> {
    tauri::async_runtime::spawn_blocking(startup::list_items)
        .await
        .map_err(|e| format!("枚举启动项失败：{}", e))
}

/// 切换启动项启用/禁用（与任务管理器等效、可逆）。
#[tauri::command]
async fn set_startup_enabled(id: String, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || startup::set_enabled(&id, enabled))
        .await
        .map_err(|e| format!("切换失败：{}", e))?
}

/// 内存体检报告（水位 + 进程分组 Top20）。
#[tauri::command]
async fn memory_report() -> Result<MemoryReport, String> {
    tauri::async_runtime::spawn_blocking(memory::report)
        .await
        .map_err(|e| format!("内存体检失败：{}", e))
}

/// 页面文件一致性核验（注册表配置 vs 本次开机实际启用）。
#[tauri::command]
async fn pagefile_check() -> Result<memory::PagefileCheck, String> {
    tauri::async_runtime::spawn_blocking(memory::pagefile_check)
        .await
        .map_err(|e| format!("页面文件核验失败：{}", e))
}

/// 打开系统回收站窗口（虚拟目录，走 shell: 协议而非文件路径）。
#[tauri::command]
fn open_recycle_bin() -> Result<(), String> {
    cleanup::open_recycle_bin()
}

/// 生成规则采集报告（写到桌面，由用户自主决定是否分享）。
#[tauri::command]
async fn collect_rules(include_drives: bool) -> Result<collector::CollectResult, String> {
    tauri::async_runtime::spawn_blocking(move || collector::collect(include_drives))
        .await
        .map_err(|e| format!("采集失败：{}", e))?
}

/// 自检：后台 MFT 秒扫服务是否已注册。false = 已被移除、扫描将回退慢速遍历。
#[tauri::command]
fn scan_service_available() -> bool {
    svc_client::service_installed()
}

/// 一键修复：重装扫描服务恢复秒级加速（提权，弹一次 UAC）。
#[tauri::command]
async fn repair_scan_service() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(svc_client::repair_service)
        .await
        .map_err(|e| format!("修复任务失败：{}", e))?
}

/// 枚举已安装的桌面软件（注册表 Uninstall 键）。
#[tauri::command]
async fn list_installed_apps() -> Result<Vec<InstalledApp>, String> {
    tauri::async_runtime::spawn_blocking(uninstall::list_installed)
        .await
        .map_err(|e| format!("枚举已装软件失败：{}", e))
}

/// 调用软件自带卸载器（向导模式），阻塞等待其结束，返回退出码。
#[tauri::command]
async fn run_app_uninstaller(id: String) -> Result<i32, String> {
    tauri::async_runtime::spawn_blocking(move || uninstall::run_uninstaller(&id))
        .await
        .map_err(|e| format!("卸载任务失败：{}", e))?
}

/// 扫描残留（文件夹 + 注册表）；卸载后原注册表键已删，按前端缓存字段扫描。
#[tauri::command]
async fn scan_app_leftovers(
    name: String,
    publisher: String,
    install_location: String,
) -> Result<Vec<LeftoverItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        uninstall::scan_leftovers(&name, &publisher, &install_location)
    })
    .await
    .map_err(|e| format!("扫描残留失败：{}", e))
}

/// 删除勾选的残留项（文件夹进回收站、注册表先导出 .reg 备份后删）。
#[tauri::command]
async fn remove_app_leftovers(items: Vec<LeftoverInput>) -> Result<Vec<RemoveResult>, String> {
    tauri::async_runtime::spawn_blocking(move || uninstall::remove_leftovers(items))
        .await
        .map_err(|e| format!("删除残留失败：{}", e))
}

/// 提取应用图标（按需、逐个）：优先 DisplayIcon，取不到逐级退回安装目录/卸载器主 exe。返回 PNG base64 data URI。
#[tauri::command]
async fn app_icon(
    icon_path: String,
    install_location: String,
    uninstall_string: String,
    name: String,
) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        icon::icon_data_uri(&icon_path, &install_location, &uninstall_string, &name)
    })
    .await
    .ok()
    .flatten()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_drives,
            scan_drive,
            scan_dir,
            load_scan_cache,
            list_cleanup_items,
            run_cleanup,
            run_deep_analyze,
            run_deep_clean,
            get_cleanup_stats,
            list_startup_items,
            set_startup_enabled,
            memory_report,
            pagefile_check,
            open_recycle_bin,
            collect_rules,
            scan_service_available,
            repair_scan_service,
            list_installed_apps,
            run_app_uninstaller,
            scan_app_leftovers,
            remove_app_leftovers,
            app_icon
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
