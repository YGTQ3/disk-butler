# C 盘管家 v0.7.5 更新说明

## 清理规则扩充

- **iSlide 日志**：清理 iSlide PPT 插件运行日志（~2 GB）
- **MiKTeX 缓存**：清理 LaTeX 字体/包缓存与日志（~46 MB）
- **OriginLab 临时文件**：清理 Origin 数据分析临时文件（~84 MB）
- **TeamViewer 日志**：清理远控软件运行日志（~19 MB）
- **游戏日志**：清理文明 6 / 幻兽帕鲁 / 庄园领主 / 杀戮尖塔 2 等 7 款游戏的日志与崩溃转储（~135 MB）
- **Steam ShaderCache 扩充**：新增 Steam htmlcache/ShaderCache 清理（~311 MB）

## 知识库扩充

新增 13 条 AppData 分类规则，覆盖上述软件及 Zotero、MathWorks、Adobe Dunamis 等（精确识别个人数据，不会误清）。

## 样本入库

friend-f 样本（学术/工程工作站）评估入库，新增清理候选覆盖更多专业软件场景。

## 测试

106 项单元测试全绿，前端类型检查 0 错误。
