//! 一键清理引擎：白名单制。
//! 只清理经过验证安全的目录；每一项都向用户说明「这是什么、删了会怎样」。
//! 前端只能传回条目 id，实际路径始终由本模块重新解析，杜绝任意路径删除。

use jwalk::WalkDir;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathDetail {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    pub id: String,
    pub name: String,
    /// 这是什么
    pub description: String,
    /// 删了会怎样
    pub impact: String,
    /// 将被清理的具体路径及各自大小（详情透明化）
    pub paths: Vec<PathDetail>,
    pub size: u64,
    /// "safe" | "caution"
    pub safety: String,
    /// 清理策略分型："junk" 垃圾残留 | "cache" 性能缓存 | "data" 数据类
    pub kind: String,
    /// 占用相对磁盘剩余空间微不足道，不值得动
    pub negligible: bool,
    pub default_checked: bool,
}

/// 清理页扫描结果：项目列表 + 磁盘空间水位（前端据此展示缓存策略提示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupScan {
    pub items: Vec<CleanupItem>,
    pub free: u64,
    pub space_tight: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub id: String,
    pub name: String,
    pub freed: u64,
    /// 被占用而跳过的文件数（正在运行的程序持有）
    pub skipped: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub results: Vec<ItemResult>,
    pub total_freed: u64,
    pub free_before: u64,
    pub free_after: u64,
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .skip_hidden(false)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// 删除目录「内容」（保留目录本身）。返回跳过的文件/目录数，被占用的自动跳过。
fn delete_contents(path: &Path) -> u64 {
    let mut skipped = 0u64;
    let Ok(read) = std::fs::read_dir(path) else {
        return 1;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if std::fs::remove_dir_all(&p).is_err() {
                // 整体删除失败：深入删除能删的部分，剩余计为跳过
                skipped += delete_contents(&p);
                if std::fs::remove_dir(&p).is_err() {
                    skipped += 1;
                }
            }
        } else if std::fs::remove_file(&p).is_err() {
            skipped += 1;
        }
    }
    skipped
}

// ---------- 回收站（Windows Shell API） ----------

fn recycle_bin_size() -> u64 {
    use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};
    unsafe {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            i64Size: 0,
            i64NumItems: 0,
        };
        let hr = SHQueryRecycleBinW(std::ptr::null(), &mut info);
        if hr == 0 {
            info.i64Size as u64
        } else {
            0
        }
    }
}

fn empty_recycle_bin() -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    };
    unsafe {
        let hr = SHEmptyRecycleBinW(
            std::ptr::null_mut(),
            std::ptr::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        );
        // S_OK=0；回收站本来就空时部分系统返回 E_UNEXPECTED，同样视作成功
        if hr == 0 || recycle_bin_size() == 0 {
            Ok(())
        } else {
            Err(format!("清空回收站失败 (HRESULT=0x{:08X})", hr))
        }
    }
}

// ---------- 白名单定义 ----------

struct Candidate {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    impact: &'static str,
    safety: &'static str,
    paths: Vec<PathBuf>,
}

