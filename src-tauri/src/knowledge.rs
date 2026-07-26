//! 分类知识库：把一路磁盘优化实战经验沉淀成路径规则。
//! 每条规则将「路径片段」映射到「分类 + 人话名称 + 说明 + 安全等级」，
//! 供前端在 TreeMap 悬停/详情卡中向普通用户解释「这是什么、能不能动」。

use serde::Serialize;

/// 磁盘占用分类。决定 TreeMap 的色彩语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// 操作系统本体（蓝）
    System,
    /// 已安装软件（紫）
    Software,
    /// 应用缓存 / 临时文件（橙，暗示可清理）
    Cache,
    /// 个人文件（绿）
    Personal,
    /// 休眠 / 虚拟内存等系统大文件（灰）
    SystemFile,
    /// 未识别（默认中性色）
    Other,
}

/// 安全等级：能否清理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Safety {
    /// 可安全清理（缓存、临时文件）
    Safe,
    /// 清理有代价，需谨慎（更新备份、断点续传块等）
    Caution,
    /// 请勿删除（系统核心、个人数据）
    Keep,
}

/// 一条命中的知识条目（返回给前端）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeHit {
    pub category: Category,
    /// 人话名称，如「Windows 组件仓库」
    pub friendly_name: String,
    /// 一句话说明「这是什么、能不能动」
    pub description: String,
    pub safety: Safety,
}

struct Rule {
    /// 匹配用的路径片段（小写，使用正斜杠）。命中即采用。
    needle: &'static str,
    category: Category,
    friendly_name: &'static str,
    description: &'static str,
    safety: Safety,
}

