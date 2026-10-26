//! 「发送命令」独立窗口（#send-cmd-window 2026-10-26）的渲染级回归。
//!
//! 这块 UI 2026-10-26 从 AppWindow 的浮层搬成了**真·系统窗口**
//! （`ui/send_cmd_window.slint`，做法同 proc_window.slint）。独立窗口意味着
//! 探针可以只建它一个、直接量它的帧，不用再点 ↗ 开面板、也不用算窗格偏移。
//!
//! 运行：`cargo test --test cmd_expand_polish`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。

use mterm::ui::{QuickCmd, SendCmdWindow, SendScope, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 620;
const H: usize = 420;

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
    fn grab(&mut self, ui: &SendCmdWindow) {
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

fn qc(name: &str, command: &str, group: &str, header: bool, collapsed: bool, idx: i32) -> QuickCmd {
    QuickCmd {
        name: name.into(),
        command: command.into(),
        group: group.into(),
        group_header: if header { group.into() } else { "".into() },
        collapsed,
        orig_index: idx,
        send_enter: true,
    }
}

/// 12 条出厂预置（4 组），与真实数据同形；`collapsed` 控制分组折叠态。
fn preset_rows(collapsed: bool) -> Vec<QuickCmd> {
    let data = [
        ("磁盘与内存", "磁盘占用", "df -h"),
        ("磁盘与内存", "目录占用", "du -h --max-depth=1 . | sort -rh | head -20"),
        ("磁盘与内存", "内存使用", "free -h"),
        ("网络排查", "网卡地址", "ip -br addr"),
        ("网络排查", "监听端口", "ss -tlnp"),
        ("网络排查", "连通测试", "ping -c 4 "),
        ("服务与日志", "运行中服务", "systemctl list-units --type=service"),
        ("服务与日志", "跟踪系统日志", "journalctl -f -n 100"),
        ("服务与日志", "内核消息", "dmesg | tail -50"),
        ("进程与容器", "实时进程", "top"),
        ("进程与容器", "容器列表", "docker ps -a"),
        ("进程与容器", "异常服务", "systemctl --failed"),
    ];
    let mut out = Vec::new();
    let mut last = "";
    for (i, (group, name, command)) in data.iter().enumerate() {
        let header = *group != last;
        last = group;
        out.push(qc(name, command, group, header, collapsed, i as i32));
    }
    out
}

fn setup(rows: Vec<QuickCmd>) -> SendCmdWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = SendCmdWindow::new().unwrap();
    ui.window()
        .set_size(slint::LogicalSize::new(W as f32, H as f32));
    ui.set_quick_commands(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_command_history(ModelRc::from(Rc::new(VecModel::from(vec![
        "rm -rf test/".into(),
        "systemctl restart nginx".into(),
    ]))));
    ui.set_scopes(ModelRc::from(Rc::new(VecModel::from(vec![SendScope {
        letter: "A".into(),
        color: slint::Color::from_rgb_u8(230, 40, 40),
        count: 2,
    }]))));
    ui.set_tab_count(3);
    // 真机上是无边框自绘标题栏（Windows/Linux），边框/四角那条路径也要走
    ui.set_custom_titlebar(true);
    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(60));
    ui
}

fn click(ui: &SendCmdWindow, x: f32, y: f32) {
    let p = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: p,
        button: PointerEventButton::Left,
    });
    std::thread::sleep(std::time::Duration::from_millis(120));
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
    std::thread::sleep(std::time::Duration::from_millis(80));
}

fn type_text(ui: &SendCmdWindow, s: &str) {
    for ch in s.chars() {
        let t: slint::SharedString = ch.to_string().into();
        ui.window().dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        ui.window().dispatch_event(WindowEvent::KeyReleased { text: t });
    }
    std::thread::sleep(std::time::Duration::from_millis(60));
}

