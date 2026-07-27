//! MFT 直读扫描引擎（WizTree 同款原理）：一次性载入 NTFS 主文件表，
//! rayon 并行解析记录 + 记录号稠密数组聚合（无哈希、无逐记录堆分配），
//! 名字只在剪枝后的少量节点上按需读取，全盘统计秒级完成。
//! 需要管理员权限（打开 `\\.\C:` 裸卷句柄是系统硬性要求）。

use crate::knowledge::{self, Category, Safety};
use crate::scan::{TreeNode, MAX_DEPTH, TOP_N_PER_LEVEL};
use ntfs_reader::api::{NtfsAttributeType, FIRST_NORMAL_RECORD, ROOT_RECORD};
use ntfs_reader::attribute::DataRun;
use ntfs_reader::file::NtfsFile;
use ntfs_reader::mft::Mft;
use ntfs_reader::volume::Volume;
use rayon::prelude::*;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 上溯父链的最大深度（防御 MFT 数据损坏导致的环）。
const MAX_PARENT_DEPTH: usize = 1024;
/// 并行解析的分块大小（记录条数）。
const PAR_CHUNK: usize = 8192;

/// 无父记录的哨兵值。
const NO_PARENT: u64 = u64::MAX;
/// 非目录记录在稠密目录索引中的哨兵值。
const NOT_DIR: u32 = u32::MAX;

/// $FILE_NAME 属性值内的字段偏移（NTFS 磁盘布局，见 NtfsFileNameHeader）。
const FN_PARENT: usize = 0;
const FN_REAL_SIZE: usize = 48;
const FN_NAME_LEN: usize = 64;
const FN_NAMESPACE: usize = 65;
const FN_NAME: usize = 66;
const NS_DOS: u8 = 2;
const NS_WIN32: u8 = 1;
const NS_WIN32_DOS: u8 = 3;

