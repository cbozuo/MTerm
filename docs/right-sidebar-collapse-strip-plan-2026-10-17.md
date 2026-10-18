# 右侧栏收起后残留 36px 空白竖条 —— 修改清单（2026-10-17，**待批准，未动代码**）

> **状态：已落地（2026-10-17）** —— 本节内容已实现并通过测试；文档留作实现记录。


> 现象：右侧资源栏收起后，窗口右边仍留一条空白竖条（无图标、纯窗口底色）。
> 期望：收起 = 完全让出空间，内容区贴到窗口右缘；展开仍走标题栏那个切换按钮。
> 状态：仅记录清单，等通知再改。

---

## 1. 证据（截图量化）

截图 693×1755（2x DPI，对应 346×877 逻辑 px），横向扫描 y=300/600/900 三行结果一致：

| x 区间（2x 像素） | 颜色 | 含义 |
|---|---|---|
| 0 – 348 | (24,26,29) | 内容区（`term-bg`） |
| **349 – 418** | **(22,22,25)** | **残留竖条 = 70px / 2 = 35 逻辑 px（≈36）** |
| 419 | (78,81,87) | 窗口描边 / 圆角发丝线 |
| 420 | (22,22,25) | 描边内侧 1px |
| 421+ | ~(211,215,217) | 窗口外（桌面/其它窗口） |

→ 竖条颜色正好是 `Theme.window-base`（mterm-dark root `#161619`），宽度 36px，无任何内容。

---

## 2. 根因（代码事实）

`ui/app.slint:2548-2555`：

```slint
property <length> sb-w: root.zen-mode ? 0px
    : dock-area.right-sidebar-strip-outside
        || dock-area.combine-collapsed-tools ? 0px
    : root.right-sidebar-collapsed ? 36px : root.right-sidebar-width;   // ← 就是这里
property <length> sb-h: ... 同样在收起时给 36px
```

- `ui/app.slint:2738-2741`（pane 区）`width = parent.width - sb-w` → 收起时仍扣掉 36px。
- `ui/app.slint:2567` 收起时 `if !root.right-sidebar-collapsed` **不渲染任何东西** —— 注释里
  说的"collapsing swaps it for the ToolStrip icon bar"对应的 ToolStrip 已经不存在
  （v6.2 改成标题栏的 `sidebar-right-expand` 按钮，`ui/app.slint:2230-2241`）。
- 左侧栏没有这个问题：`left-sidebar-size = (!zen && !collapsed) ? width : 0px`（2296 行）——
  右侧栏没有跟着同款改造，属于**遗留占位**。

结论：**纯占位，无功能**。行为上等于"收起后仍保留 36px 空槽"。

---

## 3. 修改点（1 处主改 + 3 处连带）

1. **主改** `ui/app.slint:2548-2555`
   ```slint
   property <length> sb-w: root.zen-mode || root.right-sidebar-collapsed ? 0px
       : dock-area.right-sidebar-strip-outside || dock-area.combine-collapsed-tools ? 0px
       : root.right-sidebar-width;
   ```
   `sb-h` 同款（`|| root.right-sidebar-collapsed`）。对齐左栏 `left-sidebar-size` 的写法，
   两栏"收起 = 0"语义一致。
2. **注释清理**：2545-2547 / 2564-2566 / 2777（"Collapsed resource panel now reserves its own
   ToolStrip slot"）—— 这些说法已经过期，改成"收起不占位，展开走标题栏按钮"。
3. **核对 `right-sidebar-strip-outside` / `combine-collapsed-tools` 两个分支**：它们是「合并
   折叠工具条」的场景（左栏展开时共享 ToolStrip），主改保留这两个分支在前，确认收起后
   `right-sidebar-outside-strip`（2294 行，恒 0）不会又把 36px 加回来。
4. **核对 splitter / drag-to-dock / quick panel**：
   - splitter（2660-2664）在收起时 `visible: false`，公式里的 `sb-w - 2px` 不受影响；
   - pane 区（2738-2741）与 quick panel 的并排宽度公式都用 `sb-w`，主改后自动跟随；
   - 停靠 top/bottom/left 时用 `sb-h`，同样自动跟随（需实测四种停靠位）。

**不改**：`left-sidebar-*`、标题栏切换按钮、`dock-area` 的 `left-sidebar-taken` 公式、
`pane.reserve-right`（那是 macOS 红绿灯预留，与本条无关）。

---

## 4. 验收

1. 右侧栏收起：内容区（标签栏 / 终端区）**贴到窗口右缘**，无任何空白竖条；
   截图同样取 (W-2, H/2)，应为 `term-bg` (24,26,29)，不是 root (22,22,25)。
2. 标题栏右侧按钮可再次展开，展开动画（150ms）与之前一致，展开后宽度 = 设置值。
3. 四种停靠位（right / left / top / bottom）收起后都无残留；bottom 停靠尤其要测 `sb-h`。
4. 右栏与快捷面板同时打开时，两者并列不错位、无空隙带。
5. zen 模式（Ctrl+Alt+Z）行为不变（本来就 0）。

---

## 5. 测试（离屏渲染，防止回归）

新增 `tests/sidebar_collapse_strip.rs`（复用 `dialog_scrim_drag` 的 `MinimalSoftwareWindow`
骨架，独立进程）：

- 窗口 1400×900，`set_right_sidebar_collapsed(true)` → 抓帧；
- 断言 `px(W-2, H/2)` == `term-bg`（容差 1），**且** != `window-base`（容差 1）——
  两者在 mterm-dark 下是 (24,26,29) vs (22,22,25)，2~4 级差，足够区分；
- 对照：展开态下同一点应为右栏的 `panel` 色（右栏展开时 x=W-2 落在右栏内）；
- 再跑一遍 `right_sidebar_dock = "bottom"`（用 `px(W/2, H-2)`）覆盖 `sb-h`。

> 为什么不用"扫描 36px 带"的断言：带内颜色就是 root，而 root 也可能出现在别处；
> 直接钉"最边缘像素 = 内容色"更小、更稳。

---

## 6. 风险 / 影响面

- 改动 **2 个属性 + 注释**，不动组件结构；风险低。
- 唯一可能的连带：若有代码把 `sb-w == 36px` 当作"收起态"的判据（当前 grep 未见，
  仅 splitter/pane/quick 三处几何计算），改完需回归这三种场景。
- 回滚：把 `|| root.right-sidebar-collapsed` 去掉即可。
