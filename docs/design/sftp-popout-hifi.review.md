# SFTP 弹出独立窗口 · 高保真 v8 审计清单

> 审计对象：`docs/design/sftp-popout-hifi.html`（v8 · 2026-11-17）
> 审计日期：2026-11-17
> **v9 更新（同日）**：用户定了三处改动并已落到稿里 —— ① 面板常态 = **主窗底部停靠**（主舞台改为完整外壳 + 底部面板槽，面板本体同一份 DOM 在底部 ⇄ 独立窗口之间搬；底部状态栏右端加「弹出独立窗口」图标）；② **右击不再产生勾选态**（只定位当前行）；③ **当前行只留阴影**（去掉底色），树上只有当前目录描边、祖先不再染色。本清单其余结论仍适用；涉及这两处描述的条目以 v9 为准。
> 审计方式：① 脚本逐枚比对 `<symbol>` ↔ `assets/icons/*.svg` 路径几何；② 令牌比对 `src/theme/palettes.rs` / `ui/theme.slint`；
> ③ 稿内「实机 文件:行号」引用逐条回查；④ 浏览器实跑全交互链路（控制台 0 报错）。
> **本轮未改动任何文件**（按用户决定：先不改稿，只出清单）。

## 结论

稿子本身是健康的：**零 JS 报错 · 51/53 图标与项目资产逐字一致 · 稿内实机引用全部核对通过 · 令牌/尺寸大部分对得上**。
要修的是三类：**A 事实性偏差**（稿内自相矛盾/与项目现状不符）、**B 保真细节**（与实机组件规格的差异）、**C 新增功能登记**（实机没有、落地即新活）。
行高口径已定：**保持稿内 28px，注明为弹出窗多列表格的稿内新增**（实机 `sftp_panel.slint:1167` 是 24px）。

| 分组 | 条数 | 性质 |
|---|---|---|
| A 事实性偏差 | 5 | 低风险，纯文档/注释/令牌 |
| B 保真细节 | 6 | 会动少量视觉，需逐条确认 |
| C 实机无 → 新增 | 13 项 | 落地工作量，需登记 |
| D 组 | 6 | 1 条已定（D1 行高）· 1 条已核实无需拍板（D3 无 X）· 4 条待拍板 |

---

## A 组 · 事实性偏差（建议直接改）

### A1　头部「图标暂缺」注释与稿内正文、项目现状三方矛盾
- **现象**：文件头注释写「仅 pause/play/window-min 三枚项目暂缺，落地需新增 svg」；稿内第 1139 行的卡片写「项目暂缺 **1** 枚（pause）…play 已按项目 play.svg 几何替换，window-min 改用项目 minimize-to-tray.svg」；第 758 行注释仍写「项目暂缺的三枚」。
- **证据**：`assets/icons/play.svg` **存在**，且与稿内 `#i-play` 的 path 几何**逐字一致**（我已比对）；`assets/icons/` 全量 132 个 svg 中**没有** `pause.svg`、没有 `minus.svg`、没有 `window-min.svg`；稿内也**从未定义** `window-min` 符号（标题栏最小化键用的是 `i-minimize-to-tray` → 对应项目 `minimize-to-tray.svg` ✓）。
- **改法**：第 14–16 行与第 758 行注释统一为「仅 pause 一枚暂缺；play / minimize-to-tray 项目已有」。

### A2　`i-minus` 是死符号
- **现象**：第 761 行 `<symbol id="i-minus">`，无项目对应资产（项目无 `minus.svg`），稿内**任何位置都未引用**（静态 `href`、`ico('minus')`、`setAttribute` 全查过）。
- **改法**：删除该符号；若后续要表达「缩小」，改用项目 `assets/icons/remove.svg` 并注册。

### A3　面板规格声明的行高与实机不符
- **现象**：文件头声明「面板规格对齐 `ui/sftp_panel.slint`（34px 工具栏 / **28px 行** / 26px 菜单）」。
- **证据**：工具栏 `sftp_panel.slint:314` = 34px ✓；菜单项 `sftp_panel.slint:54` = 26px ✓；**文件行 `sftp_panel.slint:1167` = 24px**（不是 28px）。
- **改法（行高口径已定）**：声明改为「工具栏 34px / 菜单 26px = 实机；文件行实机 24px，**稿内 28px = 弹出窗多列信息表行高（稿内新增）**」。

