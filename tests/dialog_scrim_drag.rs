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
use slint::{ Brush, ComponentHandle, LogicalPosition, LogicalSize};
use std::rc::Rc;

const W: usize = 1400;
const H: usize = 900;

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
