//! 取色盘业务模块：Slint `ColorPickerDialog`（ui/color_picker.slint）背后的
//! 纯颜色数学与位图烘焙。
//!
//! 与 `theme` 的分工：theme 管「主题配色」业务（调色板、槽位、强调色 3:1
//! 守卫）；这里只管「取色」这件事本身 —— hex/HSV/HSL 换算、色轮命中、色块
//! 字色判定、色轮与明度条位图。Slint 侧组件经 `PickerMath` 全局回调使用，
//! 接线在 src/app.rs 的「PickerMath 接线」块。
//!
//! 位图烘焙（依赖 slint::Image）单独放在 [`images`] 子模块。

/// 解析 "#RRGGBB" / "RRGGBB"；非法（位数/字符不对）返回 None。
pub fn parse_hex(s: &str) -> Option<u32> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    u32::from_str_radix(s, 16).ok()
}

/// HSV（h:0..360、s:0..1、v:0..1）→ `0xRRGGBB`。取色盘用；与色轮命中共用，
/// 所以 hsv_to_hex(hex_to_hsv(c)) 必须能精确回到 c（避免往返偏色）。
pub fn hsv_to_hex(h: f32, s: f32, v: f32) -> u32 {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let q = |f: f32| (((f + m) * 255.0).round().clamp(0.0, 255.0)) as u32;
    (q(r) << 16) | (q(g) << 8) | q(b)
}

