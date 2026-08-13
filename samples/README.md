# 📥 规则采集样本收件箱（samples/）

> 存放朋友/贡献者发回的 `diskbutler-rule-report-*.md/.json` 采集报告。
> ⚠ **本目录内除本 README 外的一切文件都被 .gitignore 排除，永不提交**——
> 报告含贡献者机器的软件清单和目录名，属于隐私数据，只留在本地。

## 归档命名规范

收到报告后重命名为：`<来源代号>-<机器类型>-<日期>.md/.json`

- 来源代号：自定，如 `self`、`friend-a`、`friend-b`（不要用真名）
- 机器类型：`dev`（开发机）/ `office`（办公机）/ `game`（游戏机）/ `design`（设计机）/ `home`（家用机）
- 例：`friend-a-game-20260728.md`

## 处理状态登记（每处理一份就更新）

| 样本 | 收到日期 | 模式 | 评估状态 | 产出 |
|---|---|---|---|---|
| self-dev-20260727 | 2026-07-27 | 完整 | ✅ 已评估入库 | +3 规则（剪映/Playwright/钉钉），拒 2（baidu/Package Cache） |
| friend-a-workstation-20260727 | 2026-07-27 | 基础 | ✅ 已评估入库 | +4 清理规则（VS Code/uv/Adobe媒体缓存/WPS）+5 知识库规则（含 WSL 保护）；拒：Tencent系/Postman/Blackmagic/douyin等（见下方观察名单） |
| diskbutler-rule-report-20260728-1640（friend-b 电商办公机） | 2026-07-28 | 完整 | ✅ 已评估入库 | +browser-cache 扩充 360 系三浏览器 +wps-old-versions（注册表锚点）；拒 35 项（评估报告见 .verify\OpenCode实验\）；衍生产出：electron-cache 指纹规则、VERSION-SIBLINGS 探测、孤儿页面文件检测（D盘11GB实战闭环，侦察记录 wps/pagefile-recon-result.txt 留档本目录） |
| diskbutler-rule-report-20260728-2005（friend-c 游戏家用机 Win11 24H2） | 2026-07-28 | 完整 | ✅ 已评估入库 | +browser-cache 扩充 CentBrowser/Quark +知识库 .minecraft 存档保护；修复采集器 PERSONAL 误报（精确匹配目录名，不再误伤 ai.opencode.desktop 等包名）；QQ 边界见观察名单 |
| diskbutler-rule-report-20260729-1054（friend-d 教育+AI工作机 Win11 Pro 25H2） | 2026-07-29 | 基础（软件内贡献 collector=app） | 🔍 已登记待评估 | 画像：学而思/ClassIn/GeoGebra/LyX/讯飞E听说 + 重度 AI 工具（LM Studio/Chatbox/ollama/通义/千问/秘塔/爱问云）+ 开发全家桶；装火绒。现有规则可覆盖：Quark 缓存(2.8G)/Playwright(683M)/electron-cache(通义/Chatbox/ollama 等)/wps-cache(kingsoft 2.5G)。新线索见观察名单 |
| diskbutler-rule-report-20260730-1517（friend-e 电气/嵌入式工程工作站 Win10 Pro 22H2） | 2026-07-30 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | +wps-cache 扩充 4 路径（office6\log、PDF\Cache、Kingsoft\kupdateUI\cache、wpsoffice\cache，双样本佐证）+doubao-shadercache（存在才收录）；知识库 +7（WPS 边界 4 条、/shadercache 通用、Keil_v5/Arm Packs 二条 Software/Caution）；拒：Keil/Arm 入清理白名单（芯片包删后不自动重建，自检 #2 不过）、元宝 ShaderCache（Tencent 系红线不开例外，知识库通用规则覆盖解释）。Kingsoft VERSION-SIBLINGS 经核为 wps-old-versions 已覆盖。画像与永拒名单验证见观察名单 |
| diskbutler-rule-report-20260801-1259（friend-f 学术/工程工作站 Win11 Home 23H2） | 2026-08-01 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | +islide-logs(2G)/miktex-cache/originlab-temp/teamviewer-logs/game-logs（Civ VI 等 7 款游戏日志）+gpu-cache 扩充 Steam ShaderCache(311M)；知识库 +13（iSlide/MiKTeX/OriginLab/TeamViewer/6 家游戏厂商 + Zotero/MathWorks Personal/Keep + com.adobe.dunamis Software/Keep）；拒 10 项（Package Cache×3/Tencent/MathWorks/Zotero/Downloaded Installations/com.adobe.dunamis/Python/本软件）。子智能体独立验证发现 2 处已覆盖判断错误（CEF/Steam ShaderCache），已修正。画像与观察名单见下方 |
| disk-butler-webview2-report（用户机器 Win10 22H2 家庭版 x64） | 2026-08-03 | 诊断报告（`tools/webview2-diagnose.bat` 输出，非规则采集） | ✅ 已分析（根因：见下方「webview2 安装失败样本」） | 结论：仅 x86 老版 WebView2 Runtime 113.0.1774.35（随 32 位 Edge 113 附带、EdgeUpdate 停更），缺 x64 组件；Tauri 安装器按注册表 pv 判定"已装"而跳过自动安装 → x64 应用运行时初始化失败。建议：用户装 Evergreen x64 Runtime；诊断脚本 x64 判据修正（查 `Application\<ver>\EBWebView\x64` 子目录，勿查 `Program Files\Microsoft\EdgeWebView`） |
| diskbutler-rule-report-20260804-1538（造价/石油办公+设计+游戏混合机 Win10 Pro 22H2） | 2026-08-05 | 完整 | ✅ 已评估入库 | 合法缓存类覆盖率≈100%（uv/pip/temp/CrashDumps/Steam ShaderCache/Doubao ShaderCache/WPS/electron 指纹/browser/adobe 全部命中）。大额剩余均为红线（Apple MobileSync 备份 14.8G/baidu 网盘 8G/Tencent 8.4G/Desktop\.accelerate 3.8G）或 ProgramData 边界外（Topaz 14.9G/NVIDIA App 2.9G/LGHUB 657M）。**知识库 +5 已实施**：`/.lingma`（Cache/Caution，含 Qoder CN 改名实证）、`topaz labs`、`nvidia app`、`lghub`（ProgramData 三条，只解释不动手）、`windows/memory.dmp`（SystemFile/Keep）；清理候选 `apple-logs` 评估后**弃**（收益未证实，14.8G 大头是 MobileSync 备份清不到）；观察名单新增 .accelerate、SAP/Listary/EA 等（见下） |
| diskbutler-rule-report-20260806-1735（游戏/家用机 Win10 Pro 22H2，C盘剩5.8G/D盘剩1.5G） | 2026-08-06 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **清理 +1**：onedrive-logs（`Microsoft\OneDrive\logs`，safe/junk，微软官方排障步骤佐证——补齐"知识库 onedrivetemp 说可清但白名单无对应"的断层）；**知识库 +1**：renpy（Personal/Keep，galgame 存档保护，.minecraft 同型）；**覆盖验证**：AMD RadeonSoftware cache(59M) 已由 gpu-cache 覆盖、QQ(591M)/QQEX(587M)/TapTap(169M)/kuaijiasu(79M) 三件套已由 electron-cache 收编、WPS(1.7G) 已覆盖、Roaming 大头全红线正确拦截（baidu 4.5G/Tencent 3.4G/C:\QQ文件 3.2G/D:\xwechat_files 12.4G/Desktop\Telegram 2.1G）；观察名单 +7（见下） |
| diskbutler-rule-report-20260807-1547（游戏机 Win11 Pro 24H2） | 2026-08-08 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **gpu-cache 扩充 +2**：`AMD\AMDRSSrcExt\cache`、`AMD\PPC\temp`（AMD 14.6GB 大头未覆盖部分，Radeon 设置组件缓存+遥测待传暂存，可清自动重建；PPC 删除顺带停遥测上传）；**知识库 +3**：amdrssrcext/cache（Cache/Safe）、amd/ppc（Cache/Safe，CVE-2020-8950 文献佐证 PPC=AMD 用户体验计划遥测暂存）、programdata/ul/3dmark（Caution，只解释不动手——3DMark 跑分临时 12.6GB，topaz 同型）；**覆盖验证**：3DMark tmp/网易云/Steam ShaderCache 命中；观察名单 +5（见下） |
| diskbutler-rule-report-20260807-1708（无人机测绘机 Win11 Pro 25H2） | 2026-08-08 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | 大头覆盖验证：Packages(1.5G)/Programs 红线正确放行，kingsoft(kwallpaper Cache)、Doubao ShaderCache、Steam 三件套已覆盖；新增观察：**Mission Planner (1426M ProgramData，飞控地面站飞行日志=个人数据)**、SPChrome/CapableWin/winToolBox/MyChrome/app_shell_cache 等浏览器类（见观察名单）；无新入库（单样本无人机测绘垂直机型，需取证） |
| diskbutler-rule-report-20260807-1719（监控+开发机 Win10 Edu 22H2） | 2026-08-08 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | 覆盖验证力度强：大量 `*-updater` 规则命中（scnet/quark/apifox/chatglm/123pan/xmind/qq-chat/ee/fiddler 9 个更新器）、electron-cache 收编 Cursor/Code/npm；新增观察：**微信开发者工具 (835M，含用户小程序项目=红线不可动)**、GitHubDesktop (1.75G，electron-cache 指纹需校验)、DingTalk APPDATA/ChatGLM/123pan 客户端数据、ShadowBot (172M 影刀 RPA)、QianniuTemp (537M 千牛 Temp)（见观察名单）；无新增必入库 |
| diskbutler-rule-report-20260809-1258（游戏机 Win11 Pro 25H2） | 2026-08-10 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | 覆盖验证良好：AMD RadeonSoftware\cache 命中 gpu-cache、Steam ShaderCache 命中、WPS(kingsoft) 命中 wps-cache、leigod/Battle.net 命中 electron-cache、红线正确拦截（Microsoft TokenBroker/Packages/Tencent）；无新入库（机器空间充裕 C 盘 263G/300G，全部小额缓存已由现有规则覆盖） |
| diskbutler-rule-report-20260809-2354（游戏机 Win11 Pro 25H2，C 盘剩 250G/447G） | 2026-08-10 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **覆盖验证**：Steam ShaderCache/QuarkCloudDrive ShaderCache/QuarkCloudDriveUpdater 命中、electron-cache 收编 QQEX/QQ/heybox-pc-launcher/heybox-chat-electron/ACLOS、wps-cache 命中 kingsoft、onedrive-logs 命中、红线正确拦截（Tencent 2.5G/Packages/QQ/QQEX）；**新增观察**：**CapCut (2.2G，User Data\Cache+Log，非标准 Electron 指纹不匹配)**、oopz(1.1G 身份不明)/KOOK(883M 游戏语音)/PixPin(415M 截图工具 Crashpad+Temp)/Shandianshuo(895M Roaming logs)（见观察名单）；**知识库已加**：无（均为观察项） |
| diskbutler-rule-report-20260810-1149（开发机 Win11 Pro 25H2，C 盘剩 157G/312G） | 2026-08-10 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **清理 +1**：cargo-cache（`~/.cargo/registry`，safe——补齐 cargo 在清理白名单的空白）；**知识库 +2**：godot（Personal/Keep，游戏引擎含用户项目）、cocos（Software/Caution，ProgramData 5.6G 引擎共享数据）；**覆盖验证**：AndroidStudio2024.3 log+tmp 命中 androidstudio-logs、DingTalk_133 命中 dingtalk-cache、electron-cache 收编 CodeBuddy CN/QoderCN/Cursor/Code/CherryStudio/pc-link-app/r2modman 等 10+ 应用、Steam ShaderCache 命中、cargo 469M 新入清理白名单、红线正确拦截（微信开发者工具 1.3G/Packages 1.85G/Tencent 3.3G）；128 测试全绿；新增观察：BCUT(225M)/AzureFunctionsTools(917M VERSION-SIBLINGS)/node-gyp(59M)/Yodao(456M)（见观察名单） |
| diskbutler-rule-report-20260809-0955（游戏+开发混合机 Win11 Home China 25H2，C 盘剩 71G/453G） | 2026-08-10 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **规则成熟度验证样本**——覆盖率≈100%，无新入库。electron-cache 一网打尽 TRAE SOLO CN(9.7G)/kimi-desktop(950M)/Cindy(723M)/Coze/Cursor/maskfog/qq_guild/FeelFish 等 10+ Electron 应用；jetbrains-cache 7.7G、wps-cache 3.5G、hf-cache 7.98G(caution) 全部命中；红线正确拦截（Tencent 6.7G/Packages 1.8G/baidu 1.8G）；conda pkgs 2.95G 触发永久黑名单（硬链接）；oopz/node-gyp 获第二样本佐证（见观察名单） |
| diskbutler-rule-report-20260811-0823（政府环资部门工作站 Win11 Pro 25H2，6 盘位，C 盘剩 34.5G/112G） | 2026-08-11 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **覆盖率≈95%**，无新入库。electron-cache 收编 QClaw/CodeBuddy/SodaMusic/TapTap/miHoYo 等 10+ 应用、wps-cache/dingtalk-cache/Steam ShaderCache/doubao-shadercache/onedrive-logs 全部命中（注：原记录的“huorong-logs 命中”系笔误——当时并无此规则，火绒日志属 ProgramData 边界只解释不动手，已补知识库条目）；红线正确拦截（Tencent 4.3G/Packages）。新观察：WinSxS\Temp\InFlight 5.76G（系统级不可自动清理）、.NET 预览版/RC 残留 ~800M（开发者机器）、QuarkCloudDrive 三版本 VERSION-SIBLINGS（D 盘非标准位置） |
| diskbutler-rule-report-20260811-1001（GIS/环资专业工作站 Win11 Pro 25H2，C 盘剩 722G/930G） | 2026-08-11 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **覆盖率≈90%，NeatDM 第四份佐证**。neatdm-cache 命中 1.3G（第四台机器确认）、electron-cache 收编 Code/QQEX/yuque/bilibili 等、pip cache/ESRI Staging 命中；红线正确拦截（Tencent 2.4G/baidu 2.2G/anaconda3 永久黑名单）。空间极充裕无清理压力。新观察：MathWorks ServiceHost\logs 第二佐证（1.24G）、OSGeo4W 2.99G（GIS 专业数据红线）、LocaSpace/TuxinEarth（国产 GIS 工具） |
| diskbutler-rule-report-20260811-1221（视频剪辑/AI漫剧制作工作站 **Win10 LTSC 2021**，5 盘位，C 盘剩 220G/300G） | 2026-08-11 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **覆盖率≈92%，无新入库**（候选均单样本）。jianying-cache 命中 977M（剪映第三台机器）、electron-cache 收编 Trae CN/Antigravity/OpenCode/obsidian/LolAICoach/Hanako、Twinkstar/npm-cache/nuget-cache/wps-cache 全命中；红线正确拦截（Tencent 3.2G/**Blackmagic 4.5G 永久拒绝第二佐证**/Package Cache 937M）。首份 LTSC 样本。画像：Adobe 2018+2022 便携版×12/DaVinci 双版本/AI 漫剧工具集 126G/虚拟机 470G/系统备份 421G。新观察：Buzz 8.2G/pypoetry/NuGet 第二路径/QuarkCloudDrive 多版本第二佐证/RunningCheeseChrome/Red Giant Logs（见下） |
| diskbutler-rule-report-20260811-1644（轻量家用机 Win11 Home China 23H2，C 盘剩 136.9G/200G） | 2026-08-11 | 基础（软件内贡献 collector=app，driveTopDirs 空） | ✅ 已评估入库 | **覆盖率≈95%，无新入库**。wps-cache 命中 kingsoft 3.08G、electron-cache 收编 QQEX/QQ/qq_guild/subos/lx-music-desktop、pip 命中；红线正确拦截（Tencent 4.9G/Packages/Programs/Package Cache×2/TokenBroker）。新观察：顶层 EBWebView 257M（归属不明）、Intel Package Cache {GUID} 变体 274M（白名单架构天然守住）、NBTool 298M 身份不明、DigiDNA(iMazing) 233M 备份红线、GameViewer 第三佐证（见下） |
| diskbutler-rule-report-20260811-2256（重度游戏机 Win11 Pro **26H2**，4 盘位，C 盘剩 327.9G/507G） | 2026-08-12 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **知识库 +1**：windows.old（System/Caution，排在所有 windows/* 规则之前防抢先，2256 样本 39.25G 升级残留实证，只解释不动手——官方 10 天自动删/磁盘清理可加速）。jianying-cache 命中 7.07G（**第五台机器彻底坐实**）、gpu-cache/Steam ShaderCache/wps-cache/quark-cloud-drive/electron-cache 全命中；BCUT/Battle.net/PixPin/Nutstore/Listary 均获第二佐证（见观察名单）。新观察：Ashampoo Driver Updater 3.28G、TslGame(PUBG) Saved\Logs 576M、O+Connect 562M、GreenCore 双目录 675M 身份不明 |
| diskbutler-rule-report-20260812-0832（柳工企业办公机 Win10 Pro 1909，**C 盘仅剩 11.3G/102.4G** 迄今最强清理压力） | 2026-08-12 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **清理 +1**：sogoupdf-logs（双佐证 123M→1G，纯日志 safe/junk）；**知识库 +3**：larkshell（飞书数据 Personal/Keep，5.6G 实证）、sogoupdf/log（Cache/Safe）、huorong/sysdiag/log（Cache/Caution，三佐证 687M 但 ProgramData 边界只解释）。覆盖验证强度极高：updaters 通用规则一网 10 个更新器(~2.2G)、jianying-cache 5.94G 第五台、wps-cache 5.45G、islide-logs 1.29G 第二台、electron-cache 收编 13 应用、onedrive-logs/doubao-shadercache/pip 全命中；ShadowBot/KuGou8/cdsyy/ichat 均获第二佐证（见观察名单）；红线正确拦截（Tencent 4G/baidu 2G/WeChat Files 39.8G/DreamMail6 29.9G）；131 测试全绿 |
| diskbutler-rule-report-20260812-0951（海外轨道交通投标工作机 Win11 Home 25H2，C 盘剩 54G/145G） | 2026-08-12 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **知识库 +1**：windows/livekernelreports（SystemFile/Caution，1.54G DripsWatchdog 转储实证，MEMORY.DMP 同型只解释）。覆盖验证亮点：**pcsuite 知识条目第二样本命中（11.66G，vivo 办公套件云笔记文档 Personal/Keep 保护链验证成功）**、islide-logs 第三台机器 887M、wps-cache 7.56G、electron-cache 收编 TDAppDesktop/WorkBuddy/CherryStudio/Coze/QQEX/QQ/qq_guild/BdTranslateClient 等、doubao-shadercache/npm-cache/Steam ShaderCache/onedrive-logs 全命中；红线正确拦截（Tencent 4.34G/Packages/xwechat 21G/WeChat缓存文件 15.75G/Foxmail Storage/WPS云盘/投标文件群）；132 测试全绿。新观察：QianwenUpdater 双佐证、wpsphoto+\cache、美图/ima.copilot/PCAppStore/Google 5.13G 身份待查（见下） |
| diskbutler-rule-report-20260812-2257（服装电商/短视频运营机 Win11 Home China 25H2，双用户，C 盘剩 58.4G/300G） | 2026-08-13 | 完整 | ✅ 已评估入库 | jianying-cache 命中 9.05G（**第七台机器**）、sogoupdf 未装未现、doubao-shadercache 命中 6.16G 大头、electron-cache 收编 yuque-desktop 4.27G/uTools/QQ/z-library 等、wps-cache 含 pdf 全家桶命中、larkshell 知识条目第二样本（Roaming 901M）、gpu-cache(AMD) 命中；**wpsphoto+\cache 获第二佐证→已扩充入 wps-cache**、**美图获第二佐证→meitu-cache 已入库**；红线正确拦截（Tencent 3.67G/Baidu 1.68G/Packages/xwechat 15.2G/第二用户目录 20G/Topaz models 21.5G 只解释）。新观察：Photoshop Temp×4 共 22G(D盘盘根异常退出残留)、夸克 videoCache 自定义路径 23G、指纹浏览器系、ima.copilot 第二佐证 2.16G（见下）；134 测试全绿 |
| diskbutler-rule-report-20260813-0037（开发+游戏混合机，**C 盘仅剩 24.3G/149G** 强压力） | 2026-08-13 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **game-logs 扩充 +2**：TslGame(PUBG)/NRC 的 Saved\Logs（均双样本佐证）；**sogoupdf-logs 新规则第三台机器命中 1.2G**（刚入库即验证）。jianying-cache 8.76G、jetbrains-cache/Playwright/npm/uv/yarn/wsl 保护条目全命中、electron-cache 收编 Code/QQ/QQEX/ACLOS/CindyGlobal/Trae CN/Xmind/leigod 等 15+ 应用、pcsuite 第三样本保护链持续生效；红线正确拦截（Tencent 7.17G/Package Cache 1.5G）；NetEase CloudMusic 589M/DeadByDaylight/Back4Blood/Labcenter(Proteus) 628M 等入观察（见下） |
| diskbutler-rule-report-20260813-0211（华硕 ROG 游戏机 Win10 Pro 22H2，C 盘剩 250G/456G） | 2026-08-13 | 基础（软件内贡献 collector=app，driveTopDirs 空） | ✅ 已评估入库 | BG3 重度玩家机：.minecraft 3.19G 保护条目命中、Paradox launcher-v2 命中 game-logs、cherrystudio-updater 命中 updaters、Steam ShaderCache/crash-reports 命中；红线正确拦截（Tencent 1.95G/Packages/Package Cache/NVIDIA 系）。新观察：**Larian Studios 6.53G（Launcher\Cache+Logs，博德之门3 启动器，体积大单样本）**、bililive 1.7G、app_shell_cache_6383 第二佐证、QuarkUpdater 无连字符漏网、Roaming\Quark\Cache 第二佐证（见下） |
| diskbutler-rule-report-20260812-1612（水利工程造价/监理工作站 Win10 Pro **1809**，5 盘位，C 盘剩 29.1G/111.8G，MD 格式报告） | 2026-08-13 | 完整（MD 格式） | ✅ 已评估入库 | **清理 +1**：wondershare-logs（三样本佐证 0037/2257/1612，扫描 PDFelement*/PDFThumbnail 前缀的 Log+Temp，版本目录名可变）。jianying-cache 命中 318M（**第八台机器**）、electron-cache 收编 QQEX/QQ/CindyGlobal/qq_guild/lx-music/aDrive/kekedy.tv/douyin-downloader 等 10+、wps-cache/Autodesk 日志特征/QuarkCloudDriveUpdater 全命中；**QuarkCloudDrive 多版本第三佐证**（D 盘 3 版本 3.61G）、**夸克 videoCache 自定义路径第二佐证**（4.13G）；红线正确拦截（Tencent 4.3G/WeChat Files 56.9G+xwechat 31.4G/枞阳水利工程资料群/Switch 游戏资源）。画像亮点：广联达造价+鸿合幼教双用途机、竞对“磨针C盘清理”同机。新观察：随机名目录疑似 PUP、DeliveryOptimization 迁 D 盘（见下） |
| diskbutler-rule-report-20260812-2120（手机刷机/硬件发烧友+开发机 Win11 Pro 25H2，C 盘剩 28G/165G，MD 格式报告） | 2026-08-13 | 完整（MD 格式） | ✅ 已评估入库 | **知识库 +1**：huorong/sysdiag/quarantine（SystemFile/Keep，火绒病毒隔离区 2.17G 实证，防用户误删）。**入库规则即时验证**：TslGame Saved\Logs 刚入 game-logs 即第三佐证命中；gpu-cache(AMD 4.28G)/doubao-shadercache(2.34G)/jetbrains-cache/Steam ShaderCache/wps-cache/.minecraft 保护全命中；红线正确拦截（Tencent 3.26G/Packages/火绒隔离区/WinSxS 17.3G）。新观察：KwaiLive 1.72G、Telegram tdata dumps/temp、CamScanner 738M、app_shell_cache_6383 第三佐证、O+Connect/QuarkUpdater 第二佐证（见下） |

