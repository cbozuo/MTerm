# 多行粘贴弹框 · 设计规格（2026-11-06 v1）

> 本文件是「多行粘贴确认弹框」高保真设计的**事实底稿**：左侧为代码事实（带
> `文件:行`，可直接核对），右侧为设计约束与验收口径。高保真 HTML 与评审报告
> 均以本文件为准；与主流对标的结论见
> [multiline-paste-research-2026-11-06.md](./multiline-paste-research-2026-11-06.md)。

## 1. 目的

MTerm（Rust + Slint 的 SSH/终端客户端）当前多行粘贴确认**复用通用
`ConfirmDialog`**——一个为 SFTP 删除、主机密钥信任等场景设计的 380px 窄卡片。
多行粘贴的实际内容是「一段可能十几行、几百字符、含危险命令的脚本」，需要一个
**专用评审面（review surface）**：可读的预览、行号、危险命令标记、粘贴范围与
目标会话上下文，以及「粘了会不会直接执行」的明确承诺。

本次交付：**高保真视觉稿 + 交互规格**（不改 `ui/*.slint`、不改 `src/*`）。

## 2. 现状（代码事实）

### 2.1 触发链

| 事实 | 位置 |
| --- | --- |
| 粘贴入口：中键 / `Ctrl+Shift+V` → `on-paste-from-clipboard(tab-id)` | `ui/app.slint:2424-2425`, `src/app.rs:12734-12800` |
| 剪贴板在后台线程读（arboard 在事件循环线程读会卡死应用） | `src/app.rs:12739-12756` |
| 弹框条件：`text.contains(['\r','\n']) && (设置项 paste_confirm_enabled || text.len() > 100*1024)` | `src/app.rs:12759-12760` |
| `force_review` 阈值 **100 KB**：即使关掉设置项也强制弹 | `src/app.rs:12759` |
| `large = paste_requires_large_review(&text)` | `src/app.rs:12761` |
| 预览内容 = **原文整段**（未归一化、未截断、无行号） | `src/app.rs:12762` |
| 确认后才编码发送：`encode_pasted_text(&text, bracketed)` | `src/app.rs:12773` |
| 括号粘贴（bracketed paste）能力来自远端：`terminal_uses_bracketed_paste(&bufs, tab)` | `src/app.rs:12749` |
| 设置项 `paste_confirm_enabled` 默认 **true**；`extra_paste_shortcuts_enabled` 默认 true | `ui/app.slint:1701-1704`, `src/app.rs:3446-3447` |

### 2.2 编码语义（确认之后发生什么）

`src/terminal/impls/input.rs`:

- `normalize_pasted_newlines(text)`（:7-9）：`\r\n`→`\r`，再 `\n`→`\r`。终端的
  Enter 是单个 CR，所以**每一行结尾都会变成一次真正的回车**。
- `encode_pasted_text(text, bracketed)`（:62）：
  - `bracketed == true`：剥掉 `ESC`(0x1b) 与 `Ctrl-C`(0x03)，用 `ESC[200~ … ESC[201~` 包裹；
  - `bracketed == false`：原样字节（含全部 CR）。
- `paste_requires_large_review(text)`（:93）：判定是否走「大内容评审」模式
  （阈值见函数体，当前量级为 ~600 字符 / ~12 行）。

**关键含义**：粘贴内容里**没有任何「只粘贴不执行」的开关**。剪贴板尾部的换行会被
归一化成 CR，于是 `命令\n` 粘完就等于按了回车 → 直接执行。这是本设计要正面处理的
头号语义问题。

### 2.3 当前弹框（`ui/confirm_dialog.slint`）

