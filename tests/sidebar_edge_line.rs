//! (#sb-edge-snap 2026-10-21) 右栏边界线回归：终端底色 → 边界线（1 设备像素）
//! → 右栏底色，三列必须紧挨着，中间不许漏一条面板底色。
//!
//! 用户截图：终端与右栏之间有一条白缝（浅色主题），因为右栏宽度是拖拽来的
//! 浮点值，125% 下边界落在半个设备像素上，而边界线的 snap 又把线推到右栏
//! 内侧 1 设备像素。修法见 app.slint 的 `#sb-edge-snap`。
//!
//! 运行：`cargo test --test sidebar_edge_line`

use mterm::ui::{AppWindow, PaneInfo, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{WindowAdapter, WindowEvent};
use slint::{Brush, ComponentHandle, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const LW: f32 = 1240.0;
const LH: f32 = 800.0;
const DPR: f32 = 1.25;
const W: usize = 1550;
const H: usize = 1000;
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

struct Frame {
    buf: Vec<PremultipliedRgbaColor>,
}
impl Frame {
    fn new() -> Self {
        Self { buf: vec![PremultipliedRgbaColor::default(); W * H] }
    }
    fn grab(&mut self, ui: &AppWindow) {
        ui.window().request_redraw();
        WIN.with(|win| win.draw_if_needed(|r| { r.render(&mut self.buf, W); }));
    }
    fn rgb(&self, x: usize, y: usize) -> Rgb {
        let p = self.buf[y * W + x];
        if p.alpha == 0 {
            return (0, 0, 0);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue))
    }
}

fn solid(b: &Brush) -> Rgb {
    match b {
        Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => (0, 0, 0),
    }
}

fn dist(a: Rgb, b: Rgb) -> i32 {
    (a.0 as i32 - b.0 as i32).abs()
        + (a.1 as i32 - b.1 as i32).abs()
        + (a.2 as i32 - b.2 as i32).abs()
}

fn setup(width: f32) -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: DPR });
    ui.window().set_size(LogicalSize::new(LW, LH));
    ui.global::<Theme>().set_device_pixel_ratio(DPR);
    ui.set_left_sidebar_collapsed(true);
    ui.set_right_sidebar_width(width);

    let term = TerminalState { id: "p".into(), status: "probe".into(), ..Default::default() };
    ui.set_terminals(ModelRc::from(Rc::new(VecModel::from(vec![term]))));
    let tab = TabInfo { id: "p".into(), ..Default::default() };
    let pane = PaneInfo {
        id: 0,
        x: 0.0,
        y: 0.0,
        w: LW - width,
        h: LH - 60.0,
        active_id: "p".into(),
        focused: true,
        reserve_right: 0.0,
        tabs: ModelRc::from(Rc::new(VecModel::from(vec![tab]))),
    };
    ui.set_panes(ModelRc::from(Rc::new(VecModel::from(vec![pane]))));
    ui.set_active_tab_id("p".into());
    ui.show().unwrap();
    ui
}

#[test]
fn splitter_line_sits_on_the_boundary() {
    // 220 = 默认值；520 = 拖拽上限（用户当前）；519.2 / 400.4 = 会落在半个设备
    // 像素上的浮点宽度（旧实现就是在这里漏白缝）。
    for width in [220.0f32, 420.0, 520.0, 519.2, 400.4] {
        let ui = setup(width);
        std::thread::sleep(std::time::Duration::from_millis(120));
        let mut f = Frame::new();
        f.grab(&ui);
        let theme = ui.global::<Theme>();
        let term_bg = solid(&theme.get_term_bg());
        let line = solid(&theme.get_border_subtle());

        // 不预测落在哪一列（浮点 round 的边界不好复现），直接找那条线：
        // 1 设备像素宽、左边终端底色、右边面板底色，且中线 ±3 列内只有它一条。
        let y = 500;
        let mut hits: Vec<usize> = vec![];
        for x in ((LW - width) * DPR).round() as usize - 6..((LW - width) * DPR).round() as usize + 8 {
            if dist(f.rgb(x, y), line) <= 10 {
                hits.push(x);
            }
        }
        assert_eq!(
            hits.len(),
            1,
            "width={width}：边界处应恰好 1 列边界线，实得 {hits:?}（0 = 线被底色盖住，2 = 线跨了两个设备像素）"
        );
        let x = hits[0];
        assert!(
            dist(f.rgb(x - 1, y), term_bg) <= 6,
            "width={width}：线左列 {} 应为终端底色 {term_bg:?}，实测 {:?}",
            x - 1,
            f.rgb(x - 1, y)
        );
        if std::env::var_os("PROBE_DUMP_DIR").is_some() {
            // 边界两侧各 20 列、纵向 120 行，供人工比对（P6）
            let (x0, y0) = (x.saturating_sub(20), 400usize);
            let (cw, chh) = (40usize, 120usize);
            let mut data = Vec::with_capacity(cw * chh * 3);
            for yy in y0..y0 + chh {
                for xx in x0..x0 + cw {
                    let c = f.rgb(xx, yy);
                    data.extend_from_slice(&[c.0, c.1, c.2]);
                }
            }
            let dir = std::env::var("PROBE_DUMP_DIR").unwrap();
            let path = std::path::Path::new(&dir).join(format!("edge-{width}.ppm"));
            let header = format!("P6\n{cw} {chh}\n255\n");
            let _ = std::fs::write(&path, [header.as_bytes(), &data].concat());
            eprintln!("dumped {}", path.display());
        }
        let right = f.rgb(x + 1, y);
        assert!(
            dist(right, line) > 10 && dist(right, term_bg) > 10,
            "width={width}：线右列 {} 应为右栏底色（不是线、也不是终端底色），实测 {right:?}",
            x + 1
        );
    }
}
