//! (#theme-row-align 2026-10-15) 主题选择器行对齐 / 星标 chip / 设置卡内钳位
//! 的渲染级回归。
//!
//! 用户反馈（两图）：
//!   ① 「色块带 五角星和文字 统一最右侧对齐」—— 实测色带右缘随主题名长短
//!      漂移（MTerm·亮 477 / 原子·暗 445 / 卡布奇诺·暗 493 物理 px）；
//!   ② 「移出常用 / 设为常用要有对应的样式，让知道可以点击」；
//!   ③ 「设置中的主题下拉框不要超出设置弹框」。
//!
//! 根因（离屏实测 + i-slint-core 源码）：
//!   a. 行布局写了 `alignment: start` → 主轴 stretch 语义被废，吸空块
//!      flex_grow 归零，余量改由 name 文本（max=∞）均摊 → 色带右缘漂移；
//!   b. ✓ 是流内条件元素 → 选中行色带整体左移 ≈19px；
//!   c. 浮层位置只钳全窗，从设置卡内打开时会探出卡片右缘。
//!
//! 本探针自建 MinimalSoftwareWindow 平台拿整帧（同 tests/dialog_scrim_drag.rs），
//! 用**合成条目**（名字长短刻意拉开，含真实最长名）驱动渲染，量色带右缘是否
//! 各行一致、chip 悬停是否提亮、设置卡内打开是否钳在卡内。

use mterm::ui::{AppWindow, Theme, ThemeEntry};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{Brush, ComponentHandle, LogicalPosition, LogicalSize, ModelRc, SharedString};
use std::rc::Rc;

const W: usize = 1400;
const H: usize = 900;

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

struct Frame { buf: Vec<PremultipliedRgbaColor> }
impl Frame {
    fn new() -> Self { Self { buf: vec![PremultipliedRgbaColor::default(); W * H] } }
    /// 快照（聚焦前后对比用 —— grab 是覆写式的）。
    fn snapshot(&self) -> Frame { Frame { buf: self.buf.clone() } }
    fn grab(&mut self, ui: &AppWindow) {
        ui.window().request_redraw();
        WIN.with(|win| win.draw_if_needed(|r| { r.render(&mut self.buf, W); }));
    }
    fn px(&self, x: usize, y: usize) -> (u8, u8, u8, u8) {
        let p = self.buf[y * W + x];
        if p.alpha == 0 { return (0, 0, 0, 0); }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue), p.alpha)
    }
    /// 只要 RGB（比较色值时用；alpha 全透明处返回 0,0,0）。
    fn px3(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let (r, g, b, _) = self.px(x, y);
        (r, g, b)
    }
    /// 颜色 C（容差 tol）的像素包围盒，限定在给定区域内。
    fn bbox_of(&self, c: (u8, u8, u8), tol: u8, region: (usize, usize, usize, usize))
        -> Option<(usize, usize, usize, usize)> {
        let (x0, y0, x1, y1) = region;
        let (mut minx, mut miny, mut maxx, mut maxy) = (usize::MAX, usize::MAX, 0, 0);
        let mut found = false;
        for y in y0..y1 {
            for x in x0..x1 {
                let (r, g, b, a) = self.px(x, y);
                if a == 0 { continue; }
                if r.abs_diff(c.0) <= tol && g.abs_diff(c.1) <= tol && b.abs_diff(c.2) <= tol {
                    found = true;
                    minx = minx.min(x); miny = miny.min(y);
                    maxx = maxx.max(x); maxy = maxy.max(y);
                }
            }
        }
        found.then_some((minx, miny, maxx, maxy))
    }
}

fn brush_rgb(b: &Brush) -> (u8, u8, u8) {
    match b { Brush::SolidColor(c) => (c.red(), c.green(), c.blue()), _ => (0, 0, 0) }
}

fn color_rgb(c: slint::Color) -> (u8, u8, u8) { (c.red(), c.green(), c.blue()) }

fn setup() -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();
    ui
}

