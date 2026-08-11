# C盘管家 v0.7.7 更新说明

## 🌙 暗色模式（issue #5）

- **跟随系统自动切换**：整套界面配色收敛为语义令牌，Windows 深色模式下界面自动翻转，不再刺眼
- **手动三态切换按钮**：跟随系统 / 浅色 / 深色 一键切换，选择会被记住

## 🔧 安装器修复

- **WebView2 检测修正**（反馈 P）：之前"注册表显示已装但实际缺 x64 组件"时直接跳过安装，导致装完打不开；现在检测到缺失会用大白话提示并引导安装 Evergreen x64 运行库

## 🧹 清理规则扩充

- **Edge/Chrome Service Worker 缓存**：CacheStorage/ScriptCache 纳入清理（绝不碰 Database 等站点数据）
- **AMD 显卡驱动缓存**：AMDRSSrcExt 缓存 + AMD PPC 临时文件（样本实测）
- **知识库 +3**：amdrssrcext/amd ppc/3dmark 条目，磁盘透视解释更懂你的电脑

## ✅ 测试

- 125 个 Rust 单元测试全绿
- 前端 TypeScript 0 错误
- CodeReview 子智能体独立复核通过