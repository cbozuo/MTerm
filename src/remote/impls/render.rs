//! Turning a decoded remote frame into something Slint can draw.
//!
//! Both clients end up with "a rectangle of RGBA pixels"; everything
//! protocol-specific has already been resolved by the time a frame reaches
//! here. Keeping the conversion in one place is what lets the RDP and VNC
//! drivers stay thin and guarantees the two tabs scale identically.

use image::RgbaImage;
use slint::{Rgba8Pixel, SharedPixelBuffer};

/// Pack a frame into a Slint pixel buffer **at its own size**.
///
/// A remote desktop is the content itself, drawn one screen pixel per remote
/// pixel, so the buffer keeps the size it arrived with. Any resize here is
/// undone by the view's own scaling, and a downscale-then-upscale is what
/// turns a sharp desktop into a soft one (#rdp-pixel-exact 2026-11-16).
///
/// This used to clamp the long edge to 2560 px — a limit taken from the
/// wallpaper pipeline, which resamples by nature. A desktop is not a wallpaper:
/// a 3055 px-wide tab was silently served a 2560 px frame that the view then
/// stretched back to 3055, which is exactly the blur this replaces. (The
/// wallpaper path never even went through here; it has its own copy.)
///
/// Called from the session's thread, not the UI thread, and returns a buffer
/// rather than an `Image` because `Image` cannot cross threads
/// (#rdp-frame-off-ui 2026-11-16).
pub fn to_buffer_exact(img: &RgbaImage) -> SharedPixelBuffer<Rgba8Pixel> {
    let (w, h) = img.dimensions();
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    // `image` stores RGBA byte-wise, which is exactly Slint's Rgba8Pixel layout,
    // so this is a copy, not a conversion.
    buf.make_mut_bytes().copy_from_slice(img.as_raw());
    buf
}

/// How a remote frame is fitted into the tab, and where a local pointer
/// position lands inside it.
///
/// The view letterboxes the frame (P0-6: no stretching, no cropping), which
/// means the drawn image does **not** fill the tab. Pointer coordinates
/// therefore have to be mapped through the same fit, otherwise clicking near a
/// corner would land tens of remote pixels away from where the user pointed
/// (P0-8).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))] // 测试 API
pub struct Viewport {
    /// Remote frame size in pixels.
    pub frame_w: u16,
    pub frame_h: u16,
    /// Tab content size in logical pixels.
    pub view_w: f32,
    pub view_h: f32,
}

impl Viewport {
    /// Uniform scale factor of the letterboxed fit. 0 when there is nothing to
    /// draw, so callers can treat it as "no mapping available".
#[cfg_attr(not(test), allow(dead_code))] // 测试 API（render 几何测试在用）
    pub fn scale(&self) -> f32 {
        let (fw, fh) = (self.frame_w as f32, self.frame_h as f32);
        let (vw, vh) = (self.view_w, self.view_h);
        if fw <= 0.0 || fh <= 0.0 || vw <= 0.0 || vh <= 0.0 {
            return 0.0;
        }
        (vw / fw).min(vh / fh)
    }

    /// Top-left of the drawn image inside the tab (the letterbox bars).
#[cfg_attr(not(test), allow(dead_code))] // 测试 API（render 几何测试在用）
    pub fn origin(&self) -> (f32, f32) {
        let s = self.scale();
        let drawn_w = self.frame_w as f32 * s;
        let drawn_h = self.frame_h as f32 * s;
        ((self.view_w - drawn_w) / 2.0, (self.view_h - drawn_h) / 2.0)
    }

    /// Map a pointer position in tab coordinates to remote frame coordinates.
    ///
    /// Returns `None` when the pointer is in a letterbox bar (there is no remote
    /// pixel under it) or the frame has no size yet — the caller then drops the
    /// event instead of sending a bogus coordinate to the server.
#[cfg_attr(not(test), allow(dead_code))] // 测试 API（render 几何测试在用）
    pub fn to_remote(&self, x: f32, y: f32) -> Option<(u16, u16)> {
        let s = self.scale();
        if s <= 0.0 {
            return None;
        }
        let (ox, oy) = self.origin();
        let rx = ((x - ox) / s).floor();
        let ry = ((y - oy) / s).floor();
        if rx < 0.0 || ry < 0.0 {
            return None;
        }
        let (rx, ry) = (rx as u32, ry as u32);
        if rx >= self.frame_w as u32 || ry >= self.frame_h as u32 {
            return None;
        }
        Some((rx as u16, ry as u16))
    }
}

