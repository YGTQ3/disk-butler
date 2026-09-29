//! MFT 直读扫描引擎（WizTree 同款原理）：一次性载入 NTFS 主文件表，
//! rayon 并行解析记录 + 记录号稠密数组聚合（无哈希、无逐记录堆分配），
//! 名字只在剪枝后的少量节点上按需读取，全盘统计秒级完成。
//! 需要管理员权限（打开 `\\.\C:` 裸卷句柄是系统硬性要求）。
//!
//! 本文件是主程序 src-tauri/src/mft_scan.rs 的独立采集器移植版：
//! 核心解析逻辑逐字一致，仅剥离了知识库分类（采集器只需 名字/大小/是否目录）。

use ntfs_reader::{
    Mft, NtfsAttributeType, NtfsFile, NtfsFileNamespace, Volume, FIRST_NORMAL_RECORD, ROOT_RECORD,
};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 每层保留的子项数量上限，其余聚合为「其他」。
const TOP_N_PER_LEVEL: usize = 30;
/// 返回树的最大深度（从扫描根算起）。
const MAX_DEPTH: usize = 4;
/// 上溯父链的最大深度（防御 MFT 数据损坏导致的环）。
const MAX_PARENT_DEPTH: usize = 1024;
/// 并行解析的分块大小（记录条数）。
const PAR_CHUNK: usize = 8192;

/// 无父记录的哨兵值。
const NO_PARENT: u64 = u64::MAX;
/// 非目录记录在稠密目录索引中的哨兵值。
const NOT_DIR: u32 = u32::MAX;

/// 剪枝树节点（采集器精简版：无分类字段）。
/// 部分字段（name/has_children）由引擎构建但采集器报告未消费，保留以与主程序引擎结构一致。
#[allow(dead_code)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    /// 该目录是否还有更深内容未展开。
    pub has_children: bool,
    pub children: Vec<TreeNode>,
}

/// 全部 MFT 记录提炼出的信息（按记录号索引的稠密数组）。
struct Records {
    /// 记录号 -> 父目录记录号（NO_PARENT = 无效/未使用记录）
    parent: Vec<u64>,
    /// 记录号 -> 文件自身大小（目录为 0）
    size: Vec<u64>,
    /// 记录号 -> 是否目录
    is_dir: Vec<bool>,
}

/// 校验并规范化根路径："C:\" / "c:" -> ('C', "C:\")。
fn parse_root(root: &str) -> Result<(char, String), String> {
    let bytes = root.as_bytes();
    if bytes.len() < 2 || bytes.len() > 3 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        return Err(format!("MFT 扫描只支持盘符根目录：{}", root));
    }
    if bytes.len() == 3 && bytes[2] != b'\\' {
        return Err(format!("MFT 扫描只支持盘符根目录：{}", root));
    }
    let letter = (bytes[0] as char).to_ascii_uppercase();
    Ok((letter, format!("{}:\\", letter)))
}

struct RecInfo {
    parent: u64,
    is_dir: bool,
    size: u64,
}

/// 单条记录的单遍属性解析：一次遍历同时拿 $FILE_NAME（父目录/命名空间）与 $DATA 大小。
/// attributes() 已覆盖属性列表指向的扩展记录，名字或 $DATA 被挪进扩展记录时无需另走慢路径。
fn parse_record(f: &NtfsFile) -> Option<RecInfo> {
    let mut parent: Option<u64> = None;
    let mut best_rank = 0u8; // 0=未找到 1=Posix 2=Win32

    for att in f.attributes() {
        match att.attribute_type() {
            Some(NtfsAttributeType::FileName) => {
                if best_rank >= 2 {
                    continue;
                }
                let Some(n) = att.file_name() else {
                    continue;
                };
                let rank = match n.namespace() {
                    Some(NtfsFileNamespace::Dos) => continue, // DOS 短名是硬链接别名，跳过防止重复计数
                    Some(NtfsFileNamespace::Win32 | NtfsFileNamespace::Win32AndDos) => 2,
                    _ => 1,
                };
                if rank > best_rank {
                    best_rank = rank;
                    parent = Some(n.parent_number());
                }
            }
            _ => {}
        }
    }

    let parent = match parent {
        Some(p) => p,
        // 没有非 DOS 名字（罕见）：退回库的 best_name
        None => f.best_name()?.parent_number(),
    };

    // 默认（未命名）$DATA 流的大小：改走 data_streams()，而不是手动扫 $DATA 属性判断
    // att.name().is_some()——name() 对「确实无名」和「声明了名字但读不出（记录损坏）」
    // 都返回 None，两者无法用 name() 本身区分，会把损坏的命名流（ADS）错当成默认流计入大小。
    // data_streams() 内部能看到属性头的原始 name_length，正确排除后一种情况。
    let size = f
        .data_streams()
        .find(|s| s.name.is_none())
        .map(|s| s.size)
        .unwrap_or(0);

    Some(RecInfo { parent, is_dir: f.is_directory(), size })
}