/// 规则表：靠前的更具体，优先命中。约 30 条，全部来自实战经验。
const RULES: &[Rule] = &[
    // ---------- 系统本体 ----------
    Rule {
        needle: "windows/winsxs",
        category: Category::System,
        friendly_name: "Windows 组件仓库 (WinSxS)",
        description: "系统零件仓库与更新备份，会随更新膨胀。请勿手动删除，只能用系统自带的 DISM 工具清理旧备份。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "windows/installer",
        category: Category::System,
        friendly_name: "Windows 安装缓存",
        description: "已安装程序的卸载/修复所需文件。删除可能导致某些软件无法卸载或更新，请勿手动清理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "windows/system32",
        category: Category::System,
        friendly_name: "系统核心文件 (System32)",
        description: "Windows 运行的核心，绝对不能删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "windows/syswow64",
        category: Category::System,
        friendly_name: "32 位兼容核心 (SysWOW64)",
        description: "运行 32 位程序所需的系统文件，不能删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "windows/softwaredistribution",
        category: Category::Cache,
        friendly_name: "Windows 更新缓存",
        description: "已下载的更新安装包。更新完成后可通过磁盘清理释放，属于安全清理项。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "windows/temp",
        category: Category::Cache,
        friendly_name: "系统临时文件",
        description: "系统运行产生的临时文件，可安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "windows/logs",
        category: Category::Cache,
        friendly_name: "系统日志",
        description: "系统运行日志，可安全清理，不影响使用。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "windows",
        category: Category::System,
        friendly_name: "Windows 系统",
        description: "操作系统本体文件，除专门的清理工具外请勿手动改动。",
        safety: Safety::Keep,
    },
    // ---------- 系统大文件 ----------
    Rule {
        needle: "hiberfil.sys",
        category: Category::SystemFile,
        friendly_name: "休眠文件",
        description: "保存休眠时的内存快照。如果你使用休眠功能，请保留；关闭休眠可释放此空间。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "pagefile.sys",
        category: Category::SystemFile,
        friendly_name: "虚拟内存 (页面文件)",
        description: "物理内存不足时的补充。可在系统设置中调整大小或迁移到其他分区，不建议直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "swapfile.sys",
        category: Category::SystemFile,
        friendly_name: "系统交换文件",
        description: "现代应用的内存交换文件，由系统管理，不能手动删除。",
        safety: Safety::Keep,
    },
    // ---------- 应用缓存（AppData） ----------
    Rule {
        needle: "appdata/local/temp",
        category: Category::Cache,
        friendly_name: "临时文件 (Temp)",
        description: "各种程序产生的临时文件，可安全清理，正在使用的文件会自动跳过。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/local/npm-cache",
        category: Category::Cache,
        friendly_name: "npm 缓存",
        description: "Node.js 包管理器的下载缓存，可用 npm cache clean --force 安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/roaming/npm",
        category: Category::Software,
        friendly_name: "npm 全局包",
        description: "全局安装的 Node 命令行工具，删除会导致对应命令失效，请谨慎。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/pip/cache",
        category: Category::Cache,
        friendly_name: "pip 缓存",
        description: "Python 包管理器的下载缓存，可用 pip cache purge 安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/local/jianyingpro",
        category: Category::Cache,
        friendly_name: "剪映缓存/草稿",
        description: "剪映的草稿与素材缓存，占用较大。可在剪映内清理，草稿位置也可迁移到其他分区。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "roaming/idm/dwnldata",
        category: Category::Cache,
        friendly_name: "IDM 下载分块",
        description: "IDM 未完成下载的临时分块。直接删除会丢失断点续传进度，建议在 IDM 内删除无用任务。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/jetbrains",
        category: Category::Cache,
        friendly_name: "JetBrains 索引缓存",
        description: "IDE 的代码索引缓存，可在 IDE 内 Invalidate Caches 重建，删除不影响项目。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/local/ms-playwright",
        category: Category::Software,
        friendly_name: "Playwright 浏览器内核",
        description: "自动化测试用的浏览器内核，删除后相关测试需重新下载。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/microsoft/windows/inetcache",
        category: Category::Cache,
        friendly_name: "IE/系统网页缓存",
        description: "系统网页组件的缓存，可安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/local/google/chrome",
        category: Category::Cache,
        friendly_name: "Chrome 数据/缓存",
        description: "Chrome 浏览器缓存与配置，缓存部分可在浏览器内清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/microsoft/edge",
        category: Category::Cache,
        friendly_name: "Edge 数据/缓存",
        description: "Edge 浏览器缓存与配置，缓存部分可在浏览器内清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/roaming/tencent",
        category: Category::Cache,
        friendly_name: "腾讯软件数据",
        description: "QQ/微信等腾讯软件的聊天与缓存数据，含聊天记录，清理前请确认。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/roaming/dingtalk",
        category: Category::Cache,
        friendly_name: "钉钉数据",
        description: "钉钉的缓存与本地数据，含文件缓存，可在钉钉内清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "weixin",
        category: Category::Cache,
        friendly_name: "微信数据/缓存",
        description: "微信聊天记录、图片、小程序缓存，占用往往很大。含重要数据，请在微信内清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "baidunetdisk",
        category: Category::Cache,
        friendly_name: "百度网盘缓存",
        description: "百度网盘的下载与传输缓存，可在客户端设置中清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "-updater",
        category: Category::Cache,
        friendly_name: "软件更新包残留",
        description: "各软件下载的旧版更新包，一般可删除，软件会在需要时重新下载。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/local",
        category: Category::Cache,
        friendly_name: "应用本地数据",
        description: "各软件的本地数据与缓存，多数缓存可清理，但也可能含配置，逐项确认更稳妥。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/roaming",
        category: Category::Software,
        friendly_name: "应用配置数据",
        description: "软件的配置与账户数据，删除会丢失设置，请谨慎。",
        safety: Safety::Caution,
    },
    // ---------- 开发环境 ----------
    Rule {
        needle: "miniconda3",
        category: Category::Software,
        friendly_name: "Miniconda 环境",
        description: "Python 环境与依赖包，删除会破坏 Python 开发环境。",
        safety: Safety::Keep,
    },
    Rule {
        needle: ".cargo",
        category: Category::Cache,
        friendly_name: "Rust 依赖缓存",
        description: "Rust 的依赖包缓存，可用 cargo cache 清理，删除后会重新下载。",
        safety: Safety::Caution,
    },
    Rule {
        needle: ".rustup",
        category: Category::Software,
        friendly_name: "Rust 工具链",
        description: "Rust 编译器工具链，删除会破坏 Rust 开发环境。",
        safety: Safety::Keep,
    },
    // ---------- 个人文件 ----------
    Rule {
        needle: "downloads",
        category: Category::Personal,
        friendly_name: "下载文件夹",
        description: "你下载的文件，建议人工浏览后清理不需要的安装包。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "desktop",
        category: Category::Personal,
        friendly_name: "桌面",
        description: "桌面上的文件，属于个人数据，请自行整理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "documents",
        category: Category::Personal,
        friendly_name: "文档",
        description: "个人文档，请勿随意清理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "onedrive",
        category: Category::Personal,
        friendly_name: "OneDrive 云盘",
        description: "OneDrive 本地副本，属于个人数据，删除会触发云端同步变化。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "pictures",
        category: Category::Personal,
        friendly_name: "图片",
        description: "个人图片，请勿随意清理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "videos",
        category: Category::Personal,
        friendly_name: "视频",
        description: "个人视频，占用可能较大，请自行整理。",
        safety: Safety::Keep,
    },
    // ---------- 已安装软件 ----------
    Rule {
        needle: "program files (x86)",
        category: Category::Software,
        friendly_name: "已安装软件 (32 位)",
        description: "已安装的 32 位软件本体，请通过卸载程序移除，而非手动删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "program files",
        category: Category::Software,
        friendly_name: "已安装软件",
        description: "已安装软件的本体，请通过卸载程序移除，而非手动删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "programdata",
        category: Category::Software,
        friendly_name: "软件公共数据",
        description: "所有用户共享的软件数据，部分为缓存，删除前请确认。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "$recycle.bin",
        category: Category::Cache,
        friendly_name: "回收站",
        description: "已删除文件的暂存区，清空回收站即可释放。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "system volume information",
        category: Category::SystemFile,
        friendly_name: "系统还原点",
        description: "系统还原点与卷影副本，出问题时可用于恢复。可在系统保护中限制其大小。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "users",
        category: Category::Personal,
        friendly_name: "用户目录",
        description: "你的个人文件与应用数据都在这里，需逐层查看再决定。",
        safety: Safety::Keep,
    },
];

