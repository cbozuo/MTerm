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

    // ③ Esc 逐级取消：首次打开会自动展开快捷键气泡，所以第一次 Esc 只收气泡
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: Key::Escape.into() });
    assert!(ui.get_gm_open(), "第一次 Esc 应该只收起快捷键气泡，不关弹框");
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
