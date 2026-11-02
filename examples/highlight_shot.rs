//! (#hl-pal-parity / #hl-pal-curfix / #hl-add-row 2026-11-02) 高亮设置弹框
//! 本轮四点修订的离屏渲染出图 —— 与强调色取色弹框逐项比对的证据图。
//!
//! 运行:`cargo run --example highlight_shot`(example = bin 类链接,绕开
//! test 目标在这台机器上的 dlltool 0xc0000409 环境病)。
//! 输出:output/highlight/{01-form,02-palette,03-empty}.png
//!
//!   01 表单区:颜色行 + 添加钮合并同行、添加钮底色跟随选定色(#E1306C)
//!   02 色板卡:副标题三态(警示行)、最近行、47 色格、合并钮、页脚三键
//!   03 空态:默认 #1FC8B4 下表单区(添加钮为强调青)
//!
//! TouchArea 命中区(0 宽真凶)不在像素里,由 02 图的存在性证明:合并钮
//! 绝对定位后 cur-ta 铺满整卡,离屏图能看到「选定/调色盘」两段完整绘制。

use mterm::ui::{HighlightDialog, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::fs;

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

const W: usize = 760;
const H: usize = 780;

fn grab(ui: &HighlightDialog, buf: &mut Vec<PremultipliedRgbaColor>, path: &str) {
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(buf, W);
        });
    });
    // 预乘 → RGBA PNG
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
    fs::create_dir_all("output/highlight").ok();
    img.save(path).expect("save png");
    println!("saved {path}");
}

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = HighlightDialog::new().expect("build highlight dialog");
    ui.window().set_size(slint::LogicalSize::new(W as f32, H as f32));

    // 空模型:弹框只渲染 chrome + 自定义页表单,不依赖 Rust 业务层
    ui.set_presets(ModelRc::new(VecModel::default()));
    ui.set_customs(ModelRc::new(VecModel::default()));
    ui.set_bounds_w(slint::LogicalLength::new(W as f32).into());
    ui.set_bounds_h(slint::LogicalLength::new(H as f32).into());
    ui.set_enabled_count(0);
    ui.set_status(SharedString::from(""));

    // 03 空态(默认青色表单)
    ui.set_tab(1);
    grab(&ui, &mut vec![PremultipliedRgbaColor::default(); W * H], "output/highlight/03-empty.png");

    // 01 表单区:选定洋红 #E1306C(用户截图同款) → 添加钮底色应为洋红
    ui.set_new_color(SharedString::from("#E1306C"));
    grab(&ui, &mut vec![PremultipliedRgbaColor::default(); W * H], "output/highlight/01-form.png");

    // 02 色板卡:开卡 + 洋红已选定(警示行?洋红对深底 5.3:1 达标,走"已选未生效"行)
    ui.set_pal_base(SharedString::from("#1FC8B4"));
    ui.set_pal_open(true);
    grab(&ui, &mut vec![PremultipliedRgbaColor::default(); W * H], "output/highlight/02-palette.png");

    let _ = Theme::default();
}