## 观察名单（见过但未入库，等更多样本佐证）- ~~Electron 应用通用 Cache 模式~~ ✅ 已于 2026-07-28 以 electron-cache 指纹规则统一收编（Cache+Code Cache/GPUCache 同级并存才认定；Tencent 系仍排除）；
- **QQ 缓存边界**（friend-c 样本）：QQ 顶层目录名为 `QQ`（非 Tencent），会被 electron-cache 指纹命中，但只取 Chromium 标准缓存三件套，QQ 聊天数据（nt_qq/Documents 树）不在其中——判定为安全，维持现状不排除；如后续样本发现 QQ 把用户数据混入 Cache 同级，再评估加排除；
- QianwenUpdater：不带连字符后缀，现有 *-updater 规则漏网（双佐证：48M → 89.7M，均带 UPDATER-RESIDUE 标记）。不扩充为 ends_with("updater") 的原因：会误伤 StandaloneUpdater 等未证实可删目录，维持观察；
- **学而思网校缓存**（friend-d）：`%LOCALAPPDATA%\XesCache_literacy`(493M)、`XesCache_subject`(172M)、`XesStudent_*`(含 cache 子目录)——命名含 Cache、清晰是缓存；但教育软件专用、单机样本，等更多教育机佐证再评估是否入库；
- **GeoGebra 升级残留**（friend-d）：`%LOCALAPPDATA%\GeoGebra_CAS`(774M) 打了 VERSION-SIBLINGS（多版本并存）——可套 wps-old-versions 的"注册表安装版本锚点"思路做残留清理候选，但需确认 GeoGebra 版本目录命名规律；
- ~~**WPS 缓存边界扩充**~~ ✅ 已于 2026-07-30 双样本佐证后入库：wps-cache 扩充 office6\log、PDF\Cache、Kingsoft\kupdateUI\cache、wpsoffice\cache（存在才收录）；Kingsoft VERSION-SIBLINGS 经核实为 WPS Office 版本目录并存，wps-old-versions 已覆盖，无需新规则；
- **Keil/Arm 包缓存**（friend-e）：清理白名单已评估**拒绝**（CMSIS 芯片包删后不自动重建，需 Pack Installer 手动重装，自检 #2 不过）；已入知识库 Software/Caution 解释。剩余待办：若未来完整模式样本证实 `Arm\Packs\.Download`（安装后残留的 .pack 源包）独立存在，可单独评估该子目录为 junk；
- ~~**豆包/元宝 WebView 缓存**~~ ✅ 部分入库（2026-07-30）：豆包 ShaderCache 入清理白名单（doubao-shadercache，存在才收录）；元宝 EBWebView\ShaderCache 技术上安全但 **Tencent 系永拒红线不开例外**，仅由知识库 `/shadercache` 通用规则解释为可安全清理；两者的 Crashpad 目录未处理，暂缓；
- **工业软件 ProgramData 数据**（friend-e）：EPLAN(2.5G)/CODESYS(1.9G)/Altium(639M)/SOLIDWORKS Electrical(617M)——大概率含元件库/工程数据，默认不动，仅记录画像；HDDog(880M) 身份待查明后再议。
- **随机名目录疑似广告软件**（friend-f）：`%LOCALAPPDATA%\r5ym9vr4`(19M)、`qkoimv2w`(19M)、`3y4y5ygn`(11M)、`acvt1dv0`(11M)、`z5ds4udz`(11M)——共 ~71MB。全部带 `User Data\Crashpad + ShaderCache` 结构，8 字符随机字母数字命名，配合"鼠大侠"等可疑软件，高度疑似广告软件/PUP 残留。单机样本，等更多佐证再决定是否入孤儿检测；
- **华为电脑管家 (PCManager) 数据**（friend-f）：`%ProgramData%\Comms` (1121M) + `%LocalAppData%\PCManager` (25M)。预装推广软件的大量缓存/日志，技术上可清理但属 OEM 软件边界，暂记录画像。
- **`.accelerate` 身份待查**（20260804 样本，用户机）：`%USERPROFILE%\Desktop\.accelerate`(3.82G) + E 盘各学习/视频文件夹 `.accelerate`(合计 ~7.8G)。Desktop 红线 + 身份不明，疑似某下载/视频加速器缓存。已建议用户自查是什么软件生成，确认后再评估；
- **`.lingma`（通义灵码/Qoder CN 数据目录）**（20260804 样本 + 用户机实测）：⚠️ **不能入清理白名单**。关键事实：① 改名 Qoder CN 只是品牌升级，**官方 FAQ 明确"相关路径和进程名仍以 `.lingma`/`Lingma.exe` 为准"**——数据目录没变，VSC 插件虽 Deprecated 但"现有用户仍可继续使用"，样本机 1.95GB 很可能是活跃索引数据而非残留；② 内含登录令牌（machine_token.json）+ AI 会话历史，官方排障步骤"删 .lingma → 重启 IDE 重新登录"证实删除有代价（自检 #2/#5 不过）；③ 位置漂移：VSC 插件形态 `%USERPROFILE%\.lingma` vs Qoder IDE 形态 `%LOCALAPPDATA%\.lingma`，单规则覆盖不全。CSDN 实证大头为 `index\meta\v4\index.db` + `.dbc` 代码索引（案例 10G~100G），1.95GB ≈ 重度使用痕迹。**用户本机** `%LOCALAPPDATA%\.lingma` 仅 21MB 且 3 个月零写入（2026-04-25 后），确为废弃孤儿；**但用户决定暂不删除**（JetBrains/VSC 插件形态仍以 `.lingma`/`Lingma.exe` 为准，删了需重登，留着无害）。替代方案：知识库 +1（Cache/Caution 解释"代码索引+缓存+聊天记录，删除需重新登录/重建索引"，已实施 ✓）；
- **SAP GUI 缓存/Listary 缓存/EA AC 缓存/Anki2 logs/Nutstore logs/CNPC 会议日志**（20260804 样本）：均单样本小项（88M/321M 目录内/865M 目录内/1.2G 目录内/272M 目录内/148M 目录内），命名像缓存但需更多样本佐证，暂观察；
- **Doubao Crashpad**（20260804 样本第二次出现）：`%LOCALAPPDATA%\Doubao\User Data\Crashpad`，维持"暂缓"决定（Crashpad 通常几十 MB 量级，第三样本若超 200M 再评估）；
- **Roaming\Quark\Cache**(48M)：无 Chromium 三件套指纹，browser-cache 未覆盖，等佐证。
- **Turbo 数据目录**（20260806）：`%LOCALAPPDATA%\Turbo`(1.58G) 带 `User Data\Crashpad + ShaderCache` 结构，软件列表有 "Turbo" 但身份不明（疑似加速器/浏览器类），1.58G 大头构成未明（cacheHits 只标了 Crashpad/ShaderCache），单样本，已建议用户自查；
- **gtgzgh（高途高中规划）**（20260806）：`%LOCALAPPDATA%\gtgzgh`(872M)，教育软件课程数据，无缓存特征，单样本（学而思 XesCache 同型处理），等教育机佐证；
- **NeatDM（Neat Download Manager）**（20260806 首现 307M → 20260810 双样本佐证 7.5G+896M）：`%APPDATA%\NeatDM`，含下载记录数据库 + 未完成下载分块，与 IDM `DwnlData` 同型。✅ 已入库 `neatdm-cache`（caution，data 类永不默认勾选）+ 知识库 `roaming/neatdm`；
- **QQEX 边界**（20260806）：`%APPDATA%\QQEX`(587M) QQ 扩展组件，Chromium 三件套已由 electron-cache 收编（QQ 判定同型）；额外特征 `mega-converter\cache`、`Features\Cache` 不在三件套内，等佐证再评估；
- **OneDrive StandaloneUpdater**（20260806）：`%LOCALAPPDATA%\Microsoft\OneDrive\StandaloneUpdater`，更新器目录无官方删除佐证，仅 logs 已入库（onedrive-logs），此目录等佐证；
- **火绒 Sysdiag\log**（20260806）：`%ProgramData%\Huorong\Sysdiag\log`(122M)，火绒诊断日志，安全软件边界，单样本（friend-d 也装火绒未报此路径）；
- **Roaming\baidu 画像**（20260806）：`%APPDATA%\baidu`(4.5G) 无缓存特征，网盘客户端数据红线不动；`BaiduYunGuanjia\logs`(78M)/`BaiduYunKernel`(34M) 同属 baidu 系，仅记录；
- 小项画像（20260806，均单样本暂缓）：`Microsoft\Internet Explorer\CacheStorage`（browser-cache 未覆盖 IE）、`Microsoft\Olk`（Outlook 临时）、`Whale\temp`(50M)、`ProgramData\Kingsoft`(274M 无特征)、`cdsyy\Log`(188M)、`ETS`(337M)、`dsgame\cache+log`(32M)、`GameViewer`(77M)、`flutter_webview_windows`(64M)、`PC Manager Store`(38M)、`com.follow`(34M)、`NexBox\EBWebView`(90M，Crashpad 维持暂缓)；本机 `%LOCALAPPDATA%\Microsoft\Packages`(1.5G) 为 UWP 数据不动；D:\TapTap 游戏 20.5G/D:\xwechat 12.4G 为个人文件红线。
- **Mission Planner（1708 测绘机）**：`%ProgramData%\Mission Planner`(1.4G)。无人机地面站软件，内含飞行日志/地图缓存，ProgramData + 个人工程数据双红线，只记录画像不动；
- 未知浏览器/工具类（1708 测绘机，均单样本、身份待查明）：`SPChrome`(1.3G)、`CapableWin`(521M)、`winToolBox`(330M)、`MyChrome`(293M)、`app_shell_cache_6383`(243M)、`liebao` 猎豹(453M)——多数带 `User Data\Crashpad+ShaderCache` 结构，疑似 Chromium 外壳/浏览器系，等更多样本佐证；
- **微信开发者工具**（1719 监控+开发机）：`%LOCALAPPDATA%\微信开发者工具`(835M)。含微信小程序工程代码/用户数据=个人数据红线，不动；该样本其余大头（GitHubDesktop 1.75G VERSION-SIBLINGS、DingTalk_108/133、electron-cache 收编 Cursor/Code/npm、scnet 等 9 个 `*-updater` 更新器残留）均已由覆盖验证，见上表；
- **QianniuTemp**（1719）：`%LOCALAPPDATA%\QianniuTemp`(537M)，千牛（淘宝商家工具）临时文件带 `Temp`，单样本暂缓；
- **ShadowBot**（1719）：`%LOCALAPPDATA%\ShadowBot`/`ShadowBotBrowser`(共 ~200M)，影刀 RPA 自动化工具，非缓存特征不确定，暂观察；
- 小项画像（20260808，均单样本、身份待查）：`Okeanos`(NetEase 网易云 3.7G)、`KOOK`(1.8G→第二样本 883M，游戏语音)、`Qingfeng`(416M)、`Autodesk` Logs(422M)、`kfastpic_sogou`(200M)、`Battle.net`(136M)、`sogoupdf`(123M)、`clash-verge-rev`(76M)、`GameViewer`(87M)、`calabiyau`(63M)、`LCEDA-Pro`(48M)/`Bentley`(47M)/`betaflight-configurator`(30M)/`inav-configurator`(28M)/`ESRI`(15M)（1708 军工/测绘系）——均单样本，待佐证；
- **CapCut**（2354 游戏机）：`%LOCALAPPDATA%\CapCut`(2.2G)，cacheHits=`User Data\Cache; User Data\Log`——只有 Cache+Log，缺 Code Cache/GPUCache，不满足 electron-cache 指纹规则；视频编辑器，User Data\Cache 可能含视频预览渲染缓存，单样本暂观察；
- **oopz**（2354 + 0955 双样本）：`%LOCALAPPDATA%\oopz`(1.1G+1.1G)，无 cacheHits，身份不明，两台不同机器均安装；
- **PixPin**（2354）：`%LOCALAPPDATA%\PixPin`(416M)，截图工具，cacheHits=`Crashpad; Temp`；
- **Shandianshuo（闪电说）**（2354）：`%APPDATA%\Shandianshuo`(895M，Roaming logs) + `%LOCALAPPDATA%\Shandianshuo`(69M，EBWebView Crashpad+ShaderCache)；系统优化/加速类工具；
- **BCUT（必剪）**（1149 开发机）：`%LOCALAPPDATA%\BCUT`(225M)，B站视频编辑工具，cacheHits=`log; dd\cache`；
- **AzureFunctionsTools**（1149）：`%LOCALAPPDATA%\AzureFunctionsTools`(917M)，VERSION-SIBLINGS 标记，Azure  Functions 本地开发工具多版本并存，可能适合版本残留清理但需确认目录结构；
- **node-gyp**（1149 + 0955 双样本）：`%LOCALAPPDATA%\node-gyp`(59M+51M)，cacheHits=`Cache`，Node.js 原生模块编译缓存，VERSION-SIBLINGS 标记；
- **Yodao/有道**（1149）：`%LOCALAPPDATA%\Yodao`(456M) + `youdao`(340M)，有道词典/翻译，无缓存特征；
- **r2modman**（1149）：`%APPDATA%\r2modmanPlus-local`(122M，RiskOfRain2 cache) + `%APPDATA%\r2modman`(29M，electron-cache 已收编)；游戏 mod 管理器，Roaming 部分含游戏 mod 缓存数据；
- **KuGou8（酷狗音乐）**（2354）：`%APPDATA%\KuGou8`(468M)，cacheHits=`log; CefCache89\*`——非标准 CEF 缓存结构（带版本号目录 CefCache89），不匹配 electron-cache 指纹；
- **MathWorks/MATLAB 日志**（1001 第二佐证）：`%LOCALAPPDATA%\MathWorks`(1.24G，含 ServiceHost\logs)，friend-f 也有出现。两份样本均含 logs 特征，但 1.24G 不全是日志还含 MATLAB 客户端数据。等第三份佐证确认 logs 子目录独立大小后再评估是否入 `matlab-logs`；
- **WinSxS\Temp\InFlight**（0823）：`C:\Windows\WinSxS\Temp\InFlight`(5.76G)。Windows 组件存储事务性暂存目录，正常在 CBS 操作完成后自动清理。5.76G 说明有中断的 Windows Update/DISM 残留。**系统级目录不可做自动清理规则**（中途删除会破坏系统），仅建议用户手动跑 `Dism /Online /Cleanup-Image /StartComponentCleanup`；
- **.NET Runtime 预览/RC 版本残留**（0823）：该机器装了 .NET 6/7/8/10/11 五个大版本，其中 11.0.0 Preview 4+5、10.0.0 RC 1、8.0.0 Preview 2、6.0.10(x86)、7.0.4(x86) 等预览/旧版合计 ~800MB+。开发者机器特有问题，删了可能影响依赖特定版本的开发项目。单样本、风险高，暂观察；
- **QuarkCloudDrive VERSION-SIBLINGS**（0823）：`D:\Program Files\QuarkCloudDrive` 三版本并存（7.0.5/7.0.6/6.9.7，共 3.6G）。装在非标准位置（D 盘），现有 wps-old-versions 只覆盖注册表锚点型，需确认 QuarkCloudDrive 是否有注册表安装版本记录；
- **OSGeo4W**（1001）：`%ProgramData%\OSGeo4W`(2.99G，含 var\log)。GIS 开源地理空间工具链，专业软件数据目录=红线区域，只记录画像不动；
- **LocaSpace / TuxinEarth**（1001）：国产三维地球/遥感工具（Roaming 41M/381M），LocaSpace 含 Cache+Temp，TuxinEarth 含 CefCache 但非标准 electron 指纹不匹配。均单样本小项，暂观察；
- **Buzz（Whisper 转写工具）**（1221）：`%LOCALAPPDATA%\Buzz`(8.24G)，cacheHits=`Cache; Logs`——开源语音转文字工具，8G 大头疑似 Whisper 模型文件，Cache 具体构成未明。等第二样本确认 Cache 内模型/缓存占比后再评估；
- **pypoetry 缓存**（1221）：`%LOCALAPPDATA%\pypoetry`(698M)，cacheHits=`Cache; Cache\cache`（artifact 缓存，pip-cache 同型，理论 safe），单样本待佐证；
- **NuGet 第二路径候选**（1221）：`%LOCALAPPDATA%\NuGet`(876M，无 cacheHits)——现有 nuget-cache 只收 `~/.nuget/packages`，此处为 VS/NuGet HTTP 缓存（v3-cache）形态。等佐证后评估扩充；
- **QuarkCloudDrive 多版本第二佐证**（1221）：`D:\QuarkCloudDrive` 三版本并存（6.9.5.757/7.0.5.767/7.0.6.771 共 3.61G），与 0823 同型（均在 D 盘非标准位置）。两份样本坐实多版本残留模式，仍待确认注册表锚点可行性；
- **RunningCheeseChrome 便携浏览器缓存**（1221）：`D:\RunningCheeseChrome_Win7\Cache`(1.77G)，奶酪便携 Chrome，自定义路径规则扫不到，仅记录画像；
- **Red Giant ProgramData\Logs**（1221）：`%ProgramData%\Red Giant`(875M，Logs 命中)，AE/PR 专业插件系日志，ProgramData 专业软件边界，等佐证；
- **顶层 EBWebView（归属不明）**（1644）：`%LOCALAPPDATA%\EBWebView`(257M)，带完整 Chromium 三件套（Default\Cache+Code Cache+GPUCache）——是某个未建专属目录的 WebView2 应用的共享数据区，清理技术上安全但无法归属到具体软件，单样本暂观察；
- **Intel Package Cache {GUID} 变体**（1644）：`%ProgramData%\Intel Package Cache {1CEAC85D-...}`(274M)，Package Cache 家族首次出现 GUID 后缀变体。规则为白名单架构（无规则=永不删），误删风险天然不存在，仅记录画像；
- **NBTool**（1644）：`%APPDATA%\NBTool`(298M)，无缓存特征，身份不明，单样本待查；
- **DigiDNA/iMazing**（1644）：`%ProgramData%\DigiDNA`(233M)，iMazing iOS 设备管理/备份数据，个人数据红线不动（与 Apple MobileSync 同型）；
- 小项画像（1644，均单样本暂缓）：`Oray\SunloginClientLite\log`(59M 向日葵精简版日志)、`Lenovo`(131M OEM 日志 devicecenter/ImController\Temp/Udc)、`ProgramData\Tencent\QQPinyin`(85M，cache/log/Temp 命中但 Tencent 系永拒不开例外)、`Foxmail7`(91M 邮件数据红线)、`GameViewer` 第三佐证(159M，网易 UU 远程，cache 命中仍小项)、`sogousdk`/`kdiskmgr_sogou`(共 56M 搜狗残留)、`ichat`(205M，CefLocalStorage 前缀非标准指纹不匹配)；
- **Ashampoo Driver Updater**（2256）：`%APPDATA%\Ashampoo`(3.28G)，驱动更新器下载的驱动备份/安装包，体积大但单样本，等佐证再评估是否入 junk；
- **TslGame（PUBG）Saved\Logs**（2256）：`%LOCALAPPDATA%\TslGame`(576M，Saved\Logs)，与已入库的 Pal Saved\Logs 同型，可扩充 game-logs 名单，单样本等佐证；
- **O+Connect（OPPO 互联）**（2256）：`%APPDATA%\O+Connect`(562M)，完整 electron 三件套+devicespace\temp 已被 electron-cache 部分收编，手机互联工具，单样本；
- **GreenCore7z/greencore**（2256）：`%APPDATA%` 双目录共 675M，无缓存特征，疑似绿色版 7-Zip 系工具残留，身份不明单样本待查；
- 第二佐证批量更新（2256）：`BCUT`(376M，Cache+mod\cache)、`Battle.net`(85M，Cache+Logs)、`PixPin`(729M，Crashpad+Temp，体积翻倍)、`Nutstore logs`、`Listary`(85M，UserProfile\Cache)——均维持观察，再来一份即可评估；
- **ShadowBot（影刀 RPA）第二佐证**（0832）：`%LOCALAPPDATA%\ShadowBot`(1.07G，首次出现明确 cache;log;cef\cache;cef_main\cache 特征)，RPA 工具可能含用户脚本工程，大头构成未明，等第三样本确认后再评估只清 cache/log 子目录；
- **iLiuGong（柳工通企业版）**（0832）：`%LOCALAPPDATA%\iLiuGong`(1.48G，User Data\Cache)，企业定制应用，缓存可清但单样本；
- **DM Pro6（DreamMail6）**（0832）：`%APPDATA%\DM Pro6`(469M，VERSION-SIBLINGS+Log)，邮件客户端日志，邮件本体红线但 Log 子目录理论可清，单样本；
- 第二佐证批量更新（0832）：`KuGou8`(382M，CefCache89)、`cdsyy`(360M，Log)、`ichat`(185M)——维持观察；火绒 Sysdiag\log 三佐证（122M→68M→687M）已入知识库只解释（ProgramData 边界不入清理白名单，观察名单按 SOP 不收 ProgramData 项）；
- ~~**美图秀秀/美图（Meitu）**~~ ✅ 已于 2026-08-13 双佐证后入库：meitu-cache（MTXXAgent\Cache、MTXXPCL\Cache、XiuXiu\Cache、XiuXiu\Temp，存在才收录）+ 知识库 appdata/local/meitu（Cache/Safe）；
- ~~**wpsphoto+\cache（wps-cache 扩充候选）**~~ ✅ 已于 2026-08-13 双佐证（0951+0037）后扩充入 wps-cache；
- **ima.copilot（腾讯 IMA）**（0951）：`%LOCALAPPDATA%\ima.copilot`(1.38G)，仅 Crashpad+ShaderCache 不满足三件套指纹。IMA 知识库以个人知识资产为主，单样本暂观察；
- **PCAppStore（火绒应用商店客户端）**（0951）：`%LOCALAPPDATA%\PCAppStore`(505M，storecache\Cache+Code Cache)，带 storecache 前缀非标准指纹，单样本；
- **Google Local 5.13G**（0951）：`%LOCALAPPDATA%\Google` 无 cacheHits，软件列表无 Chrome，身份不明（疑似 Google Drive/地球类），已建议用户自查；
- **微信双数据目录并存**（0951）：`D:\xwechat_files`(21G) + `D:\微信缓存文件\WeChat Files`(15.75G)，疑似旧版微信迁移后旧目录未清——属用户自决项，工具侧不碰，可作未来“迁移残留提示”功能线索；
- 小项画像（0951，均单样本暂缓）：`secoresdk`(451M 身份不明)、`WorkBuddy` 系(Roaming 1.03G electron+Local Extension 111M+`.workbuddy` 2.49G 程序本体)、`360ChromeX`(506M 浏览器本体非缓存)、`Yodao\DeskDict\updaters` 第二佐证、`360huabao`(141M)、`Thunder Network`(76M)、`MAXHUB`(13M Log+Temp)、`Timi Personal Computing`(550M ProgramData 只解释)；
- **Larian Studios（博德之门3 启动器）**（0211）：`%LOCALAPPDATA%\Larian Studios`(6.53G，Launcher\Cache+Launcher\Logs)，体积大且缓存特征明确，但 BG3 启动器 Cache 可能含 mod 元数据，单样本等佐证再评估；
- **bililive（哔哩哔哩直播姬）**（0211）：`%LOCALAPPDATA%\bililive`(1.7G，仅 User Data\Cache 不满足三件套指纹)，单样本；
- **app_shell_cache_6383 第二佐证**（0211）：485M（1708 测绘机 243M），仍身份不明（疑似某 Chromium 外壳应用缓存），维持观察；
- **QuarkUpdater 无连字符漏网**（0037）：`%LOCALAPPDATA%\QuarkUpdater`(49M，UPDATER-RESIDUE)，与 QianwenUpdater 同型的无连字符更新器漏网案例，维持观察不动通用规则；
- **Roaming\Quark\Cache 第二佐证**（0211）：39M（20260806 首现 48M），仍小项，维持观察；
- **NetEase CloudMusic（网易云音乐）**（0037）：`%LOCALAPPDATA%\NetEase\CloudMusic`(589M，Cache+dumps+Log+Temp)，目录名与旧样本 Okeanos 不同，缓存特征明确但单样本；
- **指纹浏览器系（电商多账号机）**（2257）：`WaXiangBrowser`(1.09G VERSION-SIBLINGS)、`FingerprintBrowser`(801M)、`Waxiang`(527M)——无缓存特征，可能含多账号会话数据=红线倾向，仅记录画像；
- **ima.copilot 第二佐证**（2257）：2.16G（0951 为 1.38G，VERSION-SIBLINGS），仍以 Crashpad+ShaderCache 为主，IMA 知识资产红线倾向，维持观察；
- **Photoshop Temp 盘根残留**（2257）：`D:\Photoshop Temp<随机数>` ×4 共 ~22G，PS 异常退出未自清的临时文件，技术上可删但盘根自定义路径规则扫不到——用户自决项，可作未来“异常退出残留检测”功能线索；
- **夸克 videoCache 自定义路径**（2257）：`D:\ProgramData\QuarkCloudDrive\videoCache`(17.4G)+`D:\ProgramData\Quark\videoCache`(5.7G)，视频播放缓存体积巨大但在用户自定义盘路径，规则扫不到，用户自决项；
- 小项画像（0037/0211，均单样本暂缓）：`DeadByDaylight`(55M Saved\Logs 可扩充 game-logs 等佐证)、`Back4Blood`(308M 无特征)、`b1`(603M Saved\Logs 身份不明)、`Labcenter Electronics`(628M Proteus 工程软件数据红线倾向)、`yoo_web_cache`(558M EBWebView)、`IQIYI Video`(318M LStyle/QyGeePlayer cache)、`PixCake-qt_pro`(888M cef 缓存)、`TP-LINK Surveillance`(311M 监控缓存)、`Wondershare`(见下方已入库划掉行)、`ToDesk`(46M Logs)、`KoeiTecmo`/`BG3ScriptExtender`(BG3 mod 相关小项)；
- ~~**Wondershare PDFelement Log/Temp**~~ ✅ 已于 2026-08-13 三样本佐证（0037/2257/1612）后入库：wondershare-logs（扫描 Roaming\Wondershare 下 PDFelement*/PDFThumbnail 前缀目录的 Log+Temp，junk/safe）；
- **KwaiLive（快手直播伴侣）**（2120）：`%APPDATA%\KwaiLive`(1.72G，完整 electron 三件套+AegonConfig\cache+basic\tmp)，体积大特征明确但单样本；
- **Telegram Desktop tdata dumps/temp**（2120）：`%APPDATA%\Telegram Desktop\tdata`(183M 中 dumps+temp 可清，但 tdata 本体含账号会话=红线)，若未来入库必须只点名 dumps/temp 子目录；
- **CamScanner（扫描全能王）**（2120）：Local 738M 无特征（大头疑似文档扫描数据=红线倾向）+Roaming 40M electron 三件套，仅记录；
- 佐证批量更新（2120/1612）：`app_shell_cache_6383` **第三佐证**(333M，仍身份不明)、`O+Connect` 第二佐证(115M)、`QuarkUpdater` 第二佐证(41M 无连字符漏网)、`TslGame` 第三佐证（刚入 game-logs 即验证）、`GameViewer` 持续小项佐证、`QuarkCloudDrive 多版本` **第三佐证**(1612 D盘 3 版本 3.61G)、`夸克 videoCache 自定义路径` 第二佐证(1612 4.13G)；
- **随机名目录疑似 PUP 第二例**（1612）：`hygyzt`/`hyszdl`/`hacadyex1`/`BypassRuntm`/`ZhxxPlus` 等无特征随机/怪名目录（friend-f 随机名广告软件同型），体积均小，维持观察；
- **DeliveryOptimization 迁移 D 盘**（1612）：`D:\DeliveryOptimization\Cache`(2.88G)，Windows 传递优化缓存被系统迁到 D 盘，非标准位置规则扫不到，仅记录；
- 永久拒绝：Tencent 系（聊天数据同树）、Postman（含 workspace）、Blackmagic/DaVinci（含项目库）、OCS/yuque/Gandownload（PERSONAL 标记）、Package Cache（Windows Installer 缓存，删了会破坏软件修复/卸载）。

