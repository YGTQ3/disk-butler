# 🔍 规则采集器使用说明

> 帮 C盘管家认识更多软件的缓存。跑一次约 1 分钟，生成一份**你可以逐行审查**的报告——
> 里面只有软件名、目录名和大小，**没有文件名、没有文件内容、没有你的用户名**。

## 两个版本，任选其一

| 版本 | 适合谁 | 特点 |
|---|---|---|
| ⭐ **DiskButlerCollector.exe**（推荐） | 所有人 | 绿色单文件约 0.3MB，**双击即用**，全中文界面永不乱码，速度更快 |
| collect-rules.ps1 + run-collector.bat | 只信明文脚本的朋友 | 每一行代码都能看见，两者报告格式完全一致 |

两个版本采集的内容、隐私规则、报告格式完全相同，都**不清理、不删除、不修改任何东西，也不联网**。

## 它是干什么的？

C盘管家的清理白名单和磁盘透视知识库都靠"规则"驱动——但每台电脑装的软件不一样。
这个脚本扫描你机器上的**缓存分布线索**，生成报告；我们按照
[清理规则 SOP](../docs/RULES-CLEANUP.md) 人工逐条评估后，把安全的条目变成正式规则，
让所有用户受益。

**它不清理、不删除、不修改任何东西，也不联网**——只在你桌面上写两个报告文件。

## 方式一（推荐）：双击 exe，两步完事

1. 从 [Releases](https://github.com/YGTQ3/disk-butler/releases) 下载 `DiskButlerCollector.exe`，双击运行（不需要管理员权限）；
   窗口是全中文：输入 `1`（基础模式，直接回车也行）或 `2`（完整模式）；
2. **审查后发送**：桌面上生成 `diskbutler-rule-report-时间戳.md`，
   用记事本打开**自己先看一遍**，确认没有你不想分享的内容，再发到邮箱：**ygtq1021@126.com**。

命令行用法（可选）：`DiskButlerCollector.exe --basic`（基础）/ `--full`（完整），不弹菜单直接跑。

> exe 由仓库内 `tools/collector/` 的 Rust 源码编译，开源可审计；自己编译：`cargo build --release`。

## 方式二：明文脚本版（三步）

1. **下载两个文件**到同一个文件夹（打开链接 → 点 `Raw` → `Ctrl+S` 保存）：
   - [collect-rules.ps1](https://github.com/YGTQ3/disk-butler/blob/main/tools/collect-rules.ps1)（采集脚本本体）
   - [run-collector.bat](https://github.com/YGTQ3/disk-butler/blob/main/tools/run-collector.bat)（双击启动器）

2. **双击 `run-collector.bat`**，按提示选择模式（直接回车 = 基础模式），**不需要管理员权限**；
   （为什么不直接双击 .ps1？Windows 默认双击 .ps1 是用记事本打开，不会运行）

   > 黑窗口里是简单英文（不同电脑的中文编码不统一，英文才能保证人人不乱码），对照翻译：
   > - `Safe: NO delete, NO change, NO network.` → 安全：不删除、不修改、不联网
   > - `[1] Basic scan` → 基础扫描（约 1 分钟，推荐）
   > - `[2] Full scan` → 完整扫描（5~10 分钟，会列出 D 盘等处的大文件夹名）
   > - `Type 1 or 2, then press Enter` → 输入 1 或 2 后按回车（直接回车 = 选 1）
   > - `Done! Report is on your Desktop` → 完成！报告在桌面上

3. **审查后发送**：桌面上会生成 `diskbutler-rule-report-时间戳.md`，
   用记事本打开**自己先看一遍**，确认没有你不想分享的内容，再发到邮箱：**ygtq1021@126.com**。

## 可选：全盘线索模式（完整模式）

exe 版在菜单里输入 `2`；ps1 版双击 `run-collector.bat` 后输入 `2`，或命令行直接运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\collect-rules.ps1 -IncludeDrives
```

额外扫描各磁盘根目录下**两级、大于 1GB** 的目录名和大小（用于识别"硬盘刺客"类大目录，
喂给磁盘透视知识库）。⚠ 注意：这会让报告包含你数据盘的顶层文件夹名
（例如 `D:\我的项目`），**隐私敏感请不要开这个开关**，默认是关闭的。

## 报告里有什么 / 没有什么

| ✅ 有 | ❌ 没有 |
|---|---|
| 已安装软件清单（名称+大小） | 任何文件名 |
| AppData/ProgramData 一级目录名+大小 | 任何文件内容 |
| 缓存特征目录命中（Cache/GPUCache 等） | 你的用户名（已替换为 `%USERPROFILE%`） |
| 包管理器缓存大小（npm/pip/conda 等） | 计算机名 |
| （可选）盘根两级 ≥1GB 目录 | 浏览记录、账号、聊天数据 |

## 收到报告的人怎么处理？

把 `.md` 或 `.json` 交给 AI（或人工），按仓库内两份 SOP 执行：

- [docs/RULES-CLEANUP.md](../docs/RULES-CLEANUP.md) —— 评估哪些缓存可以进清理白名单
  （五重自检 + 永久黑名单 + 三字段判定 + 单测验收）；
- [docs/RULES-KNOWLEDGE.md](../docs/RULES-KNOWLEDGE.md) —— 大目录身份进磁盘透视知识库。

一句话指令即可：**"这是一份 diskbutler-rule-report，按 RULES-CLEANUP.md 逐条评估，产出可入库的规则。"**
