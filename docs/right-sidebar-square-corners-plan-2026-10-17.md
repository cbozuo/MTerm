# 右侧面板左上/左下圆角缺口 —— 修改清单（2026-10-17，**待批准，未动代码**）

> **状态：已落地（2026-10-17）** —— 本节内容已实现并通过测试；文档留作实现记录。


> 现象：右侧资源面板的左上角、左下角是圆弧，和终端区不"贴合"。
> 期望：直角整块，描边直接收口（与左侧栏一致）。
> 状态：仅记录清单，等通知再改。

---

## 1. 证据（截图量化，2x DPI）

- 面板左缘描边在 **x = 1938**（设备像素），颜色 `(35,36,40)` = `border-subtle`，
  宽度 **1 设备像素**（`Theme.hairline()`，左右各 1px 才是逻辑 1px，所以看起来是细线）。
- 面板顶边在 **y = 64**（标题栏 32 逻辑 px × 2）。
- 沿 y 扫描：y=68…92 描边恒在 x=1938（直线）；到 y=64 附近描边起点右移到 x≈1946，
  y=64…68 之间是一段**弧** —— 用户红圈圈的就是这段 1px 圆弧（半径 6 逻辑 px = 12 设备 px）。
- 角落"缺口"里的颜色与面板底相同 (24,26,29)，所以看不到色差，只看到一条弯线 = "圆形"。

## 2. 根因

`ui/right_sidebar.slint` 两处分支的卡片壳都带圆角：

| 行 | 分支 | 现状 |
|---|---|---|
| 545 | `if !root.lay-horizontal`（竖向停靠 right/left） | `border-radius: Theme.radius-md;`（6 逻辑 px） |
| 601 | `if root.lay-horizontal`（横向停靠 top/bottom） | `border-radius: Theme.radius-md;` |

对照：左栏 `LeftSidebar` 的根只有 `background: Theme.zone-sidebar;`，**没有圆角**
（app.slint 里左栏容器也没有），所以只有右侧出现弧线。v5.24/v6.2 的「侧栏贴边 · 直角整块 ·
去 12px 内缩」只改了底色与内缩，**圆角是漏改项**。

## 3. 修改点

```slint
// ui/right_sidebar.slint:545 与 601
background: Theme.zone-sidebar;
border-radius: Theme.radius-md;   // ← 改成 0px
border-width: Theme.hairline();
border-color: Theme.border-subtle;
```

- **保留 1 设备像素描边**：它是面板与终端之间唯一的分隔线（两者同色 `#181A1D`），
  左栏右侧也有一条同样的线（截图中 x=545）；去掉会让左右不对称。
- `clip: true` 保留（内容越界仍要裁）。
- 内层块（`StatusBlock` / `StatsBlock` / 网络/磁盘卡片）自己的圆角不动 —— 用户圈的是
  **面板外缘**，不是内部卡片。

## 4. 连带核对

1. 四种停靠位：right（默认）/ left / top / bottom —— 两个分支都改后应全部直角；
   top/bottom 时圆角在左右两端，同样要贴边。
2. 面板收起/展开动画：`animate width/height`（app.slint:2581）不涉及半径。
3. 右侧栏与快捷面板并排、zen 模式：不涉及。
4. 左栏无需改动（本来就无圆角）。

## 5. 验收

1. 截图右面板左上角 / 左下角：**没有弧线**，描边在角上直接以直角收口。
2. 左、右两条面板分隔线粗细与颜色一致（都是 1 设备像素 `border-subtle`）。
3. 停靠 top/bottom 时同样直角贴边。
4. 面板内容不因 `border-radius: 0` 而溢出（`clip` 仍在）。

## 6. 测试（可选，离屏断言）

用 `MinimalSoftwareWindow` 抓帧后取**面板左上角**那一个像素：
- 圆角时该点是背景色 (24,26,29)；
- 直角时该点是描边色 (35,36,40)。

断言 `px(panel_left, panel_top) == border-subtle`（容差 2）即可锁死；再取
`(panel_left, panel_top+20)` 应为描边色、`(panel_left-4, panel_top+20)` 为 `term-bg`。

> 坐标由窗口宽度与右栏宽度（`right-sidebar-width`，默认 220）算出：
> `panel_left = W - 220`（逻辑），乘 DPR 后取整。

## 7. 风险

- 纯视觉 1 行改动（两处），不涉及数据/配置/交互。
- 唯一注意：`Theme.radius-md` 若被这两个分支以外的同一面板元素复用（当前 grep 只有这两处），
  改完全局不变。
