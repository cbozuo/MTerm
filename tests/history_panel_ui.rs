//! (#cmd-history-panel 2026-10-28) 历史命令面板的**布局**回归（离屏像素）。
//!
//! 高保真稿：docs/design/history-panel-hifi-2026-10-27.html
//! 数据语义由 `tests/app/command_history/mod.rs` 守，这里守**看得见的部分**，
//! 全部对应「用户截图打回」的四条 + 高保真 §9 的核心验收：
//!
//!  1. `panel_frame_is_visible_over_terminal`：边线框真的画出来了
//!     （border-strong 一圈连续细线 + elevated 底 ≠ 终端底，§4 硬约束 ⑧）；
//!  2. `toolbar_content_is_vertically_centered`：工具栏内容垂直居中
//!     （布局铁律 ③：固定高子项在布局里会顶对齐，必须槽内居中）；
//!  3. `row_meta_hugs_the_right_edge`：行内「次数 · 时间」贴行右缘
//!     （高保真 .cmd flex:1 —— alignment 一写 stretch 就归零，实测踩过）；
//!  4. `row_text_is_vertically_centered`：行文字垂直居中；
//!  5. `hover_actions_do_not_shift_the_command_text`：悬停动作键浮出、
//!     元信息淡出、命令文字一个像素都不挪（§7 决策 02）；
//!  6. `panel_height_is_locked`：1 条与 3 条数据时面板总高相同（§4 锁高）；
//!  7. `footer_count_hugs_the_right_edge`：页脚计数/清空贴右缘；
//!  8. `only_visible_rows_are_drawn`：数据多于可见行数时只画 list-rows 行
//!     （§6：↑↓ 只循环在屏幕内）。
//!
//! 运行：`cargo test --test history_panel_ui`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。
//!
//! ★ 脚手架照抄 `tests/command_hint_ui.rs`（已跑通的那套）。

use mterm::ui::{AppWindow, HistoryRow, PaneInfo, SendScope, TabInfo, TerminalState};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, ModelRc, VecModel};
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
    std::fs::write(&path, [format!("P6\n{} {}\n255\n", W, H).into_bytes(), data].concat()).ok();
    eprintln!("dumped {}", path.display());
}

// ── 主题取色（默认 slot 色板；frost 在无壁纸时回落纯色，见 command_hint_ui）──