fn bright_runs(f: &Frame, y: usize, x0: usize, x1: usize, min_lum: u32) -> bool {
    let mut run = 0usize;
    for x in x0..x1.min(W) {
        if lum(f.rgb(x, y)) > min_lum {
            run += 1;
            if run >= 2 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// 一段区域里「有字的行」的起始 y（同一文本行的几个字形行并成一条，
/// 单行高的孤点——分隔线/边框——直接丢掉）。
fn text_rows(f: &Frame, x0: usize, x1: usize, y0: usize, y1: usize, min_lum: u32) -> Vec<usize> {
    let mut band: Option<(usize, usize)> = None;
    let mut out: Vec<usize> = Vec::new();
    for y in y0..y1.min(H) {
        if bright_runs(f, y, x0, x1, min_lum) {
            match band {
                Some((start, last)) if y <= last + 4 => band = Some((start, y)),
                Some((start, last)) => {
                    if last > start + 1 {
                        out.push(start);
                    }
                    band = Some((y, y));
                }
                None => band = Some((y, y)),
            }
        }
    }
    if let Some((start, last)) = band {
        if last > start + 1 {
            out.push(start);
        }
    }
    out
}

/// 标题栏高 46、页脚高 56 —— 中间那块是「主体」。
const TITLE_H: usize = 46;
const FOOTER_H: usize = 56;
/// 左栏宽 224（面板时代就是它，中缝竖线画在这个位置）。
const LEFT_W: usize = 224;

#[test]
fn window_lists_all_quick_commands_even_when_groups_are_collapsed() {
    let ui = setup(preset_rows(true));
    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "sendwin-list");

    let rows = text_rows(&f, 8, LEFT_W - 8, TITLE_H + 4, H - FOOTER_H - 40, 60);
    eprintln!("左栏文字行 y：{:?}", rows);
    assert!(
        rows.len() >= 8,
        "分组折叠时左栏只剩 {} 行（应列出全部快捷命令）：{:?}",
        rows.len(),
        rows
    );
}

/// (#send-cmd-filter 2026-10-27) 右侧打字时左栏跟着过滤 —— 与命令栏打字
/// 补全同一套规则：大小写不敏感子串、name 或 command 命中；清空恢复
/// 分组视图。
#[test]
fn window_typing_filters_the_left_column() {
    let ui = setup(preset_rows(false));
    let mut f = Frame::new();
    f.grab(&ui);
    let full = text_rows(&f, 8, LEFT_W - 8, TITLE_H + 4, H - FOOTER_H - 40, 60).len();
    assert!(full >= 8, "未过滤时左栏应有整列条目，只有 {} 行", full);

    // "df" 只命中「磁盘占用 df -h」一条（命令行两行文字）+ 两个节标题
    ui.set_text("df".into());
    std::thread::sleep(std::time::Duration::from_millis(60));
    f.grab(&ui);
    maybe_dump(&f, "sendwin-filter");
    let filtered = text_rows(&f, 8, LEFT_W - 8, TITLE_H + 4, H - FOOTER_H - 40, 60);
    eprintln!("过滤后左栏文字行 y：{:?}", filtered);
    assert!(
        filtered.len() < full && filtered.len() >= 1,
        "过滤后应只剩命中的少量行：full={} filtered={}",
        full,
        filtered.len()
    );

    // 大小写不敏感：大写 "DF" 与 "df" 命中同样的行
    ui.set_text("DF".into());
    std::thread::sleep(std::time::Duration::from_millis(60));
    f.grab(&ui);
    let upper = text_rows(&f, 8, LEFT_W - 8, TITLE_H + 4, H - FOOTER_H - 40, 60).len();
    assert_eq!(filtered.len(), upper, "大小写不应影响命中");

    // 清空恢复完整分组视图
    ui.set_text("".into());
    std::thread::sleep(std::time::Duration::from_millis(60));
    f.grab(&ui);
    let restored = text_rows(&f, 8, LEFT_W - 8, TITLE_H + 4, H - FOOTER_H - 40, 60).len();
    assert_eq!(restored, full, "清空后应恢复完整分组视图");
}

#[test]
fn window_clicking_a_command_fills_the_editor() {
    let ui = setup(preset_rows(false));

    // 先在编辑器里敲两个字：这会让 TextInput 的 text 与外部绑定断开，
    // 正是线上那条会让「点命令没反应」的路径。
    click(&ui, 420.0, 110.0);
    type_text(&ui, "ls");

    // (#send-cmd-filter) 打字会触发左栏过滤，"ls" 无命中、清单全空 ——
    // 点命令前清掉（清空编辑器同样保持「text 与 TextInput 绑定断开」的场景）。
    ui.set_text("".into());
    std::thread::sleep(std::time::Duration::from_millis(60));

    // 点第一条快捷命令（组名 24 + 条目 34 → 中心约 y=141）
    click(&ui, 110.0, 141.0);
    std::thread::sleep(std::time::Duration::from_millis(120));

    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "sendwin-pick");
    let body = text_rows(&f, LEFT_W + 38, W - 40, TITLE_H + 4, TITLE_H + 120, 45);
    eprintln!("编辑器文字行 y：{:?}", body);
    assert!(
        !body.is_empty(),
        "点左栏命令后编辑器里没出现文字（TextInput 的 text 绑定断了？）"
    );
}

#[test]
fn window_line_numbers_align_with_editor_rows() {
    let ui = setup(preset_rows(false));
    click(&ui, 420.0, 110.0);
    for line in ["df -h", "free -h", "ss -tlnp", "dmesg | tail"] {
        type_text(&ui, line);
        let ret: slint::SharedString = slint::platform::Key::Return.into();
        ui.window().dispatch_event(WindowEvent::KeyPressed { text: ret.clone() });
        ui.window().dispatch_event(WindowEvent::KeyReleased { text: ret });
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    std::thread::sleep(std::time::Duration::from_millis(120));

    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "sendwin-lines");

    let gutter = text_rows(&f, LEFT_W + 2, LEFT_W + 33, TITLE_H + 4, TITLE_H + 250, 45);
    let body = text_rows(&f, LEFT_W + 38, W - 40, TITLE_H + 4, TITLE_H + 250, 45);
    eprintln!("行号 y：{:?}\n正文 y：{:?}", gutter, body);
    assert!(body.len() >= 3, "编辑器里可量的行不足：{:?}", body);
    assert!(gutter.len() >= 3, "行号列可量的行不足：{:?}", gutter);
    assert!(
        gutter[0].abs_diff(body[0]) <= 2,
        "第一行行号与正文不对齐（行号 {} / 正文 {}）",
        gutter[0],
        body[0]
    );
    let g_pitch = (gutter[2] - gutter[0]) as i64 / 2;
    let b_pitch = (body[2] - body[0]) as i64 / 2;
    assert!(
        (g_pitch - b_pitch).abs() <= 1,
        "行号行距 {} 与正文行距 {} 不一致",
        g_pitch,
        b_pitch
    );
}

#[test]
fn window_title_body_and_footer_share_one_background() {
    let ui = setup(preset_rows(false));
    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "sendwin-bg");

    let pts = [
        ("标题行", W - 60, 14),
        ("左栏", LEFT_W - 30, H / 2),
        ("编辑区", W - 50, H / 2),
        ("页脚", W / 2 - 100, H - 20),
    ];
    let base = f.rgb(pts[0].1, pts[0].2);
    for (name, x, y) in pts {
        let c = f.rgb(x, y);
        assert!(
            close(c, base, 2),
            "{name} 底色与标题行不一致：{c:?} vs {base:?}"
        );
    }
}

