# 清理白名单规则扩充 SOP（给 AI 与人类贡献者）

> 本文档是可执行规范：**逐字照做即可安全地新增一条清理项，无需自行发挥**。
> 适用文件：`src-tauri/src/cleanup.rs`
> 最高原则：**宁可漏掉 100 个可清理项，不可误伤 1 个用户文件。拿不准 = 不添加。**

---

## -1. 评估前的强制准备（硬性步骤，跳过则整份评估无效）

> 本节来自一次真实的外部模型评估事故：它没读现有代码，交了 40+ 条候选，其中 1/3 早已入库、
> 多条 needle 路径凭空臆造永不命中、还把微软登录令牌目录当成了安全缓存。

1. **先读现有规则**：打开 `src-tauri/src/cleanup.rs`，列出 `candidates()` 中全部现有 id 与路径；打开 `src-tauri/src/knowledge.rs`，列出 RULES 中全部 needle。评估报告开头必须附上这两份清单；
2. **逐条去重**：候选与现有规则重叠的，标注"已入库"并跳过，禁止作为新候选提交；
3. **路径实证**：每条候选必须写出一条真实完整路径（如 `C:\Users\x\AppData\Local\...`）。注意：`ProgramData`、`%USERPROFILE%\.nuget` 等目录**不在 AppData 下**，禁止臆造 `appdata/` 前缀；
4. **禁止通配符**：清理路径禁止用 `*` 通配整个厂商目录（如 `Adobe\*`、`NVIDIA\*`），只允许点名具体子目录；带版本号的目录用代码枚举（参考 `updaters`/`dingtalk-cache` 的写法）。

---

## 0. 开始前的自检（全部回答"是"才能继续）

| # | 问题 | 若答"否" |
|---|---|---|
| 1 | 该目录里的内容是**程序自动生成**的吗（不是用户亲手保存/下载的）？ | ⛔ 停止，禁止添加 |
| 2 | 删掉后程序能**自动重建**它（或按需重新下载）吗？ | ⛔ 停止，禁止添加 |
| 3 | 你能用一句话向完全不懂电脑的人解释"删了会怎样"吗？ | ⛔ 停止，先查资料 |
| 4 | 该路径能通过 `LOCALAPPDATA` / `APPDATA` / `USERPROFILE` 环境变量拼出吗？ | ⛔ 停止（禁止硬编码盘符和用户名） |
| 5 | 该目录**不含**任何账号、密码、聊天记录、文档、照片、下载成品吗？ | ⛔ 停止，禁止添加 |

### 永久黑名单（无论看起来多像缓存，都禁止加入白名单）

- 任何名字含 `download`（下载目录是**成品**，不是缓存）——真实事故：`BaiduNetdiskDownload` 曾被误判为缓存
- `WeChat Files` / `WXWork Files` 等聊天软件数据目录（含聊天记录和收发文件）
- 网盘同步目录（OneDrive / 百度网盘 / 坚果云 等）
- conda 的 `pkgs` 目录（与环境存在硬链接，删了会损坏环境）
- 浏览器 `User Data` 下除 `Cache`/`Code Cache`/`GPUCache` 以外的任何目录（含密码、Cookie、历史）
- 任何 `Documents` / `Desktop` / `Pictures` / `Videos` 下的路径
- 回收站（已有专门实现，走 Shell API，不走文件白名单）
- **登录凭证与令牌类目录**：`TokenBroker`、`IdentityCache`、`Credentials`、`.ssh`、`.aws` 等——**名字带 Cache 也不许碰**。真实反例：`Microsoft\TokenBroker\Cache` 曾被外部模型判为"安全缓存"，它实为微软账户登录令牌，删除会导致账号登录异常

---

## 1. 三个字段的判定表（照查，不许自创）

### 1.1 `safety`（安全等级，二选一）

| 值 | 判定标准 | 例子 |
|---|---|---|
| `"safe"` | 删除后**用户完全无感**，或最坏情况只是"某软件下次启动稍慢" | Temp、npm 缓存、着色器缓存 |
| `"caution"` | 删除有**用户可感知的代价**（要重新下载大文件 / 丢断点 / 变慢好几分钟） | HuggingFace 模型、IDM 分块、JetBrains 索引 |

> 没有第三个值。觉得比 caution 更危险 = 回到第 0 节 ⛔ 禁止添加。

### 1.2 `kind`（清理分型，改 `kind_of()` 函数）

| 值 | 判定标准 | 默认勾选行为（自动，勿手改） |
|---|---|---|
| `"junk"` | 纯垃圾：残留、报告、过期安装包，**留着没有任何好处** | safe 且非微小 → 默认勾选 |
| `"cache"` | 性能缓存：**留着能让软件跑得更快**，删了要重建 | 仅磁盘空间紧张时默认勾选 |
| `"data"` | 含用户数据或进度（断点、回收站） | **永不默认勾选** |

