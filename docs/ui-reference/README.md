# 高级功能 UI 设计基准（新增高级清理类功能一律沿用）

> 参考截图见本目录 PNG。以「系统深度清理(DISM)」「系统临时文件+更新缓存」为范式，新增同类高级功能务必复用此模式，保证体验一致。

## 标准交互流程（状态机）
`idle → intro(引导弹窗) → analyzing(只读分析) → analyzed(结果面板) → confirm(确认弹窗) → running → done(完成弹窗)`

## 三个弹窗（居中遮罩 `absolute inset-0 z-30 bg-black/30`，卡片 `max-w-lg rounded-2xl bg-surface p-7`，framer-motion 缩放入场）
1. **分析前引导弹窗**：标题「开始分析前，先看这 N 件事」+ 编号步骤（圆形序号徽章 `bg-primary`）+ 绿色底安全条「只读取信息，不删除不修改」+ 取消/确定开始。
2. **确认清理弹窗**：左上角图标 + 标题 + 编号要点（真正高危的用红色 `bg-[#FEF2F2]` 强调，低风险的用普通/绿色✓）+ 再想想/开始清理。
3. **完成庆祝弹窗**：`CheckCircle2` 大图标 + 「本次释放」大字(`text-4xl font-bold text-primary-dark`) + C盘剩余变化 + 「历史累计已释放」黄色胶囊 + 好的收下了。

## 结果面板（inline，两列，**核心样式**）
- 外层 `flex gap-3`。
- **左**：`flex-1 rounded-xl bg-bg p-3.5` 明细区。每行 `flex items-center justify-between`，**数字用更黑更粗字体** `<b class="text-sm text-[var(--color-text-main)]">`，标签用 `text-secondary`，形成对比、一眼看清。
- **右**：`w-56 rounded-xl bg-[var(--color-primary-soft)] p-4 text-center` **绿色底面板**，内含「预计可释放」大字 + **清理按钮就放在这个绿框里**（`w-full bg-primary`）。不适用的徽章（如"微软推荐"）按功能取舍，不强加。

## 累计记账
所有清理路径（普通/DISM/系统级/新增）后端都必须 `stats::record(&app, freed)`，前端清完 `loadStats()`，让释放量并入历史累计。

## 红线
风险操作不允许"一个按钮直接清"——必须先只读分析给出预计释放量，让用户看清收益再决定；不到阈值(如 50MB)提示"可先不清"。
