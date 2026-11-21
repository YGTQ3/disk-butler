//! 透视页「大空间目录一键转移」：复制 → 校验 → 原子换名 → 建 Junction → 清理临时源。
//!
//! 流水线思想逆向自 AppDataCleaner（egui 应用）的转移模块，按本仓库规范重建并修复其全部
//! 已知缺陷（docs/RULES-SECURITY · DEV-STANDARDS）：
//! - 预检兜底：源/目标合法性全部在后端重验，不信任前端直传路径；
//! - 删源不直接删：先同卷原子换名为 `原名.__transferring__`，建 Junction 失败即换名回退，
//!   任何失败路径源目录都原样保留（修复参考项目“删源后建链失败=断链”）;
//! - 空间预检 / 同名冲突 / 嵌套互斥 / 黑名单（系统目录、用户与 AppData 根、盘根）；
//! - OneDrive 云占位文件直接中止并大白话提示，绝不静默触发强制下载；
//! - 完成后 `stats::record` 记账 + 失效源盘扫描缓存。
//!
//! Junction（目录联接）免管理员权限、免 UAC，对应用程序透明：旧路径自动指向新位置。

use serde::Serialize;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 同一时刻只允许一个转移任务（前后端双保险，前端按钮置灰 + 后端 BUSY 拒绝）。
static TRANSFER_BUSY: AtomicBool = AtomicBool::new(false);
/// 取消请求：只在复制阶段响应（此后均为原子/快速操作，中断没有意义也不安全）。
static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

/// 忙标志的 RAII 守卫：流水线线程作用域退出（含 panic）都释放，
/// 防止流水线意外 panic 后 TRANSFER_BUSY 卡死、所有后续转移被拒。
struct BusyGuard;

impl Drop for BusyGuard {
    fn drop(&mut self) {
        TRANSFER_BUSY.store(false, Ordering::SeqCst);
    }
}

/// 换名临时目录的固定后缀（清理残留时也带此标记，便于识别）。
const TRANSFERRING_SUFFIX: &str = ".__transferring__";
/// OneDrive 等云盘“按需下载”的占位文件属性：读内容会强制联网下载，转移必须中止。
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

/// 是否云占位文件属性（任一 RECALL 位置位即命中）。
fn is_cloud_placeholder(attrs: u32) -> bool {
    attrs & (FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0
}
/// 复制阶段的取消哨兵。
const CANCEL_SENTINEL: &str = "__cancelled__";

// ---------- 结构体 ----------

/// 预检（只读分析）结果，供前端结果面板展示。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPlan {
    pub source_path: String,
    pub source_name: String,
    /// 源目录递归总大小（字节）
    pub source_size: u64,
    pub file_count: u64,
    pub dir_count: u64,
    /// 实际落位：目标目录\源目录名
    pub destination: String,
    pub target_letter: String,
    /// 目标盘当前剩余空间（字节）
    pub target_free: u64,
    /// 非致命提示（如跳过的链接子项数量）
    pub warnings: Vec<String>,
}

/// 转移进度事件 payload（前端 listen `transfer-progress`）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    /// measure | copy | verify | swap | link | cleanup | done | error | cancelled
    pub phase: String,
    /// 0~100
    pub percent: f32,
    pub current_path: String,
    pub message: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<TransferReport>,
}

/// 转移完成/失败报告。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferReport {
    /// 已转移到目标盘的字节数
    pub moved_bytes: u64,
    /// 实际从源盘清掉的字节数（临时源清理完成的才计入，并写入历史累计）
    pub freed_bytes: u64,
    pub file_count: u64,
    /// 跳过的 reparse 子项（源目录里的 junction/符号链接）
    pub skipped: Vec<String>,
    /// 清理阶段残留字节（文件被占用时 >0，不影响已生效的 Junction）
    pub temp_leftover: u64,
    /// 有残留时其实际路径（临时目录名带后缀，可能是 `原名.__transferring__` 或带数字序号）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_leftover_path: Option<String>,
    pub duration_secs: u64,
}

// ---------- 内部统计 ----------

/// 源目录快速统计（单遍遍历，只读元数据）。
#[derive(Debug, Default)]
struct SourceStat {
    size: u64,
    files: u64,
    dirs: u64,
    /// 源目录里的 junction/符号链接子项（复制时跳过，不追踪）
    skipped: Vec<String>,
    /// 是否含云占位文件（含则中止，绝不强制下载）
    has_placeholder: bool,
}

impl SourceStat {
    fn is_empty(&self) -> bool {
        self.files == 0 && self.dirs == 0
    }
}

// ---------- 预检与校验 ----------

/// 去掉 canonicalize 产生的 `\\?\` / `\\?\UNC\` 扩展前缀，还原成常规路径。
fn normalize(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", rest))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p
    }
}

/// 路径的卷根（如 `C:\`）；无法识别时返回 None。
fn volume_root(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        Some(format!("{}:\\", s.chars().next().unwrap().to_ascii_uppercase()))
    } else {
        None
    }
}

/// 是否盘根本身（canonicalize 后组件只有「卷前缀 + 根」两段，如 `C:\`）。
fn is_volume_root(p: &Path) -> bool {
    p.components().count() <= 2
}

