# 两个侧栏收展样式 · 规格书 v8

> 状态：rev 8（2026-11-04）｜高保真基准：`docs/design/two-sidebar-hifi-2026-11-04.html`（2026-11-04 定稿，按侧预设 + 单组滑块）
> 范围：只评审「设置里如何配置左右侧栏的收展样式」，不评审 D1/D2 已实现入口（见附录 C）。
> 本方案不改代码；后续改动点清单见 §6.4。

## 0. 口径与结论

### 0.1 术语

| 术语 | 含义 |
| --- | --- |
| 侧栏收展 | 左栏、右栏从展开宽度收到 0，或从 0 展开到目标宽度 |
| 收展形态 | 收展过程中采用的空间/时间编排，共 8 种（§2.3） |
| 编排系 | 推挤、吞没、手风琴、归巢；改变布局或行级几何 |
| 力感系 | 果冻；用 spring 过冲制造回弹 |
| 伪 3D/滤镜系 | 翻页、溶解；Slint 1.18 不能真实表达，只做高保真演示 |
| `panel_anim_*` | 本次新增的 7 个设置键（§0.2） |
| 作用对象 | 设置页当前指向的栏：左栏 / 右栏 / 左右；是界面状态，不是配置键 |
| 形态滑块 | 一条 8 档离散滑块，取代左右各一套形态按钮 |

### 0.2 七个配置键

| 键 | Rust 类型 | 默认 | 范围/取值 | 作用 |
| --- | --- | --- | --- | --- |
| `panel_anim_left` | `String` | `"push"` | 8 个形态 id | 左栏形态 |
| `panel_anim_right` | `String` | `"push"` | 8 个形态 id | 右栏形态 |
| `panel_anim_ms_left` | `u16` | `240` | `80..=600` | 左栏收/展时长（毫秒） |
| `panel_anim_ms_right` | `u16` | `240` | `80..=600` | 右栏收/展时长（毫秒） |
| `panel_anim_easing_left` | `String` | `"smooth"` | `standard`/`smooth`/`spring` | 左栏缓动档位 |
| `panel_anim_easing_right` | `String` | `"smooth"` | `standard`/`smooth`/`spring` | 右栏缓动档位 |
| `panel_anim_reduced_motion` | `bool` | `true` | 开/关 | 跟随系统「减少动效」；开启且系统要求时降级为瞬时 |

形态 id 固定为：`instant`、`push`、`swallow`、`accordion`、`homing`、`jelly`、`flip`、`dissolve`。前 6 个可落地，后 2 个仅演示。左右各有一套形态、时长、缓动；减少动效为两侧共用。

**不再有 `panel_anim_sync`**：设置页的「作用对象」（左栏 / 右栏 / 左右）取代它，只作为界面状态、不落盘。旧配置迁移：旧单值 `panel_anim_ms` / `panel_anim_easing` 读作左右同值；旧同步键忽略，加载后作用对象默认「左右」。

### 0.3 行号基准

`ui/app.slint` 当前工作树 9,687 行 / 569,655 B（2026-11-04 14:00 核对）；该文件含并行 Ping 任务未提交的 +8 行改动，导致 1376 行之后的行号整体后移 1–8 行；本文 `file:line` 按当前工作树写，若该并行改动被回退，行号需整体前移。`ui/interface_panel.slint`、`ui/left_sidebar.slint`、`ui/right_sidebar.slint`、`ui/settings_dialog.slint`、`src/app.rs` 本轮无漂移。文内所有 `file:line` 以该基准为准；若实现时文件再变动，以符号名回查为准。

### 0.4 一句话结论

- 八形态中 6 种可选：瞬时、推挤、吞没、手风琴、归巢、果冻；翻页、溶解仅演示，设置页置灰。
- **产品默认＝「顺滑」预设**（推挤 + 240ms + `smooth`，左右都是这一组）；首启读到的默认值就是这个组合。预设只是参数组合的命名，不新增配置键。
- **预设可分别套到左栏、右栏或两侧**；形态只有一组控件，通过「作用对象」决定当前编辑哪一侧。左右可以各自挂不同预设，不再要求两侧一致。
- 设置页只增加「收展动效」一组，不新增第二个入口；作用对象默认「左右」。
- 两个工程坑（首帧误播、动画期间 snap）必须在实现层解决，不交给用户设置。

---

## 1. 现况与阻塞

### 1.1 三块面板，当前都没有 animate

| 面板 | 实例/尺寸 | 当前收展方式 |
| --- | --- | --- |
| 左栏 `LeftSidebar` | `ui/left_sidebar.slint:855`；宽 240px（`ui/app.slint:1700`） | `if !root.left-sidebar-collapsed` 条件挂载（`ui/app.slint:2870`），瞬时跳变 |
| 右栏 `RightSidebar` | `ui/right_sidebar.slint:464`；宽 220px、高 240px（`ui/app.slint:1109-1110`） | 条件挂载（`ui/app.slint:3129`），**故意不使用 animate** |
| 快捷面板 `QuickDockPanel` | `ui/terminal_view.slint:890`，实例 `ui/app.slint:3941`；右 260px、高 220px | 条件挂载，瞬时跳变 |

右栏那条注释是本次设计最重要的现成教训，原注释大意（`ui/app.slint:3145-3150`）：

> 右栏故意不使用 `animate`：Slint 的 `animate` 在元素首次布局时也会播（0→目标），右侧面板每次被重建/重排都会「从上往下长出来」（`#right-panel-no-anim 2026-10-18`）。

也就是说，今天不是「没想到做动画」，而是「直接加 `animate` 会坏」。§4.1 的首帧门卫就是为它补条件。

### 1.2 三个结构性阻塞

1. **条件挂载**：`if !collapsed` 一旦变 false 就卸载，没有「先播完再卸载」的中间态。必须把 `collapsed`（用户意图）和 `mounted`（视觉是否还在）拆开。
2. **宽度同时是布局输入**：`left-sidebar-size`（`ui/app.slint:2798`）和 `right-sidebar` 的 `sb-w`（`ui/app.slint:3094-3101`）直接参与终端区宽度计算；动画期间宽度逐帧变化，PTY resize 不能逐帧发。
3. **36px 残留**：左栏 `left-sidebar-taken` 仍有 `left-sidebar-collapsed ? 36px : left-sidebar-width`（`ui/app.slint:1705-1706`）；右栏、快捷面板同批残留（`ui/app.slint:2816-2819`、`src/app.rs:7398-7402`/`7414-7415`/`7438-7439`）。用户已确认折叠态是**完全隐藏**，这些 36px 必须先清零，否则任何「收到底」的动画都会停在一条空条上。

### 1.3 可复用资产

| 资产 | 位置 | 用途 |
| --- | --- | --- |
| `SettingRow` | `ui/interface_panel.slint:108` | 设置行骨架：`label`/`desc`/`show-divider`/`@children` |
| `Switch` | `ui/interface_panel.slint:137` | 38×22 开关，`on`/`toggled(bool)`，自带 `animate x` |
| `SegGroup` | `ui/seg.slint:15` | 三档缓动的分段控件；`active-index`/`count`/`compact`/`enabled` |
| `MTermSlider` | `ui/widgets.slint:851` | 时长滑杆；`minimum`/`maximum`/`default-value`/`value`/`changed`/`released`/`reset` |
| `SectionHeader` | `ui/interface_panel.slint:99` | 「收展动效」组标题 |
| `DirChip` 视觉参考 | `ui/storage_settings.slint:121` | 仓库没有通用 Chip 导出，预设胶囊可照此风格新建局部组件（形态改用滑块） |
| 侧栏设置页 | `ui/interface_panel.slint:2359` | 新分组直接落在现有 4 行下方，页尾 `2406` |

---

## 2. 收展形态规格（主章）

