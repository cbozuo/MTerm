//! The VNC framebuffer: decoded pixels in a fixed layout, plus the two
//! operations the RFB protocol needs (blit a raw rectangle, copy one rectangle
//! to another).
//!
//! vnc-rs hands back rectangles in whatever pixel format was negotiated, so the
//! decoding here is where "wire bytes" becomes "pixels I can draw".

use image::RgbaImage;

/// Largest framebuffer accepted in either dimension. A server announcing more
/// than this is either broken or hostile; allocating from its numbers without a
/// bound is a trivial out-of-memory.
pub const MAX_FRAMEBUFFER_SIDE: u16 = 8192;

/// A full-frame RGB buffer with the server's geometry.
pub struct VncFramebuffer {
    pub width: u16,
    pub height: u16,
    /// Row-major, 3 bytes per pixel.
    pub rgb: Vec<u8>,
}

impl VncFramebuffer {
    /// Allocate a black framebuffer, refusing sizes that cannot be indexed.
    pub fn new(width: u16, height: u16) -> Result<Self, String> {
        if width == 0
            || height == 0
            || width > MAX_FRAMEBUFFER_SIDE
            || height > MAX_FRAMEBUFFER_SIDE
        {
            return Err(format!("VNC framebuffer {width}x{height} is out of range"));
        }
        let len = usize::from(width) * usize::from(height) * 3;
        Ok(Self {
            width,
            height,
            rgb: vec![0u8; len],
        })
    }

    /// Copy server bytes for `rect` into the framebuffer.
    ///
    /// `data` is in the negotiated pixel format; `bpp` is its bytes-per-pixel.
    /// Out-of-range rectangles and short buffers are errors rather than panics:
    /// a malformed update must fail the session visibly, not take the tab down.
    pub fn blit_raw(&mut self, rect: Rect, data: &[u8], bpp: usize) -> Result<(), String> {
        if bpp == 0 {
            return Err("VNC bits_per_pixel is 0".to_string());
        }
        let (rx, ry) = (rect.x as usize, rect.y as usize);
        let (rw, rh) = (rect.width as usize, rect.height as usize);
        if rw == 0 || rh == 0 || rx + rw > self.width as usize || ry + rh > self.height as usize {
            return Err("VNC rectangle falls outside the framebuffer".to_string());
        }
        let src_stride = rw * bpp;
        if data.len() < src_stride * rh {
            return Err(format!(
                "VNC update is {} bytes, expected {} (bpp={bpp}, {rw}x{rh})",
                data.len(),
                src_stride * rh
            ));
        }
        let dst_stride = self.width as usize * 3;
        for row in 0..rh {
            let src = &data[row * src_stride..row * src_stride + src_stride];
            let dst_start = (ry + row) * dst_stride + rx * 3;
            let dst = &mut self.rgb[dst_start..dst_start + rw * 3];
            for col in 0..rw {
                let px = &src[col * bpp..col * bpp + bpp];
                // The client asks for BGRA, so the wire order for a 32bpp pixel
                // is B, G, R, A (see RFC 6143 §7.4 with
                // red_shift=16 / blue_shift=0 on a little-endian host).
                let (r, g, b): (u8, u8, u8) = match bpp {
                    4 => (px[2], px[1], px[0]),
                    2 => {
                        // 16bpp arrives as 5-6-5; each component is scaled from
                        // its 5/6-bit range up to 8 bits.
                        let v = u16::from_le_bytes([px[0], px[1]]);
                        let scale5 = |c: u16| ((c * 255) / 31) as u8;
                        let scale6 = |c: u16| ((c * 255) / 63) as u8;
                        (
                            scale5((v >> 10) & 0x1f),
                            scale6((v >> 5) & 0x3f),
                            scale5(v & 0x1f),
                        )
                    }
                    1 => (px[0], px[0], px[0]),
                    _ => (0, 0, 0),
                };
                dst[col * 3] = r;
                dst[col * 3 + 1] = g;
                dst[col * 3 + 2] = b;
            }
        }
        Ok(())
    }

    /// `CopyRect`: move pixels inside the framebuffer.
    ///
    /// Rows are copied through a scratch buffer because source and destination
    /// may overlap — RFB explicitly allows a copy that shifts the image. The
    /// scratch row handles a horizontal overlap; a *vertical* one needs the rows
    /// walked in the right direction as well, because writing a destination row
    /// can otherwise clobber a source row that has not been read yet. That is
    /// the common case: scrolling a window down copies a block from above onto
    /// itself.
    pub fn copy_rect(&mut self, dst: Rect, src: Rect) -> Result<(), String> {
        let w = src.width.min(dst.width) as usize;
        let h = src.height.min(dst.height) as usize;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let stride = self.width as usize * 3;
        if (src.x as usize) + w > self.width as usize
            || (src.y as usize) + h > self.height as usize
            || (dst.x as usize) + w > self.width as usize
            || (dst.y as usize) + h > self.height as usize
        {
            return Err("VNC CopyRect falls outside the framebuffer".to_string());
        }
        let mut row_buf = vec![0u8; w * 3];
        // Copy away from the overlap: when the destination starts *below* the
        // source, the last row has to be moved first, or the second write reads
        // a row the first write already overwrote. Moving up is safe in the
        // natural order.
        let downwards = dst.y > src.y;
        for step in 0..h {
            let y = if downwards { h - 1 - step } else { step };
            let src_start = (src.y as usize + y) * stride + src.x as usize * 3;
            row_buf.copy_from_slice(&self.rgb[src_start..src_start + w * 3]);
            let dst_start = (dst.y as usize + y) * stride + dst.x as usize * 3;
            self.rgb[dst_start..dst_start + w * 3].copy_from_slice(&row_buf);
        }
        Ok(())
    }

