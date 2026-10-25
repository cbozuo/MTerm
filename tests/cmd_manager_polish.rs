//! (#cm-polish 2026-10-25) 命令管理第二轮用户反馈的渲染证据 + 守卫。
//!
//!  1. 卡片底部分组 chip：文字不居中、右侧空一截；
//!  2. 二级编辑卡「名称 / 分组」两个输入框不等宽；
//!  3. 左栏「我的分组」计数徽章被拉成胶囊，应为正圆、数字居中；
//!  4. 页脚底色偏黑、页脚「完成」与头部「新建命令」不同高；
//!  5. 「移动至」丢了 ▾ 符号，菜单没对准按钮中心；
//!  6. 选中卡点删除后，「确认」要落在删除图标的位置。
//!
//! 运行：`cargo test --test cmd_manager_polish`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。

use mterm::ui::{AppWindow, CmCard, CmGroupRow, PaneInfo, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{Brush, ComponentHandle, LogicalPosition, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 1280;
const H: usize = 800;

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

fn runs_color(f: &Frame, y: usize, c: (u8, u8, u8), tol: u8) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
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

fn group(key: &str, display: &str, count: i32, kind: i32) -> CmGroupRow {
    CmGroupRow {
        key: key.into(),
        display: display.into(),
        count,
        hit: count,
        kind,
    }
}

fn card(key: &str, name: &str, group: &str, picked: bool) -> CmCard {
    CmCard {
        key: key.into(),
        name: name.into(),
        note: "替换 <svc> 后发送".into(),
        command: "systemctl status <svc>".into(),
        group_key: group.into(),
        group_display: "默认分组".into(),
        send_enter: true,
        picked,
        can_up: false,
        can_down: false,
        show_group: true,
    }
}

fn setup(picked: bool) -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));

    let term = TerminalState { id: "probe-term".into(), ..Default::default() };
    ui.set_terminals(ModelRc::from(Rc::new(VecModel::from(vec![term]))));
    let tab = TabInfo { id: "probe-term".into(), ..Default::default() };
    let pane = PaneInfo {
        id: 0,
        x: 0.0,
        y: 0.0,
        w: 700.0,
        h: (H - 40) as f32,
        active_id: "probe-term".into(),
        focused: true,
        reserve_right: 0.0,
        tabs: ModelRc::from(Rc::new(VecModel::from(vec![tab]))),
    };
    ui.set_panes(ModelRc::from(Rc::new(VecModel::from(vec![pane]))));
    ui.set_active_tab_id("probe-term".into());

    ui.set_cm_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        group("__all", "全部命令", 35, 0),
        group("", "默认分组", 2, 1),
        group("g1", "系统与资源", 6, 2),
        group("g2", "网络排查", 6, 2),
        group("g3", "进程与服务", 4, 2),
    ]))));
    ui.set_cm_menu_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        group("", "默认分组", 2, 1),
        group("g1", "系统与资源", 6, 2),
        group("g3", "进程与服务", 4, 2),
    ]))));
    ui.set_cm_cards(ModelRc::from(Rc::new(VecModel::from(vec![
        card("k1", "服务状态", "g3", picked),
        card("k2", "重启服务", "g3", false),
    ]))));
    ui.set_cm_total_count(35);
    ui.set_cm_custom_count(4);
    ui.set_cm_cur_name("进程与服务".into());
    ui.set_cm_cur_count(4);
    ui.set_cm_cur_index(4);
    ui.set_cm_view_count(2);
    ui.set_cm_subtitle("4 条命令".into());
    ui.set_cm_search_hits(2);
    ui.set_cm_has_any(true);
    ui.set_cm_picked_count(if picked { 1 } else { 0 });
    ui.set_cm_selected("g3".into());
    ui.set_quick_cmd_manage_open(true);

    std::thread::sleep(std::time::Duration::from_millis(120));
    ui
}

/// 一帧里某色构成的横线（200..300 长）= 卡片上下边；返回卡片 bbox。
fn card_box(f: &Frame, line: (u8, u8, u8)) -> (usize, usize, usize, usize) {
    let mut rows: Vec<(usize, usize, usize)> = Vec::new();
    for y in 150..H - 80 {
        let hits: Vec<(usize, usize)> = runs_color(f, y, line, 3)
            .into_iter()
            .filter(|(_, l)| (180..300).contains(l))
            .collect();
        if hits.len() >= 2 {
            rows.push((y, hits[0].0, hits[0].1));
        }
    }
    assert!(rows.len() >= 2, "没找到卡片外框（mgr-line={:?}）", line);
    let (top, x0, len) = rows[0];
    (x0, top, x0 + len, rows[rows.len() - 1].0)
}