D1/D2 是现状基线（左上 LOGO 双态、右上常显开关、⋯ → 设置弹框 → 侧栏页），已实现，不在本章展开；细节见附录 C。本章只评审「收起来长什么样」。

### 2.1 状态模型：每侧一个控制器

左右各一套，互不共享计时器。以左栏为例，右栏把 `left` 换成 `right` 即可。

| 状态/属性 | 类型 | 含义 |
| --- | --- | --- |
| `panel-mounted` | bool | 视觉容器是否还在树上；收起动画结束才置 false |
| `panel-phase` | enum | `expanded` / `collapsing` / `collapsed` / `expanding` |
| `panel-target` | length | 目标宽度：展开 240px（右 220px），收起 0px |
| `panel-width` | length | 当前渲染宽度；动画改它，布局读它 |
| `panel-animating` | bool | 是否有动画在跑；用于停 snap、抑制重复点击 |
| `panel-generation` | int | 每次用户意图 +1；落定回调只接受最新一代 |
| `panel-settle-timer` | Timer | 非 spring 形态的兜底落定；spring 另用阈值判据 |

**状态迁移**

| 从 | 事件 | 到 | 动作 |
| --- | --- | --- | --- |
| expanded | 收起 | collapsing | `target=0`、`animating=true`、启动落定计时 |
| collapsing | 展开 | expanding | `target=240`、保持 mounted、同一次动画反向，不重挂载 |
| collapsing | 落定 | collapsed | `mounted=false`、`animating=false`、提交一次 PTY resize |
| collapsed | 展开 | expanding | `mounted=true`、先按 0 宽挂载、下一帧再设 target=240 |
| expanding | 收起 | collapsing | `target=0`、保持 mounted |
| expanding | 落定 | expanded | `animating=false`、提交一次 PTY resize |

**瞬时形态**跳过中间态：收起直接 `mounted=false`，展开直接 `mounted=true`、`phase=expanded`；不创建计时器。

**宽度来源**：左栏展开宽固定 240px（`ui/app.slint:1700`）；右栏展开宽 220px、高 240px（`ui/app.slint:1109-1110`）。右栏如果是 top/bottom 停靠，收展改的是高，不是宽；本规格的形态默认按左右停靠写，top/bottom 复用同一套「沿主轴收」的逻辑，方向由 dock 决定。

**布局口径**：`panel-width` 同时是布局宽度。左栏用 `left-sidebar-size`（`ui/app.slint:2798`）替换为 `panel-mounted ? panel-width : 0px`；`left-sidebar-taken`（`ui/app.slint:2799`）同步。右栏用 `sb-w`（`ui/app.slint:3094-3101`）的同一位置读取 `panel-width`，保留现有 `snap-to-device` 只在落定帧调用。

**PTY 口径**：动画期间不调用 PTY resize；落定帧只发一次。非 spring 形态若**超过设定时长 + 80ms**仍没有落定信号（掉帧），用 120ms 防抖兜底发一次；spring 形态改用 §4.3 的 `max(10×ms, 1200ms)` 阈值判据。终端最小宽度仍为 520px（附录 D），动画不得把可交互区域压到 520px 以下；若窗口本身不够，先收右栏再收左栏，不在动画中途改优先级。

### 2.2 八形态总表

| # | 形态 | 家族 | 机制一句话 | 感知 | 优点 | 代价 | 成本 | 可落地 | 推荐 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 瞬时 `instant` | 无过渡 | 条件挂载直接切 | 啪一下出现/消失 | 零成本、零故障面 | 没有空间连续感 | 易 | 可落地 | 减少动效兜底 |
| 2 | 推挤 `push` | 编排 | 外壳宽 240→0，内容随布局回流，终端被推 | 侧栏把终端推回去 | 实现最直接、空间关系真实 | 文字逐帧重排；PTY 需收敛 | 易 | 可落地 | **默认推荐** |
| 3 | 吞没 `swallow` | 编排 | 外壳宽 240→0，内层固定 240，`clip:true`，外缘扫过 | 内容原地被扫掉 | 不逐帧重排，比推挤稳 | 多一层 wrapper；要处理 1px 压缝 | 中 | 可落地 | 推荐备选 |
| 4 | 手风琴 `accordion` | 编排 | 每行高度/纵向缩放 1→0，逐行延迟，宽度最后收 | 像折扇合上 | 层次强、有「收」的动作感 | 行数多时总时长难控 | 中 | 可落地 | 进阶 |
| 5 | 归巢 `homing` | 编排 | 每行向标题栏按钮方向位移+缩到 0.55+淡出 | 内容飞回按钮 | 有方向、和入口呼应 | 需要按钮坐标；逐行成本 | 中偏高 | 可落地 | 进阶 |
| 6 | 果冻 `jelly` | 力感 | 宽度 spring 过冲回弹 | Q 弹一下 | 活泼、有记忆点 | 过冲会挤终端；落定难判 | 中 | 可落地 | 可选 |
| 7 | 翻页 `flip` | 伪 3D | 视觉目标 rotateY(-72°) + 透视（仅演示） | 像门板翻过去 | 冲击力最强 | Slint 无 3D 变换 | 不可落地 | 仅演示 | 不推荐 |
| 8 | 溶解 `dissolve` | 滤镜 | 点阵/遮罩渐隐 | 像沙子散掉 | 最柔和 | Slint 无遮罩着色器 | 不可落地 | 仅演示 | 不推荐 |

成本四档：易 / 中 / 中偏高 / 不可落地；「中偏高」只出现在归巢（要行级坐标换算），翻页/溶解标为不可落地，仅用于高保真演示。

### 2.3 逐形态规格

> 以下片段用 Slint 1.18 语法表达实现方式，省略宿主绑定，不是完整可编译示例；变量名按实现替换。所有 `animate` 都必须带 §4.1 的首帧 `enabled` 门卫，片段为可读性省略。


#### 2.3.1 瞬时 `instant`

```slint
if root.panel-mounted : Rectangle {
    width: root.panel-width;   // 0 或 240px，不写 animate
    // 条件切换即完成；不建 Timer，不碰 snap
}
```

- **机制/感知**：`panel-mounted` 直接跟随用户意图；收起卸载，展开按目标宽度挂载；没有过渡，但有明确的「在/不在」。
- **优点/代价**：零动画成本；不会误播；天然适合减少动效；没有空间连续感；快速连点时视觉会闪。
- **落地/推荐**：保留现有 `if !collapsed` 的语义即可，只需把条件换成 `mounted`；作为「减少动效」的最终兜底；不做成默认。

#### 2.3.2 推挤 `push`

```slint
Rectangle {
    width: root.panel-width;   // 240px → 0px
    animate width { duration: root.panel-anim-duration; easing: cubic-bezier(0.25, 1, 0.3, 1); }
    // 内容随布局逐帧回流，终端被推挤
}
```

- **机制/感知**：外壳宽度动画，内部布局实时重排，终端区被同步推挤/让出；侧栏像抽屉一样把终端推回去；空间关系最直白。
- **优点/代价**：代码最少；和现有 `left-sidebar-size`/`sb-w` 的布局模型一致；任何内容都能用；文字和列表逐帧重排，长列表会抖；PTY resize 必须压到落定帧。
- **落地/推荐**：把 §2.1 的 `panel-width` 接到 `left-sidebar-size`/`sb-w`；`animate width` 即可；**默认推荐**。240ms + `smooth`（cubic-bezier 0.25,1,0.3,1）是「跟手但不拖」的平衡点；不建议超过 300ms。

#### 2.3.3 吞没 `swallow`

```slint
Rectangle {                              // 外壳：只动 width
    width: root.panel-width; clip: true;
    Rectangle { width: root.panel-expanded-width; x: 0; }  // 左栏贴左；右栏贴右
}
```

