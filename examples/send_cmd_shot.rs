//! (#send-cmd-p1..p3) 发送命令窗口离屏渲染出图 —— 与高保真稿逐项比对的证据图。
//!
//! 运行:`cargo run --example send_cmd_shot`(example = bin 类链接,绕开
//! test 目标在这台机器上的 dlltool 0xc0000409 环境病)。
//! 输出:output/send-cmd/{01-default,02-panel,03-danger,04-receipt}.png
//!
//! 逻辑链不在本例断言(单测 9/9 已覆盖 send_check;Slint 绑定类型由编译器
//! 保证);这里负责**视觉**:四态整帧渲染,供与沙盘 1:1 核对。

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

const W: usize = 800;
const H: usize = 560;

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

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = SendCmdWindow::new().expect("build send window");
    ui.window().set_size(slint::LogicalSize::new(760.0, 500.0));
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

    let mut frame = vec![PremultipliedRgbaColor::default(); W * H];

    // ── 01 默认态 ──
    grab(&ui, &mut frame, "output/send-cmd/01-default.png");

    // ── 02 面板态:lib-rows 手工造(段头+命令+历史),lib-open=true ──
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
    ui.set_lib_open(true);
    grab(&ui, &mut frame, "output/send-cmd/02-panel.png");
    ui.set_lib_open(false);

    // ── 03 危险+多行:rm -rf 文本 + 检测镜像 + 送达胶囊 ──
    ui.set_text(slint::SharedString::from("rm -rf /var/log/*
systemctl stop nginx"));
    ui.set_danger_token("rm -rf".into());
    ui.set_risk_info("".into());
    ui.set_lib_rows(ModelRc::from(Rc::new(VecModel::from(Vec::new()))));
    ui.set_armed(true);
    grab(&ui, &mut frame, "output/send-cmd/03-danger.png");

    // ── 04 回执态:3 台 1 失败 ──
    ui.set_text("systemctl restart nginx".into());
    ui.set_danger_token("".into());
    let receipt = vec![
        crate_receipt("web-01", true),
        crate_receipt("db-02", true),
        crate_receipt("gw-04", false),
    ];
    ui.set_receipt(ModelRc::from(Rc::new(VecModel::from(receipt))));
    ui.set_receipt_bad(1);
    ui.set_receipt_ok(2);
    ui.set_tip_open(true);
    grab(&ui, &mut frame, "output/send-cmd/04-receipt.png");

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
}