/// 第一遍：rayon 并行解析全部 MFT 记录。
/// progress(已发现文件数, 已统计字节数, 精确百分比)
fn collect_records<F: FnMut(u64, u64, f32)>(mft: &Mft, progress: &mut F) -> Records {
    let cap = mft.record_count() as usize;
    let mut parent = vec![NO_PARENT; cap];
    let mut size = vec![0u64; cap];
    let mut is_dir = vec![false; cap];

    let files_cnt = AtomicU64::new(0);
    let bytes_cnt = AtomicU64::new(0);
    let recs_done = AtomicU64::new(0);
    let finished = AtomicBool::new(false);

    std::thread::scope(|s| {
        let worker = s.spawn(|| {
            parent
                .par_chunks_mut(PAR_CHUNK)
                .zip(size.par_chunks_mut(PAR_CHUNK))
                .zip(is_dir.par_chunks_mut(PAR_CHUNK))
                .enumerate()
                .for_each(|(chunk_no, ((pc, sc), dc))| {
                    let base = chunk_no * PAR_CHUNK;
                    let mut local_files = 0u64;
                    let mut local_bytes = 0u64;
                    for i in 0..pc.len() {
                        let number = (base + i) as u64;
                        // 系统元记录（$MFT/$Bitmap 等，<24）不进树，与目录遍历行为一致
                        if number < FIRST_NORMAL_RECORD || !mft.is_allocated(number) {
                            continue;
                        }
                        let Some(f) = mft.record(number) else {
                            continue;
                        };
                        // 扩展记录归属基记录，跳过避免重复计数
                        if !f.is_used() || f.is_extension() {
                            continue;
                        }
                        let Some(info) = parse_record(&f) else {
                            continue;
                        };
                        pc[i] = info.parent;
                        sc[i] = info.size;
                        dc[i] = info.is_dir;
                        if !info.is_dir {
                            local_files += 1;
                            local_bytes += info.size;
                        }
                    }
                    files_cnt.fetch_add(local_files, Ordering::Relaxed);
                    bytes_cnt.fetch_add(local_bytes, Ordering::Relaxed);
                    recs_done.fetch_add(pc.len() as u64, Ordering::Relaxed);
                });
            finished.store(true, Ordering::SeqCst);
        });

        // 主线程轮询原子计数器上报进度（rayon 工作线程内不便回调 FnMut）
        while !finished.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            progress(
                files_cnt.load(Ordering::Relaxed),
                bytes_cnt.load(Ordering::Relaxed),
                (recs_done.load(Ordering::Relaxed) as f32 / cap.max(1) as f32) * 100.0,
            );
        }
        let _ = worker.join();
    });
    progress(
        files_cnt.load(Ordering::Relaxed),
        bytes_cnt.load(Ordering::Relaxed),
        100.0,
    );

    let mut rec = Records { parent, size, is_dir };
    // 根目录（记录号 5）强制兑底：files()/记录范围都不含它，且部分卷上根的 $FILE_NAME
    // 无法常规解析；根缺失会导致整棵树为空
    let r = ROOT_RECORD as usize;
    if r < rec.parent.len() {
        rec.parent[r] = ROOT_RECORD;
        rec.is_dir[r] = true;
    }
    rec
}

/// 第二遍产物：稠密目录索引（无哈希查找）。
struct DirIndex {
    /// 记录号 -> 稠密目录序号（NOT_DIR = 非目录）
    dir_idx: Vec<u32>,
    /// 目录序号 -> 递归总大小
    total_size: Vec<u64>,
    /// 目录序号 -> 直接子项记录号列表（目录 + 非空文件）
    children: Vec<Vec<u32>>,
}