/// `0xRRGGBB` → (h, s, v)，色相归一到 **0..360**（见函数末尾的 rem_euclid）。
/// 饱和度按 HSV 定义 s=(max-min)/max —— 不能用 HSL 的
/// d/(1-|2L-1|)，否则与 hsv_to_hex 往返会偏色（这正是高保真稿调试时踩过的坑）。
pub fn hex_to_hsv(c: u32) -> (f32, f32, f32) {
    let r = ((c >> 16) & 0xFF) as f32 / 255.0;
    let g = ((c >> 8) & 0xFF) as f32 / 255.0;
    let b = (c & 0xFF) as f32 / 255.0;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    if d <= f32::EPSILON {
        return (0.0, 0.0, mx);
    }
    let h = if mx == r {
        ((g - b) / d) % 6.0
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    // ⚠️ Rust 的 `%` 是「余数」不是「模」：红区（mx==r 且 g<b）时 (g-b)/d ∈ (-1,0)，
    // h*60 落在 -60..0 —— H 输入框会显示 -32 而不是 328（#F279B9 实测）。
    // 三个分支里只有 mx==r 可能为负，但统一归一最省心（其余分支 rem_euclid 无副作用）。
    ((h * 60.0).rem_euclid(360.0), d / mx, mx)
}

/// HSV（h:0..360、s/v:0..1）→ HSL（h:0..360、s/l:0..1）。取色盘 HSL 档用；
/// 与 hsl_to_hsv 互为逆变换（h 保持不变）。
pub fn hsv_to_hsl(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let l = v * (1.0 - s / 2.0);
    let sl = if l <= 0.0 || l >= 1.0 {
        0.0
    } else {
        (v - l) / l.min(1.0 - l)
    };
    (h.rem_euclid(360.0), sl.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

/// HSL（h:0..360、s/l:0..1）→ HSV。取色盘 HSL 档写回唯一事实源 hex 前使用。
pub fn hsl_to_hsv(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let v = l + s * l.min(1.0 - l);
    let sv = if v <= 0.0 { 0.0 } else { 2.0 * (1.0 - l / v) };
    (h.rem_euclid(360.0), sv.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
}

/// 色轮命中：dx/dy 是相对圆心的归一化坐标（-1..1，超出按半径 1 截断），
/// 返回「该点色相 + 该半径饱和度 + 给定明度」合成颜色值。
pub fn wheel_pick(dx: f32, dy: f32, v: f32) -> u32 {
    let (dx, dy) = (dx.clamp(-1.0, 1.0), dy.clamp(-1.0, 1.0));
    let r = (dx * dx + dy * dy).sqrt().min(1.0);
    let h = dy.atan2(dx).to_degrees().rem_euclid(360.0);
    hsv_to_hex(h, r, v.clamp(0.0, 1.0))
}

/// 色块上的字色判定：加权亮度（0.299/0.587/0.114）>140 用深字，否则白字。
/// 返回 `0x000000` / `0xFFFFFF`，调用方映射成 slint::Color。
pub fn ink_on(c: u32) -> u32 {
    let l = ((c >> 16 & 0xFF) as f32) * 0.299
        + ((c >> 8 & 0xFF) as f32) * 0.587
        + ((c & 0xFF) as f32) * 0.114;
    if l > 140.0 {
        0x000000
    } else {
        0xFFFFFF
    }
}

mod images;

pub use images::{bake_slider_image, bake_wheel_image};

// (#picker-input-edit 2026-10-15) 取色盘输入行的 UI 级回归测试（真派发鼠标/键盘
// 事件）。跑真断言需要 SLINT_EMIT_DEBUG_INFO=1，否则自行跳过 —— 见文件头。
#[cfg(test)]
mod ui_input_test;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_ok() {
        assert_eq!(parse_hex("#7C5CFF"), Some(0x7C5CFF));
        assert_eq!(parse_hex("7C5CFF"), Some(0x7C5CFF));
        assert_eq!(parse_hex("#zzzzzz"), None);
        assert_eq!(parse_hex("#12345"), None);
    }

    #[test]
    fn hsv_hex_roundtrip_exact() {
        // 取色盘唯一事实源是 hex：hsv_to_hex(hex_to_hsv(c)) 必须精确回到 c
        for c in [0xA400F4u32, 0x2AACB8, 0x76777A, 0x0B0D12, 0xFAFAFA, 0xFF0000, 0x808080] {
            let (h, s, v) = hex_to_hsv(c);
            assert_eq!(hsv_to_hex(h, s, v), c, "{c:06X} 往返偏色");
        }
    }

    #[test]
    fn hue_normalized_to_0_360() {
        // 回归：红区（mx==r 且 g<b）曾因 `%` 是余数而非模而返回**负色相** ——
        // HSV 档的 H 输入框会显示 -32 而不是 328。色相必须恒在 0..360。
        for c in [0xF279B9u32, 0xFF00FF, 0xFF0080, 0xFF0000, 0xFE0001, 0x800000] {
            let (h, s, v) = hex_to_hsv(c);
            assert!((0.0..360.0).contains(&h), "{c:06X} 色相 {h} 越界");
            assert_eq!(hsv_to_hex(h, s, v), c, "{c:06X} 往返偏色");
        }
        // #F279B9 的例：-(64/121)*60 = -31.7 → 328.3
        assert!((hex_to_hsv(0xF279B9).0 - 328.3).abs() < 0.6);
    }

    #[test]
    fn hsl_hsv_roundtrip() {
        // 取色盘 HSL 档：hsv→hsl→hsv 必须无损（h 不动，s/v 走两组互逆公式）
        for c in [0xA400F4u32, 0x2AACB8, 0x76777A, 0x0B0D12, 0xFAFAFA, 0xFF0000, 0x808080] {
            let (h, s, v) = hex_to_hsv(c);
            let (h2, s2, l) = hsv_to_hsl(h, s, v);
            let back = hsl_to_hsv(h2, s2, l);
            assert!((back.0 - h).abs() < 0.001, "{c:06X} 色相漂移");
            assert!((back.1 - s).abs() < 0.001, "{c:06X} 饱和漂移");
            assert!((back.2 - v).abs() < 0.001, "{c:06X} 明度漂移");
            // hex 往返也守恒（量化误差 ≤1/255）
            let re = hsv_to_hex(back.0, back.1, back.2);
            assert!((re as i32 - c as i32).abs() <= 0x010101, "{c:06X} → {re:06X}");
        }
        // 端点：黑/白/纯色在 HSL 域的落点
        assert_eq!(hsv_to_hsl(0.0, 0.0, 0.0), (0.0, 0.0, 0.0));
        assert_eq!(hsv_to_hsl(120.0, 0.0, 1.0), (120.0, 0.0, 1.0));
        assert_eq!(hsl_to_hsv(0.0, 1.0, 0.5), (0.0, 1.0, 1.0));
    }

    #[test]
    fn wheel_pick_axes() {
        // 数学坐标（+y 向上）：右=红、上=绿、左=青、下=紫；半径 0=灰白、v=0=黑
        assert_eq!(wheel_pick(1.0, 0.0, 1.0), 0xFF0000);
        assert_eq!(wheel_pick(0.0, 1.0, 1.0), 0x80FF00); // 90° = 黄绿
        assert_eq!(wheel_pick(-1.0, 0.0, 1.0), 0x00FFFF);
        assert_eq!(wheel_pick(0.0, 0.0, 1.0), 0xFFFFFF);
        assert_eq!(wheel_pick(0.5, 0.0, 0.0), 0x000000);
    }

    #[test]
    fn ink_on_threshold() {
        assert_eq!(ink_on(0xFAFAFA), 0x000000); // 白光色 → 黑字
        assert_eq!(ink_on(0x0B0D12), 0xFFFFFF); // 极深色 → 白字
    }
}
