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

/// 注册表卸载表中 WPS Office 当前安装版本（如 "12.1.0.26895"）。
/// 查不到、或多处记录版本不一致（歧义）时返回 None——此时宁可不提供旧版本清理项。
fn wps_installed_version() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let hives: [(winreg::HKEY, &str); 3] = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    let mut found: Option<String> = None;
    for (hive, path) in hives {
        let Ok(key) = RegKey::predef(hive).open_subkey(path) else { continue };
        for sub in key.enum_keys().flatten() {
            let Ok(k) = key.open_subkey(&sub) else { continue };
            let Ok(name) = k.get_value::<String, _>("DisplayName") else { continue };
            if !name.contains("WPS Office") {
                continue;
            }
            let Ok(ver) = k.get_value::<String, _>("DisplayVersion") else { continue };
            let ver = ver.trim().to_string();
            if ver.is_empty() {
                continue;
            }
            match &found {
                Some(v) if *v != ver => return None,
                _ => found = Some(ver),
            }
        }
    }
    found
}

/// 从 WPS Office 目录下的子目录名中挑出「非当前版本」的版本目录（升级残留）。
/// 以注册表安装版本为锚点：当前版本目录必须存在于列表中，否则说明目录结构与
/// 注册表对不上（可能是绿色版/异常安装），返回空——宁可不清。
/// 只认纯版本号命名（如 12.1.0.26375），其余目录名一律不碰。
fn wps_stale_version_dirs(dir_names: &[String], installed: &str) -> Vec<String> {
    fn is_version_name(s: &str) -> bool {
        !s.is_empty()
            && s.split('.').count() >= 2
            && s.split('.')
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    }
    if !dir_names.iter().any(|d| d == installed) {
        return Vec::new();
    }
    dir_names
        .iter()
        .filter(|d| is_version_name(d) && d.as_str() != installed)
        .cloned()
        .collect()
}

/// Chromium 缓存指纹：同级目录下 Cache 与 (Code Cache 或 GPUCache) 并存才认定——
/// 单独一个 "Cache" 命名太泛（可能是应用自定义数据），不认。
/// 命中后只点名标准可重建缓存子目录，绝不返回同级的其他目录（账号/配置/聊天数据）。
fn chromium_cache_dirs(dir: &Path) -> Vec<PathBuf> {
    let cache = dir.join("Cache");
    if !cache.is_dir() {
        return Vec::new();
    }
    if !dir.join("Code Cache").is_dir() && !dir.join("GPUCache").is_dir() {
        return Vec::new();
    }
    let mut out = vec![cache];
    for sub in ["Code Cache", "GPUCache", "DawnCache", "ShaderCache"] {
        let p = dir.join(sub);
        if p.is_dir() {
            out.push(p);
        }
    }
    out
}

