//! (#editor-hifi 2026-11-15) SFTP 文本编辑器离屏出图 —— 与高保真稿
//! docs/design/sftp-editor-hifi-2026-11-14.html 六场景逐项比对。
//!
//! 运行:`cargo run --profile fast --example editor_shot`
//! 输出:output/editor-hifi/scene1..6.png
//!
//! 说明:渲染层对照(布局/色板/浮层);高亮引擎逻辑由
//! `cargo test --lib editor::` 的 24 个单测覆盖,这里手工 seed 行模型
//! (spans 用 example 内 mini tokenizer 生成,与设计稿 JS 规则同源)。

use mterm::ui::{EditorLineRow, EditorSpan, EditorSpanKind, EditorTabInfo, EditorWindow, PickerMath, Theme};
use regex::Regex;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
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

fn grab(ui: &EditorWindow, path: &str) {
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

// ── mini tokenizer(设计稿 JS 规则精简版,仅示例着色)──────────
#[derive(Clone, Copy, PartialEq)]
enum K {
    Plain,
    Kw,
    Ty,
    Fn,
    Str,
    Num,
    Com,
    Ann,
    Ts,
    Thr,
    LvlError,
    LvlWarn,
    LvlInfo,
    LvlDebug,
    Exc,
    At,
    Cs,
    Hl,
    HlCur,
}

impl K {
    fn to_slint(self) -> EditorSpanKind {
        match self {
            K::Plain => EditorSpanKind::Plain,
            K::Kw => EditorSpanKind::Kw,
            K::Ty => EditorSpanKind::Ty,
            K::Fn => EditorSpanKind::Fn,
            K::Str => EditorSpanKind::Str,
            K::Num => EditorSpanKind::Num,
            K::Com => EditorSpanKind::Com,
            K::Ann => EditorSpanKind::Ann,
            K::Ts => EditorSpanKind::Ts,
            K::Thr => EditorSpanKind::Thr,
            K::LvlError => EditorSpanKind::LvlError,
            K::LvlWarn => EditorSpanKind::LvlWarn,
            K::LvlInfo => EditorSpanKind::LvlInfo,
            K::LvlDebug => EditorSpanKind::LvlDebug,
            K::Exc => EditorSpanKind::Exc,
            K::At => EditorSpanKind::At,
            K::Cs => EditorSpanKind::Cs,
            K::Hl => EditorSpanKind::Hl,
            K::HlCur => EditorSpanKind::HlCur,
        }
    }
}

struct Rule {
    re: Regex,
    k: K,
}
fn ri(p: &str, k: K) -> Rule {
    Rule { re: Regex::new(p).unwrap(), k }
}

fn java_rules() -> Vec<Rule> {
    vec![
        ri(r"//.*$|/\*.*$|^\s*\*.*$", K::Com),
        ri(r#""(?:\\.|[^"\\])*""#, K::Str),
        ri(r"@\w+", K::Ann),
        ri(r"\b(public|private|protected|static|final|void|class|interface|return|new|if|else|for|while|try|catch|throw|throws|implements|extends|import|package|this|super|null|true|false)\b", K::Kw),
        ri(r"\b[A-Z][A-Za-z0-9_]*\b", K::Ty),
        ri(r"\b\d+\b", K::Num),
    ]
}
fn log_rules() -> Vec<Rule> {
    vec![
        ri(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+", K::Ts),
        ri(r"\[[\w .,-]+\]", K::Thr),
        ri(r"\b(?:FATAL|ERROR)\b", K::LvlError),
        ri(r"\bWARN(?:ING)?\b", K::LvlWarn),
        ri(r"\bINFO\b", K::LvlInfo),
        ri(r"\b(?:DEBUG|TRACE)\b", K::LvlDebug),
        ri(r"^Caused by:.*$", K::Cs),
        ri(r"^\s+at .*", K::At),
        ri(r"[\w.$]+(?:Exception|Error)\b", K::Exc),
    ]
}
fn nginx_rules() -> Vec<Rule> {
    vec![
        ri(r"#.*$", K::Com),
        ri(r#""[^"]*""#, K::Str),
        ri(r"\$\w+", K::Ann),
        ri(r"\b(user|worker_processes|pid|events|http|include|default_type|log_format|access_log|sendfile|tcp_nopush|keepalive_timeout|client_max_body_size|gzip|gzip_types|server|listen|server_name|ssl_certificate|ssl_certificate_key|location|proxy_pass|proxy_set_header|proxy_read_timeout|root|try_files|worker_connections)\b", K::Fn),
        ri(r"\b\d+\b", K::Num),
        ri(r"[{}]", K::Ty),
    ]
}
fn sql_rules() -> Vec<Rule> {
    vec![
        ri(r"--.*$", K::Com),
        ri(r"'[^']*'", K::Str),
        ri(r"`[^`]*`", K::Ty),
        ri(r"\b(CREATE|TABLE|INSERT|INTO|SELECT|FROM|WHERE|GROUP|BY|ORDER|ON|DUPLICATE|KEY|UPDATE|VALUES|NOT|NULL|DEFAULT|PRIMARY|UNIQUE|COUNT|SUM|DATE|LIMIT|AND|OR|IN|AS)\b", K::Kw),
        ri(r"\b\d+\b", K::Num),
    ]
}

/// 一行 → spans(逐位置最早命中,与设计稿 JS 版同语义)。
fn tok(line: &str, rules: &[Rule]) -> Vec<EditorSpan> {
    let mut out: Vec<EditorSpan> = Vec::new();
    let mut i = 0usize;
    let total = line.chars().count();
    while i < line.len() {
        if !line.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let mut best: Option<(usize, usize, K)> = None;
        for rule in rules {
            if let Some(m) = rule.re.find_at(line, i) {
                if best.is_none() || m.start() < best.unwrap().0 {
                    best = Some((m.start(), m.end(), rule.k));
                }
            }
        }
        match best {
            Some((s, e, k)) => {
                if s > i {
                    push_plain(&mut out, line, i, s);
                }
                push_span(&mut out, line, s, e, k);
                i = e;
            }
            None => {
                push_plain(&mut out, line, i, line.len());
                break;
            }
        }
    }
    if out.iter().map(|s| s.start as usize + s.text.chars().count()).max().unwrap_or(0) < total {
        let consumed = out.iter().map(|s| s.start as usize + s.text.chars().count()).max().unwrap_or(0);
        let seg: String = line.chars().skip(consumed).collect();
        let w = (seg.chars().count() as f32) * 7.8;
        if !seg.is_empty() {
            out.push(EditorSpan {
                text: seg.into(),
                start: consumed as i32,
                width: w,
                kind: EditorSpanKind::Plain,
            });
        }
    }
    out
}

fn char_at(line: &str, b: usize) -> usize {
    line[..b].chars().count()
}

fn push_plain(out: &mut Vec<EditorSpan>, line: &str, s: usize, e: usize) {
    push_span(out, line, s, e, K::Plain);
}
fn push_span(out: &mut Vec<EditorSpan>, line: &str, s: usize, e: usize, k: K) {
    let cs = char_at(line, s);
    let ce = char_at(line, e);
    if ce > cs {
        let seg: String = line[s..e].to_string();
        out.push(EditorSpan {
            text: seg.into(),
            start: cs as i32,
            width: ((ce - cs) as f32) * 7.8,
            kind: k.to_slint(),
        });
    }
}

fn row(
    line: usize,
    text: &str,
    rules: &[Rule],
    is_current: bool,
    cursor_col: usize,
    sel: Option<(usize, usize)>,
    mark: bool,
) -> EditorLineRow {
    let spans = tok(text, rules);
    let cursor_x = if is_current {
        cursor_col as f32 * 7.8
    } else {
        0.0
    };
    let (sel_x, sel_w) = match sel {
        Some((a, b)) => (
            (a.min(b) as f32) * 7.8,
            ((a.max(b) - a.min(b)) as f32) * 7.8,
        ),
        None => (0.0, 0.0),
    };
    EditorLineRow {
        line: line as i32,
        text: text.into(),
        spans: ModelRc::new(VecModel::from(spans)),
        is_current,
        cursor_x,
        sel_x,
        sel_w,
        mark,
        hit: false,
    }
}

fn tab(name: &str, lang: &str, letter: &str, active: bool, dirty: bool) -> EditorTabInfo {
    let color = match letter {
        "J" => slint::Color::from_rgb_u8(0xE8, 0xA3, 0x3E),
        "L" => slint::Color::from_rgb_u8(0x7F, 0x84, 0x8B),
        "N" => slint::Color::from_rgb_u8(0x73, 0xB0, 0x0A),
        "S" => slint::Color::from_rgb_u8(0xC7, 0x7D, 0xBB),
        _ => slint::Color::from_rgb_u8(0x7F, 0x84, 0x8B),
    };
    EditorTabInfo {
        id: name.into(),
        name: name.into(),
        dirty,
        active,
        lang: lang.into(),
        letter: letter.into(),
        icon_color: color,
    }
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = EditorWindow::new().expect("build editor window");
    ui.window().set_size(slint::LogicalSize::new(W as f32, H as f32));
    {
        let th = ui.global::<Theme<'_>>();
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
        // (#editor-hifi) 离屏环境字体链:mono 用 Consolas,其 13px 单格宽 ≈7.15px
        th.set_font_mono("Consolas".into());
        th.set_ui_font_family("Segoe UI".into());
    }
    ui.set_char_px(7.15);
    // 纯回调必须注册(否则返回默认透明色;签名与 color_picker.slint 一致)
    let pk = ui.global::<PickerMath<'_>>();
    pk.on_ink_on(move |hex: SharedString| {
        let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xFFFFFF);
        let (r, g, b) = (((v >> 16) & 0xFF) as f64, ((v >> 8) & 0xFF) as f64, (v & 0xFF) as f64);
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        if luma > 128.0 { slint::Color::from_rgb_u8(0x0B, 0x26, 0x22) } else { slint::Color::from_rgb_u8(0xFF, 0xFF, 0xFF) }
    });

    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    let jr = java_rules();
    let lr = log_rules();
    let nr = nginx_rules();
    let sr = sql_rules();

    // ── 场景① Java 编辑态 ──
    {
        let src: Vec<&str> = vec![
            "package com.mterm.transfer.sftp;",
            "",
            "import java.io.IOException;",
            "import java.nio.file.Path;",
            "import java.time.Duration;",
            "import java.util.List;",
            "",
            "import org.slf4j.Logger;",
            "import org.slf4j.LoggerFactory;",
            "",
            "/** SFTP 传输服务:上传/下载/断点续传。 */",
            "@Service",
            "public class SftpTransferService implements TransferService {",
            "    private static final Logger log = LoggerFactory.getLogger(SftpTransferService.class);",
            "",
            "    private static final int MAX_RETRIES = 3;",
            "    private final RetryPolicy retryPolicy;",
            "    private final SshSessionPool sessionPool;",
            "",
            "    public SftpTransferService(SshSessionPool sessionPool, RetryPolicy retryPolicy) {",
            "        this.sessionPool = sessionPool;",
            "        this.retryPolicy = retryPolicy;",
            "    }",
            "",
            "    /** 同步上传(带断点续传),返回远端字节数。 */",
            "    public long syncUpload(String remotePath, Path localFile) throws IOException {",
            "        for (int attempt = 1; attempt <= MAX_RETRIES; attempt++) {",
            "            try (SftpClient sftp = sessionPool.acquire()) {",
            "                sftp.resume(localFile).remote(remotePath).chunk(1 << 20);",
            "                long bytes = sftp.finish();",
            "                log.info(\"upload done: {} ({} bytes)\", remotePath, bytes);",
            "                return bytes;",
            "            } catch (IOException e) {",
            "                if (attempt == MAX_RETRIES) throw e;",
            "                TimeUnit.MILLISECONDS.sleep(backoff(attempt));",
            "            }",
            "        }",
            "        throw new IOException(\"unreachable\");",
            "    }",
        ];
        let mut rows = Vec::new();
        for (i, text) in src.iter().enumerate() {
            let ln = i + 1;
            let mark = (21..=23).contains(&ln);
            rows.push(row(ln, text, &jr, ln == 30, 14, if (31..=33).contains(&ln) { Some((0, 200)) } else { None }, mark));
        }
        ui.set_editor_tabs(model(vec![
            tab("SftpService.java", "Java", "J", true, true),
            tab("app.log", "日志", "L", false, false),
            tab("nginx.conf", "nginx.conf", "N", false, false),
            tab("orders.sql", "SQL", "S", false, false),
        ]));
        ui.set_editor_lines(model(rows));
        ui.set_editor_first_row(0);
        ui.set_editor_scroll_frac(0);
        ui.set_editor_scroll_x(0);
        ui.set_editor_scroll_y(0);
        ui.set_editor_content_w(1800);
        ui.set_editor_content_h(39740);
        ui.set_editor_gutter_w(56);
        ui.set_editor_name("SftpService.java".into());
        ui.set_editor_path("/var/www/app/src/main/java/com/mterm/transfer/sftp/SftpService.java".into());
        ui.set_editor_host("prod-web01".into());
        ui.set_editor_encoding("UTF-8".into());
        ui.set_editor_lang_display("Java".into());
        ui.set_editor_badge("修改".into());
        ui.set_editor_badge_kind("dirty".into());
        ui.set_editor_readonly(false);
        ui.set_editor_cursor_line(30);
        ui.set_editor_cursor_col(15);
        ui.set_editor_sel_lines(3);
        ui.set_editor_sel_chars(96);
        ui.set_editor_upload_percent(-1);
        ui.set_editor_status_msg("".into());
        ui.set_editor_banner_visible(false);
        ui.set_editor_find_open(false);
        ui.set_editor_ime_open(false);
        ui.set_ctx_open(false);
        ui.set_tabmenu_open(false);
        ui.set_dl_open(false);
        ui.set_goto_open(false);
        grab(&ui, "output/editor-hifi/scene1.png");
    }

    // ── 场景② 大日志只读 ──
    {
        let src: Vec<&str> = vec![
            "2026-11-14 21:02:44.102 [http-nio-8080-exec-7] ERROR c.m.t.s.SftpTransferService - Failed to upload /data/upload/order_20261114.csv",
            "org.apache.catalina.connector.ClientAbortException: java.net.SocketTimeoutException: Read timed out",
            "    at org.apache.catalina.connector.OutputBuffer.doFlush(OutputBuffer.java:356)",
            "    at org.apache.catalina.connector.OutputBuffer.flush(OutputBuffer.java:319)",
            "    at com.mterm.transfer.sftp.SftpTransferService.upload(SftpTransferService.java:118)",
            "",
            "Caused by: java.net.SocketTimeoutException: Read timed out",
            "    at java.base/java.net.SocketInputStream.socketRead0(Native Method)",
            "",
            "2026-11-14 21:02:44.318 [http-nio-8080-exec-2] INFO  c.m.t.s.OrderService - Order ORD-20261114-84912 settled, amount=128.50",
            "2026-11-14 21:02:44.412 [scheduling-1] WARN  c.m.t.s.RateLimiter - bucket nearly full: 982/1000, shedding 4 requests",
            "2026-11-14 21:02:44.501 [http-nio-8080-exec-4] INFO  c.m.t.w.HealthController - health check ok, uptime=3d 04:11:02",
            "2026-11-14 21:02:45.017 [pool-3-thread-9] DEBUG c.m.t.s.ConnectionPool - lease connection #42, active=17 idle=31",
            "2026-11-14 21:02:45.183 [http-nio-8080-exec-7] ERROR c.m.t.s.UploadQueue - java.lang.OutOfMemoryError: Metaspace retained 412MB",
        ];
        let mut rows = Vec::new();
        for (i, text) in src.iter().enumerate() {
            rows.push(row(84912 + i, text, &lr, i == 0, 0, if (2..=4).contains(&i) { Some((0, 300)) } else { None }, false));
        }
        ui.set_editor_tabs(model(vec![
            tab("SftpService.java", "Java", "J", false, true),
            tab("app.log", "日志", "L", true, false),
            tab("nginx.conf", "nginx.conf", "N", false, false),
            tab("orders.sql", "SQL", "S", false, false),
        ]));
        ui.set_editor_lines(model(rows));
        ui.set_editor_first_row(84911);
        ui.set_editor_content_h(1204557 * 20);
        ui.set_editor_gutter_w(74);
        ui.set_editor_replace_open(false);
        ui.set_editor_name("app.log".into());
        ui.set_editor_path("/var/log/app/app.log".into());
        ui.set_editor_badge("只读".into());
        ui.set_editor_badge_kind("ro".into());
        ui.set_editor_readonly(true);
        ui.set_editor_banner_visible(true);
        ui.set_editor_banner_text("已按只读模式打开 · 84.3 MB · 1,204,557 行 · 高亮:日志规则(大文件模式,后台流式建索引)".into());
        ui.set_editor_lang_display("日志".into());
        ui.set_editor_cursor_line(84912);
        ui.set_editor_cursor_col(1);
        ui.set_editor_sel_lines(2);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-hifi/scene2.png");
    }

    // ── 场景③ nginx.conf ──
    {
        let src: Vec<&str> = vec![
            "# MTerm 生产机 nginx 主配置",
            "# /etc/nginx/nginx.conf — 2026-11-12 由 ops 同步",
            "",
            "user www-data;",
            "worker_processes auto;",
            "",
            "http {",
            "    include /etc/nginx/mime.types;",
            "    default_type application/octet-stream;",
            "",
            "    server {",
            "        listen 443 ssl;",
            "        server_name mterm.example.com;",
            "",
            "        location /api/ {",
            "            proxy_pass http://127.0.0.1:8080;",
            "            proxy_set_header Host $host;",
            "            proxy_set_header X-Real-IP $remote_addr;",
            "            proxy_read_timeout 300s;",
            "        }",
            "",
            "        location / {",
            "            root /var/www/mterm;",
            "            try_files $uri $uri/ /index.html;",
            "        }",
            "    }",
            "}",
        ];
        let mut rows = Vec::new();
        for (i, text) in src.iter().enumerate() {
            rows.push(row(i + 1, text, &nr, i == 12, 22, if (15..=17).contains(&i) { Some((0, 260)) } else { None }, false));
        }
        ui.set_editor_tabs(model(vec![
            tab("SftpService.java", "Java", "J", false, false),
            tab("app.log", "日志", "L", false, false),
            tab("nginx.conf", "nginx.conf", "N", true, false),
            tab("orders.sql", "SQL", "S", false, false),
        ]));
        ui.set_editor_lines(model(rows));
        ui.set_editor_first_row(0);
        ui.set_editor_content_h(127 * 20);
        ui.set_editor_gutter_w(56);
        ui.set_editor_name("nginx.conf".into());
        ui.set_editor_path("/etc/nginx/nginx.conf".into());
        ui.set_editor_badge("".into());
        ui.set_editor_badge_kind("none".into());
        ui.set_editor_readonly(false);
        ui.set_editor_banner_visible(false);
        ui.set_editor_lang_display("nginx.conf".into());
        ui.set_editor_sel_lines(3);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-hifi/scene3.png");
    }

    // ── 场景④ orders.sql ──
    {
        let src: Vec<&str> = vec![
            "-- 订单月度汇总(供 BI 抽数)",
            "",
            "CREATE TABLE `order_daily_stats` (",
            "  `id`         BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,",
            "  `stat_date`  DATE            NOT NULL COMMENT '统计日期',",
            "  `orders`     INT UNSIGNED    NOT NULL DEFAULT 0,",
            "  `gmv`        DECIMAL(12,2)   NOT NULL DEFAULT 0.00,",
            "  PRIMARY KEY (`id`),",
            "  UNIQUE KEY `uk_date` (`stat_date`)",
            ") ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='订单日统计';",
            "",
            "INSERT INTO order_daily_stats (stat_date, orders, gmv)",
            "SELECT DATE(o.created_at)   AS stat_date,",
            "       COUNT(DISTINCT o.id) AS orders,",
            "       SUM(o.amount)        AS gmv",
            "  FROM orders o",
            " WHERE o.status IN (2, 3)",
            " GROUP BY DATE(o.created_at)",
            " ORDER BY stat_date",
            " ON DUPLICATE KEY UPDATE gmv = VALUES(gmv);",
        ];
        let mut rows = Vec::new();
        for (i, text) in src.iter().enumerate() {
            rows.push(row(i + 1, text, &sr, i == 16, 8, None, false));
        }
        ui.set_editor_tabs(model(vec![
            tab("SftpService.java", "Java", "J", false, false),
            tab("app.log", "日志", "L", false, false),
            tab("nginx.conf", "nginx.conf", "N", false, false),
            tab("orders.sql", "SQL", "S", true, false),
        ]));
        ui.set_editor_lines(model(rows));
        ui.set_editor_first_row(0);
        ui.set_editor_content_h(86 * 20);
        ui.set_editor_gutter_w(56);
        ui.set_editor_name("orders.sql".into());
        ui.set_editor_path("/home/chenbozuo/sql/orders.sql".into());
        ui.set_editor_host("prod-db01".into());
        ui.set_editor_badge("".into());
        ui.set_editor_badge_kind("none".into());
        ui.set_editor_banner_visible(false);
        ui.set_editor_lang_display("SQL".into());
        ui.set_editor_sel_lines(0);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-hifi/scene4.png");
    }

    // ── 场景⑤ 查找/替换(命中 hl/hl-cur)──
    {
        let src: Vec<&str> = vec![
            "    private final RetryPolicy retryPolicy;",
            "",
            "    public long syncUpload(String remotePath, Path localFile) throws IOException {",
            "        for (int attempt = 1; attempt <= MAX_RETRIES; attempt++) {",
            "            try (SftpClient sftp = sessionPool.acquire()) {",
            "                sftp.resume(localFile).remote(remotePath).chunk(1 << 20);",
            "                long bytes = sftp.finish();",
            "                log.info(\"upload done: {} ({} bytes)\", remotePath, bytes);",
            "                return bytes;",
        ];
        let mut rows = Vec::new();
        for (i, text) in src.iter().enumerate() {
            let mut r = row(i + 1, text, &jr, false, 0, None, false);
            // sftp 命中:第 2 处为当前命中(行 2 的 SftpClient / sftp)
            if i == 1 {
                // "public long syncUpload(...)" 里的 sftp?命中串 "sftp"(大小写不敏感)
            }
            if i == 3 {
                let mut spans2: Vec<EditorSpan> = Vec::new();
                for sp in r.spans.iter() {
                    spans2.push(sp.clone());
                }
                // 在 "sftp = sessionPool" 处插命中:近似——直接在行尾追加 hl 片段演示
                let _ = &mut spans2;
                r.spans = model(spans2);
            }
            rows.push(r);
        }
        // 简化命中演示:行 4 "SftpClient sftp" 的 sftp 词加 hl(直接构造)
        let mut r4 = row(4, "            try (SftpClient sftp = sessionPool.acquire()) {", &jr, false, 0, None, false);
        let mut spans4: Vec<EditorSpan> = Vec::new();
        for sp in r4.spans.iter() {
            spans4.push(sp.clone());
        }
        spans4.push(EditorSpan {
            text: "sftp".into(),
            start: 28,
            width: 4.0 * 7.8,
            kind: EditorSpanKind::HlCur,
        });
        r4.spans = model(spans4);
        rows[3] = r4;
        ui.set_editor_tabs(model(vec![
            tab("SftpService.java", "Java", "J", true, true),
            tab("app.log", "日志", "L", false, false),
            tab("nginx.conf", "nginx.conf", "N", false, false),
            tab("orders.sql", "SQL", "S", false, false),
        ]));
        ui.set_editor_lines(model(rows));
        ui.set_editor_first_row(0);
        ui.set_editor_gutter_w(56);
        ui.set_editor_name("SftpService.java".into());
        ui.set_editor_path("/var/www/app/src/main/java/com/mterm/transfer/sftp/SftpService.java".into());
        ui.set_editor_badge("修改".into());
        ui.set_editor_badge_kind("dirty".into());
        ui.set_editor_readonly(false);
        ui.set_editor_banner_visible(false);
        ui.set_editor_find_open(true);
        ui.set_editor_find_query("sftp".into());
        ui.set_editor_replace_text("重试链接".into());
        ui.set_editor_replace_open(true);
        ui.set_editor_find_count(13);
        ui.set_editor_find_cur(4);
        ui.set_editor_find_hit_lines(model(vec![1, 5, 6, 8, 20]));
        ui.set_editor_lang_display("Java".into());
        ui.set_editor_sel_lines(0);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-hifi/scene5.png");
    }

    // ── 场景⑥ 保存回传中 ──
    {
        let src: Vec<&str> = vec![
            "    public SftpTransferService(SshSessionPool sessionPool, RetryPolicy retryPolicy) {",
            "        this.sessionPool = sessionPool;",
            "        this.retryPolicy = retryPolicy;",
            "    }",
            "",
            "    /** 同步上传(带断点续传),返回远端字节数。 */",
            "    public long syncUpload(String remotePath, Path localFile) throws IOException {",
            "        for (int attempt = 1; attempt <= MAX_RETRIES; attempt++) {",
            "            try (SftpClient sftp = sessionPool.acquire()) {",
            "                sftp.resume(localFile).remote(remotePath).chunk(1 << 20);",
        ];
        let mut rows = Vec::new();
        for (i, text) in src.iter().enumerate() {
            rows.push(row(i + 1, text, &jr, i == 2, 14, None, (0..=2).contains(&i)));
        }
        ui.set_editor_tabs(model(vec![
            tab("SftpService.java", "Java", "J", true, true),
            tab("app.log", "日志", "L", false, false),
            tab("nginx.conf", "nginx.conf", "N", false, false),
            tab("orders.sql", "SQL", "S", false, false),
        ]));
        ui.set_editor_lines(model(rows));
        ui.set_editor_first_row(0);
        ui.set_editor_gutter_w(56);
        ui.set_editor_name("SftpService.java".into());
        ui.set_editor_path("/var/www/app/src/main/java/com/mterm/transfer/sftp/SftpService.java".into());
        ui.set_editor_badge("回传中".into());
        ui.set_editor_badge_kind("up".into());
        ui.set_editor_readonly(false);
        ui.set_editor_upload_percent(68);
        ui.set_editor_upload_text("回传 prod-web01 · SftpService.java".into());
        ui.set_editor_banner_visible(false);
        ui.set_editor_find_open(false);
        ui.set_editor_lang_display("Java".into());
        ui.set_editor_sel_lines(0);
        ui.set_editor_status_msg("".into());
        grab(&ui, "output/editor-hifi/scene6.png");
    }

    println!("editor_shot 全部完成 → output/editor-hifi/");
}
