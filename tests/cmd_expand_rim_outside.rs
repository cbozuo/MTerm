//! (#cmdbar-expand-rim / #scope-menu-outside / #cm-confirm-inplace 2026-10-25)
//! 用户三张截图反馈的渲染级回归：
//!
//!  1. 弹出编辑面板（命令栏 ↗）的外框**断成三截**：Rectangle 的 border 画在
//!     background 之上、子元素之下，主体两栏的不透明底把左右竖线整段盖掉，
//!     只剩标题栏与底栏那两段露出来；
//!  2. 弹窗底部「发送范围」菜单开着时，点菜单外（面板内空白 / 终端）菜单不关；
//!  3. 命令卡的删除二段式里，红底「确认」跑到卡片中间（五个图标钮 `visible:
//!     false` 仍占宽），应落在删除钮那个位置。
//!
//! 运行：`cargo test --test cmd_expand_rim_outside`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。

use mterm::ui::{
    AppWindow, CmCard, CmGroupRow, PaneInfo, SendScope, TabInfo, TerminalState, Theme,
};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{Brush, ComponentHandle, LogicalPosition, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 1280;
const H: usize = 800;
const SIDE_LEFT: usize = 240;
const PANE_X: usize = 0;
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

fn lum(c: (u8, u8, u8)) -> u32 {
    (c.0 as u32 + c.1 as u32 + c.2 as u32) / 3
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: u8) -> bool {
    a.0.abs_diff(b.0) <= tol && a.1.abs_diff(b.1) <= tol && a.2.abs_diff(b.2) <= tol
}

fn solid(b: &Brush) -> Option<(u8, u8, u8)> {
    match b {
        Brush::SolidColor(c) => Some((c.red(), c.green(), c.blue())),
        _ => None,
    }
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

    ui.set_sendable_tabs(2);
    ui.set_send_scopes(ModelRc::from(Rc::new(VecModel::from(vec![SendScope {
        letter: "A".into(),
        color: slint::Color::from_rgb_u8(230, 40, 40),
        count: 2,
    }]))));

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
    std::thread::sleep(std::time::Duration::from_millis(150));
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
    std::thread::sleep(std::time::Duration::from_millis(80));
}

/// 命令栏那一行的中线：找 accent 色的 "$" 提示符。
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

/// 一整行里最长的亮段 (起点, 长度)。
fn longest_run(f: &Frame, y: usize) -> (usize, usize) {
    let mut best = (0usize, 0usize);
    let mut run = 0usize;
    let mut start = 0usize;
    for x in 0..W {
        if lum(f.rgb(x, y)) > 55 {
            if run == 0 {
                start = x;
            }
            run += 1;
            if run > best.1 {
                best = (start, run);
            }
        } else {
            run = 0;
        }
    }
    best
}

/// 弹窗面板的顶边：宽度落在 560..700 的最上面那一行 → (y, x0, len)。
fn panel_top_edge(f: &Frame) -> Option<(usize, usize, usize)> {
    (0..H)
        .map(|y| (y, longest_run(f, y)))
        .find(|(_, (_, len))| (560..700).contains(len))
        .map(|(y, (x0, len))| (y, x0, len))
}

/// 在 [x_from, x_to) 里找一条"比左右各 6px 都亮"的细竖线（1–3px 的描边）。
fn find_vline(f: &Frame, y: usize, x_from: usize, x_to: usize) -> Option<usize> {
    (x_from.max(6)..x_to.min(W - 7)).find(|&x| {
        let c = lum(f.rgb(x, y));
        c > lum(f.rgb(x - 6, y)) + 8 && c > lum(f.rgb(x + 6, y)) + 8
    })
}

#[test]
fn command_card_confirm_button_sits_where_delete_was() {
    let ui = setup();
    let theme = ui.global::<Theme>();
    let danger = solid(&theme.get_danger()).expect("danger 应为纯色");

    let card = |key: &str, name: &str| CmCard {
        key: key.into(),
        name: name.into(),
        note: name.into(),
        command: "echo 111".into(),
        group_key: "__all".into(),
        group_display: "默认分组".into(),
        send_enter: true,
        picked: false,
        can_up: false,
        can_down: false,
        show_group: true,
    };

    ui.set_cm_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        CmGroupRow {
            key: "__all".into(),
            display: "全部命令".into(),
            count: 2,
            hit: 2,
            kind: 0,
        },
    ]))));
    ui.set_cm_menu_groups(ModelRc::from(Rc::new(VecModel::from(vec![CmGroupRow {
        key: "__all".into(),
        display: "默认分组".into(),
        count: 2,
        hit: 2,
        kind: 1,
    }]))));
    ui.set_cm_cards(ModelRc::from(Rc::new(VecModel::from(vec![
        card("k1", "111"),
        card("k2", "222"),
    ]))));
    ui.set_cm_total_count(2);
    ui.set_cm_cur_name("全部命令".into());
    ui.set_cm_cur_count(2);
    ui.set_cm_cur_index(0);
    ui.set_cm_view_count(2);
    ui.set_cm_subtitle("2 条命令".into());
    ui.set_cm_search_hits(2);
    ui.set_cm_has_any(true);
    ui.set_quick_cmd_manage_open(true);

    std::thread::sleep(std::time::Duration::from_millis(120));
    let mut f0 = Frame::new();
    f0.grab(&ui);
    maybe_dump(&f0, "cm-open");

    // 卡片的 1px 边框（Theme.mgr-line）是最好认的地标：圆角 9px 的矩形，
    // 上下边就是 200..300 长的横线，且同一行会有**两条**（2 列卡片）——
    // 只出现一条的都不是卡片（搜索框下沿、右栏竖线等）。卡片底色 popup-field
    // 不能用：窗口左上角标题栏是同一种深色，全屏扫会把整个窗口框进来。
    let line = solid(&theme.get_mgr_line()).expect("mgr-line 应为纯色");
    let mut rows: Vec<(usize, usize, usize)> = Vec::new(); // (y, x0, len)
    for y in 150..H - 80 {
        let hits: Vec<(usize, usize)> = runs_color(&f0, y, line, 3)
            .into_iter()
            .filter(|(_, l)| (200..300).contains(l))
            .collect();
        if hits.len() >= 2 {
            rows.push((y, hits[0].0, hits[0].1));
        }
    }
    assert!(
        rows.len() >= 2,
        "没找到命令卡的外框（mgr-line={:?}，命中 {} 行）",
        line,
        rows.len()
    );
    let (card_top, bx0, blen) = rows[0];
    let card_bottom = rows[rows.len() - 1].0;
    let (card_x0, card_x1) = (bx0, bx0 + blen);
    let bbox = (card_x0, card_top, card_x1, card_bottom);
    eprintln!("card bbox = {:?} (line={:?})", bbox, line);
    assert!(
        card_bottom - card_top > 60,
        "卡片高度异常：{}–{}",
        card_top,
        card_bottom
    );

    // 鼠标移到卡片右下角让操作组淡入，再点删除钮（操作组最右、20×20、
    // 距卡片内边 11 / 8 → 中心 = (右缘-21, 底缘-18)）。
    let (cx0, cy0, cx1, cy1) = bbox;
    let del_x = cx1.saturating_sub(21);
    let del_y = cy1.saturating_sub(18);
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(del_x as f32, del_y as f32),
    });
    std::thread::sleep(std::time::Duration::from_millis(220));
    let mut hovered = Frame::new();
    hovered.grab(&ui);
    maybe_dump(&hovered, "cm-hover");

    click(&ui, del_x as f32, del_y as f32);
    let mut armed = Frame::new();
    armed.grab(&ui);
    maybe_dump(&armed, "cm-armed");

    // 红底确认钮：danger 实心、宽 >= 30、高 20。只在卡片自己的纵坐标范围内找，
    // 否则底栏那行红色的「待删除」提示文字会一起被算进 bbox。
    let ok = color_bbox(&armed, cx0, cx1 + 1, cy0, cy1 + 1, |c| close(c, danger, 26))
        .unwrap_or_else(|| panic!("点删除后没出现红底「确认」钮（card={:?}）", bbox));
    eprintln!("confirm bbox = {:?} card = {:?}", ok, bbox);
    let gap_right = cx1.saturating_sub(ok.2);
    assert!(
        gap_right <= 14,
        "「确认」钮没有落在删除钮的位置：卡片右缘 {}，确认钮右缘 {}（相距 {}）",
        cx1,
        ok.2,
        gap_right
    );
    assert!(
        ok.2 - ok.0 >= 28,
        "「确认」钮宽度异常：{}–{}",
        ok.0,
        ok.2
    );
}

/// 一行里所有「命中某色」的连续段 (起点, 长度)。
fn runs_color(f: &Frame, y: usize, c: (u8, u8, u8), tol: u8) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut run = 0usize;
    let mut start = 0usize;
    for x in 0..W {
        if close(f.rgb(x, y), c, tol) {
            if run == 0 {
                start = x;
            }
            run += 1;
        } else {
            if run > 0 {
                out.push((start, run));
            }
            run = 0;
        }
    }
    if run > 0 {
        out.push((start, run));
    }
    out
}

fn color_bbox(
    f: &Frame,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    hit: impl Fn((u8, u8, u8)) -> bool,
) -> Option<(usize, usize, usize, usize)> {
    let mut min = (usize::MAX, usize::MAX);
    let mut max = (0usize, 0usize);
    let mut found = false;
    for y in y0..y1.min(H) {
        for x in x0..x1.min(W) {
            if hit(f.rgb(x, y)) {
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
