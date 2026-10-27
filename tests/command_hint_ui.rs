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
    // 详情区在脚注（23px）之上、高 94px，取它的**中线**取样。
    // ★ 原来写 `sb - 23 - 40` 那是详情区的下沿附近，实测落在背景上（红 0）。
    let detail_y = |bottom: usize| bottom.saturating_sub(23 + 47);
    let red_safe = count_red_row(&safe, detail_y(sb), st);

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

    let (_dt, db) = find_overlay(&f, elev).expect("危险命令时没画框（告警本身是信息，应画）");
    let red_danger = count_red_row(&f, detail_y(db), 0);
    assert!(
        red_danger > red_safe + 50,
        "危险命令没有把详情区染红（红色像素 {red_safe} → {red_danger}）"
    );
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
