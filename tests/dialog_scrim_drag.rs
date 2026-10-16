//! (#scrim-unify 2026-10-15) 弹框遮罩统一 + 关闭弹框空白区拖动 —— 渲染级回归。
//!
//! 用户反馈（附两张截图）：「同步输入弹框没有遮罩」「新建会话 / 设置 / 主界面
//! 右上角关闭的弹框这几个遮罩好像又不一样了」「关闭弹框的效果也要和其它弹框
//! 一致，支持透明度，支持空白处拖动」。
//!
//! 根因：每个弹框各写各的遮罩色（#00000080 / #00000088 / #000000aa，同步输入
//! 干脆没有），既不看深浅色也不看「界面通透度」三档；关闭弹框的卡片还没有
//! 空白区拖动。修法 = 全部收敛到 Theme.modal-scrim / Theme.popup-bg 两个令牌，
//! 并给关闭弹框补上与会话弹框同款的拖动。
//!
//! 为什么是独立集成测试而不是 src 里的单测：
//!   ① 要断言**渲染后的像素**（遮罩是用户看到的效果），必须自建
//!      MinimalSoftwareWindow 平台拿整帧 —— Slint 测试后端不暴露像素；
//!   ② `set_platform` 一个进程只能装一次，独立测试文件 = 独立进程，不会和
//!      `src/picker/ui_input_test.rs` 已装的测试后端打架；
//!   ③ 同一进程里多个窗口共用一个线程局部适配器，所以本文件的平台相关断言
//!      合并在**一个** `#[test]` 里顺序执行（Slint 官方测试的同款约束）。
//!
//! 断言口径：
//!   * 统一：四个弹框（新建会话 / 设置 / 同步输入 / 关闭主窗口）在同一采样点
//!     的像素必须等于 `mix(无弹框基线, Theme.modal-scrim, alpha)` —— 即全部
//!     渲染同一个主题令牌，而不是各写各的 hex。同步输入顺带证明遮罩真的画出来
//!     了（修复前该点与基线逐字节相同）。
//!   * 透明度：关闭弹框卡片底在「标准」档不透明、切到「通透」档变成玻璃
//!     （popup-alpha 跟随设置），即用户要求的「支持透明度」。
//!   * 内衬透明度（#popup-inset-alpha 2026-10-16）：关闭方式两块**内衬卡**
//!     也要跟随通透档 —— 旧写法 `bg-root` 在无壁纸时不透明，把弹窗玻璃
//!     「焊死」，只有悬浮（chip 半透明）才透（用户截图：中间两块没有通透性）。
//!     判据用**棋盘壁纸**做高频背景：无壁纸帧 vs 棋盘帧的「变化像素比例」
//!     （修复前实心 = 0%；修复后透 = 白格覆盖处整体变化，≥5%）。
//!   * 拖动：标题行空白处按下并移动 (+120,+80)，卡片随指针位移（阈值判别 +
//!     窗口内钳制与 SessionDialog 同款）。
//!
//! 运行：`cargo test --test dialog_scrim_drag`（不需要 SLINT_EMIT_DEBUG_INFO，
//! 全部断言走像素，不依赖元素调试信息）。

use mterm::ui::{AppWindow, SyncInputState, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent};
use slint::{
    Brush, ComponentHandle, Image, LogicalPosition, LogicalSize, Rgba8Pixel, SharedPixelBuffer,
};
use std::rc::Rc;

const W: usize = 1400;
const H: usize = 900;

/// 关闭方式弹框「完全退出」卡左半的可扫描区（绝对逻辑 px）。
/// 关闭卡 440×255 居中 → 内衬卡 y 418.5~500.5、exit 卡 x 705~902；
/// 内缩 6px 避开 1px 边框与圆角，再取左半避开图标（水平居中 ~792~814）
/// 与文字（主/副标签宽 ~72px，居中 ≥767）。真值以 SCRIM_DUMP_DIR 导出的
/// 证据图复核为准（#popup-inset-alpha）。
const EXIT_SCAN: (usize, usize, usize, usize) = (712, 426, 768, 494);

/// 会话弹框「名称」输入框内的可扫描带（绝对逻辑 px；证据图实测：框体
/// x 422~691、y 237~272，placeholder 文字占 x 433~556）。取"全框内缩"
/// 426~686 × 246~266：既覆盖文字右侧空白、又横跨棋盘格边界（格宽 140px，
/// 跨边界才保证有"白格覆盖区"参与变化检测，单格落在黑格会假阴性）。
const DIALOG_FIELD_SCAN: (usize, usize, usize, usize) = (426, 246, 686, 266);

thread_local! {
    static WIN: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}

/// 离屏平台：窗口适配器 = 本线程的 MinimalSoftwareWindow（Slint 官方测试同款）。
struct ProbePlatform;

impl slint::platform::Platform for ProbePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(WIN.with(|w| w.clone()))
    }
}