/// 卷文件系统是否为 NTFS（Junction 只能在 NTFS 上建；FAT32/exFAT 会在建链阶段失败）。
fn is_ntfs(volume: &str) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
    let wide: Vec<u16> = volume.encode_utf16().chain(Some(0)).collect();
    let mut fs_name: [u16; 32] = [0; 32];
    let ok = unsafe {
        GetVolumeInformationW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    };
    if ok == 0 {
        return false;
    }
    let len = fs_name.iter().position(|c| *c == 0).unwrap_or(fs_name.len());
    String::from_utf16_lossy(&fs_name[..len]).eq_ignore_ascii_case("NTFS")
}

/// 系统关键目录黑名单：目录自身命中即拒绝（任何深度，这些名字不该被搬走）。
const BLOCKED_NAMES: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "system volume information",
    "$recycle.bin",
];
/// 父目录命中即拒绝：这些位置的直接子项要么是系统目录，要么是用户配置档案/AppData 根。
/// 注意只拦“直接子项”：`C:\Users\<用户>\Videos`、`AppData\Local\<某游戏>` 等深层目录仍可转移。
const BLOCKED_PARENT_NAMES: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "users",
    "appdata",
];

/// 源目录黑名单：命中返回大白话原因。
fn blocked_reason(path: &Path) -> Option<String> {
    // 盘根（C:\）没有 file_name，外层已单独拒绝；这里处理有名字的路径
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name.starts_with('$') {
        return Some(format!("「{}」是系统元数据目录，不支持转移", name));
    }
    if BLOCKED_NAMES.contains(&name.as_str()) {
        return Some(format!("「{}」是系统关键目录，转移会导致系统或软件异常", name));
    }
    if let Some(parent) = path.parent() {
        if let Some(pname) = parent.file_name() {
            let pname = pname.to_string_lossy().to_lowercase();
            if BLOCKED_PARENT_NAMES.contains(&pname.as_str()) {
                return Some(format!(
                    "「{}」位于系统目录「{}」的下一层，转移有风险，暂不支持",
                    name, pname
                ));
            }
        }
    }
    None
}

/// 目标卷是否为本地固定磁盘（GetDriveTypeW；拒绝 U 盘/移动硬盘/网络盘，中途拔盘会丢数据）。
fn is_fixed_drive(volume: &str) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let wide: Vec<u16> = volume.encode_utf16().chain(Some(0)).collect();
    // DRIVE_FIXED = 3
    unsafe { GetDriveTypeW(wide.as_ptr()) == 3 }
}

/// 目标卷剩余空间（GetDiskFreeSpaceExW，字节）。
fn free_space(volume: &str) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = volume.encode_utf16().chain(Some(0)).collect();
    let mut free_to_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free_to_caller, &mut total, &mut total_free) };
    if ok != 0 {
        Some(free_to_caller)
    } else {
        None
    }
}

/// 便宜校验（不做源树统计）：预检与 start_transfer 共用，保证“前端传来的路径”永远被后端重验。
/// 返回 (规范化源路径, 实际落位路径)。
fn validate(source: &str, target: &str) -> Result<(PathBuf, PathBuf), String> {
    // ---- 源 ----
    if source.trim().is_empty() {
        return Err("未指定要转移的目录".into());
    }
    let src = Path::new(source);
    let meta = fs::symlink_metadata(src).map_err(|_| format!("目录不存在或无法访问：{}", source))?;
    if meta.is_symlink() {
        return Err("这个目录本身已经是一个链接（Junction/符号链接），不需要再转移".into());
    }
    if !meta.is_dir() {
        return Err("要转移的路径不是一个目录".into());
    }
    let src = normalize(fs::canonicalize(src).map_err(|e| format!("无法解析目录：{}", e))?);
    if let Some(reason) = blocked_reason(&src) {
        return Err(reason);
    }
    // 盘根（C:\）没有父目录，无处放 Junction；盘根下一级（如 C:\Games）可以转移
    if is_volume_root(&src) {
        return Err("盘符根目录不支持转移".into());
    }

    // ---- 目标 ----
    if target.trim().is_empty() {
        return Err("未指定转移到的目标目录".into());
    }
    let tgt = Path::new(target);
    let tgt_meta = fs::metadata(tgt).map_err(|_| format!("目标目录不存在或无法访问：{}", target))?;
    if !tgt_meta.is_dir() {
        return Err("目标路径不是一个目录".into());
    }
    let tgt = normalize(fs::canonicalize(tgt).map_err(|e| format!("无法解析目标目录：{}", e))?);

    // ---- 卷校验：目标必须是另一块本地固定磁盘（转移是为了给源盘腾空间）----
    let src_vol = volume_root(&src).ok_or("无法识别源目录所在磁盘")?;
    let tgt_vol = volume_root(&tgt).ok_or("无法识别目标目录所在磁盘")?;
    if src_vol == tgt_vol {
        return Err(
            "目标目录必须和源目录不在同一块磁盘上——转移的目的是给源盘腾出空间，同一块盘内挪动省不出空间"
                .into(),
        );
    }
    if !is_fixed_drive(&tgt_vol) {
        return Err("目标所在磁盘不是本地固定磁盘（U 盘/移动硬盘/网络盘中途拔出会丢数据），请换一块内置磁盘".into());
    }
    // 源盘也必须是固定磁盘且 NTFS：Junction 依赖 NTFS，提前拦截避免白做几十 GB 的复制
    if !is_fixed_drive(&src_vol) {
        return Err("源目录所在磁盘不是本地固定磁盘，不支持转移".into());
    }
    if !is_ntfs(&src_vol) || !is_ntfs(&tgt_vol) {
        return Err("链接（Junction）只能在 NTFS 磁盘上创建。源盘或目标盘不是 NTFS（如 FAT32/exFAT 的 U 盘/移动硬盘），暂不支持转移".into());
    }

    // ---- 嵌套互斥 ----
    let src_prefix = src.as_os_str().to_os_string();
    let tgt_prefix = tgt.as_os_str().to_os_string();
    if tgt.starts_with(&src_prefix) {
        return Err("目标目录在源目录里面：转移会把目标一起搬走，形成套娃".into());
    }
    if src.starts_with(&tgt_prefix) {
        return Err("源目录在目标目录里面：转移会形成套娃，请换一个目标".into());
    }
    // 换名临时目录（源目录旁边）也不能是目标的一部分，否则清理时会连目标一起删
    let src_name = src.file_name().expect("前面已排除盘根").to_string_lossy().to_string();
    let temp = src.with_file_name(format!("{}{}", src_name, TRANSFERRING_SUFFIX));
    if tgt.starts_with(&temp) {
        return Err("目标目录不能选在源目录旁边即将创建的临时目录里".into());
    }

    // ---- 落位冲突：目标目录里不能已有同名文件夹 ----
    let destination = tgt.join(&src_name);
    if destination.symlink_metadata().is_ok() {
        return Err(format!(
            "目标目录里已存在同名文件夹「{}」，请换个目标目录或先处理同名文件夹",
            src_name
        ));
    }

    Ok((src, destination))
}