fn aggregate(rec: &Records) -> DirIndex {
    let cap = rec.parent.len();

    // 目录稠密编号
    let mut dir_idx = vec![NOT_DIR; cap];
    let mut ndirs = 0u32;
    for i in 0..cap {
        if rec.is_dir[i] && rec.parent[i] != NO_PARENT {
            dir_idx[i] = ndirs;
            ndirs += 1;
        }
    }
    let n = ndirs as usize;
    let mut direct_size = vec![0u64; n];
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); n];

    // 一遍挂父子 + 累计各目录的「直接」大小
    for i in 0..cap {
        let parent = rec.parent[i];
        if parent == NO_PARENT || i as u64 == ROOT_RECORD {
            continue;
        }
        let pd = if (parent as usize) < cap { dir_idx[parent as usize] } else { NOT_DIR };
        if pd == NOT_DIR {
            continue; // 父记录不是有效目录（孤儿），无法挂树
        }
        let pdi = pd as usize;
        if rec.is_dir[i] {
            children[pdi].push(i as u32);
        } else if rec.size[i] > 0 {
            children[pdi].push(i as u32);
            direct_size[pdi] += rec.size[i];
        }
    }

    // 每个目录把「直接量」沿父链上溯（O(目录数 × 深度)，远小于按文件上溯）
    let mut total_size = direct_size.clone();
    for i in 0..cap {
        let di = dir_idx[i];
        if di == NOT_DIR || i as u64 == ROOT_RECORD {
            continue;
        }
        let ds = direct_size[di as usize];
        if ds == 0 {
            continue;
        }
        let mut cur = rec.parent[i];
        for _ in 0..MAX_PARENT_DEPTH {
            let c = cur as usize;
            if c >= cap {
                break;
            }
            let cdi = dir_idx[c];
            if cdi == NOT_DIR {
                break;
            }
            total_size[cdi as usize] += ds;
            if cur == ROOT_RECORD {
                break;
            }
            let next = rec.parent[c];
            if next == cur {
                break; // 防环
            }
            cur = next;
        }
    }

    DirIndex { dir_idx, total_size, children }
}

/// 第三遍：从根出发建剪枝树。每层先按大小排序取 Top-N，只为保留下来的节点取名字、拼路径。
fn build_tree(
    rec: &Records,
    index: &DirIndex,
    resolve_name: &dyn Fn(u64) -> String,
    number: u64,
    name: String,
    path: &str,
    depth: usize,
) -> TreeNode {
    let di = index.dir_idx[number as usize] as usize;
    let size = index.total_size[di];

    let child_recs = &index.children[di];
    let has_children = !child_recs.is_empty();

    // 达到最大深度：不再展开，只标记是否还有下级
    if depth >= MAX_DEPTH {
        return TreeNode {
            name,
            path: path.to_string(),
            size,
            is_dir: true,
            has_children,
            children: Vec::new(),
        };
    }

    // 先按大小剪枝，再为保留项构建节点（避免为海量被剪掉的子项取名）
    let mut cand: Vec<(u32, u64)> = child_recs
        .iter()
        .map(|&c| {
            let cdi = index.dir_idx[c as usize];
            let csize = if cdi != NOT_DIR {
                index.total_size[cdi as usize]
            } else {
                rec.size[c as usize]
            };
            (c, csize)
        })
        .collect();
    cand.sort_by(|a, b| b.1.cmp(&a.1));

    let overflow_count = cand.len().saturating_sub(TOP_N_PER_LEVEL);
    let overflow_size: u64 = cand.iter().skip(TOP_N_PER_LEVEL).map(|&(_, s)| s).sum();
    cand.truncate(TOP_N_PER_LEVEL);

    let mut kids: Vec<TreeNode> = Vec::with_capacity(cand.len() + 1);
    for (c, csize) in cand {
        let crec = c as u64;
        let cname = resolve_name(crec);
        let child_path = if path.ends_with('\\') {
            format!("{}{}", path, cname)
        } else {
            format!("{}\\{}", path, cname)
        };
        if rec.is_dir[c as usize] {
            kids.push(build_tree(rec, index, resolve_name, crec, cname, &child_path, depth + 1));
        } else {
            kids.push(TreeNode {
                name: cname,
                path: child_path,
                size: csize,
                is_dir: false,
                has_children: false,
                children: Vec::new(),
            });
        }
    }

    if overflow_count > 0 && overflow_size > 0 {
        kids.push(TreeNode {
            name: format!("其他 ({} 项)", overflow_count),
            path: format!("{}::__others__", path),
            size: overflow_size,
            is_dir: false,
            has_children: false,
            children: Vec::new(),
        });
    }

    TreeNode {
        name,
        path: path.to_string(),
        size,
        is_dir: true,
        has_children,
        children: kids,
    }
}

