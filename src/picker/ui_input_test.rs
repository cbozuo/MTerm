//! (#picker-input-edit 2026-10-15) 调色盘输入行 + 分量滑杆的 UI 级回归测试。
//!
//! 这是本仓库第一处「真派发事件」的 UI 测试：用 Slint 官方测试后端
//! （`i-slint-backend-testing`）建出真的 `AppWindow`、把 `ColorPickerDialog`
//! 打开（三个可见性属性），再用 `ElementQuery` 取到四个 `TextInput` 的**真实
//! 布局坐标**，在坐标上派发 PointerPressed/Released + KeyPressed，最后读回
//! `accessible-value`（框里的字）与 `accent-pick-hex`（事实源）断言。
//!
//! 为什么非要有这层测试：用户报了两次的「四个输入框第一次点不能编辑、光标落
//! 行首、到不了行尾」，光看代码推不出来 —— 一半是 Slint 命中测试对**右对齐**
//! 文本的偏移映射（文字左侧空白全映射到 0），一半是打字途中 `hex-apply` 钳过
//! 的值回灌抢走用户敲到一半的串。两处都只有在真实事件序列下才现形。
//!
//! ⚠ 运行前提：生成代码里要带元素调试信息，否则 `ElementQuery` 找不到元素
//! （Slint 会打印提示）。此时本测试**跳过**而不是失败，避免污染日常
//! `cargo test`；要真跑断言：
//! ```text
//! set SLINT_EMIT_DEBUG_INFO=1 && cargo test --lib picker_input -- --nocapture
//! ```

use crate::ui::AppWindow;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};