    /// Snapshot as an RGBA image for the renderer.
    ///
    /// Copies rather than aliasing because the RGB layout differs from RGBA and
    /// Slint needs its own owned buffer anyway.
    pub fn to_rgba(&self) -> RgbaImage {
        let mut img = RgbaImage::new(u32::from(self.width), u32::from(self.height));
        let count = (self.width as usize) * (self.height as usize);
        {
            // `RgbaImage::as_mut` yields `&mut [u8]`, so each pixel's four bytes
            // are written as a slice rather than through the `Rgba` wrapper.
            let dst = img.as_mut();
            for (i, src) in self.rgb.chunks_exact(3).take(count).enumerate() {
                dst[i * 4..i * 4 + 4].copy_from_slice(&[src[0], src[1], src[2], 0xff]);
            }
        }
        img
    }
}

/// A rectangle in framebuffer coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bgra_update_lands_in_the_right_channels() {
        let mut fb = VncFramebuffer::new(2, 1).unwrap();
        // BGRA: blue=0x10, green=0x20, red=0x30.
        let data = [0x10u8, 0x20, 0x30, 0xff];
        fb.blit_raw(
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            &data,
            4,
        )
        .unwrap();
        assert_eq!(&fb.rgb[0..3], &[0x30, 0x20, 0x10]);
    }

    #[test]
    fn an_out_of_range_rectangle_is_an_error_not_a_panic() {
        let mut fb = VncFramebuffer::new(4, 4).unwrap();
        let data = [0u8; 16 * 4];
        let err = fb
            .blit_raw(
                Rect {
                    x: 3,
                    y: 0,
                    width: 4,
                    height: 1,
                },
                &data,
                4,
            )
            .unwrap_err();
        assert!(err.contains("outside"));
    }

    #[test]
    fn a_short_update_is_an_error_not_a_panic() {
        let mut fb = VncFramebuffer::new(4, 4).unwrap();
        let err = fb
            .blit_raw(
                Rect {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 4,
                },
                &[0u8; 8],
                4,
            )
            .unwrap_err();
        assert!(err.contains("expected"));
    }

    #[test]
    fn copy_rect_handles_overlapping_regions() {
        // Shifting the whole image one pixel left is the classic overlap case:
        // a naive forward copy would smear the first column across the rest.
        let mut fb = VncFramebuffer::new(4, 1).unwrap();
        for i in 0..4 {
            fb.rgb[i * 3] = (i as u8 + 1) * 10;
        }
        fb.copy_rect(
            Rect {
                x: 0,
                y: 0,
                width: 3,
                height: 1,
            },
            Rect {
                x: 1,
                y: 0,
                width: 3,
                height: 1,
            },
        )
        .unwrap();
        assert_eq!(
            fb.rgb[0..3],
            [20, 0, 0],
            "first pixel must come from the source, not from the partially written dest"
        );
    }

    #[test]
    fn copy_rect_walks_rows_backwards_when_the_destination_is_below() {
        // Scrolling down: the destination starts *below* the source, so a
        // forward row walk writes row 1 before row 2 has been read and smears
        // the first source row over everything beneath it.
        let mut fb = VncFramebuffer::new(1, 4).unwrap();
        for i in 0..4 {
            fb.rgb[i * 3] = (i as u8 + 1) * 10; // 10, 20, 30, 40
        }
        fb.copy_rect(
            Rect {
                x: 0,
                y: 1,
                width: 1,
                height: 3,
            },
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 3,
            },
        )
        .unwrap();
        let got = [fb.rgb[0], fb.rgb[3], fb.rgb[6], fb.rgb[9]];
        assert_eq!(got, [10, 10, 20, 30], "each row moves down by one");
    }

    #[test]
    fn copy_rect_moving_up_keeps_the_rows_it_has_not_read_yet() {
        // The mirror case is already safe in a forward walk, but it must stay
        // that way: reversing unconditionally would break it instead.
        let mut fb = VncFramebuffer::new(1, 4).unwrap();
        for i in 0..4 {
            fb.rgb[i * 3] = (i as u8 + 1) * 10; // 10, 20, 30, 40
        }
        fb.copy_rect(
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 3,
            },
            Rect {
                x: 0,
                y: 1,
                width: 1,
                height: 3,
            },
        )
        .unwrap();
        let got = [fb.rgb[0], fb.rgb[3], fb.rgb[6], fb.rgb[9]];
        assert_eq!(got, [20, 30, 40, 40], "each row moves up by one");
    }

    #[test]
    fn a_zero_sized_framebuffer_is_refused() {
        assert!(VncFramebuffer::new(0, 100).is_err());
        assert!(VncFramebuffer::new(100, 0).is_err());
        assert!(VncFramebuffer::new(MAX_FRAMEBUFFER_SIDE + 1, 10).is_err());
    }

    #[test]
    fn the_rgba_snapshot_carries_full_opacity() {
        let mut fb = VncFramebuffer::new(1, 1).unwrap();
        fb.rgb = vec![1, 2, 3];
        let img = fb.to_rgba();
        assert_eq!(img.as_raw()[..4], [1, 2, 3, 0xff]);
    }
}
