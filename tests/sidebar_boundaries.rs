//! (#line-dedup 2026-10-18) 主界面「区域边界线」的像素回归。
//!
//! 用户反馈（两张截图、6 个红圈）：
//!   ① 「不同区域重叠时候的线条加粗」——左右侧栏边界、标题栏底线在侧栏上方
//!      都变成 2 设备像素；
//!   ② 「鼠标悬浮在可以扩宽的左右两边的边界线的时候，需要强调高亮」。
//!
//! 根因（真机 + 代码核对）：
//!   * 左栏容器里还画着一条 `1px`（**逻辑**像素）边线 —— 200% 下 2 物理像素；
//!   * compact 侧栏卡片、右栏根节点各自还有一圈 hairline 边框，与标题栏底线 /
//!     splitter 可见线 / 窗口描边叠在一起 → 相邻两行/列各一条线 = 2 设备像素；
//!   * splitter 的可见线原来"在 4px 命中区里居中"，分数缩放下会落在半设备像素上，
//!     被摊成两个半透明像素（看起来又粗又虚）。
//!
//! 修法：边界线全屏只保留一条 —— 由 splitter 的可见线承担（位置贴设备网格、
//! 对齐侧栏真实边缘、厚 1 设备像素、hover/拖拽转 accent）；侧栏自身边框/容器
//! 边线删除。本测试锁：两条侧栏边界各 1 像素、标题栏底线在侧栏上方仍 1 像素、
//! hover 变 accent 且不改变粗细。
//!
//! ⚠️ 离屏 DPR=1；200%/femtovg 由真机截图脚本验证（target/edge-lines/）。
//!
//! 运行：`cargo test --test sidebar_boundaries`

use mterm::ui::{AppWindow, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{WindowAdapter, WindowEvent};
use slint::{Brush, ComponentHandle, LogicalPosition, LogicalSize};
use std::rc::Rc;
use std::time::Duration;

const W: usize = 800;
const H: usize = 600;

thread_local! {
    static WIN: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}

struct ProbePlatform;

impl slint::platform::Platform for ProbePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(WIN.with(|w| w.clone()))
    }
}

struct Frame {
    buf: Vec<PremultipliedRgbaColor>,
}

impl Frame {
    fn new() -> Self {
        Self { buf: vec![PremultipliedRgbaColor::default(); W * H] }
    }

    fn grab(&mut self, ui: &AppWindow) {
        ui.window().request_redraw();
        WIN.with(|win| {
            win.draw_if_needed(|r| {
                r.render(&mut self.buf, W);
            });
        });
    }

    fn px(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let p = self.buf[y * W + x];
        if p.alpha == 0 {
            return (0, 0, 0);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue))
    }
}

fn solid(brush: Brush) -> (u8, u8, u8) {
    match brush {
        Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("token 必须是纯色"),
    }
}

/// 把指针移到 (x, y)，等过动画时长后再补一次移动事件推进 Slint 动画
/// （离屏探针的已证实行为：只 sleep 不推进 animate）。
fn hover(ui: &AppWindow, x: f32, y: f32) {
    let p = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    std::thread::sleep(Duration::from_millis(170));
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
}

#[test]
fn zone_boundaries_are_single_device_px_and_highlight_on_hover() {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    // 右栏默认 dock="left"（Rust 启动时才从配置改），测试里显式放右边。
    ui.set_right_sidebar_dock("right".into());
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();

    let theme = ui.global::<Theme>();
    let line = solid(theme.get_border_subtle());
    let accent = theme.get_accent_solid();
    let accent = (accent.red(), accent.green(), accent.blue());
    // DPR=1：hairline = 1 逻辑 px；左栏 240 / 右栏 220（AppWindow 默认值）。
    let left_w = ui.get_left_sidebar_width() as usize;
    let right_w = ui.get_right_sidebar_width() as usize;
    let left_line_x = left_w - 1; // dock=left：边界内侧 1 设备像素
    let right_line_x = W - right_w; // dock=right：面板第一设备像素
    let mid_y = H / 2;
    let titlebar_line_y = 31; // 标题栏 32 逻辑 px，底线在最后一设备行

    let mut f = Frame::new();
    f.grab(&ui);

    // ---- ① 两条区域边界线：恰好 1 设备像素 ----
    for (name, x, y) in [
        ("左栏边界", left_line_x, mid_y),
        ("右栏边界", right_line_x, mid_y),
    ] {
        assert_eq!(f.px(x, y), line, "{name}：边界线上必须是 border-subtle");
        assert_ne!(f.px(x - 1, y), line, "{name}：左侧邻像素不该是同一条线（>1 设备像素）");
        assert_ne!(f.px(x + 1, y), line, "{name}：右侧邻像素不该是同一条线（>1 设备像素）");
    }

    // ---- ② 标题栏底线：中间与左右侧栏上方都只有 1 设备像素 ----
    for x in [120usize, W / 2, W - 120] {
        assert_eq!(f.px(x, titlebar_line_y), line, "标题栏底线缺失 @x={x}");
        assert_ne!(f.px(x, titlebar_line_y - 1), line, "标题栏底线上方多了第二像素 @x={x}");
        assert_ne!(f.px(x, titlebar_line_y + 1), line, "标题栏底线下方多了第二像素 @x={x}");
    }

    // ---- ③ hover：左边界线转 accent，且仍是 1 设备像素 ----
    hover(&ui, left_w as f32, mid_y as f32);
    f.grab(&ui);
    assert_eq!(
        f.px(left_line_x, mid_y),
        accent,
        "hover 左边界后线没有变 accent（强调高亮失效）"
    );
    assert_ne!(f.px(left_line_x - 1, mid_y), accent, "hover 后左边界变粗了");
    assert_ne!(f.px(left_line_x + 1, mid_y), accent, "hover 后左边界变粗了");

    // 移开后恢复 border-subtle
    hover(&ui, 100.0, mid_y as f32);
    f.grab(&ui);
    assert_eq!(f.px(left_line_x, mid_y), line, "移开后左边界没有恢复 border-subtle");

    // ---- ④ hover：右边界线同样转 accent、不变粗 ----
    hover(&ui, (W - right_w) as f32, mid_y as f32);
    f.grab(&ui);
    assert_eq!(
        f.px(right_line_x, mid_y),
        accent,
        "hover 右边界后线没有变 accent（强调高亮失效）"
    );
    assert_ne!(f.px(right_line_x - 1, mid_y), accent, "hover 后右边界变粗了");
    assert_ne!(f.px(right_line_x + 1, mid_y), accent, "hover 后右边界变粗了");

    // 指针离开窗口后恢复静态线
    hover(&ui, -10.0, mid_y as f32);
    f.grab(&ui);
    assert_eq!(f.px(right_line_x, mid_y), line, "移开后右边界没有恢复 border-subtle");
}
