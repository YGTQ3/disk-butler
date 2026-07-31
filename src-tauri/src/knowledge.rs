//! 分类知识库：把一路磁盘优化实战经验沉淀成路径规则。
//! 每条规则将「路径片段」映射到「分类 + 人话名称 + 说明 + 安全等级」，
//! 供前端在 TreeMap 悬停/详情卡中向普通用户解释「这是什么、能不能动」。

use serde::{Deserialize, Serialize};

/// 磁盘占用分类。决定 TreeMap 的色彩语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        needle: "appdata/local/dingtalk",
        category: Category::Cache,
        friendly_name: "钉钉内置浏览器数据",
        description: "钉钉内置浏览器的缓存与页面数据，其中 Cache 部分可由一键清理安全清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/wsl",
        category: Category::SystemFile,
        friendly_name: "WSL Linux 子系统磁盘",
        description: "Linux 子系统的虚拟磁盘，内含你的 Linux 环境和文件。请用 wsl 命令管理，不要直接删文件。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/uv",
        category: Category::Cache,
        friendly_name: "uv 包缓存",
        description: "Python 包管理器 uv 的下载缓存，可用 uv cache clean 安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/local/synologydrive",
        category: Category::Software,
        friendly_name: "Synology Drive 客户端数据",
        description: "群晖同步客户端的程序数据（你同步的文件不在这里），其中 log/temp 可由一键清理安全清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/google/androidstudio",
        category: Category::Software,
        friendly_name: "Android Studio 数据",
        description: "Android Studio 的缓存与日志（项目不在这里），其中 log/tmp 可由一键清理安全清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "radeonsoftware/cache",
        category: Category::Cache,
        friendly_name: "AMD Radeon 软件缓存",
        description: "AMD 显卡管理软件的缓存，可安全清理，会自动重建。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "appdata/roaming/code",
        category: Category::Software,
        friendly_name: "VS Code 数据",
        description: "VS Code 的设置与缓存，其中 Cache 类子目录可由一键清理安全清理，设置不受影响。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "adobe/common/media cache",
        category: Category::Cache,
        friendly_name: "Adobe 媒体缓存",
        description: "Premiere/AE 的预览渲染缓存，可安全清理，打开旧项目时会重新生成。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "kingsoft/office6/cache",
        category: Category::Cache,
        friendly_name: "WPS 缓存",
        description: "WPS Office 的运行缓存，可安全清理，文档不受影响。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "kingsoft/office6/log",
        category: Category::Cache,
        friendly_name: "WPS 日志",
        description: "WPS Office 的运行日志，可安全清理，不影响使用。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "kingsoft/pdf/cache",
        category: Category::Cache,
        friendly_name: "WPS PDF 缓存",
        description: "WPS PDF 组件的运行缓存，可安全清理，文档不受影响。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "kingsoft/kupdateui/cache",
        category: Category::Cache,
        friendly_name: "WPS 升级组件缓存",
        description: "WPS 升级程序的界面缓存，可安全清理，会自动重建。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "kingsoft/wpsoffice/cache",
        category: Category::Cache,
        friendly_name: "WPS 程序缓存",
        description: "WPS Office 程序侧的运行缓存，可安全清理，文档不受影响。",
        safety: Safety::Safe,
    },
    // 通用着色器缓存：任何路径含 /shadercache 的目录都是可再生的显卡编译缓存
    // （豆包/元宝等 WebView 应用；浏览器自身的先被上方更具体的浏览器规则命中）
    Rule {
        needle: "/shadercache",
        category: Category::Cache,
        friendly_name: "着色器缓存",
        description: "程序生成的显卡着色器编译缓存，可安全清理，下次启动会自动重建。",
        safety: Safety::Safe,
    },
    // Keil/Arm 芯片支持包：删除后不会自动重建，需在 Pack Installer 手动重装——
    // 不进清理白名单，仅在透视页解释（friend-e 样本，嵌入式开发机常见大头）
    Rule {
        needle: "appdata/local/keil_v5",
        category: Category::Software,
        friendly_name: "Keil MDK 数据 (Keil_v5)",
        description: "Keil 开发工具的芯片支持包与配置。删除后需在 Pack Installer 重新下载安装，请勿当垃圾清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/local/arm/packs",
        category: Category::Software,
        friendly_name: "Arm CMSIS 芯片支持包",
        description: "嵌入式开发的芯片支持包（CMSIS Pack）。删除后编译会报错，需重新下载安装，请勿当垃圾清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "appdata/roaming/pcsuite",
        category: Category::Personal,
        friendly_name: "vivo 办公套件数据 (pcsuite)",
        description: "vivo 办公套件同步的云笔记与文档（PDF/PPT/录音等）。云端一般有保留，但删除前请先确认没有只存在本机的文件；软件体检的「残留检查」可引导安全清理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "weixin",
        category: Category::Cache,
        friendly_name: "微信数据/缓存",
        description: "微信聊天记录、图片、小程序缓存，占用往往很大。含重要数据，请在微信内清理。",
        safety: Safety::Caution,
    },
    // ---------- 下载/接收类目录：名字像缓存，实为个人数据，必须排在对应的程序缓存规则之前 ----------
    Rule {
        needle: "baidunetdiskdownload",
        category: Category::Personal,
        friendly_name: "百度网盘下载的文件",
        description: "你从百度网盘下载下来的文件，是个人数据而不是缓存，请自行整理，不要当垃圾清理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "wechat files",
        category: Category::Personal,
        friendly_name: "微信接收的文件",
        description: "聊天中接收的文件/图片/视频。删除后若聊天记录已过期将无法重新下载，谨慎清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "xwechat_files",
        category: Category::Personal,
        friendly_name: "微信接收的文件",
        description: "聊天中接收的文件/图片/视频。删除后若聊天记录已过期将无法重新下载，谨慎清理。",
        safety: Safety::Caution,
    },
    // 中文目录名与泛化名：用户自行迁移微信数据到 D:\微信 等目录时也要认得出（泛规则必须排在上方具体规则之后）
    Rule {
        needle: "微信",
        category: Category::Personal,
        friendly_name: "微信数据",
        description: "微信相关数据（聊天记录、接收的文件或迁移过来的存储目录），属于个人数据，请在微信内管理，不要直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "wechat",
        category: Category::Personal,
        friendly_name: "微信数据",
        description: "微信相关数据（聊天记录、接收的文件或程序数据），含个人数据，请在微信内管理，不要直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "聊天记录",
        category: Category::Personal,
        friendly_name: "聊天记录备份",
        description: "聊天记录类数据，属于个人数据，请勿删除。",
        safety: Safety::Keep,
    },
    // 创作类个人数据：删了无法找回，一律 Keep
    Rule {
        needle: "jianyingpro drafts",
        category: Category::Personal,
        friendly_name: "剪映草稿",
        description: "剪映的视频工程草稿，是你的创作成果，删了无法找回，请勿删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: ".minecraft",
        category: Category::Personal,
        friendly_name: "Minecraft 游戏数据",
        description: "我的世界的存档、材质包与模组（saves 里是你亲手搭建的世界），删除无法找回，请勿删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "mailmasterdata",
        category: Category::Personal,
        friendly_name: "网易邮箱大师邮件数据",
        description: "本地邮件与附件数据，属于个人数据，请在邮箱大师内管理，不要直接删除。",
        safety: Safety::Keep,
    },
    // 常见中文个人目录名：用户自建目录大量用中文命名，宁可过度保护不可漏保护
    Rule {
        needle: "照片",
        category: Category::Personal,
        friendly_name: "照片",
        description: "照片类个人文件，请自行整理，不要直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "图片",
        category: Category::Personal,
        friendly_name: "图片",
        description: "图片类个人文件，请自行整理，不要直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "视频",
        category: Category::Personal,
        friendly_name: "视频文件",
        description: "视频类个人文件，请自行整理，不要直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "音乐",
        category: Category::Personal,
        friendly_name: "音乐文件",
        description: "音乐类个人文件，请自行整理，不要直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "备份",
        category: Category::Personal,
        friendly_name: "备份数据",
        description: "备份类数据，删了可能无法找回，请勿删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "下载",
        category: Category::Personal,
        friendly_name: "下载的文件",
        description: "下载目录里是你主动获得的文件，属于个人数据，请自行整理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "文档",
        category: Category::Personal,
        friendly_name: "文档",
        description: "文档类个人文件，请自行整理，不要直接删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "thunderdownload",
        category: Category::Personal,
        friendly_name: "迅雷下载的文件",
        description: "你用迅雷下载的文件，属于个人数据，请自行整理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "baidunetdisk",
        category: Category::Cache,
        friendly_name: "百度网盘程序数据",
        description: "百度网盘客户端的程序数据与传输缓存（注意：你下载的文件不在这里），可在客户端设置中清理。",
        safety: Safety::Caution,
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
    // ---------- 游戏/开发/AI 生态 ----------
    Rule {
        needle: "steamapps",
        category: Category::Software,
        friendly_name: "Steam 游戏本体",
        description: "已安装的 Steam 游戏文件。想释放空间请在 Steam 内卸载游戏，不要手动删除。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "epic games",
        category: Category::Software,
        friendly_name: "Epic 游戏本体",
        description: "Epic 平台的游戏文件，请通过 Epic 启动器管理。",
        safety: Safety::Keep,
    },
    Rule {
        needle: "node_modules",
        category: Category::Software,
        friendly_name: "前端项目依赖",
        description: "代码项目的依赖包。删除后需 npm install 重装；不再维护的旧项目可连项目一起处理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: ".nuget/packages",
        category: Category::Cache,
        friendly_name: "NuGet 包缓存",
        description: ".NET 依赖包缓存，删除后构建时重新下载。",
        safety: Safety::Caution,
    },
    Rule {
        needle: ".m2/repository",
        category: Category::Cache,
        friendly_name: "Maven 依赖缓存",
        description: "Java 项目依赖缓存，删除后构建时重新下载（可能很慢）。",
        safety: Safety::Caution,
    },
    Rule {
        needle: ".gradle/caches",
        category: Category::Cache,
        friendly_name: "Gradle 构建缓存",
        description: "Android/Java 构建缓存，可安全删除，下次构建时重新生成。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "go/pkg/mod",
        category: Category::Cache,
        friendly_name: "Go 模块缓存",
        description: "Go 依赖缓存（文件带只读属性），建议用 go clean -modcache 清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "yarn/cache",
        category: Category::Cache,
        friendly_name: "Yarn 缓存",
        description: "Node.js 依赖缓存，可安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "pnpm",
        category: Category::Cache,
        friendly_name: "pnpm 存储",
        description: "pnpm 的全局依赖存储，删除后项目需重新安装依赖。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "huggingface",
        category: Category::Cache,
        friendly_name: "HuggingFace AI 模型缓存",
        description: "AI 模型文件，动辄数 GB。删除后再用时需重新下载大文件。",
        safety: Safety::Caution,
    },
    Rule {
        needle: ".ollama",
        category: Category::Software,
        friendly_name: "Ollama 本地大模型",
        description: "本地大语言模型文件，建议用 ollama rm 命令删除不用的模型。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "conda/pkgs",
        category: Category::Cache,
        friendly_name: "conda 包缓存",
        description: "与已建环境存在硬链接共享，手动删除可能破坏环境！请用 conda clean -p 清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "miniconda3/pkgs",
        category: Category::Cache,
        friendly_name: "conda 包缓存",
        description: "与已建环境存在硬链接共享，手动删除可能破坏环境！请用 conda clean -p 清理。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "docker/wsl",
        category: Category::SystemFile,
        friendly_name: "Docker 虚拟磁盘",
        description: "Docker 的镜像与容器数据盘，体积大。请通过 Docker Desktop 清理镜像来瘦身。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "d3dscache",
        category: Category::Cache,
        friendly_name: "DirectX 着色器缓存",
        description: "图形着色器缓存，可安全清理，游戏/应用首次启动时重新生成。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "crashdumps",
        category: Category::Cache,
        friendly_name: "程序崩溃转储",
        description: "程序崩溃时的现场快照，排查问题用，可安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "windows/wer",
        category: Category::Cache,
        friendly_name: "Windows 错误报告",
        description: "程序出错时的报告存档，可安全清理。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "nvidia/dxcache",
        category: Category::Cache,
        friendly_name: "显卡着色器缓存",
        description: "NVIDIA 着色器缓存，可安全清理，会自动重建。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "nvidia/glcache",
        category: Category::Cache,
        friendly_name: "显卡着色器缓存",
        description: "NVIDIA 着色器缓存，可安全清理，会自动重建。",
        safety: Safety::Safe,
    },
    Rule {
        needle: "$windows.~bt",
        category: Category::Cache,
        friendly_name: "系统升级临时文件",
        description: "Windows 大版本升级的临时文件，升级完成后可用磁盘清理工具安全删除。",
        safety: Safety::Caution,
    },
    Rule {
        needle: "onedrivetemp",
        category: Category::Cache,
        friendly_name: "OneDrive 临时文件",
        description: "OneDrive 同步产生的临时文件，可安全清理。",
        safety: Safety::Safe,
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

// ---------- 内容启发式推断：不只看目录名，还看里面装的是什么 ----------

pub const EXT_GROUP_COUNT: usize = 7;
const G_MEDIA: usize = 0; // 视频/图片/音频
const G_DOCS: usize = 1; // 文档/电子书
const G_ARCHIVE: usize = 2; // 压缩包/镜像
const G_PROGRAM: usize = 3; // 可执行/程序库
const G_CODE: usize = 4; // 代码/工程文件
const G_CACHE: usize = 5; // 临时/日志/缓存类
const G_OTHER: usize = 6;

/// 未知扩展名/无扩展名的兑底分组。
pub const EXT_GROUP_OTHER: usize = G_OTHER;

/// 根据文件扩展名归入内容组
pub fn ext_group(path: &std::path::Path) -> usize {
    let Some(ext) = path.extension().map(|e| e.to_string_lossy().to_lowercase()) else {
        return G_OTHER;
    };
    ext_group_of(&ext)
}

/// 同 ext_group，但直接接收已小写的扩展名（MFT 扫描零分配路径使用）
pub fn ext_group_of(ext: &str) -> usize {
    match ext {
        "mp4" | "mkv" | "avi" | "mov" | "flv" | "wmv" | "ts" | "webm" | "mp3" | "wav" | "flac"
        | "aac" | "m4a" | "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "bmp" | "raw"
        | "psd" => G_MEDIA,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "epub"
        | "mobi" | "csv" => G_DOCS,
        "zip" | "rar" | "7z" | "tar" | "gz" | "xz" | "iso" | "img" => G_ARCHIVE,
        "exe" | "dll" | "msi" | "sys" | "ocx" | "so" | "node" | "pyd" | "winmd" | "mui" => G_PROGRAM,
        "js" | "ts" | "tsx" | "jsx" | "py" | "rs" | "java" | "c" | "cpp" | "h" | "hpp" | "go"
        | "cs" | "json" | "toml" | "yaml" | "yml" | "css" | "html" | "vue" | "lock" => G_CODE,
        "tmp" | "temp" | "log" | "cache" | "db" | "ldb" | "dat" | "etl" | "dmp" | "old" | "bak"
        | "idx" => G_CACHE,
        _ => G_OTHER,
    }
}

/// 对「未识别」目录按内容构成推断分类。
/// 只在体量足够大时才猜；推断结果仅用于展示，永不进入清理白名单。
pub fn profile_classify(profile: &[u64; EXT_GROUP_COUNT]) -> Option<KnowledgeHit> {
    let total: u64 = profile.iter().sum();
    if total < 50 * 1024 * 1024 {
        return None; // 太小的目录不值得猜，也猜不准
    }
    let share = |i: usize| profile[i] as f64 / total as f64;
    let pct = |i: usize| (share(i) * 100.0).round() as u32;

    if share(G_MEDIA) >= 0.6 {
        return Some(KnowledgeHit {
            category: Category::Personal,
            friendly_name: "个人媒体文件（根据内容推断）".to_string(),
            description: format!(
                "里面约 {}% 是视频/图片/音频，更像你的个人文件而不是缓存，请勿当垃圾清理。",
                pct(G_MEDIA)
            ),
            safety: Safety::Keep,
        });
    }
    if share(G_MEDIA) + share(G_DOCS) + share(G_ARCHIVE) >= 0.7 {
        return Some(KnowledgeHit {
            category: Category::Personal,
            friendly_name: "个人文件为主（根据内容推断）".to_string(),
            description: format!(
                "里面主要是媒体({}%)、文档({}%)和压缩包({}%)，更像个人数据，请自行整理而非直接清理。",
                pct(G_MEDIA),
                pct(G_DOCS),
                pct(G_ARCHIVE)
            ),
            safety: Safety::Keep,
        });
    }
    if share(G_PROGRAM) >= 0.6 {
        return Some(KnowledgeHit {
            category: Category::Software,
            friendly_name: "程序目录（根据内容推断）".to_string(),
            description: format!(
                "里面约 {}% 是程序文件，可能是某个软件的安装目录，请通过卸载程序移除而非手动删除。",
                pct(G_PROGRAM)
            ),
            safety: Safety::Keep,
        });
    }
    if share(G_CODE) >= 0.5 {
        return Some(KnowledgeHit {
            category: Category::Software,
            friendly_name: "代码工程（根据内容推断）".to_string(),
            description: format!(
                "里面约 {}% 是代码/工程文件，像是开发项目，请确认不再需要后再处理。",
                pct(G_CODE)
            ),
            safety: Safety::Keep,
        });
    }
    if share(G_CACHE) >= 0.7 {
        return Some(KnowledgeHit {
            category: Category::Cache,
            friendly_name: "疑似缓存（根据内容推断）".to_string(),
            description: format!(
                "里面约 {}% 是临时/日志类文件。这只是推测，清理前请先确认它属于哪个软件。",
                pct(G_CACHE)
            ),
            safety: Safety::Caution,
        });
    }
    None
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
    fn classify_dingtalk_local_versioned_dir() {
        // 目录名带版本号（DingTalk_133）也必须命中 appdata/local/dingtalk 规则
        assert_cat(r"C:\Users\x\AppData\Local\DingTalk_133", Category::Cache);
    }

    #[test]
    fn classify_wsl_is_systemfile_caution() {
        // WSL 虚拟磁盘含用户 Linux 环境，必须 Caution，不可被泛 appdata/local 规则归为可清
        let hit = classify(r"C:\Users\x\AppData\Local\wsl");
        assert_eq!(hit.category, Category::SystemFile);
        assert_eq!(hit.safety, Safety::Caution);
    }

    #[test]
    fn classify_adobe_media_cache_safe() {
        let hit = classify(r"C:\Users\x\AppData\Roaming\Adobe\Common\Media Cache Files");
        assert_eq!(hit.category, Category::Cache);
        assert_eq!(hit.safety, Safety::Safe);
    }

    #[test]
    fn classify_chinese_wechat_dir_on_data_drive() {
        // 用户自行迁移到 D 盘的中文命名微信目录必须识别为个人数据/请勿删除
        let hit = classify(r"D:\微信");
        assert_eq!(hit.category, Category::Personal);
        assert_eq!(hit.safety, Safety::Keep);
        let hit2 = classify(r"D:\WeChat");
        assert_eq!(hit2.category, Category::Personal);
        assert_eq!(hit2.safety, Safety::Keep);
    }

    #[test]
    fn classify_creation_and_chinese_personal_dirs() {
        // 创作类与中文个人目录：全部 Personal/Keep
        for p in [
            r"D:\JianyingPro Drafts",
            r"D:\MailMasterData",
            r"D:\备份",
            r"D:\旅行照片",
            r"E:\我的文档",
        ] {
            let hit = classify(p);
            assert_eq!(hit.category, Category::Personal, "path: {p}");
            assert_eq!(hit.safety, Safety::Keep, "path: {p}");
        }
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

    // ---------- 新增规则：下载目录不是缓存 ----------

    #[test]
    fn baidu_download_dir_is_personal_not_cache() {
        let hit = classify(r"C:\Users\x\BaiduNetdiskDownload");
        assert_eq!(hit.category, Category::Personal);
        assert_eq!(hit.safety, Safety::Keep);
    }

    #[test]
    fn baidu_app_data_is_caution_cache() {
        let hit = classify(r"C:\Users\x\AppData\Roaming\BaiduNetdisk");
        assert_eq!(hit.category, Category::Cache);
        assert_eq!(hit.safety, Safety::Caution);
    }

    #[test]
    fn wechat_files_is_personal() {
        assert_eq!(classify(r"D:\WeChat Files\wxid_123\FileStorage").category, Category::Personal);
        assert_eq!(classify(r"D:\Documents\xwechat_files").category, Category::Personal);
    }

    #[test]
    fn steamapps_is_software_keep() {
        let hit = classify(r"D:\Steam\steamapps\common\Game");
        assert_eq!(hit.category, Category::Software);
        assert_eq!(hit.safety, Safety::Keep);
    }

    #[test]
    fn minecraft_is_personal_keep() {
        let hit = classify(r"C:\Users\x\AppData\Roaming\.minecraft\saves");
        assert_eq!(hit.category, Category::Personal);
        assert_eq!(hit.safety, Safety::Keep);
    }

    #[test]
    fn conda_pkgs_is_caution() {
        assert_eq!(classify(r"C:\Users\x\miniconda3\pkgs").safety, Safety::Caution);
    }

    // ---------- 新增规则：friend-e 样本（WPS 边界 / 着色器缓存 / Keil 包） ----------

    #[test]
    fn wps_log_and_pdf_cache_are_safe() {
        let log = classify(r"C:\Users\x\AppData\Roaming\kingsoft\office6\log\wps.log");
        assert_eq!(log.friendly_name, "WPS 日志");
        assert_eq!(log.safety, Safety::Safe);
        let pdf = classify(r"C:\Users\x\AppData\Roaming\kingsoft\PDF\Cache\f.bin");
        assert_eq!(pdf.friendly_name, "WPS PDF 缓存");
        assert_eq!(pdf.safety, Safety::Safe);
    }

    #[test]
    fn kingsoft_local_caches_are_safe() {
        let up = classify(r"C:\Users\x\AppData\Local\Kingsoft\kupdateUI\cache\a.png");
        assert_eq!(up.friendly_name, "WPS 升级组件缓存");
        assert_eq!(up.safety, Safety::Safe);
        let of = classify(r"C:\Users\x\AppData\Local\Kingsoft\wpsoffice\cache\b.dat");
        assert_eq!(of.friendly_name, "WPS 程序缓存");
        assert_eq!(of.safety, Safety::Safe);
    }

    #[test]
    fn shadercache_generic_rule_hits_webview_apps() {
        // 豆包与元宝的非标准布局 ShaderCache 都应命中通用规则
        let db = classify(r"C:\Users\x\AppData\Local\Doubao\User Data\ShaderCache");
        assert_eq!(db.friendly_name, "着色器缓存");
        assert_eq!(db.safety, Safety::Safe);
        let yb = classify(r"C:\Users\x\AppData\Local\com.tencent.yuanbao\EBWebView\ShaderCache");
        assert_eq!(yb.friendly_name, "着色器缓存");
        // 浏览器自己的 ShaderCache 应先被更具体的浏览器规则命中，不落到通用规则
        let ch = classify(r"C:\Users\x\AppData\Local\Google\Chrome\User Data\ShaderCache");
        assert_ne!(ch.friendly_name, "着色器缓存");
    }

    #[test]
    fn pcsuite_is_personal_keep() {
        // 真机反例：vivo 办公套件的 pcsuite 目录藏 2GB 用户文档（云笔记同步），
        // 必须 Personal/Keep，绝不能当缓存/残留建议清理
        let hit = classify(r"C:\Users\x\AppData\Roaming\pcsuite\data\Note\Document\a.pdf");
        assert_eq!(hit.friendly_name, "vivo 办公套件数据 (pcsuite)");
        assert_eq!(hit.category, Category::Personal);
        assert_eq!(hit.safety, Safety::Keep);
    }

    #[test]
    fn keil_and_arm_packs_are_caution_software() {
        let keil = classify(r"C:\Users\x\AppData\Local\Keil_v5\ARM\PACK");
        assert_eq!(keil.category, Category::Software);
        assert_eq!(keil.safety, Safety::Caution);
        let arm = classify(r"C:\Users\x\AppData\Local\Arm\Packs\Keil\STM32F1xx_DFP");
        assert_eq!(arm.friendly_name, "Arm CMSIS 芯片支持包");
        assert_eq!(arm.safety, Safety::Caution);
        // needle 锚定在 arm/packs：不得误伤 Armoury 等 arm 前缀目录
        let armoury = classify(r"C:\Users\x\AppData\Local\ARMOURY CRATE Service");
        assert_ne!(armoury.friendly_name, "Arm CMSIS 芯片支持包");
    }

    // ---------- 内容启发式 ----------

    fn mb(n: u64) -> u64 {
        n * 1024 * 1024
    }

    #[test]
    fn profile_media_dominant_is_personal() {
        let mut p = [0u64; EXT_GROUP_COUNT];
        p[0] = mb(700); // media
        p[6] = mb(100);
        let hit = profile_classify(&p).unwrap();
        assert_eq!(hit.category, Category::Personal);
        assert_eq!(hit.safety, Safety::Keep);
    }

    #[test]
    fn profile_cacheish_is_caution_not_safe() {
        let mut p = [0u64; EXT_GROUP_COUNT];
        p[5] = mb(800); // cache-ish
        p[6] = mb(100);
        let hit = profile_classify(&p).unwrap();
        assert_eq!(hit.category, Category::Cache);
        // 推断出的缓存只能是 Caution，绝不能是 Safe
        assert_eq!(hit.safety, Safety::Caution);
    }

    #[test]
    fn profile_small_dir_returns_none() {
        let mut p = [0u64; EXT_GROUP_COUNT];
        p[0] = mb(10); // 不足 50MB 不猜
        assert!(profile_classify(&p).is_none());
    }

    #[test]
    fn profile_mixed_returns_none() {
        let mut p = [0u64; EXT_GROUP_COUNT];
        for i in 0..EXT_GROUP_COUNT {
            p[i] = mb(100); // 均匀混合，无法判断
        }
        assert!(profile_classify(&p).is_none());
    }

    #[test]
    fn ext_group_basic() {
        use std::path::Path;
        assert_eq!(ext_group(Path::new("a.mp4")), 0);
        assert_eq!(ext_group(Path::new("b.pdf")), 1);
        assert_eq!(ext_group(Path::new("c.zip")), 2);
        assert_eq!(ext_group(Path::new("d.exe")), 3);
        assert_eq!(ext_group(Path::new("e.rs")), 4);
        assert_eq!(ext_group(Path::new("f.tmp")), 5);
        assert_eq!(ext_group(Path::new("noext")), 6);
    }
}
