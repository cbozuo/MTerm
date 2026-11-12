//! (#panel-settings-r2 2026-11-12) 面板设置弹框 r2 三改离屏出图 —— 卡高全
//! 展开(修 ScrollView preferred 塌陷)/边框 hairline/组头菜单中文补译 验证证据。
//!
//! 运行:`cargo run --example panel_settings_shot`
//! 输出:output/panel-settings/dialog.png + grp-menu.png

use mterm::ui::{AppWindow, SessionInfo};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::fs;
use std::rc::Rc;

thread_local! {
    static WIN: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}

struct ShotPlatform;
impl Platform for ShotPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(WIN.with(|w| w.clone()))
    }
}

const W: usize = 900;
const H: usize = 640;

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = AppWindow::new().expect("build app window");
    ui.window().set_size(slint::LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // zh 翻译(@tr 走打包 po;组件创建后 select,src/i18n 同款要求)。
    let _ = slint::select_bundled_translation("zh");

    // 离屏须手动接线(paste_confirm_shot 同款):PrimaryButton/完成钮字色
    // = ink-on(accent-hex),缺省不接线 = 文字透明。
    ui.global::<mterm::ui::Theme<'_>>().set_accent_hex("#1FC8B4".into());
    let pm = ui.global::<mterm::ui::PickerMath<'_>>();
    pm.on_hex_color(|hex| match mterm::picker::parse_hex(&hex) {
        Some(v) => slint::Color::from_rgb_u8(
            ((v >> 16) & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            (v & 0xFF) as u8,
        ),
        None => slint::Color::from_rgb_u8(0, 0, 0),
    });
    pm.on_ink_on(|hex| {
        let c = mterm::picker::parse_hex(&hex).unwrap_or(0);
        let ink = mterm::picker::ink_on(c);
        slint::Color::from_rgb_u8(
            ((ink >> 16) & 0xFF) as u8,
            ((ink >> 8) & 0xFF) as u8,
            (ink & 0xFF) as u8,
        )
    });

    // 树最小模型(左栏背景有内容,弹框遮罩对比才真实)。
    let mk = |row_kind: &str, name: &str, id: &str, depth: i32, total: i32| SessionInfo {
        row_kind: row_kind.into(),
        name: name.into(),
        id: id.into(),
        depth,
        group_total: total,
        ..Default::default()
    };
    ui.set_sessions(ModelRc::from(Rc::new(VecModel::from(vec![
        mk("group", "本地终端", "", 0, 2),
        mk("session", "web-01", "s1", 1, 0),
        mk("group", "Test", "", 0, 3),
        // (#gm-subgroup) 子分组树基线:depth1 组头 + depth2 会话。
        mk("group", "子分组A", "", 1, 1),
        mk("session", "db-01", "s2", 2, 0),
    ]))));

    // ── 图 1:面板设置弹框 —— 8 项全展开(修复后卡高 = 54+483+50 = 587 > 640
    // 窗高贴边? 640-36 = 604 > 587,应完整显示、无滚动条)、hairline 外框。
    ui.set_panel_settings_open(true);
    shot(&ui, "output/panel-settings/dialog.png");
    ui.set_panel_settings_open(false);

    // ── 图 2:组头菜单 —— 验证今天补译的中文(在此分组新建会话/新建子分组/
    // 分组颜色…);Test 非保留组 → 九项全出。
    ui.set_ctx_open_key("g1".into());
    ui.set_ctx_menu_kind("group".into());
    ui.set_ctx_menu_group("Test".into());
    ui.set_ctx_menu_x(180.0);
    ui.set_ctx_menu_y(90.0);
    ui.set_ctx_menu_space_below(500.0);
    shot(&ui, "output/panel-settings/grp-menu.png");
    ui.set_ctx_open_key("".into());
    ui.set_ctx_menu_kind("".into());

    // ── 图 3:移动到分组弹框 r2(#move-h)—— 卡高按内容全展开(修 ScrollView
    // + 行壳双塌陷)、hairline 边框;4 组带色点/深度缩进/计数,Test = 当前。
    use mterm::ui::MoveGroupRow;
    let mr = |name: &str, depth: i32, color: &str, has: bool, count: i32, cur: bool| MoveGroupRow {
        name: name.into(),
        depth,
        color: slint::Color::from_rgb_u8(0xE6, 0xDB, 0x70),
        has_color: has,
        count,
        current: cur,
    };
    ui.set_move_group_rows(ModelRc::from(Rc::new(VecModel::from(vec![
        mr("default", 0, "", true, 2, false),
        mr("Test", 0, "", true, 3, true),
        mr("3", 0, "", true, 1, false),
        mr("测试", 0, "", true, 5, false),
        mr("  子分组", 1, "", false, 2, false),
    ]))));
    ui.set_move_dialog_count(1);
    ui.set_move_dialog_open(true);
    shot(&ui, "output/panel-settings/move-dialog.png");
    ui.set_move_dialog_open(false);

    // ── 图 4:左栏树子分组基线(#gm-subgroup)—— depth1 组头/depth2 会话的
    // 缩进与导轨线观感(改动前后对照)。注意左栏在弹框关闭后渲染。
    shot(&ui, "output/panel-settings/tree-sub.png");

    // ── 图 4b:#row-name-overlap 长名成员行悬停态 —— ctx-open-key 钉住
    // act-on=1(⋮ 显+让位 34),验证名称让位是否生效(用户截图长名遮 ⋮)。
    ui.set_sessions(ModelRc::from(Rc::new(VecModel::from(vec![
        mk("group", "测试", "", 0, 2),
        mk("session", "s (copy)但是服务范围服务费长名称测试", "s-long", 1, 0),
    ]))));
    ui.set_ctx_open_key("s-long".into());
    shot(&ui, "output/panel-settings/row-name-overlap.png");
    ui.set_ctx_open_key("".into());

    // ── 图 5:分组管理台子分组支持(#gm-subgroup)—— 组列表 DFS 序 + depth
    // 缩进;选中 Test(根级)→「新建子分组」入口可用。
    use mterm::ui::GroupRow;
    let gr = |name: &str, depth: i32, cnt: i32, reserved: bool| GroupRow {
        name: name.into(),
        color: slint::Color::from_rgb_u8(0xE6, 0xDB, 0x70),
        color_hex: "#E6DB70".into(),
        count: cnt,
        hit: cnt,
        reserved,
        depth,
    };
    ui.set_gm_groups(ModelRc::from(Rc::new(VecModel::from(vec![
        gr("默认组", 0, 2, true),
        gr("Test", 0, 3, false),
        gr("子分组A", 1, 1, false),
        gr("3", 0, 1, false),
        gr("测试", 0, 5, false),
    ]))));
    ui.set_gm_selected("Test".into());
    ui.set_gm_custom_count(4);
    ui.set_gm_session_count(12);
    ui.set_gm_cur_index(1);
    ui.set_gm_open(true);
    shot(&ui, "output/panel-settings/gm-subgroup.png");

    println!("saved output/panel-settings/ dialog + grp-menu + move-dialog + tree-sub + gm-subgroup .png");
}

fn shot(ui: &AppWindow, path: &str) {
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    save_rgba(&buf, path);
}

fn save_rgba(buf: &[PremultipliedRgbaColor], path: &str) {
    let mut pixels: Vec<u8> = Vec::with_capacity(W * H * 4);
    for p in buf.iter() {
        let (r, g, b) = if p.alpha == 0 {
            (0, 0, 0)
        } else {
            let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
            (un(p.red), un(p.green), un(p.blue))
        };
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
        pixels.push(255);
    }
    fs::create_dir_all("output/panel-settings").ok();
    image::save_buffer(path, &pixels, W as u32, H as u32, image::ColorType::Rgba8)
        .expect("save png");
}