#[test]
fn window_middle_divider_has_no_gap() {
    let ui = setup(preset_rows(false));
    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "sendwin-divider");

    let top = TITLE_H + 1;
    let bottom = H - FOOTER_H - 1;
    let bg = f.rgb(8, H / 2);
    let mut best = (0usize, 0usize);
    for col in (LEFT_W - 6)..=(LEFT_W + 6) {
        let hits = (top..bottom)
            .filter(|&y| !close(f.rgb(col, y), bg, 2))
            .count();
        if hits > best.1 {
            best = (col, hits);
        }
    }
    let (col, hits) = best;
    let total = bottom - top;
    eprintln!("中缝竖线 x={} 连续 {}/{} 行", col, hits, total);
    assert!(
        hits * 100 >= total * 99,
        "中缝竖线断开了：{}/{} 行有像素",
        hits,
        total
    );
}

#[test]
fn window_send_reports_command_and_keeps_the_text() {
    let ui = setup(preset_rows(false));
    let sent = Rc::new(std::cell::RefCell::new(Vec::<(String, String)>::new()));
    {
        let sent = sent.clone();
        ui.on_send(move |cmd, scope| {
            sent.borrow_mut().push((cmd.to_string(), scope.to_string()));
        });
    }

    click(&ui, 420.0, 110.0);
    type_text(&ui, "df -h");
    click(&ui, W as f32 - 44.0, H as f32 - 28.0); // 页脚右侧的发送键
    std::thread::sleep(std::time::Duration::from_millis(150));

    let calls = sent.borrow().clone();
    eprintln!("send 回调：{:?}", calls);
    assert_eq!(calls.len(), 1, "点发送没有回调（{:?}）", calls);
    assert_eq!(calls[0].0, "df -h");
    assert_eq!(calls[0].1, "", "默认范围应是「当前标签页」（空串）");
    // 文本保留（连发：换个范围再发同一条）
    assert_eq!(ui.get_text().as_str(), "df -h");
}

