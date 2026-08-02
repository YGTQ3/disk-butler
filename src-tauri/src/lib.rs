// bloatware 模块始终编译：孤儿残留检测(scan/clean_orphans)已搬到「一键清理」属发布功能、依赖本模块；
// 「软件体检」相关命令仍由 feature_bloatware 门控是否注册（默认不注册=UI 不可达）。
// feature 关时软件体检那部分函数未被调用，允许 dead_code 以免告警。
#[cfg_attr(not(feature_bloatware), allow(dead_code))]
mod bloatware;
mod cache;
mod cleanup;
mod collector;
pub mod icon;
pub mod knowledge;
mod memory;
pub mod mft_scan;
pub mod scan;
mod startup;
mod stats;
pub mod svc;
mod svc_client;

use cache::ScanCache;
use cleanup::{CleanupReport, CleanupScan, DeepAnalyzeReport, DeepCleanReport, SystemAnalyzeReport, SystemCleanReport};
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
        // 记账用实际磁盘变化（而非各项 freed 之和），保证累计数字与用户可观测的 C 盘剩余空间一致
        stats::record(&app, r.free_after.saturating_sub(r.free_before));
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

/// 高级：系统级清理（清系统临时目录 + Windows 更新下载缓存，需 UAC 授权，单次提权）。
#[tauri::command]
async fn run_deep_clean_system(app: tauri::AppHandle) -> Result<SystemCleanReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let r = cleanup::deep_clean_system()?;
        stats::record(&app, r.freed);
        Ok(r)
    })
    .await
    .map_err(|e| format!("系统清理任务失败：{}", e))?
}

/// 高级：系统级清理·只读分析（提权量出临时目录 + 更新缓存大小，不做任何更改）。
#[tauri::command]
async fn analyze_system_clean() -> Result<SystemAnalyzeReport, String> {
    tauri::async_runtime::spawn_blocking(cleanup::analyze_system_clean)
        .await
        .map_err(|e| format!("分析任务失败：{}", e))?
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

/// 软件体检：陈述软件的客观行为（开机自启/后台常驻/占用较大），不定性、不点名（只读）。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn scan_bloatware(app: tauri::AppHandle, include_all: bool) -> Result<bloatware::BloatwareScan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ignored = bloatware::load_ignored(&app);
        bloatware::scan(&ignored, include_all)
    })
    .await
    .map_err(|e| format!("软件体检失败：{}", e))
}

/// 一键卸载：运行指定软件自带的官方卸载程序（后端据 id 重读卸载命令）。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn uninstall_software(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::uninstall(&id))
        .await
        .map_err(|e| format!("卸载任务失败：{}", e))?
}

/// 卸载后残留全景扫描：安装目录 + 各安装根下同名数据目录 + HKCU 注册表键（严格白名单）。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn scan_residue(
    name: String,
    publisher: String,
    install_location: String,
) -> Result<bloatware::ResidueScanReport, String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::scan_residue(&name, &publisher, &install_location))
        .await
        .map_err(|e| format!("残留扫描失败：{}", e))
}

/// 清理残留（目录+注册表键）：允许集合由后端当场重新推导，前端选中项必须命中才执行。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn clean_residue(
    name: String,
    publisher: String,
    install_location: String,
    dirs: Vec<String>,
    reg_keys: Vec<String>,
) -> Result<bloatware::ResidueReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        bloatware::clean_residue(&name, &publisher, &install_location, dirs, reg_keys)
    })
    .await
    .map_err(|e| format!("清理残留失败：{}", e))
}

/// 软件体检白名单：记录/取消用户"不再提醒"某软件（本地持久化）。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn bloatware_set_ignored(app: tauri::AppHandle, key: String, ignored: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::set_ignored(&app, key, ignored))
        .await
        .map_err(|e| format!("更新失败：{}", e))
}

/// 打开 Windows「应用和功能」设置页（无卸载命令的软件兜底）。
#[tauri::command]
fn open_apps_settings() -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg("ms-settings:appsfeatures")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开设置失败：{}", e))
}

