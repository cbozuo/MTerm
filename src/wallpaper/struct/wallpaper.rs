use slint::Image;

/// Colours derived from a wallpaper, pushed into the Theme global so panels,
/// accent and backgrounds harmonise with the image.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Vivid accent colour sharing the wallpaper's dominant hue.
    pub accent: (u8, u8, u8),
}

pub struct Wallpaper {
    pub image: Image,
    pub palette: Palette,
}