/// 单遍统计源树（只读元数据，不读文件内容）。
fn measure_source(root: &Path) -> SourceStat {
    let mut stat = SourceStat::default();
    for entry in jwalk::WalkDir::new(root).skip_hidden(false) {
        let Ok(entry) = entry else { continue };
        let ft = entry.file_type();
        let path = entry.path();
        if path == root {
            continue;
        }
        if ft.is_symlink() {
            stat.skipped.push(path.to_string_lossy().to_string());
            continue;
        }
        if ft.is_dir() {
            stat.dirs += 1;
            continue;
        }
        let Ok(md) = entry.metadata() else { continue };
        if is_cloud_placeholder(md.file_attributes()) {
            stat.has_placeholder = true;
        }
        stat.size = stat.size.saturating_add(md.len());
        stat.files += 1;
    }
    stat
}

/// 预检入口（只读，不改任何文件）：校验 + 统计 + 组装计划。
pub fn preflight(source: &str, target: &str) -> Result<TransferPlan, String> {
    let (src, destination) = validate(source, target)?;
    let stat = measure_source(&src);
    if stat.has_placeholder {
        return Err("目录里有未下载的云占位文件（如 OneDrive「按需下载」的文件），转移会强制联网下载甚至丢失内容。请先在云盘设置里让这些文件保持本地可用，再回来转移".into());
    }
    if stat.is_empty() {
        return Err("这个目录是空的，没有转移的价值".into());
    }

    let tgt_vol = volume_root(destination.parent().expect("落位路径必有父目录"))
        .ok_or("无法识别目标目录所在磁盘")?;
    let target_free = free_space(&tgt_vol).ok_or("无法读取目标磁盘剩余空间")?;
    if target_free <= stat.size {
        return Err(format!(
            "目标磁盘剩余空间不够：目录共 {}，目标盘只剩 {}，请换一块剩余空间更多的磁盘",
            format_bytes(stat.size),
            format_bytes(target_free)
        ));
    }

    let mut warnings = Vec::new();
    if !stat.skipped.is_empty() {
        warnings.push(format!(
            "源目录里有 {} 个链接子项（Junction/符号链接），转移时将跳过、留在原地",
            stat.skipped.len()
        ));
    }
    // 空间余量小于源大小 1.2 倍时提示（复制过程中系统/临时文件也在占空间）
    if target_free < stat.size.saturating_add(stat.size / 5) {
        warnings.push("目标磁盘剩余空间刚好够用，转移期间请不要往目标盘写入其他大文件".into());
    }

    Ok(TransferPlan {
        source_path: src.to_string_lossy().to_string(),
        source_name: src.file_name().unwrap().to_string_lossy().to_string(),
        source_size: stat.size,
        file_count: stat.files,
        dir_count: stat.dirs,
        destination: destination.to_string_lossy().to_string(),
        target_letter: tgt_vol.chars().next().unwrap().to_string(),
        target_free,
        warnings,
    })
}

// ---------- 命令入口 ----------

/// 是否有转移任务正在跑。
pub fn is_busy() -> bool {
    TRANSFER_BUSY.load(Ordering::SeqCst)
}

/// 请求取消（仅复制阶段响应）。返回是否成功受理（无任务进行中时为 false）。
pub fn request_cancel() -> bool {
    if !is_busy() {
        return false;
    }
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    true
}

