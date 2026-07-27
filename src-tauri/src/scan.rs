//! 磁盘扫描引擎：并行遍历目录、聚合大小、剪枝生成树，供前端 TreeMap 使用。

use crate::knowledge::{self, Category, Safety};
use jwalk::WalkDir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 每层保留的子项数量上限，其余聚合为「其他」。
pub const TOP_N_PER_LEVEL: usize = 30;
/// 返回给前端的树的最大深度（从扫描根算起）。
pub const MAX_DEPTH: usize = 4;

/// 树节点，直接序列化给前端（也用于扫描结果缓存的存取）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    /// 该目录是否还有更深内容未展开（前端据此决定能否继续下钻）。
    pub has_children: bool,
    pub category: Category,
    pub friendly_name: String,
    pub description: String,
    pub safety: Safety,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<TreeNode>,
}

/// 扫描进度事件（emit 给前端）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub files_scanned: u64,
    pub bytes_scanned: u64,
    pub current_path: String,
    pub done: bool,
    /// 精确百分比（MFT 引擎提供；慢速引擎为 None，由前端按字节估算）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f32>,
    /// 阶段文案，如「正在读取文件表（极速）」
    pub phase: String,
}

/// 盘符信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub letter: String,
    pub mount_point: String,
    pub total: u64,
    pub free: u64,
    pub used: u64,
}

/// 内部：先把整棵目录的大小聚合到一张「路径 -> (自身大小, 是否目录)」的表里。
struct SizeIndex {
    /// 每个目录的递归总大小
    dir_sizes: HashMap<PathBuf, u64>,
    /// 每个目录的直接子项（文件或子目录）
    children: HashMap<PathBuf, Vec<PathBuf>>,
    /// 文件大小
    file_sizes: HashMap<PathBuf, u64>,
    /// 每个目录的内容构成（按扩展名分组的累计字节数），用于未知目录的启发式推断
    dir_profiles: HashMap<PathBuf, [u64; knowledge::EXT_GROUP_COUNT]>,
}

/// 枚举系统盘符。
pub fn list_drives() -> Vec<DriveInfo> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    let mut out = Vec::new();
    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().to_string();
        let letter = mount.chars().next().map(|c| c.to_string()).unwrap_or_default();
        let total = disk.total_space();
        let free = disk.available_space();
        out.push(DriveInfo {
            letter,
            mount_point: mount,
            total,
            free,
            used: total.saturating_sub(free),
        });
    }
    // 去重（同一盘符可能出现多次），按盘符排序
    out.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    out.dedup_by(|a, b| a.mount_point == b.mount_point);
    out
}

