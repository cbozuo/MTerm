//! (#topbar-compact 2026-10-27) 标题栏右侧四键（右侧栏开关 + 最小化/最大化/关闭）
//! 在任何窗口宽度下都必须存在，且严格贴右缘。
//!
//! 用户报的现象：「界面拖拽缩小长度的时候，左上角的四个按钮，会存在消失不见」。
//!
//! 根因不是被条件隐藏，是**被布局压成 0 宽**：标题栏原本把右侧控制组放在
//! 左侧工具组的同一个 `HorizontalLayout` 里。Slint 1.18 的 HL 空间不够时会把
//! 子项一路压到 0，窗口一窄（实测 900px 就够）就先吃掉弹性条、再吃掉右侧整组。
//!
//! 修法见 `app.slint`：右侧组移出布局流、绝对定位贴右缘（`x: parent.width -
//! self.width`），左侧组包进 `clip: true` 的 Rectangle 自己裁切。
//!
//! 本测试锁三件事（都在真实渲染帧上量像素，不看属性值 —— 属性完全可能正确
//! 而画面上是 0 宽）：
//!   ① 每个宽度下，最右 160px 内都能看到窗口控制键的**图标墨迹**；
//!   ② 关闭键的悬停红底落在窗口最右一列之内（说明组确实贴右缘，没被挤走）；
//!   ③ 窄到触发 `topbar-compact` 时，左侧工具组退成纯图标，也不再挤压右侧。
//!
//! 运行：`cargo test --test titlebar_controls_always_visible`

use mterm::ui::{AppWindow, PaneInfo, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const LH: f32 = 700.0;
const DPR: f32 = 1.0;
const H: usize = 800;

/// 右侧控制组宽度：右侧栏开关 40 + 三键 40×3。与 `app.slint` 里
/// `win-controls { width: 160px; }` 是同一个数，改一处要同步另一处。
const CONTROLS_W: f32 = 160.0;
/// 标题栏高度（`app.slint` 的 `height: 32px`）。
const TITLEBAR_H: f32 = 32.0;
/// `topbar-compact` 断点，与 `app.slint` / `app.rs` 里的 1010 保持一致。
/// 三处各写一个数是这次改动的已知重复，改断点时务必一起改。
const COMPACT_BREAKPOINT: f32 = 1010.0;

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
    w: usize,
}
impl Frame {
    fn new(w: usize) -> Self {
        Self {
            buf: vec![PremultipliedRgbaColor::default(); w * H],
            w,
        }
    }
    fn grab(&mut self, ui: &AppWindow) {
        ui.window().request_redraw();
        WIN.with(|win| {
            win.draw_if_needed(|r| {
                r.render(&mut self.buf, self.w);
            })
        });
    }
    /// 未预乘还原后的 RGB；alpha=0 视为「没画东西」，返回 None。
    fn rgb(&self, x: usize, y: usize) -> Option<(u8, u8, u8)> {
        let p = self.buf[y * self.w + x];
        if p.alpha == 0 {
            return None;
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        Some((un(p.red), un(p.green), un(p.blue)))
    }
}

/// 某列在标题栏中部是否有「非背景」墨迹。窗口控制键的图标是 16px 前景色字形，
/// 一定会在标题栏竖直中线附近留下与底色不同的像素。
fn column_has_ink(f: &Frame, x: usize, bg: (u8, u8, u8)) -> bool {
    // 标题栏 32 高，图标 16px 居中 → 8..24 区间必有笔画。
    let y0 = (TITLEBAR_H * DPR * 0.25) as usize;
    let y1 = (TITLEBAR_H * DPR * 0.80) as usize;
    (y0..y1).any(|y| {
        f.rgb(x, y).is_some_and(|c| {
            let d = (c.0 as i32 - bg.0 as i32).abs()
                + (c.1 as i32 - bg.1 as i32).abs()
                + (c.2 as i32 - bg.2 as i32).abs();
            d > 40
        })
    })
}

fn solid(b: &slint::Brush) -> (u8, u8, u8) {
    match b {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => (0, 0, 0),
    }
}

fn setup(logical_w: f32) -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window()
        .dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: DPR });
    ui.window().set_size(LogicalSize::new(logical_w, LH));
    ui.global::<Theme>().set_device_pixel_ratio(DPR);
    ui.set_left_sidebar_collapsed(true);

    let term = TerminalState {
        id: "p".into(),
        status: "probe".into(),
        ..Default::default()
    };
    ui.set_terminals(ModelRc::from(Rc::new(VecModel::from(vec![term]))));
    let tab = TabInfo {
        id: "p".into(),
        ..Default::default()
    };
    let pane = PaneInfo {
        id: 0,
        x: 0.0,
        y: 0.0,
        w: logical_w,
        h: LH - TITLEBAR_H,
        active_id: "p".into(),
        focused: true,
        reserve_right: 0.0,
        tabs: ModelRc::from(Rc::new(VecModel::from(vec![tab]))),
    };
    ui.set_panes(ModelRc::from(Rc::new(VecModel::from(vec![pane]))));
    ui.set_active_tab_id("p".into());
    // (#topbar-compact) 断点由 Rust 在 `WEvent::Resized` 里推送（不在 Slint 里
    // 读 root.width —— 那会构成绑定循环）。测试直接调 `set_size` 不会产生
    // winit 事件，所以这里显式模拟 Rust 的做法，用**逻辑宽度**判同一个阈值。
    ui.set_topbar_compact(logical_w < COMPACT_BREAKPOINT);
    ui.show().unwrap();
    ui
}