/// 启动转移：快速校验通过后起后台线程跑流水线，进度经 `transfer-progress` 事件推送。
pub fn start(app: &AppHandle, source: &str, target: &str) -> Result<(), String> {
    if TRANSFER_BUSY
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("已有一个转移任务在进行中，请等它结束".into());
    }
    // 信任边界：启动前把全部便宜校验重跑（源树统计放线程里做，顺便当进度分母）
    let (src, destination) = match validate(source, target) {
        Ok(v) => v,
        Err(e) => {
            TRANSFER_BUSY.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let _guard = BusyGuard;
        run_pipeline(Some(&app), &src, &destination);
    });
    Ok(())
}

// ---------- 流水线 ----------

/// 进度上报：app 为 None 时不发事件（测试直跑流水线各步骤用）。
fn emit(app: Option<&AppHandle>, phase: &str, percent: f32, current: &str, message: &str) {
    let Some(app) = app else { return };
    let _ = app.emit(
        "transfer-progress",
        TransferProgress {
            phase: phase.to_string(),
            percent: percent.clamp(0.0, 100.0),
            current_path: current.to_string(),
            message: message.to_string(),
            done: matches!(phase, "done" | "error" | "cancelled"),
            report: None,
        },
    );
}

/// 删除自建半成品目标目录。`born` 是创建目标目录时的创建时间：长复制期间若外部程序
/// 恰好创建了同名目录（创建时间不同），绝不删别人的数据。
/// 返回 true = 目录没删掉（被保留，需提示用户手动处理）；false = 已删净/本就不在。
fn remove_partial_copy(destination: &Path, born: Option<std::time::SystemTime>) -> bool {
    // 目录都不存在（创建目标就失败）→ 无半成品可清理
    if destination.symlink_metadata().is_err() {
        return false;
    }
    let ours = born.is_some_and(|b| {
        fs::metadata(destination)
            .ok()
            .and_then(|m| m.created().ok())
            .is_some_and(|created| created == b)
    });
    if ours {
        fs::remove_dir_all(destination).is_err()
    } else {
        true // 无法确认是我们建的 → 保守保留
    }
}

/// 复制/校验阶段失败的统一收尾：删除自建半成品目标目录（源目录从未动过）。
fn abort_copy(destination: &Path, born: Option<std::time::SystemTime>, app: Option<&AppHandle>, msg: &str) {
    if remove_partial_copy(destination, born) {
        emit(
            app,
            "error",
            0.0,
            "",
            &format!(
                "{}\n注意：目标位置在转移期间被外部改动过，为保护数据未自动删除，副本保留在 {}，请手动确认后删除",
                msg,
                destination.display()
            ),
        );
    } else {
        emit(app, "error", 0.0, "", msg);
    }
}

