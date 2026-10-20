//! (#slider-disabled 2026-10-20) 禁用态滑杆圆点回归。
//!
//! 用户截图反馈：外观设置里「壁纸透明度」滑杆在未选壁纸（enabled=false）
//! 时圆圈里看得到底下的滑条 —— 根因是圆点三层同心圆整体套了 group
//! `opacity: 0.45`，半透明圆点盖不住 4px 轨道（混合色圆盘浮在轨道上）。
//! 修复：禁用态不再降 opacity，改为换灰阶（accent → border-control，与
//! 轨道同阶），间隙保持不透明 panel-solid，圆盘整体始终遮住轨道。
//!
//! MTermSlider inherits Rectangle，slint 不会为其生成 Rust 类型（只有
//! inherits Window 的顶层组件才行），所以本探针走完整 AppWindow：
//! 默认无壁纸（Theme.wallpaper-active=false）+ 打开设置弹窗
//! （interface-open）即落在「外观」页（ifd-page 默认 "theme"），此时
//! 「壁纸透明度」滑杆是禁用态、「分区透明度」四根是启用态 —— 恰好是
//! 同一组件 MTermSlider 的两种状态同屏。
//!
//! 断言（全像素、不猜坐标）：
//!   1. 扫描出外观页里的滑杆轨道（4px 高 border-control 长横条）≥5 条
//!      （壁纸 1 + 分区 4）；
//!   2. 每条轨道左端两个候选圆心（disabled 圆盘与轨道同色融入 → 圆心在
//!      段起点 +10；enabled 圆盘 accent 盖住左端 → 圆心在段起点 −10），
//!      按中心色判别圆盘状态：disabled 盘心 == border-control，enabled
//!      盘心 == accent；
//!   3. 修复前 disabled 盘心是 0.45·accent ⊕ 0.55·轨道 的混合色 → 第 2 步
//!      判不出 disabled 盘（sanity 挂）+ 窗口断言挂，双保险；
//!   4. 圆盘窗口（19×19）内每列「垂直连续 ≥3px 的色块」必须 ∈ 主题
//!      token 集（AA 边缘 1~2px 混色天然被过滤）—— 修复前盘内 14px 高的
//!      混合色 run 直接命中。

use mterm::ui::{AppWindow, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize};
use std::rc::Rc;

const W: usize = 1400;
const H: usize = 900;
type Rgb = (u8, u8, u8);

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
    /// 探针产物（失败时人工查看）：output/*.ppm 是既有惯例。
    fn dump(&self, name: &str) {
        let mut data = Vec::with_capacity(W * H * 3);
        for y in 0..H {
            for x in 0..W {
                let (r, g, b, _) = self.px(x, y);
                data.extend_from_slice(&[r, g, b]);
            }
        }
        let path = std::path::Path::new("output").join(format!("{name}.ppm"));
        let header = format!("P6\n{} {}\n255\n", W, H);
        let _ = std::fs::write(&path, [header.as_bytes(), &data].concat());
        eprintln!("dumped {}", path.display());
    }
}

fn brush_rgb(b: &slint::Brush) -> Rgb {
    match b { slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()), _ => (0, 0, 0) }
}

/// `<color>` 型 token（accent-solid / panel-solid）。
fn color_rgb(c: slint::Color) -> Rgb { (c.red(), c.green(), c.blue()) }

fn near(a: Rgb, b: Rgb, tol: i32) -> bool {
    (a.0 as i32 - b.0 as i32).abs() <= tol
        && (a.1 as i32 - b.1 as i32).abs() <= tol
        && (a.2 as i32 - b.2 as i32).abs() <= tol
}

fn setup() -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();
    ui
}

fn settle() { std::thread::sleep(std::time::Duration::from_millis(180)); }

/// 在 (x, y) 派发滚轮。delta_y < 0 = 向下滚（看滚动内容下方）。
fn scroll(ui: &AppWindow, x: f32, y: f32, dy: f32) {
    ui.window().dispatch_event(WindowEvent::PointerScrolled {
        position: LogicalPosition::new(x, y),
        delta_x: 0.0,
        delta_y: dy,
    });
}

/// 从 (x, y) 向上下扩的垂直连续同色（tol 3、alpha 255）run 长度。
fn vrun(f: &Frame, x: usize, y: usize, c: Rgb, tol: i32) -> usize {
    let mut n = 1usize;
    let mut yy = y;
    while yy > 0 { yy -= 1; let p = f.px(x, yy);
        if p.3 == 255 && near((p.0, p.1, p.2), c, tol) { n += 1 } else { break } }
    let mut yd = y;
    while yd + 1 < H { yd += 1; let p = f.px(x, yd);
        if p.3 == 255 && near((p.0, p.1, p.2), c, tol) { n += 1 } else { break } }
    n
}

/// 窗口内逐列合并垂直同色 run；返回 run ≥3 且不匹配任何 token 的"游离色块"。
/// 窗口 ±5 = 盘心核心区：修复后核心区应为均色 token；修复前整块是
/// 0.45·accent ⊕ 0.55·轨道 的混合色 → 命中。圆周 AA 竖向带在 ±7 之外，
/// 不会被扫到。
fn foreign_blocks(f: &Frame, cx: i32, cy: i32, tokens: &[Rgb]) -> Vec<(i32, i32, Rgb, usize)> {
    let mut bad = vec![];
    for dx in -5i32..=5 {
        let x = (cx + dx) as usize;
        let mut y = cy.saturating_sub(5) as i32;
        let y1 = (cy as i32 + 5).min(H as i32 - 1);
        while y <= y1 {
            let p = f.px(x, y as usize);
            if p.3 != 255 { y += 1; continue; }
            let c: Rgb = (p.0, p.1, p.2);
            let n = vrun(f, x, y as usize, c, 3) as i32;
            if n >= 3 && !tokens.iter().any(|t| near(c, *t, 4)) {
                bad.push((x as i32, y, c, n as usize));
            }
            y += n; // 跳过整段
        }
    }
    bad
}

