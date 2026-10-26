//! 分组管理弹框冒烟：能打开、能渲染、↑↓ 切组、Esc 关闭。
//!
//! 离屏 DPR=1（与 tests/sidebar_boundaries.rs 同一套 MinimalSoftwareWindow），
//! 只验证「接线 + 键盘 + 首帧」这些单测够不到的部分；像素细节不在这里锁。

use mterm::ui::{AppWindow, GmMember, GroupRow, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Key, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalSize, ModelRc, VecModel};
use std::rc::Rc;

const W: usize = 800;
const H: usize = 600;

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

fn group(name: &str, count: i32, reserved: bool) -> GroupRow {
    GroupRow {
        name: name.into(),
        color: slint::Color::default(),
        color_hex: "".into(),
        count,
        hit: count,
        reserved,
    }
}

#[test]
fn group_manager_opens_renders_and_handles_keys() {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));

    ui.set_gm_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        group("默认组", 1, true),
        group("生产", 1, false),
    ]))));
    ui.set_gm_members(ModelRc::from(Rc::new(VecModel::from(vec![GmMember {
        id: "s1".into(),
        name: "web-01".into(),
        host: "10.0.0.1".into(),
        user: "root".into(),
        auth: "password".into(),
        note: "".into(),
        kind: "ssh".into(),
        online: false,
        picked: false,
        hit: false,
    }]))));
    ui.set_gm_custom_count(1);
    ui.set_gm_session_count(2);
    ui.set_gm_selected("默认组".into());
    ui.set_gm_cur_index(0);
    ui.set_gm_open(true);
    ui.show().unwrap();

    // ① 首帧：中心像素 = 弹框底色（不是全透明 / 不是 scrim）
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        })
    });
    let center = buf[(H / 2) * W + W / 2];
    assert!(center.alpha > 0, "弹框没有渲染出来（中心像素全透明）");
    let bg = match ui.global::<Theme>().get_popup_bg() {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("popup-bg 必须是纯色"),
    };
    assert_eq!(
        (center.red, center.green, center.blue),
        bg,
        "弹框中心不是 popup-bg"
    );

    // ② ↓：切到下一组
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: Key::DownArrow.into() });
    assert_eq!(ui.get_gm_selected().as_str(), "生产", "↓ 没有切到下一组");

    // ③ Esc：快捷键气泡改成「按需展开」后（ui/group_manager.slint 的 keys-open
    //    注释：打开弹框时不再自动弹），第一次 Esc 就是关弹框 —— 没有"先收气泡"
    //    那一层了。旧断言留在这是红测试（2026-10-25 发现并同步）。
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: Key::Escape.into() });
    assert!(!ui.get_gm_open(), "Esc 没有关闭弹框");
}

/// 重名时按「完成」不能关窗（#gm-dup-guard）。
///
/// 旧版 `commit_edit()` 不回报成功与否：`完成` 处无条件 `root.close()`，
/// 重名提交失败后弹框照关，用户回来发现名字没改、也没有任何提示。
/// 现在 `commit_edit()` 返回 bool，关窗/切组/开新建行都先问它。
#[test]
fn duplicate_rename_keeps_dialog_open() {
    use slint::platform::PointerEventButton;
    use slint::{LogicalPosition, SharedString};

    fn click(ui: &AppWindow, x: f32, y: f32) {
        let pos = LogicalPosition::new(x, y);
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: pos });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: pos,
            button: PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: pos,
            button: PointerEventButton::Left,
        });
    }

    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(W as f32, H as f32));

    ui.set_gm_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        group("默认组", 0, true),
        group("1", 3, false),
        group("1方法", 5, false),
    ]))));
    ui.set_gm_members(ModelRc::from(Rc::new(VecModel::from(Vec::<GmMember>::new()))));
    ui.set_gm_selected("1方法".into());
    ui.set_gm_cur_index(2);
    ui.set_gm_open(true);
    ui.show().unwrap();
    // 弹框 760×560 居中于 800×600 → 原点 (20,20)，「完成」在右下角
    let done = (736.0, 556.0);

    // ① 没有待提交的编辑：点「完成」应当关窗（同时校准上面那个坐标真的打在按钮上）
    click(&ui, done.0, done.1);
    assert!(!ui.get_gm_open(), "「完成」没关窗 —— 测试坐标打偏了");

    // ② 重名：F2 → Enter 提交失败 → 点「完成」必须留在弹框里
    ui.on_gm_rename(|_orig, _name| -> SharedString {
        "已存在同名分组（不区分大小写）".into()
    });
    ui.set_gm_open(true);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: Key::F2.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: Key::Return.into() });
    click(&ui, done.0, done.1);
    assert!(ui.get_gm_open(), "重名时按「完成」把弹框关掉了 —— 改名会被静默丢弃");
}