- **机制/感知**：外壳宽度 240→0，内层保持 240 并靠外缘锚定，`clip:true` 把内容原地裁掉；可在裁切边缘加 2px 扫描高光；内容不是被「压扁」，而是被「扫掉」；比推挤更稳。
- **优点/代价**：内层不重排，文字不逐帧换行；长列表成本低；视觉干净；多一层 wrapper；右栏要靠右锚定；现有 `clip:false` 的 1px 压缝（`ui/app.slint:2890`）会被外层 `clip` 裁掉，需在内层保留 1phx 外扩补偿。
- **落地/推荐**：左栏外层 `clip:true`、内层固定 `left-sidebar-width`；右栏同理，`x: parent.width - self.width`；**推荐备选**；适合右侧资源栏这种「内容重、但不想抖」的面板。

#### 2.3.4 手风琴 `accordion`

```slint
for row[i] in root.rows : Rectangle {
    height: root.row-open ? row.height : 0px;
    animate height { duration: root.row-duration; delay: i * 18ms; easing: ease-in; }
}
```

- **机制/感知**：每行高度或 `transform-scale-y` 从 1→0，行间 `delay: 18ms * i` 逐行折；宽度最后收；展开逆序；像折扇/手风琴合上，最后整块宽度消失。
- **优点/代价**：层次感最强；适合行结构清晰的左栏（会话、文件树）；每行都要动画，行数多时掉帧；总时长受行数影响，必须封顶。
- **落地/推荐**：行数 ≤ 12 时可用；超过 12 行降级为推挤；不要为每行建独立 Timer，用循环索引统一算延迟（`i * 18ms`）；进阶形态，默认不选；左栏内容简单时可作为「活泼」预设的备选。

#### 2.3.5 归巢 `homing`

```slint
Rectangle {
    x: root.row-away ? 12px : 0px;
    opacity: root.row-away ? 0.0 : 1.0;
    animate x { duration: root.row-duration; delay: i * 18ms; }
    animate opacity { duration: root.row-duration; delay: i * 18ms; }
}
```

- **机制/感知**：每行向标题栏按钮方向位移、缩到 55%（2D `Transform` 的 `transform-scale-x/y: 55%`，`builtin_elements.rs:979-983`；高保真取 0.55，brief 原建议 0.6，以高保真为准）、同时淡出；行间 `delay: 18ms * i`；内容「飞回」左上/右上按钮，和入口有因果关系。
- **优点/代价**：方向性最强；能暗示「从哪来回哪去」；需要按钮中心坐标；逐行成本；飞出面板边界时必须由外层 `clip:true` 裁住。
- **落地/推荐**：按钮坐标可由 `BrandLogoBtn`/`rb-toggle-ta` 的全局位置换算；先做左栏，右栏坐标随窗口宽度变化，需在 `changed width` 时重算；进阶形态；适合作为「电影感」预设的备选，不作为默认。

#### 2.3.6 果冻 `jelly`

```slint
Rectangle {
    width: root.panel-width;
    animate width {
        duration: root.panel-anim-duration;
        easing: spring(0.8);
    }
}
```

- **机制/感知**：宽度用 `spring(0.8)`（或仓库已有的 `spring(0.35)`，`ui/left_sidebar.slint:340`/`:727`）做过冲回弹；收到底会「弹一下」，展开会「胀一下」。
- **优点/代价**：有生命力；实现只换 easing，成本低；过冲会临时挤压终端；spring 的结束由内部 settled 决定，落定判据复杂（§4.3）；减少动效时必须禁用。
- **落地/推荐**：见 §4.3；过冲夹在 ±8px，且不得让终端窄于 520px，否则该侧自动退化为推挤；可选；只建议单侧使用，不建议左右同时果冻。

#### 2.3.7 翻页 `flip`（仅演示）

```slint
// Slint 1.18 的 Transform 只有 2D：
Transform {
    transform-rotation: -8deg;   // 平面内旋转，不是 rotateY
}
// 真正的 rotateY(-72deg) + perspective 无法表达
```

- **机制/感知**：把侧栏当成门板，沿 Y 轴 3D 翻转；背面可见终端；最强冲击力，像翻书/关门。
- **优点/代价**：演示效果最好；适合对外展示；Slint 1.18 无 3D 变换；用 2D 旋转近似会变成「平面歪一下」，不是翻页；无透视、无背面。
- **落地/推荐**：**不可落地**。设置页置灰，标注「仅演示」；若未来 Slint 支持 3D，再评估；不推荐，不进入产品设置。

#### 2.3.8 溶解 `dissolve`（仅演示）

```slint
// 目标：点阵/径向遮罩渐隐（mask-image 类效果）
// Slint 1.18：无遮罩、无自定义着色器
// 可做的只有整块 opacity 渐隐，不是「溶解」
opacity: root.panel-visible ? 1.0 : 0.0;
animate opacity { duration: root.panel-anim-duration; }
```

- **机制/感知**：用点阵或径向遮罩让侧栏像沙粒一样散掉；最柔和，没有「推/折」的机械感。
- **优点/代价**：演示氛围好；适合宣传片；Slint 无遮罩着色器；整块 opacity 只是淡出，和溶解不是一回事；逐像素成本也不适合实时窗口。
- **落地/推荐**：**不可落地**；设置页置灰，标注「仅演示」；不推荐，不进入产品设置。

### 2.4 四组预设

| 预设 | 形态 | 时长 | 缓动 | 套用范围 | 感觉 | 标记 |
| --- | --- | --- | --- | --- | --- | --- |
| 跟手 | 推挤 | 120ms | `standard` | 按当前作用对象 | 最短、最快，几乎不等 | 快速操作 |
| 顺滑 | 推挤 | 240ms | `smooth` | 按当前作用对象 | 稳、跟手、不拖 | **默认** |
| 电影感 | 吞没 | 320ms | `smooth` | 按当前作用对象 | 内容被扫走，画面不抖 | 大屏/演示 |
| 活泼 | 果冻 | 380ms | `spring` | 按当前作用对象 | 弹一下，有性格 | 个人化 |

- **默认＝顺滑**：首启即 `panel_anim_left="push"`、`panel_anim_right="push"`、`panel_anim_ms_left=240`、`panel_anim_ms_right=240`、`panel_anim_easing_left="smooth"`、`panel_anim_easing_right="smooth"`；作用对象默认「左右」。默认值就是这组参数，不存在另一套“推荐值”。
- **点预设＝按目标整组写入**：作用对象为「左栏」只写左栏三键，「右栏」只写右栏三键，「左右」同时写两侧。一次点击写完形态、时长、缓动三组参数，不写 `panel_anim_reduced_motion`；「当前：…」文案同步更新。
- **预设选中态不落盘**：由目标侧的形态、时长、缓动反推。目标为单侧时，该侧完全匹配就高亮；目标为「左右」时，两侧都匹配才高亮，只有一侧匹配则卡片半亮（1/2 状态），两侧都不匹配则不高亮。用户手改任一参数立即转「自定义」，无胶囊高亮。预设只是参数组合的命名，不新增配置键。
- **预设不覆盖减少动效**：`panel_anim_reduced_motion` 保持用户值；系统要求减少动效时，任何预设都按瞬时播放。
- **果冻的 380ms 不是结束时间**：`spring` 以 `duration` 为自然频率，实际落定常见 1×–2×，上限约 10×（§4.3）。高保真预览按 380ms 播放；落地实测若明显超过 1.2s，把「活泼」降到 300ms，并保留 `spring`。

### 2.5 左右不同形态是否冲突

允许，但要有约束：

| 左 | 右 | 结论 |
| --- | --- | --- |
| 推挤 | 推挤 | 最稳，默认 |
| 推挤 | 吞没 | 推荐组合：左栏轻、右栏重 |
| 吞没 | 吞没 | 可，画面最干净 |
| 手风琴/归巢 | 任意 | 仅建议目标单侧使用；两侧都逐行会卡 |
| 果冻 | 果冻 | 不推荐：两侧同时过冲，终端会「晃」 |
| 果冻 | 任意非果冻 | 可，但只建议大屏场景；窄窗自动退化 |
| 任意 | 翻页/溶解 | 不允许：置灰项不能被写入 |