/// 持久帧缓冲：渲染器只重绘脏区，未脏区保留上一帧内容（真实帧缓冲的行为），
/// 所以缓冲必须在多次 grab 之间复用，不能每次清零。
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
        let p = self.buf[y * W + x];
        if p.alpha == 0 {
            return (0, 0, 0, 0);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue), p.alpha)
    }

    /// 颜色 C（容差 tol）的像素包围盒，限定在给定区域内。
    fn bbox_of(&self, c: (u8, u8, u8), tol: u8, region: (usize, usize, usize, usize)) -> Option<(usize, usize, usize, usize)> {
        let (x0, y0, x1, y1) = region;
        let mut min = (usize::MAX, usize::MAX);
        let mut max = (0usize, 0usize);
        let mut found = false;
        for y in y0..y1 {
            for x in x0..x1 {
                let (r, g, b, a) = self.px(x, y);
                if a == 0 {
                    continue;
                }
                if r.abs_diff(c.0) <= tol && g.abs_diff(c.1) <= tol && b.abs_diff(c.2) <= tol {
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
}

fn brush_solid(brush: &Brush) -> Option<(u8, u8, u8, u8)> {
    match brush {
        Brush::SolidColor(c) => Some((c.red(), c.green(), c.blue(), c.alpha())),
        _ => None,
    }
}

/// 可选导出当前帧为 PPM（P6，直通 RGBA→RGB）。设了环境变量
/// `SCRIM_DUMP_DIR` 就跑证据图，正常 `cargo test` 不产文件。
fn maybe_dump(f: &Frame, name: &str) {
    let Ok(dir) = std::env::var("SCRIM_DUMP_DIR") else { return };
    let mut data = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            let (r, g, b, _) = f.px(x, y);
            data.extend_from_slice(&[r, g, b]);
        }
    }
    let path = std::path::Path::new(&dir).join(format!("{}.ppm", name));
    let header = format!("P6\n{} {}\n255\n", W, H);
    std::fs::write(&path, header.as_bytes()).and_then(|_| std::fs::write(&path, [header.as_bytes(), &data].concat())).ok();
    eprintln!("dumped {}", path.display());
}

/// base 上按 alpha 叠 over（与 Slint 的源-over 混色一致）。
fn mix(base: (u8, u8, u8), over: (u8, u8, u8), alpha: f32) -> (u8, u8, u8) {
    let f = |b: u8, o: u8| (b as f32 * (1.0 - alpha) + o as f32 * alpha).round() as u8;
    (f(base.0, over.0), f(base.1, over.1), f(base.2, over.2))
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: u8) -> bool {
    a.0.abs_diff(b.0) <= tol && a.1.abs_diff(b.1) <= tol && a.2.abs_diff(b.2) <= tol
}

/// 400×400、40px 格的黑白棋盘（`image-fit: cover` 放大 ~3.5 倍铺满 1400×900）。
/// 用作"高频背景"：不透明表面 vs 半透明表面在上面的像素变化比例差异极大
/// （实心 ≈0%，玻璃 ≥5%），是颜色断言在"同色系收敛"场景下的替代判据。
fn checker_image() -> Image {
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(400, 400);
    {
        let px = buf.make_mut_slice();
        for y in 0..400 {
            for x in 0..400 {
                let white = (x / 40 + y / 40) % 2 == 0;
                let v = if white { 255u8 } else { 0u8 };
                px[y * 400 + x] = Rgba8Pixel { r: v, g: v, b: v, a: 255 };
            }
        }
    }
    Image::from_rgba8(buf)
}

fn setup() -> AppWindow {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();
    ui
}

fn press(ui: &AppWindow, x: f32, y: f32) {
    let p = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: p,
        button: PointerEventButton::Left,
    });
}

fn drag_to(ui: &AppWindow, x: f32, y: f32) {
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(x, y),
    });
}

fn release(ui: &AppWindow, x: f32, y: f32) {
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: LogicalPosition::new(x, y),
        button: PointerEventButton::Left,
    });
}

