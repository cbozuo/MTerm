//! (#cm-card-eq 2026-10-26) 命令卡网格：**两列必须等宽**。
//!
//! 用户截图反馈「预设的命令块大小不一致」。根因是行内两张卡的 preferred
//! 宽度随命令/备注文字长度变化，Slint 的 box layout 以 pref 为起点均摊余量
//! → 同一列在不同行宽度不同（实测 229 / 289 / 299 / 191 px 四种列宽，最窄
//! 那行的分组 chip 被压成「…」）。
//!
//! 修法：列宽由网格宽度算出来（`card-w`），行内两张卡都显式 `width`（Slint
//! 把布局子项上的 width 绑定当作 min=max 固定宽）。本测试锁定「每行两条边线
//! 等长、且各行等长、奇偶行同宽」，用**内容长度差异很大**的卡片复现原始症状。
//!
//! 运行：`cargo test --test cmd_manager_card_grid`；
//! `PROBE_DUMP_DIR=<dir>` 时额外导出 PPM 证据图。

use mterm::ui::{AppWindow, CmCard, CmGroupRow, PaneInfo, TabInfo, TerminalState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{WindowAdapter};
use slint::{Brush, ComponentHandle, LogicalSize, ModelRc, VecModel};
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

fn group(key: &str, display: &str, count: i32, kind: i32) -> CmGroupRow {
    CmGroupRow {
        key: key.into(),
        display: display.into(),
        count,
        hit: count,
        kind,
    }
}

/// 内容长度刻意拉开（短命令 / 换行长命令 / 超长单行 / 短备注）：列宽必须只由
/// 网格决定，不能被任何一张卡的内容带偏。
fn card(key: &str, name: &str, note: &str, command: &str, group: &str) -> CmCard {
    CmCard {
        key: key.into(),
        name: name.into(),
        note: note.into(),
        command: command.into(),
        group_key: group.into(),
        group_display: group.into(),
        send_enter: command != "ping -c 4 ",
        picked: false,
        can_up: false,
        can_down: false,
        show_group: true,
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
        group("__all", "全部命令", 12, 0),
        group("", "默认分组", 0, 1),
        group("g1", "磁盘与内存", 3, 2),
        group("g2", "网络排查", 3, 2),
        group("g3", "服务与日志", 3, 2),
        group("g4", "进程与容器", 3, 2),
    ]))));
    ui.set_cm_menu_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        group("g1", "磁盘与内存", 3, 2),
        group("g3", "服务与日志", 3, 2),
    ]))));
    // 12 条 = 出厂预置清单的分组形状（#preset-groups），「全部命令」视图 6 行
    ui.set_cm_cards(ModelRc::from(Rc::new(VecModel::from(vec![
        card("k1", "磁盘占用", "各挂载点容量与使用率", "df -h", "磁盘与内存"),
        card(
            "k2",
            "目录占用",
            "当前目录下最占空间的 Top 20",
            "du -h --max-depth=1 . | sort -rh | head -20",
            "磁盘与内存",
        ),
        card("k3", "内存使用", "内存与交换分区", "free -h", "磁盘与内存"),
        card("k4", "网卡地址", "IP 与状态一行式", "ip -br addr", "网络排查"),
        card("k5", "监听端口", "哪个进程监听哪个端口", "ss -tlnp", "网络排查"),
        card("k6", "连通测试", "补全目标地址再回车", "ping -c 4 ", "网络排查"),
        card(
            "k7",
            "运行中服务",
            "systemd 服务一览",
            "systemctl list-units --type=service --state=running",
            "服务与日志",
        ),
        card("k8", "跟踪系统日志", "实时滚动系统日志", "journalctl -f -n 100", "服务与日志"),
        card("k9", "内核消息", "最近的内核告警", "dmesg | tail -50", "服务与日志"),
        card("k10", "实时进程", "按 CPU / 内存排序", "top", "进程与容器"),
        card("k11", "容器列表", "全部容器及状态", "docker ps -a", "进程与容器"),
        card("k12", "异常服务", "启动失败的服务", "systemctl --failed", "进程与容器"),
    ]))));
    ui.set_cm_total_count(12);
    ui.set_cm_custom_count(4);
    ui.set_cm_cur_name("全部命令".into());
    ui.set_cm_cur_count(12);
    ui.set_cm_cur_index(0);
    ui.set_cm_view_count(12);
    ui.set_cm_subtitle("12 条命令".into());
    ui.set_cm_search_hits(12);
    ui.set_cm_has_any(true);
    ui.set_cm_picked_count(0);
    ui.set_cm_selected("__all".into());
    ui.set_quick_cmd_manage_open(true);

    std::thread::sleep(std::time::Duration::from_millis(150));
    ui
}