| 事实 | 位置 |
| --- | --- |
| `ConfirmDialog` 是通用组件：`title` / `message` / `detail` / `confirm-label` | `ui/confirm_dialog.slint:37-57` |
| `confirm-on-enter`：**按 Enter = 确认**（多行粘贴调用点开启） | `ui/confirm_dialog.slint:45,76-83`；`ui/app.slint:8203` |
| `large-detail`：卡片 `min(宽-40px, 1160px) × min(高-40px, 760px)`；否则固定宽 `380px` | `ui/confirm_dialog.slint:114-115` |
| `dismiss-on-backdrop`：多行粘贴设为 **false**（点背板不取消，#262） | `ui/confirm_dialog.slint:55,94`；`ui/app.slint:8202` |
| 卡片可拖动、每次打开回中 | `ui/confirm_dialog.slint:97-117` |
| 背板 = `Scrim` 组件（9 样式 + 随机重掷，统一 `Theme.modal-scrim`） | `ui/confirm_dialog.slint:60-65` |
| 多行粘贴调用点：标题「确认多行粘贴 / Confirm multi-line paste」+ 一句警告 + `detail = paste-confirm-preview` | `ui/app.slint:8197-8217` |
| 状态属性：`paste-confirm-open/tab/text/preview/large` | `ui/app.slint:2252-2256` |
| 打开期间抑制全局快捷键：`&& !root.confirm-delete-open && !root.paste-confirm-open` | `ui/app.slint:5259` |
| 确认后把焦点还给终端：`root.terminal-focus-sequence += 1` | `ui/app.slint:8214` |

### 2.4 缺口清单（设计必须回应）

1. **预览是「裸文本」**：无行号、无等宽排版约束、无危险命令标记、无长行折行策略。
2. **无「粘了就会执行」的承诺**：用户看不到「末尾换行 = 回车」的后果。
3. **无「只粘贴不执行」出口**：选项缺失（主流做法见研究稿）。
4. **无目标会话上下文**：弹框不显示将要粘贴到哪个 tab/主机，多标签场景易贴错。
5. **控制字符被静默剥离**：bracketed 模式下 `ESC`/`Ctrl-C` 被丢弃，用户不知情。
6. **Enter 一键确认过强**：`confirm-on-enter: true` + 无回车数提示，回车即可执行整段脚本。
7. **小/大两档切换生硬**：380px ↔ 1160px 两档，中等内容（5–12 行）没有合适档位。
8. **巨量内容（100KB 强制弹）无性能/摘要策略**：直接塞进 `detail`。

## 3. 设计目标

- **G1 一眼可判**：0.5 秒内看清「多少行 / 多少字符 / 会不会执行 / 粘到哪里」。
- **G2 危险前置**：`rm -rf`、`sudo`、`dd`、`:>file`、`curl|sh`、`chmod 777`、
  `mkfs`、`shutdown` 等行级标记，带类别与理由。
- **G3 默认安全**：默认动作是「只粘贴不执行（去尾回车）」还是「原样粘贴」，需在
  研究结论之上做一次明确取舍并写进规格（见 §5）。
- **G4 与项目同源**：复用 `ConfirmDialog` 的视觉语言（`Theme.popup-bg`、
  `radius-lg`、`Scrim`、同款按钮高度 30px / `radius-sm`），不引入第三种弹框风格。
- **G5 三档自适应**：紧凑（1–3 行）/ 标准（4–12 行）/ 评审（>12 行或 >600 字符，
  对齐 `paste_requires_large_review`）。
- **G6 键盘可达**：Tab 序、Esc、Enter 语义、修饰键逃生舱明确且可预期。

## 4. 设计 Token（唯一色源）

### 4.1 权威色板 `mterm-dark` / `mterm-light`（`src/theme/palettes.rs`）

| 槽位 | mterm-dark | mterm-light |
| --- | --- | --- |
| root | `#161619` | `#F2F0EC` |
| panel | `#181A1D` | `#FAF8F4` |
| palt | `#1B1D21` | `#E9E6DF` |
| elev | `#1E2024` | `#FFFEFA` |
| hov | `#22262D` | `#E8E5E1` |
| act | `#252A33` | `#DFDDD9` |
| line | `#232428` | `#E0DEDA` |
| lstr | `#4E5157` | `#D0CECB` |
| t1 | `#BCBEC4` | `#33302C` |
| t2 | `#7A7E85` | `#7B766F` |
| t3 | `#6F737A` | `#968F85` |
| tbg（终端底） | `#181A1D` | `#FAF8F4` |
| tfg（终端字） | `#BCBEC4` | `#33302C` |
| ac（强调） | `#1FC8B4` | `#1F766B` |
| ok | `#73B00A` | `#427213` |
| wr | `#E8A33E` | `#8F631C` |
| dg（危险） | `#F75464` | `#B0463F` |