/// 流水线主体：复制 → 校验 → 原子换名 → 建 Junction → 清理临时源。
/// 任何失败路径都保证源目录原样保留。
fn run_pipeline(app: Option<&AppHandle>, source: &Path, destination: &Path) {
    let started = Instant::now();
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    // 0. 统计源树（进度分母）
    emit(app, "measure", 0.0, &source.to_string_lossy(), "正在统计目录内容…");
    let stat = measure_source(source);
    if stat.is_empty() {
        abort_copy(destination, None, app, "这个目录是空的，没有转移的价值");
        return;
    }

    // 1. 复制（0~75%，可取消）。源目录原样在原地，复制失败/取消都无损伤
    emit(app, "copy", 0.0, "", "正在复制到目标磁盘…");
    let copy_result = copy_tree(source, destination, stat.size, &CANCEL_FLAG, app);
    let born = fs::metadata(destination)
        .ok()
        .and_then(|m| m.created().ok());
    let (copied_bytes, skipped) = match copy_result {
        Ok(v) => v,
        Err(msg) => {
            if msg == CANCEL_SENTINEL {
                if remove_partial_copy(destination, born) {
                    emit(
                        app,
                        "cancelled",
                        0.0,
                        "",
                        &format!(
                            "已取消：目标位置在转移期间被外部改动过，为保护数据半成品保留在 {}，请手动删除；源目录原样保留",
                            destination.display()
                        ),
                    );
                } else {
                    emit(app, "cancelled", 0.0, "", "已取消：半成品已清理，源目录原样保留");
                }
            } else {
                abort_copy(
                    destination,
                    born,
                    app,
                    &format!("复制中断：{}\n源目录没有任何改动，可以放心重试", msg),
                );
            }
            return;
        }
    };
    if !skipped.is_empty() {
        emit(
            app,
            "verify",
            75.0,
            "",
            &format!("已跳过 {} 个链接子项（保留在原地）", skipped.len()),
        );
    }

    // 2. 校验（75~85%）：两侧文件集合 + 逐文件大小必须完全一致
    emit(app, "verify", 75.0, "", "正在核对复制结果…");
    if let Err(msg) = verify_tree(source, destination) {
        abort_copy(
            destination,
            born,
            app,
            &format!("校验未通过：{}。为安全起见已中止，源目录没有任何改动", msg),
        );
        return;
    }

    // 3. 原子换名（85~88%）：源目录整目录改名保留在旁，删除与建链都基于它回退
    emit(app, "swap", 85.0, "", "正在做转移前最后的原子切换…");
    let Some(temp) = unique_temp_path(source) else {
        abort_copy(destination, born, app, "无法创建临时目录名，请重试");
        return;
    };
    if let Err(e) = fs::rename(source, &temp) {
        if remove_partial_copy(destination, born) {
            emit(
                app,
                "error",
                0.0,
                "",
                &format!(
                    "源目录里有文件正被使用（{}），转移已中止。目标副本保留在 {}（请手动确认后删除），源目录原样保留。请关闭正在使用该目录的软件后重试",
                    e,
                    destination.display()
                ),
            );
        } else {
            emit(
                app,
                "error",
                0.0,
                "",
                &format!(
                    "源目录里有文件正被使用（{}），转移已中止：目标副本已清理，源目录原样保留。请关闭正在使用该目录的软件后重试",
                    e
                ),
            );
        }
        return;
    }

    // 4. 建 Junction（88~90%）：旧路径即刻恢复可用，指向新位置
    emit(app, "link", 88.0, "", "正在建立路径兼容（Junction）…");
    if let Err(e) = junction::create(destination, source) {
        // 建链失败 → 换名回退，一切恢复原状（修复参考项目断链缺陷的关键）
        if fs::rename(&temp, source).is_ok() {
            if remove_partial_copy(destination, born) {
                emit(
                    app,
                    "error",
                    0.0,
                    "",
                    &format!(
                        "建立 Junction 失败（{}），源目录已恢复，但目标副本没删掉：{}。请确认后手动删除",
                        e,
                        destination.display()
                    ),
                );
            } else {
                emit(
                    app,
                    "error",
                    0.0,
                    "",
                    &format!("建立 Junction 失败（{}），已完整恢复原状，请把错误反馈给开发者", e),
                );
            }
        } else {
            emit(
                app,
                "error",
                0.0,
                "",
                &format!(
                    "建立 Junction 失败（{}），且自动恢复也失败：数据完好保存在 {}，请手动把「{}」改名回「{}」",
                    e,
                    destination.display(),
                    temp.display(),
                    source.display()
                ),
            );
        }
        return;
    }

    // 5. 清理临时源（90~100%）：逐文件删除并记账，被占用文件如实报告
    emit(app, "cleanup", 90.0, "", "正在删除源盘上的临时副本…");
    let (freed, leftover_bytes) = delete_tree_with_retry(&temp, stat.size, app);

    // 6. 收尾：记账 + 失效源盘缓存
    if let Some(app) = app {
        crate::stats::record(app, freed); // 纪律：清理必记账
        invalidate_source_cache(app, source);
    }
    let report = TransferReport {
        moved_bytes: copied_bytes,
        freed_bytes: freed,
        file_count: stat.files,
        skipped,
        temp_leftover: leftover_bytes,
        temp_leftover_path: if leftover_bytes > 0 {
            Some(temp.to_string_lossy().to_string())
        } else {
            None
        },
        duration_secs: started.elapsed().as_secs(),
    };
    if let Some(app) = app {
        let _ = app.emit(
            "transfer-progress",
            TransferProgress {
                phase: "done".into(),
                percent: 100.0,
                current_path: String::new(),
                message: if leftover_bytes > 0 {
                    format!(
                        "转移完成。临时目录有 {} 因文件被占用未删净，不影响使用，可稍后在资源管理器手动删除「{}」",
                        format_bytes(leftover_bytes),
                        temp.display()
                    )
                } else {
                    "转移完成".into()
                },
                done: true,
                report: Some(report),
            },
        );
    }
}

/// 递归复制，返回 (已复制字节, 跳过的链接子项)。出错返回 Err（含文件路径与原因）。
/// `cancel` 由调用方注入（流水线传全局标志，测试传本地标志，避免并发测试互扰）。
fn copy_tree(
    src_root: &Path,
    dst_root: &Path,
    total_bytes: u64,
    cancel: &AtomicBool,
    app: Option<&AppHandle>,
) -> Result<(u64, Vec<String>), String> {
    let mut copied: u64 = 0;
    let mut skipped: Vec<String> = Vec::new();
    let mut last_emit = Instant::now() - Duration::from_secs(1);

    fs::create_dir_all(dst_root).map_err(|e| format!("创建目标目录失败：{}", e))?;
    rec(
        src_root,
        src_root,
        dst_root,
        total_bytes,
        cancel,
        &mut copied,
        &mut skipped,
        &mut last_emit,
        app,
    )?;
    Ok((copied, skipped))
}

