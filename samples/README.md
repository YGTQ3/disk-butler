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