/// 并行遍历 root，构建大小索引，同时通过 app 上报进度。
fn build_index(root: &Path, app: Option<&AppHandle>) -> (SizeIndex, u64) {
    let files_scanned = Arc::new(AtomicU64::new(0));
    let bytes_scanned = Arc::new(AtomicU64::new(0));
    let mut file_sizes: HashMap<PathBuf, u64> = HashMap::new();

    let counter = files_scanned.clone();
    let byte_counter = bytes_scanned.clone();

    let mut last_emit = Instant::now();
    let mut last_path = String::new();

    for entry in WalkDir::new(root)
        .skip_hidden(false)
        .parallelism(jwalk::Parallelism::RayonDefaultPool {
            busy_timeout: Duration::from_secs(5),
        })
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        if entry.file_type().is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            file_sizes.insert(path.clone(), size);
            counter.fetch_add(1, Ordering::Relaxed);
            byte_counter.fetch_add(size, Ordering::Relaxed);
            last_path = path.to_string_lossy().to_string();
        }

        // 每 500ms 上报一次进度，避免 emit 过于频繁
        if let Some(app) = app {
            if last_emit.elapsed() >= Duration::from_millis(500) {
                let _ = app.emit(
                    "scan-progress",
                    ScanProgress {
                        files_scanned: counter.load(Ordering::Relaxed),
                        bytes_scanned: byte_counter.load(Ordering::Relaxed),
                        current_path: last_path.clone(),
                        done: false,
                        percent: None,
                        phase: "正在遍历目录".to_string(),
                    },
                );
                last_emit = Instant::now();
            }
        }
    }

    // 由文件大小自底向上聚合出目录大小、父子关系与内容构成
    let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();
    let mut children: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut dir_profiles: HashMap<PathBuf, [u64; knowledge::EXT_GROUP_COUNT]> = HashMap::new();
    let root_buf = root.to_path_buf();

    for (file, size) in &file_sizes {
        let group = knowledge::ext_group(file);
        let mut cur = file.parent().map(|p| p.to_path_buf());
        // 记录直接父子关系
        if let Some(parent) = file.parent() {
            children
                .entry(parent.to_path_buf())
                .or_default()
                .push(file.clone());
        }
        // 向上累加到 root 为止
        while let Some(dir) = cur {
            *dir_sizes.entry(dir.clone()).or_insert(0) += size;
            dir_profiles.entry(dir.clone()).or_insert([0; knowledge::EXT_GROUP_COUNT])[group] += size;
            if dir == root_buf {
                break;
            }
            // 记录目录的父子关系（子目录 -> 父目录）
            if let Some(parent) = dir.parent() {
                let list = children.entry(parent.to_path_buf()).or_default();
                if !list.contains(&dir) {
                    list.push(dir.clone());
                }
                cur = Some(parent.to_path_buf());
            } else {
                break;
            }
        }
    }

    let total = bytes_scanned.load(Ordering::Relaxed);
    (
        SizeIndex {
            dir_sizes,
            children,
            file_sizes,
            dir_profiles,
        },
        total,
    )
}

/// 从索引里构建以 `dir` 为根的树，限制深度与每层数量。
fn build_tree(index: &SizeIndex, dir: &Path, depth: usize) -> TreeNode {
    let path_str = dir.to_string_lossy().to_string();
    let size = index.dir_sizes.get(dir).copied().unwrap_or(0);
    let mut hit = knowledge::classify(&path_str);
    // 名字认不出的目录，按内容构成推断（不只看目录名）
    if hit.category == knowledge::Category::Other {
        if let Some(profile) = index.dir_profiles.get(dir) {
            if let Some(h) = knowledge::profile_classify(profile) {
                hit = h;
            }
        }
    }
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.clone());

    let empty = Vec::new();
    let child_paths = index.children.get(dir).unwrap_or(&empty);
    let has_children = !child_paths.is_empty();

    // 达到最大深度：不再展开，只标记是否还有下级
    if depth >= MAX_DEPTH {
        return TreeNode {
            name,
            path: path_str,
            size,
            is_dir: true,
            has_children,
            category: hit.category,
            friendly_name: hit.friendly_name,
            description: hit.description,
            safety: hit.safety,
            children: Vec::new(),
        };
    }

    // 收集子节点（目录递归，文件直接建叶子）
    let mut kids: Vec<TreeNode> = Vec::new();
    for child in child_paths {
        if index.dir_sizes.contains_key(child) {
            kids.push(build_tree(index, child, depth + 1));
        } else if let Some(fsize) = index.file_sizes.get(child) {
            let cpath = child.to_string_lossy().to_string();
            let chit = knowledge::classify(&cpath);
            let cname = child
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            kids.push(TreeNode {
                name: cname,
                path: cpath,
                size: *fsize,
                is_dir: false,
                has_children: false,
                category: chit.category,
                friendly_name: chit.friendly_name,
                description: chit.description,
                safety: chit.safety,
                children: Vec::new(),
            });
        }
    }

    kids.sort_by(|a, b| b.size.cmp(&a.size));

    // 剪枝：超过 TOP_N 的聚合成「其他」
    if kids.len() > TOP_N_PER_LEVEL {
        let overflow: Vec<TreeNode> = kids.split_off(TOP_N_PER_LEVEL);
        let other_size: u64 = overflow.iter().map(|k| k.size).sum();
        let count = overflow.len();
        if other_size > 0 {
            kids.push(TreeNode {
                name: format!("其他 ({} 项)", count),
                path: format!("{}::__others__", path_str),
                size: other_size,
                is_dir: false,
                has_children: false,
                category: Category::Other,
                friendly_name: format!("其他 {} 个较小项目", count),
                description: "这些项目单个占用较小，已合并显示。".to_string(),
                safety: Safety::Keep,
                children: Vec::new(),
            });
        }
    }

    TreeNode {
        name,
        path: path_str,
        size,
        is_dir: true,
        has_children,
        category: hit.category,
        friendly_name: hit.friendly_name,
        description: hit.description,
        safety: hit.safety,
        children: kids,
    }
}