运行时不联动：左栏收展开关只改左栏；右栏只改右栏。设置页的“联动”只发生在写入时：作用对象为「左右」时同一次点击写两侧，但两侧仍各自计时、各自落定；作用对象为单侧时只改该侧，另一侧完全不动。切换作用对象只切换编辑目标，不改任何已存值。

---

## 3. 设置页信息架构（IA）

### 3.1 入口与位置

沿用现状，不新增第二入口：

`⋯` 更多菜单 →「设置」→ `SettingsDialog`（默认落在「外观」页，`ui/app.slint:6130` 的 `ifd-page="theme"`）→ 左侧导航「侧栏」（`ui/interface_panel.slint:1217-1219`；Windows 上第 8 项，无 WSL 时第 7 项）→ 在「侧栏」页配置。

侧栏页容器在 `ui/interface_panel.slint:2359`，现有 4 行保留原位（`2365-2371`、`2372-2381`、`2382-2390`、`2392-2404`），新分组接在 `2404` 之后、页尾 `2406` 之前。不新增导航项，不新增「收展设置」二级弹框。

### 3.2 版面

| 项 | 值 | 依据 |
| --- | --- | --- |
| 设置弹框 | 960×640 基准；实际尺寸由 `ifd-w`/`ifd-h` 按窗口计算 | `ui/app.slint:1564-1565` |
| 左导航 | 168px | `ui/interface_panel.slint:1160` |
| 内容区 | 弹框宽 - 168px | 现状布局 |
| 内容内边距 | 左右 20px、上 14px | `ui/interface_panel.slint:2360-2362` |
| 设置行 | 标签列 + 控件列，行高随 `SettingRow` | `ui/interface_panel.slint:108` |
| 预览区 | 260×170（与高保真同尺寸；宽屏可放大到 300×210），右侧固定；窄于 760px 时移到分组底部 | 本规格新增 |
| 滚动 | 内容区纵向滚动；预览区在宽屏时 sticky | 本规格新增 |

控件映射：新分组挂在 `ui/interface_panel.slint:2359` 的侧栏页容器里，行骨架统一用 `SettingRow`（`:108`）；「作用对象」「缓动」用 `SegGroup`（`ui/seg.slint:15`）；「形态」「时长」用 `MTermSlider`（`ui/widgets.slint:851`），形态滑块在 `changed` 里用 `Math.round` 吸附 0–7；「减少动效」用 `Switch`（`:137`）；预设胶囊用局部 `Rectangle`+`Text` 自绘（视觉参考 `ui/storage_settings.slint:121` 的 `DirChip`），不新增全局控件文件。

> 高保真 mock 用 1000×660、左导航 240px 展示；落地以真实设置弹框为准：960×640 基准、导航 168px（`ui/interface_panel.slint:1160`）。

### 3.3 控件与配置键对应

| 分组/行 | 控件 | 配置键/目标 | 默认 | 说明 |
| --- | --- | --- | --- | --- |
| 现有 4 行 | 原控件 | `collapse_right_sidebar_default`、`zen_mode`、`collapse_sftp_default`、`quick_commands_as_sidebar` | 原默认 | **保留原位**，不改语义 |
| 作用对象 | `SegGroup` 三档 | 界面状态 `target`（不落盘） | 左右 | 左栏 / 右栏 / 左右；决定下面所有行写哪一侧；`#tgtSeg` |
| 当前状态 | 只读文本 | 由左右配置派生 | `左 顺滑 · 右 顺滑` | 两侧不同时第二名带色点；`#sideSummary` |
| 预设 | 4 个胶囊 + 右侧只读文案 | 写目标侧形态/时长/缓动 | 顺滑 | 第一个可操作行；胶囊为跟手 120 / 顺滑 240（「默认」角标）/ 电影感 320 / 活泼 380；点选整组写入目标；`#prCur` |
| 形态 | `MTermSlider` 8 档 | 目标侧 `panel_anim_left` / `panel_anim_right` | `push` | 0 吞没 … 7 瞬时；翻页/溶解置灰；拖动实时预览、释放写配置；`#shapeRange` |
| 形态说明 | 只读文本 + 徽标 | 不落盘 | — | 当前档一句机制 + 「可落地/仅演示」；目标=左右且两侧不同时前缀「左栏：」，只说明滑块所指的那一侧；`#shapeNow` |
| 左右不同 | 只读提示 | 不落盘 | — | 仅作用对象=左右且两侧不同时显示；`#shapeMix` |
| 时长 | `MTermSlider` + 数值 | 目标侧 `panel_anim_ms_left` / `panel_anim_ms_right` | `240` | 80–600ms，步进 10ms；作用对象=左右且两侧不同时显示 `—`、不可拖，行尾提示「左右不同，分别设置」 |
| 缓动 | `SegGroup` 三档 | 目标侧 `panel_anim_easing_left` / `panel_anim_easing_right` | `smooth` | 作用对象=左右且两侧不同：三段均不高亮，行尾同一提示 |
| 减少动效 | `Switch` | `panel_anim_reduced_motion` | `true` | 跟随系统；任何预设不覆盖 |
| 试播 | 按钮 | 不持久化 | — | 按作用对象播放；不写配置 |

**作用对象选择器**只改变“下面控件编辑哪一侧”，切换本身不写任何配置；默认「左右」。目标为单侧时，时长、缓动、形态都显示该侧真实值；目标为「左右」且两侧不同时，时长显示 `—`、缓动三段都不选中，必须先选单侧或直接改形态/预设把两侧拉回同一组。

预设行右侧固定一行只读文案，按作用对象取值：

- 目标=左栏：`左栏：顺滑（推挤 · 240ms · 顺滑）`；目标=右栏：`右栏：顺滑（推挤 · 240ms · 顺滑）`。
- 目标=左右且两侧相同：`当前：顺滑（推挤 · 240ms · 顺滑）`。
- 目标=左右且两侧不同：`左右不同（左 手风琴 · 右 果冻）`。
- 参数不匹配任何预设：`自定义（推挤 · 300ms · 弹性）`；目标为单侧时前缀「左栏：」或「右栏：」。文案只读，不写盘。

**预设半亮**：目标=左右且只有一侧匹配某预设时，该预设卡半亮（琥珀边框 + 浅琥珀底），并在标题后追加「1/2」小胶囊；全亮（两侧都匹配）时不显示「1/2」。点击半亮卡仍整组写入作用对象，把两侧拉回同一预设。每张卡底部文案随作用对象变「套用到左栏 / 右栏 / 左右」，角标（最短/慢/弹/默认）位置不变。

**形态混合态**：目标=左右且左右形态不同时，滑块 thumb 停在左栏档位，右栏档位在刻度名上以琥珀色标出；`#shapeMix` 显示「左右不同：左 手风琴 · 右 果冻」。拖动滑块或点预设会同时写两侧，混合态清除。

### 3.4 形态滑块刻度与配置值

滑块固定 8 档，顺序与高保真一致；档位序号即滑块的 0–7：

| 档位 | 形态 | 写入值 | 可点 | 说明 |
| --- | --- | --- | --- | --- |
| 0 | 吞没 | `swallow` | 是 | 内容原地被扫掉，布局最稳 |
| 1 | 手风琴 | `accordion` | 是 | 逐行折，行数多时降级 |
| 2 | 归巢 | `homing` | 是 | 向标题栏按钮方向收拢 |
| 3 | 翻页 | `flip` | 否 | 仅演示，置灰并标「仅演示」 |
| 4 | 溶解 | `dissolve` | 否 | 仅演示，置灰并标「仅演示」 |
| 5 | 果冻 | `jelly` | 是 | spring 过冲，需落定判据 |
| 6 | 推挤 | `push` | 是 | 默认；宽度带动布局回流 |
| 7 | 瞬时 | `instant` | 是 | 减少动效兜底，无过渡 |