/// copy_tree 的递归体。
fn rec(
    src: &Path,
    src_root: &Path,
    dst_root: &Path,
    total_bytes: u64,
    cancel: &AtomicBool,
    copied: &mut u64,
    skipped: &mut Vec<String>,
    last_emit: &mut Instant,
    app: Option<&AppHandle>,
) -> Result<(), String> {
    let entries = fs::read_dir(src).map_err(|e| format!("读取目录 {} 失败：{}", src.display(), e))?;
    for entry in entries {
        if cancel.load(Ordering::SeqCst) {
            return Err(CANCEL_SENTINEL.into());
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };

        if ft.is_symlink() {
            // 不追踪链接：内容语义会变（指向的数据可能在别处），跳过并如实报告
            skipped.push(path.to_string_lossy().to_string());
            continue;
        }
        let rel = path.strip_prefix(src_root).expect("read_dir 结果必在根之下");
        let target = dst_root.join(rel);

        if ft.is_dir() {
                fs::create_dir_all(&target).map_err(|e| format!("创建目录 {} 失败：{}", target.display(), e))?;
                rec(
                    &path,
                    src_root,
                    dst_root,
                    total_bytes,
                    cancel,
                    copied,
                    skipped,
                    last_emit,
                    app,
                )?;
        } else {
                let md = entry
                    .metadata()
                    .map_err(|e| format!("读取 {} 信息失败：{}", path.display(), e))?;
                if is_cloud_placeholder(md.file_attributes()) {
                    return Err(format!(
                        "{} 是云盘占位文件（内容还没下载到本地），继续转移会强制下载或丢失内容。请先在云盘设置里让文件保持本地可用",
                        path.display()
                    ));
                }
            fs::copy(&path, &target)
                .map_err(|e| format!("复制 {} 失败：{}（源目录没有任何改动）", path.display(), e))?;
            *copied = copied.saturating_add(md.len());
            // 节流上报：200ms
            if last_emit.elapsed() >= Duration::from_millis(200) {
                *last_emit = Instant::now();
                let pct = if total_bytes > 0 {
                    (*copied as f32 / total_bytes as f32) * 75.0
                } else {
                    37.5
                };
                emit(app, "copy", pct, &path.to_string_lossy(), "正在复制到目标磁盘…");
            }
        }
    }
    Ok(())
}

/// 校验：两侧文件相对路径集合与逐文件大小、目录集合必须完全一致（只读元数据，秒级）。
fn verify_tree(src_root: &Path, dst_root: &Path) -> Result<(), String> {
    let mut src_files: Vec<(PathBuf, u64)> = Vec::new();
    let mut src_dirs: Vec<PathBuf> = Vec::new();
    collect(src_root, src_root, &mut src_files, &mut src_dirs)?;
    let mut dst_files: Vec<(PathBuf, u64)> = Vec::new();
    let mut dst_dirs: Vec<PathBuf> = Vec::new();
    collect(dst_root, dst_root, &mut dst_files, &mut dst_dirs)?;

    src_files.sort_unstable();
    dst_files.sort_unstable();
    if src_files.len() != dst_files.len() {
        return Err(format!(
            "文件数量不一致（源 {} 个，复制后 {} 个）",
            src_files.len(),
            dst_files.len()
        ));
    }
    for ((sp, ss), (dp, ds)) in src_files.iter().zip(dst_files.iter()) {
        if sp != dp {
            return Err(format!("文件清单不一致：{}", sp.display()));
        }
        if ss != ds {
            return Err(format!("文件大小不一致：{}", sp.display()));
        }
    }
    src_dirs.sort_unstable();
    dst_dirs.sort_unstable();
    if src_dirs != dst_dirs {
        return Err("目录结构不一致".into());
    }
    Ok(())
}

/// 收集相对路径清单（跳过链接子项，与复制阶段口径一致）。
/// `top` 是固定的顶层根：递归时相对路径始终相对它，保证 `b/inner/d.txt` 这类深层路径不丢前缀。
fn collect(
    dir: &Path,
    top: &Path,
    files: &mut Vec<(PathBuf, u64)>,
    dirs: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("读取目录 {} 失败：{}", dir.display(), e))?;
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let rel = path.strip_prefix(top).expect("遍历结果必在顶层之下").to_path_buf();
        if ft.is_dir() {
            dirs.push(rel);
            collect(&path, top, files, dirs)?;
        } else {
            let len = entry
                .metadata()
                .map_err(|e| format!("读取 {} 信息失败：{}", path.display(), e))?
                .len();
            files.push((rel, len));
        }
    }
    Ok(())
}

/// 换名临时路径：`原名.__transferring__`，被占用（历史残留）时追加序号。
fn unique_temp_path(source: &Path) -> Option<PathBuf> {
    let name = source.file_name()?.to_string_lossy().to_string();
    let mut candidate = source.with_file_name(format!("{}{}", name, TRANSFERRING_SUFFIX));
    let mut n = 0;
    while candidate.symlink_metadata().is_ok() {
        n += 1;
        candidate = source.with_file_name(format!("{}{}{}", name, TRANSFERRING_SUFFIX, n));
        if n > 99 {
            return None;
        }
    }
    Some(candidate)
}

/// 逐文件删除临时源并统计释放字节；被占用/只读文件先摘只读再试，整树最多重试 3 遍。
/// 返回 (已释放字节, 残留字节)。
fn delete_tree_with_retry(root: &Path, total_bytes: u64, app: Option<&AppHandle>) -> (u64, u64) {
    let mut freed: u64 = 0;
    let mut locked: Vec<PathBuf> = Vec::new();
    for _pass in 0..3 {
        // freed 跨遍累加：前一遍删掉的字节后面不会重复统计（文件已不在树上）
        locked.clear();
        delete_pass(root, &mut freed, &mut locked, total_bytes, app);
        if locked.is_empty() {
            break;
        }
        // 锁定文件可能几秒内被释放，稍候重试
        std::thread::sleep(Duration::from_millis(800));
    }
    if fs::remove_dir(root).is_err() {
        // 目录非空（含残留）：统计残留量
        let leftover = measure_source(root).size;
        return (freed, leftover);
    }
    (freed, 0)
}

