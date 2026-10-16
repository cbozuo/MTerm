//! 取色盘位图烘焙：色轮与明度条。Slint 无法程序化画渐变，由 Rust 烘成
//! `slint::Image`；位图按显示尺寸的 2× 烘，保证 200% DPI 不糊。

use super::hsv_to_hex;

/// 色轮位图：圆心=色相（角度）、半径=饱和度、明度固定 1；明度由旁边的竖条
/// 另调、Slint 侧用黑色蒙罩（opacity=1-v）压暗 —— 与逐像素缩放 V 等价。
/// 按 `size`（显示尺寸的 2 倍）烘，保证 200% 屏不糊；圆外像素 alpha=0。
pub fn bake_wheel_image(size: u32) -> slint::Image {
    let mut buf = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let dy = 1.0 - (y as f32 + 0.5) / size as f32 * 2.0;
            let rr = (dx * dx + dy * dy).sqrt();
            let h = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            let c = hsv_to_hex(h, rr.min(1.0), 1.0);
            buf.push(((c >> 16) & 0xFF) as u8);
            buf.push(((c >> 8) & 0xFF) as u8);
            buf.push((c & 0xFF) as u8);
            buf.push(if rr <= 1.0 { 255 } else { 0 });
        }
    }
    let mut px = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(size, size);
    px.make_mut_bytes().copy_from_slice(&buf);
    slint::Image::from_rgba8(px)
}

/// 明度条渐变位图（顶=当前色相满饱和、底=黑）。显示 22×240，按 2× 烘成
/// 44×480；色相随色轮拖动换，故每次改色重烘。
pub fn bake_slider_image(hue: f32) -> slint::Image {
    const W: u32 = 44;
    const H: u32 = 480;
    let top = hsv_to_hex(hue, 1.0, 1.0);
    let mut buf = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        // 顶→底：满饱和色相线性掺黑（高保真的 linear-gradient(to top,#000,hue)）
        let t = 1.0 - (y as f32 + 0.5) / H as f32;
        let mix = |a: u32, b: u32| ((a as f32 * t + b as f32 * (1.0 - t)).round()) as u8;
        let (r, g, b) = (
            mix((top >> 16) & 0xFF, 0),
            mix((top >> 8) & 0xFF, 0),
            mix(top & 0xFF, 0),
        );
        for _ in 0..W {
            buf.push(r);
            buf.push(g);
            buf.push(b);
            buf.push(255);
        }
    }
    let mut px = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(W, H);
    px.make_mut_bytes().copy_from_slice(&buf);
    slint::Image::from_rgba8(px)
}

/// 色相条位图（左 = 0° 红，右 ≈ 360° 回到红；满饱和、满亮度），给 H 分量滑杆
/// 当底。内容与当前选色无关，**开机烘一次**即可。显示约 200×6，按 2× 烘。
///
/// 存在的理由：H 条原先用 Slint 的**七停** `@linear-gradient`（其余分量是两停），
/// 实测有可见断层（用户反馈「彩色滑动条显示不全 → 有断层」）。位图逐像素求色，
/// 与明度条同一范式 —— Slint 侧只负责按尺寸贴图。
pub fn bake_hue_image(w: u32, h: u32) -> slint::Image {
    let mut buf = Vec::with_capacity((w * h * 4) as usize);
    for _y in 0..h {
        for x in 0..w {
            let c = hue_at(x, w);
            buf.push(((c >> 16) & 0xFF) as u8);
            buf.push(((c >> 8) & 0xFF) as u8);
            buf.push((c & 0xFF) as u8);
            buf.push(255);
        }
    }
    let mut px = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    px.make_mut_bytes().copy_from_slice(&buf);
    slint::Image::from_rgba8(px)
}

/// 色相条第 `x` 列（宽 `w`）的色值：0° 红 → 360° 回到红，均匀铺满整个宽度
/// （末列半像素逼近 360°）。抽成独立函数是为了能单测 —— 位图本身不便回读像素。
fn hue_at(x: u32, w: u32) -> u32 {
    hsv_to_hex((x as f32 + 0.5) / w as f32 * 360.0, 1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(c: u32) -> (u32, u32, u32) {
        ((c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF)
    }

    /// (#picker-hue-ramp 2026-10-16) 色相条必须**覆盖整个色相环**、首尾都落在红。
    /// 防「只画了半条 / 色相映射错位」这类回归 —— 用户报的「断层」正是在这条条上
    /// （原七停 @linear-gradient 换成按像素求色的位图后，这里锁住取值口径）。
    #[test]
    fn hue_strip_spans_the_whole_hue_circle() {
        let w = 1024;
        let (r0, g0, b0) = split(hue_at(0, w));
        assert!(r0 > 200 && g0 < 60 && b0 < 60, "左端应近纯红：{:06X}", hue_at(0, w));
        let (rl, gl, bl) = split(hue_at(w - 1, w));
        assert!(rl > 200 && gl < 60 && bl < 60, "右端应回到红：{:06X}", hue_at(w - 1, w));
        let (rm, gm, bm) = split(hue_at(w / 2, w));
        assert!(gm > 200 && bm > 200 && rm < 60, "正中应近青：{:06X}", hue_at(w / 2, w));
    }
}
