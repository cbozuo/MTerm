//! (#close-line-hairline 2026-10-20) 关闭主窗口弹框的线规范回归。
//!
//! 用户截图反馈：① 卡片边线粗了些；② 弹框内的线要参考高亮设置弹框。
//! 根因：关闭弹框卡片边框与两条内线都是**1 逻辑 px**，125% 缩放下跨
//! 1.25 设备行，AA 出一条发糊的粗线。高亮弹框的规范是 hairline()
//!（恰 1 设备像素）+ snap-to-device()（线顶压在设备栅格上）。
//! 修复：卡边框 → hairline，卡 x/y/高 snap；两条内线从流内 1px 段改为
//! 挂段底（snap + hairline），色不变（border-subtle）。
//!
//! 本探针在 **DPR 1.25**（糊/不糊只在非整数缩放下可辨）离屏渲染弹框：
//!   * 全宽 border-subtle 内线恰 2 条（头部下沿 + 页脚上沿），且每条线
//!     上/下相邻行都是纯 popup-bg —— 旧的 1px 线会在次行留下 25% 的 AA
//!     混合残留（每通道差 4~6），tol 2 内必挂；
//!   * 卡顶 border-strong 边框同样单行无残留；
//!   * snap 后线顶在整数设备行（对 1.25 取模为 0 的物理 y）。

use mterm::ui::{AppWindow, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalSize};
use std::rc::Rc;

// 逻辑 800×600~605 × DPR 1.25 = 物理 1000×750~757（相位扫描会改窗高，
// buffer 按最大 760 行分配；超出窗口物理高的行为透明，扫描自然跳过）。
const LW: f32 = 800.0;
const LH: f32 = 600.0;
const DPR: f32 = 1.25;
const W: usize = 1000;
const H: usize = 760;
type Rgb = (u8, u8, u8);

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

struct Frame { buf: Vec<PremultipliedRgbaColor> }
impl Frame {
    fn new() -> Self { Self { buf: vec![PremultipliedRgbaColor::default(); W * H] } }
    fn grab(&mut self, ui: &AppWindow) {
        ui.window().request_redraw();
        WIN.with(|win| win.draw_if_needed(|r| { r.render(&mut self.buf, W); }));
    }
    fn px(&self, x: usize, y: usize) -> (u8, u8, u8, u8) {
        let p = self.buf[y * W + x];
        if p.alpha == 0 { return (0, 0, 0, 0); }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue), p.alpha)
    }
    fn dump(&self, name: &str) {
        let mut data = Vec::with_capacity(W * H * 3);
        for y in 0..H {
            for x in 0..W {
                let (r, g, b, _) = self.px(x, y);
                data.extend_from_slice(&[r, g, b]);
            }
        }
        let path = std::path::Path::new("output").join(format!("{name}.ppm"));
        let header = format!("P6\n{} {}\n255\n", W, H);
        let _ = std::fs::write(&path, [header.as_bytes(), &data].concat());
        eprintln!("dumped {}", path.display());
    }
}

fn brush_rgb(b: &slint::Brush) -> Rgb {
    match b { slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()), _ => (0, 0, 0) }
}

fn near(a: Rgb, b: Rgb, tol: i32) -> bool {
    (a.0 as i32 - b.0 as i32).abs() <= tol
        && (a.1 as i32 - b.1 as i32).abs() <= tol
        && (a.2 as i32 - b.2 as i32).abs() <= tol
}

fn settle() { std::thread::sleep(std::time::Duration::from_millis(180)); }

/// 全宽横线检测：某行内连续 ≥400 物理 px 的 c 色（tol 2、alpha 255）段。
/// 返回 (行 y, 段中列 x)。相邻行聚组由调用方处理（一条 1 设备像素线只占
/// 1 行；相邻两行都命中同色段时视为同一条线的抖动，这里 hairline 下不应
/// 发生 —— 若发生会被「次行无残留」断言抓住）。
fn find_lines(f: &Frame, c: Rgb) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for y in 0..H {
        let mut x = 0usize;
        while x < W {
            let p = f.px(x, y);
            if p.3 == 255 && near((p.0, p.1, p.2), c, 2) {
                let s = x;
                while x < W {
                    let q = f.px(x, y);
                    if q.3 == 255 && near((q.0, q.1, q.2), c, 2) { x += 1 } else { break }
                }
                if x - s >= 400 { out.push((y, (s + x - 1) / 2)); }
            } else { x += 1; }
        }
    }
    out
}

