
use crate::terminal::{CompiledOutputRule, HistSpan, RuleColor};
use crate::ui::TermSpan;

/// 客户端侧输出高亮（#hl-settings-hifi 2026-10-20 重写）。
///
/// 规则表已按优先级排好（自定义在前、预设按表序），逐条**认领**：
/// 每条规则只染还没被认领的、程序没上过色的片段；先命中先着色，后来的规则
/// 只能染剩余部分。`whole_line` 规则（自定义整行 / 表格分隔线）是「任一
/// run 命中即染整行」。`rainbow` 开时最后走彩虹括号通道（按括号层级循环
/// 着色，只染剩下的括号字符）。
///
/// (#hl-rule-guard 2026-10-21) `pal` 参与守卫：规则的绝对 RGB（自定义规则 /
/// 彩虹括号）在落到主题终端底之前过 `theme::fit_rule_fg`（保色相、3:1）——
/// 只作用于**本地规则色**，远端程序发的真彩/256 色照旧直通（那是尊重远端
/// 本意的惯例，不适用本地用户选色）。ANSI 槽位色（Idx）是主题自己的表，
/// 已由 `_meta.ansi_check` 守过，不重复处理。
pub(crate) fn highlight_plain_output(
    runs: Vec<HistSpan>,
    rules: &[CompiledOutputRule],
    rainbow: bool,
    pal: &crate::theme::palettes::Palette,
) -> Vec<HistSpan> {
    if rules.is_empty() && !rainbow {
        return runs;
    }
    let mut runs = runs;
    for rule in rules {
        apply_rule(&mut runs, rule, pal);
    }
    if rainbow {
        apply_rainbow_parens(&mut runs, pal);
    }
    runs
}

/// 程序自己上过色的 run 永远不碰（fg/bg 非默认、加粗、反色都算）。
fn is_plain_run(run: &HistSpan) -> bool {
    matches!(run.fg, vt100::Color::Default)
        && matches!(run.bg, vt100::Color::Default)
        && !run.bold
        && !run.inverse
}

/// 彩虹括号自己的「可染」判定：只看颜色 / 反色，**忽略 bold**。
///
/// 为什么单开一套：大量 shell 脚本用 `\e[1m` / `\e[1;37m` 做强调（`[INFO]`
/// 这类前缀），那并不代表它想给括号指定配色；若沿用 `is_plain_run` 的严格
/// 口径（bold 也算「已上色」），`[INFO]`、`[pool-8-thread-1]` 这些最常见的
/// 行永远吃不到彩虹（用户截图反馈）。仍然不碰 fg/bg 被显式设置或反色的
/// run —— 那才是程序在用颜色表达语法（vim / ls --color / 带 ANSI 的日志）。
fn is_rainbow_ok(run: &HistSpan) -> bool {
    matches!(run.fg, vt100::Color::Default)
        && matches!(run.bg, vt100::Color::Default)
        && !run.inverse
}

fn style_run(run: &mut HistSpan, color: RuleColor, pal: &crate::theme::palettes::Palette, bold: bool) {
    run.fg = match color {
        RuleColor::Idx(i) => vt100::Color::Idx(i),
        RuleColor::Rgb(r, g, b) => {
            // (#hl-rule-guard) 本地规则色过守卫：暗底提亮、亮底压暗，保色相。
            let fitted = crate::theme::fit_rule_fg(
                ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
                pal.tbg,
                3.0,
            );
            vt100::Color::Rgb(
                ((fitted >> 16) & 0xFF) as u8,
                ((fitted >> 8) & 0xFF) as u8,
                (fitted & 0xFF) as u8,
            )
        }
    };
    // (#hl-bold-tier) 日志级加粗（渲染端 bold+基色 → bright 档）；标识类细体
    // 走 normal 槽 —— 同色相靠粗细+明暗双维度区分语义。
    run.bold = bold;
}

/// 一条规则的命中区间（字节）。`group > 0` 时只取该捕获组
/// （path 预设用前置组占位，只染路径本身）；否则整个匹配。
fn rule_match_spans(rule: &CompiledOutputRule, text: &str) -> Vec<(usize, usize)> {
    if rule.group > 0 {
        rule.matcher
            .captures_iter(text)
            .filter_map(|caps| caps.get(rule.group).map(|m| (m.start(), m.end())))
            .filter(|(start, end)| start < end)
            .collect()
    } else {
        rule.matcher
            .find_iter(text)
            .filter(|m| !m.is_empty())
            .map(|m| (m.start(), m.end()))
            .collect()
    }
}

fn apply_rule(runs: &mut Vec<HistSpan>, rule: &CompiledOutputRule, pal: &crate::theme::palettes::Palette) {
    if rule.whole_line {
        let hit = runs
            .iter()
            .any(|run| is_plain_run(run) && rule.matcher.is_match(&run.text));
        if hit {
            for run in runs.iter_mut() {
                if is_plain_run(run) {
                    style_run(run, rule.color, pal, rule.bold);
                }
            }
        }
        return;
    }
    let taken = std::mem::take(runs);
    let mut next = Vec::with_capacity(taken.len() + 2);
    for run in taken {
        if !is_plain_run(&run) {
            next.push(run);
            continue;
        }
        let spans: Vec<(usize, usize, RuleColor)> = rule_match_spans(rule, &run.text)
            .into_iter()
            .map(|(start, end)| (start, end, rule.color))
            .collect();
        if spans.is_empty() {
            next.push(run);
        } else {
            next.extend(split_run(run, &spans, pal, rule.bold));
        }
    }
    *runs = next;
}