fn ansi8() -> ModelRc<slint::Color> {
    ModelRc::from(Rc::new(slint::VecModel::from(vec![
        slint::Color::from_rgb_u8(255, 0, 0),
        slint::Color::from_rgb_u8(0, 200, 0),
        slint::Color::from_rgb_u8(0, 80, 255),
        slint::Color::from_rgb_u8(255, 230, 0),
        slint::Color::from_rgb_u8(230, 0, 200),
        slint::Color::from_rgb_u8(0, 220, 220),
        slint::Color::from_rgb_u8(255, 130, 0),
        slint::Color::from_rgb_u8(150, 60, 255),
    ])))
}

/// 合成一行。ac 用灰色（不与八色带混）；名字长短刻意拉开。
fn row(id: &str, zh: &str, mode: &str, faved: bool) -> ThemeEntry {
    ThemeEntry {
        id: id.into(),
        zh: zh.into(),
        mode: mode.into(),
        kind: "radix".into(),
        root: slint::Color::from_rgb_u8(20, 22, 26),
        panel: slint::Color::from_rgb_u8(26, 28, 33),
        tbg: slint::Color::from_rgb_u8(16, 18, 22),
        tfg: slint::Color::from_rgb_u8(200, 200, 200),
        ac: slint::Color::from_rgb_u8(128, 128, 128),
        ansi_eight: ansi8(),
        faved,
        matched: true,
    }
}

/// 找所有主题行：饱和带（带宽 ≥ 50px，即八色带的 ~75px；动作行 / 搜索行
/// 里的 accent 元素都是窄条，会被滤掉）。返回每行中心 y。
fn find_row_centers(f: &Frame, y_from: usize, y_to: usize) -> Vec<usize> {
    let mut bands: Vec<(usize, usize, usize, usize)> = Vec::new(); // (y0,y1,x0,x1)
    let mut cur: Option<(usize, usize, usize, usize)> = None;
    for y in y_from..y_to {
        // 只扫色带区（x ≥ 250）：选中行的**强调色名字**也是饱和色，且在色带
        // 左侧（实测 x158..222），不排除会把一行劈成两条 band（行号错位）。
        let xs: Vec<usize> = (250..(W - 60))
            .filter(|&x| { let (r, g, b, a) = f.px(x, y); a > 0 && (r.max(g).max(b) - r.min(g).min(b)) > 60 })
            .collect();
        let wide = match (xs.first(), xs.last()) {
            (Some(a), Some(b)) => b - a >= 50,
            _ => false,
        };
        if wide {
            let (x0, x1) = (*xs.first().unwrap(), *xs.last().unwrap());
            match cur {
                Some((y0, _, cx0, cx1)) => cur = Some((y0, y, cx0.min(x0), cx1.max(x1))),
                None => cur = Some((y, y, x0, x1)),
            }
        } else if let Some(b) = cur.take() {
            bands.push(b);
        }
    }
    if let Some(b) = cur.take() { bands.push(b); }
    bands.iter().map(|(y0, y1, _, _)| (y0 + y1) / 2).collect()
}

/// 行内**第一条宽饱和带** = 八色带（~75px）。必须取第一条：选中行的 ✓ 与
/// 收藏行的淡星也是饱和色，且在色带右侧，取"最右"会把它们算进来（实测跨度
/// 被拉到 33px）。圆点用灰（原 3 色阶条 2026-10-16 已删），都不会误触。
fn strip_run(f: &Frame, y: usize) -> Option<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut last: Option<usize> = None;
    for x in 150..(W - 60) {
        let (r, g, b, a) = f.px(x, y);
        let sat = a > 0 && (r.max(g).max(b) - r.min(g).min(b)) > 60;
        if sat {
            if start.is_none() { start = Some(x); }
            last = Some(x);
        } else if let (Some(st), Some(ls)) = (start, last) {
            if x - ls > 6 {
                runs.push((st, ls));
                start = None;
                last = None;
            }
        }
    }
    if let (Some(st), Some(ls)) = (start, last) { runs.push((st, ls)); }
    runs.into_iter().find(|(a, b)| b - a >= 55)
}