## webview2 安装失败样本（2026-08-03 分析）

**样本**：`disk-butler-webview2-report.txt`——用户机器（Win10 22H2 家庭版 x64）跑 `tools/webview2-diagnose.bat` 的输出，反馈"C盘管家安装失败/打不开"。

**根因（关键反转）**：微软官方确认 WebView2 Runtime（含 x64 版）**始终装在 `C:\Program Files (x86)\Microsoft\EdgeWebView`**，官方"是否已装"检测法就是查 `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-...}\pv`。本机 **pv=113.0.1774.35 存在** → 按微软标准 Runtime"已安装"，Tauri NSIS 安装器据此**跳过自动安装**。但实际这台机器只有 **x86 架构的旧版 Runtime 113**（2023 年 5 月版，随 32 位 Edge 113 附带，EdgeUpdate 停更未升级）——x64 版组件从未装上，x64 的 C盘管家初始化 WebView2 失败。
- 佐证：Edge 装在 `C:\Program Files (x86)\Microsoft\Edge`（32 位）、版本 113.0.1774.35 与 Runtime 同号 → 两者同源同版本，EdgeUpdate 停更；`Program Files\Microsoft\EdgeWebView` 不存在是脚本误导项（微软从不装那里）。
- 教训：**"注册表有 pv" ≠ "x64 Runtime 可用"**。x64 机器上 pv 存在但缺 x64 组件是真实场景（微软官方 issue #1044/#2794/wails #2208 同型）。

