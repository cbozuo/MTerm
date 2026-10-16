//! (#ansi-yellow-fix 2026-10-16) 亮色主题 ANSI 黄色槽的可读性回归。
//!
//! 用户反馈：「亮色主题，终端里面的黄色字体 看不见」。
//!
//! 根因：9 套亮色主题的 yellow（ANSI 槽 3）/ brightYellow（槽 11）直接照搬
//! 上游原色，可那些黄本来是配深色底的 —— 落到浅色 tbg 上对比度只有
//! 2.05~4.50:1（ayu-light 的亮黄最低 2.05，肉眼几乎读不出）。验收线在
//! `scripts/_ansi-group.json` 的 `_meta.ansi_check`：12 彩色槽 ≥ 4.5:1。
//!
//! 修正方式：保 H/S 降 L 压到达标线（脚本 `scripts/_audit/fix_light_yellow.py`，
//! 真源是 `_ansi-group.json`，改完由 `node scripts/gen_theme_rust.js` 重生成
//! `palettes.rs`），槽 11 压到比槽 3 更深 —— 沿用 mterm-light 的
//! 「亮底 bright=更深同色相」纪律。**暗色档一律不动**。
//!
//! 本测试锁死四件事：
//!   ① 每套亮色主题的槽 3 / 槽 11 对 tbg 的对比度 ≥ 4.5；
//!   ② 本次修过的 9 套，槽 11 必须严格深于槽 3；
//!   ③ 暗色主题黄色两槽与修正前逐值一致（不许被顺手压暗）；
//!   ④ 用户点名过的三套锁死具体色值（观感回归）。

use mterm::theme::palettes::{Palette, PALETTES};