/// 组装候选清理项。路径全部由环境变量动态解析。
fn candidates() -> Vec<Candidate> {
    let local = env_path("LOCALAPPDATA");
    let roaming = env_path("APPDATA");
    let mut out: Vec<Candidate> = Vec::new();

    if let Some(local) = &local {
        out.push(Candidate {
            id: "temp",
            name: "临时文件 (Temp)",
            description: "各种程序运行时产生的临时文件，Windows 不会主动清空。",
            impact: "没有影响。正在使用的文件会自动跳过，不会影响正在运行的程序。",
            safety: "safe",
            paths: vec![local.join("Temp")],
        });
        out.push(Candidate {
            id: "npm-cache",
            name: "npm 缓存",
            description: "Node.js 包管理器下载的安装包缓存。",
            impact: "没有影响。以后安装依赖时会按需重新下载。",
            safety: "safe",
            paths: vec![local.join("npm-cache")],
        });
        out.push(Candidate {
            id: "pip-cache",
            name: "pip 缓存",
            description: "Python 包管理器下载的安装包缓存。",
            impact: "没有影响。以后安装依赖时会按需重新下载。",
            safety: "safe",
            paths: vec![local.join("pip").join("cache")],
        });

        // 各软件更新包残留：LOCALAPPDATA 下 *-updater 目录
        if let Ok(read) = std::fs::read_dir(local) {
            let updaters: Vec<PathBuf> = read
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .to_lowercase()
                        .ends_with("-updater")
                        && e.path().is_dir()
                })
                .map(|e| e.path())
                .collect();
            if !updaters.is_empty() {
                out.push(Candidate {
                    id: "updaters",
                    name: "软件更新包残留",
                    description: "各软件下载的旧版本更新安装包（*-updater 目录）。",
                    impact: "没有影响。软件需要更新时会重新下载。",
                    safety: "safe",
                    paths: updaters,
                });
            }
        }

        // 浏览器缓存（仅 Cache/Code Cache 目录，不碰账户与历史记录）
        let mut browser: Vec<PathBuf> = Vec::new();
        for base in [
            local.join("Microsoft").join("Edge").join("User Data").join("Default"),
            local.join("Google").join("Chrome").join("User Data").join("Default"),
        ] {
            for sub in ["Cache", "Code Cache", "GPUCache"] {
                let p = base.join(sub);
                if p.exists() {
                    browser.push(p);
                }
            }
        }
        if !browser.is_empty() {
            out.push(Candidate {
                id: "browser-cache",
                name: "浏览器缓存 (Edge/Chrome)",
                description: "浏览器缓存的网页图片和脚本，只清缓存，不碰账户、密码和历史记录。",
                impact: "几乎没有影响。常用网页首次打开会稍慢一点，浏览器使用中时部分文件会跳过。",
                safety: "safe",
                paths: browser,
            });
        }

        out.push(Candidate {
            id: "jetbrains-cache",
            name: "JetBrains IDE 索引缓存",
            description: "IDEA/PyCharm 等的代码索引缓存（配置和项目不在这里）。",
            impact: "打开项目后 IDE 需要重新建立索引，期间会变慢几分钟。不常用 IDE 可放心清。",
            safety: "caution",
            paths: vec![local.join("JetBrains")],
        });

        // 图形着色器缓存：完全可再生
        let mut gpu: Vec<PathBuf> = Vec::new();
        for p in [
            local.join("D3DSCache"),
            local.join("NVIDIA").join("DXCache"),
            local.join("NVIDIA").join("GLCache"),
            local.join("AMD").join("DxCache"),
            local.join("AMD").join("DxcCache"),
            local.join("AMD").join("GLCache"),
        ] {
            if p.exists() {
                gpu.push(p);
            }
        }
        if !gpu.is_empty() {
            out.push(Candidate {
                id: "gpu-cache",
                name: "显卡着色器缓存",
                description: "DirectX/显卡驱动的着色器编译缓存。",
                impact: "没有影响。游戏/应用首次启动时会自动重新生成，首次加载稍慢。",
                safety: "safe",
                paths: gpu,
            });
        }

        out.push(Candidate {
            id: "crash-reports",
            name: "崩溃转储与错误报告",
            description: "程序崩溃时的现场快照和 Windows 错误报告存档，只对排查问题有用。",
            impact: "没有影响。除非你正在追查某个程序的崩溃原因。",
            safety: "safe",
            paths: vec![
                local.join("CrashDumps"),
                local.join("Microsoft").join("Windows").join("WER"),
            ],
        });

        // 前端包管理器缓存（npm-cache 已单独列出）
        let mut jscache: Vec<PathBuf> = Vec::new();
        for p in [
            local.join("Yarn").join("Cache"),
            local.join("pnpm").join("store"),
            local.join("pnpm-cache"),
        ] {
            if p.exists() {
                jscache.push(p);
            }
        }
        if !jscache.is_empty() {
            out.push(Candidate {
                id: "js-pkg-cache",
                name: "Yarn/pnpm 缓存",
                description: "Node.js 包管理器的下载缓存。",
                impact: "基本没有影响。pnpm 项目重装依赖时需重新下载。",
                safety: "safe",
                paths: jscache,
            });
        }
    }

    // 用户主目录下的开发/AI 缓存（体积大、重下载成本高，均列为谨慎项）
    if let Some(home) = env_path("USERPROFILE") {
        out.push(Candidate {
            id: "gradle-cache",
            name: "Gradle 构建缓存",
            description: "Android/Java 项目的构建与依赖缓存。",
            impact: "没有影响。下次构建时重新下载依赖，首次构建变慢。",
            safety: "safe",
            paths: vec![home.join(".gradle").join("caches")],
        });
        out.push(Candidate {
            id: "nuget-cache",
            name: "NuGet 包缓存",
            description: ".NET 项目的依赖包缓存。",
            impact: "项目重新构建时需重新下载全部依赖，可能耗时较长。",
            safety: "caution",
            paths: vec![home.join(".nuget").join("packages")],
        });
        out.push(Candidate {
            id: "maven-cache",
            name: "Maven 依赖缓存",
            description: "Java 项目的依赖包缓存 (.m2)。",
            impact: "项目重新构建时需重新下载全部依赖，国内网络下可能很慢。",
            safety: "caution",
            paths: vec![home.join(".m2").join("repository")],
        });
        out.push(Candidate {
            id: "hf-cache",
            name: "HuggingFace AI 模型缓存",
            description: "AI 模型文件缓存，单个模型动辄数 GB。",
            impact: "⚠ 再次使用对应模型时需重新下载大文件，耗时取决于网速。",
            safety: "caution",
            paths: vec![home.join(".cache").join("huggingface")],
        });
    }

    if let Some(roaming) = &roaming {
        out.push(Candidate {
            id: "idm-dwnldata",
            name: "IDM 未完成下载分块",
            description: "IDM 下载器暂停/未完成任务的临时分块文件。",
            impact: "⚠ 未完成的下载任务会丢失断点进度，需要从头下载。已完成的文件不受影响。",
            safety: "caution",
            paths: vec![roaming.join("IDM").join("DwnlData")],
        });
    }

    out
}