- `MTermSlider` 连续拖动，`changed` 中先 `Math.round(value)` 吸附到 0–7；拖动过程只更新预览与当前档高亮，`released` 才写目标侧配置。吸附到置灰档（3/4）时落到相邻可用档（2 归巢或 5 果冻），不能停住。
- 当前档刻度名加粗、发蓝；左右方向键 ±1，Home/End 跳首尾，置灰档跳过；键盘操作同样在释放/确认时写配置。
- 置灰档必须带 `Tooltip`：中文「当前渲染层不支持 3D / 遮罩，仅高保真演示」；英文「Not supported by the current renderer; preview only.」
- 无障碍：滑块的 `accessible-name` 用「左栏形态：推挤」这种完整短语，并读出「第 7 档，共 8 档」；不要只读「推挤」。
- 混合态按 §3.3 处理；切换到单侧作用对象后，滑块显示该侧档位，才可单独改一侧。

### 3.5 中英文案

| 位置 | 中文 | English |
| --- | --- | --- |
| 分组标题 | 收展动效 | Panel motion |
| 作用对象 | 作用对象 | Applies to |
| 左栏 / 右栏 / 左右 | 左栏 / 右栏 / 左右 | Left / Right / Both |
| 当前状态 | 当前状态 | Current |
| 预设 | 预设 | Presets |
| 形态 | 形态 | Shape |
| 左右不同 | 左右不同 | Left and right differ |
| 自定义 | 自定义 | Custom |
| 时长 | 时长 | Duration |
| 缓动 | 缓动 | Easing |
| 减少动效 | 减少动效 | Reduce Motion |
| 试播 | 试播 | Preview |
| 置灰提示 | 仅演示，当前渲染层不支持 | Preview only; not supported by the current renderer |

「减少动效」行右侧的说明文案为「跟随系统」；开关本身仍叫「减少动效」，不新增独立设置项。

### 3.6 预览区

- 260×170 的窗口缩略图（高保真 `#previewWin` 同尺寸），左侧/右侧各一块迷你面板；按当前作用对象播放收→开→收一轮；作用对象=左右时两侧按各自参数同时播放，目标侧外框高亮。
- 播放顺序：展开目标侧 `panel_anim_ms_left/right`（默认 240ms）→ 停留 300ms → 收起同一时长；总时长不超过 `2×600+300=1500ms`，UI 上限 2.5s。
- `减少动效` 生效时，预览也走瞬时，并在预览角标显示「减少动效：瞬时」。
- 预览播放本身只创建迷你 Rectangle，不创建真实 `LeftSidebar`/`RightSidebar`，不触发 PTY resize，不写配置；只有用户点「重置」会套用预设。
- 连续改时长/缓动时 150ms 防抖，只保留最后一次；试播按钮可强制重播。
- 预览同样需要首帧门卫（§4.1），否则第一次显示会「长出来」。
- 高保真预览卡里的「重置」＝回到顺滑预设（推挤 · 240ms · 顺滑），按当前作用对象写入，走与点预设相同的写入路径；作用对象=左右时两侧同时回顺滑。设置页不新增第二个重置入口。

### 3.7 约束

- **只此一页**：所有收展样式配置都在「侧栏」页；不新增顶部工具条、不新增侧栏头部齿轮、不新增标题栏独立设置图标。
- **作用对象只改编辑目标**：`target` 是设置页局部状态，不落盘；默认「左右」；作用对象=左右时写入两侧，单侧时只写该侧。没有同步键，切换作用对象不改任何已存值。
- **首启读默认，不额外写盘**：首启读到的默认值就是顺滑组合（左右都是 `push` / 240ms / `smooth`，`panel_anim_reduced_motion=true`，作用对象=左右）；只有用户改任一控件（预设 / 形态滑块 / 时长 / 缓动 / 减少动效）才写盘。
- **预设选中态是派生值，不写盘**：按目标侧反推；单侧完全匹配才高亮，目标=左右时两侧都匹配才高亮、只有一侧匹配半亮，否则「自定义」。用户手改任一参数立即转自定义。
- **持久化**：改动即时写 cache，沿用设置页现有保存流程；试播不持久化。
- **不新增设置页重置按钮**：预览卡的「重置」等价于点「顺滑」预设，不是设置页级重置。

---

## 4. 实现注意（两个坑 + 三个边界）

### 4.1 坑一：首帧误播

**现象**：`animate` 在元素第一次布局时也会从「默认值」动画到「绑定值」；右栏注释 `ui/app.slint:3145-3150` 记录的 `#right-panel-no-anim` 就是这个问题：面板重新挂载时宽度从 0 长到 220，seen-sidebar 误报。

**做法**：

1. 每一侧增加 `panel-anim-ready: bool`，初值 `false`。
2. 所有 `animate` 写 `enabled: root.panel-anim-ready;`；`PropertyAnimation` 支持 `enabled`（`i-slint-compiler-1.18.1/builtin_elements.rs:3568-3578`）。
3. 首帧后再置 `true`：用 `Timer { interval: 0ms; running: true; }` 或 `init` 后等一次 `root.width > 0`，两者都必须在布局完成后触发；不要在 `init` 里直接置 true。
4. 预览区有自己的 `preview-anim-ready`，不复用主面板的。
5. 瞬时形态不依赖门卫，但保留同一属性，避免以后切换形态时漏掉。

**反例**：只在 `if !collapsed` 外层加 `animate`，第一次打开仍会播；因为挂载那一帧就是 0→目标。门卫必须覆盖「挂载后的第一次布局」。

### 4.2 坑二：动画期间 snap

**现象**：右栏宽度来自拖拽浮点，`sb-w` 在 `ui/app.slint:3094-3101` 用 `Theme.snap-to-device` 把边界吸到设备像素；125%/150% 缩放下，动画中途的宽度如果逐帧 snap，会在半像素边界闪 1px 白缝（`#sb-edge-snap` 注释 `ui/app.slint:3086-3091`）。

**做法**：

1. 动画期间 `panel-animating=true`，`sb-w` 直接返回 `panel-width`，**不调用** `snap-to-device`。
2. 落定帧先算 `panel-target`，再调用一次 `Theme.snap-to-device`；左/上停靠取整宽度，右/下停靠按「窗口内侧 - 宽度」取整（沿用 3090-3091 的规则）。
3. 左栏同样处理：`left-sidebar-size` 动画期间用未取整的 `panel-width`，落定帧再取整；`left-sidebar-taken` 只读落定值。
4. 预览区不需要 snap（没有真实终端边界），但保留同一接口，避免两套代码。

### 4.3 果冻的落定判据

`spring()` 是物理弹簧：`duration` 只定自然频率，动画**跑到系统判定 settled 才结束**，不是到 `duration` 就结束；Slint 1.18 在超过 `duration` 仍未 settle 时会把尾部重新阻尼，最长约 **10×`duration`**（`i-slint-core-1.18.1/animations.rs:342-349`、`properties/properties_animations.rs:212-264`）。

- 每 16ms 检查 `abs(panel-width - panel-target) < 0.5px`，连续 2 次即视为落定，停 Timer，强制写目标值。
- 兜底 Timer 用 `max(10 × panel_anim_ms_left/right, 1200ms)`（按目标侧取值），而不是 `duration`；超时未落定则强制写目标值并记一条 warn。
- 过冲夹在目标值 ±8px；如果 `panel-width` 会让终端窄于 520px，取消过冲，直接退化为 `ease-out` 推挤。
- 建议只在单侧启用果冻；`panel_anim_left/right` 都是 `jelly` 时，设置页提示「双侧果冻会晃动终端」，但不强制阻止（用户可选）。

### 4.4 挂载/卸载与快速反向

不能用 `if !collapsed` 直接控制视觉容器；用 `mounted`：