> 高保真页的暗色变量必须与上表逐值一致（沿用
> `docs/design/left-panel-session-hifi-2026-10-28-v3.html` 的取样口径）。

### 4.2 派生 token（`ui/theme.slint`）

| token | 定义（行） |
| --- | --- |
| `popup-bg` = panel 底 × `popup-alpha` | `ui/theme.slint:235` |
| `popup-bg-elevated` / `popup-bg-alt` / `popup-bg-inset` | `ui/theme.slint:236,246,255` |
| `border-strong` = `slot-lstr` | `ui/theme.slint:271` |
| `field` / `field-2` | `ui/theme.slint:279-280` |
| `line-2` | `ui/theme.slint:281` |
| `danger` = `slot-dg`；`danger-strong` = `#dc4b44` | `ui/theme.slint:398,330` |
| `accent-soft` = accent × alpha(暗 0.16 / 亮 0.12) | `ui/theme.slint:337` |
| `fs-2xs/xs/sm/md/lg/xl` = 10/11/13/13/15/18 px × `ui-scale` | `ui/theme.slint:453-458` |
| `fw-regular/strong` = 400 / 700 | `ui/theme.slint:464-465` |
| `radius-sm/md/lg/xl` = 4/6/10/16 px × `ui-scale` | `ui/theme.slint:541-544` |
| `font-mono`（Rust 写入） | `ui/theme.slint:471` |

**字体**（沿用既有高保真页）：UI `"Segoe UI Variable Text","Segoe UI","Microsoft YaHei UI",system-ui`；
等宽 `"Cascadia Mono","Cascadia Code","JetBrains Mono",Consolas,"Sarasa Mono SC",monospace`。

### 4.3 现状弹框几何（高保真必须能对上）

- 按钮高 `30px`、`min-width: 80px`、`radius-sm`（`ui/confirm_dialog.slint:10-12`）；
- 危险按钮态：常态 `Theme.danger`，hover `#ec6a6a`，pressed `#c44a4a`（`:13`）；
- 卡片 `radius-lg`、底 `popup-bg`、可拖动、打开回中（`:113-119`）；
- 评审档 `min(宽-40px,1160px) × min(高-40px,760px)`，紧凑档 `380px` 宽（`:114-115`）。

### 4.4 对比度约束（WCAG 2.1，实测；v1.1 新增）

正文 ≥ 4.5:1，次要/装饰 ≥ 3:1，大字号（≥18.66px 加粗或 ≥24px）按 3:1。下表为按 §4.1 色值实算
（可复现脚本：`.tmp-lead/contrast.mjs`）：

| 前景 → 背景 | 比值 | 判定 |
| --- | --- | --- |
| 暗 `t1 #BCBEC4` → `panel #181A1D` | **9.38** | ✅ 正文可用 |
| 暗 `t2 #7A7E85` → `panel #181A1D` | **4.28** | ❌ 正文不达标（`palt` 上 4.14、`elev` 上 4.00 同样不达标） |
| 暗 `t3 #6F737A` → `panel #181A1D` | **3.66** | ⚠ 仅装饰/行号；正文不可用 |
| 暗 `dg #F75464` → `panel #181A1D` | **5.32** | ✅ 危险色**文字**可用 |
| 亮 `t1 #33302C` → `panel #FAF8F4` | **12.37** | ✅ 正文可用（`palt` 上 10.53） |
| 亮 `t2 #7B766F` → `panel #FAF8F4` | **4.25** | ❌ 正文不达标 |
| 亮 `t3 #968F85` → `panel #FAF8F4` | **3.02** | ⚠ 仅装饰；`palt` 上仅 2.57 不可用 |
| 亮 `dg #B0463F` → `panel #FAF8F4` | **5.22** | ✅ |
| 白字 → 暗 `dg #F75464`（现状 DangerButton 实心态） | **3.28** | ❌ 30px 按钮上 13px 白字不达标 |
| 白字 → 亮 `dg #B0463F` | **5.54** | ✅ |
| `root #161619` 字 → 暗 `dg #F75464` 实心底 | **5.51** | ✅ 实心危险按钮的合规替代 |
| `root #161619` 字 → 暗 `ac #1FC8B4` 实心底 | **8.58** | ✅ 主按钮（暗） |
| `panel #FAF8F4` 字 → 亮 `ac #1F766B` 实心底 | **5.13** | ✅ 主按钮（亮） |

