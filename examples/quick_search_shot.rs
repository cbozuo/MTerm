//! (#double-shift-v1 2026-11-11) 全局搜索浮层离屏出图 —— 与高保真稿
//! docs/design/double-shift-hifi-2026-11-06-v1.html §04 逐项比对。
//!
//! 运行:`cargo run --profile fast --example quick_search_shot`
//! 输出:output/quick-search/results.png(有结果态)/ recent.png(空查询态)
//!
//! 面板数据全部离屏手工 seed(不跑 search_ui::install —— 那需要完整
//! ConfigStore 与 ctx);三个纯回调必须注册,否则脚注渲染即 panic。

use mterm::ui::{AppWindow, QsMeta, QsPreview, QsRow};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
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

const W: usize = 1240;
const H: usize = 800;

fn save_rgba(buf: &[PremultipliedRgbaColor], path: &str) {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(dir);
    }
    // 反预乘 → PNG(照 left_panel_shot 的 PPM→PNG 简化:直接写 PNG 需要
    // image crate,这里沿用项目 PPM 约定更省依赖 —— 但对比用 PNG 直观,
    // 用最小的 PNG 编码器不如直接复用 PPM;不过本仓 examples 有现成 PNG
    // 写法么 —— left_panel_shot 写 PPM。跟随项目:写 PPM(可用任意看图器打开)。
    // 直接写 PNG（自带 zlib store 块，无外部依赖）——看图工具链统一吃 PNG。
    let mut raw: Vec<u8> = Vec::with_capacity(W * H * 3 + H);
    for y in 0..H {
        raw.push(0); // filter: none
        for x in 0..W {
            let p = buf[y * W + x];
            // 反预乘（软件渲染输出是预乘 alpha）
            let (r, g, b) = if p.alpha == 255 {
                (p.red, p.green, p.blue)
            } else if p.alpha == 0 {
                (0, 0, 0)
            } else {
                (
                    ((p.red as u32 * 255) / p.alpha as u32).min(255) as u8,
                    ((p.green as u32 * 255) / p.alpha as u32).min(255) as u8,
                    ((p.blue as u32 * 255) / p.alpha as u32).min(255) as u8,
                )
            };
            raw.push(r);
            raw.push(g);
            raw.push(b);
        }
    }
    let _ = fs::write(path, png_bytes(&raw));
    println!("wrote {path}");
}

