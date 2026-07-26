# 🛡️ C盘管家 (Disk Butler)

> 给普通人的 Windows 磁盘与内存维护助手 —— 每一项操作都先告诉你"这是什么、能不能动、删了会怎样"。

[![Release](https://img.shields.io/github/v/release/YGTQ3/disk-butler)](https://github.com/YGTQ3/disk-butler/releases)
[![License](https://img.shields.io/badge/license-MIT-green)](#-许可)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-blue)
![Size](https://img.shields.io/badge/installer-~2MB-brightgreen)

## 为什么做这个？

市面上的"清理大师"们喜欢用**"发现 3000 个垃圾！"**吓唬你，然后偷偷装全家桶。而懂技术的工具（WizTree、TreeSize）对普通人又太硬核。

C盘管家选择第三条路：**把用户当成年人**。

- 🔍 **看得懂**：磁盘占用画成彩色拼图，每个色块都用人话解释"这是什么"
- 🛡️ **敢放心**：清理走白名单制——只碰经过验证安全的目录，删除前可查看**具体到每个路径**的明细
- 🪶 **零负担**：安装包仅 ~2 MB（Tauri 原生），不驻留后台、不开机自启、不保存任何数据、卸载即干净

## ✨ 功能

### 磁盘透视
- TreeMap 树图可视化整个磁盘，大块头一眼可见
- 单击色块逐层下钻，面包屑随时返回
- 色彩语义：🔵 系统 / 🟣 已装软件 / 🟠 缓存（可清理暗示）/ 🟢 个人文件 / ⚪ 系统大文件
- 内置 40+ 条中文软件生态知识库：剪映草稿、微信小程序缓存、npm/pip/conda、IDM 下载分块、WinSxS……每一项都解释"能不能动"

### 安全一键清理
- **白名单机制**：路径由后端强制解析，前端只能传条目 ID——想删白名单外的东西？门都没有
- 每张卡片回答三个问题：**这是什么？为什么占空间？删了会怎样？**
- "查看详情"展开将被清理的每个具体路径及大小，界面显示的就是实际执行的
- 有代价的项目（IDM 断点、IDE 索引、回收站）默认不勾选 + 二次确认黄色警告
- 正在使用中的文件自动跳过，不影响运行中的程序
- 清理后显示释放量与磁盘空间前后对比

### 规划中
- 🚀 启动管理：显示自启动项真实路径与内存占用，给出"建议禁用/保留"标签
- 🧠 内存体检：水位仪表盘 + 内存大户排行 + 人话解读
- 🤖 AI 分析报告：基于目录元数据（不读文件内容）生成智能清理建议

## 📥 下载安装

前往 [Releases](https://github.com/YGTQ3/disk-butler/releases) 下载最新的 `DiskButler_x.x.x_x64-setup.exe`，双击安装。

- 系统要求：Windows 10/11 x64（WebView2 系统自带）
- 首次运行如遇 SmartScreen 提示"未知发布者"，点击"更多信息 → 仍要运行"（个人项目暂无代码签名证书）

## 🔨 从源码构建

```bash
# 环境要求：Node.js 18+、Rust (MSVC 工具链)、VS Build Tools 2022 (C++ 工作负载)
git clone https://github.com/YGTQ3/disk-butler.git
cd disk-butler
npm install

# 开发模式
npm run tauri dev

# 打包安装程序（16GB 内存机器建议限制并行度）
set CARGO_BUILD_JOBS=2 && npm run tauri build
```

> 💡 16 GB 内存机器如遇 `LLVM ERROR: out of memory`：`Cargo.toml` 已为 `windows` 系巨型绑定 crate 单独降级 `opt-level = 1`（纯 API 壳，性能零损失），并建议用 `CARGO_BUILD_JOBS=1/2` 限制并行。

## 🧱 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2（Rust 后端 + 系统 WebView） |
| 扫描引擎 | Rust + jwalk 并行遍历，实时进度推送，Top-N 剪枝 + 按需下钻 |
| 前端 | React 18 + TypeScript + Vite + Tailwind CSS v4 |
| 可视化 | d3-hierarchy squarify 布局 + React 自绘 + framer-motion 动效 |
| 设计体系 | 全令牌化（tokens.css），设计决策见 [DESIGN.md](./DESIGN.md) |

## 📜 设计原则

1. **用户知情权优先**：每个可执行操作都清晰展示作用对象、原理、影响与恢复方式，禁止恐吓式文案；
2. **保守安全**：白名单后端强制、谨慎项默认不勾、使用中文件跳过；
3. **数据永远新鲜**：不落盘、不缓存历史，打开页面即实时扫描；
4. **说人话**：`1.5 GB` 而不是 `1610612736 字节`。

## 📄 许可

MIT License

---

*这个项目源自一次真实的 C 盘清理实战：从 46 GB 剩余空间清出 80+ GB 后，我们把整套分析经验产品化成了这个工具。*