/// 一行里与 `bg` 相近的连续像素段（卡片底色填充）。
fn fill_runs(f: &Frame, y: usize, bg: (u8, u8, u8), x0: usize, x1: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut run = 0usize;
    let mut start = 0usize;
    for x in x0..x1 {
        if close(f.rgb(x, y), bg, 3) {
            if run == 0 {
                start = x;
            }
            run += 1;
        } else {
            if run >= 150 {
                out.push((start, run));
            }
            run = 0;
        }
    }
    if run >= 150 {
        out.push((start, run));
    }
    out
}

#[test]
fn command_cards_share_one_column_width() {
    let ui = setup();
    let theme = ui.global::<Theme>();
    let fill = solid(&theme.get_popup_field()).expect("popup-field 应为纯色");

    let mut f = Frame::new();
    f.grab(&ui);
    maybe_dump(&f, "cm-card-grid");

    // 逐行找「恰好两段卡片底色」的横线；文字/图标会把填充切断，所以按**卡片行**
    // 归组后取每组的最大宽度 —— 断口只会让 run 变短，不会变长。
    let mut lines: Vec<(usize, usize, usize)> = Vec::new(); // (y, 左宽, 右宽)
    for y in 150..H - 90 {
        let runs = fill_runs(&f, y, fill, 400, 1045);
        // 两列卡的判据 = 两段填充之间只隔一个列间距（card-gap 8px + 两条边线）
        for w in runs.windows(2) {
            let gap = w[1].0.saturating_sub(w[0].0 + w[0].1);
            if gap <= 14 {
                lines.push((y, w[0].1, w[1].1));
                break;
            }
        }
    }
    assert!(!lines.is_empty(), "一条干净卡片行都没采到");

    // 归组：纵向间隔 > 4px 就是另一张卡
    let mut bands: Vec<(usize, usize, usize)> = Vec::new(); // (左宽 max, 右宽 max, 行数)
    let mut cur: Option<(usize, usize, usize, usize)> = None; // (last y, lmax, rmax, n)
    for (y, l, r) in lines {
        match cur {
            Some((ly, lmax, rmax, n)) if y <= ly + 4 => {
                cur = Some((y, lmax.max(l), rmax.max(r), n + 1));
            }
            Some((_, lmax, rmax, n)) => {
                bands.push((lmax, rmax, n));
                cur = Some((y, l, r, 1));
            }
            None => cur = Some((y, l, r, 1)),
        }
    }
    if let Some((_, lmax, rmax, n)) = cur {
        bands.push((lmax, rmax, n));
    }
    eprintln!("卡片行（左宽, 右宽, 采样行数）：{:?}", bands);
    assert!(bands.len() >= 3, "卡片行数不足：{:?}", bands);

    // 每行两卡等宽（1px 容差 = 边框取整）
    for (l, r, n) in &bands {
        assert!(
            l.abs_diff(*r) <= 1,
            "同一行两张卡宽度不一致：左 {} vs 右 {}（该行采样 {} 条）",
            l,
            r,
            n
        );
    }
    // 行与行之间也等宽
    let base = bands[0].0;
    for (l, r, _) in &bands {
        assert!(
            // 2px 是布局取整的极限（同样是整列宽，末行会差 1~2px）；
            // 之前「每行各算各的」时差到 100px 以上，这个阈值足以抓住回归。
            l.abs_diff(base) <= 4 && r.abs_diff(base) <= 4,
            "不同行卡宽不一致：基准 {}，实测 {} / {}（{:?}）",
            base,
            l,
            r,
            bands
        );
    }
}
