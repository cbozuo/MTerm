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

/// 明度条渐变位图（顶=当前色相满饱和、底=黑）。显示 22×216，按 2× 烘成
/// 44×432；色相随色轮拖动换，故每次改色重烘。
pub fn bake_slider_image(hue: f32) -> slint::Image {
    const W: u32 = 44;
    const H: u32 = 432;
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