/// sRGB → 相对亮度（WCAG 2.x 定义，与 gen_ansi_group.js 的 contrast 同式）。
fn rel_luminance(rgb: (u8, u8, u8)) -> f64 {
    fn lin(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(rgb.0) + 0.7152 * lin(rgb.1) + 0.0722 * lin(rgb.2)
}

fn rgb_of(v: u32) -> (u8, u8, u8) {
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn contrast(a: u32, b: u32) -> f64 {
    let (la, lb) = (rel_luminance(rgb_of(a)), rel_luminance(rgb_of(b)));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn hex(v: u32) -> String {
    format!("#{:06X}", v)
}

fn pal(id: &str) -> &'static Palette {
    PALETTES
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("色表缺 {id}"))
}

const SLOT_YELLOW: usize = 3;
const SLOT_BRIGHT_YELLOW: usize = 11;
const SLOT_WHITE: usize = 7;
const SLOT_BRIGHT_BLACK: usize = 8;
const SLOT_BRIGHT_WHITE: usize = 15;
/// 验收线 = `_meta.ansi_check` 的 4.5:1（生成脚本按 4.6 / 4.7 打表，余量不在测试里算）。
const MIN: f64 = 4.5;

#[test]
fn light_themes_keep_ansi_yellow_readable() {
    let mut checked = 0usize;
    for p in PALETTES.iter().filter(|p: &&Palette| !p.dark) {
        let y = p.ansi16[SLOT_YELLOW];
        let by = p.ansi16[SLOT_BRIGHT_YELLOW];
        let cy = contrast(y, p.tbg);
        let cby = contrast(by, p.tbg);
        assert!(
            cy >= MIN,
            "{}：ANSI yellow {} 对 tbg {} 只有 {cy:.2}:1（< {MIN}）—— 黄字在浅底终端里读不出来",
            p.id,
            hex(y),
            hex(p.tbg)
        );
        assert!(
            cby >= MIN,
            "{}：ANSI brightYellow {} 对 tbg {} 只有 {cby:.2}:1（< {MIN}）",
            p.id,
            hex(by),
            hex(p.tbg)
        );
        checked += 1;
    }
    assert_eq!(checked, 14, "亮色主题数量变了（现 {checked}），色表可能增删了套");
}

/// 本次修过的 9 套：槽 11 必须严格深于槽 3（亮底 bright=更深同色相）。
/// 注：`graphite-light` 是 Radix 色阶派生、normal 与 bright 同值（既有设计，
/// 不在本次修正范围），所以只对下面这张清单断言严格小于。
#[test]
fn fixed_light_themes_bright_yellow_is_deeper_than_normal() {
    const FIXED: &[&str] = &[
        "nord-light",
        "catppuccin-light",
        "rosepine-light",
        "everforest-light",
        "gruvbox-material-light",
        "ayu-light",
        "flexoki-light",
        "solarized-light",
        "darcula-light",
    ];
    assert_eq!(FIXED.len(), 9, "修正套数变了，同步改脚本与本测试");
    for id in FIXED {
        let p = pal(id);
        assert!(!p.dark, "{id} 应是亮色档");
        let ly = rel_luminance(rgb_of(p.ansi16[SLOT_YELLOW]));
        let lb = rel_luminance(rgb_of(p.ansi16[SLOT_BRIGHT_YELLOW]));
        assert!(
            lb < ly,
            "{}：亮底上 brightYellow {} 不该比 yellow {} 更亮（亮度 {lb:.4} vs {ly:.4}）",
            id,
            hex(p.ansi16[SLOT_BRIGHT_YELLOW]),
            hex(p.ansi16[SLOT_YELLOW])
        );
    }
}

/// 这次修正只许动亮色档 —— 暗色档的黄是配深底调的，压暗反而看不清。
/// 快照取自 #ansi-yellow-fix 之前的 palettes.rs（git HEAD）。
#[test]
fn dark_themes_ansi_yellow_was_left_alone() {
    const BEFORE: &[(&str, u32, u32)] = &[
        ("mterm-dark", 0xE8A33E, 0xF0B95E),
        ("onedark-dark", 0xD19A66, 0xD19A66),
        ("graphite-dark", 0xF5E147, 0xF6EEB4),
        ("nord-dark", 0xEBCB8B, 0xEBCB8B),
        ("catppuccin-dark", 0xF9E2AF, 0xF9E2AF),
        ("dracula-dark", 0xF1FA8C, 0xFFFFA5),
        ("rosepine-dark", 0xF6C177, 0xF6C177),
        ("everforest-dark", 0xDBBC7F, 0xE1C793),
        ("kanagawa-wave", 0xC0A36E, 0xE6C384),
        ("gruvbox-material-dark", 0xD8A657, 0xDEB472),
        ("ayu-mirage", 0xFDCC60, 0xFFD165),
        ("flexoki-dark", 0xAD8301, 0xD0A215),
        ("solarized-dark", 0xB58900, 0x657B83),
        ("darcula-dark", 0xC2C300, 0xFFFF00),
    ];
    assert_eq!(BEFORE.len(), 14, "暗色套数变了，色表可能增删了套");
    for (id, y, by) in BEFORE {
        let p = pal(id);
        assert!(p.dark, "{id} 应是暗色档");
        assert_eq!(
            p.ansi16[SLOT_YELLOW], *y,
            "{id} 的 dark yellow 被改动了 —— #ansi-yellow-fix 只许动亮色档"
        );
        assert_eq!(
            p.ansi16[SLOT_BRIGHT_YELLOW], *by,
            "{id} 的 dark brightYellow 被改动了 —— #ansi-yellow-fix 只许动亮色档"
        );
    }
}

/// 三套被用户点名过的（黄色最不可读）锁死具体值 —— 观感回归。
#[test]
fn the_three_worst_light_yellows_are_pinned() {
    for (id, y, by) in [
        ("catppuccin-light", 0x94621Fu32, 0x895A1Cu32),
        ("rosepine-light", 0x966425, 0x8B5D22),
        ("ayu-light", 0x9C6827, 0x8E6228),
    ] {
        let p = pal(id);
        assert_eq!(p.ansi16[SLOT_YELLOW], y, "{id} yellow 色值变了");
        assert_eq!(
            p.ansi16[SLOT_BRIGHT_YELLOW], by,
            "{id} brightYellow 色值变了"
        );
    }
}

/// (#ansi-exempt-slots 2026-10-16) 亮色主题的**白 / 灰 / 亮白**三槽也必须
/// ≥ 4.5:1。这三槽原本在 `_meta.ansi_check` 里被豁免「只记录不判失败」——
/// 那是暗底惯例；落到浅底上 `ls -l` 的大小列（槽7）与日期列（槽8）直接塌到
/// 1.0~2.9:1（用户截图：Kanagawa·亮 槽7 = 2.93 / 槽8 = 2.23，整片列表读不出）。
/// 槽15 是加粗白字，最低到 1.14，同样不可读。
#[test]
fn light_themes_keep_white_grey_slots_readable() {
    let mut checked = 0usize;
    for p in PALETTES.iter().filter(|p: &&Palette| !p.dark) {
        for (slot, name) in [
            (SLOT_WHITE, "white"),
            (SLOT_BRIGHT_BLACK, "brightBlack"),
            (SLOT_BRIGHT_WHITE, "brightWhite"),
        ] {
            let c = contrast(p.ansi16[slot], p.tbg);
            assert!(
                c >= MIN,
                "{}：ANSI {name} 槽{slot} {} 对 tbg {} 只有 {c:.2}:1（< {MIN}）—— \
                 辅助信息在浅底终端里读不出来",
                p.id,
                hex(p.ansi16[slot]),
                hex(p.tbg)
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 14, "亮色主题数量变了（现 {checked}），色表可能增删了套");
}

/// 白 / 灰 / 亮白三槽在亮底上必须是**中性灰**，不能带色相。
///
/// 这是生成脚本踩过的坑：HSL 里近白色的 S 会爆表（#FFFCF0 算出 S=100%、
/// H=48°），于是「保色相降 L」一路把它带成 #8A6E00（暗黄）—— 白色档变
/// 黄色档。所以脚本改用**通道极差** max(r,g,b)−min(r,g,b) 判定并锁饱和。
/// 本测试从结果侧兜住：改色时若又用 HSL 保色相，这里立刻炸。
#[test]
fn light_white_slots_stay_neutral_grey() {
    /// 允许的最大通道极差（脚本的「弱色」档上界 80 之上算彩色档，但彩色档
    /// 刻意调过时也会低于此；120 能兜住「明显带色相」的回归而不误伤）。
    const MAX_SPREAD: u8 = 120;
    for p in PALETTES.iter().filter(|p: &&Palette| !p.dark) {
        for slot in [SLOT_WHITE, SLOT_BRIGHT_BLACK, SLOT_BRIGHT_WHITE] {
            let (r, g, b) = rgb_of(p.ansi16[slot]);
            let spread = r.max(g).max(b) - r.min(g).min(b);
            assert!(
                spread <= MAX_SPREAD,
                "{}：ANSI 槽{slot} 压深后带上了色相 {}（通道极差 {spread} > {MAX_SPREAD}）—— \
                 近中性色应锁 sat=0，脚本若退回 HSL 保色相就会把白档染成黄/蓝",
                p.id,
                hex(p.ansi16[slot])
            );
        }
    }
}

/// 这次修正同样只许动亮色档 —— 暗色档的白 / 灰 / 亮白在深底上本来就够读，
/// 压深反而会把加粗白字弄暗。快照取自 #ansi-exempt-slots 之前的 palettes.rs
/// （git HEAD 时的暗色值；本次改动前后暗色 `ansi16` 应逐值一致）。
#[test]
fn dark_themes_grey_slots_were_left_alone() {
    const BEFORE: &[(&str, u32, u32, u32)] = &[
        //  (id,                槽7,      槽8,      槽15)
        ("mterm-dark", 0xBCBEC4, 0x6F737A, 0xD4D5D9),
        ("onedark-dark", 0xABB2BF, 0x5C6370, 0xFFFFFF),
        ("graphite-dark", 0xEEEEEE, 0x484848, 0xEEEEEE),
        ("nord-dark", 0xE5E9F0, 0x4C566A, 0xECEFF4),
        ("catppuccin-dark", 0xBAC2DE, 0x585B70, 0xA6ADC8),
        ("dracula-dark", 0xF8F8F2, 0x6272A4, 0xFFFFFF),
        ("rosepine-dark", 0xE0DEF4, 0x6E6A86, 0xE0DEF4),
        ("everforest-dark", 0xD3C6AA, 0x646E73, 0xDACFB8),
        ("kanagawa-wave", 0xC8C093, 0x727169, 0xDCD7BA),
        ("gruvbox-material-dark", 0xD4BE98, 0x5B5856, 0xDBC8A8),
        ("ayu-mirage", 0xFAFAFA, 0x686868, 0xFFFFFF),
        ("flexoki-dark", 0xFFFCF0, 0x100F0F, 0xFFFCF0),
        ("solarized-dark", 0xEEE8D5, 0x002B36, 0xFDF6E3),
        ("darcula-dark", 0xADADAD, 0x555555, 0xEEEEEE),
    ];
    assert_eq!(BEFORE.len(), 14, "暗色套数变了，色表可能增删了套");
    for (id, w, g, bw) in BEFORE {
        let p = pal(id);
        assert!(p.dark, "{id} 应是暗色档");
        for (slot, expect, name) in [
            (SLOT_WHITE, *w, "white"),
            (SLOT_BRIGHT_BLACK, *g, "brightBlack"),
            (SLOT_BRIGHT_WHITE, *bw, "brightWhite"),
        ] {
            assert_eq!(
                p.ansi16[slot], expect,
                "{id} 的暗色 {name} 被改动了 —— #ansi-exempt-slots 只许动亮色档"
            );
        }
    }
}