/// 找滑杆轨道：≥60px 的 border-control 横段（tol 2），中点垂直厚度 3..7
/// （4px 轨道 + 圆角 AA 余量；1px 控件边线被厚度过滤排除）。同一 4px 轨道
/// 会连续 4 行各报一段 → 按 (s/8, e/8) 聚类取中间行。
fn find_tracks(f: &Frame, track: Rgb) -> Vec<(usize, usize, usize)> {
    let mut raw: Vec<(usize, usize, usize)> = vec![];
    for y in 0..H {
        let mut x = 0usize;
        while x < W {
            let p = f.px(x, y);
            if p.3 == 255 && near((p.0, p.1, p.2), track, 2) {
                let s = x;
                while x < W {
                    let q = f.px(x, y);
                    if q.3 == 255 && near((q.0, q.1, q.2), track, 2) { x += 1 } else { break }
                }
                if x - s >= 60 { raw.push((y, s, x - 1)); }
            } else { x += 1; }
        }
    }
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<(usize, usize), Vec<(usize, usize, usize)>> = BTreeMap::new();
    for t in raw { groups.entry((t.1 / 8, t.2 / 8)).or_default().push(t); }
    groups.into_values()
        .map(|mut g| { g.sort_by_key(|t| t.0); g[g.len() / 2] })
        .filter(|&(y, s, e)| { let t = vrun(f, (s + e) / 2, y, track, 2); t >= 3 && t <= 7 })
        .collect()
}

#[test]
fn slider_dot_disabled_is_opaque_and_grey() {
    let ui = setup();
    let (accent, panel, t1, t2, t3, track, popup) = {
        let t = ui.global::<Theme>();
        (
            color_rgb(t.get_accent_solid()),
            color_rgb(t.get_panel_solid()),
            brush_rgb(&t.get_text_primary()),
            brush_rgb(&t.get_text_secondary()),
            brush_rgb(&t.get_text_muted()),
            brush_rgb(&t.get_border_control()),
            brush_rgb(&t.get_popup_bg()),
        )
    };
    // 出厂默认：弹窗不透明 → 卡底 popup-bg 与 panel-solid 同为 base-bg-panel。
    assert!(near(popup, panel, 2), "出厂 popup-alpha=1.0，卡底应与 panel-solid 同色（否则探针判据要重看）");

    ui.set_interface_open(true);
    settle(); settle();

    // 外观页比弹窗可视区高 —— 壁纸/分区透明度滑杆在滚动内容深处。
    // 在设置卡内容区滚轮向下滚，直到滑杆区进入可视（轨道 ≥5 条）。
    let mut f = Frame::new();
    let mut tracks = vec![];
    for _ in 0..14 {
        f.grab(&ui);
        tracks = find_tracks(&f, track);
        if tracks.len() >= 5 { break; }
        scroll(&ui, 700.0, 450.0, -120.0);
        settle();
    }
    f.grab(&ui);
    f.dump("slider-disabled-probe");
    assert!(tracks.len() >= 5,
        "外观页应至少见 5 条滑杆轨道（壁纸 1 + 分区 4），实得 {} 条：{:?}",
        tracks.len(), tracks);

    // ── 2) 每条轨道左端圆盘：圆心都在 s−10 ──
    // 实测几何（探针帧）：圆盘 20px 盖在轨道左端，右缘的 2px 间隙环
    // （卡底色）把「盘心实心」与「轨道」隔开 → 行扫描的 border-control
    // 段起点 s = 圆盘右缘（enabled：accent 盘截断；disabled：间隙环截断）。
    // 两种状态圆心都 = s−10，判别只看盘心色：track → disabled 灰阶，
    // accent → enabled。
    let tokens = [accent, track, panel, popup, t1, t2, t3];
    let mut disabled_dots = 0usize;
    let mut enabled_dots = 0usize;
    for &(y, s, _e) in &tracks {
        // 诊断：轨道左端的实际像素（调试判据用）
        let p0 = f.px(s, y); let p10 = f.px(s + 10, y);
        eprintln!("track y={y} s={s}: px(s)={p0:?} px(s+10)={p10:?} vrun(s+10)={}", vrun(&f, s + 10, y, (p10.0, p10.1, p10.2), 3));
        if s < 30 { continue; }
        let cx = s - 10;
        let (r, g, b, a) = f.px(cx, y);
        if a != 255 { continue; }
        let core = (r, g, b);
        let is_disabled = near(core, track, 3) && vrun(&f, cx, y, track, 3) >= 8;
        let is_enabled = near(core, accent, 3);
        if !is_disabled && !is_enabled { continue; }
        if is_disabled { disabled_dots += 1 } else { enabled_dots += 1 }
        let bad = foreign_blocks(&f, cx as i32, y as i32, &tokens);
        assert!(bad.is_empty(),
            "{}圆点窗口内出现游离混合色：{:?}（track={:?} accent={:?}）",
            if is_disabled { "禁用" } else { "启用" }, bad, track, accent);
    }
    assert!(disabled_dots >= 1,
        "应检到 ≥1 个禁用圆点（壁纸透明度，盘心须为 border-control 灰阶）。\
         若为 0：要么盘心仍是混合色（opacity 未删干净），要么探针定位失效");
    assert!(enabled_dots >= 1,
        "应检到 ≥1 个启用圆点（分区透明度，盘心 accent）——探针 sanity");
}