/// Reject a proposed remote desktop size that no server would accept.
///
/// IronRDP and the VNC framebuffer both index by `u16`, and a server that
/// cannot resize mid-session simply keeps the size it was given — asking for a
/// 0x0 or a 20000-px desktop either panics the client or gets the session
/// dropped. Clamping here (P0-9) means a transient 0-size tab during a window
/// minimise can never reach the wire.
pub fn clamp_desktop_size(w: u16, h: u16) -> Option<(u16, u16)> {
    const MIN_SIDE: u16 = 200;
    const MAX_SIDE: u16 = 8192;
    let w = w.clamp(MIN_SIDE, MAX_SIDE);
    let h = h.clamp(MIN_SIDE, MAX_SIDE);
    // Round down to a multiple of 4: RDP encodes the desktop size in 4-pixel
    // units, so an odd size makes the server round and the frame we get back no
    // longer matches the size we asked for. MIN_SIDE is far above 4, so the
    // clamp above already rules out a zero here.
    Some((w & !3, h & !3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_frame_is_letterboxed_with_bars_on_the_sides() {
        // 1600x800 into a square 800x800 tab: scale 0.5, so 800x400 drawn and
        // 200px bars above and below.
        let vp = Viewport {
            frame_w: 1600,
            frame_h: 800,
            view_w: 800.0,
            view_h: 800.0,
        };
        assert!((vp.scale() - 0.5).abs() < 1e-6);
        let (ox, oy) = vp.origin();
        assert!((ox - 0.0).abs() < 1e-6);
        assert!((oy - 200.0).abs() < 1e-6);
    }

    #[test]
    fn pointer_coordinates_map_through_the_letterbox() {
        // Same 1600x800 frame in an 800x800 tab: the top 200px is a bar, the
        // image starts at y=200 and the centre of the tab is the centre of the
        // frame.
        let vp = Viewport {
            frame_w: 1600,
            frame_h: 800,
            view_w: 800.0,
            view_h: 800.0,
        };
        // Centre of the tab -> centre of the remote desktop.
        assert_eq!(vp.to_remote(400.0, 400.0), Some((800, 400)));
        // Top-left pixel of the image.
        assert_eq!(vp.to_remote(0.0, 200.0), Some((0, 0)));
        // Inside the letterbox bar: no remote pixel lives here.
        assert_eq!(vp.to_remote(400.0, 10.0), None);
        // Past the right edge of the frame. The image is 800 view-pixels wide
        // here (1600 at scale 0.5), so it fills the tab's width exactly and the
        // first column without a remote pixel is x = 800.
        assert_eq!(vp.to_remote(810.0, 400.0), None);
    }

    #[test]
    fn a_zero_sized_view_maps_nothing_rather_than_dividing_by_zero() {
        let vp = Viewport {
            frame_w: 0,
            frame_h: 0,
            view_w: 800.0,
            view_h: 600.0,
        };
        assert_eq!(vp.scale(), 0.0);
        assert_eq!(vp.to_remote(10.0, 10.0), None);
    }

    #[test]
    fn desktop_sizes_are_clamped_and_four_pixel_aligned() {
        // A collapsed tab during a window minimise must not reach the wire.
        assert_eq!(clamp_desktop_size(0, 0), Some((200, 200)));
        assert_eq!(clamp_desktop_size(100, 100), Some((200, 200)));
        // Rounded down to the 4-pixel unit the protocol encodes in.
        assert_eq!(clamp_desktop_size(1281, 721), Some((1280, 720)));
        // Oversized is clamped rather than refused, so a 4K tab still connects.
        // `u16::MAX` is the largest size the protocol could even name.
        assert_eq!(clamp_desktop_size(u16::MAX, u16::MAX), Some((8192, 8192)));
    }

    /// A desktop frame is delivered at the size it arrived with
    /// (#rdp-pixel-exact 2026-11-16).
    ///
    /// The long edge used to be clamped to 2560 px on the way here, so a tab
    /// wider than that was served a smaller frame and the view stretched it
    /// back — the frame was already a resample before it was drawn.
    #[test]
    fn a_desktop_frame_keeps_its_own_size() {
        let img = RgbaImage::new(3055, 1789);
        let buf = to_buffer_exact(&img);
        assert_eq!(buf.width(), 3055);
        assert_eq!(buf.height(), 1789);
    }

    /// The pixels are copied through untouched: the frame is already in the
    /// layout the UI draws, so this must stay a copy rather than a conversion.
    #[test]
    fn a_desktop_frame_keeps_its_pixels() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, image::Rgba([1, 2, 3, 4]));
        img.put_pixel(1, 0, image::Rgba([5, 6, 7, 8]));
        let buf = to_buffer_exact(&img);
        assert_eq!(buf.as_bytes(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
