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

## 观察名单（见过但未入库，等更多样本佐证）

- ~~Electron 应用通用 Cache 模式~~ ✅ 已于 2026-07-28 以 electron-cache 指纹规则统一收编（Cache+Code Cache/GPUCache 同级并存才认定；Tencent 系仍排除）；
- **QQ 缓存边界**（friend-c 样本）：QQ 顶层目录名为 `QQ`（非 Tencent），会被 electron-cache 指纹命中，但只取 Chromium 标准缓存三件套，QQ 聊天数据（nt_qq/Documents 树）不在其中——判定为安全，维持现状不排除；如后续样本发现 QQ 把用户数据混入 Cache 同级，再评估加排除；
- QianwenUpdater：不带连字符后缀，现有 *-updater 规则漏网（48MB，优先级低）；
- 永久拒绝：Tencent 系（聊天数据同树）、Postman（含 workspace）、Blackmagic/DaVinci（含项目库）、OCS/yuque/Gandownload（PERSONAL 标记）。

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
