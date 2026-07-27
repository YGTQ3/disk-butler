# 🔍 规则采集器（collect-rules.ps1）使用说明

> 帮 C盘管家认识更多软件的缓存。跑一次约 1 分钟，生成一份**你可以逐行审查**的报告——
> 里面只有软件名、目录名和大小，**没有文件名、没有文件内容、没有你的用户名**。

## 它是干什么的？

C盘管家的清理白名单和磁盘透视知识库都靠"规则"驱动——但每台电脑装的软件不一样。
这个脚本扫描你机器上的**缓存分布线索**，生成报告；我们按照
[清理规则 SOP](../docs/RULES-CLEANUP.md) 人工逐条评估后，把安全的条目变成正式规则，
让所有用户受益。

**它不清理、不删除、不修改任何东西，也不联网**——只在你桌面上写两个报告文件。

## 怎么用（三步，双击即可）

1. **下载两个文件**到同一个文件夹（打开链接 → 点 `Raw` → `Ctrl+S` 保存）：
   - [collect-rules.ps1](https://github.com/YGTQ3/disk-butler/blob/main/tools/collect-rules.ps1)（采集脚本本体）
   - [run-collector.bat](https://github.com/YGTQ3/disk-butler/blob/main/tools/run-collector.bat)（双击启动器）

2. **双击 `run-collector.bat`**，按提示选择模式（直接回车 = 基础模式），**不需要管理员权限**；
   （为什么不直接双击 .ps1？Windows 默认双击 .ps1 是用记事本打开，不会运行）

3. **审查后发送**：桌面上会生成 `diskbutler-rule-report-时间戳.md`，
   用记事本打开**自己先看一遍**，确认没有你不想分享的内容，再发给收集人。

## 可选：全盘线索模式

双击 `run-collector.bat` 后在菜单里输入 `2`（完整模式）；或命令行直接运行：

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