/// 清理策略分型：垃圾残留（删了纯赚）/ 性能缓存（空间充足可留）/ 数据类（默认不动）
fn kind_of(id: &str) -> &'static str {
    match id {
        "temp" | "updaters" | "crash-reports" => "junk",
        "idm-dwnldata" | "recycle-bin" => "data",
        _ => "cache",
    }
}

/// 智能默认勾选：
/// - 微小项（占用 < max(100MB, 剩余空间的1%)）不勾——不值得动；
/// - 垃圾残留：安全且不微小则勾；
/// - 性能缓存：只有磁盘空间紧张时才勾（缓存是拿空间换体验的资产）；
/// - 数据类：永不默认勾。
fn compute_default(kind: &str, safety: &str, negligible: bool, space_tight: bool) -> bool {
    if negligible || safety != "safe" {
        return false;
    }
    match kind {
        "junk" => true,
        "cache" => space_tight,
        _ => false,
    }
}

/// 枚举所有存在且非空的清理项（含大小计算，可能耗几秒）。
pub fn list_items() -> CleanupScan {
    let free = c_drive_free();
    let total = c_drive_total();
    // 空间紧张：剩余 < 15% 或 < 20GB
    let space_tight = free < total / 100 * 15 || free < 20 * 1024 * 1024 * 1024;
    // 微小阈值：剩余空间的 1%，不低于 100MB
    let negligible_threshold = std::cmp::max(100 * 1024 * 1024, free / 100);

    let mut items: Vec<CleanupItem> = candidates()
        .into_iter()
        .filter_map(|c| {
            // 逐路径算大小，只保留非空路径，供前端“查看详情”展示
            let mut details: Vec<PathDetail> = c
                .paths
                .iter()
                .filter_map(|p| {
                    let s = dir_size(p);
                    if s == 0 {
                        return None;
                    }
                    Some(PathDetail {
                        path: p.to_string_lossy().to_string(),
                        size: s,
                    })
                })
                .collect();
            details.sort_by(|a, b| b.size.cmp(&a.size));
            let size: u64 = details.iter().map(|d| d.size).sum();
            if size == 0 {
                return None;
            }
            let kind = kind_of(c.id);
            let negligible = size < negligible_threshold;
            Some(CleanupItem {
                id: c.id.to_string(),
                name: c.name.to_string(),
                description: c.description.to_string(),
                impact: c.impact.to_string(),
                paths: details,
                size,
                safety: c.safety.to_string(),
                kind: kind.to_string(),
                negligible,
                default_checked: compute_default(kind, c.safety, negligible, space_tight),
            })
        })
        .collect();

    // 回收站（独立于文件系统白名单）
    let rb = recycle_bin_size();
    if rb > 0 {
        items.push(CleanupItem {
            id: "recycle-bin".to_string(),
            name: "回收站".to_string(),
            description: "你之前删除的文件都暂存在这里。".to_string(),
            impact: "⚠ 清空后这些文件将永久删除、无法找回。请先确认里面没有还想要的东西。"
                .to_string(),
            paths: vec![PathDetail {
                path: "系统回收站（所有盘符）".to_string(),
                size: rb,
            }],
            size: rb,
            safety: "caution".to_string(),
            kind: "data".to_string(),
            negligible: rb < negligible_threshold,
            default_checked: false,
        });
    }

    items.sort_by(|a, b| b.size.cmp(&a.size));
    CleanupScan {
        items,
        free,
        space_tight,
    }
}

fn c_drive_free() -> u64 {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with('C'.to_string().as_str()))
        .map(|d| d.available_space())
        .unwrap_or(0)
}

fn c_drive_total() -> u64 {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with('C'.to_string().as_str()))
        .map(|d| d.total_space())
        .unwrap_or(0)
}

// ---------- 高级：系统深度清理（DISM 组件存储清理） ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepCleanReport {
    pub freed: u64,
    pub free_before: u64,
    pub free_after: u64,
}