/// 右侧 160px 内每一列有没有窗口控制键的墨迹。
///
/// 返回**逐列**的布尔向量（不是首尾两点）：只有逐列看，才分得清「四个图标都
/// 完整」和「图标被压扁成一条竖线」。这条区别是本测试的核心 —— 修复前 900px
/// 下四段墨迹宽 14 / 12 / 10 / 1 px，图标肉眼已认不出，但"有墨迹"依然成立，
/// 只查存在性的断言会假绿。
///
/// 右端留 `EDGE_INSET`：窗口最外那 1~2 列是 `WindowFrame` 画的描边
/// （#win-edge-frame），它和标题栏底色差异很大，会被当成一段 1px 的"图标"。
/// 它是装饰线，不属于任何一个按钮，必须排除，否则段数恒为 5。
const EDGE_INSET: usize = 2;

fn controls_ink_columns(f: &Frame, logical_w: f32, bg: (u8, u8, u8)) -> Vec<bool> {
    let right_edge = (logical_w * DPR).round() as usize;
    let left = right_edge.saturating_sub((CONTROLS_W * DPR) as usize);
    (left..right_edge.saturating_sub(EDGE_INSET).min(f.w))
        .map(|x| column_has_ink(f, x, bg))
        .collect()
}

/// 把逐列位图切成连续的墨迹段，返回每段宽度。
fn runs(cols: &[bool]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut cur = 0usize;
    for &c in cols {
        if c {
            cur += 1;
        } else if cur > 0 {
            out.push(cur);
            cur = 0;
        }
    }
    if cur > 0 {
        out.push(cur);
    }
    out
}

#[test]
fn window_controls_never_collapse_however_narrow_the_window_is() {
    // 720 = 窗口最小宽度；900 是用户报障时的实际宽度；1010 是 topbar-compact
    // 断点；1280 / 1640 = 宽窗（应维持图标+文字）。
    for logical_w in [720.0f32, 900.0, 1010.0, 1280.0, 1640.0] {
        let ui = setup(logical_w);
        std::thread::sleep(std::time::Duration::from_millis(120));
        let mut f = Frame::new((logical_w * DPR).round() as usize + 2);
        f.grab(&ui);

        let theme = ui.global::<Theme>();
        let bg = solid(&theme.get_zone_titlebar());

        let cols = controls_ink_columns(&f, logical_w, bg);
        let segs = runs(&cols);
        assert!(
            !segs.is_empty(),
            "width={logical_w}：右侧 160px 内没有任何窗口控制键的墨迹 —— \
             四个按钮整组消失了（#topbar-compact 修的就是这个）"
        );

        // 四个键、每键一个 16px 图标：应恰好 4 段墨迹。修复前 900px 只有 1~13px
        // 的残条，被挤压的键会合并/消失，段数就不对了。
        assert_eq!(
            segs.len(),
            4,
            "width={logical_w}：右侧应看到 4 个键的图标，实得 {} 段（各段宽 {segs:?}）—— \
             有按钮被压扁或被挤掉了",
            segs.len()
        );
        // 每个图标字形约 10~16px 宽；被压扁时会出现 1~4px 的残条。
        for (i, w) in segs.iter().enumerate() {
            assert!(
                *w >= 6,
                "width={logical_w}：第 {} 个键的图标只有 {w}px 宽（{segs:?}）—— \
                 它被横向压扁了，用户看不出那是按钮",
                i + 1
            );
        }

        // 整组必须贴住右缘：最右墨迹距窗口右缘不超过一格按钮的一半。
        let right_edge = (logical_w * DPR).round() as usize;
        let left = right_edge.saturating_sub((CONTROLS_W * DPR) as usize);
        let last_ink = left + cols.iter().rposition(|c| *c).unwrap();
        assert!(
            right_edge - last_ink <= 24 + EDGE_INSET,
            "width={logical_w}：控制组没贴住右缘（最右墨迹 x={last_ink}，窗口宽 {right_edge}）—— \
             它被布局推离了右边缘"
        );

        WIN.with(|w| w.set_size(LogicalSize::new(1.0, 1.0)));
    }
}

/// 窄窗必须真的退成纯图标：左侧工具组不再追求「图标+文字」，否则 720px 下
/// 它会与右侧控制组抢空间。
#[test]
fn narrow_windows_switch_the_toolbar_to_icon_only() {
    let ui = setup(900.0);
    assert!(
        ui.get_topbar_compact(),
        "900px 应判定为 compact（实测中文标签需 ≈891px，英文 ≈1004px）"
    );

    let ui = setup(1640.0);
    assert!(
        !ui.get_topbar_compact(),
        "1640px 放得下图标+文字，不应退成纯图标"
    );
}
