//! (#panel-anim 2026-11-05) 侧栏收展动效离屏出图 —— 与高保真稿逐项比对的证据图。
//!
//! 运行:`cargo run --example panel_anim_shot`（example = bin 类链接，绕开
//! test 目标在这台机器上的 dlltool 0xc0000409 环境病）。
//! 输出:output/panel-anim/{01-settings,02-both-open,03-both-collapsed,04-push-collapse}.png
//!
//! 01 = 设置弹框「侧栏」页（收展动效分组，对照高保真 §01）；
//! 02/03 = 主窗口两栏展开/收起终态；04 = 推挤收起中间帧（动画进行中）。

use mterm::ui::{AppWindow, PanelAnim};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::fs;
use std::rc::Rc;

/// 推进 Slint 动画时钟：离屏无事件循环，动画 tick 由事件处理驱动
/// （同 tests/panel_anim.rs 的 advance）。
fn advance(ui: &AppWindow, ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(-10.0, -10.0),
    });
}

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

const W: usize = 1200;
const H: usize = 780;

fn grab(ui: &AppWindow, buf: &mut Vec<PremultipliedRgbaColor>, path: &str) {
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(buf, W);
        });
    });
    let mut pixels: Vec<u8> = Vec::with_capacity(W * H * 4);
    for p in buf.iter() {
        let (r, g, b) = if p.alpha == 0 {
            (0, 0, 0)
        } else {
            let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
            (un(p.red), un(p.green), un(p.blue))
        };
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
        pixels.push(p.alpha);
    }
    let img = image::RgbaImage::from_raw(W as u32, H as u32, pixels).unwrap();
    fs::create_dir_all("output/panel-anim").ok();
    img.save(path).expect("save png");
    println!("saved {path}");
}

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = AppWindow::new().expect("build app window");
    ui.window().set_size(slint::LogicalSize::new(W as f32, H as f32));

    // seed（与 open_window 的 seed 块同口径）。
    let pa = ui.global::<PanelAnim>();
    pa.set_shape("push".into());
    pa.set_ms(240);
    pa.set_easing("smooth".into());
    pa.set_reduced(false);
    pa.set_reduced_follow(true);
    pa.set_ready(true);

    // 01 设置弹框「侧栏」页。
    ui.set_ifd_page("sidebar".into());
    ui.set_interface_open(true);
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.show().unwrap();
    grab(&ui, &mut buf, "output/panel-anim/01-settings.png");
    ui.set_interface_open(false);

    // 02 两栏展开终态。
    advance(&ui, 600);
    grab(&ui, &mut buf, "output/panel-anim/02-both-open.png");

    // 04 推挤收起中间帧：先抓起点附近，再等收敛。
    ui.set_right_sidebar_collapsed(true);
    grab(&ui, &mut buf, "output/panel-anim/04-push-collapse-start.png");
    advance(&ui, 120);
    grab(&ui, &mut buf, "output/panel-anim/04-push-collapse-mid.png");

    // 03 两栏收起终态（左栏补一枪）。
    ui.set_left_sidebar_collapsed(true);
    advance(&ui, 600);
    grab(&ui, &mut buf, "output/panel-anim/03-both-collapsed.png");
}
