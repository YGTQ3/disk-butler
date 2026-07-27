# 磁盘透视知识库规则扩充 SOP（给 AI 与人类贡献者）

> 本文档是可执行规范：**逐字照做即可安全地新增一条透视展示规则，无需自行发挥**。
> 适用文件：`src-tauri/src/knowledge.rs`（`RULES` 常量表）
> 知识库只做**展示解释**，不参与任何删除决策——但它的 `safety` 标签会影响用户行为，**标错 Safe 等同于教唆用户误删**。

---

## -1. 评估前的强制准备（硬性步骤，跳过则整份评估无效）

1. **先读现有规则**：打开 `src-tauri/src/knowledge.rs`，列出 RULES 中全部 needle（命令：搜索 `needle: "`）。候选与现有 needle 重叠的，标注"已入库"并跳过；
2. **needle 路径实证**：每条新 needle 必须写出一条能命中它的真实完整路径，并验证“小写+正斜杠”后确实包含该 needle。注意：`ProgramData` 不在 AppData 下（正确：`programdata/xxx`，错误：`appdata/programdata/xxx`）；`.nuget`/`.cargo`/`.m2` 在 `%USERPROFILE%` 下不在 AppData 下；
3. **语义一致性检查**：`category: Cache` 的规则 `safety` 不得为 `Keep`（“缓存”和“请勿删除”自相矛盾）——若确需保护，category 应改为 Software/Personal/SystemFile。

---

## 0. 与清理白名单的本质区别（先想清楚再动手）

| | 知识库（本文档） | 清理白名单（RULES-CLEANUP.md） |
|---|---|---|
| 作用 | 在 TreeMap 里向用户解释"这是什么、能不能动" | 程序**真的会去删** |
| 出错后果 | 用户被误导（严重但有挽回余地） | 用户文件直接没了（不可挽回） |
| 收录门槛 | 认识它就可以写 | 五重自检+黑名单 |

结论：知识库可以大胆扩充**覆盖率**，但 `safety` 字段必须保守。

## 1. 一条规则的结构与四个字段判定

```rust
Rule {
    needle: "appdata/local/xxx",          // 匹配片段：全小写 + 正斜杠
    category: Category::Cache,            // 决定 TreeMap 颜色
    friendly_name: "XX 缓存",             // 人话名称
    description: "一句话：这是什么 + 能不能动。",
    safety: Safety::Safe,                 // 能否清理的建议
},
```

### 1.1 `needle`（匹配片段）——最容易出错的字段

匹配算法：`路径转小写、反斜杠转正斜杠后，做 contains() 子串匹配；规则表从上到下，命中即停`。因此：

