//! (#accent-seg-ghost 2026-10-15) 强调色两态分段的「幽灵选中」回归。
//!
//! 用户反馈：「强调色 没有设置自定义的时候，关闭设置弹框，再打开要默认的选项
//! 要是 跟随主题，而不是滑块在自定义上面」。
//!
//! 根因：点「自定义」分段时会**预置** `accent-mode = "custom"`（AppWindow 的
//! in-out 属性），而真正的落盘只在强调色弹框的「应用」里。没点应用就关掉设置卡，
//! 没有任何路径把 mode 还原 → 重开设置卡分段仍停在「自定义」。
//!
//! 修法：分段选中态改为「弹框开着 **或** 已应用自定义」；「自定义」点击只开
//! 弹框不碰 mode，「应用」才把 mode 落成 custom。
//!
//! 本测试用 Slint 测试后端 + ElementQuery 真派发点击（与
//! `src/picker/ui_input_test.rs` 同模式），断言 state 而不是像素：分段当前
//! 选中项在 Slint 侧没有可读属性，但 `accent-mode` / `accent-pop-open` 是它的
//! 全部输入，钉住这两个值即钉住显示。
//!
//! ⚠ 与 `tests/dialog_scrim_drag.rs` 分开文件：那个装自定义离屏平台拿像素，
//! 本文件用官方测试后端，`set_platform` 一进程一次，必须各自独立进程。
//! ⚠ 运行需要生成代码带元素调试信息，否则 ElementQuery 找不到元素，本测试
//! **跳过**而非失败（同 picker 测试的口径）：
//! ```text
//! set SLINT_EMIT_DEBUG_INFO=1 && cargo test --test accent_seg_state -- --nocapture
//! ```

use mterm::ui::AppWindow;
use slint::ComponentHandle;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{LogicalPosition, LogicalSize, SharedString};

fn advance(ms: u64) {
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(ms));
}

/// 点窗口左上角空白 = 点强调色弹框的全屏遮罩（「点空白 = 还原并关闭」的
/// 真实路径）。不走 Esc：Esc 要焦点恰好在设置卡的 esc-scope 上，测试后端的
/// 焦点保持与真实后端不同，与本次修复无关。
fn click_backdrop(ui: &AppWindow, x: f32, y: f32) {
    let p = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: p,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: p,
        button: PointerEventButton::Left,
    });
}

/// 找可见文本元素（按 accessible label，即文字本身）。
fn find_text(ui: &AppWindow, text: &str) -> Option<i_slint_backend_testing::ElementHandle> {
    i_slint_backend_testing::ElementQuery::from_root(ui)
        .match_type_name("Text")
        .match_predicate({
            let text: SharedString = text.into();
            move |e| e.accessible_label().as_ref() == Some(&text)
        })
        .find_first()
}

/// 按标签现找分段。设置卡是条件元素：关闭即销毁，旧 ElementHandle 会失效，
/// 所以每次要用都重新查。
fn find_seg(
    ui: &AppWindow,
    label: &str,
) -> Option<i_slint_backend_testing::ElementHandle> {
    i_slint_backend_testing::ElementQuery::from_root(ui)
        .match_type_name("AccentSeg")
        .find_all()
        .into_iter()
        .find(|e| seg_label(e).as_deref() == Some(label))
}

/// AccentSeg 的段标签（取自其唯一 Text 子元素）。
fn seg_label(e: &i_slint_backend_testing::ElementHandle) -> Option<String> {
    e.query_descendants()
        .match_type_name("Text")
        .find_first()
        .and_then(|t| t.accessible_label())
        .map(|s| s.to_string())
}

#[test]
fn accent_segment_never_shows_custom_without_an_apply() {
    i_slint_backend_testing::init_integration_test_with_mock_time();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(1280.0, 900.0));
    ui.set_interface_open(true);
    ui.set_ifd_page("theme".into());
    ui.show().unwrap();
    advance(0);

    let segs = i_slint_backend_testing::ElementQuery::from_root(&ui)
        .match_type_name("AccentSeg")
        .find_all();
    if segs.len() < 2 {
        eprintln!(
            "accent_segment_never_shows_custom_without_an_apply: 跳过 —— \
             需要 SLINT_EMIT_DEBUG_INFO=1 重新编译才能查到元素"
        );
        return;
    }
    let custom = segs
        .iter()
        .find(|e| seg_label(e).as_deref() == Some("自定义"))
        .expect("找不到「自定义」分段");

    // ① 从没设置过自定义：mode 就是 follow（分段因此显示「跟随主题」）。
    assert_eq!(ui.get_accent_mode().as_str(), "follow");

    // ② 点「自定义」：弹框打开，但 mode 不许被预置（修复点）。
    custom.mock_single_click(PointerEventButton::Left);
    advance(30);
    assert!(ui.get_accent_pop_open(), "点「自定义」应打开强调色弹框");
    assert_eq!(
        ui.get_accent_mode().as_str(),
        "follow",
        "点「自定义」只是开弹框，不能预置 accent-mode"
    );

    // ③ 用户的实际路径：点遮罩关弹框 → 关设置卡 → 重开，仍应是「跟随主题」。
    click_backdrop(&ui, 10.0, 10.0);
    advance(30);
    assert!(!ui.get_accent_pop_open(), "点遮罩应关掉强调色弹框");
    ui.set_interface_open(false);
    advance(30);
    ui.set_interface_open(true);
    advance(30);
    assert_eq!(
        ui.get_accent_mode().as_str(),
        "follow",
        "没应用过自定义，重开设置卡必须还是「跟随主题」"
    );

    // ④ 反证（防止修成「一律弹回跟随主题」）：真点了「应用」→ mode 落 custom，
    //    且关掉重开仍然停在「自定义」。
    find_seg(&ui, "自定义")
        .expect("重开后应还能找到「自定义」分段")
        .mock_single_click(PointerEventButton::Left);
    advance(30);
    assert!(ui.get_accent_pop_open());
    let apply = find_text(&ui, "应用").expect("弹框里应能找到「应用」按钮");
    apply.mock_single_click(PointerEventButton::Left);
    advance(30);
    assert_eq!(
        ui.get_accent_mode().as_str(),
        "custom",
        "点「应用」必须把 accent-mode 落成 custom"
    );
    assert!(!ui.get_accent_pop_open(), "应用后弹框应关闭");
    ui.set_interface_open(false);
    advance(30);
    ui.set_interface_open(true);
    advance(30);
    assert_eq!(
        ui.get_accent_mode().as_str(),
        "custom",
        "应用过自定义后，重开设置卡应停在「自定义」"
    );

    // ⑤ 从自定义切回「跟随主题」：立即落盘并关弹框。
    find_seg(&ui, "跟随主题")
        .expect("应能找到「跟随主题」分段")
        .mock_single_click(PointerEventButton::Left);
    advance(30);
    assert_eq!(ui.get_accent_mode().as_str(), "follow");
    assert!(!ui.get_accent_pop_open());
}
