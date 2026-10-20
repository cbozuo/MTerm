//! 主题系统（theme-wallpaper-split 稿落地）。
//!
//! 色表查表在 Rust 侧（`palettes.rs`，生成文件）：`apply_theme` 一次性把
//! 当前套的 20 个 UI 槽 / 分组色 / 频道色写进 Slint `Theme` global，
//! 终端 ANSI 由 `presentation.rs` 查同一张表——UI 与终端同源。
//! 强调色三态（跟随主题 / 9 预设 / 自定义）在此计算，含 3:1 对比度守卫。

pub mod palettes;

use palettes::Palette;

pub use palettes::find;

use crate::config::AppearanceValues;
use crate::ui::Theme;
use slint::{Color, ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;

/// 把 0xRRGGBB 转成 Slint 颜色。
pub fn color(hex: u32) -> Color {
    Color::from_rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn color_model(vals: &[u32]) -> ModelRc<Color> {
    ModelRc::new(Rc::new(VecModel::from(
        vals.iter().map(|v| color(*v)).collect::<Vec<_>>(),
    )))
}

/// 运行时主题状态（来自 config + 壁纸开关组合）。Rust 持有，切任何一项
/// 都整体重算并重写 Slint Theme（§1：主题/强调色/壁纸三层解耦后的唯一入口）。
#[derive(Clone)]
pub struct ThemeState {
    pub theme_id: String,
    pub accent_mode: AccentCfg,
    /// (#wallpaper-scope-params 2026-10-18) 当前**生效**的一整套外观数值：
    /// 有壁纸时来自 (主题, 壁纸, 作用域) 那一套，没有壁纸时来自明暗槽 ——
    /// 由 `Config::effective_appearance()` 统一解析，resolve() 不再自己选槽。
    pub appearance: AppearanceValues,
    /// (#zone-default-context 2026-10-19) 四根分区滑杆「双击恢复」的目标
    /// （有壁纸 = 该组合首设默认值；无壁纸 = 全不透明）。
    pub zone_defaults: ZoneDefaults,
    /// 壁纸作用域："window" | "terminal"（与明暗无关，全局）。
    pub wallpaper_scope: String,
    /// 壁纸填充方式："cover" | "contain" | "fill"（全局）。
    pub wallpaper_fit: String,
    /// 遮罩样式（9 选，全局）。
    pub scrim_style: String,
    pub dark_pref: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccentCfg {
    Follow,
    Preset(&'static str),
    Custom(u32),
}

impl AccentCfg {
    pub fn from_config(mode: &str, preset: &str, custom: &str) -> Self {
        match mode {
            "preset" => {
                let key = palettes::ACCENT_PRESETS
                    .iter()
                    .find(|(k, _, _)| *k == preset)
                    .map(|(k, _, _)| *k);
                match key {
                    Some(k) => AccentCfg::Preset(k),
                    None => AccentCfg::Follow,
                }
            }
            "custom" => crate::picker::parse_hex(custom).map(AccentCfg::Custom).unwrap_or(AccentCfg::Follow),
            _ => AccentCfg::Follow,
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            AccentCfg::Follow => "follow",
            AccentCfg::Preset(_) => "preset",
            AccentCfg::Custom(_) => "custom",
        }
    }
}

/// (#zone-default-context 2026-10-19) 四根分区滑杆的「默认」值（alpha）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneDefaults {
    pub titlebar: f32,
    pub tabbar: f32,
    pub sidebar: f32,
    pub term: f32,
}

impl Default for ZoneDefaults {
    fn default() -> Self {
        Self { titlebar: 1.0, tabbar: 1.0, sidebar: 1.0, term: 1.0 }
    }
}

/// 解析后的整套应用状态（apply 各窗口版本的公共输入）。
pub struct Resolved {
    pub palette: &'static Palette,
    pub accent: u32,
    pub titlebar_alpha: f32,
    pub tabbar_alpha: f32,
    pub sidebar_alpha: f32,
    pub term_alpha: f32,
    pub wallpaper_transparency: f32,
    pub wallpaper_scope: String,
    pub wallpaper_fit: String,
    pub scrim_style: String,
    pub scrim_alpha: f32,
    pub popup_transparency: f32,
    pub zone_defaults: ZoneDefaults,
}

/// 计算（与窗口无关）：主题套解析 + 强调色三态叠加 + 各透明度钳制。
/// (#zone-alpha 2026-10-17) 四分区全部 0–1 全范围（下限钳制已按用户要求移除）。
pub fn resolve(st: &ThemeState) -> Resolved {
    let dark_now = match st.theme_id.is_empty() {
        true => st.dark_pref,
        false => by_id(&st.theme_id).map(|p| p.dark).unwrap_or(st.dark_pref),
    };
    let p = palette_or_default(&st.theme_id, dark_now);
    let (accent, _) = resolve_accent(p, st.accent_mode);
    // (#wallpaper-scope-params) 数值已由调用方按 主题×壁纸×作用域 解析好。
    let av = &st.appearance;
    // (#wallpaper-scope-workspace 2026-10-19) 三档白名单；未知值回落 window。
    let scope = match st.wallpaper_scope.as_str() {
        "workspace" => "workspace".to_string(),
        "terminal" => "terminal".to_string(),
        _ => "window".to_string(),
    };
    let fit = match st.wallpaper_fit.as_str() {
        // (#wallpaper-auto-fit 2026-10-18) 「自动」必须原样透传：这里曾经只认
        // contain/fill，其余一律回落 cover —— 会把用户的「自动」静默改成填充。
        "auto" => "auto".to_string(),
        "contain" => "contain".to_string(),
        "fill" => "fill".to_string(),
        _ => "cover".to_string(),
    };
    let style = match st.scrim_style.as_str() {
        "dark" | "light" | "tint" | "soft" | "frost" | "spot" | "sheen" | "random" => {
            st.scrim_style.clone()
        }
        _ => "auto".to_string(),
    };
    Resolved {
        palette: p,
        accent,
        titlebar_alpha: av.titlebar_alpha.clamp(0.0, 1.0),
        tabbar_alpha: av.tabbar_alpha.clamp(0.0, 1.0),
        sidebar_alpha: av.sidebar_alpha.clamp(0.0, 1.0),
        term_alpha: av.term_alpha.clamp(0.0, 1.0),
        wallpaper_transparency: av.wallpaper_transparency.clamp(0.0, 1.0),
        wallpaper_scope: scope,
        wallpaper_fit: fit,
        scrim_style: style,
        scrim_alpha: av.scrim_alpha.clamp(0.0, 1.0),
        popup_transparency: av.popup_transparency.clamp(0.0, 1.0),
        zone_defaults: st.zone_defaults,
    }
}

/// 把解析结果写进一个含 Theme global 的窗口。泛型无法约束「T 包含
/// Theme」（生成的 Global 实现绑定在具体组件上），用宏为每个窗口生成。
macro_rules! impl_theme_apply {
    ($fn_name:ident, $win_ty:ty) => {
        /// 把当前主题整套槽位写进窗口的 Theme global。
        /// 返回生效的套（调用方接着刷新终端 buffer 与广播）。
        pub fn $fn_name(win: &$win_ty, st: &ThemeState) -> &'static Palette {
            let r = resolve(st);
            let p = r.palette;
            let t = win.global::<Theme<'_>>();
            t.set_theme_id(p.id.into());
            t.set_theme_name(format!("{} · {}", p.zh, if p.dark { "暗" } else { "亮" }).into());
            t.set_dark(p.dark);
            t.set_slot_root(color(p.root));
            t.set_slot_panel(color(p.panel));
            t.set_slot_palt(color(p.palt));
            t.set_slot_elev(color(p.elev));
            t.set_slot_hov(color(p.hov));
            t.set_slot_act(color(p.act));
            t.set_slot_line(color(p.line));
            t.set_slot_lstr(color(p.lstr));
            t.set_slot_t1(color(p.t1));
            t.set_slot_t2(color(p.t2));
            t.set_slot_t3(color(p.t3));
            t.set_slot_tbg(color(p.tbg));
            t.set_slot_tfg(color(p.tfg));
            t.set_slot_ac(color(r.accent));
            t.set_slot_ok(color(p.ok));
            t.set_slot_wr(color(p.wr));
            t.set_slot_dg(color(p.dg));
            t.set_group_colors(color_model(&p.group24));
            t.set_channel_colors(color_model(&p.channel));
            t.set_titlebar_transparency(1.0 - r.titlebar_alpha);
            t.set_tabbar_transparency(1.0 - r.tabbar_alpha);
            t.set_sidebar_transparency(1.0 - r.sidebar_alpha);
            t.set_term_transparency(1.0 - r.term_alpha);
            t.set_wallpaper_transparency(r.wallpaper_transparency);
            // (#wallpaper-scope-workspace) 0 = 整个界面 / 1 = 主区域 / 2 = 终端。
            t.set_wallpaper_scope(match r.wallpaper_scope.as_str() {
                "workspace" => 1,
                "terminal" => 2,
                _ => 0,
            });
            t.set_wallpaper_fit(r.wallpaper_fit.as_str().into());
            t.set_scrim_style(r.scrim_style.as_str().into());
            t.set_scrim_strength(r.scrim_alpha);
            t.set_popup_transparency(r.popup_transparency);
            // (#zone-default-context) 双击复位的目标值（滑杆语义 = 透明程度）。
            t.set_titlebar_default_transparency(1.0 - r.zone_defaults.titlebar);
            t.set_tabbar_default_transparency(1.0 - r.zone_defaults.tabbar);
            t.set_sidebar_default_transparency(1.0 - r.zone_defaults.sidebar);
            t.set_term_default_transparency(1.0 - r.zone_defaults.term);
            p
        }
    };
}

impl_theme_apply!(apply, crate::ui::AppWindow);
impl_theme_apply!(apply_proc, crate::ui::ProcWindow);
impl_theme_apply!(apply_editor, crate::ui::EditorWindow);
impl_theme_apply!(apply_system_info, crate::ui::SystemInfoWindow);
impl_theme_apply!(apply_about, crate::ui::AboutWindow);

/// (#dpr-hairline 2026-10-11) 把窗口真实缩放(OS 显示缩放)写进 Theme global。
///
/// `theme.slint` 里的 `device-pixel-ratio` 是 Theme **自己声明**的属性,用来遮蔽
/// Slint 内建的窗口属性(Theme 是 global,拿不到窗口上下文)。因此 Rust 侧写
/// `slint::Window::set_device_pixel_ratio()` 对它无效 —— 它的默认值 1.0 会一直
/// 留着,hairline() = 1px/1.0 = 1 **逻辑**px,在 200% 屏上就是 2 个物理像素:
/// 窗口四边框、面板外框、splitter 全比设计值粗一倍。必须显式写这个槽位。
/// （#win-edge-frame-r7 2026-10-18：`radius-win` 现为 8 DIP 的**固定逻辑值**、
/// 不再被 DPR 除 —— 真机实测 DWM 圆角是 8 DIP，随 DPI 缩放。）
macro_rules! impl_theme_dpr {
    ($fn_name:ident, $win_ty:ty) => {
        pub fn $fn_name(win: &$win_ty, scale: f32) {
            if scale.is_finite() && scale > 0.0 {
                win.global::<Theme<'_>>().set_device_pixel_ratio(scale);
            }
        }
    };
}

impl_theme_dpr!(set_dpr, crate::ui::AppWindow);
impl_theme_dpr!(set_dpr_proc, crate::ui::ProcWindow);
impl_theme_dpr!(set_dpr_editor, crate::ui::EditorWindow);
impl_theme_dpr!(set_dpr_system_info, crate::ui::SystemInfoWindow);
impl_theme_dpr!(set_dpr_about, crate::ui::AboutWindow);

/// WCAG 2.1 相对亮度（0.0–1.0）。
pub fn luminance(hex: u32) -> f64 {
    let lin = |v: u32| {
        let v = (v & 0xFF) as f64 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(hex >> 16) + 0.7152 * lin(hex >> 8) + 0.0722 * lin(hex)
}

/// WCAG 对比度（≥1.0）。
pub fn contrast(a: u32, b: u32) -> f64 {
    let (hi, lo) = (luminance(a).max(luminance(b)), luminance(a).min(luminance(b)));
    (hi + 0.05) / (lo + 0.05)
}

/// 保持色相/饱和、调明度，使 `fg` 在 `bg` 上对比度 ≥ `target`。
/// 原色已达标则原样返回（最小干预）。明度二分在 gamma 空间近似即可：
/// 校正幅度小（预设在各主题底上大多已达标）。
pub fn fit_contrast(fg: u32, bg: u32, target: f64) -> u32 {
    if contrast(fg, bg) >= target {
        return fg;
    }
    let darken = luminance(bg) > luminance(fg);
    // 线性插值到黑/白端找首个达标点（24 步足够平滑）
    let (er, eg, eb) = if darken { (0x10, 0x0F, 0x0F) } else { (0xFF, 0xFC, 0xF0) };
    for step in 1..=24u32 {
        let t = step as f64 / 24.0;
        let mix = |c: u32, e: u32| -> u32 {
            let v = (c as f64 * (1.0 - t) + e as f64 * t).round() as u32;
            v.min(255)
        };
        let cand = (mix(fg >> 16, er) << 16) | (mix((fg >> 8) & 0xFF, eg) << 8) | mix(fg & 0xFF, eb);
        if contrast(cand, bg) >= target {
            return cand;
        }
    }
    if darken { 0x100000 } else { 0xFFFFF0 }
}

/// (#hl-rule-guard 2026-10-21) 自定义高亮规则色的终端前景守卫：**保持色相/饱和**，
/// 只沿明度轴收敛到 `target` 对比度。与 `fit_contrast`（线性插到黑白端、会洗
/// 色相）不同，这条走 HSL 细步（1024 档），停下时即「刚好达标的最亮/最暗一档」
/// —— 深底提亮、浅底压暗双向。渲染端（presentation::style_run）与设置弹框
/// 色点（terminal_ui::rule_dot）共用同一函数，保证「色点 = 终端实色」且换主题
/// 自适应；用户在暗底主题下从 47 色卡选出的隐身色（carbon #424242 对
/// mterm-dark 只有 1.74:1）在两端同时被拉回可读。
pub fn fit_rule_fg(fg: u32, bg: u32, target: f64) -> u32 {
    if contrast(fg, bg) >= target {
        return fg;
    }
    let (h, s, l0) = rgb_to_hsl(fg);
    // 方向只由底色决定：底暗（亮度 <0.5）提亮、底亮压暗 —— 始终远离底色。
    // 不能用「bg 亮于 fg」判：fg 已亮于暗底但对比不足时（#651FFF on
    // mterm-dark 2.58:1）那个判据会把色往黑压、越压越糊。
    let lighten = luminance(bg) < 0.5;
    // 与 presentation::darken_light_fg 相同的细步纪律：粗步长会把饱和色一路
    // 推到纯黑/纯白、丢掉色相（#4DC4A8 曾被渲成 #000000）。
    const STEPS: u32 = 1024;
    let mut best = if lighten { 0xFFFFF0 } else { 0x100F0F };
    for i in 1..=STEPS {
        let t = i as f32 / STEPS as f32;
        let new_l = if lighten { l0 + (1.0 - l0) * t } else { l0 * (1.0 - t) };
        let cand = hsl_to_rgb(h, s, new_l);
        if contrast(cand, bg) >= target {
            return cand;
        }
        best = cand;
    }
    best
}

fn rgb_to_hsl(hex: u32) -> (f32, f32, f32) {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if (mx - r).abs() < 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (mx - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> u32 {
    if s < 1e-6 {
        let v = (l * 255.0).round() as u32;
        return (v << 16) | (v << 8) | v;
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| -> f32 {
        if t < 0.0 { t += 1.0; }
        if t > 1.0 { t -= 1.0; }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 0.5 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };
    let r = (hue(h + 1.0 / 3.0) * 255.0).round() as u32;
    let g = (hue(h) * 255.0).round() as u32;
    let b = (hue(h - 1.0 / 3.0) * 255.0).round() as u32;
    (r << 16) | (g << 8) | b
}

/// 计算最终生效的强调色：跟随 → 主题自带；覆盖 → 先做 3:1 守卫
/// （过暗提亮、过亮压暗），并把实际生效值回传给设置页提示。
pub fn resolve_accent(p: &Palette, mode: AccentCfg) -> (u32, u32) {
    let panel = p.panel;
    match mode {
        AccentCfg::Follow => (p.ac, p.ac),
            AccentCfg::Preset(key) => {
            let raw = palettes::ACCENT_PRESETS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, _, v)| *v)
                .unwrap_or(p.ac);
            let fitted = fit_contrast(raw, panel, 3.0);
            (fitted, fitted)
        }
        AccentCfg::Custom(hex) => {
            let fitted = fit_contrast(hex, panel, 3.0);
            (fitted, fitted)
        }
    }
}

/// 强调色守卫信息（设置页 §3.8 的提示行用）：原色 / 生效色 / 面板色 /
/// 对比度 / 是否被 3:1 守卫自动调过。跟随态原色=生效色=主题自带 ac。
pub struct AccentGuard {
    pub raw: u32,
    pub eff: u32,
    pub panel: u32,
    pub ratio: f64,
    pub adjusted: bool,
}

pub fn accent_guard(p: &Palette, mode: AccentCfg) -> AccentGuard {
    let raw = match mode {
        AccentCfg::Follow => p.ac,
        AccentCfg::Preset(key) => palettes::ACCENT_PRESETS
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, _, v)| *v)
            .unwrap_or(p.ac),
        AccentCfg::Custom(hex) => hex,
    };
    let (eff, _) = resolve_accent(p, mode);
    AccentGuard {
        raw,
        eff,
        panel: p.panel,
        ratio: contrast(eff, p.panel),
        adjusted: eff != raw,
    }
}

/// 全部预设 id（选择器分组用）：radix → brand → terminal → meat
pub fn ordered_ids() -> Vec<&'static str> {
    palettes::PALETTES.iter().map(|p| p.id).collect()
}

/// 「常用主题」上限（#theme-gallery 2026-10-18：外观页画廊 = 常用快捷位，
/// ≤6 + 「＋更多」，写满时提示「先移出一套」）。6 卡 + 更多 = 7 张 82px 卡
/// 正好落在设置页内容宽内（按高保真 wallpaper-hifi-2026-10-17 的定稿）。
pub const FAVORITE_LIMIT: usize = 6;

/// 强调色的色相名（#theme-dedupe §5 选择器搜索用）：八段中文标签
/// 「红 / 橙 / 黄 / 绿 / 青 / 蓝 / 紫 / 粉」；无彩度返回「灰」。
pub fn hue_label(c: u32) -> &'static str {
    let r = ((c >> 16) & 0xFF) as f32 / 255.0;
    let g = ((c >> 8) & 0xFF) as f32 / 255.0;
    let b = (c & 0xFF) as f32 / 255.0;
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    if d == 0.0 {
        return "灰";
    }
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    match (h * 60.0).rem_euclid(360.0) {
        x if x < 21.0 || x >= 340.0 => "红",
        x if x < 51.0 => "橙",
        x if x < 81.0 => "黄",
        x if x < 141.0 => "绿",
        x if x < 201.0 => "青",
        x if x < 251.0 => "蓝",
        x if x < 301.0 => "紫",
        _ => "粉",
    }
}

