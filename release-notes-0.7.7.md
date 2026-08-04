# C 盘管家 v0.7.7 更新说明

## DISM 输出兼容性修复

- DISM 组件存储分析现在兼容 GBK、UTF-8 及带 BOM 的输出格式。
- 在 Windows 11 启用“Beta：使用 Unicode UTF-8 提供全球语言支持”的系统区域设置时，也能正确读取 DISM 分析结果。
- 修复仅负责识别并保留 DISM 实际输出的文本，不会翻译 DISM 的本地化资源。