/// 全部 MFT 记录提炼出的信息（按记录号索引的稠密数组）。
struct Records {
    /// 记录号 -> 父目录记录号（NO_PARENT = 无效/未使用记录）
    parent: Vec<u64>,
    /// 记录号 -> 文件自身大小（目录为 0）
    size: Vec<u64>,
    /// 记录号 -> 是否目录
    is_dir: Vec<bool>,
    /// 记录号 -> 文件内容组（knowledge::ext_group 语义）
    ext: Vec<u8>,
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

/// 从 UTF-16LE 原始名字节流中提取扩展名并归组（零堆分配）。
fn ext_group_utf16(raw: &[u8]) -> usize {
    let n = raw.len() / 2;
    let ch = |i: usize| u16::from_le_bytes([raw[2 * i], raw[2 * i + 1]]);
    let mut dot = None;
    for i in (0..n).rev() {
        if ch(i) == '.' as u16 {
            dot = Some(i);
            break;
        }
    }
    let Some(d) = dot else {
        return knowledge::EXT_GROUP_OTHER;
    };
    let len = n - d - 1;
    if len == 0 || len > 12 {
        return knowledge::EXT_GROUP_OTHER;
    }
    let mut buf = [0u8; 12];
    for k in 0..len {
        let c = ch(d + 1 + k);
        if c >= 128 {
            return knowledge::EXT_GROUP_OTHER;
        }
        buf[k] = (c as u8).to_ascii_lowercase();
    }
    knowledge::ext_group_of(std::str::from_utf8(&buf[..len]).unwrap_or(""))
}

/// 把一段磁盘连续区（data run）读进 out，8MB 大块顺序读，尾部不足一个对齐块时补齐读再裁剪。
fn read_run<F: FnMut(u64)>(
    raw: &mut std::fs::File,
    lcn: u64,
    want: u64,
    out: &mut Vec<u8>,
    on_read: &mut F,
) -> Result<(), String> {
    const CHUNK: u64 = 8 << 20;
    const ALIGN: u64 = 4096;
    raw.seek(SeekFrom::Start(lcn))
        .map_err(|e| format!("定位 MFT 数据区失败：{}", e))?;
    let mut left = want;
    while left > 0 {
        let n = left.min(CHUNK);
        let aligned = n & !(ALIGN - 1);
        if aligned > 0 {
            let start = out.len();
            out.resize(start + aligned as usize, 0);
            raw.read_exact(&mut out[start..])
                .map_err(|e| format!("读取 MFT 数据失败：{}", e))?;
            left -= aligned;
            on_read(aligned);
        } else {
            // 裸卷读取要求扇区对齐：尾部按对齐块读满再取所需部分
            let mut tmp = [0u8; ALIGN as usize];
            raw.read_exact(&mut tmp)
                .map_err(|e| format!("读取 MFT 数据尾部失败：{}", e))?;
            out.extend_from_slice(&tmp[..n as usize]);
            on_read(n);
            left = 0;
        }
    }
    Ok(())
}

/// NTFS 记录的 USA 修正（update sequence array fixup）：
/// 每扇区末 2 字节被 USN 占位，需用 USA 中的原值恢复；USN 不匹配说明记录撞写，作废处理。
fn fixup_record(data: &mut [u8]) {
    const SECTOR: usize = 512;
    if data.len() < 48 || &data[0..4] != b"FILE" {
        return;
    }
    let usa_off = u16::from_le_bytes([data[4], data[5]]) as usize;
    let usa_len = u16::from_le_bytes([data[6], data[7]]) as usize;
    if usa_len < 2 || usa_off + usa_len * 2 > data.len() {
        data[0] = 0; // 破坏签名使 is_valid 判废
        return;
    }
    let usn = [data[usa_off], data[usa_off + 1]];
    for i in 1..usa_len {
        let end = i * SECTOR;
        if end > data.len() {
            break;
        }
        if data[end - 2] != usn[0] || data[end - 1] != usn[1] {
            data[0] = 0;
            return;
        }
        let fix = usa_off + i * 2;
        data[end - 2] = data[fix];
        data[end - 1] = data[fix + 1];
    }
}

/// 快速载入 MFT：绕开 ntfs-reader 内部 4KB 对齐小读（Mft::new 的主要耗时，
/// GB 级 MFT 需八十万次卷读取），改用 8MB 大块顺序读 + 并行 fixup，并上报载入进度。
/// 任何边界情况（$DATA 进属性列表等）回退到库的慢速但完备的 Mft::new。
fn load_mft_fast<F: FnMut(u64, u64)>(volume: Volume, on_read: &mut F) -> Result<Mft, String> {
    use ntfs_reader::aligned_reader::open_volume;

    let fast = (|| -> Option<Mft> {
        let mut reader = open_volume(&volume.path).ok()?;
        let rec0 =
            Mft::get_record_fs(&mut reader, volume.file_record_size, volume.mft_position).ok()?;
        let f0 = NtfsFile::new(0, &rec0);
        // $Bitmap 很小（每记录 1 bit），用库的常规路径读即可
        let bitmap =
            Mft::read_data_fs(&volume, &mut reader, &rec0, NtfsAttributeType::Bitmap).ok()??;
        let data_attr = f0.get_attribute(NtfsAttributeType::Data)?;
        let (size, runs) = data_attr.get_nonresident_data_runs(&volume).ok()?;

        let mut raw = std::fs::File::open(&volume.path).ok()?;
        let mut data = Vec::new();
        data.try_reserve_exact(size as usize).ok()?;
        let mut copied = 0u64;
        let mut loaded = 0u64;
        for run in &runs {
            if copied >= size {
                break;
            }
            match run {
                DataRun::Data { lcn, length } => {
                    let want = (*length).min(size - copied);
                    read_run(&mut raw, *lcn, want, &mut data, &mut |n| {
                        loaded += n;
                        on_read(loaded, size);
                    })
                    .ok()?;
                    copied += want;
                }
                DataRun::Sparse { length } => {
                    let want = (*length).min(size - copied);
                    data.resize(data.len() + want as usize, 0);
                    copied += want;
                }
            }
        }

        let rs = volume.file_record_size as usize;
        let max_record = data.len() as u64 / volume.file_record_size;
        data.par_chunks_mut(rs).for_each(fixup_record);
        Some(Mft {
            volume: volume.clone(),
            data,
            bitmap,
            max_record,
        })
    })();

    match fast {
        Some(mft) => Ok(mft),
        None => Mft::new(volume).map_err(|e| format!("读取 MFT 失败：{:?}", e)),
    }
}

struct RecInfo {
    parent: u64,
    is_dir: bool,
    size: u64,
    ext: u8,
}

/// 单条记录的单遍属性解析：一次遍历同时拿 $FILE_NAME（父目录/命名空间/扩展名）与 $DATA 大小。
fn parse_record(mft: &Mft, f: &NtfsFile) -> Option<RecInfo> {
    let mut parent: Option<u64> = None;
    let mut best_rank = 0u8; // 0=未找到 1=Posix 2=Win32
    let mut ext = knowledge::EXT_GROUP_OTHER as u8;
    let mut size = 0u64;
    let mut size_found = false;
    let mut name_real_size = 0u64;
    let mut has_attr_list = false;

    f.attributes(|att| {
        let ty = att.header.type_id;
        if ty == NtfsAttributeType::FileName as u32 {
            if best_rank >= 2 {
                return;
            }
            let Some(h) = att.resident_header() else {
                return;
            };
            let off = h.value_offset as usize;
            let len = h.value_length as usize;
            let data = att.data();
            if len < FN_NAME || off + len > data.len() {
                return;
            }
            let v = &data[off..off + len];
            let ns = v[FN_NAMESPACE];
            if ns == NS_DOS {
                return; // DOS 短名是硬链接别名，跳过防止重复计数
            }
            let name_len = v[FN_NAME_LEN] as usize;
            if FN_NAME + 2 * name_len > len {
                return;
            }
            let rank = if ns == NS_WIN32 || ns == NS_WIN32_DOS { 2 } else { 1 };
            if rank > best_rank {
                best_rank = rank;
                parent = Some(
                    u64::from_le_bytes(v[FN_PARENT..FN_PARENT + 8].try_into().unwrap())
                        & 0x0000_FFFF_FFFF_FFFF,
                );
                name_real_size =
                    u64::from_le_bytes(v[FN_REAL_SIZE..FN_REAL_SIZE + 8].try_into().unwrap());
                ext = ext_group_utf16(&v[FN_NAME..FN_NAME + 2 * name_len]) as u8;
            }
        } else if ty == NtfsAttributeType::Data as u32 {
            // 只取第一个未命名 $DATA 流：命名流（ADS）不计入，避免大小虚高
            if size_found || att.header.name_length != 0 {
                return;
            }
            size_found = true;
            size = if att.header.is_non_resident == 0 {
                att.resident_header().map(|h| h.value_length as u64).unwrap_or(0)
            } else {
                att.nonresident_header().map(|h| h.data_size).unwrap_or(0)
            };
        } else if ty == NtfsAttributeType::AttributeList as u32 {
            has_attr_list = true;
        }
    });

    let parent = match parent {
        Some(p) => p,
        None => {
            // 名字被挪进属性列表扩展记录（罕见）：走库的慢路径解析
            let n = f.get_best_file_name(mft)?;
            let name = n.to_string();
            ext = knowledge::ext_group(std::path::Path::new(&name)) as u8;
            name_real_size = n.header.real_size;
            n.parent()
        }
    };

    let is_dir = f.is_directory();
    // $DATA 也可能整体在扩展记录里：用 $FILE_NAME 的 real_size 兜底（略滞后但远好于记 0）
    if !is_dir && !size_found && has_attr_list {
        size = name_real_size;
    }
    Some(RecInfo { parent, is_dir, size, ext })
}

/// 第一遍：rayon 并行解析全部 MFT 记录。
/// progress(已发现文件数, 已统计字节数, 精确百分比)
fn collect_records<F: FnMut(u64, u64, f32)>(mft: &Mft, progress: &mut F) -> Records {
    let cap = mft.max_record as usize;
    let mut parent = vec![NO_PARENT; cap];
    let mut size = vec![0u64; cap];
    let mut is_dir = vec![false; cap];
    let mut ext = vec![knowledge::EXT_GROUP_OTHER as u8; cap];

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
                .zip(ext.par_chunks_mut(PAR_CHUNK))
                .enumerate()
                .for_each(|(chunk_no, (((pc, sc), dc), ec))| {
                    let base = chunk_no * PAR_CHUNK;
                    let mut local_files = 0u64;
                    let mut local_bytes = 0u64;
                    for i in 0..pc.len() {
                        let number = (base + i) as u64;
                        // 系统元记录（$MFT/$Bitmap 等，<24）不进树，与目录遍历行为一致
                        if number < FIRST_NORMAL_RECORD || !mft.record_exists(number) {
                            continue;
                        }
                        let Some(f) = mft.get_record(number) else {
                            continue;
                        };
                        // 扩展记录归属基记录，跳过避免重复计数
                        if !f.is_used() || f.header.base_reference != 0 {
                            continue;
                        }
                        let Some(info) = parse_record(mft, &f) else {
                            continue;
                        };
                        pc[i] = info.parent;
                        sc[i] = info.size;
                        dc[i] = info.is_dir;
                        ec[i] = info.ext;
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

    let mut rec = Records { parent, size, is_dir, ext };
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
    /// 目录序号 -> 递归内容构成（按扩展名分组）
    total_profile: Vec<[u64; knowledge::EXT_GROUP_COUNT]>,
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
    let mut direct_profile = vec![[0u64; knowledge::EXT_GROUP_COUNT]; n];
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); n];

    // 一遍挂父子 + 累计各目录的「直接」大小与内容构成
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
            direct_profile[pdi][rec.ext[i] as usize] += rec.size[i];
        }
    }