**修复建议（移交功能开发）**：
1. 用户侧：装 Evergreen x64 Runtime（官网独立安装器）即可；装 x64 Edge 也会带上。
2. 诊断脚本 `webview2-diagnose.bat` 判据修正：x64 组件存在性应查 `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\<版本>\EBWebView\x64`（或 `msedgewebview2.exe` 位深度），现有"查 `Program Files\Microsoft\EdgeWebView`"必然报不存在、误导判断。
3. 安装器侧（可选项）：NSIS 装前可加"pv 存在但缺 x64 组件"检查；或接受现状、靠诊断脚本+README 指引兜底。

## 提权诊断样本（反馈 S，2026-08-12 分析）

**样本**：`disk-butler-elevation-report.txt`——反馈 S 用户（Lenovo 台式机，账户 lenovo）跑 `tools/elevation-diagnose.bat` 的回传报告，原诉"高级功能『系统临时文件夹更新缓存』分析报『启动分析失败』，提示管理员用策略规则限制了访问"。

**关键发现（与原报错矛盾 → 拦截不在系统层）**：
- ① **提权通道测试 A/B 均成功**（6.3s/3.6s，退出码 0）——系统层提权机制完全正常，排除组策略/AppLocker/WDAC（后两项实测均为 0 规则）；
- ② **UAC 完全关闭**（EnableLUA=0）且当前令牌已是完整管理员（非过滤令牌）——按反馈 R 的结论，此时 DiskButler 内 RunAs 应自动放行无弹窗；
- ③ **装机两款安全软件：火绒 6.0.11.1 + 奇安信天擎 10.8.0.6500**——天擎是企业级 EDR，具备进程链管控能力。

