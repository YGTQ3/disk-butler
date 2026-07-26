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
    /// "safe" 默认勾选；"caution" 默认不勾选
    pub safety: String,
    pub default_checked: bool,
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

/// 枚举所有存在且非空的清理项（含大小计算，可能耗几秒）。
pub fn list_items() -> Vec<CleanupItem> {
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
            Some(CleanupItem {
                id: c.id.to_string(),
                name: c.name.to_string(),
                description: c.description.to_string(),
                impact: c.impact.to_string(),
                paths: details,
                size,
                safety: c.safety.to_string(),
                default_checked: c.safety == "safe",
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
            default_checked: false,
        });
    }

    items.sort_by(|a, b| b.size.cmp(&a.size));
    items
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
