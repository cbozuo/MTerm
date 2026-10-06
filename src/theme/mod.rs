//! 主题系统（theme-wallpaper-split 稿落地）。
//!
//! 色表查表在 Rust 侧（`palettes.rs`，生成文件）：`apply_theme` 一次性把
//! 当前套的 20 个 UI 槽 / 分组色 / 频道色写进 Slint `Theme` global，
//! 终端 ANSI 由 `presentation.rs` 查同一张表——UI 与终端同源。
//! 强调色三态（跟随主题 / 9 预设 / 自定义）在此计算，含 3:1 对比度守卫。

pub mod palettes;

use palettes::Palette;

pub use palettes::find;

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

/// 强调色三态（§4-L2）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccentMode {
    /// 跟随主题（默认）——语义是「没有覆盖」，不是又一个 if
    Follow,
    /// 9 预设之一（ACCENT_PRESETS 的 key）
    Preset(&'static str),
    /// 自定义 #RRGGBB
    Custom(u32),
}

/// 计算最终生效的强调色：跟随 → 主题自带；覆盖 → 先做 3:1 守卫
/// （过暗提亮、过亮压暗），并把实际生效值回传给设置页提示。
pub fn resolve_accent(p: &Palette, mode: AccentMode) -> (u32, u32) {
    let panel = p.panel;
    match mode {
        AccentMode::Follow => (p.ac, p.ac),
        AccentMode::Preset(key) => {
            let raw = palettes::ACCENT_PRESETS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, _, v)| *v)
                .unwrap_or(p.ac);
            let fitted = fit_contrast(raw, panel, 3.0);
            (fitted, fitted)
        }
        AccentMode::Custom(hex) => {
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
        assert_eq!(PALETTES.len(), 38);
        assert!(find("graphite-dark").is_some());
        assert!(find("meat-light").is_some());
        assert!(find("dracula-light").is_none(), "已删套不得回潮");
        // id 唯一
        let mut ids: Vec<_> = PALETTES.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 38);
    }

    #[test]
    fn meat_light_keeps_bg_table() {
        let m = find("meat-light").unwrap();
        assert!(m.ansi16bg.is_some(), "亮色 TUI 背景双表特性保留");
        assert!(find("meat-dark").unwrap().ansi16bg.is_none());
    }

    #[test]
    fn meat_elev_refit() {
        // tab 稿：非选中 tab 底降对比（暗 #25282f / 亮 #e2e2e8）
        assert_eq!(find("meat-dark").unwrap().elev, 0x25282F);
        assert_eq!(find("meat-light").unwrap().elev, 0xE2E2E8);
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