#[test]
fn dialog_scrims_unified_and_close_dialog_draggable() {
    let ui = setup();
    let mut f = Frame::new();
    f.grab(&ui); // 基线：无弹框

    // 采样点：右下角，避开四个弹框卡片与会话卡（设置卡默认居中 960×640，
    // 关闭卡 440×255 居中，同步卡 680×480 居中，会话卡 ≤600 宽居中）。
    let sample = (W - 24, H - 24);
    let base = { let (r, g, b, _) = f.px(sample.0, sample.1); (r, g, b) };
    // 窗口中心背后的基线（弹框卡片正下方）：通透档玻璃期望值以它为底。
    let base_center = { let (r, g, b, _) = f.px(W / 2, H / 2); (r, g, b) };

    let theme = ui.global::<Theme>();
    let (sr, sg, sb, sa) =
        brush_solid(&theme.get_modal_scrim()).expect("modal-scrim 应为纯色");
    let scrim_alpha = sa as f32 / 255.0;
    let expect = mix(base, (sr, sg, sb), scrim_alpha);
    assert!(scrim_alpha > 0.0, "modal-scrim 不应全透明");

    // ── ① 四个弹框的遮罩必须渲染同一个主题令牌 ─────────────────────────
    let openers: [(&str, fn(&AppWindow), fn(&AppWindow)); 4] = [
        ("新建会话", |u| u.set_dialog_open(true), |u| u.set_dialog_open(false)),
        ("设置", |u| u.set_interface_open(true), |u| u.set_interface_open(false)),
        (
            "同步输入",
            |u| u.global::<SyncInputState>().set_dialog_open(true),
            |u| u.global::<SyncInputState>().set_dialog_open(false),
        ),
        (
            "关闭主窗口",
            |u| u.set_confirm_close_open(true),
            |u| u.set_confirm_close_open(false),
        ),
    ];
    let mut seen: Vec<((u8, u8, u8), &str)> = Vec::new();
    for (name, open, close_dlg) in openers {
        open(&ui);
        f.grab(&ui);
        let got = { let (r, g, b, _) = f.px(sample.0, sample.1); (r, g, b) };
        assert!(
            close(got, expect, 3),
            "{} 弹框遮罩应等于 modal-scrim 混色 {:?}，实得 {:?}（基线 {:?}，alpha {}）",
            name,
            expect,
            got,
            base,
            scrim_alpha
        );
        // 同步输入弹框修复前根本没有遮罩：该点会与基线逐字节相同。
        assert!(
            !close(got, base, 1),
            "{} 弹框遮罩应可见（与无弹框基线不同）",
            name
        );
        seen.push((got, name));
        maybe_dump(&f, &format!("scrim-{}", name));
        close_dlg(&ui);
        f.grab(&ui);
    }
    // 四个弹框彼此也必须同色（统一的最直白判据）。
    for i in 1..seen.len() {
        assert!(
            close(seen[i].0, seen[0].0, 2),
            "{} 与 {} 的遮罩颜色不一致：{:?} vs {:?}",
            seen[i].1,
            seen[0].1,
            seen[i].0,
            seen[0].0
        );
    }

    // 附带：设置弹框「显示与字体」页证据图（#cursor-seg 自检 —— 光标类型
    // 行由三按钮改为 SegGroup 滑动分段，此处留帧供证据图复核分段宽度）。
    ui.set_interface_open(true);
    ui.set_ifd_page("font".into());
    f.grab(&ui);
    maybe_dump(&f, "settings-font");
    ui.set_interface_open(false);
    ui.set_ifd_page("theme".into());
    f.grab(&ui);

    // ── ② 关闭弹框卡片透明度跟随「界面通透度」 ─────────────────────────
    // 标准档：popup-alpha = 1.0，卡片不透明。
    ui.set_confirm_close_open(true);
    f.grab(&ui);
    let (cr, cg, cb, ca) =
        brush_solid(&theme.get_popup_bg()).expect("popup-bg 应为纯色");
    let card_rgb = (cr, cg, cb);
    let std_card = {
        let (r, g, b, _) = f.px(sample.0 / 2, H / 2);
        (r, g, b)
    };
    // 卡片中心（窗口中心即卡片中心：440×255 居中于 1400×900）。
    let center = { let (r, g, b, _) = f.px(W / 2, H / 2); (r, g, b) };
    assert!(
        close(center, card_rgb, 3) && ca == 255,
        "标准档卡片应不透明且等于 popup-bg {:?}，实得 {:?}（alpha {}）",
        card_rgb,
        center,
        ca
    );
    // 通透档：popup-alpha = 0.82，卡片变玻璃（与背后的遮罩混色）。
    ui.set_popup_transparency(1);
    f.grab(&ui);
    let glass = { let (r, g, b, _) = f.px(W / 2, H / 2); (r, g, b) };
    let expect_glass = mix(mix(base_center, (sr, sg, sb), scrim_alpha), card_rgb, 0.82);
    assert!(
        close(glass, expect_glass, 4),
        "通透档卡片应为 82% 玻璃 {:?}，实得 {:?}",
        expect_glass,
        glass
    );
    assert!(
        !close(glass, std_card, 3),
        "通透档卡片应与标准档可见不同（{:?} vs {:?}）",
        glass,
        std_card
    );
    // 遮罩在通透档也要变重（scrim-alpha +0.10），兜住弹窗之上的可读性。
    let glass_scrim = { let (r, g, b, _) = f.px(sample.0, sample.1); (r, g, b) };
    let expect_scrim1 = mix(base, (sr, sg, sb), (scrim_alpha + 0.10).min(0.60));
    assert!(
        close(glass_scrim, expect_scrim1, 4),
        "通透档遮罩应加深 +0.10 → {:?}，实得 {:?}",
        expect_scrim1,
        glass_scrim
    );
    maybe_dump(&f, "close-glass-通透");
    ui.set_popup_transparency(0);
    f.grab(&ui);

    // ── ②c 关闭方式两块内衬卡跟随通透档（#popup-inset-alpha）────────────
    // 用户截图：通透档下两块卡片没有通透性（旧 bg-root 无壁纸时不透明），
    // 只有悬浮（chip 半透明）才透。颜色断言在此不可行 —— root 与 panel 只差
    // 2 级、经 alpha 收敛后 <1.5 级（容差内不可辨）；改用**棋盘壁纸做高频
    // 背景**：同一档位下「无壁纸帧 → 棋盘帧」的变化像素比例。修复前内衬是
    // 实心 root，切换壁纸纹丝不动（≈0%）；修复后内衬随档透出棋盘（≥5%）。
    let scan = |f: &Frame, (x0, y0, x1, y1): (usize, usize, usize, usize)| -> Vec<(u8, u8, u8)> {
        let mut v = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let (r, g, b, a) = f.px(x, y);
                if a > 0 {
                    v.push((r, g, b));
                }
            }
        }
        v
    };
    let avg_rb = |v: &[(u8, u8, u8)]| -> f32 {
        v.iter().map(|p| p.0 as f32 - p.2 as f32).sum::<f32>() / v.len() as f32
    };

    // 护栏：悬停 exit 卡 → 扫描区必须出现 hover 红染（证明区域落在卡内；
    // 偏到弹框底会假阳性，因为弹框底在通透档本来就透）。
    // ⚠️ 卡片底有 120ms animate：**动画只在处理指针事件时推进**（i-slint-core
    // window.rs `process_mouse_input` 首行 `update_animations(now)`；渲染器
    // 自己不推进）。sleep 完必须**再派发一次同坐标 PointerMoved**，否则渲染到
    // 的仍是动画起点色（首版实测：无 animate 的图标 tint 已变红、卡底纹丝不动）。
    let hover = |ui: &AppWindow, x: f32, y: f32| {
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(x, y),
        });
    };
    hover(&ui, 100.0, 100.0);
    f.grab(&ui);
    let no_hover = avg_rb(&scan(&f, EXIT_SCAN));
    hover(&ui, 803.0, 459.0); // exit 卡中心
    f.grab(&ui); // 起始帧（动画 0%）
    std::thread::sleep(std::time::Duration::from_millis(170));
    hover(&ui, 803.0, 459.0); // 推进到位的同坐标重发
    f.grab(&ui);
    maybe_dump(&f, "inset-hover-exit");
    let hovered = avg_rb(&scan(&f, EXIT_SCAN));
    assert!(
        hovered - no_hover >= 10.0,
        "exit 卡悬停应出现红染（r-b 均值 {no_hover:.1} → {hovered:.1}）—— \
         扫描区可能不在卡内（EXIT_SCAN 需按证据图复核）"
    );
    hover(&ui, 100.0, 100.0);
    f.grab(&ui);
    std::thread::sleep(std::time::Duration::from_millis(170));
    hover(&ui, 100.0, 100.0); // 恢复同样要推进动画
    f.grab(&ui);

    // 全透档（popup-alpha 0.55）：同一档位下对比「无壁纸 vs 棋盘壁纸」。
    // 棋盘 400×400、40px 格，`image-fit: cover` 放大 ~3.5 倍铺满窗口。
    // ⚠️ 内衬背景带 120ms animate：档位/壁纸变更**不是指针事件**，动画不会
    // 自己推进 —— 每次变更后必须补一次 tick（sleep + 同坐标 PointerMoved），
    // 否则渲染停在"上次推进的值"（首版两个帧都量到实色 root，误判修复没生效）。
    ui.set_popup_transparency(2);
    f.grab(&ui);
    std::thread::sleep(std::time::Duration::from_millis(150));
    hover(&ui, 100.0, 100.0); // tick：推进内衬动画到档 2 目标值
    f.grab(&ui);
    maybe_dump(&f, "inset-base-np");
    let base_np = scan(&f, EXIT_SCAN);
    // 令牌契约（#popup-inset-alpha）：inset = root 色 × 当前 popup-alpha。
    // 像素断言之外的第二道保险，锁住"修复的实现核心"——档 2 下 alpha 必须是
    // 140（0.55×255）而不是实色 255；若先在令牌层失败，报错信息比
    // "变化像素 0%" 更指向根因。
    eprintln!(
        "diag档2: inset={:?} popup-bg={:?} bg-root={:?} alpha={}",
        brush_solid(&theme.get_popup_bg_inset()),
        brush_solid(&theme.get_popup_bg()),
        brush_solid(&theme.get_bg_root()),
        theme.get_popup_alpha()
    );
    let (_, _, _, ia) = brush_solid(&theme.get_popup_bg_inset()).expect("popup-bg-inset 应为纯色");
    assert!(
        (ia as i32 - 140).abs() <= 1,
        "档 2 下 popup-bg-inset 的 alpha 应 = 140（0.55×255，跟随弹窗通透档），实得 {ia}"
    );

    ui.set_wallpaper_img(checker_image());
    ui.set_wallpaper_active(true);
    f.grab(&ui);
    std::thread::sleep(std::time::Duration::from_millis(150));
    hover(&ui, 100.0, 100.0); // tick：壁纸激活同样要让内衬动画就位
    f.grab(&ui);
    f.grab(&ui); // 壁纸上传后的稳定帧
    maybe_dump(&f, "inset-checker");
    let with_wp = scan(&f, EXIT_SCAN);

    let changed = base_np
        .iter()
        .zip(with_wp.iter())
        .filter(|(a, b)| a.0.abs_diff(b.0) > 8 || a.1.abs_diff(b.1) > 8 || a.2.abs_diff(b.2) > 8)
        .count();
    let ratio = changed as f32 / base_np.len() as f32;
    assert!(
        ratio >= 0.05,
        "全透档下内衬卡应透出棋盘壁纸（变化像素 {changed}/{} = {:.1}%）—— \
         修复前内衬被不透明底焊死，此处应为 ≈0%",
        base_np.len(),
        ratio * 100.0
    );

    ui.set_wallpaper_active(false);
    ui.set_popup_transparency(0);
    f.grab(&ui);
    std::thread::sleep(std::time::Duration::from_millis(150));
    hover(&ui, 100.0, 100.0); // tick：恢复档 0 实色后再进 ③ 段
    f.grab(&ui);

    // ── ②d 会话弹框的输入框同样跟随通透档（#popup-surface-alpha）──────────
    // 用户第二轮反馈「其它弹框的没生效，比如会话的输入框」：弹框内的内容表面
    // （field / bg-root / bg-elevated…）此前是不透明常量，通透档下就是一块块
    // 实心贴片。判据同 ②c（棋盘帧 vs 无壁纸帧的变化像素比例），扫描「名称」
    // 输入框内一处空白带（DIALOG_FIELD_SCAN）。
    ui.set_confirm_close_open(false);
    ui.set_dialog_open(true);
    f.grab(&ui);
    maybe_dump(&f, "dlg-field-std");
    // 护栏：档 0 时扫描区应基本等于 popup-field 的实色值（证明区域落在输入
    // 框内 —— 偏到弹框底会让下面的变化断言失去意义）。
    let want_field = brush_solid(&theme.get_popup_field()).expect("popup-field 应为纯色");
    let field_stats = |f: &Frame| -> (usize, usize) {
        let (x0, y0, x1, y1) = DIALOG_FIELD_SCAN;
        let (mut hit, mut total) = (0usize, 0usize);
        for y in y0..y1 {
            for x in x0..x1 {
                let (r, g, b, a) = f.px(x, y);
                if a == 0 {
                    continue;
                }
                total += 1;
                if r.abs_diff(want_field.0) <= 6
                    && g.abs_diff(want_field.1) <= 6
                    && b.abs_diff(want_field.2) <= 6
                {
                    hit += 1;
                }
            }
        }
        (hit, total)
    };
    let (fh, ft) = field_stats(&f);
    assert!(
        fh * 2 > ft,
        "档 0 时名称输入框扫描区应多数为 field 底色（{fh}/{ft} 命中，期望 {want_field:?}）—— \
         区域可能不在输入框内（DIALOG_FIELD_SCAN 需按证据图复核）"
    );

    // 档 2 + 棋盘：输入框表面应透出棋盘（tick 规则同 ②c：动画只在指针事件
    // 推进；field 的底虽多为直接绑定，仍按同款保险处理）。
    ui.set_popup_transparency(2);
    f.grab(&ui);
    std::thread::sleep(std::time::Duration::from_millis(150));
    hover(&ui, 100.0, 100.0); // tick
    f.grab(&ui);
    maybe_dump(&f, "dlg-field-base");
    let dlg_base = scan(&f, DIALOG_FIELD_SCAN);

    ui.set_wallpaper_img(checker_image());
    ui.set_wallpaper_active(true);
    f.grab(&ui);
    std::thread::sleep(std::time::Duration::from_millis(150));
    hover(&ui, 100.0, 100.0); // tick
    f.grab(&ui);
    f.grab(&ui);
    maybe_dump(&f, "dlg-field-checker");
    let dlg_wp = scan(&f, DIALOG_FIELD_SCAN);

    let dlg_changed = dlg_base
        .iter()
        .zip(dlg_wp.iter())
        .filter(|(a, b)| a.0.abs_diff(b.0) > 8 || a.1.abs_diff(b.1) > 8 || a.2.abs_diff(b.2) > 8)
        .count();
    let dlg_ratio = dlg_changed as f32 / dlg_base.len() as f32;
    assert!(
        dlg_ratio >= 0.05,
        "全透档下会话弹框的输入框应透出棋盘壁纸（变化像素 {dlg_changed}/{} = {:.1}%）—— \
         修复前 field 是不透明常量，此处应为 ≈0%",
        dlg_base.len(),
        dlg_ratio * 100.0
    );

    // 收尾：关会话弹框、恢复壁纸/档位，并把关闭弹框重开（③ 段的前置状态）。
    ui.set_wallpaper_active(false);
    ui.set_dialog_open(false);
    ui.set_popup_transparency(0);
    ui.set_confirm_close_open(true);
    f.grab(&ui);
    std::thread::sleep(std::time::Duration::from_millis(150));
    hover(&ui, 100.0, 100.0); // tick
    f.grab(&ui);

    // ── ③ 空白区拖动关闭弹框卡片 ───────────────────────────────────────
    let bbox0 = f
        .bbox_of(card_rgb, 4, (W / 2 - 260, H / 2 - 160, W / 2 + 260, H / 2 + 160))
        .expect("应能找到不透明的关闭弹框卡片");
    // 按下点：卡片内左上区域（标题行留白，落在卡片吞层上；✕ 在右上不受影响）。
    let (cx0, cy0, _, _) = bbox0;
    let px0 = (cx0 + 20) as f32;
    let py0 = (cy0 + 20) as f32;
    let (dx, dy) = (120.0f32, 80.0f32);
    press(&ui, px0, py0);
    drag_to(&ui, px0 + dx, py0 + dy);
    release(&ui, px0 + dx, py0 + dy);
    f.grab(&ui);
    maybe_dump(&f, "close-dragged");
    let bbox1 = f
        .bbox_of(card_rgb, 4, (W / 2 - 400, H / 2 - 300, W / 2 + 400, H / 2 + 300))
        .expect("拖动后应仍能找到卡片");
    let moved_x = bbox1.0 as f32 - bbox0.0 as f32;
    let moved_y = bbox1.1 as f32 - bbox0.1 as f32;
    assert!(
        (moved_x - dx).abs() <= 3.0 && (moved_y - dy).abs() <= 3.0,
        "空白区拖动应把卡片移动 ({}, {})，实得 ({}, {})",
        dx,
        dy,
        moved_x,
        moved_y
    );

    // 拖动后点击 ✕ 仍能关（拖动不曾破坏卡片交互）。
    ui.set_confirm_close_open(false);
    f.grab(&ui);
    let after = { let (r, g, b, _) = f.px(sample.0, sample.1); (r, g, b) };
    assert!(close(after, base, 3), "关闭后应恢复无弹框基线 {:?}，实得 {:?}", base, after);
}