/// 把 run 按给定字节区间切开：区间内按各自颜色着色（`emph` = 是否加粗），
/// 区间外保持原样。列号/格数按显示宽度重算（宽字符下字节数 ≠ 字符数 ≠ 格数）。
fn split_run(
    run: HistSpan,
    spans: &[(usize, usize, RuleColor)],
    pal: &crate::theme::palettes::Palette,
    emph: bool,
) -> Vec<HistSpan> {
    let mut out = Vec::with_capacity(spans.len() * 2 + 1);
    let mut byte_pos = 0usize;
    let mut col = run.col;
    for &(start, end, color) in spans {
        if start < byte_pos || end > run.text.len() || start >= end {
            continue;
        }
        if start > byte_pos {
            let text = &run.text[byte_pos..start];
            let cells = text_cell_width(text);
            let mut part = run.clone();
            part.text = text.to_string();
            part.col = col;
            part.cells = cells;
            out.push(part);
            col += cells;
        }

        let text = &run.text[start..end];
        let cells = text_cell_width(text);
        let mut hit = run.clone();
        hit.text = text.to_string();
        style_run(&mut hit, color, pal, emph);
        hit.col = col;
        hit.cells = cells;
        out.push(hit);
        col += cells;
        byte_pos = end;
    }
    if byte_pos < run.text.len() {
        let mut part = run;
        part.text = part.text[byte_pos..].to_string();
        part.col = col;
        // Recompute instead of relying on subtraction: wide/combining glyphs
        // can make byte/character counts differ from terminal grid cells.
        part.cells = text_cell_width(&part.text);
        out.push(part);
    }
    out
}

/// 彩虹括号七色（绝对色值，与高保真稿一致）。刻意**不跟随主题**：
/// 彩虹的意义就是七色循环；跟随主题会把七档压成主题的几槽。
const RAINBOW_COLORS: [(u8, u8, u8); 7] = [
    (0xF7, 0x54, 0x64), // 红
    (0xF0, 0x88, 0x3E), // 橙
    (0xF5, 0xE1, 0x47), // 黄
    (0x8C, 0xCF, 0x15), // 青柠
    (0x1F, 0xC8, 0xB4), // 青
    (0x3D, 0xA5, 0xD9), // 蓝
    (0x7C, 0x6B, 0xD6), // 紫
];

const RAINBOW_OPENERS: [char; 3] = ['(', '[', '{'];
const RAINBOW_CLOSERS: [char; 3] = [')', ']', '}'];

/// 彩虹括号：对**还没被认领**的括号字符按嵌套层级循环着色（只染括号本身，
/// 不染括号里的内容）。深度栈按行跨 run 延续 —— 同一行的开括号可能在
/// 前一个 run、闭括号在后一个 run。
fn apply_rainbow_parens(runs: &mut Vec<HistSpan>, pal: &crate::theme::palettes::Palette) {
    // 栈里存 opener 的索引（0..3），只用来判空/弹栈；颜色按栈深取。
    let mut stack: Vec<usize> = Vec::new();
    let taken = std::mem::take(runs);
    let mut next = Vec::with_capacity(taken.len() + 8);
    for run in taken {
        if !is_rainbow_ok(&run)
            || !run
                .text
                .chars()
                .any(|c| RAINBOW_OPENERS.contains(&c) || RAINBOW_CLOSERS.contains(&c))
        {
            next.push(run);
            continue;
        }
        let mut spans: Vec<(usize, usize, RuleColor)> = Vec::new();
        for (idx, ch) in run.text.char_indices() {
            if let Some(opener) = RAINBOW_OPENERS.iter().position(|o| *o == ch) {
                let level = stack.len();
                let (r, g, b) = RAINBOW_COLORS[level % RAINBOW_COLORS.len()];
                spans.push((idx, idx + ch.len_utf8(), RuleColor::Rgb(r, g, b)));
                stack.push(opener);
            } else if RAINBOW_CLOSERS.contains(&ch) {
                if stack.pop().is_none() {
                    // 孤立的闭括号：按最外层色染，不炸栈
                    let (r, g, b) = RAINBOW_COLORS[0];
                    spans.push((idx, idx + ch.len_utf8(), RuleColor::Rgb(r, g, b)));
                    continue;
                }
                let level = stack.len();
                let (r, g, b) = RAINBOW_COLORS[level % RAINBOW_COLORS.len()];
                spans.push((idx, idx + ch.len_utf8(), RuleColor::Rgb(r, g, b)));
            }
        }
        next.extend(split_run(run, &spans, pal, true));
    }
    *runs = next;
}

pub(crate) fn text_cell_width(text: &str) -> i32 {
    use unicode_width::UnicodeWidthChar;
    text.chars().map(|ch| ch.width().unwrap_or(0) as i32).sum()
}