/// 让 120ms 的透明度动画走完再量。离屏平台没有事件 loop，动画按真实时间
/// 在渲染时求值 —— 不等就量会量到动画中段的随机值（实测偶发：chip 提亮
/// 只差 2/255，断言抖动）。
fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(180));
}

/// 真实 28 套条目（复刻 build_theme_entries 的 make + 暗/亮分组）。
/// 分组逻辑由 `theme_picker_group_tests` 用真函数锁定；这里只为证据图服务。
fn real_entries() -> Vec<ThemeEntry> {
    use crate_free::palettes::PALETTES;
    let make = |p: &'static crate_free::palettes::Palette, faved: bool| ThemeEntry {
        id: p.id.into(),
        zh: p.zh.into(),
        mode: if p.dark { "暗".into() } else { "亮".into() },
        kind: p.kind.into(),
        root: crate_free::color(p.root),
        panel: crate_free::color(p.panel),
        tbg: crate_free::color(p.tbg),
        tfg: crate_free::color(p.tfg),
        ac: crate_free::color(p.ac),
        ansi_eight: ModelRc::from(Rc::new(slint::VecModel::from(
            [1usize, 2, 3, 4, 5, 6, 9, 14]
                .iter()
                .map(|&i| crate_free::color(p.ansi16[i]))
                .collect::<Vec<_>>(),
        ))),
        faved,
        matched: true,
    };
    let header = |zh: &str, cnt: String| ThemeEntry {
        id: "".into(),
        zh: zh.into(),
        mode: cnt.into(),
        kind: "header".into(),
        root: Default::default(),
        panel: Default::default(),
        tbg: Default::default(),
        tfg: Default::default(),
        ac: Default::default(),
        ansi_eight: Default::default(),
        faved: false,
        matched: true,
    };
    let mut out = vec![
        header("常用", "2 / 最多 8　·　点星可整理".into()),
        make(crate_free::palettes::find("mterm-dark").unwrap(), true),
        make(crate_free::palettes::find("mterm-light").unwrap(), true),
    ];
    for dark in [true, false] {
        let g: Vec<_> = PALETTES.iter().filter(|p| p.dark == dark).collect();
        out.push(header(if dark { "暗色" } else { "亮色" }, format!("{} 套", g.len())));
        for p in g {
            out.push(make(p, false));
        }
    }
    out
}

/// crate 内模块在集成测试里不可见，经 lib 门面转一层。
mod crate_free {
    pub use mterm::theme::{color, find, palettes};
}

fn dump(f: &Frame, name: &str) {
    let Ok(dir) = std::env::var("THEME_DUMP_DIR") else { return };
    let mut data = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            let (r, g, b, _) = f.px(x, y);
            data.extend_from_slice(&[r, g, b]);
        }
    }
    let path = std::path::Path::new(&dir).join(format!("{}.ppm", name));
    let header = format!("P6\n{} {}\n255\n", W, H);
    std::fs::write(&path, header.as_bytes())
        .and_then(|_| std::fs::write(&path, [header.as_bytes(), &data].concat()))
        .ok();
    eprintln!("dumped {}", path.display());
}

fn move_to(ui: &AppWindow, x: f32, y: f32) {
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(x, y),
    });
}

fn click_at(ui: &AppWindow, x: f32, y: f32) {
    move_to(ui, x, y);
    let p = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerPressed { position: p, button: PointerEventButton::Left });
    ui.window().dispatch_event(WindowEvent::PointerReleased { position: p, button: PointerEventButton::Left });
}

