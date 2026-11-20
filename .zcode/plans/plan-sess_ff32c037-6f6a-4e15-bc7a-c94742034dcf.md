# 磁盘透视 · 大空间目录一键转移（Junction 方案）

## Context

透视页（DiskInsight）目前纯只读：TreeMap 只能看，不能动。用户希望参考 AppDataCleaner 的「复制→校验→删源→建链」转移思路，在透视页给大目录加「指定目标盘一键转移」能力，为源盘腾出空间。

逆向结论（原理层融合，不抄代码）：参考项目是 egui 应用，其五步流水线思想可取，但存在真实缺陷——无空间预检、无冲突检查、**删源后建链失败=断链且无回滚**、mklink 命令缺 `/C` 失效、固定只扫 Roaming。本实现取其流水线思想，全部缺陷按 disk-butler 规范（DEV-STANDARDS 两步式、信任边界、stats 记账）重建。

**已确认决策**：Junction（免管理员/免 UAC）· 大小+数量校验（不做 SHA-256）· 第一版单目录转移。

## 核心设计：比参考项目更安全的六步流水线

```
预检(只读) → 复制(0~75%, 可取消) → 校验(75~85%) → 原子换名(85%)
  → 建 Junction(88%) → 清理临时源(90~100%) → stats::record + 缓存失效
```

关键创新（修复参考项目的断链风险）：删源不直接删除，先 `fs::rename` 成 `源目录.__transferring__`（同卷原子操作，顺带检测文件占用），建 Junction 失败则换名回退，**任何失败路径源目录都原样保留**。清理阶段残留（文件被占用）如实报告，不影响已生效的 Junction。

## 后端（新建 `src-tauri/src/transfer.rs`，~700 行）

**结构体**（serde camelCase）：`TransferPlan`（源大小/文件数/目标盘剩余/警告）、`TransferProgress`（事件 `transfer-progress`：phase/percent/currentPath/message/done）、`TransferReport`（movedBytes/freedBytes/失败清单/临时残留）。

**三个命令**（`lib.rs` 注册）：
1. `transfer_preflight(source, target) -> Result<TransferPlan, String>` — 只读分析：canonicalize、源为目录且非 reparse point、黑名单前缀校验（windows / program files* / programdata / users / appdata 三根 / $recycle.bin / system volume information / 盘根）、目标为固定盘且与源不同卷、互不嵌套、`目标\源名` 不存在、目标剩余 ≥ 源大小（jwalk 快速统计源树）
2. `start_transfer(app, source, target)` — **后端重跑全部预检**（RULES-SECURITY：不信任前端路径），通过后起线程跑流水线
3. `cancel_transfer()` — 全局 AtomicBool，仅复制阶段响应（此后阶段均为原子/快速操作）

**流水线要点**：OneDrive 云占位文件（RECALL_ON_DATA_ACCESS）→ 直接中止并大白话提示；复制中任何 IO 错误 → 删除自建半成品目标目录后中止（源不动）；symlink/junction 子项跳过并计入报告；`fs::copy`（Windows 上走 CopyFileExW，保留时间戳属性）；成功后 `stats::record(freed)` + 失效源盘扫描缓存（`cache.rs` 新增 `invalidate`）。

**Rust 新依赖**：`junction = "1"`（免提权创建联接）。后端单测覆盖：黑名单/嵌套/同盘拒绝 + 临时目录造树跑完整流水线后 junction 解析成功。

## 前端

1. **选目录**：新增官方 dialog 插件（`package.json` + `Cargo.toml` + `lib.rs` 插件注册 + `capabilities/default.json` 加 `dialog:default`），`open({directory:true})`；弹窗内另列其他固定盘快捷选择（复用 `get_drives`）。
2. **`src/components/DetailPanel.tsx`**：目录且 ≥100MB、非 `::__others__`、safety≠keep 时显示「转移到其他磁盘」按钮 → 回调 `onTransfer(node)`。
3. **新建 `src/components/TransferDialog.tsx`**（沿用 ui-reference 范式：遮罩卡片 + framer-motion + 左明细右绿面板）：`intro(目标选择+三件事说明+绿色安全条) → analyzing → plan(结果面板) → confirm(红色要点弹窗) → running(阶段进度+取消) → done(CheckCircle2+本次释放大字)`。
4. **`src/pages/DiskInsight.tsx`**：`transferNode` 状态接线；done 后自动重扫源盘 + `refreshDrives()`。
5. **`src/types.ts`**：`TransferPlan` / `TransferProgress` / `TransferReport` 类型。

## 收尾（DEV-STANDARDS 纪律）

- docs/19 看板开工占坑 → 完工回写 docs/07 待办；
- 起 CodeReview 子智能体独立审 diff（高风险改动），问题修复后才提交（`feat(dev)`，隔离提交）。

## 验证

1. `cargo test`（`CARGO_INCREMENTAL=0`）+ `npx tsc --noEmit` 双绿；
2. 后端单测：预检拒绝用例 + 完整流水线端到端（含 junction 解析）；
3. 真机冒烟（交你执行）：转移一个 ≥100MB 测试目录到另一盘，确认旧路径经 Junction 可访问、`cleanup-stats.json` 记账、透视页自动重扫后源目录体积归零。