/// Split a styled terminal run into UI spans for the terminal grid.
/// (#twemoji-removal 2026-09-05) 彩色 emoji 渲染(Twemoji 图像 span)已移除:
/// 集成测试与体积评估认定 34MB 的全集嵌入不值(占 exe 体积约九成),emoji
/// 交由 Slint 的字体回退渲染单色轮廓。终端列数仍来自网格(span.cells),
/// 这里只做配色解析与 CJK 标记。
/// (#nerd-icon-split 2026-10-21) 私用区图标字符从**混合 run** 中逐字符拆出
/// 独立 span：vt100 的 run 按 (fg,bg,bold,inverse) 合并同色格子，而 eza/lsd
/// 把图标与文件名染成同色，一段 run 就是 `󰉋  name`。旧的「整段纯图标才
/// 放大」判据在这类输出下永不命中，图标只能按名义字号画——
/// JetBrainsMono NFM 的 folder 图标墨高仅 0.49~0.53em，比小写 x-height
/// (0.55em) 还矮，这就是「图标看上去小了些」的来源。现在图标字符单独成
/// span 交给 UI 放大居中；Powerline 原始分隔符(U+E0A0~U+E0D4)设计为恰好
/// 填满格子、首尾相接拼提示符，不拆不放。列号/格数按显示宽度重算，网格、
/// PTY 寻址与点选坐标都不变。
pub(crate) fn render_term_span(
    span: &HistSpan,
    row: i32,
    is_dark: bool,
    pal: &crate::theme::palettes::Palette,
) -> Vec<TermSpan> {
    if span.cells <= 0 || span.text.is_empty() {
        return Vec::new();
    }
    let (fg, bg) = vt_span_colors(span.fg, span.bg, span.bold, span.inverse, is_dark, pal);
    // (#nerd-icon-diag) 诊断：用户报「图标放大没生效」，需要确认拆分是否命中。
    // 设 MTERM_DIAG_NERD=1 后跑 `ls --color`，日志会打出每个含私用区字符的
    // span 的码位。**不加 cfg(debug_assertions)** —— 出问题的正是 release
    // 包，被编译掉就白加了。只在命中时打，且要求环境变量存在，不刷屏。
    if std::env::var_os("MTERM_DIAG_NERD").is_some()
        && span.text.chars().any(is_nerd_icon_char)
    {
        let cp: Vec<String> = span
            .text
            .chars()
            .map(|c| format!("U+{:04X}", c as u32))
            .collect();
        tracing::info!(
            row,
            text = %span.text,
            codepoints = ?cp,
            "nerd-diag: span 含私用区字符，将按字符拆分"
        );
    }
    // 没有图标字符就保持一 run 一 span（绝大多数行），不付拆分开销。
    if !span.text.chars().any(is_scaled_icon_char) {
        let cjk = contains_cjk(&span.text);
        return vec![TermSpan {
            text: span.text.clone().into(),
            fg,
            bg,
            bold: span.bold,
            row,
            col: span.col,
            cells: span.cells,
            cjk,
            nerd: false,
        }];
    }
    let mut out: Vec<TermSpan> = Vec::new();
    let mut col = span.col;
    let mut plain = String::new();
    for ch in span.text.chars() {
        if is_scaled_icon_char(ch) {
            push_plain_span(&mut out, &plain, fg, bg, span.bold, row, &mut col);
            plain.clear();
            out.push(TermSpan {
                text: ch.to_string().into(),
                fg,
                bg,
                bold: span.bold,
                row,
                col,
                cells: 1,
                cjk: false,
                nerd: true,
            });
            col += 1;
        } else {
            plain.push(ch);
        }
    }
    push_plain_span(&mut out, &plain, fg, bg, span.bold, row, &mut col);
    out
}

/// 把图标字符之间的普通片段压成一个 span，按显示宽度推进列号。
fn push_plain_span(
    out: &mut Vec<TermSpan>,
    text: &str,
    fg: slint::Color,
    bg: slint::Color,
    bold: bool,
    row: i32,
    col: &mut i32,
) {
    if text.is_empty() {
        return;
    }
    let cells = text_cell_width(text);
    if cells <= 0 {
        return;
    }
    out.push(TermSpan {
        text: text.into(),
        fg,
        bg,
        bold,
        row,
        col: *col,
        cells,
        cjk: contains_cjk(text),
        nerd: false,
    });
    *col += cells;
}

/// True if a terminal span contains any CJK character — ideograph, kana, or
/// (crucially) CJK punctuation like 、。，. The mono terminal font has no CJK
/// glyphs and Slint's per-script fallback tofu's *isolated* CJK punctuation
/// (it renders fine only when adjacent to a Han char), so these spans are drawn
/// with the CJK-capable UI font instead (#54). Box-drawing / powerline glyphs
/// are deliberately excluded so they keep the aligned monospace font.
fn contains_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c as u32,
            0x2E80..=0x2EFF       // CJK radicals
            | 0x3000..=0x303F     // CJK symbols & punctuation (、。「」…)
            | 0x3040..=0x30FF     // hiragana + katakana
            | 0x3100..=0x312F     // bopomofo
            | 0x3400..=0x4DBF     // CJK ext A
            | 0x4E00..=0x9FFF     // CJK unified ideographs
            | 0xF900..=0xFAFF     // CJK compatibility ideographs
            | 0xFF00..=0xFFEF     // fullwidth / halfwidth forms (，！？：；)
            | 0x20000..=0x2FA1F) // CJK ext B–F + compat supplement
    })
}

/// True if this character should be drawn as a scaled-up Nerd Font icon in its
/// own cell: a private-use-area glyph **outside the original Powerline block**.
///
/// (#nerd-icon-split / #nerd-icon-scale) Nerd Font draws its icons on the full
/// em box, but the terminal grid locks every character to one measured cell
/// (advance ≈ 0.6em). At the nominal font size the common `ls` icons (folder
/// 0.49~0.53em ink) come out *shorter than lowercase letters*, which reads as
/// "icons look too small". Scaling the glyph up inside its own cell fixes the
/// optics without touching SSH/PTY addressing, vim/htop cursor addressing or
/// click-to-select, which all index by (row, col).
///
/// Powerline's own separators (U+E0A0~U+E0D4, `` /  /  / …) are
/// deliberately excluded: they are designed to fill the cell exactly so
/// prompt segments butt together seamlessly — scaling them would tear the
/// seams. Two ranges remain recognised for scaling:
///   * U+E000..U+F8FF  — Nerd Font v2/v3 icon blocks (minus Powerline above);
///   * U+F0000..U+FFFFD — supplementary plane, for icon sets shipped out there.
fn is_scaled_icon_char(c: char) -> bool {
    is_nerd_icon_char(c) && !matches!(c as u32, 0xE0A0..=0xE0D4)
}

