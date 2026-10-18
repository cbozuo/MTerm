//! (#win-edge-frame-r7 2026-10-18) 主窗口最外圈描边的几何回归 —— 离屏像素级。
//!
//! 背景：真机实测（200% DPI）发现旧实现有两处错：
//!   ① `radius-win = 8px / device-pixel-ratio` 把 DWM 圆角当成"固定 8 物理像素"，
//!      实际是 8 **DIP**（200% 下 16 物理像素）→ 自绘角弧整段落进裁剪区外被裁掉；
//!   ② Slint `Path` 渲染前会 fit 进 (元素尺寸 − stroke-width) 并平移 stroke/2，
//!      旧注释按"1 单位 = 1 物理像素"写坐标 → 弧再缩进约一个 stroke。
//! 修法见 `ui/window_frame.slint`：直边贴真实边界、四角弧用反 fit viewbox 让外缘
//! 压在 8 DIP 裁剪弧上。本测试锁定离屏可测的部分：
//!   * `radius-win` 不再被 DPR 除（= 8 逻辑 px，DPR=2 时仍是 8 而不是 4）；
//!   * 四条边最外像素 = 描边色、厚 1 设备像素（DPR=1 下 1 逻辑 px）；
//!   * 角弧墨迹的加权半径落在裁剪弧附近（≈ radius-win − 0.5）——旧实现
//!     （圆心 7.5、中心线 5.69、外缘 6.19）会明显偏内；
//!   * `win-rounded=false` 时四角变直角（Win10 形态）。
//!
//! ⚠️ 离屏渲染器没有 DWM 圆角裁剪，所以"弧与系统裁剪边重合"只能真机验证
//! （见 docs/window-edge-frame-plan-2026-10-18.md §4）。本测试锁的是几何位置。
//!
//! 运行：`cargo test --test window_frame_geometry`（DPR=1；MinimalSoftwareWindow
//! 的 scale_factor 恒为 1）。设 `WF_DUMP=1` + `--nocapture` 可打印四角覆盖率图。

use mterm::ui::{AppWindow, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::WindowAdapter;
use slint::{Brush, ComponentHandle, LogicalSize};
use std::rc::Rc;

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
        _ => panic!("border-strong 必须是纯色 token"),
    }
}

/// 估算像素的"描边覆盖率" α：以角落内侧的内容色为 0、描边色为 1。
fn coverage(px: (u8, u8, u8), bg: (u8, u8, u8), line: (u8, u8, u8)) -> f32 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for (p, b, l) in [(px.0, bg.0, line.0), (px.1, bg.1, line.1), (px.2, bg.2, line.2)] {
        let den = l as f32 - b as f32;
        if den.abs() > 8.0 {
            sum += (p as f32 - b as f32) / den;
            n += 1.0;
        }
    }
    if n == 0.0 {
        0.0
    } else {
        sum / n
    }
}

/// 角落像素坐标（窗口左上角、右下角等）：`(ox, oy)` 是角的那个像素。
/// 返回：该角 `radius-win` 环带邻域内的 (α 加权平均半径, 最大 α, Σα)。
/// 半径按"角局部坐标"算，圆心固定在 `(r, r)`（与四个角的几何镜像一致）。
fn corner_ink(
    f: &Frame,
    r: f32,
    line: (u8, u8, u8),
    ox: usize,
    oy: usize,
) -> (f32, f32, f32) {
    let mirror_x = ox == 0;
    let mirror_y = oy == 0;
    // 角落内侧的内容色作为 bg（取 r+8 处，避开描边本身）。
    let bxi = (r as usize) + 8;
    let gx = if mirror_x { bxi } else { W - 1 - bxi };
    let gy = if mirror_y { bxi } else { H - 1 - bxi };
    let bg = f.px(gx, gy);

    let limit = (r.ceil() as usize) + 6;
    let (mut sum_r, mut sum_a, mut max_a) = (0.0f32, 0.0f32, 0.0f32);
    for ly_i in 0..limit {
        for lx_i in 0..limit {
            let gx = if mirror_x { ox + lx_i } else { ox - lx_i };
            let gy = if mirror_y { oy + ly_i } else { oy - ly_i };
            let lx = lx_i as f32 + 0.5;
            let ly = ly_i as f32 + 0.5;
            let (dx, dy) = (lx - r, ly - r);
            let rad = (dx * dx + dy * dy).sqrt();
            // 只统计环带邻域，避开角落附近的其它 UI 元素（比如左上角品牌徽章）。
            if rad < r - 2.5 || rad > r + 1.0 {
                continue;
            }
            let a = coverage(f.px(gx, gy), bg, line).clamp(0.0, 1.2);
            if a > 0.05 {
                sum_r += rad * a;
                sum_a += a;
                max_a = max_a.max(a);
            }
        }
    }
    (if sum_a > 0.0 { sum_r / sum_a } else { 0.0 }, max_a, sum_a)
}