#[test]
fn theme_rows_right_align_chip_and_stay_inside_settings_card() {
    let ui = setup();
    let mut f = Frame::new();

    // ── 合成条目：名字长短拉开（含真实最长名），两颗常用，一颗选中 ──────
    let entries = vec![
        row("t1", "MTerm · 暗", "暗", true),
        row("t2", "原子 · 暗", "暗", false),
        row("t3", "卡布奇诺 · 暗", "暗", false),
        row("t4", "Solarized · 亮", "亮", false),
        row("t5", "甲", "亮", true),
        row("t6", "乙", "暗", false),
    ];
    ui.set_theme_entries(ModelRc::from(Rc::new(slint::VecModel::from(entries))));
    ui.set_theme_total(6);
    ui.set_theme_chosen("t3".into());
    ui.set_theme_pick_x(120.0);
    ui.set_theme_pick_y(120.0);
    ui.set_theme_pick_open(true);
    f.grab(&ui);

    let centers = find_row_centers(&f, 130, 700);
    let mut rights: Vec<(usize, usize)> = Vec::new(); // (row_index, right)
    for (i, &y) in centers.iter().enumerate() {
        if let Some((_, r)) = strip_run(&f, y) {
            rights.push((i, r));
        }
    }
    assert!(rights.len() >= 5, "应至少量到 5 行色带，实得 {} 行", rights.len());

    // ① 各行色带右缘一致（吸空块独吞余量的直接证据）。
    let (min_r, max_r) = rights.iter().fold((usize::MAX, 0usize), |(a, b), (_, r)| (a.min(*r), b.max(*r)));
    assert!(
        max_r - min_r <= 2,
        "各行八色带右缘应一致（暗/亮/长短名/选中行都一样），实测跨度 {}..{}",
        min_r, max_r
    );

    // ② 选中行（t3，index 2）与其它行同右缘 —— ✓ 常驻槽位后不挤位。
    let chosen = rights.iter().find(|(i, _)| *i == 2).map(|(_, r)| *r);
    if let Some(c) = chosen {
        for (i, r) in &rights {
            if *i != 2 {
                assert!((*r as i32 - c as i32).abs() <= 2, "选中行色带右缘 {} vs 第 {} 行 {}", c, i, r);
            }
        }
    }

    // ②b 选中行（index 2）的 ✓ 槽位有笔画，未选中行同位置空白。
    //     （Text "✓" 在字体链里缺字形 renders nothing，已改 Icon check；
    //      这条断言就是防它退化成 Text 的闸门。）
    let slot_x = min_r + 14;   // ✓ 槽位中心 = 色带右缘 + 间距 9 + 半槽 5
    // 背景参照取**同一槽位在干净行**上的像素（t2：未选中、未收藏，无 ✓ 无星）。
    // ⚠️ 不能从色带区取参照 —— 实测参照点落在色块上时，整行都会被判成"有笔画"。
    let bg = { let (r, g, b, _) = f.px(slot_x, centers[1]); (r, g, b) };
    let ink = |cy: usize| -> usize {
        (cy.saturating_sub(6)..cy + 6)
            .flat_map(|y| (slot_x - 5..slot_x + 5).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let (r, g, b, _) = f.px(x, y);
                (r.abs_diff(bg.0) as u16 + g.abs_diff(bg.1) as u16 + b.abs_diff(bg.2) as u16) > 30
            })
            .count()
    };
    assert!(ink(centers[2]) > 0, "选中行 ✓ 槽位应有笔画（Icon check）");
    assert_eq!(ink(centers[1]), 0, "未选中行同位置不应有 ✓ 笔画");

    // ③ 悬停某行 → 右端 chip 显形（底色 = accent 14% 叠 popup-bg）；
    //    指针再进入 chip 自身 → 提亮到 26%。
    let theme = ui.global::<Theme>();
    let popup = brush_rgb(&theme.get_popup_bg());
    let accent = color_rgb(theme.get_accent_solid());
    // chip 中心：行右缘 - 11(padding) - 42(chip 半宽)
    let row_y = centers[1];
    let chip_cx = (min_r + 104 - 11 - 42) as f32;   // 色带右缘 +104 = 行内容右缘
    let chip_cy = row_y as f32;
    let before = { let (r, g, b, _) = f.px(chip_cx as usize, chip_cy as usize); (r, g, b) };
    move_to(&ui, 400.0, chip_cy);
    settle();
    f.grab(&ui);
    let on_row = { let (r, g, b, _) = f.px(chip_cx as usize, chip_cy as usize); (r, g, b) };
    move_to(&ui, chip_cx, chip_cy);
    settle();
    f.grab(&ui);
    let on_chip = { let (r, g, b, _) = f.px(chip_cx as usize, chip_cy as usize); (r, g, b) };
    let dist = |a: (u8, u8, u8), b: (u8, u8, u8)| {
        (a.0 as i32 - b.0 as i32).abs() + (a.1 as i32 - b.1 as i32).abs() + (a.2 as i32 - b.2 as i32).abs()
    };
    assert!(
        dist(on_row, before) > 6,
        "悬停行后 chip 应显形：{:?} → {:?}（popup {:?} / accent {:?}）",
        before, on_row, popup, accent
    );
    assert!(
        dist(on_chip, on_row) > 4,
        "指针进入 chip 自身应再提亮一档：{:?} → {:?}",
        on_row, on_chip
    );
    // 提亮方向 = 朝 accent（chip 底色变浓）
    assert!(
        dist(on_chip, accent) < dist(on_row, accent),
        "chip-hot 应更靠近 accent：{:?} vs {:?}（accent {:?}）",
        on_chip, on_row, accent
    );

    // ── ④ 设置卡内打开：浮层钳在卡片内，不探出右缘 ─────────────────────
    ui.set_theme_pick_open(false);
    ui.set_interface_open(true);
    ui.set_ifd_page("theme".into());
    f.grab(&ui);
    let card = f
        .bbox_of(popup, 2, (0, 0, W, H))
        .expect("应能找到设置卡（popup-bg 不透明区）");
    let (card_x0, _, card_x1, _) = card;
    // 锚点故意设成会探出卡片右缘的位置（按钮贴卡片右侧）
    ui.set_theme_pick_x((card_x1 - 60) as f32);
    ui.set_theme_pick_y((card.1 + 60) as f32);
    ui.set_theme_pick_open(true);
    f.grab(&ui);
    let mut rights2: Vec<usize> = Vec::new();
    for &y in find_row_centers(&f, card.1 + 60, card.1 + 560).iter() {
        if let Some((_, r)) = strip_run(&f, y) { rights2.push(r); }
    }
    assert!(!rights2.is_empty(), "设置卡内打开时浮层应有行");
    let pop_right = rights2.iter().max().copied().unwrap() + 104;
    let pop_left = rights2.iter().min().copied().unwrap() - 300; // 行内容左缘 ≈ 色带右缘 - 300
    assert!(
        pop_right <= card_x1 + 1,
        "浮层右缘 {} 不应超出设置卡右缘 {}",
        pop_right, card_x1
    );
    assert!(
        pop_left >= card_x0 - 1,
        "浮层左缘 {} 不应探出设置卡左缘 {}",
        pop_left, card_x0
    );

    // ── 证据图（THEME_DUMP_DIR 设置了才产；正常跑不写文件）─────────────
    if std::env::var("THEME_DUMP_DIR").is_ok() {
        // ① 真实 28 套 + 暗/亮分组，第二行悬停（chip 显形）
        ui.set_theme_pick_open(false);
        ui.set_interface_open(false);
        f.grab(&ui);
        let entries = real_entries();
        ui.set_theme_entries(ModelRc::from(Rc::new(slint::VecModel::from(entries))));
        ui.set_theme_total(28);
        ui.set_theme_chosen("mterm-dark".into());
        ui.set_theme_pick_x(120.0);
        ui.set_theme_pick_y(120.0);
        ui.set_theme_pick_open(true);
        f.grab(&ui);
        let centers = find_row_centers(&f, 130, 900);
        // 悬停「常用」区第二行（MTerm·亮，收藏态 → 移出常用）
        if centers.len() > 1 {
            move_to(&ui, 500.0, centers[1] as f32);
            settle();
            f.grab(&ui);
            dump(&f, "theme-pick-grouped");
            // 再悬停一条未收藏的暗色行 → 设为常用 chip
            let target = centers.iter().rev().find(|&&y| y > 400).copied();
            if let Some(y) = target {
                move_to(&ui, 500.0, y as f32);
                settle();
                f.grab(&ui);
                dump(&f, "theme-pick-hover-unfav");
            }
        }
        // ② 设置卡内打开（钳位）
        ui.set_theme_pick_open(false);
        ui.set_interface_open(true);
        ui.set_ifd_page("theme".into());
        f.grab(&ui);
        let card = f.bbox_of(popup, 2, (0, 0, W, H)).unwrap();
        ui.set_theme_pick_x((card.2 - 60) as f32);
        ui.set_theme_pick_y((card.1 + 60) as f32);
        ui.set_theme_pick_open(true);
        f.grab(&ui);
        dump(&f, "theme-pick-in-card");
    }
}

