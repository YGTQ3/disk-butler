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

## 观察名单（见过但未入库，等更多样本佐证）

- Electron 应用通用 Cache 模式（douyin/bilibili/Notion/obsidian/Doubao/Claude-3p 等）：单个都安全，但逐个入库维护重；考虑 v0.6 做“Electron 缓存通用探测”时再统一处理；
- QianwenUpdater：不带连字符后缀，现有 *-updater 规则漏网（48MB，优先级低）；
- 永久拒绝：Tencent 系（聊天数据同树）、Postman（含 workspace）、Blackmagic/DaVinci（含项目库）、OCS/yuque/Gandownload（PERSONAL 标记）。

## 处理流程

1. 报告放入本目录并按规范重命名，在上表登记；
2. 对 AI 说：**"评估 samples/xxx.md，按 docs/RULES-CLEANUP.md 逐条产出可入库的规则"**；
3. 评估完成后更新上表的"评估状态"和"产出"列；
4. 新规则随下一个版本发布。