/// 单遍删除（后序：先递归子目录再删自身），只摘只读位，不等待锁定文件。
fn delete_pass(
    dir: &Path,
    freed: &mut u64,
    locked: &mut Vec<PathBuf>,
    total_bytes: u64,
    app: Option<&AppHandle>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            delete_pass(&path, freed, locked, total_bytes, app);
            let _ = fs::remove_dir(&path); // 非空（含残留）时失败，留待下遍
            continue;
        }
        let Ok(md) = entry.metadata() else { continue };
        let size = md.len();
        if fs::remove_file(&path).is_ok() {
            *freed = freed.saturating_add(size);
        } else {
            // 只读位会挡删除：摘掉再试一次
            let mut cleared = false;
            if md.permissions().readonly() {
                let mut p = md.permissions();
                p.set_readonly(false);
                cleared = fs::set_permissions(&path, p).is_ok() && fs::remove_file(&path).is_ok();
            }
            if cleared {
                *freed = freed.saturating_add(size);
            } else {
                locked.push(path);
                continue;
            }
        }
        // 节流上报（每 64MB）：90% 起步按删除量推进
        if total_bytes > 0 && *freed > 0 && *freed / 67_108_864 != (*freed - size) / 67_108_864 {
            let pct = 90.0 + ((*freed as f32 / total_bytes as f32).min(1.0)) * 10.0;
            emit(app, "cleanup", pct, "", "正在删除源盘上的临时副本…");
        }
    }
}

/// 转移成功后源盘扫描缓存已失真：直接删除缓存文件，回页时按“无缓存”处理。
fn invalidate_source_cache(app: &AppHandle, source: &Path) {
    let Some(vol) = volume_root(source) else { return };
    crate::cache::invalidate(app, &vol);
}

/// 字节数大白话（后端错误文案用；与前端 formatBytes 口径一致）。
pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".into();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let i = (((bytes as f64).log2() / 10.0).floor() as usize).min(UNITS.len() - 1);
    let v = bytes as f64 / 2f64.powi(10 * i as i32);
    if v >= 100.0 {
        format!("{:.0} {}", v, UNITS[i])
    } else {
        format!("{:.1} {}", v, UNITS[i])
    }
}

