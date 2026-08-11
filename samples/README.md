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
| diskbutler-rule-report-20260811-0823（政府环资部门工作站 Win11 Pro 25H2，6 盘位，C 盘剩 34.5G/112G） | 2026-08-11 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **覆盖率≈95%**，无新入库。electron-cache 收编 QClaw/CodeBuddy/SodaMusic/TapTap/miHoYo 等 10+ 应用、wps-cache/dingtalk-cache/Steam ShaderCache/doubao-shadercache/onedrive-logs/huorong-logs 全部命中；红线正确拦截（Tencent 4.3G/Packages）。新观察：WinSxS\Temp\InFlight 5.76G（系统级不可自动清理）、.NET 预览版/RC 残留 ~800M（开发者机器）、QuarkCloudDrive 三版本 VERSION-SIBLINGS（D 盘非标准位置） |
| diskbutler-rule-report-20260811-1001（GIS/环资专业工作站 Win11 Pro 25H2，C 盘剩 722G/930G） | 2026-08-11 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **覆盖率≈90%，NeatDM 第四份佐证**。neatdm-cache 命中 1.3G（第四台机器确认）、electron-cache 收编 Code/QQEX/yuque/bilibili 等、pip cache/ESRI Staging 命中；红线正确拦截（Tencent 2.4G/baidu 2.2G/anaconda3 永久黑名单）。空间极充裕无清理压力。新观察：MathWorks ServiceHost\logs 第二佐证（1.24G）、OSGeo4W 2.99G（GIS 专业数据红线）、LocaSpace/TuxinEarth（国产 GIS 工具） |
| diskbutler-rule-report-20260811-1221（视频剪辑/AI漫剧制作工作站 **Win10 LTSC 2021**，5 盘位，C 盘剩 220G/300G） | 2026-08-11 | 基础（软件内贡献 collector=app） | ✅ 已评估入库 | **覆盖率≈92%，无新入库**（候选均单样本）。jianying-cache 命中 977M（剪映第三台机器）、electron-cache 收编 Trae CN/Antigravity/OpenCode/obsidian/LolAICoach/Hanako、Twinkstar/npm-cache/nuget-cache/wps-cache 全命中；红线正确拦截（Tencent 3.2G/**Blackmagic 4.5G 永久拒绝第二佐证**/Package Cache 937M）。首份 LTSC 样本。画像：Adobe 2018+2022 便携版×12/DaVinci 双版本/AI 漫剧工具集 126G/虚拟机 470G/系统备份 421G。新观察：Buzz 8.2G/pypoetry/NuGet 第二路径/QuarkCloudDrive 多版本第二佐证/RunningCheeseChrome/Red Giant Logs（见下） |
| diskbutler-rule-report-20260811-1644（轻量家用机 Win11 Home China 23H2，C 盘剩 136.9G/200G） | 2026-08-11 | 基础（软件内贡献 collector=app，driveTopDirs 空） | ✅ 已评估入库 | **覆盖率≈95%，无新入库**。wps-cache 命中 kingsoft 3.08G、electron-cache 收编 QQEX/QQ/qq_guild/subos/lx-music-desktop、pip 命中；红线正确拦截（Tencent 4.9G/Packages/Programs/Package Cache×2/TokenBroker）。新观察：顶层 EBWebView 257M（归属不明）、Intel Package Cache {GUID} 变体 274M（白名单架构天然守住）、NBTool 298M 身份不明、DigiDNA(iMazing) 233M 备份红线、GameViewer 第三佐证（见下） |

## 观察名单（见过但未入库，等更多样本佐证）- ~~Electron 应用通用 Cache 模式~~ ✅ 已于 2026-07-28 以 electron-cache 指纹规则统一收编（Cache+Code Cache/GPUCache 同级并存才认定；Tencent 系仍排除）；
- **QQ 缓存边界**（friend-c 样本）：QQ 顶层目录名为 `QQ`（非 Tencent），会被 electron-cache 指纹命中，但只取 Chromium 标准缓存三件套，QQ 聊天数据（nt_qq/Documents 树）不在其中——判定为安全，维持现状不排除；如后续样本发现 QQ 把用户数据混入 Cache 同级，再评估加排除；
- QianwenUpdater：不带连字符后缀，现有 *-updater 规则漏网（48MB，优先级低）；
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
