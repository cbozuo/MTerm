# 壁纸作用域新增「主区域」三档 —— 修改清单（2026-10-19）

> **状态：已落地（2026-10-19 03:0x）** —— 三档（整个界面 / 主区域 / 终端）、运行时索引
> 0/1/2、各档首设默认值、三档各自的自动填充比例均已实现；`cargo test --lib` 485 通过；
> 200% DPI 实机三档截图核对通过（主区域：标题栏实心、侧栏与终端都有图）。
> 文档留作实现记录。
>
> 用户决策（2026-10-19 02:1x）：
>
> 1. **默认档 = ① 整个界面**；除非用户手动改过。没有壁纸时也是「整个界面」。
> 2. 新增 **② 主区域**（标题栏以下：两侧栏 + 终端列 + 下方面板），注意它的默认值设定。


---

## 1. 三档定义

| # | 名称     | 绘制容器                      | 覆盖范围                        | en           | 运行时索引 |
| - | ------ | ------------------------- | --------------------------- | ------------ | ----- |
| ① | 整个界面   | 根层 `Image`（现有）            | 全窗，**含标题栏**                 | Whole window | 0     |
| ② | 主区域（新） | `dock-area` 内新增一层 `Image` | 标题栏**以下**：左右侧栏 + 终端列 + 下方面板 | Main area    | 1     |
| ③ | 终端     | `pane-area` 内 `Image`（现有） | 仅终端列（含标签栏行、命令栏）             | Terminal     | 2     |

**索引重排**：现在 0=整个界面 / 1=终端。新增后统一成 0/1/2（顺序与分段控件一致），`终端` 由 1 → 2。  
需要同步的比较点（共 5 处，漏一处就会"选了终端却按主区域画"）：

- `ui/app.slint` 根层 Image 条件：`Theme.wallpaper-scope == 0`（不变）
- `ui/app.slint` `pane-area` Image 条件：`== 1` → `== 2`
- `ui/app.slint` `dock-area` 新增 Image 条件：`== 1`
- `ui/interface_panel.slint` `auto-fit-resolved()`：按档取比例
- `ui/interface_panel.slint` 分段控件三段 + `root.wallpaper-scope` 读写

> 备选：保持 `终端 = 1`、`主区域 = 2`，可少改两处条件，但分段控件里的索引顺序会变成 0 / 2 / 1，后续维护容易看错。**推荐重排 0/1/2**。

---

## 2. 默认值（本次重点）

### 2.1 作用域本身的默认

| 场景         | 值                | 现状                               | 处理                                            |
| ---------- | ---------------- | -------------------------------- | --------------------------------------------- |
| 出厂 / 没配过   | `"window"`（整个界面） | ✅ 已是                             | 保持 `default_wallpaper_scope() == "window"` 不动 |
| 用户手动改过某主题  | 该主题记下的值          | ✅ 已是（`wallpaper_scope_by_theme`） | 不动                                            |
| 换主题到没配过的主题 | `"window"`       | ✅ 已是（读不到键回落）                     | 不动，补单测锁定                                      |
| 新选一张壁纸     | **不自动改作用域**      | ✅ 已是                             | 不动（避免"设个壁纸把我选好的作用域冲掉"）                        |

结论：① 为默认这条**已经是现状**，本次只补测试固定行为，不写迁移。

### 2.2 各档「首设」的四区 / 壁纸参数（`default_scope_settings`）

| 档        | 标题栏          | 标签栏       | 侧栏        | 终端区       | 壁纸透明度 | 填充   |
| -------- | ------------ | --------- | --------- | --------- | ----- | ---- |
| ① 整个界面   | 0.78（22% 透）  | 0.70（30%） | 0.72（28%） | 0.60（40%） | 0     | auto |
| ② 主区域（新） | **1.00（实心）** | 0.70      | 0.72      | 0.60      | 0     | auto |
| ③ 终端     | 1.00         | 1.00      | 1.00      | 0.72（28%） | 0     | auto |

设计理由：

- **主区域的标题栏必须实心**：这一档的图不画到标题栏后面，标题栏若半透明，透出来的是**桌面**而不是壁纸。给 1.00 是唯一说得通的首设值。
- 其余三区与「整个界面」**逐值相同**（0.70 / 0.72 / 0.60），这样在 ①② 之间来回切，侧栏和终端观感连续，只有标题栏"有没有图"的差别。
- ③ 终端沿用现有值（三区无图 → 实心；终端区 28%）。
- 双击复位（#zone-default-context）自动跟随：`default_zone_alphas()` 本来就读 `default_scope_settings(当前 scope)`，新增分支后三档各自正确，**无需改动调用链**。

### 2.3 首次切到「主区域」时的参数从哪来

`Config::set_wallpaper_scope()` 已经会调 `ensure_scope_defaults()`，给**该主题下所有用过的壁纸**补一条 `"<theme>|<wp>|workspace"` 的首设值；`scope_settings()` 读不到时也用同一份默认值兜底。  
所以：切过去立刻是一套能看见壁纸的值，不会黑屏；用户调过之后就地保存，再切走再切回来还是他的值。

---

## 3. 改动清单

### 3.1 Rust