```slint
// 收起：先动画，落定后再卸载
if root.panel-mounted : Rectangle {
    width: root.panel-width;
    animate width { duration: root.panel-anim-duration; easing: ease-out; }
}
```

- **收起**：`panel-target=0`、`panel-animating=true`；落定后 `panel-mounted=false`。
- **展开**：先 `panel-mounted=true`、`panel-width=0`；下一帧再 `panel-target=240`，否则首帧就是 240，动画被跳过。
- **快速反向**：每次点击 `panel-generation++`；落定回调检查 `generation == latest`，旧回调直接丢弃。反向时只改 `target`，不重挂载、不重置 `width`。
- **用户意图与视觉分离**：`left-sidebar-collapsed`/`right-sidebar-collapsed` 仍在点击时立即翻转（图标立即变），但 `mounted` 和 `width` 按动画走；不要把 `collapsed` 直接绑到容器条件。

### 4.5 PTY resize 收敛

- 动画期间不调用 PTY resize；`left-sidebar-taken` / 终端宽度用目标值算布局，但 PTY 只认落定值。
- 落定后 0ms 发一次；非 spring 形态若超过设定时长 + 80ms 仍没有落定（掉帧），120ms 防抖兜底发一次；spring 形态用 §4.3 的 `max(10×ms, 1200ms)` 判据。
- 连续收展只保留最后一次 pending；不要排队。
- 终端最小宽度 520px：如果目标宽度会让终端小于 520，先收右栏，再收左栏；这是窄窗策略（附录 D），不在动画层做二次判断。

### 4.6 减少动效

- `panel_anim_reduced_motion=true` 且系统返回「减少动效」→ 所有形态强制 `instant`，时长 0，不建 Timer，不跑 preview 动画。
- `panel_anim_reduced_motion=false` → 不跟随系统，按用户形态播放；设置页用 desc 提示「关闭后可能引起不适」。
- 系统 API 不可用/读不到时按「未开启减少动效」处理；这是未核实项（§6.3）。

### 4.7 行数与性能

- 手风琴/归巢最多 12 行；超过 12 行自动降级为推挤，并在设置页 desc 里说明「行数过多时自动简化」。
- 不要为每行创建 Timer；用循环索引统一算延迟（`i * 18ms`）。
- 动画期间避免每帧读 `Theme.snap-to-device`；只在落定帧调用。
- 预览区最多 2 个迷你面板，不复用真实面板的模型。

---

## 5. Slint 落地映射

### 5.0 作用对象（target）的状态

`target` 是设置页的局部状态，类型 `left | right | both`，默认 `both`，不写配置；重开设置页回到 `both`。

| 状态/事件 | 行为 |
| --- | --- |
| 初始 `both` | 预设、形态、时长、缓动行都显示“左右同值”；两侧不同时显示混合态 |
| 切到 `left` / `right` | 只改读取与写入目标；不写任何配置；行控件显示该侧真实值 |
| 控件写入 | 按 target 广播：`left` 只写 `*_left`，`right` 只写 `*_right`，`both` 同写两侧 |
| 点预设 | 整组写目标侧三键（形态/时长/缓动）；`both` 时两侧同写；不写减少动效 |
| 配置重载/外部变更 | target 不变；按 target 重算显示；`both` 且两侧不同时进入混合态 |
| 关闭/重开设置页 | target 回到 `both`；配置值不变 |

派生展示态一律现算：当前文案、预设高亮/半亮、形态滑块 thumb、时长/缓动是否置灰，都由「target + 左右配置值」计算，不额外存一份 UI 配置。

### 5.1 新增属性/回调

| 层 | 符号 | 类型 | 说明 |
| --- | --- | --- | --- |
| app 根 | `panel-anim-left-form` | string | 镜像 `panel_anim_left` |
| app 根 | `panel-anim-right-form` | string | 镜像 `panel_anim_right` |
| app 根 | `panel-anim-ms-left` | int | 镜像 `panel_anim_ms_left`，钳制 80–600 |
| app 根 | `panel-anim-ms-right` | int | 镜像 `panel_anim_ms_right`，钳制 80–600 |
| app 根 | `panel-anim-easing-left` | string | 镜像 `panel_anim_easing_left` |
| app 根 | `panel-anim-easing-right` | string | 镜像 `panel_anim_easing_right` |
| app 根 | `panel-anim-reduced-motion` | bool | 镜像 `panel_anim_reduced_motion` |
| app 根 | `panel-anim-duration-left/right` | duration | 各自 `ms * 1ms` |
| 每侧 | `panel-mounted` / `panel-phase` / `panel-target` / `panel-width` | bool/enum/length/length | §2.1 |
| 每侧 | `panel-animating` / `panel-generation` | bool/int | 落定与快速反向 |
| 每侧 | `panel-anim-ready` | bool | 首帧门卫 |
| 设置页 | `panel-anim-target` | string | `left`/`right`/`both`，界面态，不入配置 |
| 设置页 | `sidebar-panel-anim-changed(key, target, value)` | callback | 单键写入；`target` 决定广播到哪侧 |
| 设置页 | `sidebar-panel-preset-applied(preset, target)` | callback | 整组写目标侧三键 |
| 设置页 | `sidebar-panel-preview()` | callback | 试播；不持久化 |

配置写入路径：设置页控件 → `sidebar-panel-anim-changed` / `sidebar-panel-preset-applied`（带 target）→ `src/app.rs` 按 target 写 `Config` cache → 现有保存流程持久化。app 根属性由配置变更回调刷新，设置页不直接改 app 根。

### 5.2 Rust 配置字段

| 字段 | 类型 | serde | 访问器 | 说明 |
| --- | --- | --- | --- | --- |
| `panel_anim_left` | `String` | `#[serde(default = "default_panel_anim_left")]` | `panel_anim_left()` / `set_panel_anim_left()` | 新增；非法值回落 `push` |
| `panel_anim_right` | `String` | 同上 | `panel_anim_right()` / `set_panel_anim_right()` | 新增；非法值回落 `push` |
| `panel_anim_ms_left` | `u16` | `#[serde(default = "default_panel_anim_ms")]` | `panel_anim_ms_left()` / `set_panel_anim_ms_left()` | 新增；读时 clamp 80–600 |
| `panel_anim_ms_right` | `u16` | 同上 | `panel_anim_ms_right()` / `set_panel_anim_ms_right()` | 新增；读时 clamp 80–600 |
| `panel_anim_easing_left` | `String` | `#[serde(default = "default_panel_anim_easing")]` | `panel_anim_easing_left()` / `set_panel_anim_easing_left()` | 新增；非法值回落 `smooth` |
| `panel_anim_easing_right` | `String` | 同上 | `panel_anim_easing_right()` / `set_panel_anim_easing_right()` | 新增；非法值回落 `smooth` |
| `panel_anim_reduced_motion` | `bool` | `#[serde(default = "default_true")]` | `panel_anim_reduced_motion()` / `set_panel_anim_reduced_motion()` | 新增；默认 true |

以上 **7 个生效字段全部为新增**，不复用现有字段；都带 `#[serde(default)]`，因此旧配置可直接加载。另保留 2 个旧字段仅用于迁移读取：`panel_anim_ms: Option<u16>`、`panel_anim_easing: Option<String>`，不暴露 setter；旧配置出现 `panel_anim_sync` 时忽略。

**迁移**：左右分别读取；某侧新键缺失时，若旧单值存在则读作该侧值，否则用默认 240 / `smooth`。旧同步键不影响加载后的作用对象，作用对象默认「左右」。旧字段不写回、不参与新保存。

落点：`src/config/struct/config_file.rs` 现有 `zen_mode` 459-461、`quick_commands_as_sidebar` 469-472、`quick_panel_open` 473-476 附近；访问器落 `src/config/impls/config.rs` 现有 `zen_mode` 1485/1489、`quick_commands_as_sidebar` 1789/1793 的模式。setter 只写 cache，持久化交给保存流程。

