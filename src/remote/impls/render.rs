//! Turning a decoded remote frame into something Slint can draw.
//!
//! Both clients end up with "a rectangle of RGBA pixels"; everything
//! protocol-specific has already been resolved by the time a frame reaches
//! here. Keeping the conversion in one place is what lets the RDP and VNC
//! drivers stay thin and guarantees the two tabs scale identically.

use image::RgbaImage;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// Longest edge a delivered frame may have.
///
/// The desktop is rendered into an `Image` and then scaled by the view, so an
/// oversized buffer only costs memory and upload time. 2560 px on the long edge
/// matches what the wallpaper pipeline uses, and any real desktop (4K = 3840)
/// still downscales to something visually indistinguishable at tab size.
pub const MAX_FRAME_EDGE: u32 = 2560;

/// Pack an RGBA image into a Slint pixel buffer, downscaling if it is oversized.
///
/// The aspect ratio is preserved and the UI letterboxes the result, so a
/// non-matching buffer size never stretches the picture (P0-6).
pub fn to_image(img: RgbaImage) -> Image {
    Image::from_rgba8(to_buffer(img))
}

/// The buffer half of [`to_image`], exposed separately because the tab view
/// also needs the pixel size to map pointer coordinates back to remote pixels.
pub fn to_buffer(img: RgbaImage) -> SharedPixelBuffer<Rgba8Pixel> {
    let (w, h) = img.dimensions();
    let img = if w.max(h) > MAX_FRAME_EDGE && w > 0 && h > 0 {
        let scale = MAX_FRAME_EDGE as f32 / w.max(h) as f32;
        let nw = ((w as f32 * scale) as u32).max(1);
        let nh = ((h as f32 * scale) as u32).max(1);
        // Triangle filter: a box filter would alias badly on text, which is
        // most of a desktop.
        image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
    } else {
        img
    };
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
    pub fn scale(&self) -> f32 {
        let (fw, fh) = (self.frame_w as f32, self.frame_h as f32);
        let (vw, vh) = (self.view_w, self.view_h);
        if fw <= 0.0 || fh <= 0.0 || vw <= 0.0 || vh <= 0.0 {
            return 0.0;
        }
        (vw / fw).min(vh / fh)
    }

    /// Top-left of the drawn image inside the tab (the letterbox bars).
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
}