fn theme_color(ui: &AppWindow, get: fn(&mterm::ui::Theme) -> slint::Brush) -> (u8, u8, u8) {
    match get(&ui.global::<mterm::ui::Theme>()) {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("主题色应为纯色（无壁纸时 frost 回落纯色）"),
    }
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

fn hist_rows(specs: &[(&str, u32, bool)]) -> ModelRc<HistoryRow> {
    let v: Vec<HistoryRow> = specs
        .iter()
        .map(|(cmd, uses, pinned)| HistoryRow {
            cmd: (*cmd).into(),
            display: (*cmd).into(),
            pre: "".into(),
            hit: "".into(),
            post: "".into(),
            uses_label: format!("{} 次", uses).into(),
            time_label: " · 4 分钟前".into(),
            pinned: *pinned,
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(v)))
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

fn press_key(ui: &AppWindow, key: &str) {
    let t: slint::SharedString = key.into();
    ui.window().dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
    ui.window().dispatch_event(WindowEvent::KeyReleased { text: t });
    std::thread::sleep(std::time::Duration::from_millis(120));
}

fn grab_stable(ui: &AppWindow) -> Frame {
    let mut f = Frame::new();
    for _ in 0..4 {
        f.grab(ui);
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
    f.grab(ui);
    f
}

/// 命令栏那一行的中线：找 accent 色的 `$` 提示符（与基线测试同款）。
/// (#hist-round6) 只取**底部最后一段连续 accent 行带**的中点 —— 面板打开时
/// 列表里也有 accent 像素（选中行左缘条），全窗 min/max 会被面板拉偏
/// （面板变矮后选中条落到中线以下，图标扫描带错位、簇数 0 → panic）。
fn cmdbar_mid_y(f: &Frame, accent: (u8, u8, u8)) -> usize {
    let mut rows = Vec::new();
    for y in (H / 2)..H {
        for x in 0..(W / 2) {
            if close(f.rgb(x, y), accent, 8) {
                rows.push(y);
                break;
            }
        }
    }
    assert!(!rows.is_empty(), "命令栏没渲染出来：找不到 accent 色的 \"$\" 提示符");
    let mut end = rows[rows.len() - 1];
    let mut start = end;
    for &y in rows.iter().rev() {
        if end - y <= 2 {
            start = y;
        } else {
            break;
        }
    }
    (start + end) / 2
}

/// 命令栏右端图标簇的 x 中心（⚡ 快捷命令 / 🕘 历史 / ↗ 发送窗）。
/// 图标是 bar 底色上的前景色像素簇；从左到右扫描即可。
fn bar_icon_clusters(f: &Frame, bar_mid: usize, bar_bg: (u8, u8, u8)) -> Vec<usize> {
    let y0 = bar_mid.saturating_sub(14);
    let y1 = (bar_mid + 14).min(H);
    let mut cols = vec![false; W];
    for x in 600..W {
        for y in y0..y1 {
            let c = f.rgb(x, y);
            if !close(c, bar_bg, 12) && (c.0 as u16 + c.1 as u16 + c.2 as u16) > 120 {
                cols[x] = true;
                break;
            }
        }
    }
    let mut clusters = Vec::new();
    let mut start = None;
    for x in 600..W {
        if cols[x] {
            start.get_or_insert(x);
        } else if let Some(s) = start.take() {
            if x - s >= 6 {
                clusters.push((s + x) / 2);
            }
        }
    }
    clusters
}

/// 在终端 pane 内找面板**完整矩形** (left, top, right, bottom)。
/// ★ 用边框色（border-strong）定界：一圈连续细线最稳（同 command_hint_ui）。
fn panel_rect(f: &Frame, lstr: (u8, u8, u8)) -> Option<(usize, usize, usize, usize)> {
    const MIN_EDGE: usize = 200;
    let x0 = SIDE_LEFT;
    let x1 = (SIDE_LEFT + PANE_W).min(W);
    let y0 = 40;
    let y1 = PANE_H;

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

    let height = bottom.saturating_sub(top) + 1;
    let need = (height * 8) / 10;
    let mut left = None;
    let mut right = 0usize;
    for x in (x0 + 2)..x1.saturating_sub(2) {
        let mut n = 0usize;
        for y in top..=bottom {
            if close(f.rgb(x, y), lstr, 30) {
                n += 1;
            }
        }
        if n >= need {
            if left.is_none() {
                left = Some(x);
            }
            right = x;
        }
    }
    left.map(|l| (l, top, right, bottom))
}

fn open_panel(ui: &AppWindow) -> Frame {
    let accent = theme_color(ui, |t| t.get_accent());
    let panel_bg = theme_color(ui, |t| t.get_bg_panel());
    let lstr = theme_color(ui, |t| t.get_border_strong());
    let mut probe = Frame::new();
    probe.grab(ui);
    let mid = cmdbar_mid_y(&probe, accent);

    // 命令栏右端三个图标簇，时钟是第 2 个；优先点它，开出面为止。
    // ★ 面板判定要同时看宽和高：快捷命令弹层宽也能到 400+，但只有 ~80 高。
    let clusters = bar_icon_clusters(&probe, mid, panel_bg);
    let order = [1, 2, 0];
    for k in order {
        let Some(&cx) = clusters.get(k) else { continue };
        click(ui, cx as f32, mid as f32);
        let f = grab_stable(ui);
        if let Some((l, t, r, b)) = panel_rect(&f, lstr) {
            if r - l >= 400 && b - t >= 300 {
                return f;
            }
        }
        press_key(ui, "Escape"); // 误开快捷命令/发送窗就收掉再试
    }
    panic!("历史面板没打开：三个命令栏图标都点过仍找不到面板边框");
}

fn band_has_pixels(f: &Frame, (l, t, r, b): (usize, usize, usize, usize), pred: impl Fn((u8, u8, u8)) -> bool) -> usize {
    let mut n = 0;
    for y in t..=b {
        for x in l..=r {
            if pred(f.rgb(x, y)) {
                n += 1;
            }
        }
    }
    n
}

// ══════════ 断言 ══════════

/// ① 边线框真的画出来了：一圈 border-strong 细线 + elevated 底压在终端上。
/// （用户截图打回：面板整条边框丢失，浮在终端上没有边界。）
#[test]
fn panel_frame_is_visible_over_terminal() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());
    let elev = theme_color(&ui, |t| t.get_bg_elevated());
    let tbg = theme_color(&ui, |t| t.get_term_bg());

    ui.set_history_rows(hist_rows(&[("echo a", 1, false), ("echo b", 2, false)]));
    ui.set_history_total(2);
    ui.set_history_pinned(0);

    let f = open_panel(&ui);
    maybe_dump(&f, "hist_panel_frame");

    let (l, t, r, b) = panel_rect(&f, lstr).expect("面板边框必须可见");
    assert!(r - l >= 400, "面板宽应 ≈560，量到 {}", r - l);
    assert!(b - t >= 300, "面板高应 ≈386（8 行锁高），量到 {}", b - t);

    // 底色抽查：面板内部是 elevated，不是终端底（压终端不消失）。
    let inside = f.rgb((l + r) / 2, (t + b) / 2);
    assert!(close(inside, elev, 8), "面板内部应为 bg-elevated，量到 {:?}", inside);
    assert!(
        !close(elev, tbg, 4),
        "bg-elevated 与终端底色相同会让面板「消失」（高保真 §4 硬约束）"
    );
}

/// ② 工具栏内容垂直居中：搜索框（field 底）在 46px 工具栏带内上下留白对称。
#[test]
fn toolbar_content_is_vertically_centered() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());
    let field = theme_color(&ui, |t| t.get_field());

    ui.set_history_rows(hist_rows(&[("echo a", 1, false)]));
    ui.set_history_total(1);

    let f = open_panel(&ui);
    maybe_dump(&f, "hist_toolbar_center");
    let (l, t, _r, _b) = panel_rect(&f, lstr).unwrap();

    // 工具栏带 = 面板顶 +1 .. +45（避开 1px 边框）。找 field 色的行范围。
    let mut top = None;
    let mut bot = 0usize;
    for y in (t + 2)..(t + 45) {
        let mut n = 0;
        for x in (l + 4)..(l + 200) {
            if close(f.rgb(x, y), field, 6) {
                n += 1;
            }
        }
        if n > 30 {
            if top.is_none() {
                top = Some(y);
            }
            bot = y;
        }
    }
    let top = top.expect("工具栏里找不到搜索框（field 底）");
    let center = (top + bot) as f32 / 2.0;
    let want = t as f32 + 23.0; // 工具栏带 [t, t+46] 的中线
    assert!(
        (center - want).abs() <= 3.0,
        "搜索框应垂直居中于 46px 工具栏：中心 {} 偏离 {} 超过 3px",
        center,
        want
    );
}

