# AI 绘图提示词简报 —— C盘管家 Logo 委托专用

> 用途：把本文件的提示词直接粘贴给专业绘图 AI（Midjourney / DALL-E / Flux / 即梦 / 通义万相）。
> 本简报浓缩了全部品牌决策与 15 版迭代教训，提示词已内置所有纪律。

---

## 一、品牌信息卡（给 AI 的背景，也可粘贴）

- **产品**：C盘管家（DiskButler）——给普通人的 Windows 磁盘清理管家，开源、2.4MB、零联网
- **灵魂**：把用户当成年人；每项操作讲清"这是什么/为什么/删了会怎样"；口号"看得懂 · 敢放心 · 零负担"
- **气质**：可靠的管家，不是尖叫的清理大师——干净、专业、亲和，不恐吓
- **品牌色板**：
  - 主色 青绿 `#12A48C`
  - 点睛 亮薄荷 `#2BDDB9`
  - 层次 深青 `#076B5B`
  - 荣誉色（少用）琥珀金 `#F5B23E`

## 二、硬性纪律（15 版迭代换来的，筛稿时逐条核对）

1. ≤3 色（同色系深浅算一色）；底只许 纯白 / 纯黑 / 透明
2. 单一创意：一个图标只讲一个 idea
3. 剪影级识别：缩到 16px 后主体轮廓仍可辨；线状细节（细线、小星光）一律不要
4. 主体占画布 70% 以上
5. 禁用符号：扫把、垃圾桶、泡泡、火箭、闪电、盾牌+感叹号、AI 星芒——全是"流氓清理软件"或"借来的衣服"
6. 允许的隐喻：字母 C、硬盘盘片（圆环+轴心）、切开的立体环（=知情权）、领结（=管家）

## 三、主力提示词（按验证过的三个最优方向）

### 方向 1：立体切开的 C 盘（内部编号 Z2，用户已认可白底版）

**英文（Midjourney/Flux）**：
```
Minimalist flat app icon for a Windows disk utility. A bold extruded 3D letter "C" 
shaped like a hard-disk platter ring, sliced open with two clean diagonal cuts, 
the cut cross-sections glowing bright mint (#2BDDB9). Main body teal green (#12A48C), 
extrusion depth in dark teal (#076B5B), pure white background, rounded-square app 
icon format. Single concept, bold geometric silhouette readable at 16 pixels, 
no text, no clutter, professional branding, clean vector style --style raw
```

**中文（即梦/通义万相）**：
```
极简扁平应用图标：一个粗壮的立体字母 C，形似硬盘盘片圆环，被利落地斜切开，
切面露出发亮的薄荷绿（#2BDDB9），主体青绿色（#12A48C），立体厚度为深青色（#076B5B），
纯白背景，圆角方形应用图标。单一概念，几何剪影粗壮清晰，缩小到16像素仍可辨认，
无文字，矢量风格，专业品牌设计
```

### 方向 2：盘片×领结（管家概念直译，内部编号 A2）

**英文**：
```
Charming minimal app icon: a glossy teal-gradient hard disk platter (circle with 
white hub hole) wearing a small elegant dark-teal bow tie at its bottom edge, 
like a butler. White background, rounded-square icon. Colors limited to teal 
family (#12A48C, #2BDDB9, #076B5B) plus white. Cute but professional, single 
concept, bold shapes readable at 16px, no text, no sparkles, clean vector style
```

**中文**：
```
亲和极简应用图标：一张青绿渐变的硬盘盘片（圆形+白色轴孔），下缘系一枚精致的
深青色小领结，像一位管家。纯白背景，圆角方形。只用青绿色系（#12A48C、#2BDDB9、
#076B5B）加白色。可爱但专业，单一概念，图形粗壮，缩到16像素仍清晰，无文字，
无星光装饰，矢量风格
```

### 方向 3：自由发挥（只给约束，让 AI 出创意）

**英文**：
```
App icon for "DiskButler", a trustworthy Windows disk cleanup tool. Brand keywords: 
transparency, honesty, butler-like care, letter C, hard-disk platter. Teal green 
palette only (#12A48C primary, #2BDDB9 mint accent, #076B5B depth) on white or 
transparent background. Max 3 colors. One single bold idea, silhouette readable 
at 16px, 70%+ canvas coverage. Strictly avoid: brooms, trash cans, rockets, 
shields with exclamation marks, sparkle clutter. Modern flat style with subtle 
dimensionality, professional branding quality
```

## 四、通用负面提示词（Negative Prompt，支持的平台就填）

```
broom, trash can, garbage, rocket, lightning bolt, shield with exclamation mark, 
sparkles, stars, more than 3 colors, thin lines, fine details, text, letters other 
than C, gradient mesh, 3D render realism, drop shadows everywhere, cluttered, 
mascot face, cartoon eyes
```

## 五、出图后的筛选流程（人工验收）

1. 缩到 48px 和 16px 看一眼——轮廓糊了的直接淘汰（可用画图/浏览器缩放模拟）
2. 数颜色——超过 3 色系淘汰
3. 问自己"这个图形讲了几个 idea"——超过 1 个淘汰
4. 放进真实桌面对比（把图转 .ico 建空快捷方式，方法见 .verify\make-ico.js + add-*.ps1）
5. 最终两问：像不像流氓清理软件？一句话能不能讲出它的寓意？

## 六、已有资产索引

- 全部历史候选 SVG/PNG：`disk-butler/docs/brand/`
- 当前用户认可度最高：`logo-z2-solidc-white.svg`（立体C盘·白底）
- ico 生成脚本：`.verify/make-ico.js`；桌面预览脚本：`.verify/add-*.ps1`
- 参数化生成器（CC Switch 风格）：`.verify/gen-bloomdisk2.js`