/// PNG 容器：IHDR + IDAT(zlib store) + IEND。CRC32 自带实现。
fn png_bytes(raw: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *entry = c;
        }
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
        c ^ 0xFFFF_FFFF
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = Vec::with_capacity(4 + data.len());
        body.extend_from_slice(kind);
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    // zlib stream: header 0x78 0x01 + deflate stored blocks + adler32
    let mut z = vec![0x78u8, 0x01];
    let mut off = 0usize;
    while off < raw.len() {
        let take = (raw.len() - off).min(65535);
        let last = if off + take >= raw.len() { 1u8 } else { 0u8 };
        z.push(last);
        z.extend_from_slice(&(take as u16).to_le_bytes());
        z.extend_from_slice(&(!(take as u16)).to_le_bytes());
        z.extend_from_slice(&raw[off..off + take]);
        off += take;
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in raw {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(W as u32).to_be_bytes());
    ihdr.extend_from_slice(&(H as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8bit RGB
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn grab(ui: &AppWindow, path: &str) {
    // 多轮抓帧(request_redraw 需要事件循环间隙落地;command_hint_ui 同款节奏)。
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    for _ in 0..6 {
        ui.window().request_redraw();
        WIN.with(|win| {
            win.draw_if_needed(|r| {
                r.render(&mut buf, W);
            });
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    save_rgba(&buf, path);
}

fn row(kind: i32, cat: i32, title: &str, sub: &str, icon: &str, badge: &str, open: bool, shortcut: &str, qi: i32) -> QsRow {
    QsRow {
        kind,
        cat,
        title: title.into(),
        sub: sub.into(),
        icon: icon.into(),
        badge: badge.into(),
        open,
        shortcut: shortcut.into(),
        qi,
        lines: 1,
    }
}

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = AppWindow::new().expect("build app window");
    ui.window().set_size(slint::LogicalSize::new(W as f32, H as f32));
    // 主题槽注入:离屏窗口的 Theme global 默认全 0(黑),必须先写关键槽
    // 再抓帧。ThemeState 类型 pub(crate) 外部构造不了,直接手写 mterm-dark
    // 默认槽值(theme.slint:31-48 的声明值)。
    {
        let th = ui.global::<mterm::ui::Theme<'_>>();
        let c = |v: u32| {
            slint::Color::from_rgb_u8(((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8)
        };
        th.set_dark(true);
        th.set_slot_root(c(0x1b1d23));
        th.set_slot_panel(c(0x23262d));
        th.set_slot_palt(c(0x2a2d35));
        th.set_slot_elev(c(0x25282f));
        th.set_slot_hov(c(0x373a44));
        th.set_slot_act(c(0x3f4350));
        th.set_slot_line(c(0x3a3d46));
        th.set_slot_lstr(c(0x4a4e59));
        th.set_slot_t1(c(0xe6e8ee));
        th.set_slot_t2(c(0xb4b9c4));
        th.set_slot_t3(c(0x9196a3));
        th.set_slot_tbg(c(0x0e0f13));
        th.set_slot_tfg(c(0xd4d4d4));
        th.set_slot_ac(c(0x1fc8b4));
        th.set_slot_ok(c(0x4ec9b0));
        th.set_slot_wr(c(0xe2a84a));
        th.set_slot_dg(c(0xe25c5c));
        th.set_accent_hex("#1FC8B4".into());
    }
    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 纯回调必须注册(未注册被调用即 panic;签名与 quick_search.slint 一致)。
    ui.on_empty_title(|| SharedString::from("没有找到 「prod」"));
    ui.on_index_count_text(|| SharedString::from("索引 18 项"));
    ui.on_timing_text(|| SharedString::from("查询 0.6 ms · 打开 8.4 ms"));

    // ── 图 1:有结果态(查询 "prod")────────────────────────────────
    let rows = vec![
        row(0, 0, "会话 · 2", "", "", "", false, "", -1),
        row(1, 1, "web-prod-01", "root@10.24.8.11:22 · 生产环境 · 密钥", "terminal", "会话", false, "", 0),
        row(1, 1, "web-prod-02", "deploy@10.24.8.12:22 · 生产环境 · 密钥", "terminal", "会话", true, "", 1),
        row(0, 0, "命令 · 2", "", "", "", false, "", -1),
        row(1, 2, "查看 Nginx 状态", "systemctl status nginx --no-pager", "quick-command", "命令", false, "", 2),
        row(1, 2, "部署 staging", "cd /opt/app && git pull && ./deploy.sh staging", "quick-command", "命令", false, "", 3),
        row(0, 0, "设置 · 1", "", "", "", false, "", -1),
        row(1, 3, "主题", "外观 › 主题", "settings", "设置", false, "", 4),
        row(0, 0, "动作 · 2", "", "", "", false, "", -1),
        row(1, 4, "切换主题", "全局动作 · 应用内执行", "bolt", "", false, "", 5),
        row(1, 4, "切换左侧栏", "全局动作 · Ctrl+B", "bolt", "", false, "Ctrl+B", 6),
    ];
    ui.set_qs_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_qs_hi(1);
    ui.set_qs_count_all(18);
    ui.set_qs_count_session(9);
    ui.set_qs_count_command(6);
    ui.set_qs_count_setting(2);
    ui.set_qs_count_action(4);
    ui.set_qs_content_px(11 * 49 - 49 + 4 * 24 + 2 * 24);
    ui.set_qs_row_count(7);
    ui.set_qs_query_ms("0.6 ms".into());
    ui.set_qs_open_ms("8.4 ms".into());
    let metas = vec![
        QsMeta { label: "主机".into(), value: "deploy@10.24.8.12:22".into(), mono: true },
        QsMeta { label: "分组".into(), value: "生产环境".into(), mono: false },
        QsMeta { label: "认证".into(), value: "key".into(), mono: false },
        QsMeta { label: "状态".into(), value: "已打开".into(), mono: false },
    ];
    ui.set_qs_preview(QsPreview {
        has_item: true,
        icon: "terminal".into(),
        title: "web-prod-02".into(),
        cat: "会话".into(),
        desc: "SSH · 10.24.8.12".into(),
        metas: ModelRc::from(Rc::new(VecModel::from(metas))),
        code: "".into(),
        primary: "Enter 切换".into(),
        secondary: "Ctrl+Enter 复制 SSH 命令".into(),
    });
    ui.set_quick_search_open(true);
    std::thread::sleep(std::time::Duration::from_millis(300));
    grab(&ui, "output/quick-search/results.png");

    // (#qs-enter-glyph 2026-11-13) 选中「切换主题」行（无徽章无快捷键）——
    // 行尾 corner-down-left 图标（↵ 字符 mono 缺字形渲染成「丨」的替代）+
    // 行图标垂直居中复核。
    ui.set_qs_hi(9);
    grab(&ui, "output/quick-search/enter-glyph.png");
    ui.set_qs_hi(1);
    ui.set_quick_search_open(false);
    std::thread::sleep(std::time::Duration::from_millis(120));

    // ── 图 2:空查询态(最近搜索 + 最近使用)───────────────────────
    let rows = vec![
        row(0, 0, "最近使用", "", "", "", false, "", -1),
        row(1, 1, "web-prod-02", "deploy@10.24.8.12:22 · 生产环境 · 密钥", "terminal", "", true, "", 0),
        row(1, 2, "查看 Nginx 状态", "systemctl status nginx --no-pager", "quick-command", "", false, "", 1),
        row(1, 3, "主题", "外观 › 主题", "settings", "", false, "", 2),
    ];
    ui.set_qs_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_qs_hi(1);
    ui.set_qs_row_count(3);
    ui.set_qs_content_px(3 * 24 + 3 * 49);
    let recents: Vec<SharedString> = vec!["web".into(), "docker".into(), "主题".into(), "日志".into()];
    ui.set_qs_recent_searches(ModelRc::from(Rc::new(VecModel::from(recents))));
    ui.set_quick_search_open(true);
    std::thread::sleep(std::time::Duration::from_millis(300));
    grab(&ui, "output/quick-search/recent.png");

    // ── 图 3:多行命令行高自适应(#qs-multiline-row 2026-11-13)──────
    // 命令历史 title = 命令原文含换行;行高 = 49 + (lines-1)×17。
    let ml_cmd = "sudo tee /etc/profile.d/lsd.sh << 'EOF'
alias ls='lsd'
alias ll='lsd -la'
EOF";
    let ml_rows = vec![
        row(0, 0, "命令 · 2", "", "", "", false, "", -1),
        {
            let mut r = row(1, 2, ml_cmd, "命令历史", "quick-command", "命令", false, "", 0);
            r.lines = 4;
            r
        },
        row(1, 2, "/usr/local/dm7/dmdba/script/root/dm_service_installer.sh -t dm...", "命令历史", "quick-command", "命令", false, "", 1),
    ];
    ui.set_qs_rows(ModelRc::from(Rc::new(VecModel::from(ml_rows))));
    ui.set_qs_hi(0);
    ui.set_qs_content_px(24 + (49 + 3 * 17) + 49);
    ui.set_qs_row_count(2);
    ui.set_quick_search_open(true);
    std::thread::sleep(std::time::Duration::from_millis(250));
    grab(&ui, "output/quick-search/multiline.png");
    ui.set_quick_search_open(false);
    std::thread::sleep(std::time::Duration::from_millis(150));

    // ── 图 3:空结果态(无命中 + 建议 + 推荐 chips)──────────────
    ui.set_qs_search_text("zzzz-not-found".into());
    ui.set_qs_rows(ModelRc::from(Rc::new(VecModel::from(Vec::<QsRow>::new()))));
    ui.set_qs_hi(0);
    ui.set_qs_row_count(0);
    ui.set_qs_content_px(0);
    ui.set_qs_recent_searches(ModelRc::from(Rc::new(VecModel::from(Vec::<SharedString>::new()))));
    ui.set_qs_empty_hints(ModelRc::from(Rc::new(VecModel::from(vec![
        SharedString::from("1  检查拼写，确认没有多余空格"),
        SharedString::from("2  试试更短的关键词，例如「doc」"),
        SharedString::from("3  按 Tab 切换到具体分类再搜"),
    ]))));
    ui.set_qs_empty_chips(ModelRc::from(Rc::new(VecModel::from(vec![
        SharedString::from("web"),
        SharedString::from("docker"),
        SharedString::from("主题"),
        SharedString::from("日志"),
    ]))));
    ui.set_qs_preview(QsPreview::default());
    ui.on_empty_title(|| SharedString::from("没有找到 「zzzz-not-found」"));
    ui.set_quick_search_open(true);
    std::thread::sleep(std::time::Duration::from_millis(300));
    grab(&ui, "output/quick-search/empty.png");
    ui.set_quick_search_open(false);
    std::thread::sleep(std::time::Duration::from_millis(150));

    // ── 图 4:首屏引导条(hint-pill,9 秒后淡出)────────────────────
    ui.set_quick_search_hint_visible(true);
    std::thread::sleep(std::time::Duration::from_millis(300));
    grab(&ui, "output/quick-search/hint-pill.png");

    println!("done.");
}