/// 把 PickerMath 全局接到纯颜色数学上（生产接线在 src/app.rs 的 open_window 里，
/// 这里只用到取色盘输入行需要的几个）。不接线的话 `hex-ch` 返回空串，四行输入框
/// 全是空的，测试前提不成立。
fn wire_picker_math(ui: &AppWindow) {
    let pm = ui.global::<crate::ui::PickerMath>();
    pm.on_hex_color(|hex: SharedString| match crate::picker::parse_hex(&hex) {
        Some(v) => slint::Color::from_rgb_u8(
            ((v >> 16) & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            (v & 0xFF) as u8,
        ),
        None => slint::Color::from_rgb_u8(0, 0, 0),
    });
    pm.on_hex_valid(|hex: SharedString| match crate::picker::parse_hex(&hex) {
        Some(v) => format!("#{:06X}", v).into(),
        None => SharedString::default(),
    });
    pm.on_hex_ch(|hex: SharedString, ch: i32, fmt: SharedString| {
        let c = crate::picker::parse_hex(&hex).unwrap_or(0);
        let i = ch.clamp(0, 2) as usize;
        match fmt.as_str() {
            "hsv" | "hsb" => {
                let (h, s, v) = crate::picker::hex_to_hsv(c);
                match i {
                    0 => format!("{:.0}", h.round()),
                    1 => format!("{:.0}", (s * 100.0).round()),
                    _ => format!("{:.0}", (v * 100.0).round()),
                }
            }
            "hsl" => {
                let (h, s, v) = crate::picker::hex_to_hsv(c);
                let (h, s, l) = crate::picker::hsv_to_hsl(h, s, v);
                match i {
                    0 => format!("{:.0}", h.round()),
                    1 => format!("{:.0}", (s * 100.0).round()),
                    _ => format!("{:.0}", (l * 100.0).round()),
                }
            }
            _ => format!("{}", [(c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF][i]),
        }
        .into()
    });
    pm.on_hex_apply(|base: SharedString, ch: i32, val: SharedString, fmt: SharedString| {
        let c = crate::picker::parse_hex(&base).unwrap_or(0);
        let Ok(n) = val.trim().parse::<f32>() else {
            return base;
        };
        let i = ch.clamp(0, 2) as usize;
        let out = match fmt.as_str() {
            "hsv" | "hsb" => {
                let (h, s, v) = crate::picker::hex_to_hsv(c);
                match i {
                    0 => crate::picker::hsv_to_hex(n.rem_euclid(360.0), s, v),
                    1 => crate::picker::hsv_to_hex(h, (n / 100.0).clamp(0.0, 1.0), v),
                    _ => crate::picker::hsv_to_hex(h, s, (n / 100.0).clamp(0.0, 1.0)),
                }
            }
            "hsl" => {
                let (h, s, v) = crate::picker::hex_to_hsv(c);
                let (_, s, l) = crate::picker::hsv_to_hsl(h, s, v);
                let (h, s, l) = match i {
                    0 => (n.rem_euclid(360.0), s, l),
                    1 => (h, (n / 100.0).clamp(0.0, 1.0), l),
                    _ => (h, s, (n / 100.0).clamp(0.0, 1.0)),
                };
                let (h2, s2, v2) = crate::picker::hsl_to_hsv(h, s, l);
                crate::picker::hsv_to_hex(h2, s2, v2)
            }
            _ => {
                let mut rgb = [(c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF];
                rgb[i] = n.clamp(0.0, 255.0) as u32;
                (rgb[0] << 16) | (rgb[1] << 8) | rgb[2]
            }
        };
        format!("#{:06X}", out).into()
    });
    pm.on_fit_hex(|hex: SharedString| hex);
    // (#picker-channel 2026-10-15 方案 B) 分量滑杆比例 → 分量值串（与 src/app.rs
    // 的 on_track_value 同口径：rgb 0-255；H 0-360；S/V/L 0-100）
    pm.on_track_value(|ratio: f32, ch: i32, fmt: SharedString| {
        let i = ch.clamp(0, 2) as usize;
        let max = match fmt.as_str() {
            "rgb" => 255.0,
            _ if i == 0 => 360.0,
            _ => 100.0,
        };
        format!("{:.0}", (ratio.clamp(0.0, 1.0) * max).round()).into()
    });
    pm.on_ink_on(|_hex: SharedString| slint::Color::from_rgb_u8(0, 0, 0));
    pm.on_bright_label(|v: f32| format!("{:.0}", v.clamp(0.0, 1.0) * 255.0).into());
    pm.on_wheel_pick(|dx: f32, dy: f32, v: f32| {
        format!("#{:06X}", crate::picker::wheel_pick(dx, dy, v)).into()
    });
    pm.on_clamp_wheel(|mx: f32, my: f32| crate::ui::WheelPos { mx, my });
}

/// 推进 mock 时钟：0ms 计时器（点入全选）与「两次点击算不算双击」都靠它。
/// 测试后端没有事件循环，不手动推一格计时器永远不触发；
/// system_time 初始化下推时间会撞 animations.rs 的「平台时钟非单调」断言，
/// 所以本测试用 `init_integration_test_with_mock_time`。
fn advance(ms: u64) {
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(ms));
}

/// 点一下并让 0ms 计时器落地（>Slint 双击间隔的间隔另由 `advance` 控制）。
fn click(ui: &AppWindow, pos: LogicalPosition) {
    let w = ui.window();
    w.dispatch_event(WindowEvent::PointerMoved { position: pos });
    w.dispatch_event(WindowEvent::PointerPressed {
        position: pos,
        button: PointerEventButton::Left,
    });
    w.dispatch_event(WindowEvent::PointerReleased {
        position: pos,
        button: PointerEventButton::Left,
    });
    advance(20);
}

fn type_str(ui: &AppWindow, s: &str) {
    let w = ui.window();
    for ch in s.chars() {
        let text: SharedString = ch.to_string().into();
        w.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        w.dispatch_event(WindowEvent::KeyReleased { text });
    }
}

/// 元素左端（右对齐文本左侧的空白区）与右端各一个落点。
fn left_of(ui: &AppWindow, idx: usize) -> LogicalPosition {
    let e = &query_inputs(ui)[idx];
    LogicalPosition::new(
        e.absolute_position().x + 6.0,
        e.absolute_position().y + e.size().height / 2.0,
    )
}

fn right_of(ui: &AppWindow, idx: usize) -> LogicalPosition {
    let e = &query_inputs(ui)[idx];
    LogicalPosition::new(
        e.absolute_position().x + e.size().width - 6.0,
        e.absolute_position().y + e.size().height / 2.0,
    )
}

fn query_inputs(ui: &AppWindow) -> Vec<i_slint_backend_testing::ElementHandle> {
    i_slint_backend_testing::ElementQuery::from_root(ui)
        .match_type_name("TextInput")
        .find_all()
}

#[test]
fn picker_input_click_edit() {
    i_slint_backend_testing::init_integration_test_with_mock_time();
    let ui = AppWindow::new().unwrap();
    wire_picker_math(&ui);
    ui.set_interface_open(true);
    ui.set_ifd_page("theme".into());
    ui.set_accent_pick_hex("#1FC8B4".into());
    ui.set_accent_pick_fmt("hsl".into());
    ui.set_accent_pick_open(true);
    ui.window().set_size(slint::LogicalSize::new(1000.0, 800.0));
    ui.show().unwrap();
    advance(0);

    if query_inputs(&ui).len() < 4 {
        // 生成代码没带元素调试信息 → 元素查询不可用。跳过而非失败：
        // 跑真断言需要 SLINT_EMIT_DEBUG_INFO=1（见文件头）。
        eprintln!(
            "picker_input_click_edit: 跳过 —— 需要 SLINT_EMIT_DEBUG_INFO=1 重新编译才能查到元素"
        );
        return;
    }

    let value = |ui: &AppWindow, i: usize| {
        query_inputs(ui)[i]
            .accessible_value()
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    // ① HEX 行：第一次点击（空白区）+ 输入 → 整段替换（而不是前缀拼接）。
    //    这条就是用户「第一次点打不进字」的那一版：旧实现会得到 "A1FC8B4"-ish。
    //    聚焦期间不回灌，所以框里留的是用户敲的原串（大小写 / 少了 # 都照留），
    //    事实源那边已经是规范化后的 #AABBCC。
    ui.set_accent_pick_hex("#1FC8B4".into());
    click(&ui, left_of(&ui, 0));
    type_str(&ui, "AABBCC");
    assert!(
        value(&ui, 0).ends_with("AABBCC") && value(&ui, 0).len() <= 7,
        "HEX 行首次点入应当整段替换，实得 {:?}",
        value(&ui, 0)
    );
    assert_eq!(ui.get_accent_pick_hex().as_str(), "#AABBCC");

    // ② H 行：换行聚焦（第一次点）→ 也是整段替换：173 → 5（色相 5）。
    //    先把事实源复位（HEX 行此刻还持焦，按设计它留着自己的草稿不跟）。
    ui.set_accent_pick_hex("#1FC8B4".into());
    advance(20);
    click(&ui, left_of(&ui, 1));
    type_str(&ui, "5");
    assert_eq!(value(&ui, 1), "5", "H 行首次点入应当整段替换");
    assert_eq!(ui.get_accent_pick_hex().as_str(), "#C82D1F");

    // ③ 已聚焦状态下**再点一次**（拉开间隔避开双击）→ 只落光标：点右端 → 尾插。
    advance(700);
    click(&ui, right_of(&ui, 1));
    type_str(&ui, "5");
    assert_eq!(value(&ui, 1), "55", "第二次点击应当只落光标（尾插 5+5）");

    // ④ 打字途中不回灌：R 行输入 300，框里留 300，事实源钳到 FF；离焦才收敛。
    //    切分段后 R 行还握着旧草稿，先点开 HEX 行把它顶掉，再重新聚焦 R 行。
    ui.set_accent_pick_fmt("rgb".into());
    ui.set_accent_pick_hex("#1FC8B4".into());
    advance(20);
    click(&ui, left_of(&ui, 0));
    advance(700);
    click(&ui, left_of(&ui, 1));
    type_str(&ui, "300");
    assert_eq!(
        value(&ui, 1),
        "300",
        "聚焦期间输入框不该被钳过的值抢走（实得 {:?}）",
        value(&ui, 1)
    );
    assert_eq!(ui.get_accent_pick_hex().as_str(), "#FFC8B4");
    advance(700);
    click(&ui, left_of(&ui, 0)); // 焦点移走 → 收敛
    assert_eq!(value(&ui, 1), "255", "离焦后应当收敛回事实值");

    // ⑤ 分量滑杆（方案 B）：回到 HSL，把 H 滑杆拖到 75% → H = 270。
    //    滑杆的 TouchArea 无文字内容，按几何认最稳：整行高 36、宽 = 右列 274 −
    //    标签 22 − 两处间距 16 − 数字框 74 = 162（三个形状一致，顺序 = H/S/L）。
    ui.set_accent_pick_fmt("hsl".into());
    ui.set_accent_pick_hex("#1FC8B4".into());
    advance(20);
    let touch_areas = i_slint_backend_testing::ElementQuery::from_root(&ui)
        .match_type_name("TouchArea")
        .find_all();
    if std::env::var("PICKER_TEST_DUMP").is_ok() {
        for e in &touch_areas {
            println!(
                "   TA id={:?} size=({:.1}x{:.1}) pos=({:.1},{:.1})",
                e.id(),
                e.size().width,
                e.size().height,
                e.absolute_position().x,
                e.absolute_position().y
            );
        }
    }
    let tracks: Vec<_> = touch_areas
        .into_iter()
        .filter(|e| {
            let s = e.size();
            (s.height - 36.0).abs() < 1.0 && (120.0..200.0).contains(&s.width)
        })
        .collect();
    assert_eq!(tracks.len(), 3, "三行分量滑杆都该在（实得 {}）", tracks.len());
    let t = &tracks[0];
    let (tp, ts) = (t.absolute_position(), t.size());
    t.mock_drag(
        LogicalPosition::new(tp.x + ts.width * 0.75, tp.y + ts.height / 2.0),
        PointerEventButton::Left,
    );
    advance(20);
    assert_eq!(value(&ui, 1), "270", "H 滑杆拖到 75% 应当写回 270");
    assert_ne!(ui.get_accent_pick_hex().as_str(), "#1FC8B4", "拖动后选色应当变了");
    // 滚轮微调：向下滚一格 = 该分量 −1（H 270 → 269）
    t.scroll(0.0, 1.0);
    advance(20);
    assert_eq!(value(&ui, 1), "269", "H 滑杆向下滚一格应当 −1");

    // ⑥ HEX 行复制键：点它只发 copy-requested —— 正聚焦的输入框不该因此离焦，
    //    草稿也不该被收敛掉（按下被复制键的 TouchArea 独占，输入框收不到 press）。
    ui.set_accent_pick_hex("#1FC8B4".into());
    advance(20);
    click(&ui, left_of(&ui, 0));
    type_str(&ui, "FF0000");
    assert_eq!(ui.get_accent_pick_hex().as_str(), "#FF0000");
    let hex_row_y = query_inputs(&ui)[0].absolute_position().y;
    let copy_btn = i_slint_backend_testing::ElementQuery::from_root(&ui)
        .match_type_name("TouchArea")
        .find_all()
        .into_iter()
        .find(|e| {
            let s = e.size();
            (s.width - 22.0).abs() < 1.0
                && (s.height - 22.0).abs() < 1.0
                // 落在 HEX 行纵向范围内（头部那个同尺寸的 ✕ 在更上面）
                && e.absolute_position().y >= hex_row_y - 1.0
                && e.absolute_position().y <= hex_row_y + 36.0
        })
        .expect("HEX 行的复制键应当存在");
    copy_btn.mock_single_click(PointerEventButton::Left);
    advance(20);
    assert_eq!(value(&ui, 0), "FF0000", "点复制键不该把聚焦中的草稿冲掉");

    // ⑦ 版式回归（方案 B 的承诺：行高不变、总高不变）：HEX→H 之间夹着 36px 的
    //    分段行，所以行距 102；三个分量行两两 36+15 = 51。任一变大都会把色轮
    //    撑高、整卡变长。
    let ys: Vec<f32> = query_inputs(&ui)
        .iter()
        .map(|e| e.absolute_position().y)
        .collect();
    assert!((ys[1] - ys[0] - 102.0).abs() < 0.6, "HEX→H 行距变了: {ys:?}");
    assert!((ys[2] - ys[1] - 51.0).abs() < 0.6, "H→S 行距变了: {ys:?}");
    assert!((ys[3] - ys[2] - 51.0).abs() < 0.6, "S→L 行距变了: {ys:?}");
    // 三个数字框的值右缘仍成一条竖线（HEX 行因为多了复制键而左移，按方案 B 如此）
    let rights: Vec<f32> = query_inputs(&ui)
        .iter()
        .map(|e| e.absolute_position().x + e.size().width)
        .collect();
    for r in &rights[2..] {
        assert!((r - rights[1]).abs() < 0.6, "分量数字框右缘没对齐: {rights:?}");
    }
}