// ---------- 测试 ----------

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一棵测试树：3 个文件共 6 字节 + 3 个目录（b / b/inner / empty）。
    fn make_tree(root: &Path) {
        fs::create_dir_all(root.join("b/inner")).unwrap();
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::write(root.join("a.txt"), "1").unwrap();
        fs::write(root.join("b/c.txt"), "22").unwrap();
        fs::write(root.join("b/inner/d.txt"), "333").unwrap();
    }

    #[test]
    fn blocked_names_and_parents() {
        assert!(blocked_reason(Path::new(r"C:\Windows")).is_some());
        assert!(blocked_reason(Path::new(r"C:\Program Files\App")).is_some());
        assert!(blocked_reason(Path::new(r"C:\ProgramData\x")).is_some());
        assert!(blocked_reason(Path::new(r"C:\Users\张三")).is_some());
        assert!(blocked_reason(Path::new(r"C:\Users\张三\AppData")).is_some());
        assert!(blocked_reason(Path::new(r"C:\Users\张三\AppData\Local")).is_some());
        assert!(blocked_reason(Path::new(r"D:\$WinREAgent")).is_some());
        // 深层目录放行：父目录不在黑名单
        assert!(blocked_reason(Path::new(r"C:\Users\张三\Videos")).is_none());
        assert!(blocked_reason(Path::new(r"C:\Users\张三\AppData\Local\SomeGame")).is_none());
    }

    #[test]
    fn format_bytes_ergonomics() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(6), "6.0 B");
        assert_eq!(format_bytes(1_610_612_736), "1.5 GB");
    }

    #[test]
    fn validate_rejects_missing_source() {
        assert!(validate(r"C:\__disk_butler_no_such_dir__", r"D:\").is_err());
    }

    #[test]
    fn validate_rejects_same_volume() {
        // 源与目标都放临时目录（同盘）→ 必须触发同盘拒绝（带 PID，多进程并发跑测试互不干扰）
        let tmp = std::env::temp_dir();
        let dir = tmp.join(format!("db_transfer_same_vol_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let err = validate(dir.to_str().unwrap(), tmp.to_str().unwrap()).err().unwrap();
        assert!(err.contains("同一块磁盘"), "实际错误：{}", err);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn volume_root_check_allows_first_level_dirs() {
        // 盘根拒绝、盘根下一级放行（主用例：C:\Games 这类大目录必须能转移）
        assert!(is_volume_root(Path::new(r"C:\")));
        assert!(is_volume_root(Path::new(r"D:\")));
        assert!(!is_volume_root(Path::new(r"C:\Games")));
        assert!(!is_volume_root(Path::new(r"C:\Users\张三\Videos")));
    }

    #[test]
    fn unique_temp_path_appends_suffix_on_collision() {
        let base = std::env::temp_dir().join(format!("db_transfer_uniq_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src_dir");
        fs::create_dir_all(&src).unwrap();
        let first = unique_temp_path(&src).unwrap();
        assert!(first.to_string_lossy().ends_with(TRANSFERRING_SUFFIX));
        fs::create_dir_all(&first).unwrap(); // 占住第一个名字
        let second = unique_temp_path(&src).unwrap();
        assert!(second.to_string_lossy().ends_with(&format!("{}1", TRANSFERRING_SUFFIX)));
        assert_ne!(first, second);
        let _ = fs::remove_dir_all(&base);
    }

    /// 端到端（同卷临时目录内，绕过跨卷校验）：复制 → 校验 → 换名 → 建 Junction →
    /// 清理，验证旧路径经 Junction 读到新内容、临时目录清理干净。
    #[test]
    fn pipeline_copy_verify_swap_link_cleanup() {
        let base = std::env::temp_dir().join(format!("db_transfer_e2e_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src_dir");
        let dst = base.join("dst_root").join("src_dir");
        make_tree(&src);

        let stat = measure_source(&src);
        assert_eq!(stat.files, 3);
        assert_eq!(stat.dirs, 3); // b、b/inner、empty
        assert_eq!(stat.size, 6);
        assert!(!stat.is_empty());

        // 复制 + 校验
        let idle = AtomicBool::new(false);
        let (copied, skipped) = copy_tree(&src, &dst, stat.size, &idle, None).unwrap();
        assert_eq!(copied, 6);
        assert!(skipped.is_empty());
        verify_tree(&src, &dst).unwrap();

        // 换名 + 建 Junction：旧路径即刻可读新内容
        let temp = unique_temp_path(&src).unwrap();
        assert!(temp.to_string_lossy().ends_with(TRANSFERRING_SUFFIX));
        fs::rename(&src, &temp).unwrap();
        junction::create(&dst, &src).unwrap();
        assert!(junction::exists(&src).unwrap());
        // NTFS 大小写不敏感：junction 保留建链时的大小写，canonicalize 全大写，
        // 故按小写比较
        assert_eq!(
            normalize(junction::get_target(&src).unwrap()).to_string_lossy().to_lowercase(),
            normalize(fs::canonicalize(&dst).unwrap()).to_string_lossy().to_lowercase()
        );
        assert_eq!(fs::read(src.join("b/inner/d.txt")).unwrap(), b"333");

        // 清理临时源：全部删净
        let (freed, leftover) = delete_tree_with_retry(&temp, stat.size, None);
        assert_eq!(freed, 6, "释放字节数不符");
        assert_eq!(leftover, 0, "不应有残留");
        assert!(!temp.exists(), "临时目录应已删净");
        // Junction 仍完好
        assert_eq!(fs::read(src.join("a.txt")).unwrap(), b"1");

        let _ = fs::remove_dir_all(&base);
    }

    /// 校验必须能发现两侧不一致（篡改目标文件大小后 verify_tree 必须报错）。
    #[test]
    fn verify_tree_detects_mismatch() {
        let base = std::env::temp_dir().join(format!("db_transfer_verify_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src");
        let dst = base.join("dst");
        make_tree(&src);
        let idle = AtomicBool::new(false);
        copy_tree(&src, &dst, 0, &idle, None).unwrap();
        // 篡改一个文件
        fs::write(dst.join("a.txt"), "changed").unwrap();
        assert!(verify_tree(&src, &dst).is_err());
        // 少复制一个文件
        fs::remove_file(dst.join("b/c.txt")).unwrap();
        fs::write(dst.join("a.txt"), "1").unwrap();
        assert!(verify_tree(&src, &dst).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    /// 云占位属性判定（确定性单测：OS 不允许在普通文件上置 RECALL 位，
    /// 直接测谓词；真实 OneDrive 文件的识别由该谓词在生产路径覆盖）
    #[test]
    fn is_cloud_placeholder_predicate() {
        assert!(!is_cloud_placeholder(0x80)); // FILE_ATTRIBUTE_NORMAL
        assert!(is_cloud_placeholder(FILE_ATTRIBUTE_RECALL_ON_OPEN));
        assert!(is_cloud_placeholder(FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS));
        assert!(is_cloud_placeholder(
            FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
        ));
    }

    /// 取消哨兵：置起取消标志后 copy_tree 必须返回 CANCEL_SENTINEL。
    #[test]
    fn copy_tree_honours_cancel() {
        let base = std::env::temp_dir().join(format!("db_transfer_cancel_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let src = base.join("src");
        let dst = base.join("dst");
        make_tree(&src);
        // 用本地取消标志（不碰全局 CANCEL_FLAG，并发跑其他测试不会互扰）
        let cancel = AtomicBool::new(true);
        let err = copy_tree(&src, &dst, 0, &cancel, None).err().unwrap();
        assert_eq!(err, CANCEL_SENTINEL);
        let _ = fs::remove_dir_all(&base);
    }
}