/// MFT 直读扫描入口。root 形如 "C:\\"。
/// progress(已发现文件数, 已统计字节数, 精确百分比 0~100, 阶段文案)
pub fn scan_mft<F: FnMut(u64, u64, f32, &str)>(
    root: &str,
    mut progress: F,
) -> Result<TreeNode, String> {
    let (letter, root_norm) = parse_root(root)?;
    // 载入整张 MFT（可达 GB 级）
    progress(0, 0, 0.0, "正在载入文件表");
    let volume = Volume::new(format!("\\\\.\\{}:", letter))
        .map_err(|e| format!("打开卷失败（需要管理员权限）：{:?}", e))?;
    let mft = Mft::new(volume).map_err(|e| format!("读取 MFT 失败：{:?}", e))?;

    let mut last = (0u64, 0u64);
    let rec = collect_records(&mft, &mut |f, b, p| {
        last = (f, b);
        progress(f, b, p, "正在统计文件");
    });

    progress(last.0, last.1, 100.0, "正在汇总目录");
    let index = aggregate(&rec);

    // 名字仅为最终保留的少量节点解析（best_name 兼容属性列表等边界）
    let resolve = |n: u64| -> String {
        mft.record(n)
            .and_then(|f| f.best_name())
            .map(|nm| nm.to_string())
            .unwrap_or_else(|| format!("#{}", n))
    };
    Ok(build_tree(&rec, &index, &resolve, ROOT_RECORD, root_norm.clone(), &root_norm, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小目录结构：根(5) └ Windows(6) └ a.exe(7, 100B)，另有根下散文件 b.txt(8, 50B)
    fn sample_records() -> Records {
        let cap = 9;
        let mut rec = Records {
            parent: vec![NO_PARENT; cap],
            size: vec![0; cap],
            is_dir: vec![false; cap],
        };
        rec.parent[5] = 5;
        rec.is_dir[5] = true; // 根（collect_records 保证的兑底状态）
        rec.parent[6] = 5;
        rec.is_dir[6] = true;
        rec.parent[7] = 6;
        rec.size[7] = 100;
        rec.parent[8] = 5;
        rec.size[8] = 50;
        rec
    }

    fn test_resolver(n: u64) -> String {
        match n {
            6 => "Windows".into(),
            7 => "a.exe".into(),
            8 => "b.txt".into(),
            _ => format!("#{}", n),
        }
    }

    #[test]
    fn aggregate_rolls_file_sizes_up_to_root() {
        let rec = sample_records();
        let index = aggregate(&rec);
        let root_di = index.dir_idx[5] as usize;
        let win_di = index.dir_idx[6] as usize;
        assert_eq!(index.total_size[root_di], 150, "根应汇总全部文件大小");
        assert_eq!(index.total_size[win_di], 100);
        assert_eq!(index.children[root_di].len(), 2);
    }

    #[test]
    fn build_tree_produces_non_empty_root() {
        let rec = sample_records();
        let index = aggregate(&rec);
        let tree = build_tree(&rec, &index, &test_resolver, ROOT_RECORD, "C:\\".into(), "C:\\", 0);
        assert_eq!(tree.size, 150);
        assert_eq!(tree.children.len(), 2);
        let win = tree.children.iter().find(|c| c.name == "Windows").unwrap();
        assert_eq!(win.size, 100);
        assert_eq!(win.path, "C:\\Windows");
        assert_eq!(win.children[0].name, "a.exe");
    }

    #[test]
    fn parse_root_accepts_drive_roots_only() {
        assert!(parse_root("C:\\").is_ok());
        assert!(parse_root("d:").is_ok());
        assert!(parse_root("C:\\Windows").is_err());
        assert!(parse_root("\\\\server\\share").is_err());
    }
}