#[test]
fn window_edge_frame_geometry() {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    // AppWindow min-width 720：取 800×600，四边都落在像素网格上。
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();

    // ---- ① radius-win 是 8 DIP，不随 DPR 缩放（旧实现 DPR=2 时是 4） ----
    let theme = ui.global::<Theme>();
    let r_at_1 = theme.get_radius_win();
    assert!((r_at_1 - 8.0).abs() < 0.01, "DPR=1 时 radius-win 应为 8 逻辑 px，实得 {r_at_1}");
    theme.set_device_pixel_ratio(2.0);
    let r_at_2 = theme.get_radius_win();
    assert!(
        (r_at_2 - 8.0).abs() < 0.01,
        "DPR=2 时 radius-win 仍应为 8 逻辑 px（=16 物理像素的 DWM 裁剪弧），实得 {r_at_2}（旧实现为 4）"
    );
    theme.set_device_pixel_ratio(1.0);

    // ---- 渲染一帧（DPR=1） ----
    let mut f = Frame::new();
    f.grab(&ui);
    let line = solid(theme.get_border_strong());
    let r = theme.get_radius_win();

    // 调试导出（WF_DUMP=1 + --nocapture）：打印四角覆盖率分类图。
    if std::env::var_os("WF_DUMP").is_some() {
        for (name, ox, oy) in [
            ("TL", 0usize, 0usize),
            ("TR", W - 1, 0usize),
            ("BL", 0usize, H - 1),
            ("BR", W - 1, H - 1),
        ] {
            println!("--- {name} corner ---");
            for ly in 0..20usize {
                let mut row = String::new();
                for lx in 0..20usize {
                    let gx = if ox == 0 { lx } else { W - 1 - lx };
                    let gy = if oy == 0 { ly } else { H - 1 - ly };
                    let px = f.px(gx, gy);
                    let bxi = 24usize;
                    let bx = if ox == 0 { bxi } else { W - 1 - bxi };
                    let by = if oy == 0 { bxi } else { H - 1 - bxi };
                    let bg = f.px(bx, by);
                    let a = coverage(px, bg, line);
                    row.push(if a >= 0.8 {
                        '#'
                    } else if a >= 0.45 {
                        '+'
                    } else if a >= 0.2 {
                        '-'
                    } else {
                        '.'
                    });
                }
                println!("{ly:02} {row}");
            }
        }
    }

    // ---- ② 四条直边：最外像素 = 线色；第 2 像素 != 线色（厚 1 设备像素） ----
    let mid = W / 2;
    for (x, y, name) in [
        (mid, 0usize, "上边"),
        (mid, H - 1, "下边"),
        (0, H / 2, "左边"),
        (W - 1, H / 2, "右边"),
    ] {
        assert_eq!(f.px(x, y), line, "{name}的最外像素必须是描边色（线贴在窗口边界上）");
    }
    for (x, y, name) in [
        (mid, 1usize, "上边"),
        (mid, H - 2, "下边"),
        (1, H / 2, "左边"),
        (W - 2, H / 2, "右边"),
    ] {
        assert_ne!(f.px(x, y), line, "{name}第 2 个像素不该是描边色（1 设备像素，不能更粗）");
    }

    // ---- ③ 四角：弧墨迹必须落在裁剪弧附近 ----
    // 期望：描边带径向 [r−1, r]，α 加权平均半径 ≈ r−0.5。
    // 旧实现（圆心 7.5、中心线 5.69、外缘 6.19）→ 加权半径 ≈5.7，直接红。
    let bl = (0usize, H - 1);
    let (r_bl, max_bl, ink_bl) = corner_ink(&f, r, line, bl.0, bl.1);
    assert!(ink_bl > 2.0 && max_bl > 0.45, "左下角弧没画出来（ink={ink_bl:.2}, max={max_bl:.2}）");
    assert!(
        r_bl > r - 1.4 && r_bl < r - 0.05,
        "左下角弧半径不对: 加权 r={r_bl:.2}（应 ≈{:.1}；旧实现 ≈5.7）",
        r - 0.5
    );
    // 其余三角也必须有弧墨迹（内容色不同，每角单独取 bg）。
    for (name, ox, oy) in [
        ("左上", 0usize, 0usize),
        ("右上", W - 1, 0usize),
        ("右下", W - 1, H - 1),
    ] {
        let (_, max_a, ink) = corner_ink(&f, r, line, ox, oy);
        assert!(ink > 2.0 && max_a > 0.35, "{name}角弧没画出来（ink={ink:.2}, max={max_a:.2}）");
    }

    // ---- ④ 直角模式（Win10 / 圆角属性失败）：四角必须是描边色 ----
    ui.set_win_rounded(false);
    f.grab(&ui);
    for (x, y, name) in [
        (0usize, 0usize, "左上"),
        (W - 1, 0, "右上"),
        (0, H - 1, "左下"),
        (W - 1, H - 1, "右下"),
    ] {
        assert_eq!(f.px(x, y), line, "直角模式{name}角落必须是描边色");
    }
}
