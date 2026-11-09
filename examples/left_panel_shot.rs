//! (#gh-icon-center-verify 2026-11-06) 左栏组头行离屏出图 —— 组头三图标
//! (chevron/folder/⋮)垂直居中验证证据。
//!
//! 运行:`cargo run --profile fast --example left_panel_shot`
//! 输出:output/left-panel/group-head.png(280x400,两组头+两成员)

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

    // V3 树形模型 seed:两个根级组头(带统计)+ 各一成员行。
    // 字段最小化填充,其余走 Default(空串/0/false)。
    let mk = |row_kind: &str, name: &str, id: &str, depth: i32, total: i32| SessionInfo {
        row_kind: row_kind.into(),
        name: name.into(),
        id: id.into(),
        depth,
        group_total: total,
        ..Default::default()
    };
    let rows = vec![
        mk("group", "本地终端", "", 0, 2),
        mk("session", "web-01", "s1", 1, 0),
        mk("group", "默认组", "", 0, 1),
        mk("session", "db-01", "s2", 1, 0),
    ];
    ui.set_sessions(ModelRc::from(Rc::new(VecModel::from(rows))));

    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 图 2:右击组别「分组颜色」弹层(#gh-ctx-swatch)——分组管理同款
    // SwatchPopup。锚定色给个固定值,验证 initial/页脚/色格渲染。
    // group-palette 由 Rust 注入(真实运行走 rebuild_group_palette),离屏补注。
    let pal = mterm::theme::by_id(mterm::theme::default_for(true)).expect("palette");
    mterm::theme::rebuild_group_palette(&ui, pal);
    ui.set_ctx_menu_group_color_hex("#ff5500".into());
    ui.set_ctx_menu_group_color(slint::Color::from_argb_u8(0xff, 0xff, 0x55, 0x00));
    ui.set_group_color_picker_open(true);

    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    save_rgba(&buf, "output/left-panel/ctx-swatch.png");

    // 图 3:强调色弹层(#accent-swatch-single-source)——与分组弹层同一
    // model 双消费,验证 48 色网格渲染。accent-stage 给默认青作选定态。
    // accent-hex-color/ink-on 是 Rust 回调,离屏须手动注册(同 app.rs 5788)。
    ui.on_accent_hex_color(|hex| match mterm::picker::parse_hex(&hex) {
        Some(v) => slint::Color::from_rgb_u8(
            ((v >> 16) & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            (v & 0xFF) as u8,
        ),
        None => slint::Color::from_rgb_u8(0, 0, 0),
    });
    ui.on_accent_ink_on(|hex| {
        let c = mterm::picker::parse_hex(&hex).unwrap_or(0);
        let ink = mterm::picker::ink_on(c);
        slint::Color::from_rgb_u8(
            ((ink >> 16) & 0xFF) as u8,
            ((ink >> 8) & 0xFF) as u8,
            (ink & 0xFF) as u8,
        )
    });
    ui.set_interface_open(true);
    ui.set_ifd_page("theme".into());
    ui.set_accent_stage("#1FC8B4".into());
    ui.set_accent_pop_open(true);
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    save_rgba(&buf, "output/left-panel/accent-swatch.png");
    ui.set_accent_pop_open(false);
    ui.set_interface_open(false);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 图 4:高亮设置「添加新规则」表单(#hl-add-row-r2 布局验证)——表单区
    // 静态,不依赖 model;status 错误条给固定文案看贴靠位置。
    ui.set_highlight_dialog_open(true);
    ui.set_hl_tab(1);
    ui.set_highlight_rule_status("请输入正则表达式".into());
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    save_rgba(&buf, "output/left-panel/hl-form.png");
    ui.set_highlight_dialog_open(false);
    ui.set_hl_tab(0);
    ui.set_highlight_rule_status("".into());
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 图 1 复位:关掉弹层再抓组头行(弹层是全屏遮罩,必须后抓)。
    ui.set_group_color_picker_open(false);
    std::thread::sleep(std::time::Duration::from_millis(100));

    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    save_rgba(&buf, "output/left-panel/group-head.png");

    // 图 5:搜索无匹配空态(#empty-compact 2026-11-09)——sessions 清空+给
    // 搜索词,验证轻提示形态(无卡片框/一行小字居顶),不再是全高空框。
    ui.set_sessions(ModelRc::from(Rc::new(VecModel::from(Vec::<SessionInfo>::new()))));
    ui.set_host_search_query("11".into());
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    save_rgba(&buf, "output/left-panel/empty-compact.png");
    ui.set_host_search_query("".into());

    // 图 6:面板 ⋯ 菜单 ctx 浮层(#panel-menu-ctx 2026-11-09)——kind="panel"
    // 验证组菜单同款行规格/锚定(锚点取标题栏右侧附近坐标)。
    ui.set_ctx_open_key("panel".into());
    ui.set_ctx_menu_kind("panel".into());
    ui.set_ctx_menu_x(250.0);
    ui.set_ctx_menu_y(40.0);
    ui.set_ctx_menu_space_below(500.0);
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    save_rgba(&buf, "output/left-panel/panel-menu.png");
    ui.set_ctx_open_key("".into());
    ui.set_ctx_menu_kind("".into());
    println!("saved output/left-panel/ctx-swatch.png + group-head.png + empty-compact.png + panel-menu.png");
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
    fs::create_dir_all("output/left-panel").ok();
    image::save_buffer(path, &pixels, W as u32, H as u32, image::ColorType::Rgba8)
        .expect("save png");
}