#[test]
fn manager_chip_badge_footer_and_editor_fields() {
    let ui = setup(false);
    let theme = ui.global::<Theme>();
    let accent = solid(&theme.get_accent()).expect("accent 应为纯色");
    let accent_soft = solid(&theme.get_accent_soft()).expect("accent-soft 应为纯色");

    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "pm-cards");

    // ── 1. 卡片 chip：文字左右留白要接近（居中），不能右边空一大截 ──
    let chip_bg = solid(&theme.get_bg_active()).expect("bg-active 应为纯色");
    // chip 在卡片底行；用「比卡片底更亮的圆角块」定位：扫卡片下半部分
    let (cx0, _, cx1, cy1) = card_box(&f, solid(&theme.get_mgr_line()).unwrap());
    eprintln!("card = {}..{} bottom {}", cx0, cx1, cy1);
    let chip = color_bbox(&f, cx0, cx1 + 1, cy1 - 30, cy1 + 1, |c| close(c, chip_bg, 6))
        .unwrap_or_else(|| panic!("卡片底部没找到分组 chip（bg-active={:?}）", chip_bg));
    eprintln!("chip bbox = {:?}", chip);
    // chip 内的文字（text-muted）在 chip 行中线上的左右第一个/最后一个命中
    let cy_mid = (chip.1 + chip.3) / 2;
    let text_px: Vec<usize> = (chip.0..=chip.2)
        .filter(|&x| lum(f.rgb(x, cy_mid)) > lum(chip_bg) + 40)
        .collect();
    assert!(!text_px.is_empty(), "chip 里没找到文字像素");
    let left_pad = text_px[0] - chip.0;
    let right_pad = chip.2 - text_px[text_px.len() - 1];
    assert!(
        left_pad.abs_diff(right_pad) <= 12,
        "chip 文字没居中：左留白 {}，右留白 {}（chip {:?}）",
        left_pad,
        right_pad,
        chip
    );

    // ── 3. 「我的分组」计数徽章：正圆 16×16、数字居中 ──
    // accent-soft 是半透明（叠在侧栏底上），不能按固有色匹配；改按"偏青"判据
    // （绿/蓝明显高于红）在这一行里找那块小圆。
    let is_teal = |c: (u8, u8, u8)| {
        c.1 as i32 > c.0 as i32 + 8 && c.2 as i32 > c.0 as i32 + 8
    };
    let badge = color_bbox(&f, 260, 484, 250, 300, is_teal)
        .unwrap_or_else(|| panic!("左栏「我的分组」行没找到计数徽章（偏青像素）"));
    eprintln!("badge bbox = {:?}", badge);
    let bw = badge.2 - badge.0 + 1;
    let bh = badge.3 - badge.1 + 1;
    assert!(
        (12..=22).contains(&bw) && (12..=22).contains(&bh) && bw.abs_diff(bh) <= 2,
        "计数徽章应是 16×16 的正圆，实测 {}×{}（bbox {:?}）",
        bw,
        bh,
        badge
    );
    let _ = accent_soft;

    // ── 4. 页脚：底色 = 弹窗底；「完成」按钮 30 高 ──
    let popup_bg = solid(&theme.get_popup_bg()).expect("popup-bg 应为纯色");
    let foot_y = H - 1;
    let _ = foot_y;
    // 弹窗 760×560 居中 → 底边 y = (800-560)/2 + 560 = 680，页脚 48 高
    let foot_top = 680 - 48;
    let foot_col = f.rgb(300, foot_top + 24);
    assert!(
        close(foot_col, popup_bg, 4),
        "页脚底色应与弹窗底同色 {:?}，实测 {:?}",
        popup_bg,
        foot_col
    );
    let done = color_bbox(&f, 900, 1020, foot_top, 680, |c| close(c, accent, 10))
        .expect("页脚没找到「完成」按钮（accent 实心）");
    eprintln!("done btn = {:?} h={}", done, done.3 - done.1);
    assert!(
        (28..=32).contains(&(done.3 - done.1 + 1)),
        "「完成」按钮高度应为 30，实测 {}",
        done.3 - done.1 + 1
    );

    // ── 2. 二级编辑卡：两个输入框等宽 ──
    // 点头部「新建命令」（弹窗 x0=260，头部按钮约 x 866..962 / y 131..166）
    click(&ui, 900.0, 148.0);
    let mut ed = Frame::new();
    ed.grab(&ui);
    maybe_dump(&ed, "pm-editor");

    // 输入框边框 = mgr-line-strong；找两段 150..250 长的横线（两框的顶边）
    let line_strong = solid(&theme.get_mgr_line_strong()).expect("mgr-line-strong 应为纯色");
    let mut edges: Vec<(usize, usize, usize)> = Vec::new();
    for y in 250..640 {
        for (sx, sl) in runs_color(&ed, y, line_strong, 4) {
            if (120..260).contains(&sl) {
                edges.push((y, sx, sl));
            }
        }
    }
    eprintln!("editor field edges = {:?}", edges);
    assert!(
        edges.len() >= 2,
        "编辑卡里没找到两个输入框的边框（mgr-line-strong={:?}）",
        line_strong
    );
    // 顶边那一行：两条线
    let top_y = edges[0].0;
    let same_row: Vec<&(usize, usize, usize)> = edges.iter().filter(|e| e.0 == top_y).collect();
    assert!(same_row.len() >= 2, "两个输入框不在同一行：{:?}", same_row);
    let w0 = same_row[0].2;
    let w1 = same_row[1].2;
    assert!(
        w0.abs_diff(w1) <= 2,
        "名称/分组两个输入框不等宽：{} vs {}",
        w0,
        w1
    );
}

