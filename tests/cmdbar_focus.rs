//! (#cmdbar-focus-2026-10-19) 命令栏输入框：聚焦样式 = 「新建会话」同款 +
//! 聚焦后隐藏占位提示 —— 渲染级回归。
//!
//! 用户要求（附截图）：
//!   ① 「输入框的聚焦样式需要修改下，改成和新建会话同款」—— 新建会话对话框
//!      的输入框是 `1px accent 边 + 3px accent-soft 柔光环`（session_dialog.slint
//!      的 MiniInput / LabeledInput），命令栏此前只有一根 1px accent 边；
//!   ② 「聚焦之后，输入框里面的提示信息需要自动删除掉」—— 占位文字只按
//!      `text == ""` 判断，聚焦后仍压在光标 / IME 预编辑上。
//!
//! 断言（离屏像素，平台搭建同 tests/dialog_scrim_drag.rs）：
//!   * 聚焦后命令栏中线上有**两侧 1px accent 边框**，且常态同一位置不是 accent；
//!   * 边框外 3px 处出现 `mix(栏底色, accent, accent-soft.alpha)` 的光晕，
//!     而 -4px 处回到栏底色（光环只有 3px，不是一条宽边）；
//!   * 占位文字带内的"非底色"像素数：常态 > 200，聚焦后 < 40（只剩光标）。
//!
//! 运行：`cargo test --test cmdbar_focus`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。

use mterm::ui::{AppWindow, PaneInfo, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{Brush, ComponentHandle, LogicalPosition, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 1200;
const H: usize = 400;
/// 终端 pane 的几何（Rust 按 split 树算，探针按真机布局给：左侧栏 240px、
/// 右侧栏 220px，中间是终端区）。命令栏在终端区内，两侧栏会盖住它的左右端，
/// 所以 pane 必须内缩，否则右边框被侧栏压住量不到。
const PANE_X: usize = 0;   // pane.x 相对 split 容器（容器本身在左侧栏之后）
const PANE_W: usize = 700;
const PANE_H: usize = H - 40;

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

    /// 读一个像素，返回到预乘前的直通 RGBA。
    fn px(&self, x: usize, y: usize) -> (u8, u8, u8, u8) {
        let p = self.buf[y * W + x];
        if p.alpha == 0 {
            return (0, 0, 0, 0);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue), p.alpha)
    }

    fn rgb(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let (r, g, b, _) = self.px(x, y);
        (r, g, b)
    }
}

fn brush_solid(brush: &Brush) -> Option<(u8, u8, u8, u8)> {
    match brush {
        Brush::SolidColor(c) => Some((c.red(), c.green(), c.blue(), c.alpha())),
        _ => None,
    }
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: u8) -> bool {
    a.0.abs_diff(b.0) <= tol && a.1.abs_diff(b.1) <= tol && a.2.abs_diff(b.2) <= tol
}

/// base 上按 alpha 叠 over（与 Slint 的源-over 混色一致）。
fn mix(base: (u8, u8, u8), over: (u8, u8, u8), alpha: f32) -> (u8, u8, u8) {
    let f = |b: u8, o: u8| (b as f32 * (1.0 - alpha) + o as f32 * alpha).round() as u8;
    (f(base.0, over.0), f(base.1, over.1), f(base.2, over.2))
}

fn maybe_dump(f: &Frame, name: &str) {
    let Ok(dir) = std::env::var("PROBE_DUMP_DIR") else { return };
    let mut data = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            let (r, g, b, _) = f.px(x, y);
            data.extend_from_slice(&[r, g, b]);
        }
    }
    let path = std::path::Path::new(&dir).join(format!("{}.ppm", name));
    let header = format!("P6\n{} {}\n255\n", W, H);
    std::fs::write(&path, [header.as_bytes(), &data].concat()).ok();
    eprintln!("dumped {}", path.display());
}