改法：在 `kind_of()` 的 match 中把新 id 加入对应分支；**不在任何分支里 = 自动归 cache**（这是安全默认，多数新项什么都不用改）。

```rust
fn kind_of(id: &str) -> &'static str {
    match id {
        "temp" | "updaters" | "crash-reports" => "junk",   // junk 加这里
        "idm-dwnldata" | "recycle-bin" => "data",          // data 加这里
        _ => "cache",                                       // 其余自动 cache
    }
}
```

### 1.3 文案（`name` / `description` / `impact`，按句式模板填空）

- `name`：`中文名 (英文原名)`，例：`"Yarn/pnpm 缓存"`、`"临时文件 (Temp)"`
- `description`：一句话回答"**这是什么**"。句式：`「XX 软件/系统的 XX 用途文件」`。禁止术语堆砌，禁止"垃圾""无用"等恐吓词。
- `impact`：回答"**删了会怎样**"，必须以下列三种开头之一：
  - `"没有影响。<顺带说明重建机制>"` —— 仅限 safe
  - `"几乎没有影响。<说明微小代价>"` —— 仅限 safe
  - `"⚠ <具体代价>。<什么情况下不该删>"` —— caution 必须用这种
- 检验：把 description+impact 读给一个想象中的长辈听，他能决策 = 合格。

---

## 2. 添加步骤（复制模板填空）

### 第 1 步：在 `candidates()` 函数中追加

按路径根选择插入位置：`LOCALAPPDATA` 项放 `if let Some(local)` 块内，`APPDATA` 项放 `if let Some(roaming)` 块内，用户主目录项放 `if let Some(home)` 块内。

**模板 A：固定路径（首选）**
```rust
out.push(Candidate {
    id: "xxx-cache",              // 全小写-连字符，全局唯一
    name: "XX 缓存",
    description: "XX 软件的 XX 文件。",
    impact: "没有影响。XX 会在需要时自动重建。",
    safety: "safe",
    paths: vec![local.join("Vendor").join("Cache")],  // 只允许 join 拼接
});
```

**模板 B：路径存在才收录（多个候选位置时用）**
```rust
let mut found: Vec<PathBuf> = Vec::new();
for p in [local.join("A").join("Cache"), local.join("B").join("Cache")] {
    if p.exists() {
        found.push(p);
    }
}
if !found.is_empty() {
    out.push(Candidate { id: "...", /* 同模板 A */ paths: found });
}
```

> 引擎保证（你不用做任何事）：空目录自动隐藏、大小自动计算、只删目录**内容**不删目录本身、使用中文件自动跳过、微小项自动标注。

### 第 2 步：若是 junk 或 data，更新 `kind_of()`（见 1.2；cache 跳过此步）

### 第 3 步：同步磁盘透视知识库

清理白名单里的每条路径，都应在 `knowledge.rs` 有对应规则（用户在透视页看到它时能得到一致解释）。按《RULES-KNOWLEDGE.md》添加，safety 映射：cleanup 的 `safe`→knowledge 的 `Safe`，`caution`→`Caution`。

---

## 3. 验收（全部通过才能提交）

```powershell
cd src-tauri; cargo test -j 2 -q      # 必须全绿
cd ..; npx tsc --noEmit               # 必须 0 错误（本任务通常不涉及前端）
npm run tauri dev                     # 真机验证
```

真机核对清单：
- [ ] 新项出现在正确分组（垃圾残留/性能缓存/含个人数据）
- [ ] "查看详情"展开的路径与预期完全一致，**没有多出任何意外路径**
- [ ] caution 项显示黄色"谨慎"徽章且未默认勾选
- [ ] 实际执行一次清理，确认只删了列出的位置，且原目录仍存在（空壳保留）

提交信息格式：`feat(cleanup): 新增 XX 清理项（safe/caution, junk/cache/data）`

---

## 4. 完整反例复盘（为什么这些规则长这样）

**事故**：早期版本把 `BaiduNetdiskDownload` 按目录名匹配成"网盘缓存"。它实际是用户从网盘**下载成品的存放目录**——如果它进了清理白名单，用户会丢失所有下载的文件。

**教训固化成的规则**：第 0 节问题 1/5、永久黑名单第一条、以及 knowledge.rs 中"下载类目录一律 Personal/Keep 且规则前置"。

**记住**：目录名像缓存 ≠ 是缓存。判断依据永远是**内容归属**（程序自动生成 vs 用户主动获得），不是名字。
