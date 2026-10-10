//! 主题系统（theme-wallpaper-split 稿落地）。
//!
//! 色表查表在 Rust 侧（`palettes.rs`，生成文件）：`apply_theme` 一次性把
//! 当前套的 20 个 UI 槽 / 分组色 / 频道色写进 Slint `Theme` global，
//! 终端 ANSI 由 `presentation.rs` 查同一张表——UI 与终端同源。
//! 强调色三态（跟随主题 / 9 预设 / 自定义）在此计算，含 3:1 对比度守卫。

pub mod palettes;

use palettes::Palette;

pub use palettes::find;

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
    pub panel_alpha: f32,
    pub term_alpha: f32,
    pub wallpaper_visible: f32,
    pub popup_transparency: i32,
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
            "custom" => parse_hex(custom).map(AccentCfg::Custom).unwrap_or(AccentCfg::Follow),
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

/// 解析后的整套应用状态（apply 各窗口版本的公共输入）。
pub struct Resolved {
    pub palette: &'static Palette,
    pub accent: u32,
    pub panel_alpha: f32,
    pub term_alpha: f32,
    pub wallpaper_visible: f32,
    pub popup_transparency: i32,
}

/// 计算（与窗口无关）：主题套解析 + 强调色三态叠加 + 三透明度钳制。
pub fn resolve(st: &ThemeState) -> Resolved {
    let dark_now = match st.theme_id.is_empty() {
        true => st.dark_pref,
        false => by_id(&st.theme_id).map(|p| p.dark).unwrap_or(st.dark_pref),
    };
    let p = palette_or_default(&st.theme_id, dark_now);
    let (accent, _) = resolve_accent(p, st.accent_mode);
    Resolved {
        palette: p,
        accent,
        panel_alpha: st.panel_alpha.clamp(0.30, 1.0),
        // 终端可读性下限（§1）：无论滑杆怎么拖，终端至少 75% 不透明。
        term_alpha: st.term_alpha.clamp(0.75, 1.0),
        wallpaper_visible: st.wallpaper_visible.clamp(0.0, 1.0),
        popup_transparency: st.popup_transparency.clamp(0, 2),
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
            t.set_panel_alpha(r.panel_alpha);
            t.set_term_alpha(r.term_alpha);
            t.set_wallpaper_visible(r.wallpaper_visible);
            t.set_popup_transparency(r.popup_transparency);
            p
        }
    };
}

impl_theme_apply!(apply, crate::ui::AppWindow);
impl_theme_apply!(apply_proc, crate::ui::ProcWindow);
impl_theme_apply!(apply_editor, crate::ui::EditorWindow);
impl_theme_apply!(apply_system_info, crate::ui::SystemInfoWindow);
impl_theme_apply!(apply_about, crate::ui::AboutWindow);

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

/// 解析 `#RRGGBB`（自定义强调色输入）；非法返回 None。
pub fn parse_hex(s: &str) -> Option<u32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 7 || !s.starts_with('#') {
        let s2 = s.trim_start_matches('#');
        if s2.len() != 6 || !s2.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        return u32::from_str_radix(s2, 16).ok();
    }
    u32::from_str_radix(&s[1..], 16).ok()
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

/// 全部预设 id（选择器分组用）：radix → brand → terminal → meat
pub fn ordered_ids() -> Vec<&'static str> {
    palettes::PALETTES.iter().map(|p| p.id).collect()
}

/// 按套的明暗取默认主题 id（已确认决策：新老用户一律用新底色 graphite 族）。
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

    #[test]
    fn table_shape() {
        assert_eq!(PALETTES.len(), 37);
        assert!(find("graphite-dark").is_some());
        assert!(find("meat-light").is_none(), "迁移保留档不进选择器（用户决策）");
        assert!(find("dracula-light").is_none(), "已删套不得回潮");
        assert!(find("islands-dark").is_some(), "MTerm 套（Islands Dark 移植）");
        assert!(find("islands-light").is_none(), "上游只有暗色档（用户决策）");
        // 显示名不带明暗后缀：后缀由 theme_name/选择器统一拼（界面显示「MTerm · 暗」）。
        assert_eq!(find("islands-dark").unwrap().zh, "MTerm");
        // id 唯一
        let mut ids: Vec<_> = PALETTES.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 37);
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

    #[test]
    fn islands_accent_survives_guard() {
        // MTerm 套的强调色是**项目定档**的上游青 #2AACB8（terminal.ansiCyan）：
        // 在 panel #181A1D 上对比 6.39 ≥ 3.0，故「跟随主题」时 fit_contrast
        // 原样返回、不产生色相漂移。守卫若改色，说明该套的 panel/ac 被改坏了。
        let p = find("islands-dark").unwrap();
        assert_eq!(p.ac, 0x2AACB8);
        assert_eq!(fit_contrast(p.ac, p.panel, 3.0), p.ac);
        assert!(contrast(p.ac, p.panel) >= 3.0);
    }

    #[test]
    fn parse_hex_ok() {
        assert_eq!(parse_hex("#7C5CFF"), Some(0x7C5CFF));
        assert_eq!(parse_hex("7C5CFF"), Some(0x7C5CFF));
        assert_eq!(parse_hex("#zzzzzz"), None);
        assert_eq!(parse_hex("#12345"), None);
    }

    #[test]
    fn defaults_are_graphite() {
        assert_eq!(default_for(true), "graphite-dark");
        assert_eq!(default_for(false), "graphite-light");
    }
}