fn is_nerd_icon_char(c: char) -> bool {
    matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD)
}

/// 16-colour ANSI palette for **dark** terminals (VS Code "Dark+" values).
const ANSI16_DARK: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), // 0  black
    (0xcd, 0x31, 0x31), // 1  red
    (0x0d, 0xbc, 0x79), // 2  green
    (0xe5, 0xe5, 0x10), // 3  yellow
    (0x24, 0x72, 0xc8), // 4  blue
    (0xbc, 0x3f, 0xbc), // 5  magenta
    (0x11, 0xa8, 0xcd), // 6  cyan
    (0xe5, 0xe5, 0xe5), // 7  white        (light grey on dark bg)
    (0x66, 0x66, 0x66), // 8  bright black
    (0xf1, 0x4c, 0x4c), // 9  bright red
    (0x23, 0xd1, 0x8b), // 10 bright green
    (0xf5, 0xf5, 0x43), // 11 bright yellow
    (0x3b, 0x8e, 0xea), // 12 bright blue
    (0xd6, 0x70, 0xd6), // 13 bright magenta
    (0x29, 0xb8, 0xdb), // 14 bright cyan
    (0xff, 0xff, 0xff), // 15 bright white
];

/// 16-colour ANSI palette for **light** terminal **foreground** (text) use.
///
/// On a near-white (#fafafa) background, the standard "white" (slot 7) and
/// "bright white" (slot 15) are nearly invisible.  We remap them to dark greys
/// so `ls`, `git` and other tools that use colour 7 for regular text stay
/// perfectly readable.  Saturated hues are darkened for contrast.
const ANSI16_LIGHT: [(u8, u8, u8); 16] = [
    (0x1c, 0x1c, 0x1e), // 0  black        → Apple near-black
    (0xc0, 0x39, 0x2b), // 1  red
    (0x1a, 0x7f, 0x37), // 2  green        → darker for white bg
    (0x85, 0x64, 0x04), // 3  yellow       → dark amber, readable
    (0x04, 0x51, 0xa5), // 4  blue         → VS Code light blue
    (0x80, 0x00, 0x80), // 5  magenta
    (0x0e, 0x72, 0x5c), // 6  cyan         → darker teal
    (0x3a, 0x3a, 0x3c), // 7  white        → dark grey (was 0xe5e5e5, near-invisible)
    (0x55, 0x55, 0x55), // 8  bright black
    (0xe7, 0x4c, 0x3c), // 9  bright red
    (0x27, 0xae, 0x60), // 10 bright green
    (0xd4, 0xac, 0x0d), // 11 bright yellow
    (0x2e, 0x86, 0xc1), // 12 bright blue
    (0x9b, 0x59, 0xb6), // 13 bright magenta
    (0x1a, 0xbc, 0x9c), // 14 bright cyan
    (0x2c, 0x2c, 0x2e), // 15 bright white → dark (was 0xffffff, near-invisible)
];

/// 16-colour ANSI palette for **light** terminal **background** (fill) use.
///
/// When TUI programs (btop, htop, vim) paint cell backgrounds in light mode,
/// each colour maps to a light-tinted variant so the overall UI feels light.
/// "Black" (slot 0) becomes a very light grey rather than near-black, so
/// dark-background TUI apps naturally inherit a light appearance.  Foreground
/// text always uses `ANSI16_LIGHT` so readability is unaffected.
const ANSI16_LIGHT_BG: [(u8, u8, u8); 16] = [
    (0xe8, 0xe8, 0xed), // 0  black        → Apple system-grey-6 (very light)
    (0xff, 0xd5, 0xd5), // 1  red          → light rose
    (0xd5, 0xf5, 0xd5), // 2  green        → light mint
    (0xff, 0xf8, 0xd5), // 3  yellow       → light cream
    (0xd5, 0xe8, 0xf8), // 4  blue         → light sky
    (0xf5, 0xd5, 0xf5), // 5  magenta      → light lilac
    (0xd5, 0xf5, 0xf8), // 6  cyan         → light aqua
    (0xf5, 0xf5, 0xf7), // 7  white        → Apple bg (near-white)
    (0xd1, 0xd1, 0xd6), // 8  bright black → Apple system-grey-4
    (0xff, 0xbe, 0xbe), // 9  bright red   → light salmon
    (0xbe, 0xf5, 0xbe), // 10 bright green
    (0xf5, 0xf5, 0xbe), // 11 bright yellow
    (0xbe, 0xdd, 0xff), // 12 bright blue  → light periwinkle
    (0xf0, 0xbe, 0xff), // 13 bright magenta → light violet
    (0xbe, 0xf5, 0xff), // 14 bright cyan
    (0xff, 0xff, 0xff), // 15 bright white → white
];

/// Convert a vt100 foreground colour (+ bold) to a Slint colour.
/// Bold + a base colour (0–7) maps to the bright variant (8–15), matching
/// how terminals render `ls --color` (bold-green executables, bold-blue dirs).
///
/// In light mode, true-colour RGB foregrounds that are light (HSL lightness
/// ≥ 0.55) are darkened so they remain readable on a near-white background.
fn vt_color_to_slint(
    color: vt100::Color,
    bold: bool,
    is_dark: bool,
    pal: &crate::theme::palettes::Palette,
) -> slint::Color {
    let (r, g, b) = match color {
        vt100::Color::Default => fg_rgb_of(pal),
        vt100::Color::Idx(i) => idx_to_rgb(i, bold, is_dark, pal),
        vt100::Color::Rgb(r, g, b) => {
            if is_dark {
                (r, g, b)
            } else {
                // 守卫用**当前主题的 tbg**（per-theme），不是写死的近白
                let tbg = ((pal.tbg >> 16) as u8, (pal.tbg >> 8) as u8, pal.tbg as u8);
                darken_light_fg(r, g, b, tbg)
            }
        }
    };
    slint::Color::from_rgb_u8(r, g, b)
}

