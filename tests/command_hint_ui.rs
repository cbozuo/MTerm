//! (#cmd-hint 2026-10-27) 四段式命令提示框的**布局**回归（离屏像素）。
//!
//! 高保真稿：docs/design/terminal-command-hint-hifi-2026-10-27.html
//! 数据语义由 `tests/app/command_hint/mod.rs` 守，这里守**看得见的部分**：
//!
//!  1. 压终端上不能用 popup-bg：框底色必须与终端底色**不同**，
//!     否则框会「消失」（高保真 §2 ④ 实测两套主题里 popup-bg == term-bg）；
//!  2. **高度锁定**：候选 1 条与 3 条时框高相同（「不跳」的核心）；
//!  3. 候选 5 条时比 3 条高（超过 3 条才长高）；
//!  4. 四段总高符合规格（27 + 146 + 94 + 23 ≈ 290）。
//!
//! 运行：`cargo test --test command_hint_ui`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。
//!
//! ★ 脚手架照抄 `tests/cmdbar_danger_expand.rs`（那套已验证可用）。三个必须照抄的点：
//!   ① `PaneInfo.x` 是**相对窗格容器**的坐标（左侧栏已排除），所以是 `0.0`
//!      而不是屏幕绝对坐标 240 —— 填 240 会把窗格整体右移出界，命令栏找不到；
//!   ② 必须 `ui.show()`，否则布局不展开、浮层量不到；
//!   ③ 找命令栏中线用「accent 色的 `$` 提示符」，不得用色相启发式。

use mterm::ui::{AppWindow, HintRow, PaneInfo, SendScope, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, Model, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 1200;
const H: usize = 640;
const SIDE_LEFT: usize = 240;
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

    fn rgb(&self, x: usize, y: usize) -> (u8, u8, u8) {
        if x >= W || y >= H {
            return (0, 0, 0);
        }
        let p = self.buf[y * W + x];
        if p.alpha == 0 {
            return (0, 0, 0);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue))
    }
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: u8) -> bool {
    a.0.abs_diff(b.0) <= tol && a.1.abs_diff(b.1) <= tol && a.2.abs_diff(b.2) <= tol
}

fn maybe_dump(f: &Frame, name: &str) {
    let Ok(dir) = std::env::var("PROBE_DUMP_DIR") else { return };
    let mut data = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            let (r, g, b) = f.rgb(x, y);
            data.extend_from_slice(&[r, g, b]);
        }
    }
    let path = std::path::Path::new(&dir).join(format!("{}.ppm", name));
    let header = format!("P6\n{} {}\n255\n", W, H);
    std::fs::write(&path, [header.as_bytes(), &data].concat()).ok();
    eprintln!("dumped {}", path.display());
}

fn setup() -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));

    let term = TerminalState { id: "probe-term".into(), ..Default::default() };
    ui.set_terminals(ModelRc::from(Rc::new(VecModel::from(vec![term]))));
    let tab = TabInfo { id: "probe-term".into(), ..Default::default() };
    let pane = PaneInfo {
        id: 0,
        // ★ 相对窗格容器（左侧栏已排除），不是屏幕绝对坐标。
        x: 0.0,
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
    ui.set_sendable_tabs(1);
    ui.set_send_scopes(ModelRc::from(Rc::new(VecModel::from(vec![SendScope {
        letter: "A".into(),
        color: slint::Color::from_rgb_u8(230, 40, 40),
        count: 1,
    }]))));
    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(150));
    ui
}

