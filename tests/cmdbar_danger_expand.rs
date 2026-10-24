//! (#cmdbar-danger / #cmdbar-expand 2026-10-23) 命令栏的三件事：
//!
//!  1. 命令栏上下各有一条 1px 分隔线（用户截图：「这个地方是少线框吧」）；
//!  2. 输入命中危险模式（rm -rf / mkfs / dd …）→ 发送键变红「仍然发送」且
//!     命令栏上边缘浮出告警条（高保真屏 07）；
//!  3. 输入框右端的 ↗ 钮把命令升成弹出编辑窗口（高保真屏 06）。
//!
//! 运行：`cargo test --test cmdbar_danger_expand`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。

use mterm::ui::{AppWindow, PaneInfo, SendScope, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 1200;
const H: usize = 480;
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

/// 逐字符敲进当前焦点控件。
fn type_text(ui: &AppWindow, s: &str) {
    for ch in s.chars() {
        let t: slint::SharedString = ch.to_string().into();
        ui.window().dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        ui.window().dispatch_event(WindowEvent::KeyReleased { text: t });
        std::thread::sleep(std::time::Duration::from_millis(6));
    }
    std::thread::sleep(std::time::Duration::from_millis(60));
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

#[test]
fn cmdbar_has_frame_danger_turns_send_red_and_expand_opens_dialog() {
    let ui = setup();
    let theme = ui.global::<Theme>();
    let accent = theme.get_accent();
    let accent = match accent {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("accent 应为纯色"),
    };
    let danger = theme.get_danger_strong();
    let danger = (danger.red(), danger.green(), danger.blue());

    let mut rest = Frame::new();
    rest.grab(&ui);
    maybe_dump(&rest, "danger-rest");
    let mid_y = cmdbar_mid_y(&rest, accent);

    // ── 1. 命令栏上下边框 ───────────────────────────────────
    // 在范围键与发送键之间的空白列（x=860）测：从命令栏中线向上/下走到
    // 「底色块」的边界，边界外那一像素必须是更亮的 line（zone-sidebar-line）。
    // 没有这条线时，边界外直接是终端底色（更暗），或者根本没有独立的一行。
    let col = SIDE_LEFT + 620;
    let bg = rest.rgb(col, mid_y);
    assert!(lum(bg) > 30, "x={} 中线不在命令栏底色上（{:?}）", col, bg);

    let mut top = mid_y;
    while top > 0 && close(rest.rgb(col, top - 1), bg, 3) {
        top -= 1;
    }
    let top_line = rest.rgb(col, top.saturating_sub(1));
    assert!(
        !close(top_line, bg, 3) && lum(top_line) > lum(bg) + 6,
        "命令栏上边没有分隔线：底色 {:?}，上一行 {:?}（y={}）",
        bg,
        top_line,
        top.saturating_sub(1)
    );

    let mut bottom = mid_y;
    while bottom + 1 < H && close(rest.rgb(col, bottom + 1), bg, 3) {
        bottom += 1;
    }
    let bottom_line = rest.rgb(col, bottom + 1);
    assert!(
        !close(bottom_line, bg, 3),
        "命令栏下边没有分隔线：底色 {:?}，下一行 {:?}（y={}）",
        bg,
        bottom_line,
        bottom + 1
    );

    // ── 2. 危险命令：发送键变红 + 告警条浮出 ─────────────────
    click(&ui, (SIDE_LEFT + 160) as f32, mid_y as f32);   // 聚焦输入框
    type_text(&ui, "rm -rf /tmp/x");
    // 发送键的 background 有 `animate 110ms`：只 sleep 会量到动画起点（旧色），
    // 必须持续派发事件推进帧（cmdbar_focus 里同款处理）。
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new((SIDE_LEFT + 160) as f32, mid_y as f32),
        });
    }

    let mut danger_frame = Frame::new();
    danger_frame.grab(&ui);
    maybe_dump(&danger_frame, "danger-on");

    // 发送键是实心 danger-strong，在命令栏右端（x 866..930）
    let mut red_px = 0;
    for y in (mid_y - 17)..(mid_y + 17) {
        for x in (SIDE_LEFT + 626)..(SIDE_LEFT + 690) {
            if close(danger_frame.rgb(x, y), danger, 24) {
                red_px += 1;
            }
        }
    }
    assert!(
        red_px > 300,
        "输入 rm -rf 后发送键没有变红（命中 {} px）",
        red_px
    );

    // 告警条：命令栏上边缘 (y = mid_y-24-6-30 .. mid_y-24-6) 的红边框
    let mut warn = false;
    for y in (mid_y - 60)..(mid_y - 20) {
        for x in (SIDE_LEFT + 40)..(SIDE_LEFT + 660) {
            if close(danger_frame.rgb(x, y), danger, 40) {
                warn = true;
                break;
            }
        }
        if warn {
            break;
        }
    }
    assert!(warn, "危险命令没有浮出告警条（命令栏上方 60px 内找不到红色像素）");

    // ── 3. 弹出编辑 ─────────────────────────────────────────
    // ↗ 钮的 x 不能按「pane 右缘 - 若干」硬算（范围键宽度随文案变）。先找
    // **输入框右缘**：输入框内部底色与命令栏底色不同，整列都不是命令栏色；
    // 命令栏底色区是一整列都是 (41,44,52)。钮右缘距输入框右缘 32、宽 26。
    let cbar = rest.rgb(col, mid_y);
    let y0 = mid_y.saturating_sub(9);
    let y1 = mid_y + 9;
    let mut input_right = 0;
    // 只在输入框可能的地盘里扫（SIDE_LEFT+380..500）：再往右是范围键，它的
    // 1px 边框也是「整列非命令栏底色」，会把起点带偏。
    for x in (SIDE_LEFT + 380..SIDE_LEFT + 500).rev() {
        let hits = (y0..y1).filter(|&y| !close(rest.rgb(x, y), cbar, 6)).count();
        if hits == y1 - y0 {
            input_right = x;
            break;
        }
    }
    assert!(input_right > SIDE_LEFT + 300, "找不到输入框右缘");
    // (#cmdbar-clear-first 2026-10-23) 右端现在是 × ⚡ 🕘 │ ↗ 五个钮：× 移到最左（-119），
    // ↗ 仍在最右（-32，26 宽 → 中心 -19）。
    let exp_x = input_right - 19;
    click(&ui, exp_x as f32, mid_y as f32);

    let mut expand = Frame::new();
    expand.grab(&ui);
    maybe_dump(&expand, "expand-on");

    // 遮罩：取终端区**面板之外**的一点（面板宽 620 居中，左右各留 40），
    // 那里只有 42% 黑遮罩，应该明显变暗。
    let before = lum(rest.rgb(SIDE_LEFT + 20, 120));
    let after = lum(expand.rgb(SIDE_LEFT + 20, 120));
    assert!(
        after < before.saturating_mul(4) / 5,
        "点 ↗ 后没有遮罩（终端亮度 {} → {}）",
        before,
        after
    );

    // 面板：窗口中心应出现比遮罩亮得多的一块（popup-bg）
    let center = lum(expand.rgb(W / 2 - 120, H / 2));
    assert!(
        center > after + 20,
        "弹出编辑面板没出现（中心亮度 {} vs 遮罩 {}）",
        center,
        after
    );

    // ── 4. 弹窗里的范围键独立开合 ───────────────────────────
    // 以前两处共用 scope-open：点弹窗那个键，命令栏上方也会同时弹出一个菜单
    // （用户截图：两个「当前标签页 / 全部标签页 / 完成」叠着）。
    // 面板几何从像素量（宽度 620 的外框线），别硬算 —— TerminalView 的实际
    // 高度受 pane/布局影响，按窗口尺寸推会偏 30px，点到 body 上。
    let mut panel_rows: Vec<(usize, usize, usize)> = Vec::new(); // (y, x0, len)
    for y in 0..H {
        let mut run = 0usize;
        let mut best = 0usize;
        let mut start = 0usize;
        let mut bs = 0usize;
        for x in 0..W {
            if lum(expand.rgb(x, y)) > 55 {
                if run == 0 {
                    start = x;
                }
                run += 1;
                if run > best {
                    best = run;
                    bs = start;
                }
            } else {
                run = 0;
            }
        }
        if (560..680).contains(&best) {
            panel_rows.push((y, bs, best));
        }
    }
    assert!(panel_rows.len() >= 2, "找不到弹窗面板外框（{:?}）", panel_rows);
    let panel_x = panel_rows[0].1;
    let panel_top = panel_rows[0].0;
    let panel_bottom = panel_rows[panel_rows.len() - 1].0;

    // 弹窗底部的范围键：foot 56 高、键 34 高居中 → 中心 = 底边 - 28。
    let scope_y = panel_bottom - 28;
    click(&ui, (panel_x + 12 + 50) as f32, scope_y as f32);

    let mut menu_open = Frame::new();
    menu_open.grab(&ui);
    maybe_dump(&menu_open, "expand-scope-menu");

    // 先确认菜单**真的**在弹窗里开了（300 宽的外框，圆角会吃掉两侧 ~8px）
    let mut in_dialog = 0;
    for y in panel_top..panel_bottom {
        let mut run = 0usize;
        let mut best = 0usize;
        for x in panel_x..(panel_x + 620).min(W) {
            if lum(menu_open.rgb(x, y)) > 55 {
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
        if (270..320).contains(&best) {
            in_dialog += 1;
        }
    }
    assert!(in_dialog >= 2, "弹窗底部的范围键菜单没打开（未找到 300 宽外框）");
    // 命令栏正上方 40px 内不该出现 300 宽的大菜单（弹窗里的菜单在弹窗内部往上长，
    // 底边最多到面板底部以上 6px，压不到命令栏）。
    let mut stray = 0;
    for y in (mid_y - 46)..(mid_y - 12) {
        let hits = (SIDE_LEFT + 10..SIDE_LEFT + PANE_W)
            .filter(|&x| lum(menu_open.rgb(x, y)) > 55)
            .count();
        if hits > 294 {
            stray += 1;
        }
    }
    assert_eq!(
        stray, 0,
        "点弹窗里的范围键后，命令栏上方也冒出了菜单（共用 open 状态）"
    );

    // 关掉弹窗：点遮罩（面板左侧那条，x 在面板外）。Esc 只在焦点还挂在弹窗
    // 编辑器上时有效（点过左栏/范围键之后焦点会走开），所以测试走遮罩这条路。
    click(&ui, (SIDE_LEFT + 20) as f32, 100.0);
    std::thread::sleep(std::time::Duration::from_millis(60));

    let mut folded = Frame::new();
    folded.grab(&ui);
    assert!(
        lum(folded.rgb(SIDE_LEFT + 20, 120)) > after + 6,
        "点遮罩后弹窗没收起（遮罩还在）"
    );

    // ── 5. 清空按钮 ─────────────────────────────────────────
    // (#cmdbar-clear-first 2026-10-23) × 挪到内嵌钮组最左（槽位 W-119，26 宽 → 中心 -106）。
    // 清空后危险态必须一起消失。
    let clear_x = input_right - 106;
    click(&ui, clear_x as f32, mid_y as f32);
    // 发送键的 background 有 animate：清空后要推几帧才回到 accent
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(45));
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(clear_x as f32, mid_y as f32),
        });
    }
    let mut cleared = Frame::new();
    cleared.grab(&ui);
    maybe_dump(&cleared, "cmd-clear");
    let mut red_left = 0;
    for y in (mid_y - 17)..(mid_y + 17) {
        for x in (SIDE_LEFT + 626)..(SIDE_LEFT + 690) {
            if close(cleared.rgb(x, y), danger, 24) {
                red_left += 1;
            }
        }
    }
    assert!(
        red_left < 40,
        "点 × 清空后发送键还是红的（{} px）",
        red_left
    );
}