`DEFAULTS_REV` 保持 `10`（`src/config/struct/config_file.rs:47`）。原因：7 个生效字段都有 `#[serde(default)]`，旧配置能直接加载；bump 会触发不必要的迁移。Rust 默认值按 §0.2 写死为 `push` / 240 / `smooth` / `true`，新装与旧配置缺字段都落到这组默认，不需要迁移，也不 bump。

### 5.3 文件/符号改动点

| 文件 | 符号/区域 | 改什么 |
| --- | --- | --- |
| `src/config/struct/config_file.rs` | `Config` 结构体 | 加 7 个生效字段 + 2 个旧字段（仅迁移）+ 默认函数；`DEFAULTS_REV` 不动 |
| `src/config/impls/config.rs` | `impl Config` | 加 7 组 getter/setter + 枚举/范围校验；加载时旧单值迁移到左右 |
| `src/app.rs` | 设置保存/应用路径 | 接收带 target 的 `sidebar-panel-anim-changed` / `sidebar-panel-preset-applied`，按目标写 cache；不新增独立持久化文件 |
| `ui/app.slint` | 根属性区 | 加 7 个镜像属性、左右 duration、每侧运行态、预览运行态 |
| `ui/app.slint` | 左栏容器 `2870` 附近 | `if !collapsed` → `if panel-mounted`；宽接 `panel-width`；`left-sidebar-taken`（1705-1706）改为 `left-sidebar-strip + (panel-mounted ? panel-width : 0px) + right-sidebar-outside-strip`；加 `animate width` + 门卫 |
| `ui/app.slint` | 右栏容器 `3129` 附近 | 同上；保留 3145-3150 注释并更新为「门卫后允许」 |
| `ui/app.slint` | `left-sidebar-taken` `1705-1706` | 删 36px 残留，改 `panel-mounted ? panel-width : 0px` |
| `ui/app.slint` | `quick-outer-strip` `2816-2819` | 同批清 36px；快捷面板保持独立 |
| `ui/app.slint` | `sb-w` `3094-3101` | 动画期间跳过 snap；落定帧再 snap |
| `ui/interface_panel.slint` | 侧栏页 `2359`、`2404` 后 | 加「收展动效」分组（作用对象、状态摘要、预设、形态滑块、时长、缓动、减少动效）；现有 4 行原位保留 |
| `ui/interface_panel.slint` | 侧栏页属性/回调 | 加 7 个设置属性、`target` 界面态、3 个 callback；不新增导航项 |
| `ui/seg.slint` | 复用 `SegGroup` | 不改；作用对象三档、缓动三档直接实例化 |
| `ui/widgets.slint` | 复用 `MTermSlider` | 不改；形态滑块/时长滑杆直接实例化，形态值用 `Math.round` 吸附 |
| `src/app.rs` | `7398-7402`/`7414-7415`/`7438-7439` | 清 36px 残留；与 Slint 侧同批 |
| `docs/design/two-sidebar-hifi-2026-11-04.html` | 高保真定稿（按侧预设 + 单组滑块） | 另一条线由 proto-builder-2 做；本规格不直接改 |

### 5.4 D1/D2 不动

左 `BrandLogoBtn`（`ui/app.slint:749`，32×32 命中区 `760-761`）、右 `rb-toggle-ta`（`ui/app.slint:2723-2744`，40 宽）保持现状；设置入口保持「⋯ → 设置弹框（默认外观页）→ 侧栏页」。本次只改「收起来长什么样」，不改「谁控制收起来」。细节见附录 C。

---

## 6. 风险、验收与交付

### 6.1 风险

| 风险 | 概率 | 影响 | 缓解 |
| --- | --- | --- | --- |
| 首帧误播导致「面板长出来」 | 高 | 中 | §4.1 门卫；预览区单独门卫；回归用例覆盖 |
| 动画期间 snap 闪 1px 白缝 | 中 | 高 | §4.2 动画期停 snap；落定帧吸附；125%/150% 截图验收 |
| PTY 逐帧 resize 造成卡顿/闪烁 | 中 | 高 | §4.5 只在落定帧发；非 spring 时长+80ms，spring `max(10×ms, 1200ms)` 兜底 |
| 手风琴/归巢行数多掉帧 | 中 | 中 | 12 行上限；超限降级推挤 |
| 果冻过冲挤压终端 | 中 | 中 | ±8px 夹紧；<520px 退化；双侧只提示不强制禁 |
| 减少动效系统 API 不可用 | 中 | 低 | 按未开启处理；设置页可手动选瞬时 |
| 右栏 top/bottom 停靠方向写死 | 中 | 中 | 按 dock 轴收展；只在 left/right 停靠做完整验收 |
| 用户误以为「翻页/溶解」能用 | 低 | 低 | 滑块对应档位置灰 + 「仅演示」徽标 + tooltip |

### 6.2 工作量

| 阶段 | 内容 | 人日 |
| --- | --- | --- |
| 1 | 状态模型 + 左右容器改造 + 36px 清零 | 2.0 |
| 2 | 瞬时/推挤/吞没 | 1.5 |
| 3 | 手风琴/归巢/果冻 + 落定判据 | 2.0 |
| 4 | 首帧门卫 + snap 收敛 + PTY 收敛 | 1.5 |
| 5 | 设置页 IA + 作用对象 + 形态滑块 + 预设 + 预览 | 2.0 |
| 6 | 配置字段/访问器/持久化 + 回归 | 1.0 |
| 合计 |  | **10.0** |

不含高保真 HTML（proto-builder-2 线）与真机 125%/150% 视觉验收。

### 6.3 验收

| 编号 | 验收项 | 通过标准 |
| --- | --- | --- |
| V1 | 形态齐全 | 8 档滑块可见；6 可点、2 置灰；顺序与 §3.4 一致；写入值与档位表一致 |
| V2 | 默认值 | `panel_anim_left="push"`、`panel_anim_right="push"`、`panel_anim_ms_left=240`、`panel_anim_ms_right=240`、`panel_anim_easing_left="smooth"`、`panel_anim_easing_right="smooth"`、`panel_anim_reduced_motion=true`；作用对象默认「左右」 |
| V3 | 作用对象 | 选左栏只写左侧、选右栏只写右侧、选左右同时写；切换作用对象不改任何配置；左右不同时显示混合态 |
| V4 | 首帧不误播 | 冷启动、切页、重开设置、展开左/右栏各 3 次，无「从 0 长出」 |
| V5 | snap | 125%/150% 下收展，终端与右栏边界无 1px 白缝；落定帧边界为整数设备像素 |
| V6 | 减少动效 | 系统开启 + 开关开 → 所有形态瞬时；无动画、无 preview 动画 |
| V7 | 试播 | 点试播播放当前目标；不写配置；连续点不叠加 |
| V8 | 快速反向 | 200ms 内连点 5 次，最终状态与最后一次意图一致；无残影/半挂载 |
| V9 | 36px 清零 | 左/右/快捷面板折叠后完全让出空间；无 36px 空条 |
| V10 | PTY | 收展过程终端不逐帧重排；落定后仅一次 resize；无闪烁 |
| V11 | 设置页 | 现有 4 行功能不回退；新分组只在「侧栏」页；无第二入口 |
| V12 | 可访问性 | 形态滑块可 Tab/方向键；置灰档有 tooltip；文案中英一致 |
| V13 | 预设套用 | 首启「顺滑」高亮；点预设整组写目标侧三键；目标=左右时两侧同写；手改任一参数转「自定义」；预览「重置」回顺滑 |
| V14 | 按侧预设 | 目标=左栏点「电影感」只改左栏，右栏保持；再切右栏点「活泼」两侧不同且互不覆盖；目标=左右时两侧同写 |
| V15 | 单组滑块与混合态 | 形态只有一条滑块；目标=左右且左右不同时显示混合提示，滑块不伪装成单一选中；拖动滑块写两侧并清除混合 |