- **必须全小写**、用 `/` 不用 `\`；
- **越具体越好**：`"appdata/local/jianyingpro"` ✅，`"jianying"` ❌（会误伤所有含此字样的路径，包括用户自己命名的文件夹）；
- 单词类 needle（如 `"cache"`、`"temp"`）**必须带路径分隔符锚定**，如 `"/cache"` 或 `"appdata/local/temp"`，否则 `我的cache合集` 这类用户目录会被误命中；
- **绝不允许**单独用 `download`、`我的`、`backup` 这类用户高频命名词做 needle。

### 1.2 插入位置 = 优先级（规则表从上往下第一条命中即生效）

铁律：**具体规则放在泛化规则前面**。

- 新增 `windows/xxx` 子目录规则 → 必须放在兜底规则 `needle: "windows"` **之前**；
- 新增某软件数据目录规则（如 `baidunetdiskdownload`）→ 必须放在会误伤它的泛化规则（如 `baidunetdisk`）**之前**；
- 找位置的方法：在文件里搜索你 needle 的**每一个前缀子串**，若有任何已存在规则是你 needle 的子串，你的新规则必须放它前面。

### 1.3 `category`（颜色语义，六选一）

| 值 | 颜色 | 判定 |
|---|---|---|
| `System` | 蓝 | 操作系统本体 |
| `Software` | 紫 | 已安装的软件程序体（Program Files、全局包、软件本体数据） |
| `Cache` | 橙 | 程序自动生成、可再生的缓存/临时文件 |
| `Personal` | 绿 | 用户主动创建/下载/保存的东西（**下载目录、聊天记录、网盘目录都算这类**） |
| `SystemFile` | 灰 | 休眠/页面文件等系统级大文件 |
| `Other` | 中性 | 不要主动使用（这是未命中时的自动兜底） |

### 1.4 `safety`（三选一，决定详情卡的建议口径）

| 值 | 判定 | 与 category 的合法组合 |
|---|---|---|
| `Safe` | 用户手动删掉**完全无风险** | 几乎只配 Cache |
| `Caution` | 删除有代价/有更好的清理方式（如"请在软件内清理"） | Cache / Software |
| `Keep` | 不该动：系统件、个人数据 | System / Personal / SystemFile / Software |

硬约束（违反即 bug）：
- `Personal` 类**永远** `Keep`——个人文件轮不到工具建议删除；
- 拿不准 Safe 还是 Caution → 选 **Caution**；拿不准 Caution 还是 Keep → 选 **Keep**（永远往保守方向落）；
- 凡 needle 含 `download` 字样 → 必须 `Personal` + `Keep`，无一例外。

### 1.5 `description` 文案模板

一句话 = 「这是什么」+「能不能动/怎么动」。参考句式：

- Safe：`"XX 的下载缓存，可安全清理（或：可用 xx 命令安全清理）。"`
- Caution：`"XX 的 XX 文件。直接删除会 XX，建议在 XX 软件内清理。"`
- Keep（个人数据）：`"你从 XX 保存/下载的文件，是个人数据，请勿当垃圾清理。"`
- Keep（系统）：`"XX 所需的系统文件，请勿手动删除。"`

禁止："垃圾"、"没用"、"建议立即清理" 等恐吓/怂恿词；禁止不解释后果的裸命令。

---

## 2. 添加步骤

1. 打开 `src-tauri/src/knowledge.rs`，找到 `RULES` 常量表；
2. 按 **1.2** 确定插入位置（默认放进文件中对应的分区注释段落里，如 `// ---------- 应用缓存（AppData） ----------`）；
3. 复制第 1 节模板填空；
4. **必须**在文件底部 `#[cfg(test)] mod tests` 中为新规则加一条断言测试：

```rust
#[test]
fn classify_xxx() {
    let hit = classify(r"C:\Users\abc\AppData\Local\Vendor\Cache\f.bin");
    assert_eq!(hit.friendly_name, "XX 缓存");
    assert_eq!(hit.safety, Safety::Safe);
    // 若本规则是为了压过某条泛化规则，再加一条反向断言证明优先级正确
}
```

## 3. 验收

```powershell
cd src-tauri; cargo test -j 2 -q   # 必须全绿（含你新加的测试）
npm run tauri dev                  # 真机：扫描后悬停/点击该目录，核对名称、颜色、说明、安全徽章
```

核对清单：
- [ ] 新规则命中目标路径（悬停显示人话名称）
- [ ] **没有**把不相关目录也染成新规则（检查同名前缀的其他目录）
- [ ] Personal 类显示绿色且建议为"请勿清理"

提交信息格式：`feat(knowledge): 新增 XX 识别规则（Category/Safety）`

---

## 4. 内容启发式（只读，一般不要改）

`ext_group()` / `profile_classify()` 是对**未命中规则**目录的兜底推断（按文件扩展名构成猜类别）。修改它属于算法变更而非规则扩充，**不在本 SOP 范围内**；唯一允许的小改动是往 `ext_group()` 的某个现有分组补充新扩展名（如新视频格式），改完必须跑全部测试。

推断的三条设计红线（历史决策，不得推翻）：
1. <50MB 的目录不猜；
2. 推断结果只用于展示，**永不进入清理白名单**；
3. 缓存类推断最高只能给 `Caution`，绝不给 `Safe`。

---

## 5. 反例复盘

**事故**：`BaiduNetdiskDownload`（百度网盘下载目录）曾按名字被当成网盘缓存展示为可清理。用户指出：这里放的是他下载的成品文件。

**修复**：新增前置规则 `baidunetdiskdownload → Personal/Keep`，压在泛化的 `baidunetdisk` 规则之前；并由此确立 1.4 的"download = Personal + Keep"硬约束和 1.2 的优先级铁律。
