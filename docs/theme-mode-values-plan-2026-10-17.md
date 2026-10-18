# 明暗主题各存一套外观数值 —— 修改清单（2026-10-17，**待批准，未动代码**）

> **状态：已落地（2026-10-17）** —— 本节内容已实现并通过测试；文档留作实现记录。


> 问题：暗主题下调好的透明度/遮罩数值，切到亮主题后原样沿用。
> 期望：**暗 / 亮各存一套**，切换主题即切换到对应那套；某个模式第一次使用时用默认值。
> 状态：仅记录清单，等通知再改。

---

## 1. 要拆成「明暗两套」的值（建议范围）

| 设置项 | config 字段 | 默认值 | 拆分 |
|---|---|---|---|
| 标题栏透明度 | `titlebar_alpha` | 0.85（15% 透明） | ✅ |
| 标签栏透明度 | `tabbar_alpha` | 0.85 | ✅ |
| 侧栏透明度 | `sidebar_alpha` | 0.85 | ✅ |
| 终端区透明度 | `term_alpha` | 1.00（0%） | ✅ |
| 壁纸透明度 | `wallpaper_transparency` | 0.00 | ✅（图本身仍全局） |
| 弹窗透明度 | `popup_transparency` | 0.00 | ✅ |
| 遮罩浓度 | `scrim_alpha` | 0.32 | ✅ |

**不拆（保持全局，理由见 §7 待定项）**：壁纸图片、作用域、填充方式、遮罩样式、强调色三态。

---

## 2. 数据层改动

### 2.1 `src/config/struct/config_file.rs`
- 新增结构体（按现有字段逐个给 `#[serde(default = "...")]`，缺字段也能落到默认值）：
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct AppearanceValues {
      pub titlebar_alpha: f32,       // 0.85
      pub tabbar_alpha: f32,         // 0.85
      pub sidebar_alpha: f32,        // 0.85
      pub term_alpha: f32,           // 1.00
      pub wallpaper_transparency: f32, // 0.00
      pub popup_transparency: f32,   // 0.00
      pub scrim_alpha: f32,          // 0.32
  }
  ```
- `ConfigFile` 里用两个槽替换原来 7 个平铺字段：
  `#[serde(default)] pub appearance_dark: AppearanceValues,`
  `#[serde(default)] pub appearance_light: AppearanceValues,`
- 新增 `DEFAULTS_REV = 6`，旧的 7 个字段从结构体移除（项目已确立"不做旧键兼容"）。

### 2.2 `src/config/impls/config.rs`
- `fresh_config()`：两个槽都填默认值（`AppearanceValues::default()` 走上面的 serde 默认）。
- `migrate_defaults()` 增加 rev 6 分支：把旧平铺字段的值写进**当前主题所属模式**的槽，另一槽保持默认。
  （旧值读不到时 = 默认，等价于"两套都默认"。）
- 访问器改成按模式取值/写值（**推荐写法：写入方显式传 `dark`**）：
  - `fn appearance_for(&self, dark: bool) -> &AppearanceValues`（给 `theme_state_of` 拿两槽）
  - `fn appearance_mut(&mut self, dark: bool) -> &mut AppearanceValues`
  - `set_*(v)` → `set_*(v, dark: bool)`，共 7 个（titlebar / tabbar / sidebar / term /
    wallpaper-transparency / popup-transparency / scrim-alpha）。
  - 调用点 = `src/app.rs` 的 10 个外观回调（含不拆的 scope / fit / style 三个），机械改；
    `dark` 一律取**窗口当前的 `dark_mode`**（用户此刻看到的模式），不要用
    `theme_pref_is_dark()` —— 它内部会 `dark_light::detect()` 调操作系统，配置层不能依赖它。
  - 另一选择：store 内部用 `theme::by_id(theme).map(|p| p.dark)` 自算（`theme` 自 rev5 起恒为
    具体 id，可靠）；无效 id 时与 `palette_or_default` 同判据兜底。两者择一，别两套并存。
- 只读槽访问器同时给 `theme_state_of` 用（暗槽 + 亮槽都要拿）。

### 2.3 `src/theme/mod.rs`
- `ThemeState`：7 个平铺字段 → `appearance_dark: AppearanceValues` + `appearance_light: AppearanceValues`。
- `resolve()`：**先解析出 palette**（现有 `palette_or_default`），再按 `p.dark` 选槽，写进 `Resolved`（`Resolved` 结构不变，仍是单套值）。
- `apply()` 不变（写的仍是 `Resolved`）。
- `crate::theme::AppearanceValues` 复用 config 的结构体（或定义同构类型 + `From`），避免两份字段名漂移；倾向直接 `pub use crate::config::...::AppearanceValues`。