/// 改色弹层（#swatch-popup 2026-10-26 / #swatch-popup-lift）：展开后是**盖住整个
/// 界面**的取色卡（挂在窗口根，不在 760×560 的分组弹框里），与设置页「强调色」
/// 同一套。旧版是锚在色点旁的 195px 小浮层、8×6 无名色格。
///
/// 判据（不锁像素细节）：
///  ① 展开后背景被 scrim 压暗，卡片内衬仍是未压暗的 popup-bg；
///  ② 预设色块真的画出来了（ACCENT_PRESETS 的番茄色成片出现）；
///  ③ **整窗可拖**：把卡片往上拖 200px，卡片能越过分组弹框顶边（不在 gm 的
///     clip 里），同时被窗口边界钳住（top ≥ 8px）。
#[test]
fn color_popup_floats_over_the_window_and_drags_across_it() {
    use mterm::ui::GroupSwatch;

    // 比分组弹框（760×560）大一圈，才看得出「弹层在窗口根、不受 gm 裁剪」。
    const CW: usize = 1200;
    const CH: usize = 820;

    fn grab(ui: &AppWindow, buf: &mut Vec<PremultipliedRgbaColor>, w: usize) {
        ui.window().request_redraw();
        WIN.with(|win| {
            win.draw_if_needed(|r| {
                r.render(buf, w);
            });
        });
    }
    fn rgb(buf: &[PremultipliedRgbaColor], x: usize, y: usize, w: usize) -> (u8, u8, u8) {
        let p = buf[y * w + x];
        if p.alpha == 0 {
            return (0, 0, 0);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue))
    }

    slint::platform::set_platform(Box::new(ProbePlatform)).ok();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(CW as f32, CH as f32));

    // PickerMath 全局的生产接线在 src/app.rs::open_window 里，离屏探针不会跑那段；
    // 不接的话 hex→color 全返回透明，色板井/应用键看起来是空的（本测试要截证据帧）。
    {
        use slint::SharedString;
        let pm = ui.global::<mterm::ui::PickerMath>();
        pm.on_hex_color(|hex: SharedString| match mterm::picker::parse_hex(&hex) {
            Some(v) => slint::Color::from_rgb_u8(
                ((v >> 16) & 0xFF) as u8,
                ((v >> 8) & 0xFF) as u8,
                (v & 0xFF) as u8,
            ),
            None => slint::Color::from_rgb_u8(0, 0, 0),
        });
        pm.on_ink_on(|hex: SharedString| {
            let ink = mterm::picker::ink_on(mterm::picker::parse_hex(&hex).unwrap_or(0));
            slint::Color::from_rgb_u8(
                ((ink >> 16) & 0xFF) as u8,
                ((ink >> 8) & 0xFF) as u8,
                (ink & 0xFF) as u8,
            )
        });
        pm.on_fit_hex(|hex: SharedString| match mterm::picker::parse_hex(&hex) {
            Some(v) => format!("#{v:06X}").into(),
            None => SharedString::default(),
        });
    }

    ui.set_gm_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        group("默认组", 0, true),
        group("生产", 0, false),
    ]))));
    ui.set_gm_members(ModelRc::from(Rc::new(VecModel::from(Vec::<GmMember>::new()))));
    // 色板 = 生产里那份（theme::palettes::ACCENT_PRESETS，48 色），不另造数据。
    let swatches: Vec<GroupSwatch> = mterm::theme::palettes::ACCENT_PRESETS
        .iter()
        .map(|&(_, name, raw)| GroupSwatch {
            hex: format!("#{raw:06X}").into(),
            swatch: slint::Color::from_rgb_u8(
                ((raw >> 16) & 0xFF) as u8,
                ((raw >> 8) & 0xFF) as u8,
                (raw & 0xFF) as u8,
            ),
            name: name.into(),
        })
        .collect();
    ui.set_group_palette(ModelRc::from(Rc::new(VecModel::from(swatches))));
    ui.set_gm_selected("生产".into());
    ui.set_gm_cur_index(1);
    ui.set_gm_open(true);
    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(80));

    let bg = match ui.global::<Theme>().get_popup_bg() {
        slint::Brush::SolidColor(c) => (c.red(), c.green(), c.blue()),
        _ => panic!("popup-bg 必须是纯色"),
    };
    let lum = |c: (u8, u8, u8)| c.0 as u32 + c.1 as u32 + c.2 as u32;

    let mut buf = vec![PremultipliedRgbaColor::default(); CW * CH];
    grab(&ui, &mut buf, CW);
    // 未展开时，窗口左上角那块是应用底色（记下来做「被压暗」的基线）
    let corner = (8, 8);
    let base_corner = rgb(&buf, corner.0, corner.1, CW);

    // 展开改色弹层（模拟点了「生产」组的色点：当前色 = 翡翠绿）
    ui.set_gm_color_hex_for("#29A383".into());
    ui.set_gm_color_for("生产".into());
    std::thread::sleep(std::time::Duration::from_millis(80));
    let mut buf2 = vec![PremultipliedRgbaColor::default(); CW * CH];
    grab(&ui, &mut buf2, CW);

    // ① 整窗被 scrim 压暗（弹层挂在窗口根，不只是盖住分组弹框）
    let dimmed = rgb(&buf2, corner.0, corner.1, CW);
    assert!(
        lum(dimmed) < lum(base_corner),
        "改色弹层没有覆盖整个界面：窗口角落 {dimmed:?} vs 基线 {base_corner:?}"
    );

    // ② 卡片自身：656 宽居中于 1200 → 左缘 x=272，16px 内衬里的 x=280 必然是卡片底
    let card_left = (CW - 656) / 2;
    let pad = rgb(&buf2, card_left + 8, CH / 2, CW);
    assert!(
        pad.0.abs_diff(bg.0) <= 3 && pad.1.abs_diff(bg.1) <= 3 && pad.2.abs_diff(bg.2) <= 3,
        "改色卡片左内衬应为 popup-bg：{pad:?} vs {bg:?}"
    );

    // ③ 预设色块画出来了：番茄色成片出现
    let tomato = (0xF4u8, 0x43u8, 0x33u8);
    let hits = (0..CW * CH)
        .filter(|i| {
            let c = rgb(&buf2, i % CW, i / CW, CW);
            c.0.abs_diff(tomato.0) <= 4
                && c.1.abs_diff(tomato.1) <= 4
                && c.2.abs_diff(tomato.2) <= 4
        })
        .count();
    assert!(hits > 300, "色板里没画出预设色块（番茄色命中 {hits} 像素）");

    // 证据帧（与 tests/cmd_manager_polish.rs 同款：PROBE_DUMP_DIR 下导 PPM）
    if let Ok(dir) = std::env::var("PROBE_DUMP_DIR") {
        let mut data = Vec::with_capacity(CW * CH * 3);
        for y in 0..CH {
            for x in 0..CW {
                let (r, g, b) = rgb(&buf2, x, y, CW);
                data.extend_from_slice(&[r, g, b]);
            }
        }
        let header = format!("P6\n{CW} {CH}\n255\n");
        std::fs::write(
            std::path::Path::new(&dir).join("gm-color-popup.ppm"),
            [header.as_bytes(), &data].concat(),
        )
        .ok();
    }

    // ④ 整窗可拖：按住卡片空白区往上拖 200px，卡片应越过分组弹框顶边
    //    （gm 只占 760×560，顶边在 y=(820-560)/2=130），并被窗口顶边钳住。
    use slint::platform::PointerEventButton;
    use slint::LogicalPosition;
    let start = LogicalPosition::new((card_left + 8) as f32, (CH / 2) as f32);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: start });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    for step in 1..=10 {
        let p = LogicalPosition::new(start.x, start.y - 20.0 * step as f32);
        ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    }
    ui.window()
        .dispatch_event(WindowEvent::PointerReleased {
            position: LogicalPosition::new(start.x, start.y - 200.0),
            button: PointerEventButton::Left,
        });
    std::thread::sleep(std::time::Duration::from_millis(80));
    let mut buf3 = vec![PremultipliedRgbaColor::default(); CW * CH];
    grab(&ui, &mut buf3, CW);

    // 卡片内衬 y=20 处仍是卡片底：说明卡片已越过 gm 顶边（130）且没被裁
    let above = rgb(&buf3, card_left + 8, 20, CW);
    assert!(
        above.0.abs_diff(bg.0) <= 3 && above.1.abs_diff(bg.1) <= 3 && above.2.abs_diff(bg.2) <= 3,
        "卡片拖不到分组弹框之外：y=20 处是 {above:?}（应为卡片底 {bg:?}）"
    );
    // 窗口边界仍钳住（卡片顶边 8px），不会整块拖出可视区
    let edge = rgb(&buf3, card_left + 8, 3, CW);
    assert!(
        edge.0.abs_diff(bg.0) > 3 || edge.1.abs_diff(bg.1) > 3 || edge.2.abs_diff(bg.2) > 3,
        "卡片被拖出了窗口（y=3 仍是卡片底 {edge:?}）"
    );
}