**根因假设（主）**：诊断脚本链路是 powershell→powershell（全微软签名进程），而 DiskButler 链路是 **disk-butler.exe（未签名）→powershell→RunAs**。天擎类 EDR 常见策略是**按进程链信任拦截**：放行签名进程的提权，拦截未签名程序发起的提权/子进程创建——这同时解释了"诊断全通但软件报错"与用户引述的"策略规则限制访问"措辞（天擎拦截提示的典型口吻）。机器装天擎 → 大概率为单位管控电脑。

**✅ 2026-08-13 同错误码第二例（新用户评论，docs/14 反馈 W；⚠ 非反馈 S 重试）**：一位**新用户**通过平台评论报来同类问题，错误码同为 **0xc0000142（STATUS_DLL_INIT_FAILED，"由于应用程序配置不正确，应用程序未能启动"）**，高级扫描亦全 0 B。第二例意义：0xc0000142 是**可复现的共性失败形态**，非单机偶发。关键解读不变：这**不是**"提权被拒/拒绝访问"（0xc0000022/1223），而是**提权子进程被创建出来、但 DLL 初始化阶段崩溃**——典型成因包括 EDR/安全软件注入 DLL 失败（反馈 S 的天擎假设是一例）等。⚠ **归属边界**：天擎注入失败假设仅由反馈 S 的诊断报告推出（S 机装天擎），**不自动适用于新用户 W**（无任何诊断数据）；两例共性只有 0xc0000142 + RunAs 链。反馈 S 的"持续还是偶发"待确认**仍然开放**（S 尚未重试）。⚠ 存疑：W 截图的报错文案（"未能启动管理员程序。请重试或查看 帮助-常见问题"）不存在于 v0.7.3~0.7.7 任何版本代码（均为"启动分析失败：{e}"），git fetch 核实远端无更新版本，用户所跑版本待确认。