**硬性用法（高保真必须遵守）**

1. 预览正文与统计条文字一律 `t1`（暗 `#BCBEC4` / 亮 `#33302C`）；**不得**用 `t2/t3` 承载正文。
2. 行号列允许 `t3`（装饰性，正文之外）；折叠提示「… 另有 N 行」必须用 `t1`。
3. 危险行：底色 `danger` 8–12% 淡染 + 左侧 2px `danger` 竖标 + 行尾类别徽标；**行内正文仍用 `t1`**，
   徽标文字用 `danger`（暗 5.32 ✅ / 亮 5.22 ✅）。
4. 主按钮 `.btn-primary`：暗 = `ac` 实底 + `#161619` 字（8.58 ✅）；亮 = `ac` 实底 + `#FAF8F4` 字（5.13 ✅）。
5. 危险动作按钮：优先**描边式**（`danger` 字 + 1px `danger` 边 + 透明底，暗 5.32 ✅ / 亮 5.22 ✅）；
   若坚持实心红底，则字色必须换成 `root`（暗 5.51 ✅ / 亮 5.54 ✅），**不得**沿用白字（3.28 ❌）。
6. 稿子的页面壳「设计说明」区需列出本稿实际使用的每对前景/背景实测比值。
7. 上述不达标项源自权威色板 `src/theme/palettes.rs` **自身**，不是稿子引入；高保真**不得为达标而改动
   §4.1 色值**（保真优先），只能按上述规则选择用法，并在评审报告里标注「继承自权威色板」。
8. **唯一的派生色例外（Lead 裁定，v1.1）**：允许一个对比度补偿色 `--dg-ink`——暗主题取 `slot-dg` 原值，
   亮主题为 `#8E2F28`（因亮 `slot-dg #B0463F` 压在 11% danger 淡染上仅 **4.47:1**，差 0.03 不达标）。
   约束：① 仅可用于「压在 danger 淡染上的文字」；② 所有 danger 填充/描边仍取 `slot-dg`；
   ③ 稿子的设计说明必须写明派生规则与实测值（6.55:1），并给出「严格保真替代方案」（改用 `slot-t1` 文字）。
   除 `--dg-ink` 外，出现任何非 §4.1 原值的新色相 = 保真性缺陷。

## 5. 交互规格（待研究结论回填的决策点用 ⚠ 标出）

### 5.1 三档布局

| 档 | 触发 | 宽度 | 预览区 |
| --- | --- | --- | --- |
| 紧凑 compact | 1–3 行且 ≤600 字符 | 380px（沿用现状） | 3 行内原文，无行号，脚注显示行数 |
| 标准 standard | 4–12 行或 >600 字符 | 560–680px（新增档） | 行号 + 等宽 + 首尾各留 1 行 `…` 折叠 |
| 评审 review | 对齐 `paste_requires_large_review` | `min(宽-40px,1160px)` | 行号 + 危险行标记 + 顶部统计条 + 内部滚动 |

### 5.2 预览区必备元素

1. 行号列（右对齐、`t3`、非选择态不可选中）；
2. 等宽正文（`t1` 暗 / `tfg`），行高 1.55；
3. 危险行：左侧 2px `danger` 竖标 + 行底 `danger` 8–12% 淡染 + 行尾徽标（类别名）；
4. 顶部统计条：`N 行 · M 字符 · K 处风险`，风险计数用 `danger`，无风险用 `ok`；
5. 折行开关（软折行 / 横向滚动），默认软折行；
6. 超长内容：>200 行或 >100KB 时摘要 + 「展开全部」（虚拟化提示）；
7. 控制字符提示：bracketed 模式且原文含 `ESC`/`0x03` 时展示一次「将剥离 N 个控制字符」。