### A4　亮色主题令牌不是 `mterm-light`
- **现象**：文件头称「令牌取自 `src/theme/palettes.rs`（mterm-dark / mterm-light）」，暗色块完全对得上，**亮色块基本对不上**。
- **证据**（`palettes.rs:120-132` vs 稿第 60–76 行）：

  | 槽 | mterm-light | 稿内 | |
  |---|---|---|---|
  | root / panel | F2F0EC / FAF8F4 | 同 | ✓ |
  | palt | E9E6DF | F4F5F8 | ✗ |
  | elev | FFFEFFA | FFFFFF | ✗ |
  | hov | E8E5E1 | EDEEE9 | ✗ |
  | act | DFDDD9 | E4E6E0 | ✗ |
  | line | E0DEDA | DDD9D1 | ✗ |
  | lstr | D0CECB | B9B5AC | ✗ |
  | t1 / t2 / t3 | 33302C / 7B766F / 968F85 | 2B3036 / 6A6F76 / 8A8F95 | ✗ |
  | ok / wr | 427213 / 8F631C | 5E8F0A / B97818 | ✗ |
  | ac / dg | 1F766B / B0463F | 同 | ✓ |

- **改法**：亮色块逐槽校正为 `mterm-light`；若刻意用另一套自定亮色，则把文件头声明改掉（不能两个都留着）。

### A5　「图标 100% 来自项目资产」清单漏列 5 枚
- **现象**：第 1137 行的清单未列 `check` / `download` / `upload` / `gear` / `star`（`arrow-l/r/up` 由「arrow-*」涵盖）。
- **证据**：以上 5 枚符号均定义在稿内且被引用（`upload`/`download`/`star` 在界面上真实出现）。
- **改法**：清单补齐这 5 枚（`gear` 不必列，见 B2）。

---

## B 组 · 保真细节（会动视觉，建议逐条确认）

### B1　权限弹框结构与实机 #84 不一致
- **实机**：`ui/app.slint:8542-8640` + `src/app/sftp_callbacks.rs:786-813`（#84 可视化 chmod）——卡片 320×270、圆角 **radius-md 6px**、padding 18px、间距 10px、标题「修改文件权限」**fs-lg 15px 无图标**、结构为**三组（所有者/组/其他）各含 读取/写入/执行 三个 CheckBox**。
- **稿内**：`.dlg-perm` 是 **3 列平铺网格**（`所有者 读 / 所有者 写 / 所有者 执行 …` 逐格标签）+ 图标标题 18px + 卡片 380px。
- **取舍**：v8「全部弹框同款化 = ConfirmDialog」必然带来这个差异。落地时三套弹框（ConfirmDialog / #69 prompt / #84 chmod）**规格互不相同**，要么稿内如实画三套，要么先把实机收敛成 ConfirmDialog 一套。

### B2　11 枚符号定义了但从未引用
- **清单**：`check` / `checkbox` / `checkbox-blank` / `checkbox-ind` / `expand-less` / `gear` / `info` / `minus` / `pause` / `play` / `shield-alert` / `trash`
- **两处真实缺口**（不只是冗余）：
  - **队列行没有暂停/继续**：`.tn-row` 只画了 `ico('tunnel')`，`pause`/`play` 从未出现 → v8 卡片「pause 是落地需新增资产」的前提（队列可暂停）在稿内其实没画出来。
  - **树节点只有展开态**：`expand-less` 未用，树的收起/展开没有反向箭头。
  - `checkbox` 三兄弟未用：文件行的勾选框是 CSS 自绘（`.ck`），不是项目 `checkbox.svg` 资产 → 落地时图标清单要按「实际用到的」裁。
  - `gear` 未用：列设置按钮实际用的是 `#i-tune`（`sftp_panel.slint:1446`），与项目一致 ✓，`gear` 纯冗余。