/// 遮罩来源审计：模态弹框的遮罩必须引用 `Theme.modal-scrim`，不允许再写死 hex。
/// 这次用户反馈「几个遮罩不一样」的根因就是各弹框各写各的色值；用一条测试
/// 把收敛结果钉住，防止以后新弹框再漂移。
#[test]
fn modal_dialog_scrims_come_from_the_theme_token() {
    let ui_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");

    // (a) 独立弹框组件 / 内联弹框遮罩层 → 锚点后的第一个 background 必须是令牌。
    //     每项 = (文件, 锚点子串, 说明)。
    let anchors: &[(&str, &str, &str)] = &[
        ("settings_dialog.slint", "export component SettingsDialog", "设置卡"),
        ("confirm_dialog.slint", "export component ConfirmDialog", "通用确认弹框"),
        ("session_dialog.slint", "export component SessionDialog", "新建会话弹框"),
        ("sync_input_dialog.slint", "if SyncInputState.dialog-open : Rectangle", "同步输入弹框"),
        ("proc_window.slint", "if root.terminate-dialog-open : Rectangle", "进程窗口结束确认"),
        ("app.slint", "if root.group-dialog-open : Rectangle", "新建/重命名组"),
        ("app.slint", "if root.tab-rename-open : Rectangle", "重命名会话"),
        ("app.slint", "if root.quick-cmd-manage-open : Rectangle", "快捷指令管理"),
        ("app.slint", "if root.qcg-open : Rectangle", "快捷指令组名"),
        ("app.slint", "if root.sftp-prompt-open : Rectangle", "SFTP 提示"),
        ("app.slint", "if root.confirm-close-open : Rectangle", "关闭主窗口"),
        ("app.slint", "if root.chmod-open : Rectangle", "修改文件权限"),
        // 这两个是 `visible:` 常驻元素（不是 `if` 条件元素），锚点用标题注释，
        // 且 background 在 visible 之后 —— 顺序扫描不会串到下一个元素。
        ("app.slint", "// --- Keyboard shortcuts dialog", "快捷键说明"),
        ("app.slint", "// --- About dialog", "关于"),
        ("app.slint", "if root.batch-import-open : Rectangle", "批量导入"),
        ("app.slint", "if root.cred-prompt-open : Rectangle", "凭据提示"),
        ("app.slint", "if root.mfa-prompt-open : Rectangle", "MFA 提示"),
    ];
    for (file, anchor, what) in anchors {
        let src = std::fs::read_to_string(ui_dir.join(file)).unwrap();
        let idx = src
            .find(anchor)
            .unwrap_or_else(|| panic!("{} 里找不到锚点 {:?}", file, anchor));
        let after = &src[idx..];
        let bg = after
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
            .find(|l| l.starts_with("background:"))
            .unwrap_or_else(|| panic!("{}（{}）锚点后找不到 background", file, what));
        assert!(
            bg.starts_with("background: Theme.modal-scrim;"),
            "{}（{}）的遮罩必须走 Theme.modal-scrim，实得 {:?}",
            file, what, bg
        );
    }

    // (b) 全目录不允许再出现写死的深色遮罩背景。例外只有三处**功能性**用色
    //     （都与"弹框遮罩"无关），逐一注明理由；新增例外必须同步补注释。
    let allowed: &[(&str, &str)] = &[
        // 色轮明度蒙罩：整盘随明度条压暗，是取色功能的一部分。
        ("color_picker.slint", "background: #000000;"),
        // 远端桌面「只读」角标的小底板，要能透出桌面画面。
        ("remote_desktop_view.slint", "background: #00000060;"),
        // 强调色预设弹框的轻遮罩：点空白 = 还原并关闭，设计稿定稿的轻档
        // （docs/design/accent-translucency-hifi-2026-10-13.html）。
        ("app.slint", "background: #080A0E3E;"),
        // 内置编辑器遗留块：`if false && root.editor-open` 恒不渲染（编辑器
        // 已拆为独立 EditorWindow），块按原作者注释保留作源码兼容，不动它。
        ("app.slint", "background: #00000088;"),
    ];
    for entry in std::fs::read_dir(&ui_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("slint") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&path).unwrap();
        for line in src.lines() {
            let t = line.trim();
            if !t.starts_with("background:") {
                continue;
            }
            // 去掉行尾注释再比（白名单里存的是纯代码形式）。
            let code = t.split("//").next().unwrap_or(t).trim_end().to_string();
            // 只抓「纯色写死」：渐变/brush 等装饰性背景（如设置页通透度预览卡
            // 的插画渐变）不是遮罩，不在此列。
            let is_plain_dark_hex = {
                let hex = code
                    .strip_prefix("background:")
                    .map(str::trim)
                    .and_then(|v| v.strip_suffix(';'))
                    .unwrap_or("");
                hex.starts_with('#')
                    && hex.len() >= 4
                    && hex.len() <= 9
                    && hex[1..].chars().all(|c| c.is_ascii_hexdigit())
                    && hex.starts_with("#0")
            };
            if !is_plain_dark_hex {
                continue;
            }
            let ok = allowed.iter().any(|(f, l)| *f == file && code == *l);
            assert!(
                ok,
                "{} 出现写死的深色背景 {:?} —— 弹框遮罩一律走 Theme.modal-scrim；\
                 确为功能性用色请加进 allowed 白名单并注明理由",
                file, code
            );
        }
    }
}