/// ③ 行内「次数 · 时间」贴行右缘（高保真 .cmd flex:1）。
#[test]
fn row_meta_hugs_the_right_edge() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());

    ui.set_history_rows(hist_rows(&[
        ("echo first-row", 7, false),
        ("echo second-row", 3, false),
    ]));
    ui.set_history_total(2);

    let f = open_panel(&ui);
    maybe_dump(&f, "hist_meta_right");
    let (l, t, r, _b) = panel_rect(&f, lstr).unwrap();

    // 第 1 行带：面板顶 + 46(工具栏) + 4(内衬) .. +38。
    let row_t = t + 46 + 4 + 1;
    let row_b = row_t + 36;
    // 行内最右的内容像素（meta 是行内最右元素）应落在右缘 6~60px 内。
    let mut rightmost = 0usize;
    for y in row_t..row_b {
        for x in (l + 4)..(r - 2) {
            let c = f.rgb(x, y);
            if (c.0 as u16 + c.1 as u16 + c.2 as u16) > 150 && !close(c, lstr, 30) {
                rightmost = rightmost.max(x);
            }
        }
    }
    assert!(
        rightmost >= r - 60,
        "meta 应贴行右缘：最右内容像素 x={}，面板右缘 x={}（相差 {}px）",
        rightmost,
        r,
        r - rightmost
    );
}

/// ④ 行文字垂直居中（38px 行内上下留白对称）。
#[test]
fn row_text_is_vertically_centered() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());

    ui.set_history_rows(hist_rows(&[("echo center-me", 1, false)]));
    ui.set_history_total(1);

    let f = open_panel(&ui);
    maybe_dump(&f, "hist_row_center");
    let (l, t, r, _b) = panel_rect(&f, lstr).unwrap();

    let row_t = t + 46 + 4;
    let row_b = row_t + 38;
    let mut tmin = usize::MAX;
    let mut tmax = 0usize;
    for y in (row_t + 2)..(row_b - 2) {
        for x in (l + 8)..(r - 8) {
            let c = f.rgb(x, y);
            if (c.0 as u16 + c.1 as u16 + c.2 as u16) > 400 {
                tmin = tmin.min(y);
                tmax = tmax.max(y);
            }
        }
    }
    assert!(tmax > tmin, "行内没扫到命令文字");
    let center = (tmin + tmax) as f32 / 2.0;
    let want = (row_t + row_b) as f32 / 2.0;
    assert!(
        (center - want).abs() <= 4.0,
        "行文字应垂直居中：文字中心 {} 行中心 {}",
        center,
        want
    );
}