### B3　ConfirmDialog 细节两处
- **卡片不可拖拽**：实机支持**空白区拖动卡片**且**每次打开回中**（`ui/confirm_dialog.slint:97-109`，`#dialog-drag`）；稿内 `.modal-card` 既不可拖也不回中（`openModal()` 只切 `wide`/`open` 两个 class）。
- **Danger 圆角**：实机 `DangerButton` 圆角 = radius-sm **4px**（`ui/widgets.slint`，DangerButton 自带 `border-radius: Theme.radius-sm`），Primary/Ghost = 6px；稿内 `.modal-card .btn` 一律 6px，而稿内自己的注释写着「Danger 圆角 4px」——**注释与 CSS 不一致**，改 1 行 CSS 即可。
- （**右上 X**：实机 ConfirmDialog 本来就没有，稿内 `dlgHead()` 也只画图标 + 标题、无 X，关闭靠「取消」或点遮罩 —— **这一项与实机一致 ✓**，v8 卡片第 782 行「无 X 关闭键」的说法属实，**不是缺陷**。）

### B4　遮罩来源
- 实机所有弹框统一走 `Scrim` 组件（9 种样式 + 随机重掷，`ui/widgets.slint`）；稿内是单一 `rgba` 遮罩。落地说明已提「遮罩点击 = 取消」，但遮罩**外观**来源建议在稿内注明对应 `Scrim`。

### B5　应用面用了非令牌字号
- 项目令牌（`ui/theme.slint:467-472`）：fs-2xs 10 / fs-xs 11 / fs-sm 13 / fs-md 13 / fs-lg 15 / fs-xl 18。
- 稿内应用面出现：**12px ×17、12.5px ×9、11.5px ×17、10.5px ×6**（如 `.f-row` 12.5px、`.ctx-item` 12px、`.ctx-item .ks` 10.5px、`.st-result` 11.5px、`.tb-seg button` 12px、`.tn-row .st` 11.5px）。
- （文档页自身的 14px / 22px 不算，那是稿外排版。）
- **取舍**：归令牌 = 落地零歧义，但会改变列表密度（12.5→13 是最接近的一次改动）；也可以在稿内声明「弹出窗密排档，字号收紧一档 = 稿内新增」。

### B6　「隐藏文件开关」在实机不存在
- 稿内 `hiddenBtn`（隐藏文件过滤）；实机 grep `hidden` 只有「面板隐藏/折叠」语义，**没有 dotfile 开关**。同理，稿内**搜索过滤条**、**文件类型着色**（实机只有 `folder`/`file` 两枚图标 + accent/secondary 染色，`sftp_panel.slint:1236-1237`）实机都没有 → 属新增，见 C 组。

---

## C 组 · 实机现状 ↔ 稿内新增（落地工作量总表）

| 稿内能力 | 实机 | 证据 | 落地性质 |
|---|---|---|---|
| 工具栏 34px / 菜单 26px / 列头 22px | 有 | `sftp_panel.slint:314/54/971` | 沿用 |
| 列显隐齿轮 + 列头排序 + 清除排序 | 有 | `sftp_panel.slint:1067-1123` | 沿用 |
| 上传 / 刷新 / 下载选中 / 打包下载 / 逐个下载 / 删除选中 / 更多（复制路径·粘贴并前往·刷新）/ 父目录 | 有 | `sftp_panel.slint:450-615` | 沿用 |
| 行右键全套（查看·编辑·外部打开·外部编辑·重命名·权限·复制路径·复制到会话·删除·新建文件夹·新建文件·刷新） | 有 | `sftp_panel.slint:1396-1422` | 沿用 |
| 权限可视化弹框 | 有（#84） | `app.slint:8542` + `sftp_callbacks.rs:786` | 沿用（结构差异见 B1） |
| 重命名 / 新建文件夹 / 新建文件 提示弹框 | 有（#69） | `app.slint:8025` + `sftp_callbacks.rs:735` | 沿用（结构差异见 B1） |
| 下载目录设置 | 有，但**只在设置页** | `app.slint:6780-6825` + `app.rs:6376-6403` | 能力沿用，**入口新增**（稿内提到状态栏 chip） |
| 下载同名冲突处理 | 有，但是 **rfd 原生对话框** | `sftp_callbacks.rs:3` / `:47` | **交互替换**（稿内改应用内弹框） |
| 批量操作整行工具条 | 部分 | 实机是工具栏上「下载选中 N / 删除选中 N」两钮 | 交互改造 |
| 状态栏结果槽 | **无出口** | `sftp_panel.slint:1602` 注释：`sftp_status 仍在发但 nothing displays it here` | 新增（稿已注明） |
| 隐藏文件开关 | 无 | grep 无 dotfile 开关 | 新增 |
| 搜索过滤条 | 无 | grep `search` 无命中 | 新增 |
| 文件类型着色 | 无 | `sftp_panel.slint:1236-1237` 仅 folder/file | 新增 |
| 书签 | 无 | 稿内 `bmBtn` | 新增 |
| 拖拽行 → 树移动 | 无 | 稿内 dragstart | 新增 |
| 拖出窗口外 = 下载 | 无 | 落地需系统拖放（`DoDragDrop`），**可能需 vendored patch** | 新增（风险最高） |
| 隧道页（文件/隧道双标签） | 无 | 项目有 `src/tunnel/`，但面板内无此页 | 新增 |
| 上传冲突弹框（逐项跳过/覆盖） | 无 | 稿内 v6 | 新增 |
| 弹出独立窗口本身 | 无 | 落地需新 `sftp_window.slint`（稿内落地映射卡已列） | 新增（主体工作量） |

