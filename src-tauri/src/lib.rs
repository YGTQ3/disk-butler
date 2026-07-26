mod cache;
mod cleanup;
mod knowledge;
mod memory;
mod scan;
mod startup;
mod stats;

use cache::ScanCache;
use cleanup::{CleanupReport, CleanupScan, DeepAnalyzeReport, DeepCleanReport};
use memory::MemoryReport;
use scan::{DriveInfo, TreeNode};
use startup::StartupItem;
use stats::CleanupStats;

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
            memory_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