/// 按套的明暗取默认主题 id（#theme-dedupe 2026-10-15：门面 mterm 两套）。
pub fn default_for(dark: bool) -> &'static str {
    if dark { palettes::DEFAULT_DARK } else { palettes::DEFAULT_LIGHT }
}

/// 刷新分组取色面板的候选 24 色（§3-调整⑤：按当前主题查表）。切主题时由
/// on_set_theme 路径调用。
pub fn rebuild_group_palette(win: &crate::ui::AppWindow, p: &Palette) {
    let swatches: Vec<crate::ui::GroupSwatch> = p
        .group24
        .iter()
        .map(|v| crate::ui::GroupSwatch {
            hex: format!("#{:06X}", v).into(),
            swatch: color(*v),
        })
        .collect();
    win.set_group_palette(ModelRc::from(Rc::new(VecModel::from(swatches))));
}

pub fn by_id(id: &str) -> Option<&'static Palette> {
    palettes::find(id)
}

/// 非法 id 的兜底：跟随系统探测结果。
pub fn palette_or_default(id: &str, dark: bool) -> &'static Palette {
    palettes::find(id).unwrap_or_else(|| palettes::find(default_for(dark)).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use palettes::PALETTES;

    /// 0xRRGGBB 的 HSL 色相（度，0..360）。灰族同源断言的公共工具。
    fn hue_deg(c: u32) -> f32 {
        let (r, g, b) = (
            ((c >> 16) & 0xFF) as f32 / 255.0,
            ((c >> 8) & 0xFF) as f32 / 255.0,
            (c & 0xFF) as f32 / 255.0,
        );
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        let d = mx - mn;
        if d == 0.0 {
            return 0.0;
        }
        let h = if mx == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if mx == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0).rem_euclid(360.0)
    }

    #[test]
    fn table_shape() {
        assert_eq!(PALETTES.len(), 28);
        assert!(find("graphite-dark").is_some());
        // （§6.3 删）meat 迁移档「不得进选择器」的旧断言已移除：它守的是一个从不
        // 在表里的对象；意外回潮会被上面的 len == 28 间接炸出。
        // 「已删套不得回潮」——黑名单汇总（高保真 §6.3：把散着的历史断言收成一条）。
        // #theme-dedupe 下线的 12 套，任何一套回到表里都是回潮。
        for gone in [
            "slate-dark", "slate-light", "mauve-dark", "mauve-light",
            "sage-dark", "sage-light", "sand-dark", "sand-light",
            "tokyonight-dark", "tokyonight-light", "ayu-dark", "github-dark",
        ] {
            assert!(find(gone).is_none(), "已删套不得回潮：{gone}");
        }
        // 明暗配对补齐（#theme-dedupe 解封）：dracula-light 从「已删」转为正式成员。
        assert!(find("dracula-light").is_some(), "德古拉 · 亮（明暗配对补齐）");
        assert!(find("everforest-light").is_some(), "常青 · 亮（明暗配对补齐）");
        assert!(find("mterm-dark").is_some(), "MTerm 套（Islands Dark 移植）");
        // ⚠️ 2026-10-11 用户决策改：原断言是 `mterm-light` 不存在（"上游只有暗
        // 色档，不硬凑档位"）。现在 mterm-light = MTerm 自有亮色档「素纸」，
        // id 取名与暗档配对。**它不是上游 Islands 的亮档** —— 色值是自主设计，
        // 原决策的意图（禁"把暗档亮化拷贝充数"）由 mterm_light_paper_rules_hold
        // 守卫继续兜住。
        assert!(find("mterm-light").is_some(), "MTerm · 亮（自有设计「素纸」，与 mterm-dark 配对）");
        // 显示名不带明暗后缀：后缀由 theme_name/选择器统一拼（界面显示「MTerm · 暗」
        // 与配对的「MTerm · 亮」——两档 zh 同为 "MTerm"，靠 dark 列分族）。
        assert_eq!(find("mterm-dark").unwrap().zh, "MTerm");
        assert_eq!(find("mterm-light").unwrap().zh, "MTerm");
        assert!(!find("mterm-light").unwrap().dark, "mterm-light 是亮档");
        assert!(find("mterm-dark").unwrap().dark, "mterm-dark 是暗档");
        // id 唯一
        let mut ids: Vec<_> = PALETTES.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 28);
    }

    #[test]
    fn group_and_channel_shape() {
        for p in PALETTES {
            assert_eq!(p.group24.len(), 24, "{}", p.id);
            assert_eq!(p.ansi16.len(), 16, "{}", p.id);
            assert_eq!(p.channel.len(), 4, "{}", p.id);
        }
    }

    #[test]
    fn hue_labels_cover_the_wheel() {
        assert_eq!(hue_label(0x1FC8B4), "青", "MTerm 青");
        assert_eq!(hue_label(0xFF0000), "红");
        assert_eq!(hue_label(0x00FF00), "绿");
        assert_eq!(hue_label(0x0000FF), "蓝");
        assert_eq!(hue_label(0xF279B9), "粉");
        assert_eq!(hue_label(0x818181), "灰");
    }

    #[test]
    fn contrast_math() {
        // tab 稿实测：#993556 on #25282f ≈ 2.11、#c24e75 达标
        let low = contrast(0x993556, 0x25282F);
        assert!((low - 2.11).abs() < 0.06, "实得 {low}");
        assert!(contrast(0xC24E75, 0x25282F) >= 3.0);
        assert!(contrast(0x000000, 0xFFFFFF) > 20.0);
    }

    #[test]
    fn fit_contrast_minimal() {
        // 已达标 → 原样
        assert_eq!(fit_contrast(0xFFFFFF, 0x000000, 3.0), 0xFFFFFF);
        // 不达标 → 提亮至达标
        let fitted = fit_contrast(0x993556, 0x25282F, 3.0);
        assert!(contrast(fitted, 0x25282F) >= 3.0);
        // 亮底 → 压深
        let fitted2 = fit_contrast(0xFFC53D, 0xFCFCFC, 3.0);
        assert!(contrast(fitted2, 0xFCFCFC) >= 3.0);
        assert!(luminance(fitted2) < luminance(0xFFC53D));
    }

    /// (#hl-rule-guard) 自定义高亮规则色的守卫契约：47 色预设卡（取色 UI 复用
    /// 强调色那套）× 28 套主题终端底，全部拉回 ≥3:1；且只调明度、色相不失
    /// （通道极差 >8，防「守卫把彩色字洗成黑白」）。暗色模式过去不设防，
    /// carbon #424242 对 mterm-dark 只有 1.74:1 —— 用户一点就配出隐身高亮。
    #[test]
    fn rule_guard_covers_every_preset_card_on_every_terminal_bg() {
        for pal in PALETTES.iter() {
            for &(_, _, raw) in palettes::ACCENT_PRESETS {
                let fitted = fit_rule_fg(raw, pal.tbg, 3.0);
                let c = contrast(fitted, pal.tbg);
                assert!(
                    c >= 3.0,
                    "{}: 卡色 #{raw:06X} → #{fitted:06X} 对 tbg #{:06X} 只有 {c:.2}:1",
                    pal.id,
                    pal.tbg
                );
                let spread = |v: u32| {
                    let r = (v >> 16) & 0xFF;
                    let g = (v >> 8) & 0xFF;
                    let b = v & 0xFF;
                    r.max(g).max(b) - r.min(g).min(b)
                };
                // 已是灰族的卡色（carbon/lead 等）本来就没有色相，跳过色相断言
                if spread(raw) > 8 {
                    assert!(
                        spread(fitted) > 8,
                        "{}: 卡色 #{raw:06X} 守卫后 #{fitted:06X} 色相被抹掉",
                        pal.id
                    );
                }
            }
        }
        // 已达标色原样返回（守卫不做无谓改色）
        assert_eq!(fit_rule_fg(0xFFFFFF, 0x181A1D, 3.0), 0xFFFFFF);
    }

    /// (#ansi-batch3 2026-10-21) 批次三验收线固化为断言，防数据回退：
    /// * 全表彩色槽 ≥4.5（A 案压齐）；
    /// * 亮档 0/7/8/15、暗档 8 同判；
    /// * 暗档 0 深于 tbg（TUI 显式黑底要可见，禁与界面底同值）；
    /// * 门面青槽与 accent 拉开 ≥15° 且对「划选背景（accent 40% on tbg）」
    ///   合成底 ≥3:1 —— 划选一段青字（ls 的 symlink、高亮预设的 time/num）
    ///   不能再浑成一团；
    /// * 亮档 ansi16bg 必须存在（TUI 背景填充走浅彩表，修「亮主题 TUI 黑底
    ///   白字 1.0~1.5:1」）；
    /// * group24 同明暗组内不得逐位相同（26 套派生自各自 ac 色相起环）。
    #[test]
    fn ansi_batch3_contrast_and_decoupling_hold() {
        let sel_bg = |p: &palettes::Palette| -> u32 {
            // accent 40% 叠 tbg 的合成底（按通道混色）
            let (ar, ag, ab) = ((p.ac >> 16) & 0xFF, (p.ac >> 8) & 0xFF, p.ac & 0xFF);
            let (tr, tg, tb) = ((p.tbg >> 16) & 0xFF, (p.tbg >> 8) & 0xFF, p.tbg & 0xFF);
            (((tr as f64 + (ar as f64 - tr as f64) * 0.4).round() as u32) << 16)
                | (((tg as f64 + (ag as f64 - tg as f64) * 0.4).round() as u32) << 8)
                | ((tb as f64 + (ab as f64 - tb as f64) * 0.4).round() as u32)
        };
        for p in PALETTES.iter() {
            for i in [1usize, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14] {
                let c = contrast(p.ansi16[i], p.tbg);
                assert!(c >= 4.5, "{}: a{i}={:06X} on tbg 只有 {c:.2}", p.id, p.ansi16[i]);
            }
            if !p.dark {
                for i in [0usize, 7, 8, 15] {
                    let c = contrast(p.ansi16[i], p.tbg);
                    assert!(c >= 4.5, "{}: 亮档 a{i} 只有 {c:.2}", p.id);
                }
                assert!(p.ansi16bg.is_some(), "{}: 亮档缺 ansi16bg（TUI 背景填充表）", p.id);
            } else {
                let c = contrast(p.ansi16[8], p.tbg);
                assert!(c >= 4.5, "{}: 暗档 a8 只有 {c:.2}", p.id);
                assert!(
                    luminance(p.ansi16[0]) < luminance(p.tbg) * 0.9,
                    "{}: a0={:06X} 不比 tbg={:06X} 深（TUI 黑底不可见）",
                    p.id, p.ansi16[0], p.tbg
                );
                assert_ne!(p.ansi16[0], p.tbg, "{}: a0 与 tbg 同值", p.id);
            }
        }
        // 门面解耦判据
        for pid in ["mterm-dark", "mterm-light"] {
            let p = find(pid).unwrap();
            let dh = (hue_deg(p.ansi16[6]) - hue_deg(p.ac)).abs().min(360.0 - (hue_deg(p.ansi16[6]) - hue_deg(p.ac)).abs());
            assert!(dh >= 15.0, "{pid}: a6 与 ac 色相差 {dh:.0}° <15°（划选会浑）");
            let sel = sel_bg(p);
            for i in [6usize, 14] {
                let c = contrast(p.ansi16[i], sel);
                assert!(c >= 3.0, "{pid}: a{i} 对划选合成底 #{sel:06X} 只有 {c:.2}",);
            }
        }
        // group24 同明暗组唯一
        for group in [true, false] {
            let mut seen: Vec<&[u32; 24]> = Vec::new();
            for p in PALETTES.iter().filter(|p| p.dark == group) {
                assert!(
                    !seen.iter().any(|g| *g == &p.group24),
                    "{} 的 group24 与此前某套逐位相同（派生起相回潮）",
                    p.id
                );
                seen.push(&p.group24);
            }
        }
    }

    #[test]
    fn mterm_accent_survives_guard() {        // MTerm 套的强调色是**门面定档**青 #1FC8B4（#theme-dedupe 2026-10-15 起，
        // terminal.ansiCyan 同步；此前为上游青 #2AACB8）：在 panel #181A1D 上
        // 对比约 8:1 ≥ 3.0，故「跟随主题」时 fit_contrast 原样返回、不产生色相
        // 漂移。守卫若改色，说明该套的 panel/ac 被改坏了。
        let p = find("mterm-dark").unwrap();
        assert_eq!(p.ac, 0x1FC8B4);
        assert_eq!(fit_contrast(p.ac, p.panel, 3.0), p.ac);
        assert!(contrast(p.ac, p.panel) >= 3.0);
    }

    /// 「MTerm · 亮」（mterm-light，自有设计「素纸」）的三条配色纪律钉死，防止日后