/// 执行 DISM /StartComponentCleanup：清理 Windows 更新的旧版本备份（WinSxS）。
/// 会弹 UAC 提权窗口与 DISM 控制台进度窗口，耗时 5~20 分钟，同步等待完成。
pub fn deep_clean() -> Result<DeepCleanReport, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let free_before = c_drive_free();
    // 用 PowerShell 提权启动 Dism 并等待退出码；拒绝 UAC 时 Start-Process 抛异常 -> exit 1
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$p = Start-Process -Verb RunAs -Wait -PassThru -FilePath Dism.exe -ArgumentList '/Online','/Cleanup-Image','/StartComponentCleanup','/NoRestart'; exit $p.ExitCode",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("启动清理失败：{}", e))?;

    let code = status.code().unwrap_or(-1);
    // 0 = 成功；3010 = 成功但需重启
    if code != 0 && code != 3010 {
        return Err(if code == 1 {
            "已取消授权，未执行清理。".to_string()
        } else {
            format!("系统清理未完成（退出码 {}），可稍后重试。", code)
        });
    }

    let free_after = c_drive_free();
    Ok(DeepCleanReport {
        freed: free_after.saturating_sub(free_before),
        free_before,
        free_after,
    })
}

/// 执行清理。只接受白名单 id，路径由本函数重新解析。
pub fn run(ids: Vec<String>) -> CleanupReport {
    let free_before = c_drive_free();
    let all = candidates();
    let mut results: Vec<ItemResult> = Vec::new();

    for id in &ids {
        if id == "recycle-bin" {
            let before = recycle_bin_size();
            let res = empty_recycle_bin();
            let after = recycle_bin_size();
            results.push(ItemResult {
                id: id.clone(),
                name: "回收站".to_string(),
                freed: before.saturating_sub(after),
                skipped: 0,
                error: res.err(),
            });
            continue;
        }

        let Some(c) = all.iter().find(|c| c.id == *id) else {
            results.push(ItemResult {
                id: id.clone(),
                name: id.clone(),
                freed: 0,
                skipped: 0,
                error: Some("不在白名单中，已拒绝".to_string()),
            });
            continue;
        };

        let mut freed = 0u64;
        let mut skipped = 0u64;
        for p in &c.paths {
            if !p.exists() {
                continue;
            }
            let before = dir_size(p);
            skipped += delete_contents(p);
            let after = dir_size(p);
            freed += before.saturating_sub(after);
        }
        results.push(ItemResult {
            id: id.clone(),
            name: c.name.to_string(),
            freed,
            skipped,
            error: None,
        });
    }

    let free_after = c_drive_free();
    let total_freed = results.iter().map(|r| r.freed).sum();
    CleanupReport {
        results,
        total_freed,
        free_before,
        free_after,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_have_unique_ids() {
        let c = candidates();
        let mut ids: Vec<&str> = c.iter().map(|x| x.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "candidate id 重复");
    }

    #[test]
    fn kind_mapping_is_sensible() {
        assert_eq!(kind_of("temp"), "junk");
        assert_eq!(kind_of("crash-reports"), "junk");
        assert_eq!(kind_of("npm-cache"), "cache");
        assert_eq!(kind_of("recycle-bin"), "data");
        assert_eq!(kind_of("idm-dwnldata"), "data");
    }

    #[test]
    fn default_check_logic() {
        // 垃圾：安全且不微小 -> 勾
        assert!(compute_default("junk", "safe", false, false));
        // 微小项永不勾（用户：58MB 的崩溃日志不值得动）
        assert!(!compute_default("junk", "safe", true, true));
        // 缓存：空间充足不勾，紧张才勾
        assert!(!compute_default("cache", "safe", false, false));
        assert!(compute_default("cache", "safe", false, true));
        // 数据类永不默认勾
        assert!(!compute_default("data", "safe", false, true));
        // 谨慎项永不默认勾
        assert!(!compute_default("junk", "caution", false, true));
    }

    #[test]
    fn run_rejects_non_whitelisted_id() {
        let report = run(vec!["c-drive-root".to_string()]);
        assert_eq!(report.results.len(), 1);
        assert!(report.results[0].error.is_some());
        assert_eq!(report.results[0].freed, 0);
    }

    #[test]
    fn delete_contents_keeps_dir_itself() {
        let tmp = std::env::temp_dir().join("disk_butler_test_dc");
        let sub = tmp.join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("f.txt"), b"x").unwrap();
        let skipped = delete_contents(&tmp);
        assert_eq!(skipped, 0);
        assert!(tmp.exists(), "目录本身应保留");
        assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0, "内容应清空");
        let _ = std::fs::remove_dir(&tmp);
    }

    #[test]
    fn dir_size_of_missing_path_is_zero() {
        assert_eq!(dir_size(Path::new(r"C:\__no_such_dir_disk_butler__")), 0);
    }
}