/// 停止某软件的后台服务与进程（提权，破占用/自我保护）。后端据 id 重读安装目录，不信任前端直传路径。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn stop_software(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::stop_software(&id))
        .await
        .map_err(|e| format!("停止失败：{}", e))?
}

/// 强力卸载预览：列出将删除的安装目录/服务/计划任务/注册表项。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn bloatware_force_preview(id: String) -> Result<bloatware::ForcePlan, String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::force_preview(&id))
        .await
        .map_err(|e| format!("预览失败：{}", e))
}

/// 强力卸载：停/删服务、计划任务、进程、安装目录、注册表项（单次提权）。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn force_uninstall_software(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::force_uninstall(&id))
        .await
        .map_err(|e| format!("强力卸载任务失败：{}", e))?
}

/// 设置主窗口是否始终置顶（卸载期间置顶，遮盖厂商弹窗）。
#[tauri::command]
fn set_always_on_top(window: tauri::Window, on: bool) -> Result<(), String> {
    window.set_always_on_top(on).map_err(|e| e.to_string())
}

/// 提权操作是否已真正开始（UAC 已授权）——供前端进度条从"授权"推进到"执行中"。
#[cfg(feature_bloatware)]
#[tauri::command]
fn op_started() -> bool {
    bloatware::op_started()
}

/// 前台打开软件自带的官方卸载程序（安全软件等自我保护软件走引导，不提权/不静默）。
#[cfg(feature_bloatware)]
#[tauri::command]
async fn open_official_uninstaller(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::open_official_uninstaller(&id))
        .await
        .map_err(|e| format!("打开失败：{}", e))?
}

/// AppData 孤儿残留扫描：已卸载软件的遗留目录（知识库确证可清 + 未知只列出）。
#[tauri::command]
async fn scan_orphan_dirs() -> Result<bloatware::OrphanScan, String> {
    tauri::async_runtime::spawn_blocking(bloatware::scan_orphans)
        .await
        .map_err(|e| format!("残留检查失败：{}", e))
}

/// 清理孤儿残留目录：只接受后端当场重扫确证的白名单路径。
#[tauri::command]
async fn clean_orphan_dirs(paths: Vec<String>) -> Result<bloatware::ResidueReport, String> {
    tauri::async_runtime::spawn_blocking(move || bloatware::clean_orphans(paths))
        .await
        .map_err(|e| format!("清理失败：{}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    // 「软件体检」为未定稿大类功能：其命令仅在编译期开关 feature_bloatware 开启时注册，
    // 默认（公开包）不注册，与前端 __FEATURE_BLOATWARE__ 门控由同一环境变量驱动。
    #[cfg(feature_bloatware)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_drives,
        scan_drive,
        scan_dir,
        load_scan_cache,
        list_cleanup_items,
        run_cleanup,
        run_deep_analyze,
        run_deep_clean,
        run_deep_clean_system,
        analyze_system_clean,
        get_cleanup_stats,
        list_startup_items,
        set_startup_enabled,
        memory_report,
        pagefile_check,
        open_recycle_bin,
        collect_rules,
        scan_service_available,
        repair_scan_service,
        open_apps_settings,
        set_always_on_top,
        scan_bloatware,
        uninstall_software,
        scan_residue,
        clean_residue,
        bloatware_set_ignored,
        stop_software,
        bloatware_force_preview,
        force_uninstall_software,
        op_started,
        open_official_uninstaller,
        scan_orphan_dirs,
        clean_orphan_dirs
    ]);
    #[cfg(not(feature_bloatware))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        get_drives,
        scan_drive,
        scan_dir,
        load_scan_cache,
        list_cleanup_items,
        run_cleanup,
        run_deep_analyze,
        run_deep_clean,
        run_deep_clean_system,
        analyze_system_clean,
        get_cleanup_stats,
        list_startup_items,
        set_startup_enabled,
        memory_report,
        pagefile_check,
        open_recycle_bin,
        collect_rules,
        scan_service_available,
        repair_scan_service,
        open_apps_settings,
        set_always_on_top,
        scan_orphan_dirs,
        clean_orphan_dirs
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