/// 一个 pane + 一个终端 tab：命令栏只在 TerminalView 里存在，而 TerminalView
/// 只对有活动的终端 pane 实例化。
fn setup() -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));

    let term = TerminalState { id: "probe-term".into(), ..Default::default() };
    ui.set_terminals(ModelRc::from(Rc::new(VecModel::from(vec![term]))));
    let tab = TabInfo { id: "probe-term".into(), ..Default::default() };
    let pane = PaneInfo {
        id: 0,
        x: PANE_X as f32,
        y: 0.0,
        w: PANE_W as f32,
        h: PANE_H as f32,
        active_id: "probe-term".into(),
        focused: true,
        reserve_right: 0.0,
        tabs: ModelRc::from(Rc::new(VecModel::from(vec![tab]))),
    };
    ui.set_panes(ModelRc::from(Rc::new(VecModel::from(vec![pane]))));
    ui.set_active_tab_id("probe-term".into());

    ui.show().unwrap();
    ui
}

fn click(ui: &AppWindow, x: f32, y: f32) {
    let p = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: p,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
}

/// 命令栏区域里"竖直到顶 accent 线"的列区间（边框跨满框高，字形/按钮内文字
/// 的列高度远小于框高）。
fn vertical_accent_runs(
    f: &Frame,
    accent: (u8, u8, u8),
    y0: usize,
    y1: usize,
) -> Vec<(usize, usize)> {
    let min_h = (y1 - y0) * 3 / 5; // 至少 60% 的行是 accent 才算竖线
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for x in 0..W {
        let hits = (y0..y1).filter(|&y| close(f.rgb(x, y), accent, 8)).count();
        let tall = hits >= min_h;
        match (tall, start) {
            (true, None) => start = Some(x),
            (false, Some(s)) => {
                runs.push((s, x - s));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push((s, W - s));
    }
    runs
}

/// 命令栏里那枚 accent 色 "$" 提示符的包围盒（只在左半幅、命令栏那 34px 内找，
/// 避开右端整块 accent 的「发送」按钮）。
fn dollar_bbox(f: &Frame, accent: (u8, u8, u8)) -> Option<(usize, usize, usize, usize)> {
    let mut min = (usize::MAX, usize::MAX);
    let mut max = (0usize, 0usize);
    let mut found = false;
    for y in (H / 2)..H {
        for x in 0..(W / 2) {
            if close(f.rgb(x, y), accent, 8) {
                found = true;
                min.0 = min.0.min(x);
                min.1 = min.1.min(y);
                max.0 = max.0.max(x);
                max.1 = max.1.max(y);
            }
        }
    }
    found.then_some((min.0, min.1, max.0, max.1))
}

#[test]
fn cmdbar_input_focus_ring_matches_session_dialog_and_hides_hint() {
    let ui = setup();
    let theme = ui.global::<Theme>();
    let accent = brush_solid(&theme.get_accent()).expect("accent 应为纯色");
    let accent = (accent.0, accent.1, accent.2);
    let soft_alpha = {
        let s = brush_solid(&theme.get_accent_soft()).expect("accent-soft 应为纯色");
        s.3 as f32 / 255.0
    };

    let mut rest = Frame::new();
    rest.grab(&ui);
    maybe_dump(&rest, "cmdbar-rest");

    // "$" 提示符定位命令栏中线（不硬编码窗口标题栏高度）。
    let (dx0, dy0, dx1, dy1) = dollar_bbox(&rest, accent)
        .unwrap_or_else(|| panic!("命令栏没渲染出来：找不到 accent 色的 \"$\" 提示符"));
    let mid_y = (dy0 + dy1) / 2;
    assert!(dx0 < W / 2, "\"$\" 应在命令栏左端，实得 bbox {:?}", (dx0, dy0, dx1, dy1));
    assert!(dx1 - dx0 < 24, "\"$\" 应是一枚小字形，实得 bbox {:?}", (dx0, dy0, dx1, dy1));

    // 点输入框中段（拉伸项，窗口水平中心必在其内）→ 聚焦。
    click(&ui, (W / 2) as f32, mid_y as f32);
    // 边框有 `animate border-color 120ms`：只 sleep 会量到动画前的旧色，必须
    // 再派发一次同坐标 PointerMoved 推进动画帧（见 MTerm 探针备忘）。
    std::thread::sleep(std::time::Duration::from_millis(220));
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new((W / 2) as f32, mid_y as f32),
    });

    let mut foc = Frame::new();
    foc.grab(&ui);
    maybe_dump(&foc, "cmdbar-focus");

    // ── ① 1px accent 边框（两侧）────────────────────────────────────────
    // 边框是跨满框高的 1px 竖线；「$」提示符、右端整块 accent 的「发送」按钮
    // 都不满足"整列几乎全 accent"，也不会是 1-2px 宽，故按列高度 + 宽度筛。
    let thin: Vec<(usize, usize)> =
        vertical_accent_runs(&foc, accent, mid_y - 12, mid_y + 12)
            .into_iter()
            .filter(|&(_, len)| len <= 2)
            .collect();
    // 右端「发送」按钮是整块 accent，其内部列也可能凑够高度（文字镂空处），
    // 但都排在输入框右边框之后 —— 取最靠左的两根即可。
    assert!(
        thin.len() >= 2,
        "聚焦后输入框左右应各有一根 1px accent 边框，实得竖线 {:?}",
        thin
    );
    let (left, right) = (thin[0].0, thin[1].0);
    assert!(right > left + 100, "两侧边框之间应是输入框，实得 {}..{}", left, right);

    // 常态同一位置是 border-subtle，不是 accent（证明细线是"聚焦后才有"的边）。
    assert!(
        !close(rest.rgb(left, mid_y), accent, 8),
        "常态 x={} 处不该是 accent 边，实得 {:?}",
        left,
        rest.rgb(left, mid_y)
    );

    // ── ② 边框外 3px 的 accent-soft 光晕（新建会话同款）────────────────
    let bar_bg = rest.rgb(left - 2, mid_y);
    let expect_halo = mix(bar_bg, accent, soft_alpha);
    let halo = foc.rgb(left - 2, mid_y);
    assert!(
        close(halo, expect_halo, 6),
        "聚焦光晕应为 mix(栏底色 {:?}, accent, {}（accent-soft alpha）) = {:?}，实得 {:?}",
        bar_bg,
        soft_alpha,
        expect_halo,
        halo
    );
    // 光晕只占 3px：再往外（-4）必须回到栏底色。
    assert!(
        close(foc.rgb(left - 4, mid_y), bar_bg, 3),
        "光晕宽度应只有 3px，-4px 处 {:?} 应等于栏底色 {:?}",
        foc.rgb(left - 4, mid_y),
        bar_bg
    );

    // ── ③ 聚焦后占位提示消失 ───────────────────────────────────────────
    // 占位文字从框内 x=22 起（"$" 之后），取 24..260 的带；参考底色取带右侧空处。
    let bg = rest.rgb(left + 300, mid_y);
    let count_text = |f: &Frame| -> usize {
        let mut n = 0;
        for y in (mid_y - 10)..(mid_y + 10) {
            for x in (left + 24)..(left + 260) {
                if !close(f.rgb(x, y), bg, 8) {
                    n += 1;
                }
            }
        }
        n
    };
    let (rest_text, foc_text) = (count_text(&rest), count_text(&foc));
    assert!(rest_text > 200, "常态应看得到占位提示（非底色像素 {} 个）", rest_text);
    assert!(
        foc_text < 40,
        "聚焦后占位提示应消失（只余光标），实得非底色像素 {} 个",
        foc_text
    );

    // ── ④ 历史按钮不再有灰底（#cmdbar-history-quiet）────────────────────
    // 按钮排在输入框右侧 8px 间距之后、宽 28px；取它左缘内 1px、中线偏下 6px
    // 的位置 —— 那里在按钮里、但离 12px 的时钟字形还有 13px，只可能量到底色。
    let bg_elevated = brush_solid(&theme.get_bg_elevated()).map(|(r, g, b, _)| (r, g, b));
    let probe = (right + 9, mid_y + 6);
    let px = rest.rgb(probe.0, probe.1);
    assert!(
        close(px, bar_bg, 3),
        "历史按钮常态应透明（栏底色 {:?}），实得 {:?}（灰底 bg-elevated {:?}）",
        bar_bg,
        px,
        bg_elevated
    );
}