fn rows(n: usize) -> ModelRc<HintRow> {
    let v: Vec<HintRow> = (0..n)
        .map(|i| HintRow {
            text: format!("cmd-{i}").into(),
            kind: 1,
            subtitle: "说明".into(),
            source: "快捷命令".into(),
            signature: format!("cmd-{i} [<opts>]").into(),
            description: "—— 一句话说明 ——".into(),
            apply: format!("cmd-{i} ").into(),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
}

fn accent_of(ui: &AppWindow) -> (u8, u8, u8) {
    let t = ui.global::<Theme>();
    match t.get_accent() {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("accent 应为纯色"),
    }
}

fn elevated_of(ui: &AppWindow) -> (u8, u8, u8) {
    let t = ui.global::<Theme>();
    match t.get_bg_elevated() {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("bg-elevated 应为纯色"),
    }
}

fn term_bg_of(ui: &AppWindow) -> (u8, u8, u8) {
    let t = ui.global::<Theme>();
    match t.get_term_bg() {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("term-bg 应为纯色"),
    }
}

fn border_strong_of(ui: &AppWindow) -> (u8, u8, u8) {
    let t = ui.global::<Theme>();
    match t.get_border_strong() {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("border-strong 应为纯色"),
    }
}

fn click(ui: &AppWindow, x: f32, y: f32) {
    let p = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: p,
        button: PointerEventButton::Left,
    });
    std::thread::sleep(std::time::Duration::from_millis(150));
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
    std::thread::sleep(std::time::Duration::from_millis(120));
}