### 6.4 交付声明与后续改动点

**本方案不改代码。** 本文只定义交互与实现约束；落地由后续任务按 §5.3 拆分。改动点清单（文件/符号/配置键/`defaults_rev`）：

1. `src/config/struct/config_file.rs`：7 个生效字段 + 2 个旧迁移字段 + 默认函数；`DEFAULTS_REV` 保持 10。
2. `src/config/impls/config.rs`：7 组 getter/setter + 枚举/范围校验 + 旧单值迁移。
3. `src/app.rs`：带 target 的设置回调写 cache；36px 清理（Rust 三处：`7398-7402`、`7414-7415`、`7438-7439`）。
4. `ui/app.slint`：7 个镜像属性 + 左右 duration；左右容器 `mounted/width/animate`；`left-sidebar-taken` 与 `quick-outer-strip` 两处 36px 清理；`sb-w` snap 分支；首帧门卫；预览态。
5. `ui/interface_panel.slint`：侧栏页新分组（作用对象、状态摘要、预设、形态滑块、时长、缓动、减少动效、试播）；现有 4 行不动。
6. `ui/seg.slint` / `ui/widgets.slint`：只复用，不改；形态滑块用 `MTermSlider` + `Math.round` 吸附。
7. 测试：新增 8 形态、按侧预设、作用对象、混合态、首帧、snap、减少动效、快速反向、PTY 九组回归。
8. 文档：本文件 rev8；高保真 HTML 由 proto-builder-2 对齐。

### 6.5 非目标

- 不重新设计左/右栏开关（D1 已实现）。
- 不新增设置入口、不在侧栏头部放齿轮、不在标题栏加独立设置图标。
- 不做 3D 翻页/遮罩溶解的真实渲染。
- 不做跨侧栏联合时间轴（左右各自计时）。
- 不改变终端最小宽度 520px，也不改变窄窗收栏优先级（附录 D）。

### 6.6 rev 6 记录

- 主线从「D1–D6 决策」改为「收展样式」：新增 §2 八形态、§3 设置 IA、§4 实现注意、§5 映射。
- 原 D1/D2 压成 §2 开头一段现状基线说明，细节收进附录 C；D3–D6 移入附录 D，每条 2–3 行。
- rev6 的 6 个配置键在 rev8 重构为按侧 7 键；同步键由作用对象取代。
- rev6 的旧默认说明已废弃；rev7 起统一为默认「顺滑」240ms，见 §6.7。
- 明确 6 可落地 + 2 仅演示；翻页/溶解只在预览置灰。
- 新增 `#right-panel-no-anim` 首帧门卫与 `#sb-edge-snap` 动画期停 snap 的可操作做法。

### 6.7 rev 7 记录

改为预设优先：设置页「收展动效」分组新增「预设」，默认「顺滑」（推挤 + 240ms + `smooth`，默认值即此组合）；跟手改为推挤 120ms；预设选中态由参数反推、不落盘。

### 6.8 rev 8 记录

按用户 m01363 改为「预设可按侧套用 + 形态只一组控件」：设置页新增作用对象（左/右/左右），预设整组写目标侧，左右可各挂不同预设；删除左右两套形态按钮与旧的同步开关，形态改一条 8 档滑块；配置从 6 键改为按侧 7 键（旧单值迁移为左右同值，旧同步键忽略）；新增 V14/V15 与 §5.0 target 状态。

**rev8.1 对齐（同日）**：归巢形态 slug 统一为 `homing`（与高保真、CSS 类名一致）；预设半亮的「1/2」改为标题后内联小胶囊，避免盖住「默认」徽标与卡片角标；卡片底部文案随作用对象变「套用到左栏 / 右栏 / 左右」；混合态提示收敛为顶部琥珀一行，形态说明只描述滑块所指一侧（前缀「左栏：」），能力徽标回归「可落地/仅演示」。

---

## 附录 A：file:line 抽检

| # | 引用 | 核对内容 |
| --- | --- | --- |
| 1 | `ui/app.slint:2870` | `if !root.left-sidebar-collapsed : Rectangle {` 条件挂载 |
| 2 | `ui/app.slint:3145-3150` | 右栏故意不使用 animate 的注释 |
| 3 | `ui/app.slint:3094-3101` | `sb-w` 与 `Theme.snap-to-device` |
| 4 | `src/config/struct/config_file.rs:47` | `DEFAULTS_REV: u32 = 10` |
| 5 | `ui/app.slint:1564-1565` | `ifd-w`/`ifd-h` 弹框尺寸 |
| 6 | `ui/app.slint:2723-2744` | `rb-toggle-ta` 右栏开关 |

## 附录 B：brief-v6 核对

8 形态、设置页、按侧 7 键、首帧门卫、停 snap、spring 落定、4 预设、左右冲突、D3–D6 附录、不改 HTML/代码共 11 项，已分别落入 §2、§3、§4、§5、附录 D 与 §6.4；逐项自检通过。

## 附录 C：现状参考（D1/D2，已实现，不展开）

### C.1 D1 两个独立开关

| 项 | 左栏 | 右栏 |
| --- | --- | --- |
| 组件 | `BrandLogoBtn`（`ui/app.slint:749`） | `rb-toggle-ta`（`ui/app.slint:2723-2744`） |
| 位置/尺寸 | 标题栏最左端，32×32 命中区（`760-761`） | 标题栏最右端，40 宽 × 标题栏高 |
| 常态 | LOGO 徽章 18px（`799-806`） | `sidebar-right-expand`/`collapse` 16px 图标 |
| hover | 徽章淡出，换 `sidebar-left-collapse/expand`，300ms 气泡 | 背景 `Theme.bg-hover`（`2733`） |
| 点击 | 切 `left-sidebar-collapsed` 并持久化 | 切 `right-sidebar-collapsed` 并持久化（`2728-2729`） |
| 关系 | 独立、不联动 | 独立、不联动 |

可选优化（只登记，不推荐本期做）：左 32→40 统一；左改专用 sidebar 图标。两条都不影响收展样式。

### C.2 D2 设置入口

1. `⋯` 更多菜单 →「设置」（`ui/app.slint:6113-6145`，文案 `6140-6141`）。
2. `SettingsDialog`（`ui/app.slint:8168`）打开；默认落在「外观」页（`ui/app.slint:6121-6130`，`6130` 写 `ifd-page="theme"`）。
3. 点左侧导航第 8 项「侧栏」（`ui/interface_panel.slint:1217-1219`），进入侧栏页（`2359`）。
4. 页内现有 4 行：默认收起资源视图面板（`2365-2371`）、专注模式（`2372-2381`）、默认收起 SFTP（`2382-2390`）、快捷命令作为侧栏（`2392-2404`）。
5. 可选优化：从「⋯」直达侧栏页，或记住上次页。与现有 `ifd-page-reset`（`ui/app.slint:6121-6127` 注释）冲突，本期不做，成本小。

## 附录 D：其他未决项（次要）

| 项 | 规格（每条 2–3 行） | 状态 |
| --- | --- | --- |
| D3 同边冲突 | 同侧两个入口同时触发时显式互斥；300ms 提示条；8s 内可撤销。 | 非本期主线 |
| D4 折叠完全隐藏 | 折叠 = 宽度 0、不保留 36px；与 §1.2 / §5.3 的 36px 清零同批验收。 | 已确认 |
| D5 快捷面板同级 | 快捷面板与左右栏同级，不共用形态；`QuickDockPanel` 保持独立条件挂载（`ui/app.slint:3941-3945`）。 | 非本期主线 |
| D6 键位与窄窗 | 键位提案（Ctrl+Shift+[ / ]、Ctrl+,、Ctrl+Shift+/）与窄窗自动收右再左、逆序恢复、终端最小 520px、手动/自动标记分离。 | 提案，未实现 |
