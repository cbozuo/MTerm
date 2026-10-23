//! (#cmdbar-scope-multi 2026-10-23) 命令栏「发送范围」的多选菜单。
//!
//! 对应的用户反馈（三条一起验）：
//!  1. 开了两个 tab，「全部标签页」却写 3 —— 旧绑定取的是 `tabs.length`，
//!     把看不见的 welcome 标签也算进去了。现在台数由 Rust 按
//!     `kind == "terminal"` 统计后下发（本测试验按钮确实读这个属性）；
//!  2. 菜单里的勾选图标 / 频道色点在行内**顶对齐**（固定高度子项在
//!     HorizontalLayout 里默认顶对齐），看起来"不居中"；
//!  3. 发送范围要支持多选（高保真 `.scope-menu.wide`）：勾选只改草稿、菜单
//!     不关，点「完成」才写回；按钮上出现叠排色点与合计台数。
//!
//! 运行：`cargo test --test cmdbar_scope_menu`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。

use mterm::ui::{AppWindow, PaneInfo, SendScope, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{Brush, ComponentHandle, LogicalPosition, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 1200;
const H: usize = 480;
/// 终端 pane 的几何（同 cmdbar_focus）：左侧栏 240 + 右侧栏 220，命令栏在
/// pane 底部，pane 必须内缩才能让两侧栏不去盖它的右端。
/// `PANE_X` 是 pane 相对 split 容器的 x，容器本身从左侧栏之后开始 ——
/// 命令栏的右缘 = SIDE_LEFT + PANE_W（实测 940，与像素扫描一致）。
const SIDE_LEFT: usize = 240;
const PANE_X: usize = 0;
const PANE_W: usize = 700;
const PANE_H: usize = H - 40;

const CH_A: (u8, u8, u8) = (230, 40, 40);
const CH_B: (u8, u8, u8) = (40, 120, 230);

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
        if x >= W || y >= H {
            return (0, 0, 0, 0);
        }
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

fn lum(c: (u8, u8, u8)) -> u32 {
    (c.0 as u32 + c.1 as u32 + c.2 as u32) / 3
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

/// 一个 pane + 一个终端 tab：命令栏只在 TerminalView 里存在。
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

    // 两个可送达的标签页 + 两个频道（A 两台、B 一台）。台数是 Rust 算好后
    // 下发的属性：UI 不该自己从 tabs.length 推（那样 welcome 会被算进去）。
    ui.set_sendable_tabs(2);
    ui.set_send_scopes(ModelRc::from(Rc::new(VecModel::from(vec![
        SendScope {
            letter: "A".into(),
            color: slint::Color::from_rgb_u8(CH_A.0, CH_A.1, CH_A.2),
            count: 2,
        },
        SendScope {
            letter: "B".into(),
            color: slint::Color::from_rgb_u8(CH_B.0, CH_B.1, CH_B.2),
            count: 1,
        },
    ]))));

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
    // 命令栏的输入框在 ScrollView(Flickable) 里，press/release 背靠背会被
    // DelayForwarding 吞掉；留 150ms 让转发真正发生（见 cmdbar_focus）。
    std::thread::sleep(std::time::Duration::from_millis(150));
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
    std::thread::sleep(std::time::Duration::from_millis(80));
}

/// 命令栏里 accent 色的 "$" 提示符 bbox —— 用它定位命令栏那一行的中线，
/// 不硬编码标题栏 / SFTP 的高度。
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

/// 菜单里所有横贯 250px 以上的亮线。外框宽 300（上下边框两端被 9px 圆角
/// 吃掉，实测有效长度 ~282），内部分隔线宽 288 —— 两者都会命中，靠
/// `menu_box` 的配对规则区分。终端是空的（没有输出），不会出现这么长的
/// 连续亮线；y 扫描区间只到命令栏上方 360px，避开顶栏。
fn menu_lines(f: &Frame, above_y: usize) -> Vec<usize> {
    let y0 = above_y.saturating_sub(360);
    let x0 = SIDE_LEFT + 10;
    let x1 = SIDE_LEFT + PANE_W;
    (y0..above_y.saturating_sub(8))
        .filter(|&y| (x0..x1).filter(|&x| lum(f.rgb(x, y)) > 55).count() > 250)
        .collect()
}

fn group_lines(lines: &[usize]) -> Vec<(usize, usize)> {
    let mut groups: Vec<(usize, usize)> = Vec::new();
    for &y in lines {
        match groups.last_mut() {
            Some(g) if y <= g.1 + 2 => g.1 = y,
            _ => groups.push((y, y)),
        }
    }
    groups
}

/// 一行里最长的那段亮线（start, len）。终端是空的，所以只有 UI 自己的边框 /
/// 气泡边缘会命中。
fn longest_run(f: &Frame, y: usize) -> (usize, usize) {
    let mut best = (0usize, 0usize);
    let mut run = 0usize;
    let mut run_start = 0usize;
    for x in 0..W {
        if lum(f.rgb(x, y)) > 55 {
            if run == 0 {
                run_start = x;
            }
            run += 1;
            if run > best.1 {
                best = (run_start, run);
            }
        } else {
            run = 0;
        }
    }
    best
}

/// 悬停汇总气泡：命令栏上方、宽度 100–240 的小面板（菜单是 300，靠这点区分）。
/// 返回 (x0, x1, top, bottom) —— 上下两条边都在 90px 以内。
fn tip_box(f: &Frame, above_y: usize) -> Option<(usize, usize, usize, usize)> {
    let mut edges: Vec<(usize, usize, usize)> = Vec::new(); // (y, x0, len)
    for y in (above_y.saturating_sub(110)..above_y.saturating_sub(4)).rev() {
        let (start, len) = longest_run(f, y);
        if (100..240).contains(&len) {
            edges.push((y, start, len));
        }
    }
    if edges.len() < 2 {
        return None;
    }
    // edges 是从下往上收集的：第一个是底边，最后一个是顶边
    let (bottom, bx, bl) = edges[0];
    let (top, tx, _) = edges[edges.len() - 1];
    let x0 = bx.min(tx);
    Some((x0, x0 + bl.max(edges[edges.len() - 1].2), top, bottom))
}

/// 菜单的外框（左右边 x + 上下边 y）。`None` = 菜单没开。
///
/// groups 里除了上下边框还有两道**内部分隔线**（「全部标签页」下方、
/// 汇总上方），所以不能简单取首尾。规则：在所有「高度落在 150–400」的配对里
/// 取**最高**的那一对 —— 外框一定比任何「内线到边框」的跨度更高。
fn menu_box(f: &Frame, above_y: usize) -> Option<(usize, usize, usize, usize)> {
    let groups = group_lines(&menu_lines(f, above_y));
    if groups.len() < 2 {
        return None;
    }
    let mut pick: Option<(usize, usize, usize)> = None; // (h, top, bottom)
    for i in 1..groups.len() {
        for j in 0..i {
            let top = groups[j].0;
            let bottom = groups[i].1;
            let h = bottom.saturating_sub(top);
            if (150..400).contains(&h) && pick.map_or(true, |(ph, _, _)| h > ph) {
                pick = Some((h, top, bottom));
            }
        }
    }
    let (_, top, bottom) = pick?;
    // 上边框那一行的连续亮线区间 = 菜单的左右边界（两端各被 9px 圆角吃掉，
    // 实测连续段 284 → 补回 8px/侧才是真实 300 的菜单外框）。
    let mut best = (0usize, 0usize);
    let mut run_start = 0usize;
    let mut run = 0usize;
    for x in 0..W {
        if lum(f.rgb(x, top)) > 55 {
            if run == 0 {
                run_start = x;
            }
            run += 1;
            if run > best.1 {
                best = (run_start, run);
            }
        } else {
            run = 0;
        }
    }
    if best.1 < 240 {
        return None;
    }
    const R: usize = 8; // border-radius 9px 的圆角让亮线两端各短 ~8px
    Some((
        best.0.saturating_sub(R),
        best.0 + best.1 + R,
        top,
        bottom,
    ))
}

/// 在 [y0, y1) × [x0, x1) 里找命中色的 bbox。
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
    for y in y0..y1 {
        for x in x0..x1 {
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

#[test]
fn scope_menu_rows_center_icons_and_multi_select_commits_on_done() {
    let ui = setup();
    let theme = ui.global::<Theme>();
    let accent = brush_solid(&theme.get_accent()).expect("accent 应为纯色");
    let accent = (accent.0, accent.1, accent.2);

    let mut rest = Frame::new();
    rest.grab(&ui);
    maybe_dump(&rest, "scope-rest");

    let (dx0, dy0, dx1, dy1) = dollar_bbox(&rest, accent)
        .unwrap_or_else(|| panic!("命令栏没渲染出来：找不到 accent 色的 \"$\" 提示符"));
    let mid_y = (dy0 + dy1) / 2;
    assert!(dx0 < W / 2 && dx1 - dx0 < 24, "\"$\" 位置异常：{:?}", (dx0, dy0, dx1, dy1));

    // 范围键常态透明、没有边框，只能按几何点：命令栏右端是发送键（64 宽、
    // 右内距 10），它左边 10px 才是范围键（宽 ≈ 115，探针逐个试过去）。
    let cmdbar_right = SIDE_LEFT + PANE_W;
    let send_left = cmdbar_right - 10 - 64;
    let mut scope_x = 0;
    for back in [40usize, 60, 80, 100] {
        scope_x = send_left - 10 - back;
        click(&ui, scope_x as f32, mid_y as f32);
        let mut f = Frame::new();
        f.grab(&ui);
        if menu_box(&f, mid_y).is_some() {
            break;
        }
    }

    let mut open = Frame::new();
    open.grab(&ui);
    maybe_dump(&open, "scope-open");

    let (menu_x0, menu_x1, menu_top, menu_bottom) = menu_box(&open, mid_y)
        .unwrap_or_else(|| panic!("发送范围菜单没打开（点的是 x={}）", scope_x));
    assert!(menu_bottom < mid_y, "菜单应向上展开（bottom={}，命令栏中线={}）", menu_bottom, mid_y);
    assert!(menu_bottom - menu_top > 150, "菜单太矮：{}–{}", menu_top, menu_bottom);
    assert!(
        menu_x1 - menu_x0 >= 290 && menu_x1 - menu_x0 <= 310,
        "菜单宽度应为 300（高保真 .scope-menu.wide），实测 {}",
        menu_x1 - menu_x0
    );

    // ── 行几何（与 Slint 侧一一对应）────────────────────────
    // 菜单 padding 6（边框 1px 之内）；行高 38；分隔线 1；频道区标题 26。
    let content_top = menu_top + 1 + 6;
    let row1_top = content_top;                 // 当前标签页
    let row2_top = row1_top + 38 + 1;           // 全部标签页
    let ch_title_top = row2_top + 38 + 1;
    let ch_a_top = ch_title_top + 26;
    let ch_b_top = ch_a_top + 38;

    // 1) 「当前标签页」默认勾选：复选框（15×15，accent 实心）必须落在行的
    //    垂直中线上，而不是顶在行首（旧实现的病）。
    let cb_x0 = menu_x0 + 1 + 6 + 9;
    let cb_x1 = cb_x0 + 15;
    let cb = color_bbox(&open, cb_x0, cb_x1, row1_top, row1_top + 38, |c| close(c, accent, 26))
        .unwrap_or_else(|| {
            panic!(
                "第 1 行没找到 accent 复选框（x {}–{}，y {}–{}）",
                cb_x0, cb_x1, row1_top, row1_top + 38
            )
        });
    let cb_mid = (cb.1 + cb.3) / 2;
    let row_mid = row1_top + 19;
    assert!(
        cb_mid.abs_diff(row_mid) <= 2,
        "复选框没垂直居中：行中心 {}，勾中心 {}（bbox {:?}）",
        row_mid, cb_mid, cb
    );

    // 2) 频道 A 的色点（8px）：同样要落在行中线上。
    let dot_x0 = menu_x0 + 1 + 6 + 9 + 15 + 9;
    let dot_x1 = dot_x0 + 8;
    let dot = color_bbox(&open, dot_x0, dot_x1, ch_a_top, ch_a_top + 38, |c| close(c, CH_A, 30))
        .unwrap_or_else(|| {
            panic!("频道 A 行没找到色点（x {}–{}，y {}–{}）", dot_x0, dot_x1, ch_a_top, ch_a_top + 38)
        });
    let dot_mid = (dot.1 + dot.3) / 2;
    let ch_row_mid = ch_a_top + 19;
    assert!(
        dot_mid.abs_diff(ch_row_mid) <= 2,
        "频道色点没垂直居中：行中心 {}，点中心 {}（bbox {:?}）",
        ch_row_mid, dot_mid, dot
    );

    // 3) 多选：勾 A、勾 B，菜单不能关（草稿态），点「完成」才写回。
    click(&ui, (menu_x0 + 150) as f32, (ch_a_top + 19) as f32);
    let mut after_a = Frame::new();
    after_a.grab(&ui);
    assert!(menu_box(&after_a, mid_y).is_some(), "勾选频道 A 后菜单被关掉了（多选时菜单不能关）");

    click(&ui, (menu_x0 + 150) as f32, (ch_b_top + 19) as f32);
    let mut after_b = Frame::new();
    after_b.grab(&ui);
    assert!(menu_box(&after_b, mid_y).is_some(), "勾选频道 B 后菜单被关掉了");
    maybe_dump(&after_b, "scope-draft");

    // 「完成」在菜单最后一行：行高 36、按钮 64×28、右内距 6。
    let done_y = menu_bottom - 1 - 6 - 36 + 4 + 14;
    let done_x = menu_x1 - 1 - 6 - 32;
    click(&ui, done_x as f32, done_y as f32);

    let mut committed = Frame::new();
    committed.grab(&ui);
    maybe_dump(&committed, "scope-committed");
    assert!(menu_box(&committed, mid_y).is_none(), "点「完成」后菜单应关闭");

    // 按钮上应出现两个叠排色点（A 红、B 蓝）：在范围键区域内找这两种颜色。
    let btn_x0 = menu_x1.saturating_sub(150);
    let btn_y0 = mid_y.saturating_sub(12);
    let btn_y1 = mid_y + 12;
    let red = color_bbox(&committed, btn_x0, menu_x1, btn_y0, btn_y1, |c| close(c, CH_A, 30));
    let blue = color_bbox(&committed, btn_x0, menu_x1, btn_y0, btn_y1, |c| close(c, CH_B, 30));
    assert!(red.is_some(), "写回后范围键上没出现频道 A 的色点");
    assert!(blue.is_some(), "写回后范围键上没出现频道 B 的色点");

    // 4) 悬停汇总：只把鼠标移到范围键上（不点），要弹出一枚比菜单窄的小气泡，
    //    且因为选中两个频道，气泡里是「标题 + 两行频道」—— 高度明显超过单行
    //    （单行 ~30，两行频道 ~60）。
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(scope_x as f32, mid_y as f32),
    });
    std::thread::sleep(std::time::Duration::from_millis(120));
    let mut hover = Frame::new();
    hover.grab(&ui);
    maybe_dump(&hover, "scope-hover");
    let (tx0, tx1, ttop, tbottom) = tip_box(&hover, mid_y)
        .unwrap_or_else(|| panic!("范围键悬停后没有汇总气泡（上面 110px 内没找到 100–240 宽的亮线面板）"));
    let w = tx1 - tx0;
    let h = tbottom - ttop;
    assert!(
        (100..260).contains(&w),
        "气泡宽度不合理：{}（应比 300 的菜单窄）",
        w
    );
    assert!(
        h >= 45,
        "气泡太矮（{}px）：多选时应该逐行列出频道，只有标题才会这么矮（{}–{}）",
        h,
        ttop,
        tbottom
    );
}