/// 根据完整路径分类。返回最先命中的规则；未命中返回 Other/Keep。
pub fn classify(path: &str) -> KnowledgeHit {
    let lower = path.to_lowercase().replace('\\', "/");
    for rule in RULES {
        if lower.contains(rule.needle) {
            return KnowledgeHit {
                category: rule.category,
                friendly_name: rule.friendly_name.to_string(),
                description: rule.description.to_string(),
                safety: rule.safety,
            };
        }
    }
    KnowledgeHit {
        category: Category::Other,
        friendly_name: "未识别项目".to_string(),
        description: "暂无该项目的说明。如果不确定用途，建议先不要清理。".to_string(),
        safety: Safety::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_cat(path: &str, expected: Category) {
        let hit = classify(path);
        assert_eq!(hit.category, expected, "path={} got={:?}", path, hit.category);
    }

    #[test]
    fn winsxs_is_system_and_caution() {
        let hit = classify(r"C:\Windows\WinSxS");
        assert_eq!(hit.category, Category::System);
        assert_eq!(hit.safety, Safety::Caution);
    }

    #[test]
    fn temp_is_cache_and_safe() {
        let hit = classify(r"C:\Users\l3268\AppData\Local\Temp\abc.tmp");
        assert_eq!(hit.category, Category::Cache);
        assert_eq!(hit.safety, Safety::Safe);
    }

    #[test]
    fn npm_cache_is_safe() {
        assert_cat(r"C:\Users\l3268\AppData\Local\npm-cache", Category::Cache);
        assert_eq!(classify(r"C:\Users\l3268\AppData\Local\npm-cache").safety, Safety::Safe);
    }

    #[test]
    fn pip_cache_is_safe() {
        let hit = classify(r"C:\Users\l3268\AppData\Local\pip\cache");
        assert_eq!(hit.safety, Safety::Safe);
    }

    #[test]
    fn jianying_is_cache() {
        assert_cat(r"C:\Users\l3268\AppData\Local\JianyingPro", Category::Cache);
    }

    #[test]
    fn idm_dwnldata_is_caution() {
        let hit = classify(r"C:\Users\l3268\AppData\Roaming\IDM\DwnlData");
        assert_eq!(hit.safety, Safety::Caution);
    }

    #[test]
    fn hiberfil_is_system_file() {
        assert_cat(r"C:\hiberfil.sys", Category::SystemFile);
    }

    #[test]
    fn pagefile_is_system_file() {
        assert_cat(r"C:\pagefile.sys", Category::SystemFile);
    }

    #[test]
    fn program_files_is_software() {
        assert_cat(r"C:\Program Files\SomeApp", Category::Software);
        assert_cat(r"C:\Program Files (x86)\SomeApp", Category::Software);
    }

    #[test]
    fn downloads_is_personal() {
        assert_cat(r"C:\Users\l3268\Downloads", Category::Personal);
    }

    #[test]
    fn recycle_bin_is_safe() {
        let hit = classify(r"C:\$Recycle.Bin");
        assert_eq!(hit.safety, Safety::Safe);
    }

    #[test]
    fn updater_residue_is_safe() {
        let hit = classify(r"C:\Users\l3268\AppData\Local\hanako-updater");
        assert_eq!(hit.safety, Safety::Safe);
    }

    #[test]
    fn unknown_defaults_to_keep() {
        let hit = classify(r"C:\SomeRandomFolder\xyz");
        assert_eq!(hit.category, Category::Other);
        assert_eq!(hit.safety, Safety::Keep);
    }

    #[test]
    fn specific_rule_wins_over_generic() {
        // windows/winsxs 应先于泛化的 windows 命中
        let hit = classify(r"C:\Windows\WinSxS\amd64_something");
        assert_eq!(hit.friendly_name, "Windows 组件仓库 (WinSxS)");
    }
}