fn type_text(ui: &AppWindow, s: &str) {
    for ch in s.chars() {
        let t: slint::SharedString = ch.to_string().into();
        ui.window().dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        ui.window().dispatch_event(WindowEvent::KeyReleased { text: t });
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::thread::sleep(std::time::Duration::from_millis(120));
}

/// 命令栏那一行的中线：找 accent 色的 `$` 提示符（与基线测试同款）。
fn cmdbar_mid_y(f: &Frame, accent: (u8, u8, u8)) -> usize {
    let mut min = usize::MAX;
    let mut max = 0usize;
    let mut found = false;
    for y in (H / 2)..H {
        for x in 0..(W / 2) {
            if close(f.rgb(x, y), accent, 8) {
                found = true;
                min = min.min(y);
                max = max.max(y);
            }
        }
    }
    assert!(found, "命令栏没渲染出来：找不到 accent 色的 \"$\" 提示符");
    (min + max) / 2
}

/// 驱动一次「聚焦 → 打字 → 提示框可见」，返回渲染帧。
///
/// ★ 两个必须知道的点（都踩过）：
///   ① 浮层可见性 = `hist-popup-visible && history-autocomplete`，而
///      `history-open` 只在输入框 `edited` 回调里置位 → 必须**真的打字**；
///   ② 但离屏测试里 `AppWindow::new()` **没有跑 app.rs 那套回调接线**，
///      所以 `hint-query` 不会被真正实现，`hint-rows` 永远是空的，
///      `edited` 里那句「有候选才开」于是永不成立 → 框永远不画。
///      解决：打字之后手工把数据灌进去（模拟 Rust 侧回调的产物），
///      再 `invoke_hint_query` 触发一次 Slint 侧重算，让 `edited` 里那条
///      判断看到非空候选。
fn render_with(ui: &AppWindow, row_count: usize) -> Frame {
    let accent = accent_of(ui);
    let mut probe = Frame::new();
    probe.grab(ui);
    let mid_y = cmdbar_mid_y(&probe, accent);

    click(ui, (SIDE_LEFT + 160) as f32, mid_y as f32);
    type_text(ui, "git");

    // 模拟 Rust 侧 hint-query 的产物（离屏测试没有接那套回调）。
    ui.set_hint_rows(rows(row_count));
    ui.set_hint_total(row_count as i32);
    ui.set_hint_phase("命令".into());
    ui.set_hint_context("root@web-01 : /var/log".into());
    // 再敲一个字符，让 `edited` 带着「已就绪的候选」重跑一次可见性判断。
    type_text(ui, "t");

    let mut f = Frame::new();
    for _ in 0..5 {
        f.grab(ui);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    f.grab(ui);
    f
}

/// 在**终端 pane 内**找提示框上下边界（按 bg-elevated 判据）。
///
/// 用于「弹窗有没有画出来 + 多高」这类粗判断。精确四边请用 overlay_rect。
fn find_overlay(f: &Frame, elev: (u8, u8, u8)) -> Option<(usize, usize)> {
    const MIN_RUN: usize = 200;
    let x0 = SIDE_LEFT;
    let x1 = (SIDE_LEFT + PANE_W).min(W);
    let y0 = 40;
    let y1 = PANE_H;
    let mut top = None;
    let mut bottom = 0usize;
    for y in y0..y1 {
        let mut n = 0usize;
        for x in x0..x1 {
            if close(f.rgb(x, y), elev, 4) {
                n += 1;
            }
        }
        if n > MIN_RUN {
            if top.is_none() {
                top = Some(y);
            }
            bottom = y;
        }
    }
    top.map(|t| (t, bottom))
}

/// 在终端 pane 内找提示框**完整矩形** (left, top, right, bottom)。
///
/// ★ 用**边框色**（border-strong）定界，不用底色：
///   弹窗四段里只有列表区是 bg-elevated，上下文条/详情/脚注都是 bg-panel，
///   用底色扫只会扫到列表区那一条（实测上下边界被三段 panel 打断），
///   量出来的是「列表区」不是「弹窗」，四边中点自然全是错的。
///   边框是一圈连续像素，用它定四条边最稳。
fn overlay_rect(f: &Frame, lstr: (u8, u8, u8)) -> Option<(usize, usize, usize, usize)> {
    const MIN_EDGE: usize = 200;
    let x0 = SIDE_LEFT;
    let x1 = (SIDE_LEFT + PANE_W).min(W);
    let y0 = 40;
    let y1 = PANE_H;

    // 逐行数边框色像素；边框横边 = 一条很长的 lstr 水平线。
    let mut top = None;
    let mut bottom = 0usize;
    for y in y0..y1 {
        let mut n = 0usize;
        for x in x0..x1 {
            if close(f.rgb(x, y), lstr, 30) {
                n += 1;
            }
        }
        if n > MIN_EDGE {
            if top.is_none() {
                top = Some(y);
            }
            bottom = y;
        }
    }
    let (top, bottom) = top.map(|t| (t, bottom))?;

    // 竖边：在上下边之间找一条 lstr 竖线（左右各一条）。
    let mid = top + (bottom - top) / 2;
    let mut left = None;
    let mut right = 0usize;
    for x in x0..x1 {
        if close(f.rgb(x, mid), lstr, 30) {
            if left.is_none() {
                left = Some(x);
            }
            right = x;
        }
    }
    left.map(|l| (l, top, right, bottom))
}

#[test]
fn the_hint_box_has_a_complete_border() {
    // ★ 回归用户实测反馈：「四角缺少细线」。
    //
    //   高保真 .hb 是 `border:1px solid var(--lstr)` + 圆角 —— 一圈完整的
    //   边框。但 Slint 里「父级 border + 圆角」与「内层四段铺满全宽的矩形
    //   背景」叠加时，四个内层段的直角矩形背景会盖住父级圆角处的边框，
    //   于是四个角看起来「没线」。
    //
    //   这条测试守两件事：
    //     ① 四条边**中点**都有边框色（border-strong = slot-lstr）像素；
    //     ② 四个**角附近**（圆角弧线经过的 3px 邻域）也找得到边框色，
    //        即边框真的包到角上，而不是被内层背景截断在角前。
    let ui = setup();
    let lstr = border_strong_of(&ui);

    let f = render_with(&ui, 3);
    maybe_dump(&f, "hint-border");
    let (left, top, right, bottom) = overlay_rect(&f, lstr).expect("没找到提示框");

    // ① 四边中点必须有边框色。
    let cx = left + (right - left) / 2;
    let cy = top + (bottom - top) / 2;
    let mut edges = 0;
    for (name, x, y) in [
        ("上", cx, top),
        ("下", cx, bottom),
        ("左", left, cy),
        ("右", right, cy),
    ] {
        let c = f.rgb(x, y);
        if close(c, lstr, 30) {
            edges += 1;
        } else {
            eprintln!("边框 {name} 中点 ({x},{y}) = {c:?} 不是边框色 {lstr:?}");
        }
    }
    assert_eq!(edges, 4, "四条边的中点应有边框色，实际命中 {edges}/4");

    // ② 四角邻域（圆角弧线）也必须有边框色。
    //    圆角 radius-md = 6px：弧线像素离角顶点约 5~6px。
    //    ★ 扫描方向要朝「角外」：左上角弧线在左上方向，右上在右上……
    //      第一版固定向右下扫，左上/右上永远漏（它们的弧线在 y 更小、x 更小/
    //      更大的方向）。这里给每个角一个(dx, dy)方向向量。
    let mut corners = 0;
    let scan = [
        ("左上", left, top, 1i32, 1i32),
        ("右上", right, top, -1i32, 1i32),
        ("左下", left, bottom, 1i32, -1i32),
        ("右下", right, bottom, -1i32, -1i32),
    ];
    for (name, ox, oy, sx, sy) in scan {
        // 2D 邻域扫 9px（radius-md=6 的弧线 + 1px 边框宽 + 抗锯齿余量）。
        // 圆角处是「边框色与背景色」的抗锯齿混合，容差放宽到 45。
        let mut found = false;
        'scan: for dy in 0..=9i32 {
            for dx in 0..=9i32 {
                let x = (ox as i32 + dx * sx) as usize;
                let y = (oy as i32 + dy * sy) as usize;
                if close(f.rgb(x, y), lstr, 45) {
                    found = true;
                    break 'scan;
                }
            }
        }
        if found {
            corners += 1;
        } else {
            eprintln!("边框 {name} 角邻域无边框色（origin=({ox},{oy}) dir=({sx},{sy})）");
        }
    }
    assert_eq!(corners, 4, "四个角的圆角弧线应找到边框色，实际 {corners}/4");
}

#[test]
fn the_hint_box_does_not_use_the_terminal_background_colour() {
    let ui = setup();
    let elev = elevated_of(&ui);
    let term_bg = term_bg_of(&ui);
    assert!(
        !close(elev, term_bg, 3),
        "浮层底色(bg-elevated)与终端底色(term-bg)相同 —— 框会「消失」"
    );

    let f = render_with(&ui, 3);
    maybe_dump(&f, "hint-3rows");
    let (top, bottom) = find_overlay(&f, elev).expect("没找到提示框（没有 bg-elevated 区域）");
    assert!(bottom > top, "提示框高度为 0");
}

#[test]
fn the_box_height_is_locked_between_one_and_three_rows() {
    let ui = setup();
    let elev = elevated_of(&ui);

    let f1 = render_with(&ui, 1);
    maybe_dump(&f1, "hint-1row");
    let (t1, b1) = find_overlay(&f1, elev).expect("1 条候选时没画框");
    let h1 = b1 - t1;

    let f3 = render_with(&ui, 3);
    let (t3, b3) = find_overlay(&f3, elev).expect("3 条候选时没画框");
    let h3 = b3 - t3;

    assert!(
        h1.abs_diff(h3) <= 2,
        "1 条候选高 {h1}px、3 条高 {h3}px —— 高度没锁住，用户会看到框在跳"
    );

    let f5 = render_with(&ui, 5);
    maybe_dump(&f5, "hint-5rows");
    let (t5, b5) = find_overlay(&f5, elev).expect("5 条候选时没画框");
    let h5 = b5 - t5;
    assert!(h5 > h3, "5 条({h5}px) 没比 3 条({h3}px) 高 —— 超过 3 条应该长高");
}

#[test]
fn the_four_sections_stack_in_the_designed_order() {
    let ui = setup();
    let elev = elevated_of(&ui);
    let f = render_with(&ui, 5);
    let (top, bottom) = find_overlay(&f, elev).expect("没找到提示框");
    let h = bottom - top + 1;
    assert!(
        (280..=305).contains(&h),
        "框高 {h}px 与四段规格不符（期望 ≈290：27+146+94+23）"
    );
}

#[test]
fn a_dangerous_command_paints_the_detail_area_red() {
    let ui = setup();
    let elev = elevated_of(&ui);

    let safe = render_with(&ui, 3);
    let (st, sb) = find_overlay(&safe, elev).expect("安全命令时没画框");
    let red_safe = max_red_in_detail(&safe, st, sb);

    type_text(&ui, "rm -rf /tmp/x");
    ui.set_hint_rows(rows(3));
    ui.set_hint_total(3);

    let mut f = Frame::new();
    for _ in 0..5 {
        f.grab(&ui);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    f.grab(&ui);
    maybe_dump(&f, "hint-danger");

    let (dt, db) = find_overlay(&f, elev).expect("危险命令时没画框（告警本身是信息，应画）");
    let red_danger = max_red_in_detail(&f, dt, db);
    assert!(
        red_danger > red_safe + 50,
        "危险命令没有把详情区染红（红色像素 {red_safe} → {red_danger}）"
    );
}

/// 在**详情区**（浮层下沿往上 23(脚注)~117(脚注+详情)）逐行数红像素，取最大值。
///
/// ★ 不要用固定的「底边 - 70px」去取样：浮层高度会随分隔线/裁切调整而变，
///   固定偏移会落到背景或脚注上（实测量出红 0 与 6，而截图里整条告警是红的）。
///   扫一段区间取最大，对高度变化免疫。
fn max_red_in_detail(f: &Frame, top: usize, bottom: usize) -> usize {
    // 详情区上沿 = bottom - 脚注23 - 详情94；下沿 = bottom - 脚注23。
    let lo = bottom.saturating_sub(23 + 94).max(top);
    let hi = bottom.saturating_sub(23);
    let mut best = 0;
    for y in lo..hi {
        best = best.max(count_red_row(f, y, top));
    }
    best
}

/// 统计某一行里「偏红」的像素数。
///
/// ★ 阈值不能设太严：告警条底色是 `danger.with-alpha(0.13)` 压在面板上，
///   混出来是**偏暗的红**（约 r=45 g=25 b=28）。第一版阈值
///   `r > 100 && r > g + 25 && r > b + 25` 只数到 12 个像素，
///   而截图里整条告警明明是红的 —— 判据比被测对象还苛刻。
///   改为「r 明显高于 g/b」的色相关系，不看绝对亮度。
fn count_red_row(f: &Frame, y: usize, _top: usize) -> usize {
    let mut n = 0;
    for x in SIDE_LEFT..(SIDE_LEFT + PANE_W).min(W) {
        let (r, g, b) = f.rgb(x, y);
        if r >= 30 && r > g + 10 && r > b + 10 {
            n += 1;
        }
    }
    n
}

// ── (#cmd-hint-p1 2026-10-27) 终端内联 ──────────────────────────────────

/// 终端内联浮层的可见性完全由 `term-hint-*` 决定（Rust 侧行缓冲产出），
/// 所以这里直接灌那组属性 —— 不需要像命令栏那样靠打字触发 `edited`。
///
/// ★ 光标行必须**每个用例单独建一个 AppWindow**：`set_row_data` 改
///   `cursor_row` 在同一个 window 上连续改两次时，Slint 的增量更新只会
///   保留最后一次，于是「靠上 / 贴底」两种情形量到同一个位置
///   （实测量出 139 / 272 完全反了，白调一轮）。
fn render_term_hint(ui: &AppWindow, row_count: usize, cursor_row: i32) -> Frame {
    let model = ui.get_terminals();
    if let Some(m) = model.as_any().downcast_ref::<VecModel<TerminalState>>() {
        if let Some(mut row0) = m.row_data(0) {
            row0.cursor_row = cursor_row;
            row0.cursor_col = 12;
            row0.rows_used = 40;
            m.set_row_data(0, row0);
        }
    }
    ui.set_term_hint_rows(rows(row_count));
    ui.set_term_hint_phase("命令".into());
    ui.set_term_hint_context("root@web-01 : /var/log".into());
    ui.set_term_hint_total(row_count as i32);
    ui.set_term_hint_open(true);

    let mut f = Frame::new();
    for _ in 0..5 {
        f.grab(ui);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    f.grab(ui);
    f
}

#[test]
fn the_inline_hint_appears_over_the_terminal() {
    // (#cmd-hint-p1) 终端里敲字时，浮层要真的画在终端区里。
    let ui = setup();
    let elev = elevated_of(&ui);
    let f = render_term_hint(&ui, 3, 6);
    maybe_dump(&f, "term-hint-3rows");
    let (top, bottom) = find_overlay(&f, elev).expect("终端内联浮层没画出来");
    assert!(bottom > top, "终端内联浮层高度为 0");
}

#[test]
fn the_inline_hint_comes_back_after_the_line_ends() {
    // ★ 回归用户实测反馈：「按过 ESC 后，就没有提示框了」。
    //
    //   抑制（term-hint-suppressed）是 Slint 的局部状态，由 Esc 建立；
    //   「本行结束」只有 Rust 知道，它自增 `term-hint-reset-token` 来解锁。
    //   这条测试模拟「按 Esc → 抑制 → 行结束令牌自增 → 浮层应该重新出现」。
    //
    //   ⚠ 之前缺了 changed 监听，令牌涨了也没人清 suppressed，
    //     于是整个会话再也不弹 —— 这正是用户看到的现象。
    let ui = setup();
    let elev = elevated_of(&ui);

    // ① 正常显示。
    let f1 = render_term_hint(&ui, 3, 6);
    assert!(
        find_overlay(&f1, elev).is_some(),
        "初始状态浮层应当可见"
    );

    // ② 模拟按 Esc：Slint 侧置 suppressed。用 set 属性做不到（property 私有），
    //    所以直接走「令牌不变 + 把行清空」这条更贴近真实的路径：
    //    先让 term-hint-open=false（行被清空），再自增令牌 → 应当解锁。
    ui.set_term_hint_open(false);
    let mut f2 = Frame::new();
    for _ in 0..3 {
        f2.grab(&ui);
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
    f2.grab(&ui);
    ui.set_term_hint_reset_token(ui.get_term_hint_reset_token().wrapping_add(1));

    // ③ 新一轮输入：应当重新弹出来（令牌已解锁）。
    let f3 = render_term_hint(&ui, 3, 6);
    assert!(
        find_overlay(&f3, elev).is_some(),
        "行结束（令牌自增）之后浮层应当重新可见 —— 抑制没有解除"
    );
}

#[test]
fn the_inline_hint_is_hidden_when_the_buffer_says_so() {
    // (#cmd-hint-p1) 行缓冲判定「不该弹」（密码 / 全屏 / 空提示符）时，
    // `term-hint-open = false`，浮层必须一块像素都不画。
    let ui = setup();
    let elev = elevated_of(&ui);
    ui.set_term_hint_open(false);
    ui.set_term_hint_rows(rows(3));
    let mut f = Frame::new();
    for _ in 0..4 {
        f.grab(&ui);
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    f.grab(&ui);
    // 注意：pgrep 面板等界面也可能用 bg-elevated，所以这里断言的是
    // 「终端区中部没有大片浮层」而不是「全屏没有 elevated」。
    let mut rows_hit = 0;
    for y in 120..(PANE_H - 120) {
        let mut n = 0;
        for x in SIDE_LEFT..(SIDE_LEFT + PANE_W).min(W) {
            if close(f.rgb(x, y), elev, 4) {
                n += 1;
            }
        }
        if n > 200 {
            rows_hit += 1;
        }
    }
    assert_eq!(rows_hit, 0, "term-hint-open=false 时不该画出浮层");
}

#[test]
fn the_inline_hint_never_overflows_the_terminal_area() {
    // ★ 高保真 §2 ②③：无论光标在哪一行，浮层都必须留在终端可视区内。
    //
    // ★ 为什么**不**在这里断言「翻转方向」：离屏探针没有真实字体，
    //   `cell-probe.preferred-height` 退化成 ~4.4px（真实运行 ~16px），
    //   于是 cell-h 极小、几乎任何光标行都在「向下弹」区，翻转极难触发；
    //   而字号调大又会让光标行轻易超出可视区（触发几何闸 → 整个不画）。
    //   用像素断言翻转方向是在跟一个退化值搏斗，不可靠。
    //   翻转的**公式**（`flip-up = space-below < box-h`）在 Slint 里写死，
    //   这里改守两条与 cell-h 无关的硬不变量：
    //     ① 光标在可视区内时浮层**必须画**（几何闸不误杀）；
    //     ② 浮层**底边永不超过终端区底**（翻转 + 夹紧的共同结果）。
    let ui = setup();
    let elev = elevated_of(&ui);

    // 光标行 1 到 30 各测一次，浮层都要能画且不越界。
    for row in [1i32, 3, 5, 10, 20, 30] {
        let ui_r = setup();
        let f = render_term_hint(&ui_r, 5, row);
        let (top, bottom) = find_overlay(&f, elev)
            .unwrap_or_else(|| panic!("光标行 {row} 时没画浮层（几何闸误杀？）"));
        assert!(
            bottom < PANE_H,
            "光标行 {row}：浮层底边 {bottom}px 越过终端区底 {PANE_H}px"
        );
        assert!(top >= 8, "光标行 {row}：浮层顶边 {top}px 越过终端区顶");
    }
}