/// 扫描 LOCALAPPDATA/APPDATA 顶层目录的 Electron 应用缓存（指纹见 chromium_cache_dirs）。
/// 下探两级：顶层目录自身（如 taobao）+ 一级子目录（如千牛的多实例编号目录）。
/// 排除名单：已有专项规则的（防重复计数）+ 聊天数据同树的 Tencent 系（观察名单永久拒绝）。
fn collect_electron_caches(root: &Path, out: &mut Vec<PathBuf>) {
    fn excluded(name: &str) -> bool {
        let n = name.to_lowercase();
        n.starts_with("dingtalk")
            || matches!(
                n.as_str(),
                "microsoft"
                    | "google"
                    | "360chrome"
                    | "360chromex"
                    | "360se6"
                    | "code"
                    | "jianyingpro"
                    | "com.diskbutler.app"
                    | "temp"
                    | "packages"
                    | "tencent"
            )
    }
    let Ok(read) = std::fs::read_dir(root) else { return };
    for e in read.flatten() {
        let top = e.path();
        if !top.is_dir() {
            continue;
        }
        if excluded(&e.file_name().to_string_lossy()) {
            continue;
        }
        out.extend(chromium_cache_dirs(&top));
        if let Ok(subs) = std::fs::read_dir(&top) {
            for s in subs.flatten() {
                let sp = s.path();
                if sp.is_dir() {
                    out.extend(chromium_cache_dirs(&sp));
                }
            }
        }
    }
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
                    impact: "没有影响。软件需要更新时会重新下载；若某软件正提示“重启以完成更新”，建议先重启它再清理。",
                    safety: "safe",
                    paths: updaters,
                });
            }
        }

        // WPS 旧版本残留：升级后遗留的完整旧程序目录。
        // 以注册表安装版本为锚点（而非“保留最新版”——用户可能降级或回滚），
        // 查不到版本、或版本目录对不上时一律不提供，宁可漏清。
        if let Some(installed) = wps_installed_version() {
            let wps_root = local.join("Kingsoft").join("WPS Office");
            if let Ok(read) = std::fs::read_dir(&wps_root) {
                let names: Vec<String> = read
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                let stale = wps_stale_version_dirs(&names, &installed);
                if !stale.is_empty() {
                    out.push(Candidate {
                        id: "wps-old-versions",
                        name: "WPS 旧版本残留",
                        description: "WPS Office 升级后留下的旧版本程序文件（正在使用的版本不会列入）。",
                        impact: "没有影响。WPS 现在运行的是新版本，这些只是升级没删干净的旧程序。",
                        safety: "safe",
                        paths: stale.iter().map(|n| wps_root.join(n)).collect(),
                    });
                }
            }
        }

        // 浏览器缓存（仅 Cache/Code Cache 目录，不碰账户与历史记录）
        let mut browser: Vec<PathBuf> = Vec::new();
        let mut bases = vec![
            local.join("Microsoft").join("Edge").join("User Data").join("Default"),
            local.join("Google").join("Chrome").join("User Data").join("Default"),
            local.join("360Chrome").join("Chrome").join("User Data").join("Default"),
            local.join("360ChromeX").join("Chrome").join("User Data").join("Default"),
            local.join("CentBrowser").join("User Data").join("Default"),
            local.join("Quark").join("User Data").join("Default"),
        ];
        if let Some(r) = &roaming {
            bases.push(r.join("360se6").join("User Data").join("Default"));
        }
        for base in bases {
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
                name: "浏览器缓存 (Edge/Chrome/360系/夸克等)",
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

        // 剪映缓存：素材/渲染缓存，草稿和工程文件不在此目录
        let jy = local.join("JianyingPro").join("User Data").join("Cache");
        if jy.exists() {
            out.push(Candidate {
                id: "jianying-cache",
                name: "剪映缓存",
                description: "剪映视频编辑器的素材与渲染缓存（草稿和工程文件不在这里）。",
                impact: "⚠ 已下载的云端素材需要重新下载，打开旧项目首次加载会变慢。正在剪片子时不建议清。",
                safety: "caution",
                paths: vec![jy],
            });
        }

        // Playwright 自动化浏览器内核：缺失时工具会提示/自动重新下载
        let mut pw: Vec<PathBuf> = Vec::new();
        for name in ["ms-playwright", "ms-playwright-go", "ms-playwright-mcp"] {
            let p = local.join(name);
            if p.exists() {
                pw.push(p);
            }
        }
        if !pw.is_empty() {
            out.push(Candidate {
                id: "playwright-browsers",
                name: "Playwright 测试浏览器内核",
                description: "自动化工具 Playwright 下载的独立浏览器（不写代码的电脑上一般没有这项）。",
                impact: "⚠ 下次运行自动化任务时需重新下载数百 MB 浏览器内核。不用自动化/AI 工具的可放心清。",
                safety: "caution",
                paths: pw,
            });
        }

        // 钉钉网页缓存：仅 Chromium 缓存目录（目录名带版本号如 DingTalk_133），不碰消息与文件
        let mut dd: Vec<PathBuf> = Vec::new();
        if let Ok(read) = std::fs::read_dir(local) {
            for e in read.flatten() {
                let n = e.file_name().to_string_lossy().to_lowercase();
                if n.starts_with("dingtalk") && e.path().is_dir() {
                    for sub in ["Cache", "Code Cache", "GPUCache"] {
                        let p = e.path().join("Default").join(sub);
                        if p.exists() {
                            dd.push(p);
                        }
                    }
                }
            }
        }
        if !dd.is_empty() {
            out.push(Candidate {
                id: "dingtalk-cache",
                name: "钉钉网页缓存",
                description: "钉钉内置浏览器的图片与脚本缓存（聊天记录和文件不在这里）。",
                impact: "几乎没有影响。钉钉里的页面首次打开会稍慢一点。",
                safety: "safe",
                paths: dd,
            });
        }

        // uv 缓存：Python 包管理器 uv 的下载缓存（同 npm/pip 同类）
        out.push(Candidate {
            id: "uv-cache",
            name: "uv 缓存",
            description: "Python 包管理器 uv 下载的安装包缓存。",
            impact: "没有影响。以后安装依赖时会按需重新下载。",
            safety: "safe",
            paths: vec![local.join("uv").join("cache")],
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
            local.join("AMD").join("Radeonsoftware").join("cache"),
            local.join("Steam").join("htmlcache").join("ShaderCache"),
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

        // Android Studio 日志/临时文件：目录名带版本号（AndroidStudio2024.3），代码枚举
        let mut asl: Vec<PathBuf> = Vec::new();
        if let Ok(read) = std::fs::read_dir(local.join("Google")) {
            for e in read.flatten() {
                let n = e.file_name().to_string_lossy().to_lowercase();
                if n.starts_with("androidstudio") && e.path().is_dir() {
                    for sub in ["log", "tmp"] {
                        let p = e.path().join(sub);
                        if p.exists() {
                            asl.push(p);
                        }
                    }
                }
            }
        }
        if !asl.is_empty() {
            out.push(Candidate {
                id: "androidstudio-logs",
                name: "Android Studio 日志与临时文件",
                description: "Android Studio 运行产生的日志和临时文件（项目和 SDK 不在这里）。",
                impact: "没有影响。需要时会自动重建。",
                safety: "safe",
                paths: asl,
            });
        }

        // 群晖 Synology Drive 客户端日志/临时文件（同步的文件不在此）
        let mut syn: Vec<PathBuf> = Vec::new();
        for p in [
            local.join("SynologyDrive").join("log"),
            local.join("SynologyDrive").join("temp"),
            local.join("SynologyDrive").join("data").join("tmp"),
        ] {
            if p.exists() {
                syn.push(p);
            }
        }
        if !syn.is_empty() {
            out.push(Candidate {
                id: "synology-logs",
                name: "Synology Drive 日志与临时文件",
                description: "群晖同步客户端的运行日志和临时文件（你同步的文件不在这里）。",
                impact: "没有影响。客户端会自动重建。",
                safety: "safe",
                paths: syn,
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

        // 豆包着色器缓存：布局非标准 Electron 三件套（Cache 与 ShaderCache 不同级），
        // electron-cache 指纹不命中，按样本 cacheHits 实证点名收录。完全可再生。
        let doubao_sc = local.join("Doubao").join("User Data").join("ShaderCache");
        if doubao_sc.exists() {
            out.push(Candidate {
                id: "doubao-shadercache",
                name: "豆包着色器缓存",
                description: "豆包 AI 应用内置网页内核的显卡着色器缓存（对话记录和账号不在这里）。",
                impact: "没有影响。下次启动豆包时会自动重新生成。",
                safety: "safe",
                paths: vec![doubao_sc],
            });
        }

        // MiKTeX 字体/包缓存与日志：LaTeX 发行版按需自动重建。
        // friend-f 样本实证：fontconfig\cache、miktex\cache、miktex\log 三个子目录。
        let mut miktex: Vec<PathBuf> = Vec::new();
        for p in [
            local.join("MiKTeX").join("fontconfig").join("cache"),
            local.join("MiKTeX").join("miktex").join("cache"),
            local.join("MiKTeX").join("miktex").join("log"),
        ] {
            if p.exists() {
                miktex.push(p);
            }
        }
        if !miktex.is_empty() {
            out.push(Candidate {
                id: "miktex-cache",
                name: "MiKTeX 字体与包缓存",
                description: "LaTeX 排版系统 MiKTeX 的字体缓存和包管理日志。",
                impact: "没有影响。MiKTeX 编译文档时会自动重建字体缓存。",
                safety: "safe",
                paths: miktex,
            });
        }

        // OriginLab 临时文件：数据分析软件的 TMP 目录，纯临时文件。
        // friend-f 样本实证：OriginLab\102b\TMP（102b 为 Origin 2025b 版本目录）。
        let mut origin_tmp: Vec<PathBuf> = Vec::new();
        if let Ok(read) = std::fs::read_dir(local.join("OriginLab")) {
            for e in read.flatten() {
                if e.path().is_dir() {
                    let tmp = e.path().join("TMP");
                    if tmp.exists() {
                        origin_tmp.push(tmp);
                    }
                }
            }
        }
        if !origin_tmp.is_empty() {
            out.push(Candidate {
                id: "originlab-temp",
                name: "Origin 数据分析临时文件",
                description: "OriginLab 数据分析软件的临时文件目录。",
                impact: "没有影响。软件运行时按需重新创建。",
                safety: "safe",
                paths: origin_tmp,
            });
        }

        // TeamViewer 运行日志：远控软件的操作记录，官方文档提供"清除日志"按钮。
        let tv_logs = local.join("TeamViewer").join("Logs");
        if tv_logs.exists() {
            out.push(Candidate {
                id: "teamviewer-logs",
                name: "TeamViewer 运行日志",
                description: "TeamViewer 远程协助软件的运行日志。",
                impact: "没有影响。下次远程连接时会自动写新日志。",
                safety: "safe",
                paths: vec![tv_logs],
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

        // VS Code 缓存：仅标准缓存子目录，设置与插件不在其中
        let mut vsc: Vec<PathBuf> = Vec::new();
        for sub in ["Cache", "CachedData", "Code Cache", "GPUCache"] {
            let p = roaming.join("Code").join(sub);
            if p.exists() {
                vsc.push(p);
            }
        }
        if !vsc.is_empty() {
            out.push(Candidate {
                id: "vscode-cache",
                name: "VS Code 缓存",
                description: "VS Code 编辑器的界面与代码缓存（设置、插件和项目不在这里）。",
                impact: "几乎没有影响。下次启动 VS Code 会自动重建，首次打开稍慢。",
                safety: "safe",
                paths: vsc,
            });
        }

        // Adobe 媒体缓存：Pr/Ae 的预览渲染缓存，剪辑机上动辄几十 GB
        let mut amc: Vec<PathBuf> = Vec::new();
        for sub in ["Media Cache Files", "Media Cache"] {
            let p = roaming.join("Adobe").join("Common").join(sub);
            if p.exists() {
                amc.push(p);
            }
        }
        if !amc.is_empty() {
            out.push(Candidate {
                id: "adobe-media-cache",
                name: "Adobe 媒体缓存 (Pr/Ae)",
                description: "Premiere/After Effects 等生成的预览与音频缓存（工程文件和素材不在这里）。",
                impact: "⚠ 打开旧项目时需重新生成媒体缓存，首次会慢。正在剪片子时不建议清。",
                safety: "caution",
                paths: amc,
            });
        }

        // WPS 缓存：文档本体与云同步数据不在此目录。
        // friend-d/friend-e 双样本佐证扩充：office6\log、PDF\Cache 与 LOCALAPPDATA 侧
        // kupdateUI\cache、wpsoffice\cache（cacheHits 实证，只点名具体子目录，存在才收录）。
        let mut wps: Vec<PathBuf> = vec![roaming.join("kingsoft").join("office6").join("cache")];
        for p in [
            roaming.join("kingsoft").join("office6").join("log"),
            roaming.join("kingsoft").join("PDF").join("Cache"),
        ] {
            if p.exists() {
                wps.push(p);
            }
        }
        if let Some(local) = &local {
            for p in [
                local.join("Kingsoft").join("kupdateUI").join("cache"),
                local.join("Kingsoft").join("wpsoffice").join("cache"),
            ] {
                if p.exists() {
                    wps.push(p);
                }
            }
        }
        out.push(Candidate {
            id: "wps-cache",
            name: "WPS 缓存",
            description: "WPS Office 的运行缓存与日志（文档和云同步数据不在这里）。",
            impact: "没有影响。WPS 会在使用中自动重建。",
            safety: "safe",
            paths: wps,
        });

        // iSlide PPT 插件运行日志：2GB+ 的纯日志，删了无影响。
        // friend-f 样本实证：%APPDATA%\iSlide\iSlide Tools\Logs。
        let islide_logs = roaming.join("iSlide").join("iSlide Tools").join("Logs");
        if islide_logs.exists() {
            out.push(Candidate {
                id: "islide-logs",
                name: "iSlide PPT 插件日志",
                description: "iSlide PowerPoint 插件的运行日志。",
                impact: "没有影响。插件下次运行会自动写新日志。",
                safety: "safe",
                paths: vec![islide_logs],
            });
        }

        // 游戏运行日志与崩溃转储：纯程序自动生成的调试信息，不影响游戏进度。
        // friend-f 样本实证：Civ VI / SpiritCity / Pal / ManorLords / SlayTheSpire2 / Paradox launcher 等。
        // 注意：只点名 Logs/dumps/cache 子目录，不碰 Saved（存档）和 SaveGames。
        let mut game_logs: Vec<PathBuf> = Vec::new();
        // Civ VI
        let civ_base = local.as_ref().map(|l| l.join("Firaxis Games").join("Sid Meier's Civilization VI"));
        if let Some(civ) = &civ_base {
            for sub in ["Cache", "dumps", "Logs"] {
                let p = civ.join(sub);
                if p.exists() { game_logs.push(p); }
            }
        }
        // 其他游戏（Saved\Logs 模式）
        if let Some(l) = &local {
            for name in ["SpiritCity", "Pal", "ManorLords"] {
                let p = l.join(name).join("Saved").join("Logs");
                if p.exists() { game_logs.push(p); }
            }
        }
        // Daedalic (Shadow Tactics)
        if let Some(l) = &local {
            let p = l.join("Daedalic Entertainment GmbH").join("ShadowTacticsBladesoftheShogun").join("Cache");
            if p.exists() { game_logs.push(p); }
        }
        // Slay the Spire 2 (roaming)
        {
            let p = roaming.join("SlayTheSpire2").join("logs");
            if p.exists() { game_logs.push(p); }
        }
        // Paradox Interactive launcher
        if let Some(l) = &local {
            let p = l.join("Paradox Interactive").join("launcher-v2").join("logs");
            if p.exists() { game_logs.push(p); }
        }
        {
            let p = roaming.join("Paradox Interactive").join("launcher-v2").join("cache");
            if p.exists() { game_logs.push(p); }
        }
        if !game_logs.is_empty() {
            out.push(Candidate {
                id: "game-logs",
                name: "游戏运行日志与崩溃转储",
                description: "各游戏的运行日志和崩溃报告（存档和存档不在这里），仅供排查问题时使用。",
                impact: "没有影响。游戏下次运行会自动写新日志。",
                safety: "safe",
                paths: game_logs,
            });
        }
    }

    // Electron 通用缓存：Chromium 指纹识别（Cache 与 Code Cache/GPUCache 同级并存才认定），
    // 一条引擎规则覆盖淘宝/千牛/Notion 等网页内核应用；已有专项规则的目录列入排除名单防重复。
    let mut electron: Vec<PathBuf> = Vec::new();
    for root in [&local, &roaming] {
        if let Some(root) = root {
            collect_electron_caches(root, &mut electron);
        }
    }
    if !electron.is_empty() {
        electron.sort();
        electron.dedup();
        out.push(Candidate {
            id: "electron-cache",
            name: "网页型应用缓存 (Electron)",
            description: "淘宝、千牛等网页内核应用的运行缓存（按 Chromium 缓存指纹识别，只清缓存目录，不碰账号与数据）。",
            impact: "几乎没有影响。应用下次启动时会自动重建缓存，首次打开稍慢一点。",
            safety: "safe",
            paths: electron,
        });
    }

    out
}

/// 打开系统回收站窗口。回收站是虚拟目录，没有文件系统路径，
/// 不能走 openPath（Win10 上会报「找不到」），必须用 shell: 协议。
pub fn open_recycle_bin() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("shell:RecycleBinFolder")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开回收站失败：{}", e))
}

/// 清理策略分型：垃圾残留（删了纯赚）/ 性能缓存（空间充足可留）/ 数据类（默认不动）
fn kind_of(id: &str) -> &'static str {
    match id {
        "temp" | "updaters" | "crash-reports" | "androidstudio-logs" | "synology-logs"
        | "wps-old-versions" | "islide-logs" | "originlab-temp" | "teamviewer-logs"
        | "game-logs" => "junk",
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

/// 系统级清理报告（Windows\Temp + 更新缓存）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCleanReport {
    pub freed: u64,
    pub free_before: u64,
    pub free_after: u64,
    /// 因检测到挂起更新/待重启，已跳过 Windows 更新缓存（只清了系统临时目录）
    pub update_cache_skipped: bool,
}

/// 系统级清理·只读分析结果：两个目标各自当前大小 + 是否有挂起更新。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAnalyzeReport {
    /// C:\Windows\Temp 当前大小
    pub temp_bytes: u64,
    /// SoftwareDistribution\Download 更新下载缓存当前大小
    pub update_cache_bytes: u64,
    /// 是否有挂起的 Windows 更新（有则清理时会跳过更新缓存）
    pub update_pending: bool,
}

/// DISM 只读分析报告：原始关键行 + 微软是否推荐清理
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepAnalyzeReport {
    /// DISM 报告的关键行（如“备份和已禁用的功能 : 19.24 GB”），原样展示保证透明
    pub lines: Vec<String>,
    /// 微软官方建议：Some(true)=推荐清理，Some(false)=不必，None=未能识别
    pub recommended: Option<bool>,
    /// “备份和已禁用的功能”体积（GB）——可清理主体，供前端大字标注预估释放量
    pub backup_gb: Option<f64>,
}

/// 从 DISM 输出中提取关键信息行与推荐结论（中英文系统兼容）
fn parse_dism_analyze(text: &str) -> DeepAnalyzeReport {
    let mut lines_out: Vec<String> = Vec::new();
    let mut recommended: Option<bool> = None;
    let mut backup_gb: Option<f64> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('[') || line.contains("====") {
            continue; // 进度条与空行
        }
        // 过滤头部信息
        if line.starts_with("部署映像")
            || line.starts_with("Deployment Image")
            || line.starts_with("版本")
            || line.starts_with("Version")
            || line.starts_with("映像版本")
            || line.starts_with("Image Version")
        {
            continue;
        }
        if !line.contains(':') && !line.contains('：') {
            continue;
        }
        let lower = line.to_lowercase();
        if line.contains("组件存储清理") || lower.contains("component store cleanup") {
            let value = line.rsplit([':', '：']).next().unwrap_or("").trim();
            recommended = Some(value.contains('是') || value.eq_ignore_ascii_case("yes"));
        }
        // 提取可清理主体体积：“备份和已禁用的功能 : 19.24 GB”
        if line.contains("备份和已禁用") || lower.contains("backups and disabled") {
            let value = line.rsplit([':', '：']).next().unwrap_or("").trim();
            let mut parts = value.split_whitespace();
            if let Some(num) = parts.next() {
                let unit = parts.next().unwrap_or("GB").to_uppercase();
                if let Ok(n) = num.parse::<f64>() {
                    backup_gb = Some(match unit.as_str() {
                        "MB" => n / 1024.0,
                        "KB" => n / 1024.0 / 1024.0,
                        "TB" => n * 1024.0,
                        _ => n,
                    });
                }
            }
        }
        lines_out.push(line.to_string());
    }
    DeepAnalyzeReport {
        lines: lines_out,
        recommended,
        backup_gb,
    }
}

/// 只读分析：DISM /AnalyzeComponentStore，不做任何更改。
/// 提权运行并把输出写入临时日志，完成后读回（中文系统日志为 GBK 编码）。
pub fn deep_analyze() -> Result<DeepAnalyzeReport, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let log = std::env::temp_dir().join("diskbutler-dism-analyze.log");
    let _ = std::fs::remove_file(&log);

    // 用提权的 cmd 重定向输出到日志（cmd 重定向保持原始 ANSI/GBK 编码，便于统一解码）
    let ps = format!(
        r#"$p = Start-Process -Verb RunAs -Wait -PassThru -FilePath cmd.exe -ArgumentList '/d','/c','Dism /Online /Cleanup-Image /AnalyzeComponentStore > "{}" 2>&1'; exit $p.ExitCode"#,
        log.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("启动分析失败：{}", e))?;

    let code = status.code().unwrap_or(-1);
    if code != 0 {
        return Err(if code == 1 {
            "已取消授权，未执行分析。".to_string()
        } else {
            format!("分析未完成（退出码 {}），可稍后重试。", code)
        });
    }

    let bytes = std::fs::read(&log).map_err(|e| format!("读取分析结果失败：{}", e))?;
    // 中文系统为 GBK；英文系统的 ASCII 内容用 GBK 解码同样无损
    let (text, _, _) = encoding_rs::GBK.decode(&bytes);
    let report = parse_dism_analyze(&text);
    if report.lines.is_empty() {
        return Err("分析完成但未能读到报告内容，可稍后重试。".to_string());
    }
    Ok(report)
}

/// 执行 DISM /StartComponentCleanup：清理 Windows 更新的旧版本备份（WinSxS）。
/// 与 deep_analyze 同一条已验证通道：提权 cmd + 日志重定向，同步等待 5~20 分钟。
pub fn deep_clean() -> Result<DeepCleanReport, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let free_before = c_drive_free();
    let log = std::env::temp_dir().join("diskbutler-dism-clean.log");
    let _ = std::fs::remove_file(&log);

    let ps = format!(
        r#"$p = Start-Process -Verb RunAs -Wait -PassThru -FilePath cmd.exe -ArgumentList '/d','/c','Dism /Online /Cleanup-Image /StartComponentCleanup > "{}" 2>&1'; exit $p.ExitCode"#,
        log.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("启动清理失败：{}", e))?;

    let code = status.code().unwrap_or(-1);
    // 0 = 成功；3010 = 成功但需重启
    if code != 0 && code != 3010 {
        // 附带日志尾部帮助定位（GBK 解码）
        let tail = std::fs::read(&log)
            .ok()
            .map(|b| {
                let (t, _, _) = encoding_rs::GBK.decode(&b);
                t.lines()
                    .rev()
                    .filter(|l| !l.trim().is_empty() && !l.starts_with('['))
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .unwrap_or_default();
        return Err(if code == 1 {
            "已取消授权，未执行清理。".to_string()
        } else {
            format!("系统清理未完成（退出码 {}）。{}", code, tail)
        });
    }

    let free_after = c_drive_free();
    Ok(DeepCleanReport {
        freed: free_after.saturating_sub(free_before),
        free_before,
        free_after,
    })
}

// ---------- 高级：系统级清理（Windows\Temp + 更新缓存，需提权） ----------

/// 是否有挂起的 Windows 更新/待重启（清更新缓存前必须为否，避免打断进行中的更新）。
fn update_pending() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending")
        .is_ok()
        || hklm
            .open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired")
            .is_ok()
}

/// 将脚本编码为 PowerShell -EncodedCommand 参数（UTF-16LE 后 Base64）。
/// 内联传参，避免向 %TEMP% 落地可被同账户进程篡改的脚本文件（承接卸载流程的 TOCTOU 加固姿势）。
fn encode_ps_command(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    base64_encode(&utf16)
}

/// 标准 Base64 编码（无外部依赖，供 -EncodedCommand 内联提权脚本用）。
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// 高级：系统级清理——清系统临时目录 + Windows 更新下载缓存（需 UAC 授权，单次提权）。
/// 红线：`Windows\Temp` 与 `SoftwareDistribution\Download` 精确路径、删内容不删目录、占用自动跳过；
/// 有挂起更新时只清临时目录、跳过更新缓存；清更新缓存用 try/finally 停/启 wuauserv 保证服务拉回。
pub fn deep_clean_system() -> Result<SystemCleanReport, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let free_before = c_drive_free();
    let pending = update_pending();

    // 提权内联脚本：先清系统 Temp 内容；无挂起更新时才停服务清更新缓存（finally 保证 wuauserv 拉回）
    let mut inner = String::from(
        "$ErrorActionPreference='SilentlyContinue'; \
         Get-ChildItem -LiteralPath \"$env:SystemRoot\\Temp\" -Force | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue;",
    );
    if !pending {
        inner.push_str(
            " try { Stop-Service wuauserv -Force -ErrorAction SilentlyContinue; \
             Get-ChildItem -LiteralPath \"$env:SystemRoot\\SoftwareDistribution\\Download\" -Force | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue } \
             finally { Start-Service wuauserv -ErrorAction SilentlyContinue }",
        );
    }
    let encoded = encode_ps_command(&inner);
    let ps = format!(
        r#"$p = Start-Process -Verb RunAs -Wait -PassThru -FilePath powershell.exe -ArgumentList '-NoProfile','-EncodedCommand','{}'; exit $p.ExitCode"#,
        encoded
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("启动系统清理失败：{}", e))?;

    let code = status.code().unwrap_or(-1);
    if code != 0 {
        return Err(if code == 1 {
            "已取消授权，未执行清理。".to_string()
        } else {
            format!("系统清理未完成（退出码 {}），可稍后重试。", code)
        });
    }

    let free_after = c_drive_free();
    Ok(SystemCleanReport {
        freed: free_after.saturating_sub(free_before),
        free_before,
        free_after,
        update_cache_skipped: pending,
    })
}

/// 系统级清理·只读分析：提权量出 Windows\Temp 与更新缓存的当前大小，供用户决定是否值得清。
/// 与清理分离（先分析→展示→用户确认），风险操作不盲点；Windows\Temp 受 ACL 保护需提权才能量准。
pub fn analyze_system_clean() -> Result<SystemAnalyzeReport, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // 用不可预测的随机文件名（pid+纳秒），避免固定 %TEMP% 路径被同账户进程预置 junction
    // 劫持提权写入（承接 docs/17 的 TOCTOU 加固原则；DEV-STANDARDS §二.8）
    let uniq = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let log = std::env::temp_dir().join(format!("diskbutler-sysclean-{}.txt", uniq));
    let _ = std::fs::remove_file(&log);
    let inner = format!(
        "$t=(Get-ChildItem -LiteralPath \"$env:SystemRoot\\Temp\" -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum; \
         $d=(Get-ChildItem -LiteralPath \"$env:SystemRoot\\SoftwareDistribution\\Download\" -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum; \
         \"$([long]$t) $([long]$d)\" | Out-File -LiteralPath '{}' -Encoding ascii",
        log.to_string_lossy().replace('\'', "''")
    );
    let encoded = encode_ps_command(&inner);
    let ps = format!(
        r#"$p = Start-Process -Verb RunAs -Wait -PassThru -FilePath powershell.exe -ArgumentList '-NoProfile','-EncodedCommand','{}'; exit $p.ExitCode"#,
        encoded
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("启动分析失败：{}", e))?;

    let code = status.code().unwrap_or(-1);
    if code != 0 {
        return Err(if code == 1 {
            "已取消授权，未执行分析。".to_string()
        } else {
            format!("分析未完成（退出码 {}），可稍后重试。", code)
        });
    }

    let text = std::fs::read_to_string(&log).map_err(|e| format!("读取分析结果失败：{}", e))?;
    let _ = std::fs::remove_file(&log);
    let mut it = text.split_whitespace();
    let temp_bytes = it.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let update_cache_bytes = it.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    Ok(SystemAnalyzeReport {
        temp_bytes,
        update_cache_bytes,
        update_pending: update_pending(),
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
    fn base64_encode_matches_known_vectors() {
        // RFC 4648 标准向量，覆盖无补/单补/双补三种情况
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encode_ps_command_is_utf16le_base64() {
        // -EncodedCommand 期望 UTF-16LE 后 Base64；"A中" => 41 00 2D 4E => "QQAtTg=="
        assert_eq!(encode_ps_command("A中"), "QQAtTg==");
    }

    #[test]
    fn update_pending_does_not_panic() {
        // 只读注册表探测，任何机器上都不应 panic
        let _ = update_pending();
    }

    #[test]
    fn kind_mapping_is_sensible() {
        assert_eq!(kind_of("temp"), "junk");
        assert_eq!(kind_of("crash-reports"), "junk");
        assert_eq!(kind_of("wps-old-versions"), "junk");
        assert_eq!(kind_of("npm-cache"), "cache");
        assert_eq!(kind_of("electron-cache"), "cache");
        assert_eq!(kind_of("recycle-bin"), "data");
        assert_eq!(kind_of("idm-dwnldata"), "data");
    }

    #[test]
    fn chromium_fingerprint_requires_sibling_caches() {
        let base = std::env::temp_dir().join(format!("db-electron-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);

        // 完整指纹：Cache + Code Cache + GPUCache → 三个都命中
        let full = base.join("full");
        for d in ["Cache", "Code Cache", "GPUCache"] {
            std::fs::create_dir_all(full.join(d)).unwrap();
        }
        let hits = chromium_cache_dirs(&full);
        assert_eq!(hits.len(), 3);

        // 只有单独的 Cache（可能是应用自定义数据）→ 不认
        let lone = base.join("lone");
        std::fs::create_dir_all(lone.join("Cache")).unwrap();
        assert!(chromium_cache_dirs(&lone).is_empty());

        // 只有 Code Cache 没有 Cache → 不认
        let nocache = base.join("nocache");
        std::fs::create_dir_all(nocache.join("Code Cache")).unwrap();
        assert!(chromium_cache_dirs(&nocache).is_empty());

        // 命中结果只含缓存子目录，绝不包含同级其他目录
        let mixed = base.join("mixed");
        for d in ["Cache", "GPUCache", "Local Storage", "blob_storage"] {
            std::fs::create_dir_all(mixed.join(d)).unwrap();
        }
        let hits = chromium_cache_dirs(&mixed);
        assert!(hits.iter().all(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            n == "Cache" || n == "GPUCache"
        }));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn wps_stale_dirs_anchor_on_installed_version() {
        let dirs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // 正常升级：装的是新版，旧版目录是残留
        assert_eq!(
            wps_stale_version_dirs(&dirs(&["12.1.0.26375", "12.1.0.26895"]), "12.1.0.26895"),
            vec!["12.1.0.26375".to_string()]
        );
        // 用户在用旧版（降级/回滚）：残留的反而是更新的那个目录
        assert_eq!(
            wps_stale_version_dirs(&dirs(&["12.1.0.26375", "12.1.0.26895"]), "12.1.0.26375"),
            vec!["12.1.0.26895".to_string()]
        );
        // 注册表版本在目录里找不到（绿色版/异常安装）：一律不清
        assert!(wps_stale_version_dirs(&dirs(&["12.1.0.26375"]), "12.1.0.26895").is_empty());
        // 非版本号命名的目录（如配置目录）永不列入残留
        assert_eq!(
            wps_stale_version_dirs(
                &dirs(&["12.1.0.26375", "12.1.0.26895", "Common", "6.0"]),
                "12.1.0.26895"
            ),
            vec!["12.1.0.26375".to_string(), "6.0".to_string()]
        );
        // 只装了一个版本：没有残留
        assert!(wps_stale_version_dirs(&dirs(&["12.1.0.26895"]), "12.1.0.26895").is_empty());
    }

    #[test]
    fn wps_cache_paths_stay_inside_kingsoft_trees() {
        // 扩充后的 wps-cache 只允许指向 kingsoft/Kingsoft 树内的点名子目录，
        // 防止未来误把无关路径塞进该项
        let c = candidates();
        let wps = c.iter().find(|x| x.id == "wps-cache").expect("wps-cache 应始终存在");
        assert!(!wps.paths.is_empty());
        for p in &wps.paths {
            let s = p.to_string_lossy().to_lowercase().replace('\\', "/");
            assert!(s.contains("/kingsoft/"), "wps-cache 出现树外路径: {}", s);
            assert!(
                s.ends_with("/cache") || s.ends_with("/log"),
                "wps-cache 只允许点名 cache/log 子目录: {}",
                s
            );
        }
    }

    #[test]
    fn doubao_item_only_targets_shadercache() {
        // 存在才收录：本机没装豆包时该项缺席属正常；一旦出现，路径必须
        // 精确落在 User Data\ShaderCache，绝不扩大到同级账号/对话数据
        let c = candidates();
        if let Some(item) = c.iter().find(|x| x.id == "doubao-shadercache") {
            assert_eq!(item.paths.len(), 1);
            let s = item.paths[0].to_string_lossy().to_lowercase().replace('\\', "/");
            assert!(s.ends_with("doubao/user data/shadercache"), "路径越界: {}", s);
        }
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

    #[test]
    fn parse_dism_analyze_chinese_output() {
        let sample = "\
部署映像服务和管理工具\n\
版本: 10.0.26100.8737\n\
\n\
[==========================100.0%==========================]\n\
组件存储(WinSxS)信息:\n\
\n\
组件存储的实际大小 : 27.76 GB\n\
备份和已禁用的功能 : 19.24 GB\n\
可回收的程序包 : 15\n\
推荐使用组件存储清理 : 是\n\
操作成功完成。\n";
        let r = parse_dism_analyze(sample);
        assert_eq!(r.recommended, Some(true));
        assert!(r.lines.iter().any(|l| l.contains("19.24 GB")));
        // 可清理主体体积应被正确提取
        assert!((r.backup_gb.unwrap() - 19.24).abs() < 0.01);
        // 进度条和版本头应被过滤
        assert!(!r.lines.iter().any(|l| l.contains("100.0%")));
        assert!(!r.lines.iter().any(|l| l.starts_with("版本")));
    }

    #[test]
    fn parse_dism_analyze_not_recommended() {
        let sample = "推荐使用组件存储清理 : 否\n";
        assert_eq!(parse_dism_analyze(sample).recommended, Some(false));
    }
}