    // 每个目录把「直接量」沿父链上溯（O(目录数 × 深度)，远小于按文件上溯）
    let mut total_size = direct_size.clone();
    let mut total_profile = direct_profile.clone();
    for i in 0..cap {
        let di = dir_idx[i];
        if di == NOT_DIR || i as u64 == ROOT_RECORD {
            continue;
        }
        let (ds, dp) = (direct_size[di as usize], direct_profile[di as usize]);
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
            let tp = &mut total_profile[cdi as usize];
            for g in 0..knowledge::EXT_GROUP_COUNT {
                tp[g] += dp[g];
            }
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

    DirIndex { dir_idx, total_size, children, total_profile }
}

/// 第三遍：从根出发建剪枝树。每层先按大小排序取 Top-N，
/// 只为保留下来的节点取名字、拼路径、做知识库分类。
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
    let mut hit = knowledge::classify(path);
    if hit.category == Category::Other {
        if let Some(h) = knowledge::profile_classify(&index.total_profile[di]) {
            hit = h;
        }
    }

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
            category: hit.category,
            friendly_name: hit.friendly_name,
            description: hit.description,
            safety: hit.safety,
            children: Vec::new(),
        };
    }

    // 先按大小剪枝，再为保留项构建节点（避免为海量被剪掉的子项取名/分类）
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
            let chit = knowledge::classify(&child_path);
            kids.push(TreeNode {
                name: cname,
                path: child_path,
                size: csize,
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

    if overflow_count > 0 && overflow_size > 0 {
        kids.push(TreeNode {
            name: format!("其他 ({} 项)", overflow_count),
            path: format!("{}::__others__", path),
            size: overflow_size,
            is_dir: false,
            has_children: false,
            category: Category::Other,
            friendly_name: format!("其他 {} 个较小项目", overflow_count),
            description: "这些项目单个占用较小，已合并显示。".to_string(),
            safety: Safety::Keep,
            children: Vec::new(),
        });
    }

    TreeNode {
        name,
        path: path.to_string(),
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

/// MFT 直读扫描入口。root 形如 "C:\\"。
/// progress(已发现文件数, 已统计字节数, 精确百分比 0~100, 阶段文案)
pub fn scan_mft<F: FnMut(u64, u64, f32, &str)>(
    root: &str,
    mut progress: F,
) -> Result<TreeNode, String> {
    let (letter, root_norm) = parse_root(root)?;
    // 载入整张 MFT（可达 GB 级）：大块顺序读，按已读字节上报真实进度
    progress(0, 0, 0.0, "正在载入文件表");
    let volume = Volume::new(format!("\\\\.\\{}:", letter))
        .map_err(|e| format!("打开卷失败（需要管理员权限）：{:?}", e))?;
    let mut last_load = std::time::Instant::now();
    let mft = load_mft_fast(volume, &mut |loaded, total| {
        // 载入阶段自带节流（每 8MB 回调一次，再护一道 100ms）
        if last_load.elapsed().as_millis() >= 100 || loaded >= total {
            last_load = std::time::Instant::now();
            progress(0, 0, (loaded as f32 / total.max(1) as f32) * 100.0, "正在载入文件表");
        }
    })?;

    let mut last = (0u64, 0u64);
    let rec = collect_records(&mft, &mut |f, b, p| {
        last = (f, b);
        progress(f, b, p, "正在统计文件");
    });

    progress(last.0, last.1, 100.0, "正在汇总目录");
    let index = aggregate(&rec);

    // 名字仅为最终保留的少量节点解析（get_best_file_name 兼容属性列表等边界）
    let resolve = |n: u64| -> String {
        mft.get_record(n)
            .and_then(|f| f.get_best_file_name(&mft))
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
            ext: vec![knowledge::EXT_GROUP_OTHER as u8; cap],
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

    /// 回归锁：根记录未能从 MFT 解析出信息时（曾导致扫出空树），
    /// 只要按 collect_records 的兑底逻辑补齐根记录，聚合就不丢数据。
    #[test]
    fn root_fallback_keeps_tree_intact() {
        let rec = sample_records(); // parent[5]/is_dir[5] 即兑底状态
        let index = aggregate(&rec);
        let tree = build_tree(&rec, &index, &test_resolver, ROOT_RECORD, "C:\\".into(), "C:\\", 0);
        assert!(tree.size > 0 && !tree.children.is_empty());
    }

    #[test]
    fn parse_root_accepts_drive_roots_only() {
        assert!(parse_root("C:\\").is_ok());
        assert!(parse_root("d:").is_ok());
        assert!(parse_root("C:\\Windows").is_err());
        assert!(parse_root("\\\\server\\share").is_err());
    }

    #[test]
    fn ext_group_utf16_matches_path_version() {
        let utf16 = |s: &str| s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>();
        assert_eq!(
            ext_group_utf16(&utf16("视频.MP4")),
            knowledge::ext_group(std::path::Path::new("视频.mp4"))
        );
        assert_eq!(
            ext_group_utf16(&utf16("noext")),
            knowledge::EXT_GROUP_OTHER
        );
        assert_eq!(
            ext_group_utf16(&utf16("a.exe")),
            knowledge::ext_group(std::path::Path::new("a.exe"))
        );
    }
}