/// 弹框内容表面审计（#popup-surface-alpha 2026-10-16）：用户第二轮反馈
/// 「其它弹框的没生效（比如会话的输入框）」后，弹框系文件内的全部内容表面
/// 统一走 `Theme.popup-*`（= 基础 token × 弹窗通透档 = popup-alpha）；本测试
/// 防止新代码在弹框里写回不透明的表面 token。基础 token 在**主界面/独立
/// 窗口**仍有语义，所以只扫"整文件都属于弹框"的组件。
#[test]
fn dialog_content_surfaces_use_popup_tokens() {
    let ui_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let files = [
        "session_dialog.slint",
        "sync_input_dialog.slint",
        "color_picker.slint",
        "storage_settings.slint",
        "interface_panel.slint",
        "seg.slint",
    ];
    // 弹框内禁用的基础表面 token（弹框内必须用对应 popup 版）。
    let banned = [
        "Theme.field",
        "Theme.bg-root",
        "Theme.bg-panel",
        "Theme.bg-elevated",
        "Theme.bg-active",
        "Theme.panel-solid",
    ];
    // 白名单 = (文件, 行内片段)，全部"有意保留"并注明理由。
    let allowed: &[(&str, &str)] = &[
        // 会话弹框的跳板机 tooltip / 同步输入弹框的悬浮信息卡：瞬时提示层
        // （与 app.slint 的主界面 tooltip 同款语言），不属于弹框内容面。
        ("session_dialog.slint", "background: Theme.bg-elevated;"),
        ("sync_input_dialog.slint", "background: Theme.bg-elevated;"),
        // 设置「界面」页的通透度实时预览卡：本身就是"弹窗底 × popup-alpha"
        // 的演示范例（panel-solid 在此是显式语义，不是漏改）。
        (
            "interface_panel.slint",
            "Theme.panel-solid.with-alpha(Theme.popup-alpha)",
        ),
    ];
    let mut violations: Vec<String> = Vec::new();
    for file in files {
        let src = std::fs::read_to_string(ui_dir.join(file)).unwrap();
        for (i, line) in src.lines().enumerate() {
            let l = line.trim();
            if l.starts_with("//") || !banned.iter().any(|t| l.contains(t)) {
                continue;
            }
            if allowed.iter().any(|(f, frag)| *f == file && l.contains(frag)) {
                continue;
            }
            violations.push(format!("{}:{}: {}", file, i + 1, l));
        }
    }
    assert!(
        violations.is_empty(),
        "弹框系文件不允许再出现不透明的基础表面 token（应改用 Theme.popup-* 对应版）:\n  {}",
        violations.join("\n  ")
    );

    // ── 下拉选项行必须保持「主题下拉行」语言（#select-row-follow-theme）──
    // 三处（协议项 / 分组项 / SelectField）：悬浮底一律 bg-hover、选中文字
    // 一律 accent-solid。改回 field-2/accent-soft 的样式会在这里报红。
    let style_anchors: &[(&str, &str)] = &[
        (
            "session_dialog.slint",
            "background: pita.has-hover ? Theme.bg-hover : transparent;",
        ),
        (
            "session_dialog.slint",
            "color: root.selected ? Theme.accent-solid : Theme.text-primary;",
        ),
        (
            "session_dialog.slint",
            "background: g-ta.has-hover ? Theme.bg-hover : transparent;",
        ),
        (
            "widgets.slint",
            "background: c-ta.has-hover ? Theme.bg-hover : transparent;",
        ),
        (
            "widgets.slint",
            "color: root.value == c ? Theme.accent-solid : Theme.text-primary;",
        ),
    ];
    for (file, fragment) in style_anchors {
        let src = std::fs::read_to_string(ui_dir.join(file)).unwrap();
        assert!(
            src.contains(fragment),
            "{} 缺「主题下拉行」样式锚点 {:?}（#select-row-follow-theme 回归）",
            file,
            fragment
        );
    }

    // ── 红底 × 的 hover 图标必须变白（#close-hover-white）──────────────
    // 三处 #e81123 红底按钮的图标 tint 必须挂 has-hover —— 红底上恒
    // text-primary 会「显暗」（用户反馈「悬浮时 X 变暗」；其中两处曾与
    // 注释 "white glyph" 不符）。
    let close_anchors: &[(&str, &str)] = &[
        (
            "settings_dialog.slint",
            "tint: if-close-ta.has-hover ? white : Theme.text-primary;",
        ),
        (
            "app.slint",
            "tint: close-ta.has-hover ? white : Theme.text-primary;",
        ),
        (
            "proc_window.slint",
            "tint: close-ta.has-hover ? white : Theme.text-secondary;",
        ),
    ];
    for (file, fragment) in close_anchors {
        let src = std::fs::read_to_string(ui_dir.join(file)).unwrap();
        assert!(
            src.contains(fragment),
            "{} 缺红底 × 的白图标锚点 {:?}（#close-hover-white 回归）",
            file,
            fragment
        );
    }

    // ── 设置左菜单选中不得再染底色（#nav-follow-dropdown）──────────────
    // 用户定稿：选中 = 透明底 + 主题色文字（与下拉一致），不回到色块。
    let nav = std::fs::read_to_string(ui_dir.join("interface_panel.slint")).unwrap();
    assert!(
        nav.contains("color: root.active ? Theme.accent-solid : Theme.text-primary;"),
        "NavItem 选中项应为主题色文字（#nav-follow-dropdown 回归）"
    );
    assert!(
        !nav.contains("root.active ? Theme.popup-bg-active"),
        "NavItem 选中项不得再使用底色块（#nav-follow-dropdown 回归）"
    );

    // ── FontCombo 选项行 + 光标类型 SegGroup（2026-10-17）──────────────
    // FontCombo（设置页全部字体/渲染/规则/颜色下拉共用）的选中/悬浮同样
    // 对齐主题下拉行；光标类型由三个独立按钮并入 SegGroup 滑动分段。
    assert!(
        nav.contains("background: r-ta.has-hover ? Theme.bg-hover : transparent;"),
        "FontCombo 选项行悬浮应走 bg-hover（#select-row-follow-theme 回归）"
    );
    assert!(
        nav.contains("color: root.value == v ? Theme.accent-solid : Theme.text-primary;"),
        "FontCombo 选中项应为主题色文字（#select-row-follow-theme 回归）"
    );
    assert!(
        nav.contains("active-index: root.term-cursor-style == \"block\" ? 0"),
        "光标类型应使用 SegGroup（active-index 绑定 term-cursor-style，#cursor-seg 回归）"
    );
    assert!(
        !nav.contains("CursorStyleButton {"),
        "旧 CursorStyleButton 实例不得回归（#cursor-seg）"
    );
}