### 5.3 决策点（v1.1：D1–D5 已由 Lead 裁定，依据 `multiline-paste-research-2026-11-06.md`）

> 冲突以本节为准；评审以本节为判定依据。研究稿 §③ 的 D1/D2 建议被**改造后采纳**，差异见下。

**D1 ✅ 默认动作 —— 三动作结构，执行必须显式**

| 动作 | 文案（zh / en） | 样式 | 绑定 | 结果 |
| --- | --- | --- | --- | --- |
| 主（默认焦点） | `粘贴（不含末尾回车）` / `Paste without trailing newline` | `accent` 实底 + 深色字（暗 8.58 ✅ / 亮 5.13 ✅） | `Enter`、点击 | 剥离尾部 CR/LF，最后一行留在命令行不执行 |
| 次（危险） | `粘贴并执行` / `Paste and run` | 描边 `danger`（danger 字 + 1px danger 边 + 透明底，暗 5.32 ✅ / 亮 5.22 ✅） | 仅显式点击或 `Alt+P` | 原样（含尾回车）= 与现状 `ui/app.rs:12773` 行为一致 |
| 取消 | `取消` / `Cancel` | GhostButton（现几何 30px / radius-md） | `Esc`、点击 | 不发字节 |

- 原文**末尾无换行**时，两动作等价 → 只渲染一个主按钮 `粘贴 / Paste`（不再有危险按钮）。
- 原文为「单个逻辑行 + 末尾换行」时与默认动作一致（去尾），不额外分支。
- A 级危险内容命中时，主按钮文案追加后缀 `（已去掉末尾回车）`，不改变动作集合。
- 理由：终端里唯一可控的是**尾回车**（内部 CR 在非 bracketed 的远端会逐行执行，见 D5），故把「执行最后一行」这一最易误触的动作从 Enter 路径上彻底拿掉；主流（WT/Tabby）把原样粘贴设为主按钮默认焦点，在 SSH 生产主机场景下不可取。

**D2 ✅ 回车语义 —— `Enter` = 安全动作（主按钮），永不触发执行**

- `Enter` → 主按钮（D1 的「粘贴（不含末尾回车）」）；**`Enter` 绝不触发「粘贴并执行」**。
- 现状 `ui/confirm_dialog.slint:45,76-83` 的 `confirm-on-enter: true`（#392）**语义必须改指向**：Enter 命中的是安全动作，而非「确认即执行」。
- 与 scout 建议的差异（改造后采纳）：scout 建议 Enter = 取消；本裁定让 Enter 落在安全动作上 —— 既不丢操作（键盘流不必重来），也不执行任何行（`Enter` 之后不会自动追发回车），同时避免微软文档所指「ENTER 自动触发 DefaultButton 把确认与执行压成连续两击」的事故链。
- `Alt+P` = 「粘贴并执行」；`Esc` = 取消。

**D3 ✅ 记住选择 —— 采纳 scout**

复选框 `不再提示多行粘贴` / `Don't ask again for multi-line pastes`；勾选并确认后写回既有 `paste_confirm_enabled = false`（`ui/app.slint:1701-1704`，不新增设置键）。**例外**：`text.len() > 100*1024` 的强制复核路径（`ui/app.rs:12759-12760`）与命中 A 级危险的内容不受该开关静默。

**D4 ✅ 危险命令 —— 采纳 scout（改造后）**

只提示不阻断：行级徽标 + 中文类别名 + 框顶汇总 `本段含 N 处危险命令`。A 级（不可逆破坏：`rm -rf`、`mkfs`、`dd of=/dev/`、`> /dev/sd*`、fork bomb、`truncate -s 0`、`shutdown|reboot`、`DROP DATABASE` …）= 红标 + D1 后缀；B 级（提权/管道执行：`sudo`、`chmod 777`、`curl|sh`、`useradd`、`iptables`、`kill -9`、`history -c`、`git reset --hard`、`apt remove` …）= 黄标。16 条模式清单见研究稿 §③ 表，**属本设计建议而非外部权威**。