/// 范围菜单开着时，点面板内空白要关菜单（点外的终端/主窗由 OS 管）。
#[test]
fn window_scope_menu_closes_when_clicking_blank_area() {
    let ui = setup(preset_rows(false));

    let menu_rows = |f: &Frame| {
        (0..H)
            .filter(|&y| {
                let mut run = 0usize;
                let mut best = 0usize;
                for x in 0..W {
                    if lum(f.rgb(x, y)) > 55 {
                        run += 1;
                        best = best.max(run);
                    } else {
                        run = 0;
                    }
                }
                (270..320).contains(&best)
            })
            .count()
    };

    // 范围键在页脚左侧：x≈12+55、y = H-28
    click(&ui, 67.0, (H - 28) as f32);
    std::thread::sleep(std::time::Duration::from_millis(80));
    let mut f1 = Frame::new();
    f1.grab(&ui);
    maybe_dump(&f1, "sendwin-menu");
    assert!(menu_rows(&f1) >= 2, "范围菜单没打开");

    // 点标题栏空白 → 菜单收起（编辑区是 TextInput，会自己吞掉按下，不算"空白"）
    click(&ui, 300.0, 23.0);
    std::thread::sleep(std::time::Duration::from_millis(80));
    let mut f2 = Frame::new();
    f2.grab(&ui);
    maybe_dump(&f2, "sendwin-menu-closed");
    assert_eq!(
        menu_rows(&f2),
        0,
        "点面板空白没有关掉范围菜单"
    );
}