**新增修复方向（2026-08-13，移交功能开发）**：反馈 S 机 EnableLUA=0 + 完整管理员令牌 → disk-butler 本身已持完整管理员权限，**根本无需 RunAs**。代码方向：检测本进程令牌已完整提权（TokenElevation）时跳过 `-Verb RunAs` 直接执行——反馈 S 场景直接绕开拦截点，"右键以管理员运行"场景也受益；反馈 W 第二例佐证此方向价值。已记入 docs/07 想法池。

**结论移交（对应 docs/14 反馈 S）**：①非代码 bug 结论维持；②**代码签名议题（docs/07 候选 2）新增真实佐证**——未签名 exe 被 EDR 拦提权是企业机场景的实际痛点；③降级方案（docs/07）价值维持；④用户侧建议：天擎/火绒信任白名单加 DiskButler（可能需 IT 管理员）；⑤S 机 UAC 关闭+完整管理员 → S 可手动清 C:\Windows\Temp（回复草稿已据此给手动步骤）。

**小缺陷记录**：报告第 1 节只抓到 BIOS，OS 名称/版本行缺失（该机 systeminfo 输出未被过滤命中）。🔧 修复已备好、待随后续提交入库（2026-08-13，属反馈领域收尾）：第 1 节改为直读注册表 `HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion`（ProductName/CurrentBuild/UBR/DisplayVersion）为主、systeminfo 兼容兜底，本机验证通过（BOM 完好）——入库后反馈 W 若跑脚本不会再丢 OS 信息。

## 外部模型评估实验记录（2026-07-27）

用本目录两份样本让外部基础模型按 SOP 独立评估（报告：`D:\AI 工作区\rule-evaluation-report.txt`），审查结论：
- ✅ 红线全部守住（PERSONAL/聊天/网盘全拒），SOP 对基础模型的底线约束有效；
- ⛔ 危险误判被人工拦截：TokenBroker\Cache（微软登录令牌）被判 safe——已固化进 SOP 黑名单；
- 🐛 系统性问题：未读现有代码致 1/3 候选重复、needle 路径臆造——已固化为两份 SOP 的“第 -1 节强制准备”；
- ✅ 采纳净增量 3 条：Android Studio 日志、Synology Drive 日志、AMD Radeonsoftware 缓存（JetBrains log/tmp 因已被 jetbrains-cache 覆盖而去重排除）。

## 处理流程

1. 报告放入本目录并按规范重命名，在上表登记；
2. 对 AI 说：**"评估 samples/xxx.md，按 docs/RULES-CLEANUP.md 逐条产出可入库的规则"**；
3. 评估完成后更新上表的"评估状态"和"产出"列；
4. 新规则随下一个版本发布。
