//! (#wallpaper-library 2026-10-18) 自定义壁纸库。
//!
//! 数据目录下的 `wallpapers/` 文件夹是壁纸列表的**唯一来源**（与高保真
//! `wallpaper-hifi-2026-10-17` 的「该文件夹即壁纸列表的来源」一致）：
//! 放进去的图片自动出现在外观页，删掉文件即从列表消失。
//!
//! 本模块只做文件与缩略图，不碰 Slint 模型 —— 模型重建统一由 `app.rs`
//! 的 `refresh_wallpaper_items` 完成（单一写入点）。

use std::path::{Path, PathBuf};

use slint::{Image, SharedPixelBuffer, Rgba8Pixel};

use crate::datastore::data_dir;

/// 缩略图尺寸：2× 卡片（79×50 逻辑像素），高分屏下不糊。
const THUMB_W: u32 = 160;
const THUMB_H: u32 = 104;
/// 缩略图解码上限：文件本身超过这个大小就跳过（极端大图解码会吃掉几百 MB，
/// 列表宁可少一张卡，也不要在启动时把内存顶爆）。
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// 库里的一张图。
pub struct LibraryItem {
    /// 绝对路径；同时就是 `config.wallpaper` 里存的 id。
    pub path: PathBuf,
    /// 文件名去扩展名（卡片标题）。
    pub name: String,
    /// 缩略图（160×104，UI 侧 cover 裁切）。
    pub thumb: Image,
}

/// `<data_dir>/wallpapers`（按需创建）。扫描/导入/删除都以它为准。
pub fn dir() -> PathBuf {
    let dir = data_dir().join("wallpapers");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// 与文件选择框的过滤器保持一致（PNG / JPG / WEBP / BMP）。
fn is_supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "bmp")
    )
}

fn display_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// 列出库里的壁纸：按修改时间从新到旧（刚上传的排最前）。解码失败的
/// 文件直接跳过 —— 列表宁可少一张，也不要在画廊里留空卡。
pub fn list() -> Vec<LibraryItem> {
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_supported(p))
        .map(|p| {
            let modified = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (modified, p)
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
        .into_iter()
        .filter_map(|(_, path)| {
            let thumb = thumbnail(&path)?;
            Some(LibraryItem {
                name: display_name(&path),
                path,
                thumb,
            })
        })
        .collect()
}

/// 解码 + 等比缩到缩略图档位（长边不超过 160×104 的包围盒）。
fn thumbnail(path: &Path) -> Option<Image> {
    if std::fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    let img = image::open(path).ok()?.to_rgba8();
    let thumb = image::imageops::thumbnail(&img, THUMB_W, THUMB_H);
    let (w, h) = thumb.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    buf.make_mut_bytes().copy_from_slice(thumb.as_raw());
    Some(Image::from_rgba8(buf))
}

/// 该路径是否已经在库里（同一目录即可，用于跳过自复制）。
fn inside_library(path: &Path) -> bool {
    let dir = dir();
    let parent = path.parent().unwrap_or(Path::new(""));
    let same = |a: &Path, b: &Path| -> bool {
        let ca = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
        let cb = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
        ca == cb
    };
    same(parent, &dir)
}

/// 把用户挑的图片**复制**进库（原件不动），文件名冲突时加 ` (2)`、` (3)`。
/// 已经在库里就直接返回原路径；复制失败返回 None（调用方决定是否退回原路径）。
pub fn import(src: &Path) -> Option<PathBuf> {
    if inside_library(src) {
        return Some(src.to_path_buf());
    }
    let file_name = src.file_name()?.to_string_lossy().into_owned();
    let stem = Path::new(&file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "wallpaper".to_string());
    let ext = Path::new(&file_name)
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "png".to_string());
    let dir = dir();
    let mut dst = dir.join(&file_name);
    let mut n = 2;
    while dst.exists() {
        dst = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    std::fs::copy(src, &dst).ok()?;
    Some(dst)
}

/// 删除库内的一张图。**只允许删库里的文件** —— 老配置里存的是库外路径时，
/// 绝不能把用户的原始照片删掉（返回 PermissionDenied，调用方只清选中态）。
pub fn delete(path: &Path) -> std::io::Result<()> {
    if !inside_library(path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "not inside the wallpapers folder",
        ));
    }
    std::fs::remove_file(path)
}

/// 在系统文件管理器中定位这张图（Windows: 资源管理器选中该文件）。
pub fn reveal(path: &Path) {
    #[cfg(windows)]
    {
        // explorer 的成功返回值是 1（不是 0），只 spawn 不等结果。
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_extensions() {
        for name in ["a.png", "b.JPG", "c.jpeg", "d.webp", "e.bmp"] {
            assert!(is_supported(Path::new(name)), "{name}");
        }
        for name in ["a.gif", "b.txt", "c", "d.png.bak"] {
            assert!(!is_supported(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn display_name_strips_extension() {
        assert_eq!(display_name(Path::new("C:/w/山景.png")), "山景");
        assert_eq!(display_name(Path::new("C:/w/no-ext")), "no-ext");
    }
}