---

## D 组 · 待拍板（D1 已定）

- **D1　文件行高** → **已定：保持稿内 28px**，声明里注明「实机 24px，稿内 28px = 弹出窗多列信息表行高（稿内新增）」。
- **D2　三类弹框：稿内统一 ConfirmDialog 规格 vs 照实机各画各的**（见 B1）。
- **D3　（已核实，无需拍板）弹出窗弹框无 X 关闭键**：实机 ConfirmDialog 无 X，稿内也无 X（`dlgHead()` 只画图标 + 标题），两者一致 ✓。真正待办是 B3 的可拖拽 / 圆角两处。
- **D4　亮色令牌是否校正为 `mterm-light`**（见 A4）。
- **D5　应用面字号是否归令牌**（见 B5）。
- **D6　`pause.svg` 资产**：项目没有，落地需新增 `assets/icons/pause.svg` + 注册进 `ui/icons.slint`；**且队列页要先在稿内补出暂停/继续交互**（见 B2）。

---

## 核对通过 · 不需要动

- **图标几何**：53 枚符号中 51 枚与 `assets/icons/*.svg` 的 path 几何**逐字一致**（脚本去 svg 外壳 + 压空白后比对，差 0 处）；另 2 枚即 A2 的 `minus` 与项目暂缺的 `pause`。
- **实机引用**：稿内 20+ 处「文件:行号」引用**逐条回查通过** —— `sftp_callbacks.rs:3/:47/:112`、`sftp/impls/sftp.rs:2748`、`app.slint:6780-6825`、`:9953`、`app.rs:6376-6403`、`sftp_panel.slint:970-1000/:611/:585-590/:1602`、`confirm_dialog.slint:7/:37`、`widgets.slint:98`、`theme.slint`、`palettes.rs`、`icons.slint`、`panel_anim.slint`、`find_bar.slint` 全部命中且内容对得上。
- **暗色令牌**：`mterm-dark` 12 个槽位（root/panel/palt/elev/hov/act/line/lstr/t1/t2/t3/ac）逐值一致 ✓；ok/wr/dg 一致 ✓；强调色三态（accent / accent-hover / accent-soft / accent-tonal / accent-line）派生与 `theme.slint:312-342` 一致 ✓。
- **圆角/间距**：radius 4/6/10/16 与 `theme.slint:555-558` 一致 ✓；gap 4/8/12/16 一致 ✓。
- **弹框骨架**：`.modal-card`（380px / popup-bg / border-strong / radius-lg / 24px 阴影）与 ConfirmDialog 一致 ✓；`.dlg-head` 图标 18px + 标题 18px/700 与实机一致 ✓；**无 X 关闭键、关闭走取消键或遮罩 = 与实机一致 ✓**；页脚 end 对齐 + 8px 间距一致 ✓；删除确认用 `.btn.danger`（红底白字）一致 ✓。
- **运行时**：浏览器实跑**零控制台报错**；已验证链路 = 弹出/收回、右键六场景、行右键 14 项、权限弹框、新建文件夹/新建文件弹框、搜索过滤（12→1 行）、隐藏文件切换、队列页、隧道页、下载设置弹框、列头齿轮菜单。
