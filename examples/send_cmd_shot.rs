//! (#send-cmd-v2 2026-11-07) 发送命令窗口离屏渲染出图 —— 与高保真稿逐项比对的证据图。
//!
//! 运行:`cargo run --example send_cmd_shot`(example = bin 类链接,绕开
//! test 目标在这台机器上的 dlltool 0xc0000409 环境病)。
//! 输出:output/send-cmd/{01-default,02-lib,03-danger,04-receipt}.png
//!
//! v2 四态:默认(右栏 3 台) / 命令库(锚定标题栏钮下) / 危险多行(armed +
//! 危险行红条 + 送达胶囊) / 回执(逐台行尾状态 + rail-foot 重试/复制)。
//! 逻辑链不在本例断言(单测覆盖 send_check;Slint 绑定类型由编译器保证);
//! 这里负责**视觉**:整帧渲染,供与沙盘 1:1 核对。

use mterm::ui::{SendCmdWindow, Theme};
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::fs;

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

const W: usize = 940;
const H: usize = 600;

fn grab(ui: &SendCmdWindow, buf: &mut Vec<PremultipliedRgbaColor>, path: &str) {
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(buf, W);
        });
    });
    // 预乘 → RGBA PNG
    let mut pixels: Vec<u8> = Vec::with_capacity(W * H * 4);
    for p in buf.iter() {
        let (r, g, b) = if p.alpha == 0 { (0, 0, 0) } else {
            let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
            (un(p.red), un(p.green), un(p.blue))
        };
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
        pixels.push(p.alpha);
    }
    let img = image::RgbaImage::from_raw(W as u32, H as u32, pixels).unwrap();
    fs::create_dir_all("output/send-cmd").ok();
    img.save(path).expect("save png");
    println!("saved {path}");
}

