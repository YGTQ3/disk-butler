# C盘管家 v0.7.6 更新说明

## 统计口径修正

- **CleanupReport 改用实际磁盘变化**：结果页"释放了 X"不再沿用旧逻辑（各项 freed 之和可能重复计算），改为 `freeAfter - freeBefore`——与你在 C 盘属性里看到的实际变化一致，数字更老实。

## 清理规则扩充

- **Adobe CameraRaw 缓存**：清理 RAW 预览/解马赛克缓存（`Cache2`/`Cache`，实测 ~284 MB / 144 个 .dat），删后打开 RAW 自动重建。
- **Adobe PS/LR 应用缓存**：Photoshop GPU 缓存 / 字体缓存 / 日志 + Lightroom Caches，重启后自动重建，首次稍慢。

## 安全加固

- **CSP 收窄**：从 `null`（等于不设防）升级为具体策略，只允许本域 + github.com；`opener` 权限从 `**` 收窄到 `C:/**`~`H:/**`，新增 `open-url` 只允许 github.com。
- **DISM 日志卫生**：分析/清理日志文件名改为随机 nonce，且成功+失败两条路径都**用完即删**，不把临时诊断文件落在 `%TEMP%`。

## 体验优化

- **磁盘透视磁盘选择框**：右侧加下拉箭头，提示可点击切换磁盘。
- **贡献模态框**：邮箱 `ygtq1021@126.com` 直显 + 一键复制按钮；没有邮件客户端时 `mailto` 失败降级提示"复制后用手机/网页邮箱发"，不再让用户茫然。

## 测试

- 106 项 Rust 单元测试全绿
- 前端 TypeScript 类型检查 0 错误
- CodeReview 子智能体独立复核 diff 通过
