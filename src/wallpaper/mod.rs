#[path = "struct/wallpaper.rs"]
mod wallpaper_types;
#[path = "impls/wallpaper.rs"]
mod wallpaper;
// (#wallpaper-library 2026-10-18) 数据目录 wallpapers/ 文件夹 = 壁纸库来源。
#[path = "impls/library.rs"]
pub(crate) mod library;

pub(crate) use wallpaper::*;