/// 拖拽改尺寸后布局要自适应（#send-cmd-window）：标题栏仍贴顶、页脚仍贴底、
/// 中缝竖线从标题下沿贯通到页脚上沿，编辑区跟着变宽。
#[test]
fn window_adapts_when_resized() {
    let ui = setup(preset_rows(false));
    const W2: usize = 900;
    const H2: usize = 560;
    ui.window()
        .set_size(slint::LogicalSize::new(W2 as f32, H2 as f32));
    std::thread::sleep(std::time::Duration::from_millis(120));

    let mut buf = vec![PremultipliedRgbaColor::default(); W2 * H2];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W2);
        });
    });
    let rgb = |x: usize, y: usize| {
        let p = buf[y * W2 + x];
        if p.alpha == 0 {
            return (0u8, 0u8, 0u8);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue))
    };

    // 标题栏/页脚贴边
    let title = rgb(W2 - 40, 14);
    let footer = rgb(W2 / 2, H2 - 14);
    assert!(close(title, footer, 2), "标题/页脚底色不一致 {:?} vs {:?}", title, footer);

    // 中缝竖线从标题下沿贯通到页脚上沿
    let bg = rgb(8, H2 / 2);
    let top = TITLE_H + 1;
    let bottom = H2 - FOOTER_H - 1;
    let mut hits = 0usize;
    for y in top..bottom {
        if !close(rgb(LEFT_W, y), bg, 2) {
            hits += 1;
        }
    }
    let total = bottom - top;
    eprintln!("改尺寸 {}x{}：中缝竖线 {}/{} 行", W2, H2, hits, total);
    assert!(
        hits * 100 >= total * 99,
        "改尺寸后中缝竖线断了/没了：{}/{} 行",
        hits,
        total
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 2026-10-26 用户七条反馈的回归
// ─────────────────────────────────────────────────────────────────────────

/// (1) 范围菜单是浮层、横跨 x=224 那条中缝：线画在布局之后（z 最高），
/// 不处理就会出现在菜单正中间 —— 用户报的「下拉框好像穿透了」。
#[test]
fn window_middle_divider_hides_under_the_scope_menu() {
    let ui = setup(preset_rows(false));
    let mut f = Frame::new();
    f.grab(&ui);
    // 菜单没开：x=224 是一条可见分隔线（比两侧面板亮）
    let idle = f.rgb(LEFT_W, 300);

    // 与 window_scope_menu_closes_when_clicking_blank_area 同一个命中点
    click(&ui, 67.0, (H - 28) as f32);
    std::thread::sleep(std::time::Duration::from_millis(120));
    f.grab(&ui);
    maybe_dump(&f, "sendwin-menu-divider");
    let over_menu = f.rgb(LEFT_W, 300);

    assert!(
        lum(idle) > lum(over_menu) + 6,
        "范围菜单打开后中缝仍压在菜单上（无菜单 {} / 有菜单 {}）",
        lum(idle),
        lum(over_menu)
    );
}

/// (7) 窗口内分隔线要跟设置里的标题栏下沿同一档（border-subtle），
/// 不是更亮更硬的 line-2。
#[test]
fn window_chrome_lines_use_the_settings_divider_token() {
    let ui = setup(preset_rows(false));
    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "sendwin-lines");

    let want = ui.global::<Theme>().get_border_subtle();
    let slint::Brush::SolidColor(c) = want else {
        panic!("border-subtle 不是实色，测试假设失效");
    };
    let want_rgb = (c.red(), c.green(), c.blue());

    // 标题栏下沿：46 高的最后 1 设备像素（DPR=1 → y=45）；x=300 处没有文字/按钮
    let got = f.rgb(300, TITLE_H - 1);
    assert!(
        close(got, want_rgb, 6),
        "标题栏下沿 {got:?} ≠ Theme.border-subtle {want_rgb:?}"
    );

    // 页脚上沿同理
    let foot = f.rgb(300, H - FOOTER_H);
    assert!(
        close(foot, want_rgb, 6),
        "页脚上沿 {foot:?} ≠ Theme.border-subtle {want_rgb:?}"
    );
}

/// (5) 关闭键左边的「钉住」钮：点一下置顶、再点取消。
/// `Window.always-on-top` 绑定 pinned，winit 后端会落到 WindowLevel。
#[test]
fn window_pin_button_toggles_always_on_top() {
    let ui = setup(preset_rows(false));
    // 位置：右衬 12 + 关闭 28 + 间距 5 + 钉住 24 → 中心 x = W - 57
    let pin_x = (W - 57) as f32;
    let pin_y = 23.0;

    assert!(!ui.get_pinned(), "新窗口不该默认置顶");
    click(&ui, pin_x, pin_y);
    assert!(ui.get_pinned(), "点钉住后窗口没置顶");
    click(&ui, pin_x, pin_y);
    assert!(!ui.get_pinned(), "再点钉住没有取消置顶");
}

