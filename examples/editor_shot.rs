//! (#editor-native 2026-11-16) SFTP 内置编辑器离屏出图 —— 与高保真稿
//! docs/design/sftp-editor-hifi-2026-11-14.html 逐项比对。
//!
//! 运行:`cargo run --profile fast --example editor_shot`
//! 输出:output/editor-native/scene1..4.png
//!
//! 说明:本 example 手工喂「全文 + 语法区间字符串」以渲染层对照(布局 / 色板 /
//! 浮层)。真实链路里语法区间由 `src/editor`(tree-sitter / 行级规则)产出、
//! 宿主桥编码成 `start:end:AARRGGBB;`,这里用一份迷你 tokenizer 复刻同一线格式,
//! 重点验证:**中英混排不再漂移**(旧的「列 × 单格宽」网格做不到这一点)。

use mterm::ui::{EditorTabInfo, EditorWindow, PickerMath, Theme};
use regex::Regex;
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

const W: usize = 1040;
const H: usize = 760;

fn save_rgba(buf: &[PremultipliedRgbaColor], path: &str) {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let mut raw: Vec<u8> = Vec::with_capacity(W * H * 3 + H);
    for y in 0..H {
        raw.push(0);
        for x in 0..W {
            let p = buf[y * W + x];
            let (r, g, b) = if p.alpha == 255 {
                (p.red, p.green, p.blue)
            } else if p.alpha == 0 {
                (0, 0, 0)
            } else {
                let a = p.alpha as u32;
                (
                    ((p.red as u32 * 255) / a).min(255) as u8,
                    ((p.green as u32 * 255) / a).min(255) as u8,
                    ((p.blue as u32 * 255) / a).min(255) as u8,
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

/// 最小 PNG 编码(store-only deflate + adler32 + crc32),无第三方依赖。
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
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

// ══ 迷你 tokenizer:复刻宿主桥的线格式 `start:end:AARRGGBB;`(字节区间) ══
fn highlight(text: &str, lang: &str) -> String {
    let comments = Regex::new(r"#[^\n]*|//[^\n]*").unwrap();
    let strings = Regex::new(r#""(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'"#).unwrap();
    let numbers = Regex::new(r"\b\d+(?:\.\d+)?\b").unwrap();
    let kw = Regex::new(
        r"\b(?:if|then|else|elif|fi|for|while|do|done|case|esac|function|local|export|return|public|private|static|final|void|class|new|throws|try|catch|for|int|long|String|import|package|true|false|null)\b",
    )
    .unwrap();
    let mut spans: Vec<(usize, usize, &str)> = Vec::new();
    let push = |re: &Regex, color: &'static str, spans: &mut Vec<(usize, usize, &'static str)>| {
        for m in re.find_iter(text) {
            spans.push((m.start(), m.end(), color));
        }
    };
    push(&comments, "ff6a9955", &mut spans);
    push(&strings, "ffce9178", &mut spans);
    push(&numbers, "ffb5cea8", &mut spans);
    push(&kw, "ffc586c0", &mut spans);
    let _ = lang;
    // 有序、互不重叠:按起点排序,重复起点保留最长匹配,丢弃与已产出区间重叠者
    spans.sort_by_key(|(s, e, _)| (*s, std::cmp::Reverse(*e)));
    let mut out = String::new();
    let mut prev_end = 0usize;
    for (s, e, c) in spans {
        if s < prev_end || e <= s || !text.is_char_boundary(s) || !text.is_char_boundary(e) {
            continue;
        }
        prev_end = e;
        out.push_str(&format!("{s}:{e}:{c};"));
    }
    out
}

fn tab(name: &str, lang: &str, letter: &str, active: bool, dirty: bool) -> EditorTabInfo {
    EditorTabInfo {
        id: name.into(),
        name: name.into(),
        dirty,
        active,
        lang: lang.into(),
        letter: letter.into(),
        icon_color: slint::Color::default(),
    }
}

fn grab(ui: &EditorWindow, path: &str) {
    WIN.with(|w| {
        w.set_size(slint::PhysicalSize::new(W as u32, H as u32));
    });
    ui.show().unwrap();
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    WIN.with(|w| {
        w.draw_if_needed(|renderer| {
            renderer.render(&mut buf, W);
        });
    });
    save_rgba(&buf, path);
}

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).unwrap();
    let ui = EditorWindow::new().unwrap();
    ui.set_custom_titlebar(true);
    ui.set_dark_mode(true);
    {
        // 与 app 的深色主题同一套槽位(高保真稿 docs/design/sftp-editor-hifi-2026-11-14.html)
        let th = ui.global::<Theme>();
        let c = |v: u32| slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8);
        th.set_ui_scale(1.0);
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
        th.set_slot_ac(c(0x1fc8b4));
        th.set_accent_hex("#1FC8B4".into());
        th.set_font_mono("Consolas".into());
        th.set_ui_font_family("Segoe UI".into());
    }
    // PickerMath 是按窗口实例化的全局,纯回调必须在 Rust 侧接线:不接则 ink-on()
    // 返回透明色 → 标题栏「保存」与查找「替换全部」的字看不见(#editor-ink-on)。
    {
        let pk = ui.global::<PickerMath>();
        pk.on_ink_on(|hex: SharedString| {
            // 与 app 同一套实现(crate::picker::ink_on):保证离屏图字色与真机一致
            let ink = mterm::picker::ink_on(mterm::picker::parse_hex(&hex).unwrap_or(0));
            slint::Color::from_rgb_u8(
                ((ink >> 16) & 0xFF) as u8,
                ((ink >> 8) & 0xFF) as u8,
                (ink & 0xFF) as u8,
            )
        });
    }

    // ── 场景① Java 源码(中文注释:验证中英混排对齐)──
    {
        let src = concat!(
            "package com.mterm.transfer.sftp;\n",
            "\n",
            "import java.io.IOException;\n",
            "import java.nio.file.Path;\n",
            "\n",
            "/** SFTP 传输服务:上传 / 下载 / 断点续传。 */\n",
            "@Service\n",
            "public class SftpTransferService implements TransferService {\n",
            "    private static final int MAX_RETRIES = 3;\n",
            "\n",
            "    // 同步上传(带断点续传),返回远端字节数\n",
            "    public long syncUpload(String remotePath, Path localFile) throws IOException {\n",
            "        for (int attempt = 1; attempt <= MAX_RETRIES; attempt++) {\n",
            "            try (SftpClient sftp = sessionPool.acquire()) {\n",
            "                log.info(\"上传完成:{} ({} 字节)\", remotePath, bytes);\n",
            "            } catch (IOException e) {\n",
            "                if (attempt == MAX_RETRIES) { throw e; }\n",
            "            }\n",
            "        }\n",
            "    }\n",
            "}\n"
        );
        ui.set_editor_tabs(ModelRc::new(VecModel::from(vec![
            tab("SftpService.java", "Java", "J", true, true),
            tab("app.log", "日志", "L", false, false),
            tab("nginx.conf", "nginx.conf", "N", false, false),
        ])));
        ui.set_editor_content(src.into());
        ui.set_editor_syntax_source(src.into());
        ui.set_editor_syntax_spans(highlight(src, "java").into());
        ui.set_editor_name("SftpService.java".into());
        ui.set_editor_path("/var/www/app/src/main/java/com/mterm/transfer/sftp/SftpService.java".into());
        ui.set_editor_badge("修改".into());
        ui.set_editor_badge_kind("dirty".into());
        ui.set_editor_lang_display("Java".into());
        ui.set_editor_readonly(false);
        ui.set_editor_gutter_w(56);
        ui.set_editor_total_lines(src.lines().count() as i32);
        ui.set_editor_cursor_line(12);
        ui.set_editor_cursor_col(41);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-native/scene1.png");
    }

    // ── 场景② Shell 脚本(中文注释 + 关键字 + 数字)──
    {
        let src = concat!(
            "#!/usr/bin/env bash\n",
            "# 部署脚本:检查依赖后同步到目标机\n",
            "set -euo pipefail\n",
            "\n",
            "TARGET=${1:-/opt/app}\n",
            "RETRY=3\n",
            "\n",
            "if [ ! -d \"$TARGET\" ]; then\n",
            "  echo \"目标目录不存在:$TARGET\" >&2\n",
            "  exit 1\n",
            "fi\n",
            "\n",
            "for i in $(seq 1 $RETRY); do\n",
            "  echo \"第 $i 次同步…\"\n",
            "  rsync -az --delete ./dist/ \"$TARGET/\" && break\n",
            "  sleep 2\n",
            "done\n"
        );
        ui.set_editor_tabs(ModelRc::new(VecModel::from(vec![
            tab("auto_deploy.sh", "Shell", "A", true, false),
        ])));
        ui.set_editor_content(src.into());
        ui.set_editor_syntax_source(src.into());
        ui.set_editor_syntax_spans(highlight(src, "sh").into());
        ui.set_editor_name("auto_deploy.sh".into());
        ui.set_editor_path("/home/webc/java/zhenjiang/auto_deploy.sh".into());
        ui.set_editor_badge("".into());
        ui.set_editor_badge_kind("none".into());
        ui.set_editor_lang_display("Shell".into());
        ui.set_editor_gutter_w(56);
        ui.set_editor_total_lines(src.lines().count() as i32);
        ui.set_editor_cursor_line(15);
        ui.set_editor_cursor_col(3);
        grab(&ui, "output/editor-native/scene2.png");
    }

    // ── 场景③ 日志(行级规则着色)+ 查找浮层 ──
    {
        let src = concat!(
            "2026-11-16 09:12:03.441 [main] INFO  Application - 启动完成,监听 8080\n",
            "2026-11-16 09:12:04.902 [http-nio-1] WARN  Pool - 连接池使用率 87%\n",
            "2026-11-16 09:12:05.118 [http-nio-3] ERROR Sync - 上传失败:self 证书过期\n",
            "java.io.IOException: 目标主机不可达\n",
            "    at com.mterm.Sftp.upload(Sftp.java:88)\n",
            "    at com.mterm.Task.run(Task.java:41)\n",
            "Caused by: java.net.ConnectException: Connection refused\n",
            "2026-11-16 09:12:05.771 [http-nio-3] INFO  Sync - 重试 1/3 ...\n"
        );
        ui.set_editor_tabs(ModelRc::new(VecModel::from(vec![
            tab("app.log", "日志", "L", true, false),
        ])));
        ui.set_editor_content(src.into());
        ui.set_editor_syntax_source(src.into());
        ui.set_editor_syntax_spans(highlight(src, "log").into());
        ui.set_editor_name("app.log".into());
        ui.set_editor_path("/opt/app/logs/app.log".into());
        ui.set_editor_lang_display("日志".into());
        ui.set_editor_total_lines(src.lines().count() as i32);
        ui.set_editor_find_open(true);
        ui.set_editor_find_query("上传".into());
        ui.set_editor_replace_text("同步".into());
        ui.set_editor_replace_open(true);
        ui.set_editor_find_count(2);
        ui.set_editor_find_cur(1);
        ui.set_editor_find_hit_lines(ModelRc::new(VecModel::from(vec![2])));
        grab(&ui, "output/editor-native/scene3.png");
    }

    // ── 场景④ 只读大文件提示条 ──
    {
        let src = (0..40)
            .map(|i| format!("{i:04} 大文件只读预览行 · 中文内容用于验证字形对齐\n"))
            .collect::<String>();
        ui.set_editor_tabs(ModelRc::new(VecModel::from(vec![
            tab("dump.bin.txt", "纯文本", "D", true, false),
        ])));
        ui.set_editor_content(src.as_str().into());
        ui.set_editor_syntax_source(src.as_str().into());
        ui.set_editor_syntax_spans("".into());
        ui.set_editor_name("dump.bin.txt".into());
        ui.set_editor_path("/var/log/dump.bin.txt".into());
        ui.set_editor_lang_display("纯文本".into());
        ui.set_editor_readonly(true);
        ui.set_editor_banner_visible(true);
        ui.set_editor_banner_text("大文件 · 只读模式(86.2 MB)".into());
        ui.set_editor_total_lines(40);
        ui.set_editor_find_open(false);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-native/scene4.png");
    }

    // ── 场景⑤ 超长行(no-wrap:长行不折行,底部出现横向滚动条)──
    {
        let src = concat!(
            "#!/usr/bin/env bash\n",
            "# 长命令行:验证 no-wrap 不折行 + 横向滚动条出现\n",
            "docker run --rm -it --name app-deploy-prod -v /data/app/config:/app/config:ro -v /data/app/logs:/app/logs -e TZ=Asia/Shanghai -e SPRING_PROFILES_ACTIVE=prod -p 8080:8080 registry.internal/app:2026.11.16-hotfix\n",
            "echo done\n"
        );
        ui.set_editor_tabs(ModelRc::new(VecModel::from(vec![
            tab("long_line.sh", "Shell", "L", true, false),
        ])));
        ui.set_editor_content(src.into());
        ui.set_editor_syntax_source(src.into());
        ui.set_editor_syntax_spans(highlight(src, "sh").into());
        ui.set_editor_name("long_line.sh".into());
        ui.set_editor_path("/opt/app/long_line.sh".into());
        ui.set_editor_lang_display("Shell".into());
        ui.set_editor_readonly(false);
        ui.set_editor_banner_visible(false);
        ui.set_editor_gutter_w(56);
        ui.set_editor_total_lines(4);
        ui.set_editor_find_open(false);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-native/scene5.png");
    }

    println!("editor_shot 全部完成 → output/editor-native/");
}