/// 单点改色把协调性改回去：① 强调色过 3:1 守卫（不产生色相漂移）；② 灰族同源
///（tfg=t1、ANSI white=t1；brightBlack 与 t3 同色相 ≤10°，#theme-dedupe §6.4 放宽）；
/// ③ 12 个彩色槽全部 ≥4.5:1。
///
    /// 另带一条**溯源守卫**：禁止把暗档亮化拷贝充当亮档。
#[test]
    fn mterm_light_paper_rules_hold() {
        let p = find("mterm-light").expect("MTerm · 亮必须在色表里");
        assert_eq!(p.kind, "mterm");
        // 溯源：与暗档不是同一套色换个明暗
        let d = find("mterm-dark").unwrap();
        assert_ne!(p.root, d.root, "不能是 mterm-dark 的底色换个明暗");
        assert_ne!(p.ac, d.ac, "不能沿用暗档强调色");
        assert_ne!(p.ansi16, d.ansi16, "不能沿用暗档 ANSI（亮底 bright 档方向相反）");
        // 门面共同红线（§6.4 五条：t3≥3.0 / t2≠t3 / panel==tbg / 语义同源 / g24 唯一）
        assert_mterm_facade_rules(p);
        // ① 强调色守卫
        assert_eq!(p.ac, 0x1F766B);
        assert_eq!(fit_contrast(p.ac, p.panel, 3.0), p.ac, "强调色过守卫时不应被改色");
        assert!(contrast(p.ac, p.panel) >= 3.0);
        // ② 灰族唯一：终端前景 = 正文一级灰；ANSI 白/亮黑并入同一灰族
        assert_eq!(p.tfg, p.t1, "term-fg 必须等于 t1（禁双灰）");
        assert_eq!(p.ansi16[7], p.t1, "ANSI 7 white = t1");
        // #theme-dedupe §6.4：纪律从「a8 == t3」放宽为「同色相 ≤10°」——旧断言把
        // 亮档一个 2.51:1 的 t3 固化成了标准色；放宽后意图（灰族同源）保留、
        // 不合格值不再被保护。亮档 a8 保持上游 #A49E95、t3 另立 #968F85。
        let dh = (hue_deg(p.ansi16[8]) - hue_deg(p.t3)).abs();
        let dh = dh.min(360.0 - dh);
        assert!(dh <= 10.0, "灰族同源：a8 与 t3 色相差 {dh:.1}° > 10°");
        // ③ 12 彩色槽 ≥ 4.5:1（槽 1-6 / 9-14；0/7/8/15 按终端惯例不判）
        for i in [1usize, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14] {
            let c = contrast(p.ansi16[i], p.tbg);
            assert!(c >= 4.5, "ANSI{i}={:06X} on tbg 只有 {c:.2}", p.ansi16[i]);
        }
        // 频道 4 槽在两个 tab 底（非选中 tab / 选中=term-bg）上均 ≥3.0
        for c in p.channel {
            assert!(contrast(c, p.tab) >= 3.0, "频道色 {c:06X} on tab 不足 3.0");
            assert!(contrast(c, p.tbg) >= 3.0, "频道色 {c:06X} on tbg 不足 3.0");
        }
    }

    /// mterm 门面两套的**共同红线**（高保真 §6.4 定稿，2026-10-15）：
    /// ① t3 对 panel ≥ 3.0 —— 辅助级文字的独立门槛，与正文级 4.5 分开计；
    /// ② t2 与 t3 不得同值 —— 两级分工：t2 管未激活、t3 管占位符与注释；
    /// ③ 面板底 == 终端底 —— 设计选择（终端与面板视觉连续），显式钉住防"顺手修正"；
    /// ④ 语义色与同名 ANSI 槽 Δ色相 ≤30° —— 守住「界面 ↔ 终端同源」；
    /// ⑤ group24 不与其余套逐位相同 —— 取色面板独此一家。
    fn assert_mterm_facade_rules(p: &palettes::Palette) {
        assert!(
            contrast(p.t3, p.panel) >= 3.0,
            "{}: t3 on panel 只有 {:.2}（< 3.0）",
            p.id,
            contrast(p.t3, p.panel)
        );
        assert_ne!(p.t2, p.t3, "{}: t2 与 t3 不得同值（分级分工）", p.id);
        assert_eq!(p.panel, p.tbg, "{}: 面板底与终端底必须同色（设计选择）", p.id);
        // ac↔ansi6（青）/ ok↔ansi2（绿）/ wr↔ansi3（黄）/ dg↔ansi1（红）
        for (name, sem, idx) in [("ac", p.ac, 6usize), ("ok", p.ok, 2), ("wr", p.wr, 3), ("dg", p.dg, 1)] {
            let dh = (hue_deg(sem) - hue_deg(p.ansi16[idx])).abs();
            let dh = dh.min(360.0 - dh);
            assert!(
                dh <= 30.0,
                "{}: {} 与 ansi{} 色相差 {dh:.0}° > 30°（界面↔终端同源被破坏）",
                p.id,
                name,
                idx
            );
        }
        for other in PALETTES.iter().filter(|o| o.id != p.id) {
            assert_ne!(p.group24, other.group24, "{} 的取色面板与 {} 逐位相同", p.id, other.id);
        }
    }

    #[test]
    fn mterm_dark_facade_rules_hold() {
        let p = find("mterm-dark").unwrap();
        assert_mterm_facade_rules(p);
        // §4.7 体检表：12 终端彩槽全部 ≥4.5:1（全表最严的一条）
        for i in [1usize, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14] {
            let c = contrast(p.ansi16[i], p.tbg);
            assert!(c >= 4.5, "ANSI{i}={:06X} on tbg 只有 {c:.2}", p.ansi16[i]);
        }
        // 频道 4 槽在 tab 底与终端底双侧 ≥3.0（全表独有的设计资产）
        for c in p.channel {
            assert!(contrast(c, p.tab) >= 3.0, "频道色 {c:06X} on tab 不足 3.0");
            assert!(contrast(c, p.tbg) >= 3.0, "频道色 {c:06X} on tbg 不足 3.0");
        }
    }

    /// (#mode-values 2026-10-17) 外观数值按主题明暗选槽：同一份 ThemeState 里
    /// 暗/亮两套不同值，切主题即换一套 —— 用户要的「暗亮不共用数值」。
    #[test]
    fn resolve_picks_values_by_theme_mode() {
        let mut dark_av = AppearanceValues::default();
        dark_av.titlebar_alpha = 0.55;
        dark_av.popup_transparency = 0.20;
        let mut light_av = AppearanceValues::default();
        light_av.titlebar_alpha = 0.95;
        light_av.popup_transparency = 0.60;
        let mut st = ThemeState {
            theme_id: "mterm-dark".into(),
            accent_mode: AccentCfg::Follow,
            appearance: dark_av.clone(),
            zone_defaults: ZoneDefaults::default(),
            wallpaper_scope: "window".into(),
            wallpaper_fit: "cover".into(),
            scrim_style: "auto".into(),
            dark_pref: true,
        };
        let r = resolve(&st);
        assert!((r.titlebar_alpha - 0.55).abs() < 1e-6);
        assert!((r.popup_transparency - 0.20).abs() < 1e-6);

        st.theme_id = "mterm-light".into();
        st.appearance = light_av;
        let r = resolve(&st);
        assert!((r.titlebar_alpha - 0.95).abs() < 1e-6, "亮主题应取亮槽");
        assert!((r.popup_transparency - 0.60).abs() < 1e-6);

        // 切回暗主题：暗槽原值仍在。
        st.theme_id = "mterm-dark".into();
        st.appearance = dark_av;
        assert!((resolve(&st).titlebar_alpha - 0.55).abs() < 1e-6);
    }

    #[test]
    fn defaults_are_mterm() {
        assert_eq!(default_for(true), "mterm-dark");
        assert_eq!(default_for(false), "mterm-light");
    }

    /// (#wallpaper-auto-fit 2026-10-18) `resolve()` 曾把 "auto" 静默回落成
    /// "cover"（只认 contain/fill），导致设置页选了「自动」、实机却按「填充」画。
    /// 四个合法值必须原样透传，未知值才回落 cover。
    #[test]
    fn wallpaper_fit_keeps_auto() {
        let mut st = ThemeState {
            theme_id: "mterm-dark".into(),
            accent_mode: AccentCfg::Follow,
            appearance: Default::default(),
            zone_defaults: ZoneDefaults::default(),
            wallpaper_scope: "window".into(),
            wallpaper_fit: "auto".into(),
            scrim_style: "auto".into(),
            dark_pref: true,
        };
        for (stored, resolved) in [
            ("auto", "auto"),
            ("cover", "cover"),
            ("contain", "contain"),
            ("fill", "fill"),
            ("bogus", "cover"),
        ] {
            st.wallpaper_fit = stored.to_string();
            assert_eq!(resolve(&st).wallpaper_fit, resolved, "stored={stored}");
        }
    }
}