/// (6) 编辑器光标类型跟随设置：block 画一个字符格、bar 只有 2px、
/// underline 贴行底。断言靠「同一位置几种形状的像素覆盖」。
#[test]
fn window_caret_follows_the_configured_cursor_style() {
    // 先在编辑器里打一段固定宽度不会变化的文本
    let ui = setup(preset_rows(false));
    click(&ui, 420.0, 110.0);
    type_text(&ui, "ls");
    std::thread::sleep(std::time::Duration::from_millis(120));

    // 量「块状」时被染色的像素数（编辑器区，第一行附近）
    let block_pixels = |ui: &SendCmdWindow, style: &str| -> usize {
        ui.set_cursor_style(style.into());
        std::thread::sleep(std::time::Duration::from_millis(80));
        let mut f = Frame::new();
        f.grab(ui);
        let mut n = 0;
        for y in (TITLE_H + 2)..(TITLE_H + 40) {
            for x in (LEFT_W + 30)..(W - 20) {
                let (r, g, b) = f.rgb(x, y);
                // 光标色是 term-fg（深色主题近白）—— 找明显亮于面板底的像素
                if lum((r, g, b)) > 90 {
                    n += 1;
                }
            }
        }
        n
    };

    let block = block_pixels(&ui, "block");
    let bar = block_pixels(&ui, "bar");
    eprintln!("光标像素数：block={block} bar={bar}");
    assert!(
        block > bar * 2,
        "块状光标（{block} 像素）没有明显宽于竖线（{bar} 像素）"
    );

    // (#send-cmd-caret-underline) 下划线档：内建光标宽 0，行底自绘一条短横。
    // 旧实现把下划线退化成 2px 竖线（与「竖线」档无法区分，用户反馈「没跟着
    // 变」）。行中带用**同窗口差值**判断：bar 与 underline 两档的文字底噪
    // 相同，差值 = 光标本体 —— bar 有全行高竖线、underline 没有（不猜 caret
    // 的像素坐标，文字宽度随字体度量变）。
    type_text(&ui, " ");
    std::thread::sleep(std::time::Duration::from_millis(80));

    let band_pixels = |ui: &SendCmdWindow, style: &str, y0: usize, y1: usize| -> usize {
        ui.set_cursor_style(style.into());
        std::thread::sleep(std::time::Duration::from_millis(80));
        let mut f = Frame::new();
        f.grab(ui);
        let mut n = 0;
        for y in y0..y1 {
            for x in (LEFT_W + 30)..(W - 20) {
                if lum(f.rgb(x, y)) > 90 {
                    n += 1;
                }
            }
        }
        n
    };
    // 行中带（编辑器首行上半段）：bar 档 = 文字 + 全行高竖线，underline 档
    // = 同样的文字（内建光标已隐藏）→ 差值即竖线；退化成竖线时差值≈0。
    let mid_bar = band_pixels(&ui, "bar", TITLE_H + 8, TITLE_H + 20);
    let mid_underline = band_pixels(&ui, "underline", TITLE_H + 8, TITLE_H + 20);
    // 行底带：下划线档的 2px 横条画在光标行底部（行顶 ≈TITLE_H+8 + 行距 − 2）。
    let bottom_underline = band_pixels(&ui, "underline", TITLE_H + 20, TITLE_H + 34);
    eprintln!(
        "光标分带：bar 行中={mid_bar} underline 行中={mid_underline} \
         差值={} 行底={bottom_underline}",
        mid_bar as isize - mid_underline as isize
    );
    assert!(
        mid_bar as isize - mid_underline as isize >= 15,
        "竖线档行中应有全行高竖线（bar={mid_bar} underline={mid_underline}）"
    );
    assert!(
        bottom_underline >= 6,
        "下划线档行底应有一条短横（得 {bottom_underline}）"
    );
}