/// 主题槽位 + 右栏 mock 数据(频道 A · 3 台) —— 两个窗口实例共用。
fn setup(ui: &SendCmdWindow) {
    ui.window().set_size(slint::LogicalSize::new(940.0, 600.0));
    ui.set_dark_mode(true);
    ui.set_custom_titlebar(true);
    ui.set_win_rounded(true);
    let theme = ui.global::<Theme<'_>>();
    // 深色主题槽位填默认值(真值在 palettes.rs,离屏只需可读即可)
    theme.set_slot_root(slint::Color::from_rgb_u8(0x16, 0x16, 0x19));
    theme.set_slot_panel(slint::Color::from_rgb_u8(0x18, 0x1A, 0x1D));
    theme.set_slot_palt(slint::Color::from_rgb_u8(0x1B, 0x1D, 0x21));
    theme.set_slot_line(slint::Color::from_rgb_u8(0x23, 0x24, 0x28));
    theme.set_slot_lstr(slint::Color::from_rgb_u8(0x4E, 0x51, 0x57));
    theme.set_slot_t1(slint::Color::from_rgb_u8(0xBC, 0xBE, 0xC4));
    theme.set_slot_t2(slint::Color::from_rgb_u8(0x7A, 0x7E, 0x85));
    theme.set_slot_t3(slint::Color::from_rgb_u8(0x6F, 0x73, 0x7A));
    theme.set_slot_ac(slint::Color::from_rgb_u8(0x1F, 0xC8, 0xB4));
    theme.set_slot_ok(slint::Color::from_rgb_u8(0x73, 0xB0, 0x0A));
    theme.set_slot_wr(slint::Color::from_rgb_u8(0xE8, 0xA3, 0x3E));
    theme.set_slot_dg(slint::Color::from_rgb_u8(0xF7, 0x54, 0x64));

    // 频道 A(3 台)—— 右栏默认「频道 A」档
    let scopes = vec![crate_scope("A", slint::Color::from_rgb_u8(0x1F, 0xC8, 0xB4), 3)];
    ui.set_scopes(ModelRc::from(Rc::new(VecModel::from(scopes))));
    ui.set_ch_a(true);
    let targets = vec![
        crate_target("web-01", "root@10.12.4.21", "A", true, true, 0),
        crate_target("web-02", "root@10.12.4.22", "A", true, false, 0),
        crate_target("db-01", "dba@10.12.4.31", "A", true, false, 0),
    ];
    ui.set_targets(ModelRc::from(Rc::new(VecModel::from(targets))));
    ui.set_tab_count(3);
}

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = SendCmdWindow::new().expect("build send window");
    setup(&ui);

    let mut frame = vec![PremultipliedRgbaColor::default(); W * H];

    // ── 01 默认态:频道 A · 3 台,空草稿 ──
    grab(&ui, &mut frame, "output/send-cmd/01-default.png");

    // ── 02 命令库:锚定标题栏钮下,lib-rows 手工造(段头+命令+历史) ──
    let rows = vec![
        crate_row(0, "最近", ""),
        crate_row(2, "journalctl -u nginx -n 200 --no-pager", ""),
        crate_row(2, "mysqldump --single-transaction app", ""),
        crate_row(0, "快捷命令", ""),
        crate_row(1, "容器列表", "docker ps -a"),
        crate_row(1, "磁盘占用", "df -h"),
        crate_row(1, "重启 nginx", "systemctl restart nginx"),
    ];
    ui.set_lib_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_lib_hi(1);   // 第一个可选行(段头 0 之后) —— Rust refresh-lib 同口径
    ui.set_lib_open(true);
    grab(&ui, &mut frame, "output/send-cmd/02-lib.png");
    ui.set_lib_open(false);

    // ── 03 危险+多行:rm -rf 文本 + 检测镜像 + 送达胶囊 + 危险行红条 ──
    ui.set_text(slint::SharedString::from("rm -rf /var/log/*\nsystemctl stop nginx"));
    ui.set_char_count(33);
    let danger_rows: Vec<i32> = vec![0];
    ui.set_danger_rows(ModelRc::from(Rc::new(VecModel::from(danger_rows))));
    ui.set_danger_token("rm -rf".into());
    ui.set_risk_info("".into());
    ui.set_lib_rows(ModelRc::from(Rc::new(VecModel::from(Vec::new()))));
    ui.set_armed(true);
    grab(&ui, &mut frame, "output/send-cmd/03-danger.png");

    // ── 04 回执态:3 台 1 失败(逐台行尾 + rail-foot 重试/复制) ──
    ui.set_text("systemctl restart nginx".into());
    ui.set_char_count(23);
    ui.set_danger_rows(ModelRc::from(Rc::new(VecModel::from(Vec::<i32>::new()))));
    ui.set_danger_token("".into());
    ui.set_armed(false);
    let targets2 = vec![
        crate_target("web-01", "root@10.12.4.21", "A", true, true, 1),
        crate_target("web-02", "root@10.12.4.22", "A", true, false, 1),
        crate_target("db-01", "dba@10.12.4.31", "A", true, false, 2),
    ];
    ui.set_targets(ModelRc::from(Rc::new(VecModel::from(targets2))));
    let receipt = vec![
        crate_receipt("web-01", true),
        crate_receipt("web-02", true),
        crate_receipt("db-01", false),
    ];
    ui.set_receipt(ModelRc::from(Rc::new(VecModel::from(receipt))));
    ui.set_receipt_bad(1);
    ui.set_receipt_ok(2);
    grab(&ui, &mut frame, "output/send-cmd/04-receipt.png");

    // ── 05 收起态:投递栏 44px 竖条 ──
    // 离屏属性动画时钟不走(advance 的 dispatch 对 width 动画无效),复用实例
    // 会冻结在 280 —— 新实例 + rail-anim=false(0ms)拿稳态帧;实机 180ms 动画。
    let ui2 = SendCmdWindow::new().expect("build send window 2");
    setup(&ui2);
    ui2.set_rail_anim(false);
    ui2.set_rail_collapsed(true);
    grab(&ui2, &mut frame, "output/send-cmd/05-collapsed.png");

    // ── 06 范围弹层:主机段(搜索+全选在线+逐台勾选) ──
    // 独立实例:两个组件共享同一离屏 window,后 set 的组件会抢占渲染树。
    let ui3 = SendCmdWindow::new().expect("build send window 3");
    setup(&ui3);
    ui3.set_scope_open(true);
    ui3.set_scope_seg(2);
    ui3.set_host_sel(slint::SharedString::from("id-web-02,"));
    grab(&ui3, &mut frame, "output/send-cmd/06-scope.png");

    // ── 08 范围弹层频道段:项目同款 ScopeRow(复选+色点+右对齐台数) ──
    ui3.set_scope_seg(1);
    ui3.set_host_sel("".into());
    grab(&ui3, &mut frame, "output/send-cmd/08-channel.png");
    // ── 07 光标实证:underline 模式聚焦编辑器,看内置+自绘是否双光标 ──
    let ui4 = SendCmdWindow::new().expect("build send window 4");
    setup(&ui4);
    ui4.set_cursor_style(slint::SharedString::from("underline"));
    ui4.set_text("cd /home".into());
    ui4.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position: slint::LogicalPosition::new(300.0, 160.0),
        button: slint::platform::PointerEventButton::Left,
    });
    ui4.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: slint::LogicalPosition::new(300.0, 160.0),
        button: slint::platform::PointerEventButton::Left,
    });
    grab(&ui4, &mut frame, "output/send-cmd/07-caret.png");
}

fn crate_row(kind: i32, text: &str, sub: &str) -> mterm::ui::LibRow {
    mterm::ui::LibRow {
        kind,
        text: SharedString::from(text.to_string()),
        sub: SharedString::from(sub.to_string()),
    }
}
fn crate_receipt(id: &str, ok: bool) -> mterm::ui::SendReceiptItem {
    mterm::ui::SendReceiptItem { id: SharedString::from(id.to_string()), ok }
}
fn crate_scope(letter: &str, color: slint::Color, count: i32) -> mterm::ui::SendScope {
    mterm::ui::SendScope {
        letter: SharedString::from(letter.to_string()),
        color,
        count,
    }
}
fn crate_target(
    title: &str,
    user: &str,
    ch: &str,
    online: bool,
    current: bool,
    receipt: i32,
) -> mterm::ui::SendTarget {
    mterm::ui::SendTarget {
        id: SharedString::from(format!("id-{title}")),
        title: SharedString::from(title.to_string()),
        user: SharedString::from(user.to_string()),
        ch: SharedString::from(ch.to_string()),
        color: slint::Color::from_rgb_u8(0x1F, 0xC8, 0xB4),
        online,
        current,
        receipt,
    }
}