/// 主题默认前景/背景（#theme-split：按当前主题查表；meat 档即原
/// #d4d4d4/#0e0f13，各主题用表内 tfg/tbg）。
fn fg_rgb_of(pal: &crate::theme::palettes::Palette) -> (u8, u8, u8) {
    ((pal.tfg >> 16) as u8, (pal.tfg >> 8) as u8, pal.tfg as u8)
}

fn bg_rgb_of(pal: &crate::theme::palettes::Palette) -> (u8, u8, u8) {
    ((pal.tbg >> 16) as u8, (pal.tbg >> 8) as u8, pal.tbg as u8)
}

pub(crate) fn vt_span_colors(
    fg: vt100::Color,
    bg: vt100::Color,
    bold: bool,
    inverse: bool,
    is_dark: bool,
    pal: &crate::theme::palettes::Palette,
) -> (slint::Color, slint::Color) {
    if !inverse {
        return (
            vt_color_to_slint(fg, bold, is_dark, pal),
            vt_bg_to_slint(bg, is_dark, pal),
        );
    }

    let fg_color = match bg {
        vt100::Color::Default => {
            let (r, g, b) = bg_rgb_of(pal);
            slint::Color::from_rgb_u8(r, g, b)
        }
        _ => vt_color_to_slint(bg, false, is_dark, pal),
    };
    let bg_color = match fg {
        vt100::Color::Default => {
            let (r, g, b) = fg_rgb_of(pal);
            slint::Color::from_rgb_u8(r, g, b)
        }
        _ => vt_bg_to_slint(fg, is_dark, pal),
    };
    (fg_color, bg_color)
}