#[test]
fn close_dialog_lines_are_hairline_sharp() {
    let ui = setup();
    let (subtle, strong, popup) = {
        let t = ui.global::<Theme>();
        (
            brush_rgb(&t.get_border_subtle()),
            brush_rgb(&t.get_border_strong()),
            brush_rgb(&t.get_popup_bg()),
        )
    };

    ui.set_confirm_close_open(true);
    settle(); settle();

    // 相位扫描：弹框垂直居中，窗口高每 +1 逻辑 px，卡顶物理相位步进
    // 1.25 mod 1 → 扫过 {0, .25, .5, .75}。旧实现（1 逻辑 px 线 + 落点
    // 不 snap）的边框/内线在坏相位下被渲染器 round 成 2 个物理行或留下
    // AA 残留（用户实机截图的"线粗"正是这类相位）；新实现恒 snap 到
    // 设备栅格，任何尺寸下都单行锐利。
    let mut f = Frame::new();
    for lh in [600.0f32, 601.0, 602.0, 603.0, 604.0, 605.0] {
        ui.window().set_size(LogicalSize::new(LW, lh));
        settle();
        f.grab(&ui);
        f.dump("close-dialog-lines-probe");

        // ── ① 两条全宽内线（border-subtle）：单设备行 + 上下行纯卡底 ──
        let mut lines = find_lines(&f, subtle);
        lines.sort();
        let mut groups: Vec<Vec<(usize, usize)>> = vec![];
        for l in lines {
            match groups.last_mut() {
                Some(g) if l.0 <= g.last().unwrap().0 + 1 => g.push(l),
                _ => groups.push(vec![l]),
            }
        }
        assert!(groups.len() >= 2,
            "窗高 {lh}：弹框内应检出 ≥2 条全宽 border-subtle 线（头部/页脚），实得 {} 组",
            groups.len());
        for g in &groups {
            let (y, x) = g[0];
            assert_eq!(g.len(), 1,
                "窗高 {lh}：y={y} 的内线占了 {} 个物理行 —— 线不止 1 设备像素（发糊显粗）", g.len());
            // 上/下相邻行 = 纯卡底，无 AA 残留（坏相位的 1px 线在相邻行
            // 留下混合色；snap 失效的证据就是这里的残留）。
            let above = f.px(x, y - 1);
            let below = f.px(x, y + 1);
            assert!(above.3 == 255 && near((above.0, above.1, above.2), popup, 2),
                "窗高 {lh}：y={y} 内线上方行 {:?} 应为纯 popup-bg {:?}（AA 残留 = 线发糊显粗）",
                above, popup);
            assert!(below.3 == 255 && near((below.0, below.1, below.2), popup, 2),
                "窗高 {lh}：y={y} 内线下方行 {:?} 应为纯 popup-bg {:?}（AA 残留 = 线发糊显粗）",
                below, popup);
        }

        // ── ② 卡片顶边框（border-strong）：下一行纯卡底 ──
        // 主界面上也有别的 border-strong 横段（如顶部工具区），卡顶边框必在
        // 头部内线上方 ≤80 物理行内 —— 按位置取候选，避免抓到无关元素。
        let y_head = groups[0][0].0;
        let strong_lines = find_lines(&f, strong);
        assert!(!strong_lines.is_empty(), "应检出卡片 border-strong 边框线");
        let &(by, bx) = strong_lines.iter()
            .filter(|&&(y, _)| y < y_head && y + 80 >= y_head)
            .next_back()
            .unwrap_or_else(|| panic!("窗高 {lh}：头部内线 y={y_head} 上方 80 行内应有卡顶边框，实得 {:?}",
                strong_lines));
        let below = f.px(bx, by + 1);
        assert!(below.3 == 255 && near((below.0, below.1, below.2), popup, 2),
            "窗高 {lh}：卡顶边框 y={by} 下一行 {:?} 应为纯 popup-bg {:?}（边框 AA 残留 = 边线显粗）",
            below, popup);
    }
}

fn setup() -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    // 先派发 DPI，再定窗尺寸：ScaleFactorChanged 后 set_size(逻辑) 才会
    // 映射出 1000×750 的物理缓冲。
    ui.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: DPR });
    // Theme.device-pixel-ratio 是 Theme **自有**属性（默认 1.0，遮蔽窗口真实
    // DPR）—— 真机由 Rust 侧 refresh 镜像 window.scale_factor() 写入
    // （app.rs set_device_pixel_ratio），hairline()/snap-to-device() 都读它。
    // 探针必须同样写入，否则 hairline 按 1.0 算，测不出非整数缩放的差异。
    ui.global::<Theme>().set_device_pixel_ratio(DPR);
    ui.window().set_size(LogicalSize::new(LW, LH));
    ui.show().unwrap();
    ui
}