#[test]
fn batch_move_menu_is_centered_and_confirm_sits_at_delete() {
    let ui = setup(true);
    let theme = ui.global::<Theme>();
    let danger = solid(&theme.get_danger()).unwrap();

    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "pm-picked");

    // 批量条在右栏底部（y ≈ 弹窗底 680 - 48 - 10 - 42）
    // 「移动至」按钮：批量条右侧倒数第三个（全选 / 移动至 / 删除 / ×）
    let batch_y = 680 - 48 - 10 - 21;
    // 从右往左找按钮的 1px 边框（mgr-line-strong）不好认，改用「文字 + 箭头」：
    // 直接按几何点：右内距 10，依次 × (24) / 删除 (54) / 移动至 (~72) / 全选
    let batch_right: f32 = 1020.0 - 10.0 - 10.0; // 弹窗右缘 - 边距 - 内距
    let move_x: f32 = batch_right - 24.0 - 6.0 - 54.0 - 8.0 - 36.0;
    click(&ui, move_x, batch_y as f32);
    let mut m = Frame::new();
    m.grab(&ui);
    maybe_dump(&m, "pm-move-menu");

    // 菜单：左右各一条 1px 竖边框（border-strong），底色与弹窗底同色 —— 只能
    // 按"比左右 6px 都亮"的细竖线找，不能按颜色成块扫。
    let mut cols: Vec<usize> = Vec::new();
    for y in [520usize, 540, 560] {
        let mut hit: Vec<usize> = Vec::new();
        for x in 320..1000 {
            let c = lum(m.rgb(x, y));
            if c > lum(m.rgb(x - 6, y)) + 8 && c > lum(m.rgb(x + 6, y)) + 8 {
                hit.push(x);
            }
        }
        if hit.len() >= 2 {
            cols = hit;
            break;
        }
    }
    assert!(
        cols.len() >= 2,
        "「移动至」菜单没弹出来（y 520..560 找不到两条竖边框）"
    );
    let menu = (cols[0], 0usize, *cols.last().unwrap(), 0usize);
    eprintln!("move menu x = {}..{}", menu.0, menu.2);
    let menu_mid = (menu.0 + menu.2) / 2;
    assert!(
        (menu_mid as i64 - move_x as i64).abs() <= 20,
        "菜单没有对准「移动至」按钮中心：菜单中心 {}，按钮中心 {}（菜单 {}..{}）",
        menu_mid,
        move_x,
        menu.0,
        menu.2
    );
    assert!(
        menu.2 <= 1020 && menu.0 >= 260,
        "「移动至」菜单越出弹窗: {}..{}",
        menu.0,
        menu.2
    );

    // ── 6. 选中卡点删除 → 确认钮落在删除图标位置（卡片右下角）──
    // 卡片网格几何在同一窗口尺寸下是固定的（1280×800 / 弹窗 760×560）：
    // 第一张卡 = x 503..750、y 228..335（含 1px 外框），删除钮中心在右下角。
    let (cx1, cy1) = (750usize, 335usize);
    let del_x = cx1.saturating_sub(21);
    let del_y = cy1.saturating_sub(18);
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(del_x as f32, del_y as f32),
    });
    std::thread::sleep(std::time::Duration::from_millis(220));
    click(&ui, del_x as f32, del_y as f32);
    let mut armed = Frame::new();
    armed.grab(&ui);
    maybe_dump(&armed, "pm-armed");

    let ok = color_bbox(&armed, 503, cx1 + 1, 228, cy1 + 1, |c| close(c, danger, 26))
        .unwrap_or_else(|| panic!("选中卡点删除后没出现红底确认钮（card 503..{cx1}）"));
    eprintln!("confirm button = {:?}", ok);
    assert!(
        cx1.saturating_sub(ok.2) <= 14,
        "确认钮没落在删除图标位置：卡片右缘 {}，确认钮右缘 {}",
        cx1,
        ok.2
    );
    assert!(
        cy1.saturating_sub(ok.3) <= 12,
        "确认钮没贴卡片底缘：卡片底 {}，确认钮底 {}",
        cy1,
        ok.3
    );
    assert!(
        ok.0 > 503 + (cx1 - 503) / 2,
        "确认钮跑到卡片左半区了（{:?}），应在右端的删除图标位置",
        ok
    );
}