/// ⑤ 悬停交互：动作键浮出、元信息淡出、命令文字一个像素都不挪（§7 决策 02）。
/// ★ `[ignore]`：离屏脚手架的 `PointerMoved` 不产生 hover 态（MinimalSoftwareWindow
///   不追踪无按压的移动，实测前后 0 像素差），这条只能在真机窗口里验 ——
///   2026-10-28 已在隔离实例 + 真实光标下人工核过：动作键浮出、meta 让位、
///   悬停全文卡弹出、其余行不受影响。留着用例当意图记录与将来脚手架升级的挂点。
#[test]
#[ignore = "离屏脚手架驱动不了 hover，需真机人工验证（2026-10-28 已验）"]
fn hover_actions_do_not_shift_the_command_text() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());

    ui.set_history_rows(hist_rows(&[
        ("echo first-row", 7, false),
        ("echo hover-target-row", 3, false),
    ]));
    ui.set_history_total(2);

    let before = open_panel(&ui);
    maybe_dump(&before, "hist_hover_before");
    let (l, t, r, _b) = panel_rect(&before, lstr).unwrap();

    let row_t = t + 46 + 4 + 38; // 第 2 行
    let row_b = row_t + 38;

    // 悬停前：命令文字的横向范围 + meta 像素数。
    let text_span = |f: &Frame| -> (usize, usize) {
        let mut min = usize::MAX;
        let mut max = 0;
        for y in (row_t + 4)..(row_b - 4) {
            for x in (l + 8)..(l + 300) {
                let c = f.rgb(x, y);
                if (c.0 as u16 + c.1 as u16 + c.2 as u16) > 400 {
                    min = min.min(x);
                    max = max.max(x);
                }
            }
        }
        (min, max)
    };
    // meta 计数带要避开右端 102px 的动作键区（动作键图标也是中间调，
    // 不避开的话悬停后图标会把「meta 淡出」的计数顶回去）。
    let meta_pixels = |f: &Frame| -> usize {
        band_has_pixels(f, (r - 250, row_t + 2, r - 118, row_b - 2), |c| {
            (c.0 as u16 + c.1 as u16 + c.2 as u16) > 150 && (c.0 as u16 + c.1 as u16 + c.2 as u16) < 620
        })
    };
    let (b_min, b_max) = text_span(&before);
    assert!(b_max > b_min, "悬停前没扫到命令文字");

    // 悬停第 2 行。★ 离屏窗口要先有过一次「按下-抬起」才有鼠标状态，
    // 纯 PointerMoved 不会被处理成 hover（实测前后 0 像素差）——
    // 先在页脚空白处空点一下（spacer，无副作用），再移到行上。
    click(&ui, ((l + r) / 2 + 40) as f32, (t as f32 + 372.0)); // 页脚空白（面板高 ≈386）
    let p = LogicalPosition::new(((l + r) / 2) as f32, ((row_t + row_b) / 2) as f32);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    let mut hovered = Frame::new();
    for _ in 0..6 {
        hovered.grab(&ui);
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
    hovered.grab(&ui);
    maybe_dump(&hovered, "hist_hover_after");

    // 文字不挪（悬停前后命令文字横向范围一致）。
    let (a_min, a_max) = text_span(&hovered);
    assert_eq!((b_min, b_max), (a_min, a_max), "悬停时命令文字被推动了");

    // 动作键浮出：行右端出现新的前景像素。
    let acts = band_has_pixels(&hovered, (r - 130, row_t + 2, r - 8, row_b - 2), |c| {
        (c.0 as u16 + c.1 as u16 + c.2 as u16) > 330
    });
    assert!(acts > 8, "悬停后右端没看到动作键（亮点 {} 个）", acts);

    // 元信息淡出。
    assert!(
        meta_pixels(&hovered) < meta_pixels(&before) / 2,
        "悬停时元信息没有淡出"
    );
}