/// (#theme-search-redesign 2026-10-16) 搜索框必须是**输入框形态**，与
/// `PickerInput`（color_picker.slint:159-173）/ `MiniInput`
/// （session_dialog.slint:557-568）同款：常驻 field 底 + border-control 边，
/// 聚焦时边转 accent + 外扩 3px accent-soft 柔光环。
///
/// 用户反馈的是「聚焦后样式都不一致，不怎么协调」。旧实现是**扁平行**
/// （自身 transparent、无底无边），聚焦时才凭空冒一圈柔光环 —— 那一帧
/// 整行像被套了个更大的框，与浮层内其它控件不成一套。本测试从像素侧
/// 钉住形态：
///   ① 未聚焦时行内**有** field 底色（不再 transparent）；
///   ② 聚焦时边框转 accent 且出现柔光环（外扩 3px）；
///   ③ 柔光环外径 = 行高 + 6（同心 8+3=11），不是「比行宽还宽的整行框」。
#[test]
fn theme_search_row_is_an_input_not_a_flat_bar() {
    let ui = setup();
    let mut f = Frame::new();
    let theme = ui.global::<Theme>();
    let field = brush_rgb(&theme.get_field());
    let accent = color_rgb(theme.get_accent_solid());
    let dist = |a: (u8, u8, u8), b: (u8, u8, u8)| {
        (a.0 as i32 - b.0 as i32).abs()
            + (a.1 as i32 - b.1 as i32).abs()
            + (a.2 as i32 - b.2 as i32).abs()
    };

    ui.set_theme_entries(ModelRc::from(Rc::new(slint::VecModel::from(vec![
        row("t1", "MTerm · 暗", "暗", true),
        row("t2", "原子 · 暗", "暗", false),
    ]))));
    ui.set_theme_total(2);
    ui.set_theme_chosen("t1".into());
    ui.set_theme_pick_x(120.0);
    ui.set_theme_pick_y(120.0);
    ui.set_theme_pick_open(true);
    f.grab(&ui);
    let idle = f.snapshot();
    if std::env::var("THEME_DUMP_DIR").is_ok() {
        dump(&f, "theme-search-input-idle");
    }

    // 搜索行定位：不能用「field 底色包围盒」—— field 色在窗口别处也有，
    // 整帧 bbox 会跨几百像素。也不能只看某一行：行有 8px 圆角，靠上下缘
    // 的行会被圆角切掉两侧（实测同一行取到 332px 而非 348px）。改为在浮层
    // 顶部扫**field 色连续段最宽的那一行** —— 那正是行的垂直中线，此处圆角
    // 不咬边，量到的就是完整内容宽 348（浮层 360 − 2×6 内衬）。
    let mut best: Option<(usize, usize, usize)> = None; // (width, y, x0)
    for y in 120..200 {
        let mut run: Option<usize> = None;
        for x in 0..W {
            let (r, g, b, a) = f.px(x, y);
            let hit = a > 0 && dist((r, g, b), field) < 6;
            if hit {
                if run.is_none() {
                    run = Some(x);
                }
            } else if let Some(st) = run {
                let w = x - st;
                if w >= 300 && best.map(|(bw, _, _)| w > bw).unwrap_or(true) {
                    best = Some((w, y, st));
                }
                run = None;
            }
        }
    }
    let (seg_w, sy, sx0) = best.expect(
        "未聚焦的搜索行就该有 field 底色（旧扁平行是 transparent，整行没有）\
         —— 在浮层顶部找不到≥300px 宽的 field 色横向段",
    );
    let sx1 = sx0 + seg_w - 1;
    assert!(
        (340..=350).contains(&seg_w),
        "搜索行内容宽应≈348px（浮层 360 − 2×6 内衬），实测 {seg_w}（y={sy} x {sx0}..{sx1}）"
    );

    // 聚焦：点进输入框 → 边转 accent + 外扩柔光环。
    let cx = (sx0 + sx1) / 2;
    let cy = sy;
    click_at(&ui, cx as f32, cy as f32);
    settle();
    f.grab(&ui);
    if std::env::var("THEME_DUMP_DIR").is_ok() {
        dump(&f, "theme-search-input-focused");
    }

    // 柔光环：聚焦后在**边框外侧**出现 accent-soft 的 3px 环。
    //
    // ⚠️ 判据不能写成「变色后更靠近 accent」—— `accent-soft` 是**半透明**
    // accent，叠在 popup 底上，混色结果离 popup 底更近（实测 idle (35,38,45)
    // → focused (40,54,73)：离 popup 88、离 accent 277）。正确判据是
    // **相对 idle 那一帧更靠近 accent**（326 → 277），这才是"往 accent 走"。
    // 另需与 idle 逐点对照，否则无环时那些弱条件也可能成立。
    let hx = sx0.saturating_sub(3);
    let halo_span = (sy - 10..sy + 11)
        .filter(|&y| {
            let before = idle.px3(hx, y);
            let after = f.px3(hx, y);
            dist(after, before) > 6 && dist(after, accent) < dist(before, accent)
        })
        .count();
    assert!(
        halo_span >= 12,
        "聚焦后边框外 3px（x={hx}）应出现 accent-soft 柔光环并纵向贯通，\
         实测 {halo_span}/21 行变色且向accent 靠拢（搜索行 y={sy} x={sx0}..{sx1}）"
    );

    // 聚焦时框内底色仍是 field（输入框形态不因聚焦换底）。
    let inner = { let (r, g, b) = f.px3(cx, sy); (r, g, b) };
    assert!(
        dist(inner, field) < 12,
        "聚焦时框内底色应仍是 Theme.field：{:?} vs {:?}",
        inner,
        field
    );

    // 聚焦时**边框**转 accent。
    //
    // ⚠️ 取样坐标有两个坑（都实测踩过）：
    //  ① `sx0` 是**框内 field 底**的起点，不是边框 —— 未聚焦时边框画的是
    //     border-control，与 field 色差 >6，所以 field 检测从边框**内侧**起算
    //     （实测 idle：x126=边框 (49,53,62)、x127 起才是 field (25,27,33)）。
    //  ② 边框**外侧** x123..125 是柔光环（focused accent-soft (40,54,73)），
    //     再外 x121 起是 popup 底。三段都不能当"边框"取样。
    //
    // 正确做法：idle 最宽 field 段起点 sx0 再**外扩 1~2px** —— 那一列正是
    // 边框。用「idle → focused 同一坐标变色 + 变化后更靠近 accent」双向确认，
    // 避开对具体像素值的硬编码。
    let bx = sx0.saturating_sub(2);
    let border_idle = idle.px3(bx, sy);
    let border_focused = f.px3(bx, sy);
    assert!(
        dist(border_focused, border_idle) > 10,
        "聚焦时搜索框边框（x={bx}）应变色：idle {:?} → focused {:?}\
         （边框位置推错：sx0={sx0}，边框应在 sx0−2 附近）",
        border_idle,
        border_focused
    );
    assert!(
        dist(border_focused, accent) < dist(border_idle, accent),
        "聚焦后边框应更靠近 accent：idle {:?} → focused {:?}（accent {:?}）",
        border_idle,
        border_focused,
        accent
    );

    if std::env::var("THEME_DUMP_DIR").is_ok() {
        dump(&f, "theme-search-input-focused");
    }
}