**D5 ✅ 目标上下文 —— 采纳 scout**

框顶显示 `tab 名 · user@host:port`（数据源 `src/app.rs:12739-12760` 的 tab 上下文）+ bracketed 徽标（`terminal_uses_bracketed_paste`，`src/app.rs:12749`）。文案随 bracketed 变化：**关** → `整段多行将逐行执行`（危险感↑，主按钮副文案明示）；**开** → `整段进入编辑缓冲（bash/zsh 等）`。**贯穿判断**：bracketed 只证明远端开启该模式，不证明远端不执行粘贴内容（WT GH#13014 的 PowerShell 反例），故**不得**用 bracketed 已开来免除多行确认。


### 5.4 键盘

| 键 | 语义 |
| --- | --- |
| `Esc` | 取消（关闭，不发字节） |
| `Enter` | 主按钮 = 安全动作「粘贴（不含末尾回车）」（见 D1/D2）；**不触发执行** |
| `Alt+P` | 「粘贴并执行」（唯一能触发执行的键盘路径） |
| `Tab` / `Shift+Tab` | 在「取消 / 粘贴（不含末尾回车） / 粘贴并执行 / 不再提示」间循环，焦点环用 `accent` |
| `Ctrl+Shift+V` | 再次粘贴 = 忽略（避免叠加弹框）——需在稿中说明态 |

### 5.5 文案（zh / en，与项目双语风格一致）

- 标题：`确认多行粘贴` / `Confirm multi-line paste`（沿用 `ui/app.slint:8204`）
- 摘要：`N 行 · M 字符 · K 处风险`
- 主按钮：`粘贴（不含末尾回车）` / `Paste without trailing newline`；末尾无换行时退化为 `粘贴` / `Paste`
- 危险按钮：`粘贴并执行` / `Paste and run`；取消：`取消` / `Cancel`
- 复选框：`不再提示多行粘贴` / `Don't ask again for multi-line pastes`
- 上下文行：`{tab 名} · {user}@{host}:{port}` + bracketed 徽标（见 D5）
- 执行承诺句（必须有，替代现状「一句警告」）：
  - bracketed 关：`远端未开启 bracketed paste：整段多行将逐行执行。` / `Remote has bracketed paste off: each line will execute as it is sent.`
  - bracketed 开：`远端已开启 bracketed paste：整段会进入编辑缓冲。` / `Bracketed paste is on: the whole block lands in the edit buffer.`


## 6. 验收标准（高保真稿）

1. 单文件 HTML、无外链资源、离线可开；顶部有「暗/亮」切换且整页同步切换。
2. 至少渲染 **6 个状态**：暗·紧凑、暗·标准（含风险）、暗·评审（>12 行）、
   亮·标准、亮·评审、以及「已折叠/展开全部」或「控制字符警示」之一。
3. 页面壳（标题/设计说明/对标结论摘要）+ 模拟器（含 MTerm 窗口底：标题栏、标签栏、
   终端区 + Scrim 遮罩）两级结构，与既有高保真页同构。
4. 所有颜色值可在页面中追溯到 §4 的槽位名（建议在稿中标注 `slot-*` 名）。
5. 与现状弹框的**连续性**可见：能看出「同一组件家族的加宽档」，而非另起一套。
6. 无 JS 报错；`node --check` 抽出的内联脚本可通过（见
   `html-prototype-smoke-test` skill 口径）。
7. 文案 zh/en 双份并存（页内切换或并列展示）。

## 7. 产物与写入范围

| 产物 | 路径 | 负责人 |
| --- | --- | --- |
| 事实底稿（本文件） | `docs/design/multiline-paste-spec-2026-11-06.md` | lead |
| 主流对标研究 | `docs/design/multiline-paste-research-2026-11-06.md` | scout |
| 高保真稿 | `docs/design/multiline-paste-hifi-2026-11-06-v1.html` | designer |
| 评审报告 | `docs/design/multiline-paste-hifi-2026-11-06-v1.review.md` | verifier |

**不在本次范围**：`ui/*.slint`、`src/**`、`cargo build/test` 的代码改动。