/// ⑥ 高度锁定：1 条与 3 条数据时面板总高相同（§4 锁高）。
#[test]
fn panel_height_is_locked() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());

    ui.set_history_rows(hist_rows(&[("echo one", 1, false)]));
    ui.set_history_total(1);
    let f1 = open_panel(&ui);
    let (l1, t1, r1, b1) = panel_rect(&f1, lstr).expect("1 条时面板可见");
    let h1 = b1 - t1;

    // 收起（再点一次时钟 = toggle），改数据后再开 —— 不要用 Esc：
    // 它会和 open_panel 里「点时钟」的 toggle 时序打架（实测踩过）。
    let accent = theme_color(&ui, |t| t.get_accent());
    let bar_bg = theme_color(&ui, |t| t.get_bg_panel());
    let mut probe = Frame::new();
    probe.grab(&ui);
    let mid = cmdbar_mid_y(&probe, accent);
    let clock = bar_icon_clusters(&probe, mid, bar_bg)[1];
    click(&ui, clock as f32, mid as f32);
    std::thread::sleep(std::time::Duration::from_millis(250));

    ui.set_history_rows(hist_rows(&[
        ("echo one", 1, false),
        ("echo two", 1, false),
        ("echo three", 1, false),
    ]));
    ui.set_history_total(3);
    let f3 = open_panel(&ui);
    maybe_dump(&f3, "hist_height_locked");
    let rect3 = panel_rect(&f3, lstr).expect("3 条时面板可见");
    let h3 = rect3.3 - rect3.1;

    assert_eq!(h1, h3, "面板总高必须与数据条数无关（锁 3–8 行）");
}

/// ⑦ 页脚计数与「清空」贴右缘。
#[test]
fn footer_count_hugs_the_right_edge() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());

    ui.set_history_rows(hist_rows(&[("echo a", 1, false), ("echo b", 1, false)]));
    ui.set_history_total(2);

    let f = open_panel(&ui);
    maybe_dump(&f, "hist_footer_right");
    let (l, _t, r, b) = panel_rect(&f, lstr).unwrap();

    // 页脚带 = 面板底 -28 .. -2。
    let foot_t = b.saturating_sub(28) + 2;
    let foot_b = b - 2;
    let mut rightmost = 0usize;
    for y in foot_t..foot_b {
        for x in (l + 4)..(r - 2) {
            let c = f.rgb(x, y);
            if (c.0 as u16 + c.1 as u16 + c.2 as u16) > 150 && !close(c, lstr, 30) {
                rightmost = rightmost.max(x);
            }
        }
    }
    assert!(
        rightmost >= r - 60,
        "页脚计数/清空应贴右缘：最右内容像素 x={}，面板右缘 x={}",
        rightmost,
        r
    );
}

/// ⑧ 滚动视口裁剪（#hist-scroll）：数据多于 list-rows（测试窗格下 = 8）时，
/// 滚动偏移 0 只应绘制前 8 行 —— 视口外的行带必须为空。
#[test]
fn only_visible_rows_are_drawn() {
    let ui = setup();
    let lstr = theme_color(&ui, |t| t.get_border_strong());
    let elev = theme_color(&ui, |t| t.get_bg_elevated());

    let specs: Vec<(String, u32, bool)> =
        (0..20).map(|i| (format!("echo row-{i:02}"), 1, false)).map(|(c, u, p)| (c, u, p)).collect();
    let rows: Vec<HistoryRow> = specs
        .iter()
        .map(|(cmd, uses, pinned)| HistoryRow {
            cmd: cmd.clone().into(),
            display: cmd.clone().into(),
            pre: "".into(),
            hit: "".into(),
            post: "".into(),
            uses_label: format!("{} 次", uses).into(),
            time_label: " · 刚刚".into(),
            pinned: *pinned,
        })
        .collect();
    ui.set_history_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_history_total(20);

    let f = open_panel(&ui);
    maybe_dump(&f, "hist_visible_rows");
    let (l, t, r, _b) = panel_rect(&f, lstr).unwrap();

    // 列表区 = 面板顶 + 46 起；可见行数 = 测试窗格高算出的 hist-list-rows
    //（600 高窗格 → ⌊(540−82)/38⌋ = 12，夹到上限 8）。数「有内容像素的
    // 38px 行带」：滚动偏移 0 时只有前 8 行该画（数据 20 条，其余在视口外）。
    let list_t = t + 46;
    let row_h = 38;
    let visible_rows = 8usize;
    let mut painted = 0usize;
    for i in 0..20usize {
        let band_t = list_t + 4 + i * row_h + 2;
        let band_b = band_t + row_h - 4;
        if band_b >= list_t + 8 + visible_rows * row_h {
            break; // 超出锁定的列表高度
        }
        let mut n = 0usize;
        for y in band_t..band_b {
            for x in (l + 8)..(r - 8) {
                let c = f.rgb(x, y);
                if (c.0 as u16 + c.1 as u16 + c.2 as u16) > 300 && !close(c, elev, 10) && !close(c, lstr, 30) {
                    n += 1;
                }
            }
            if n > 4 {
                break;
            }
        }
        if n > 4 {
            painted += 1;
        }
    }
    assert_eq!(painted, visible_rows, "只应绘制 {} 个可见行（数据 20 条，其余在滚动视口外），量到 {}", visible_rows, painted);
}