| 文件                                 | 改动                                                                                                                    |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `src/config/struct/config_file.rs` | `default_scope_settings()` 增加 `"workspace"` 分支（上表 ②）；`default_wallpaper_scope()` 保持 `"window"`；补注释说明"主区域标题栏实心"的理由     |
| `src/config/impls/config.rs`       | `set_wallpaper_scope()` 的白名单加 `"workspace"`（现在只认 terminal，其余一律 window —— 不改这里新值会被静默吃掉）                                |
| `src/theme/mod.rs`                 | `resolve()` 的 scope 归一化加 `"workspace"`（同上，现在 `"terminal" => …，其余 => "window"`）；`set_wallpaper_scope(int)` 改为 0/1/2 映射 |
| `src/app/terminal_ui.rs`           | 无（作用域字符串原样透传）                                                                                                         |
| `src/app.rs`                       | 无（`on_set_wallpaper_scope` 收字符串）                                                                                      |

### 3.2 Slint

| 文件                         | 改动                                                                                                                                                                                                                                                                                                                   |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ui/theme.slint`           | `wallpaper-scope` 注释更新为 0=整个界面 / 1=主区域 / 2=终端                                                                                                                                                                                                                                                                        |
| `ui/app.slint`             | ① 新增 `property <float> workspace-aspect: 1.6;`；② `dock-area` 首个子元素加 `if Theme.wallpaper-active && Theme.wallpaper-scope == 1 : Image { … }`（宽高=parent，fit 用 workspace-aspect），并在 `dock-area` 的 `changed width/height` 回写 `workspace-aspect`；③ `pane-area` 的条件 `== 1` → `== 2`；④ 传给 InterfacePanel `workspace-aspect` |
| `ui/interface_panel.slint` | ① `in property <float> workspace-aspect: 1.6`；② 作用域分段 `count: 3`、宽 250 → 330、三段顺序 整个界面/主区域/终端；③ `auto-fit-resolved()` 三档取 `window-aspect` / `workspace-aspect` / `term-aspect`；④ 说明文案重写（见 §4）                                                                                                                        |

### 3.3 文档 / 日志

- `CHANGELOG.md` 增补 `#wallpaper-scope-workspace`（中英各一条）。
- 本方案落地后在文首标注「已落地」并留下实现记录（同 `theme-mode-values-plan` 的写法）。

---

## 4. 设置页文案（草案）

**作用域（zh）**

> 壁纸的**绘制区域**：  
> **整个界面** = 铺满全窗，包括标题栏（默认）；  
> **主区域** = 标题栏以下——左右侧栏、终端、下方面板都有图，只有标题栏保持实心；  
> **终端** = 只画在终端列里，侧栏后面没有图。  
> 范围之外的区域按各自透明度叠在窗口底上（没有壁纸时即桌面）。弹窗 / 浮层永远不在壁纸范围（由「弹窗与遮罩」单独控制）。  
> 注意：在「主区域」下把**标题栏透明度**调高，透出来的是桌面而不是壁纸。

**en**

> Where the wallpaper is drawn: **Whole window** (default, includes the title bar) / **Main area** (everything below the title bar — sidebars, terminal and bottom panels, with the title bar kept solid) / **Terminal** (the terminal column only).  
> Outside the scope there is no wallpaper: those areas composite over the window base (the desktop when no wallpaper). Dialogs are never part of it.  
> Note: in Main area, raising the title-bar transparency reveals the desktop, not the wallpaper.

---

## 5. 兼容 / 迁移

- `wallpaper_scope_by_theme` 存的仍是字符串，新增 `"workspace"` 取值**向后兼容**；旧版本读到该值会回落成 `window`（只影响降级，可接受）。
- **不新增迁移 rev**：默认值仍是 `window`，老配置行为零变化；`wallpaper_settings` 不会凭空多键。
- 参数存储键格式不变：`<theme>|<wallpaper>|<scope>`。

---

## 6. 验收

**单测（新增/补充）**

1. `default_wallpaper_scope() == "window"`；空 `wallpaper_scope_by_theme` 时 `wallpaper_scope()` 返回 `"window"`。
2. `set_wallpaper_scope("workspace")` 后：`scope_settings()` = 主区域默认（标题栏 1.0 / 0.70 / 0.72 / 0.60），且 `wallpaper_settings` 落键。
3. `default_zone_alphas()` 在 ① / ② / ③ 三档分别返回各自那套。
4. 非法值（如 `"foo"`）归一化为 `"window"`。

**实机（截图）**

- 三档 × 有/无 tab，各一张 200% DPI 截图：
  - ② 主区域：标题栏实心、侧栏与终端都能看到图、命令栏也在图之上；
  - ③ 终端：侧栏后面是桌面；
  - ① 整个界面：标题栏后面也有图。
- 「自动」填充在三档下的判定分别用各自矩形比例（尤其是主区域 vs 整个界面的裁剪差异）。

**命令**

- `cargo check -j 2`
- `cargo test -j 1 --lib`（现基线 484 passed / 0 failed）
- `cargo build --release -j 1` + `python scripts/package-win.py`

---

## 7. 风险 / 待确认

1. **分段控件变宽**（250 → 330px）。内容宽约 750px，一行放得下；但四区滑杆那一行不受影响。需要看一眼实际排版。
2. **主区域档的标题栏误导**：调高标题栏透明度会透出桌面。文案已写；若你希望"主区域下标题栏滑杆也禁止调"，那是另一套联动，本方案不做。
3. **索引重排**（终端 1 → 2）：全局仅 5 处比较，已列清单；这是本次最容易出错的地方。
4. **下方面板归属（上一轮的问题 2）本轮不动**：命令栏 / SFTP 面板仍走「侧栏」透明度。等你对那一条给结论后单独做（建议：命令栏归终端区、可停靠面板跟停靠边）。
