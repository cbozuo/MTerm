//! (#panel-menu-unify 2026-11-17) 面板级菜单出图：面板 ⋯ 与列表空白处右键
//! 现在是**同一份** `PanelMenu` 组件，这个探针把两个入口各渲染一张，方便核对
//! 项集/间距/宽度是否一致（也留作以后加项的回归图）。
//!
//! 运行：`cargo run --example panel_menu_shot`
//! 输出：output/panel_menu/{blank,panel}.png

use std::rc::Rc;

use mterm::ui::AppWindow;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::ComponentHandle;

thread_local! {
    static WIN: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}

struct ShotPlatform;
impl Platform for ShotPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(WIN.with(|w| w.clone()))
    }
}

const W: usize = 1280;
const H: usize = 900;

fn main() -> Result<(), slint::PlatformError> {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = AppWindow::new()?;
    ui.window()
        .set_size(slint::PhysicalSize::new(W as u32, H as u32));
    ui.show()?;

    for (kind, name) in [("blank", "blank"), ("panel", "panel")] {
        // 菜单锚在面板中部：上下都有空间，上翻/下翻两个分支都不会把菜单裁掉。
        ui.set_ctx_open_key(kind.into());
        ui.set_ctx_menu_kind(kind.into());
        ui.set_ctx_menu_x(260.0);
        ui.set_ctx_menu_y(200.0);
        ui.set_ctx_menu_space_below(640.0);
        grab(&ui, &format!("output/panel_menu/{name}.png"));
        println!("wrote output/panel_menu/{name}.png");
    }
    Ok(())
}

fn grab(_ui: &AppWindow, path: &str) {
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    let win = WIN.with(|w| w.clone());
    for _ in 0..4 {
        win.request_redraw();
        let _ = win.draw_if_needed(|renderer| {
            renderer.render(&mut buf, W as usize);
        });
    }
    let mut img = image::RgbaImage::new(W as u32, H as u32);
    for (i, px) in buf.iter().enumerate() {
        let (x, y) = ((i % W) as u32, (i / W) as u32);
        img.put_pixel(x, y, image::Rgba([px.red, px.green, px.blue, 255]));
    }
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    img.save(path).ok();
}