### 2.4 `src/app/terminal_ui.rs`
- `theme_state_of()` 两个槽都从 store 读（`appearance_for(true/false)`）。

---

## 3. 运行时行为

- 切主题（含 `invert-theme`、主题画廊、列表、系统深浅变化）→ `apply_theme` → `resolve()` 选新模式的槽 → 滑杆/面板立刻变成那套值。**设置页开着时也实时跟随**。
- 任意滑杆写入 → 只落到**当前模式**的槽（另一个模式不动）。当前模式 = 写入时窗口的
  `dark_mode`，与 `apply_theme` 写进 Theme 的 `dark` 同源，不会出现"按 A 算、按 B 存"。
- 首次使用某模式 → 该槽从未写过 = 默认值（15/15/15/0、0、0、32%）。
- 双击恢复默认 → 只重置当前模式的该字段。

---

## 4. UI 文案（`ui/interface_panel.slint`）

- 「分区透明度」说明补一句：**明暗主题各存一套数值，切换主题时自动切换**。
- 「壁纸透明度」「弹窗透明度」「遮罩浓度」的描述同样补该句（或统一放到页面顶部说明，避免每行重复）。
- 读数不显示"当前是哪一套"（主题本身已经表明），但切换主题后数值会变 —— 这是预期，不需要额外提示。

---

## 5. 迁移

- 启动时 rev 6 一次性迁移：旧配置的 7 个值 → **当前主题模式**的槽；另一模式留默认。
- 迁移后立即 `save()`（与现有 rev 4/5 同流程）。
- 若用户从未改过（值 = 默认）→ 两套都是默认，无感。

---

## 6. 测试

| 测试 | 位置 | 断言 |
|---|---|---|
| 槽默认值 | `config::tests` | 新装：两槽都 = 默认（15/15/15/0/0/0/32%） |
| 迁移 | `config::tests` | 旧配置（暗主题 + sidebar_alpha 0.62）→ 暗槽 0.62、亮槽 0.85；反向（亮主题）同理 |
| 写值只落当前槽 | `config::tests` | 暗主题下 `set_titlebar_alpha(0.5)` → 亮槽不变 |
| resolve 选槽 | `theme::tests` | `ThemeState` 两槽不同值 → dark palette 取暗槽、light palette 取亮槽 |
| 端到端（渲染级） | `dialog_scrim_drag` 或新增小测试 | 暗主题设 57% → 切亮：`Theme.popup-alpha` = 1.0（默认）；切回暗 = 0.43 |

回归口径沿用现有测试（`dialog_scrim_drag` 里 `set_popup_transparency` 在暗主题下写的就是暗槽，数值不变）。

---

## 7. 待定项（需要你拍板，默认按"不拆"）

1. **遮罩样式**（自动/深色/浅色/…）：字符串不是"数值"，但"深色遮罩"切到亮主题观感会不对。
   建议：随明暗拆（默认两套都是「自动」）。→ 改动仅在 §2 结构体加 `scrim_style` 字段。
2. **强调色（自定义 / 预设）**：自定义色在暗底调好，亮底可能偏亮/偏暗（现有 3:1 守卫会调亮但保色相）。
   建议：拆（`accent_mode` / `accent_preset` / `accent_custom` 各两套）；`follow` 本来就是跟主题。
3. **壁纸作用域 / 填充方式**：属于"同一张图怎么摆"，建议**不拆**（全局）。
4. **壁纸图片本身**：全局（不拆）。

---

## 8. 风险 / 影响面

- 配置文件 schema 变化（一次性迁移）；`Cargo` 层面无新依赖。
- 影响文件约 8 个：`config_file.rs`、`config.rs`、`theme/mod.rs`、`app/terminal_ui.rs`、（可能）`app.rs` 仅当签名变化、`interface_panel.slint`、测试 2 处、CHANGELOG。
- 无渲染管线改动，**不涉及** `Scrim` / 分区 token。
- 回滚：把 rev 6 迁移和两槽结构撤掉即可，无持久化副作用（旧字段已被覆盖，但值仍在当前槽里）。

---

## 9. 验收（你验收时按这个点）

1. 暗主题把四分区拖成 30/30/30/10、弹窗 57%、遮罩 50% → 切亮：**四分区回默认 15/15/15/0、弹窗 0%、遮罩 32%**。
2. 亮主题拖成 5/5/5/20 → 切回暗：**30/30/30/10 仍在**；再切亮：5/5/5/20 仍在。
3. 重启后两套值都还在（各自独立持久化）。
4. 全新配置：明暗两套都显示默认值。