/// sRGB 相对亮度（WCAG 2.x），`/ 255` 的通道线性化。
fn channel_luminance(c: u8) -> f64 {
    let v = c as f64 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 对比度（1.0 ~ 21.0）。
fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let la = 0.2126 * channel_luminance(a.0)
        + 0.7152 * channel_luminance(a.1)
        + 0.0722 * channel_luminance(a.2);
    let lb = 0.2126 * channel_luminance(b.0)
        + 0.7152 * channel_luminance(b.1)
        + 0.0722 * channel_luminance(b.2);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// 亮底守卫的目标对比度。取 4.5:1（WCAG AA 正文线），与主题色表的
/// `_meta.ansi_check` 验收线一致 —— 终端正文也是正文。
const LIGHT_FG_MIN_CONTRAST: f64 = 4.5;

/// In light mode, remap true-colour foregrounds so they stay readable on the
/// palette's own `tbg` (not an assumed near-white).
///
/// (#term-truecolor-guard 2026-10-16) **This used to branch on HSL lightness
/// alone** — `L < 0.55` passed through untouched. That is wrong for saturated
/// hues: HSL lightness is *channel-average based*, so bright cyans/greens/yellows
/// measure low and slipped straight through. Measured over the full 256-colour
/// cube on `mterm-light`'s `#FAF8F4`: **98 of 215 colours rendered below
/// 4.5:1**, 70 of them untouched by this function. Worst offenders were pure
/// cube colours — `#FFFF00` at **1.01:1** (indistinguishable from the
/// background) and `#00FF00` at 1.29. The user's report that started this was a
/// remote host's 256-colour cyan `#00AF87` (cube index 36) landing at 2.65:1.
///
/// Now: hold **hue**, then walk lightness down until the colour actually
/// clears 4.5:1 against the real background. Saturated colours still shift
/// (they must — a readable light-background rendering of a saturated hue is
/// necessarily darker), but hue is preserved and nothing passes through
/// unchecked. Already-dark colours are returned untouched.
///
/// `bg` is the palette's `tbg` (per-theme), so a darker light palette such as
/// `kanagawa-lotus`'s `#F2ECBC` is handled without a second constant.
fn darken_light_fg(r: u8, g: u8, b: u8, bg: (u8, u8, u8)) -> (u8, u8, u8) {
    let orig = (r, g, b);
    if contrast_ratio(orig, bg) >= LIGHT_FG_MIN_CONTRAST {
        return orig;
    }
    let (h, s, l0) = rgb_to_hsl(r, g, b);
    // 从原亮度**向下细步**到刚好过线，而不是一步降到某个固定 L。
    // ⚠️ 粗步长（例如直接从 0.6 往下扫）会把饱和色一路推到纯黑、丢掉色相 ——
    // 实测 #4DC4A8 / #ABE0D1 都被渲成 #000000。步长取 1/1024 保证在
    // 8 位输出分辨率下连续，停下时就是「刚好达标」的最亮的那一档。
    const STEPS: u32 = 1024;
    let mut best = (0u8, 0u8, 0u8);
    for i in 1..=STEPS {
        let new_l = l0 * (1.0 - i as f32 / STEPS as f32);
        let cand = hsl_to_rgb(h, s, new_l);
        if contrast_ratio(cand, bg) >= LIGHT_FG_MIN_CONTRAST {
            best = cand;
            break;
        }
        best = cand;
    }
    best
}

/// Convert a vt100 *background* colour to Slint.  The default background maps
/// to fully transparent so we don't paint a fill over the terminal's own bg.
/// Non-default backgrounds (btop/htop bars, selected rows) become opaque.
///
/// In light mode:
/// - ANSI 16 colours use `ANSI16_LIGHT_BG` (light pastels).
/// - True-colour RGB backgrounds that are dark (HSL lightness < 0.45) are
///   remapped to light pastels so programs like btop feel light-themed.
fn vt_bg_to_slint(
    color: vt100::Color,
    is_dark: bool,
    pal: &crate::theme::palettes::Palette,
) -> slint::Color {
    match color {
        vt100::Color::Default => slint::Color::from_argb_u8(0, 0, 0, 0), // transparent
        vt100::Color::Idx(i) => {
            let (r, g, b) = idx_to_rgb_bg(i, is_dark, pal);
            slint::Color::from_rgb_u8(r, g, b)
        }
        vt100::Color::Rgb(r, g, b) => {
            if is_dark {
                slint::Color::from_rgb_u8(r, g, b)
            } else {
                let (nr, ng, nb) = lighten_dark_bg(r, g, b);
                slint::Color::from_rgb_u8(nr, ng, nb)
            }
        }
    }
}

/// In light mode, remap dark true-colour backgrounds to light pastels.
/// Colours whose HSL lightness is already ≥ 0.45 pass through unchanged
/// (the program chose a light colour deliberately).
fn lighten_dark_bg(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (h, s, l) = rgb_to_hsl(r, g, b);
    if l >= 0.45 {
        return (r, g, b);
    }
    // Remap: darkest (l≈0) → very light (l≈0.92); l=0.45 → l≈0.84.
    // Reduce saturation to pastel so colours don't look garish on white.
    let new_l = 0.92 - l * 0.18;
    let new_s = (s * 0.35).min(0.25);
    hsl_to_rgb(h, new_s, new_l)
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s < 1e-6 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
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
    (
        (hue(h + 1.0 / 3.0) * 255.0).round() as u8,
        (hue(h) * 255.0).round() as u8,
        (hue(h - 1.0 / 3.0) * 255.0).round() as u8,
    )
}

/// Map an xterm-256 palette index to RGB (16 ANSI + 6×6×6 cube + grayscale).
/// (#theme-split) ANSI 16 色按当前主题查表（28 套；meat 档即原 ANSI16_DARK /
/// ANSI16_LIGHT 两常量值）。256 色立方 / 灰阶不随主题。
fn idx_to_rgb(
    i: u8,
    bold: bool,
    is_dark: bool,
    pal: &crate::theme::palettes::Palette,
) -> (u8, u8, u8) {
    let i = if bold && i < 8 { i + 8 } else { i };
    let palette = ansi16_of(pal);
    let out = match i {
        0..=15 => palette[i as usize],
        16..=231 => {
            let n = i - 16;
            let to = |v: u8| -> u8 {
                if v == 0 {
                    0
                } else {
                    55 + v * 40
                }
            };
            (to(n / 36), to((n % 36) / 6), to(n % 6))
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    };
    // (#term-truecolor-guard 2026-10-16) 16~255 档是 xterm **固定值**，不随主题
    // 变，浅底上必然有大批不可读（实测 mterm-light 的 #FAF8F4 上 215 个立方体
    // 色有 98 个低于 4.5:1，纯亮黄 #FFFF00 只有 1.01）。亮色模式下同样过
    // 守卫；0~15 档是主题自己的色表（已由 `_meta.ansi_check` 守过），不重复处理。
    if is_dark || i < 16 {
        out
    } else {
        let tbg = ((pal.tbg >> 16) as u8, (pal.tbg >> 8) as u8, pal.tbg as u8);
        darken_light_fg(out.0, out.1, out.2, tbg)
    }
}

/// 该套主题的 ANSI 16 色表（u32 → (u8,u8,u8)；每次 span 渲染构造一次，
/// 16 项拷贝可忽略）。
fn ansi16_of(pal: &crate::theme::palettes::Palette) -> [(u8, u8, u8); 16] {
    let mut out = [(0u8, 0u8, 0u8); 16];
    for (i, v) in pal.ansi16.iter().enumerate() {
        out[i] = ((*v >> 16) as u8, (*v >> 8) as u8, *v as u8);
    }
    out
}

/// Same as [`idx_to_rgb`] but for **background** fills in light mode: the 16
/// ANSI base colours use the palette's own light-TUI background table
/// （meat-light 的 ANSI16_LIGHT_BG 双表特性保留；其余亮档主题无此表时落回
/// 前景表）.  256-colour cube / grayscale are used as-is.
fn idx_to_rgb_bg(i: u8, is_dark: bool, pal: &crate::theme::palettes::Palette) -> (u8, u8, u8) {
    if !is_dark && i < 16 {
        if let Some(bg) = pal.ansi16bg {
            let v = bg[i as usize];
            return ((v >> 16) as u8, (v >> 8) as u8, v as u8);
        }
    }
    idx_to_rgb(i, false, is_dark, pal)
}

#[cfg(test)]
mod truecolor_guard_tests {
    use super::{contrast_ratio, darken_light_fg, idx_to_rgb, LIGHT_FG_MIN_CONTRAST};
    use crate::theme::palettes;

    fn tbg_of(pal: &palettes::Palette) -> (u8, u8, u8) {
        ((pal.tbg >> 16) as u8, (pal.tbg >> 8) as u8, pal.tbg as u8)
    }

    /// 旧实现按 HSL 亮度 `L < 0.55` 放过，漏掉 215 个立方体色里的 70 个
    /// （纯亮黄 #FFFF00 在 mterm-light 上只有 1.01:1）。这条锁死「256 色立方体
    /// 在亮色主题下全部过 4.5:1」，且**暗色模式逐值不变**。
    #[test]
    fn light_palettes_guard_every_cube_colour() {
        let mut worst = f64::MAX;
        let mut worst_at = 0u8;
        for pal in palettes::PALETTES.iter().filter(|p| !p.dark) {
            let bg = tbg_of(pal);
            for i in 16u8..=255 {
                let (r, g, b) = idx_to_rgb(i, false, false, pal);
                let c = contrast_ratio((r, g, b), bg);
                if c < worst {
                    worst = c;
                    worst_at = i;
                }
                assert!(
                    c >= LIGHT_FG_MIN_CONTRAST,
                    "{}: ANSI 索引 {i} 渲染为 #{r:02X}{g:02X}{b:02X}，对 tbg #{:06X} 只有 {c:.2}:1",
                    pal.id,
                    pal.tbg
                );
            }
        }
        // 守卫是逐色收敛的，实际最差值应贴近门槛（而非远低于它）。
        assert!(
            worst >= LIGHT_FG_MIN_CONTRAST,
            "最差色（索引 {worst_at}）只有 {worst:.2}:1"
        );
    }

    /// 24 位真彩同样过守卫 —— 用户报的远端青绿 `#00AF87`（HSL L≈0.34）恰好
/// 低于旧实现的 `L < 0.55` 门槛，被原样透传到浅底上，只有 2.65:1。
    #[test]
    fn light_palettes_guard_true_colour_foreground() {
        for pal in palettes::PALETTES.iter().filter(|p| !p.dark) {
            let bg = tbg_of(pal);
            // 用户实际截图里那批远端色 + 立方体两端极值
            for raw in [
                (0x00u8, 0xAFu8, 0x87u8), // 远端 ls 的青绿（立方体索引 36）
                (0xFFu8, 0xFFu8, 0x00u8), // 纯黄（旧实现 1.01:1）
                (0x00u8, 0xFFu8, 0x00u8), // 纯绿
                (0xFFu8, 0xFFu8, 0xFFu8), // 纯白
                (0x00u8, 0xFFu8, 0xFFu8), // 纯青
            ] {
                let (r, g, b) = darken_light_fg(raw.0, raw.1, raw.2, bg);
                let c = contrast_ratio((r, g, b), bg);
                assert!(
                    c >= LIGHT_FG_MIN_CONTRAST,
                    "{}: 真彩 {raw:?} 守卫后 #{r:02X}{g:02X}{b:02X} 对 tbg #{:06X} 只有 {c:.2}:1",
                    pal.id,
                    pal.tbg
                );
            }
        }
    }

    /// 暗色模式**逐值不变** —— 深底上原来的颜色本来就够读。
    #[test]
    fn dark_palettes_keep_cube_colours_verbatim() {
        let cube = |i: u8| -> (u8, u8, u8) {
            let n = i - 16;
            let to = |v: u8| -> u8 {
                if v == 0 {
                    0
                } else {
                    55 + v * 40
                }
            };
            (to(n / 36), to((n % 36) / 6), to(n % 6))
        };
        let grey = |i: u8| -> (u8, u8, u8) {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        };
        for pal in palettes::PALETTES.iter().filter(|p| p.dark) {
            // 立方体 16..=231
            for i in 16u8..=231 {
                assert_eq!(
                    idx_to_rgb(i, false, true, pal),
                    cube(i),
                    "{}: 暗色下 ANSI 索引 {i} 不该被守卫改动",
                    pal.id
                );
            }
            // 灰阶 232..=255
            for i in 232u8..=255 {
                assert_eq!(
                    idx_to_rgb(i, false, true, pal),
                    grey(i),
                    "{}: 暗色下 ANSI 灰阶 {i} 不该被守卫改动",
                    pal.id
                );
            }
        }
    }

    /// 已经足够深的色原样返回（守卫不该无谓改色 —— 那会丢掉远端本意）。
    #[test]
    fn already_readable_colours_pass_through() {
        let bg = (0xFA, 0xF8, 0xF4); // mterm-light tbg
        for c in [(0x33, 0x30, 0x2Cu8), (0x1F, 0x1C, 0x19), (0x00, 0x00, 0x00)] {
            assert_eq!(
                darken_light_fg(c.0, c.1, c.2, bg),
                c,
                "{c:?} 已达标，不该被改动"
            );
        }
    }

    /// 守卫只许改亮度，**不许改色相** —— 否则「让文字可读」变成了「把所有
    /// 彩色字变黑白」。首版用 512 步粗扫时，饱和色（`#4DC4A8`、`#ABE0D1`）
    /// 直接被推到纯黑；现在细步下降，停下时必须仍是同色相的色。
    #[test]
    fn guard_preserves_hue_of_saturated_colours() {
        let bg = (0xFA, 0xF8, 0xF4);
        for raw in [
            (0x4Du8, 0xC4u8, 0xA8u8), // 亮青绿：曾被渲成纯黑
            (0xABu8, 0xE0u8, 0xD1u8), // 亮薄荷：曾被渲成纯黑
            (0x00u8, 0xAFu8, 0x87u8), // 用户截图里的远端青绿
            (0xFFu8, 0xFFu8, 0x00u8), // 纯黄：曾只有 1.01:1
            (0x00u8, 0xFFu8, 0x00u8), // 纯绿：曾只有 1.29:1
        ] {
            let out = darken_light_fg(raw.0, raw.1, raw.2, bg);
            let spread = |c: (u8, u8, u8)| {
                c.0.max(c.1).max(c.2) - c.0.min(c.1).min(c.2)
            };
            assert!(
                spread(out) > 8,
                "{raw:?} 经守卫后成了 {out:?}（通道极差 {}）—— 色相被抹掉了，\
                 只该调亮度",
                spread(out)
            );
            assert!(
                contrast_ratio(out, bg) >= LIGHT_FG_MIN_CONTRAST,
                "{raw:?} → {out:?} 守卫后仍不达标"
            );
        }
    }
}

#[cfg(test)]
mod nerd_icon_tests {
    use super::{is_scaled_icon_char, render_term_span, HistSpan};

    fn hist_span(text: &str, col: i32) -> HistSpan {
        HistSpan {
            text: text.to_string(),
            fg: vt100::Color::Default,
            bg: vt100::Color::Default,
            bold: false,
            inverse: false,
            col,
            cells: super::text_cell_width(text),
        }
    }

    fn pal() -> &'static crate::theme::palettes::Palette {
        crate::theme::palettes::find("mterm-dark").unwrap()
    }

    /// `ls --color`（eza/lsd）的图标都是私用区（PUA）码位，且与文件名同色
    /// 合并在一个 run 里 —— 拆分后每个图标字符必须独立成 nerd span。
    #[test]
    fn splits_icon_out_of_mixed_run() {
        // eza 目录行：图标 + 两个空格 + 目录名，同色一个 run。
        let spans = render_term_span(&hist_span("\u{e5fc}  sz-smart", 0), 3, true, pal());
        assert_eq!(spans.len(), 2, "图标 + 名字两段: {:?}", spans);
        assert!(spans[0].nerd);
        assert_eq!(spans[0].text.as_str(), "\u{e5fc}");
        assert_eq!(spans[0].cells, 1);
        assert_eq!(spans[0].col, 0);
        assert!(!spans[1].nerd);
        assert_eq!(spans[1].text.as_str(), "  sz-smart");
        assert_eq!(spans[1].col, 1);
        assert_eq!(spans[1].cells, 10);
    }

    /// 混排 run（图标在中间/结尾）同样拆出，前后普通片段各成一段。
    #[test]
    fn splits_icon_at_middle_and_end() {
        let spans = render_term_span(&hist_span("a\u{e5fc}", 5), 0, true, pal());
        assert_eq!(spans.len(), 2);
        assert_eq!((spans[0].text.as_str(), spans[0].col), ("a", 5));
        assert!((spans[1].nerd) && spans[1].col == 6);

        let spans = render_term_span(&hist_span("\u{f15b}\u{e5fc}", 0), 0, true, pal());
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().all(|s| s.nerd));
        assert_eq!((spans[0].col, spans[1].col), (0, 1));
    }

    /// 前导空格 + 图标（旧判据直接放弃的形态）也能拆出图标。
    #[test]
    fn leading_space_then_icon() {
        let spans = render_term_span(&hist_span(" \u{e5fc}name", 2), 0, true, pal());
        assert_eq!(spans.len(), 3);
        assert_eq!((spans[0].text.as_str(), spans[0].cells), (" ", 1));
        assert!(spans[1].nerd);
        assert_eq!(spans[2].text.as_str(), "name");
        assert_eq!(spans[2].col, 4);
    }

    /// Powerline 分隔符（提示符里的 ``/``/``）恰好填满格子、首尾相接，
    /// 绝不能拆出放大 —— 否则接缝被撕开。
    #[test]
    fn powerline_separators_stay_plain() {
        for cp in ['\u{e0b0}', '\u{e0b1}', '\u{e0b2}', '\u{e0b3}', '\u{e0a0}', '\u{e0d4}'] {
            assert!(!is_scaled_icon_char(cp), "U+{:04X} 是 Powerline 符号", cp as u32);
        }
        // 整段 prompt 片段保持单 span、不放大
        let text = "\u{e0b0} \u{e0b0}";
        let spans = render_term_span(&hist_span(text, 0), 0, true, pal());
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].nerd);
        assert_eq!(spans[0].text.as_str(), text);
    }

    /// 没有图标字符的 run 保持一 run 一 span，不付拆分开销。
    #[test]
    fn plain_run_stays_single_span() {
        for text in [
            "backup_t_line_dual_name_20260528110821.sql",
            "drwxr-xr-x.",
            "[root@localhost ~]#",
            "中文混排、含标点",
        ] {
            let spans = render_term_span(&hist_span(text, 0), 0, true, pal());
            assert_eq!(spans.len(), 1, "{text:?}");
            assert!(!spans[0].nerd);
            assert_eq!(spans[0].cjk, text.contains('中'));
        }
    }

    /// `ls --color` 的常用图标码位全部命中放大判定。
    #[test]
    fn recognises_ls_icon_glyphs() {
        for (cp, what) in [
            ('\u{e5fc}', "directory"),
            ('\u{f15b}', "file"),
            ('\u{f013}', "cog"),
            ('\u{e615}', "locked"),
            ('\u{f408}', "database"),
        ] {
            assert!(is_scaled_icon_char(cp), "{what} U+{:04X} 应识别为图标", cp as u32);
        }
    }

    /// 拆分后的总格数与原 run 一致（网格/PTY 寻址不变）。
    #[test]
    fn split_preserves_total_cells() {
        let text = "\u{e5fc}  两个目录名";
        let orig = hist_span(text, 7);
        let spans = render_term_span(&orig, 1, true, pal());
        let total: i32 = spans.iter().map(|s| s.cells).sum();
        assert_eq!(total, orig.cells);
        // 首段列号 = 原 run 列号，末段列号收尾连贯
        assert_eq!(spans.first().unwrap().col, 7);
        let mut expect = 7;
        for s in &spans {
            assert_eq!(s.col, expect);
            expect += s.cells;
        }
    }
}