/// 盘符根目录（如 "C:\\"）且文件系统为 NTFS 时，才能走 MFT 直读。
fn is_ntfs_drive_root(root: &str) -> bool {
    let bytes = root.as_bytes();
    let is_root = matches!(bytes.len(), 2 | 3)
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes.len() == 2 || bytes[2] == b'\\');
    if !is_root {
        return false;
    }
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    disks.list().iter().any(|d| {
        d.mount_point()
            .to_string_lossy()
            .to_uppercase()
            .starts_with(&root[..2].to_uppercase())
            && d.file_system().to_string_lossy().eq_ignore_ascii_case("NTFS")
    })
}

/// MFT 引擎的进度转发：直接携带后端算出的精确百分比与阶段文案。
fn emit_mft_progress(app: Option<&AppHandle>, files: u64, bytes: u64, percent: f32, phase: &str) {
    if let Some(app) = app {
        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                files_scanned: files,
                bytes_scanned: bytes,
                current_path: String::new(),
                done: false,
                percent: Some(percent),
                phase: format!("{}（极速）", phase),
            },
        );
    }
}

/// 扫描一个根目录，返回剪枝后的树。发送进度事件（需要 AppHandle）。
///
/// 三级引擎调度：后台服务 MFT（秒级）→ 本进程 MFT（已提权时）→ jwalk 遍历（永不失败的兜底）。
pub fn scan(root: &str, app: Option<&AppHandle>) -> Result<TreeNode, String> {
    let root_path = PathBuf::from(root);
    if !root_path.exists() {
        return Err(format!("路径不存在：{}", root));
    }

    if is_ntfs_drive_root(root) {
        // 优先让后台服务扫（主程序无需管理员权限）
        let by_service = crate::svc_client::scan_via_service(root, |f, b, p, ph| {
            emit_mft_progress(app, f, b, p, ph)
        });
        let tree = match by_service {
            Ok(tree) => Some(tree),
            // 服务不在：若本进程已提权（开发模式/用户以管理员运行）则直读 MFT
            Err(_) => {
                crate::mft_scan::scan_mft(root, |f, b, p, ph| emit_mft_progress(app, f, b, p, ph))
                    .ok()
            }
        };
        // 空树视为引擎异常（如 MFT 解析边界情况）：宁可落回慢速遍历，也不把空结果当成果
        let tree = tree.filter(|t| t.size > 0 && !t.children.is_empty());
        if let Some(tree) = tree {
            if let Some(app) = app {
                let _ = app.emit(
                    "scan-progress",
                    ScanProgress {
                        files_scanned: 0,
                        bytes_scanned: tree.size,
                        current_path: String::new(),
                        done: true,
                        percent: Some(100.0),
                        phase: String::new(),
                    },
                );
            }
            return Ok(tree);
        }
        // 两条快路径都不可用，落回 jwalk 慢速遍历
    }

    let (index, _total) = build_index(&root_path, app);
    let tree = build_tree(&index, &root_path, 0);

    if let Some(app) = app {
        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                files_scanned: index.file_sizes.len() as u64,
                bytes_scanned: tree.size,
                current_path: String::new(),
                done: true,
                percent: None,
                phase: String::new(),
            },
        );
    }
    Ok(tree)
}
