# 侧栏标题行箭头清理 + 标题栏新图标亮度 —— 修改清单（2026-10-17，**待批准，未动代码**）

> **状态：已落地（2026-10-17）** —— 本节内容已实现并通过测试；文档留作实现记录。


> 两项：① 左右两个面板标题行的 `<` / `>` 收起箭头删掉；② 标题栏右上角新增的
> 侧栏切换图标比 −/□/✕ 暗，需要对齐到同一档。
> 状态：仅记录清单，等通知再改。

---

## A. 删除面板标题行的 `<` / `>` 收起箭头

### A.1 位置（代码事实）

| 面板 | 文件:行 | 元素 |
|---|---|---|
| 左（快速连接） | `ui/left_sidebar.slint:1290-1296` | `CollapseButton`（停靠边 ≠ left 时排在标题**前**） |
| 左（同上） | `ui/left_sidebar.slint:1304-1310` | `CollapseButton`（停靠 left 时排在标题**后**，截图就是这个） |
| 右（运行状态） | `ui/right_sidebar.slint:252-260` | `StatusBlock` 里的 18×18 按钮（`Icon collapse-icon`） |

两个面板的箭头都只在「停靠形态」出现（左栏 `root.compact`），tab 形态无。

### A.2 删除后靠什么收起（现状已有）

- **左栏**：标题栏左上角 LOGO 按钮 —— `ui/app.slint:3529-3541` `BrandLogoBtn`，悬停淡出徽章、
  淡入 `sidebar-left-collapse/expand`，点击切换并持久化（`set-left-sidebar-collapsed`）。
- **右栏**：标题栏右侧切换按钮 `rb-toggle-ta` —— `ui/app.slint:2224-2243`，图标随状态切换。

### A.3 ⚠️ 必须处理的平台差异

`BrandLogoBtn` 与 `rb-toggle-ta` 都在 `if root.custom-titlebar` 内（**仅 Windows / Linux**）。
macOS（`custom-titlebar = false`）走原生标题栏，浮层工具条（3444+）只有设置 / 传输 / 主题 /
同步 / 命令栏，**没有侧栏开关** —— 直接删箭头会让 macOS 无法收起左右栏。

**方案（推荐）**：两个侧栏组件各加一个开关属性，由 `app.slint` 传入：

```slint
// left_sidebar.slint / right_sidebar.slint
in property <bool> show-collapse: true;
...
if root.show-collapse && root.compact : CollapseButton { ... }   // 左
```
`app.slint`：`show-collapse: !root.custom-titlebar`（macOS 保留箭头，Win/Linux 隐藏）。
用户截图是 Windows → 箭头消失；macOS 行为不变。

**备选**：在 macOS 浮动工具条加两个 `IconBtn`（更统一，但多两个图标位，需要新图标尺寸/间距核对）。
**不推荐**：直接删 → macOS 失去入口。

### A.4 连带清理（避免留死代码）

- 左：`callback collapse();`（`left_sidebar.slint:1102`）、`app.slint` 里 `LeftSidebar { collapse => {…} }`
  接线（约 2377-2380）、`component CollapseButton`（836 行起，若删完只剩这两处引用则整块删）。
- 右：`StatusBlock.collapse` 回调（223）与 `collapse-icon` 属性（220）、`root.collapse-icon`
  派生（497-503）、两处 `collapse-icon:` / `collapse() =>` 接线（560-562、619-623）、
  `RightSidebar.toggle-collapse` 回调（493）与 `app.slint:2630` 的接线。
- 布局核对：删掉后标题行右端仍由 `Rectangle { horizontal-stretch: 1 }` 兜住，
  「运行状态 / 快速连接」的位置不变；左栏 `header-row` 的 `spacing: 8px` 不变。
- 拖动换边不受影响：右栏 header 的 `dock-drag-*` 由外层 `TouchArea` 承担；
  左栏 `header-row` 的 `TouchArea{mouse-cursor: move}` 保留。

---

## B. 标题栏右上角新图标偏暗

### B.1 量化（截图实测峰值亮度）

| 图标 | 笔画峰值 RGB | 对应 token |
|---|---|---|
| 侧栏切换（圈出） | (122,126,133) | `text-secondary` `#7A7E85` |
| 最小化 − | (188,190,196) | `text-primary` `#BCBEC4` |
| 最大化 □ | (188,190,196) | 同上 |
| 关闭 ✕ | (188,190,196) | 同上 |

差 66 级，肉眼就是"这个图标没点亮"。

### B.2 改法

`ui/app.slint:2241`：

```slint
Icon {
    glyph: root.right-sidebar-collapsed ? "sidebar-right-expand" : "sidebar-right-collapse";
    tint: Theme.text-secondary;      // ← 改成 Theme.text-primary
}
```

这正是 `WinBtn` 在 `#winbtn-tint 2026-10-16` 修过的同一个坑（377-393：三键原用
`text-secondary`，用户反馈"偏暗、不统一"）；本次是标题栏里第 4 个漏网图标。

### B.3 顺带审计（只改标题栏范围）

- `grep "tint: Theme.text-secondary"` 于标题栏区（2046-2280）：确认是否还有同类漏网；
  面板内（侧栏列表等）的 `text-secondary` 是有意的层级，不动。
- hover 态：`rb-toggle-ta` 目前只加 `bg-hover`，图标不提亮 —— 与三键一致，保持不变。

---

## C. 验收

1. 左右面板标题行**没有** `<` / `>`；标题栏右切换按钮、左上 LOGO 悬停按钮仍可收起/展开，
   状态持久化、动画正常。
2. 截图量：侧栏切换图标笔画峰值 = (188,190,196)（与三键差 ≤3）。
3. 四种停靠位（right/left/top/bottom）下，面板标题行都不出现箭头。
4. macOS（`custom-titlebar=false`）：箭头保留、仍可收起（若采纳 A.3 方案）。
5. zen 模式、拖动换边、快捷面板并排均不受影响。

---

## D. 风险

- 属"删入口"改动，需按 A.4 清单把回调/属性一并删净，否则 Slint 会报"未使用回调"不报错、
  但留下死代码。
- 唯一的真实风险是 **macOS 失去收起入口**，用 A.3 的 `show-collapse` 隔离即可。
- 不涉及主题/透明度/遮罩任何数值，无配置变化、无迁移。
