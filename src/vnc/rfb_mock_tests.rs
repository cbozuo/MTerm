//! (#vnc-connect-probe 2026-11-17) 进程内最小 RFB 服务器:用确定的字节序驱动
//! 完整握手 + 首帧,定位「无法连接」发生在哪一层 —— 版本协商、安全类型、
//! ServerInit、像素管线,而不是隔着真机猜。
//!
//! 三台"服务器":
//! * `None` 认证(RFB 3.8)—— 必须连上并出首帧;
//! * `VncAuth` 认证 —— 挑战/响应走 vnc-rs 的 DES 实现;
//! * 只提供 `Tight` —— 库不支持,必须以 Failed(带原因)结束且不重试。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use super::client::{run_with_input, DriverDeps};
use super::config::VncConnectConfig;
use crate::rdp::embedded::input::RemoteInput;
use crate::rdp::embedded::status::RemoteState;
use crate::remote::pane::RemoteEvent;

const W: u16 = 64;
const H: u16 = 48;
/// 每像素的线序字节:协商格式是 bgra()(red_shift=16 / blue_shift=0),线序 [B,G,R,A]。
const PIXEL: [u8; 4] = [0x10, 0x20, 0x30, 0xff];

/// 按子编码造 ZRLE 的**未压缩** tile 数据(调色板/游程/裸像素)。
///
/// 约定:调色板两个颜色 A/B —— **第 0 行是 A,其余行是 B**,这样断言能同时验证
/// 「调色板选取」与「索引 packing 顺序」,而不只是"解出来了"。
fn zrle_payload(mode: ZrleMode) -> Vec<u8> {
    const A: [u8; 3] = [0x10, 0x20, 0x30]; // 线序 [B,G,R]
    const B: [u8; 3] = [0x40, 0x50, 0x60];
    let (w, h) = (usize::from(W), usize::from(H));
    let mut out = Vec::new();
    match mode {
        ZrleMode::Solid => {
            out.push(1);
            out.extend_from_slice(&A);
        }
        ZrleMode::Raw => {
            out.push(0);
            for i in 0..w * h {
                out.extend_from_slice(if i < w { &A } else { &B });
            }
        }
        ZrleMode::Palette => {
            // control = 调色板大小(2);随后两个 3 字节颜色;索引 **1 bit/像素**
            // (调色板 2 色 → bits_per_index=1),MSB 优先,每行补到字节边界。
            out.push(2);
            out.extend_from_slice(&A);
            out.extend_from_slice(&B);
            for y in 0..h {
                let idx: u8 = if y == 0 { 0 } else { 1 };
                let mut byte = 0u8;
                let mut bit = 0;
                for _ in 0..w {
                    byte |= idx << (7 - bit);
                    bit += 1;
                    if bit == 8 {
                        out.push(byte);
                        byte = 0;
                        bit = 0;
                    }
                }
                if bit != 0 {
                    out.push(byte);
                }
            }
        }
        ZrleMode::Rle => {
            // control = 0x80 | 调色板大小(游程 + 索引调色板)。每条目 = 一个
            // control 字节(高位置 1 表示后面跟游程,低 7 位是索引)+ 游程分量
            // (每字节 255 续、<255 收尾;总和 + 1 = 游程长度)。
            out.push(0x80 | 2);
            out.extend_from_slice(&A);
            out.extend_from_slice(&B);
            push_indexed_run(&mut out, 0, w); // 第 0 行全 A
            push_indexed_run(&mut out, 1, (h - 1) * w); // 其余全 B
        }
    }
    out
}

/// 索引调色板的 RLE 条目:control(0x80|index)+ 游程分量(255 续 / <255 收尾),
/// 语义 = 1 + Σ分量。
fn push_indexed_run(out: &mut Vec<u8>, index: u8, run: usize) {
    assert!(run >= 1);
    out.push(0x80 | (index & 0x7f));
    let mut rem = run - 1;
    while rem >= 255 {
        out.push(255);
        rem -= 255;
    }
    out.push(rem as u8);
}

/// 手搓 ZRLE 的 zlib 数据段:0x78 0x01 + 存储块(不压缩)。
///
/// `final_block=false` 模拟**真实服务器**:RFC 6143 §7.7.6 说整条连接共用一个
/// zlib 流(每次 update 只是往里追加),所以块永远不是最后一块、adler32 也不出现。
/// 发"完整流"(BFINAL=1 + adler32)会让 vnc-rs 的 reader 把 `Status::StreamEnd`
/// 当错误报 "zlib stream end" —— 那是客户端对**收尾流**的处理,不是服务器会发的形态。
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }
    let mut out = vec![0x78, 0x01];
    if data.is_empty() {
        // 空的存储块;仍然不是最后一块(流持续)
        out.extend_from_slice(&[0, 0, 0, 0xff, 0xff]);
    } else {
        for chunk in data.chunks(65535) {
            out.push(0); // BFINAL=0 / BTYPE=00(stored):流未结束
            out.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
            out.extend_from_slice(&(!(chunk.len() as u16)).to_le_bytes());
            out.extend_from_slice(chunk);
        }
    }
    // 不收尾:不发 adler32(那是整条流结束才有的)
    let _ = adler32;
    out
}

fn pixel_format() -> Vec<u8> {
    let mut pf = Vec::with_capacity(16);
    pf.push(32); // bits-per-pixel
    pf.push(24); // depth
    pf.push(0); // big-endian
    pf.push(1); // true-colour
    pf.extend_from_slice(&255u16.to_be_bytes()); // red-max
    pf.extend_from_slice(&255u16.to_be_bytes()); // green-max
    pf.extend_from_slice(&255u16.to_be_bytes()); // blue-max
    // RFC6143 §7.4:三个 shift 各占 1 字节,后跟 3 字节 padding(此前写成 u16 会
    // 让 vnc-rs 的校验判重叠掩码失败 → Unknown pixel format)。
    pf.push(16); // red-shift
    pf.push(8); // green-shift
    pf.push(0); // blue-shift
    pf.extend_from_slice(&[0, 0, 0]); // padding
    pf
}

/// 读掉一条客户端消息,返回总字节数(内容不校验,只保持流同步)。
async fn read_client_msg<S: tokio::io::AsyncRead + Unpin>(s: &mut S) -> Result<usize, String> {
    let mut head = [0u8; 1];
    s.read_exact(&mut head).await.map_err(|e| e.to_string())?;
    match head[0] {
        0 => {
            let mut rest = [0u8; 19];
            s.read_exact(&mut rest).await.map_err(|e| e.to_string())?;
            Ok(20)
        }
        2 => {
            let mut pad_count = [0u8; 3];
            s.read_exact(&mut pad_count).await.map_err(|e| e.to_string())?;
            let count = u16::from_be_bytes([pad_count[1], pad_count[2]]) as usize;
            let mut encs = vec![0u8; count * 4];
            s.read_exact(&mut encs).await.map_err(|e| e.to_string())?;
            Ok(4 + count * 4)
        }
        3 => {
            let mut rest = [0u8; 9];
            s.read_exact(&mut rest).await.map_err(|e| e.to_string())?;
            Ok(10)
        }
        4 | 5 => {
            // ClientCutText: 1 + 3 pad + u32 len + text
            let mut rest = [0u8; 7];
            s.read_exact(&mut rest).await.map_err(|e| e.to_string())?;
            let len = u32::from_be_bytes([rest[4], rest[5], rest[6], rest[7]]) as usize;
            let mut text = vec![0u8; len];
            s.read_exact(&mut text).await.map_err(|e| e.to_string())?;
            Ok(8 + len)
        }
        other => Err(format!("unexpected client message {other}")),
    }
}

/// ZRLE 的 tile 子编码(RFC 6143 §7.7.6):服务器可能用其中任一种,
/// 只测 solid 不足以说明解码器可用。
#[derive(Clone, Copy, PartialEq, Eq)]
enum ZrleMode {
    /// control=1:整块一个颜色。
    Solid,
    /// control=0:每像素一个 3 字节 CPIXEL。
    Raw,
    /// control=n:调色板 + 每像素 2bit 索引(见下方 packing)。
    Palette,
    /// control=0x80|n:调色板 + 游程(run)编码。
    Rle,
}

#[derive(Clone, Copy)]
enum Security {
    None,
    /// None 认证 + 用 **ZRLE**(编码 16)发首帧 —— 检验库的 ZRLE 解码器。
    NoneZrle,
    VncAuth,
    /// 认证响应一律失败,并回原因串(模拟服务器数满失败次数后的封禁语)。
    VncAuthLockout,
    TightOnly,
}

/// 服务器侧观察到的事实,供断言使用。
#[derive(Default)]
struct ServerOutcome {
    /// 是否收到过客户端的 16 字节认证响应(空白密码场景必须为 false)。
    got_auth_response: bool,
    /// 接受过的连接数(空白密码时驱动先来一条只读探测连接)。
    connections: u32,
    /// 客户端是否**选择过**安全类型(探测连接必须在选择前断开)。
    sent_security_choice: bool,
}

impl ServerOutcome {
    fn merge(&mut self, other: ServerOutcome) {
        self.got_auth_response |= other.got_auth_response;
        self.sent_security_choice |= other.sent_security_choice;
        self.connections += other.connections;
    }
}

/// 一台 RFB 3.8 服务器。`security` 决定提供哪种认证。
///
/// 会持续接受连接,直到 3 秒内没有新连接 —— 空白密码时驱动先来一条**只读探测**
/// 连接(只做版本协商 + 读类型列表,不选类型),再来真正的会话连接。
async fn serve_rfb(
    listener: TcpListener,
    security: Security,
    password: &'static str,
) -> ServerOutcome {
    serve_rfb_with(listener, security, password, ZrleMode::Solid).await
}

async fn serve_rfb_with(
    listener: TcpListener,
    security: Security,
    password: &'static str,
    zrle: ZrleMode,
) -> ServerOutcome {
    let mut total = ServerOutcome::default();
    loop {
        match tokio::time::timeout(Duration::from_secs(3), listener.accept()).await {
            Ok(Ok((stream, _))) => {
                total.connections += 1;
                let one = serve_one(stream, security, password, zrle).await;
                total.merge(one);
                if total.connections >= 3 {
                    break;
                }
            }
            _ => break,
        }
    }
    total
}

/// 单条连接上的 RFB 3.8 服务。
async fn serve_one(
    stream: TcpStream,
    security: Security,
    password: &'static str,
    zrle: ZrleMode,
) -> ServerOutcome {
    let mut outcome = ServerOutcome::default();
    let (mut rd, mut wr) = stream.into_split();

    let _ = wr.write_all(b"RFB 003.008\n").await;
    let mut ver = [0u8; 12];
    if rd.read_exact(&mut ver).await.is_err() {
        return outcome;
    }

    match security {
        Security::None | Security::NoneZrle => {
            let _ = wr.write_all(&[1u8, 1]).await; // 1 种:None
            let mut chosen = [0u8; 1];
            if rd.read_exact(&mut chosen).await.is_err() {
                return outcome; // 探测连接:选择前就断开(不该被计为失败)
            }
            outcome.sent_security_choice = true;
            let _ = wr.write_all(&0u32.to_be_bytes()).await; // SecurityResult OK
        }
        Security::VncAuth | Security::VncAuthLockout => {
            let _ = wr.write_all(&[1u8, 2]).await; // 1 种:VncAuth
            let mut chosen = [0u8; 1];
            if rd.read_exact(&mut chosen).await.is_err() {
                return outcome; // 探测连接:选择前就断开
            }
            outcome.sent_security_choice = true;
            // 16 字节挑战 → 客户端回 16 字节 DES 响应 → SecurityResult
            let challenge: [u8; 16] = core::array::from_fn(|i| (i as u8) ^ 0x5a);
            let _ = wr.write_all(&challenge).await;
            // 给客户端 2 秒:空白密码时驱动在回调里就中止了,一个字节都不该来。
            let mut response = [0u8; 16];
            match tokio::time::timeout(Duration::from_secs(2), rd.read_exact(&mut response)).await {
                Ok(Ok(_)) => outcome.got_auth_response = true,
                _ => return outcome,
            }
            match security {
                Security::VncAuthLockout => {
                    // SecurityResult = 1(失败)+ 原因串(TigerVNC 的封禁语)
                    let reason = b"Too many security failures";
                    let _ = wr.write_all(&1u32.to_be_bytes()).await;
                    let _ = wr.write_all(&(reason.len() as u32).to_be_bytes()).await;
                    let _ = wr.write_all(reason).await;
                    let _ = wr.shutdown().await;
                    return outcome;
                }
                _ => {
                    let _ = wr.write_all(&0u32.to_be_bytes()).await;
                    let _ = password;
                }
            }
        }
        Security::TightOnly => {
            // 只提供 Tight(16):vnc-rs 不支持,必须以 Failed 收场
            let _ = wr.write_all(&[1u8, 16]).await;
            let mut chosen = [0u8; 1];
            let _ = rd.read_exact(&mut chosen).await;
            let _ = wr.shutdown().await;
            return outcome;
        }
    }

    // ServerInit
    let mut init = Vec::with_capacity(24 + 16 + 8);
    init.extend_from_slice(&W.to_be_bytes());
    init.extend_from_slice(&H.to_be_bytes());
    init.extend_from_slice(&pixel_format());
    init.extend_from_slice(&5u32.to_be_bytes());
    init.extend_from_slice(b"probe");
    if wr.write_all(&init).await.is_err() {
        return outcome;
    }
    let mut shared = [0u8; 1];
    if rd.read_exact(&mut shared).await.is_err() {
        return outcome;
    }

    // 客户端的 SetPixelFormat / SetEncodings / 首个 FramebufferUpdateRequest
    // 读循环常驻到连接关闭:驱动收帧后测试会关会话,socket 关闭读循环自然退出。
    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let reader = tokio::spawn(async move {
        let mut notified = false;
        loop {
            match read_client_msg(&mut rd).await {
                Ok(n) => {
                    if n == 10 && !notified {
                        notified = true;
                        let _ = notify_tx.send(()); // 首个刷新请求
                    }
                }
                Err(_) => break,
            }
        }
    });

    // 收到首个刷新请求后,发一帧更新(Raw 或 ZRLE,按 mode)
    let _ = tokio::time::timeout(Duration::from_secs(3), notify_rx.recv()).await;
    let mut upd = Vec::new();
    upd.push(0u8);
    upd.push(0);
    upd.extend_from_slice(&1u16.to_be_bytes());
    upd.extend_from_slice(&0u16.to_be_bytes());
    upd.extend_from_slice(&0u16.to_be_bytes());
    upd.extend_from_slice(&W.to_be_bytes());
    upd.extend_from_slice(&H.to_be_bytes());
    match security {
        Security::NoneZrle => {
            // RFC 6143 §7.7.6 ZRLE:矩形以 **4 字节长度** 开头,后接 zlib 流;
            // 整幅 64×48 落在第一个 64×64 tile 里,control=1 = solid,后跟一个
            // 3 字节 CPIXEL(bgra() 协商下 depth≤24 → 3 字节,alpha 省略)。
            upd.extend_from_slice(&16i32.to_be_bytes());
            let z = zlib_stored(&zrle_payload(zrle));
            upd.extend_from_slice(&(z.len() as u32).to_be_bytes());
            upd.extend_from_slice(&z);
        }
        _ => {
            upd.extend_from_slice(&0i32.to_be_bytes()); // Raw
            for _ in 0..(usize::from(W) * usize::from(H)) {
                upd.extend_from_slice(&PIXEL);
            }
        }
    }
    let _ = wr.write_all(&upd).await;

    // 连接随驱动关闭而结束:读循环在 socket 关闭后自然退出。
    let _ = reader.await;
    outcome
}

/// 用测试内构造的配置跑完整驱动,收集事件直到「首帧」或「失败」或超时。
async fn run_driver(
    port: u16,
    password: &'static str,
    has_password: bool,
) -> Vec<Result<String, String>> {
    let cfg = VncConnectConfig {
        host: "127.0.0.1".into(),
        port,
        password: password.into(),
        has_password,
        view_only: false,
        shared: true,
        clipboard: false,
        follow_tab_size: false,
    };
    let status = Arc::new(crate::rdp::embedded::status::RemoteStatus::new(
        crate::rdp::embedded::status::RemoteState::Connecting,
    ));
    let closed = Arc::new(AtomicBool::new(false));
    let (events_tx, mut events_rx) = mpsc::unbounded_channel();
    let (_trust_tx, trust_rx) = mpsc::unbounded_channel();
    let (_input_tx, input_rx) = mpsc::unbounded_channel();
    let deps = DriverDeps {
        status: status.clone(),
        closed: closed.clone(),
        events: events_tx,
        trust: trust_rx,
    };

    let driver = tokio::spawn(run_with_input(cfg, deps, input_rx));

    let mut collected: Vec<Result<String, String>> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let ev = match tokio::time::timeout_at(deadline, events_rx.recv()).await {
            Ok(Some(ev)) => ev,
            _ => break,
        };
        let done = match &ev {
            RemoteEvent::Frame(_) => {
                collected.push(Ok("frame".into()));
                true
            }
            RemoteEvent::State { state, message } => {
                if state.state() == RemoteState::Connected {
                    collected.push(Ok("connected".into()));
                    false
                } else if state.state() == RemoteState::Failed {
                    let msg = message.clone().unwrap_or_default();
                    collected.push(Err(msg));
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        if done {
            break;
        }
    }
    closed.store(true, Ordering::Relaxed);
    let _ = driver.await;
    collected
}

fn last(events: &[Result<String, String>]) -> String {
    events
        .last()
        .map(|r| match r {
            Ok(s) => format!("ok:{s}"),
            Err(e) => format!("err:{e}"),
        })
        .unwrap_or_else(|| "timeout".into())
}

#[tokio::test]
async fn rfb38_none_auth_connects_and_delivers_the_first_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(serve_rfb(listener, Security::None, ""));

    let events = run_driver(port, "", false).await;
    let _ = server.await;

    assert!(
        events.iter().any(|r| matches!(r, Ok(s) if s == "connected")),
        "未连上: {:?}",
        events
    );
    assert!(
        events.iter().any(|r| matches!(r, Ok(s) if s == "frame")),
        "未收到首帧: {:?}",
        events
    );
}

#[tokio::test]
async fn rfb38_vnc_auth_connects_and_delivers_the_first_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(serve_rfb(listener, Security::VncAuth, "s3cret"));

    let events = run_driver(port, "s3cret", true).await;
    let _ = server.await;

    assert!(
        events.iter().any(|r| matches!(r, Ok(s) if s == "frame")),
        "VncAuth 未连上: {:?}",
        events
    );
}

/// 只提供 Tight 的服务器必须以「带原因的 Failed」收场,而不是无限重连。
#[tokio::test]
async fn a_tight_only_server_fails_with_the_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(serve_rfb(listener, Security::TightOnly, ""));

    let events = run_driver(port, "", false).await;
    let _ = server.await;

    let last = last(&events);
    // 失败必须发生在握手阶段,且给的是可行动文案(不是库的原始措辞)。
    assert!(last.starts_with("err:"), "应失败: {last}");
    assert!(
        last.contains("认证方式") || last.contains("None / VNC"),
        "应给可行动指引: {last}"
    );
}

/// 首帧的像素通道顺序:线序 [B,G,R,A] 进来,RGBA 出去。
#[tokio::test]
async fn the_first_frame_carries_the_expected_rgba() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(serve_rfb(listener, Security::None, ""));

    let cfg = VncConnectConfig {
        host: "127.0.0.1".into(),
        port,
        password: String::new(),
        has_password: false,
        view_only: false,
        shared: true,
        clipboard: false,
        follow_tab_size: false,
    };
    let status = Arc::new(crate::rdp::embedded::status::RemoteStatus::new(
        crate::rdp::embedded::status::RemoteState::Connecting,
    ));
    let closed = Arc::new(AtomicBool::new(false));
    let (events_tx, mut events_rx) = mpsc::unbounded_channel();
    let (_trust_tx, trust_rx) = mpsc::unbounded_channel();
    let (_input_tx, input_rx) = mpsc::unbounded_channel();
    let deps = DriverDeps {
        status: status.clone(),
        closed: closed.clone(),
        events: events_tx,
        trust: trust_rx,
    };
    let driver = tokio::spawn(run_with_input(cfg, deps, input_rx));

    let mut first_frame = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        match tokio::time::timeout_at(deadline, events_rx.recv()).await {
            Ok(Some(RemoteEvent::Frame(buf))) => {
                first_frame = Some(buf);
                break;
            }
            Ok(Some(_)) => continue,
            _ => break,
        }
    }
    closed.store(true, Ordering::Relaxed);
    let _ = driver.await;
    let _ = server.await;

    let buf = first_frame.expect("应收到首帧");
    let bytes = buf.as_bytes();
    // 线序 [B,G,R,A] = [0x10,0x20,0x30,0xff] → RGBA [0x30,0x20,0x10,0xff]
    assert_eq!(
        &bytes[..4],
        &[0x30, 0x20, 0x10, 0xff],
        "首像素 RGBA 通道顺序错误"
    );
}

/// (#vnc-blank-password 2026-11-17) 服务器要密码、会话里没有:驱动**必须一个认证
/// 字节都不发**就失败。旧行为是发出 8 个零字节的伪造 DES 响应 —— 服务器每次记一次
/// 失败,几次之后就用 "Too many security failures" 把地址封掉(用户实测症状)。
#[tokio::test]
async fn a_vnc_auth_server_without_a_stored_password_gets_no_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(serve_rfb(listener, Security::VncAuth, ""));

    // 空密码:has_password = false
    let events = run_driver(port, "", false).await;
    let outcome = server.await.unwrap();

    let last = last(&events);
    assert!(
        !outcome.got_auth_response,
        "空白密码时绝不允许发出认证响应(那会被服务器计为一次失败)"
    );
    // #vnc-no-burn 的关键:预检连接必须在**选择安全类型之前**断开,否则"选了
    // VncAuth 再断开"同样会被 TigerVNC 计为一次失败握手(用户实测的封禁来源)。
    assert!(
        !outcome.sent_security_choice,
        "不得向服务器声明选择过安全类型(否则仍会被计为失败握手)"
    );
    assert_eq!(outcome.connections, 1, "只应有那条只读探测连接");
    assert!(last.starts_with("err:"), "应失败: {last}");
    assert!(
        last.contains("密码") || last.contains("password"),
        "应告诉用户填密码: {last}"
    );
}

/// 服务器的封禁原因串必须被翻译成「先等待」的指引,且不可重试 —— 反复点重新连接
/// 只会延长封禁。
#[tokio::test]
async fn the_too_many_security_failures_reason_is_explained() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(serve_rfb(
        listener,
        Security::VncAuthLockout,
        "s3cret",
    ));

    let events = run_driver(port, "s3cret", true).await;
    let outcome = server.await.unwrap();
    assert!(outcome.got_auth_response, "这一次确实发过认证响应(密码非空)");

    let last = last(&events);
    assert!(last.starts_with("err:"), "应失败: {last}");
    assert!(
        last.contains("等待") || last.contains("封"),
        "应给出「先等待」的指引: {last}"
    );
    assert!(
        !last.contains("Too many security failures"),
        "不应把服务器的英文原话直接抛给用户: {last}"
    );
    // 只尝试一次:重试会把封禁越拖越长
    let attempts = events.iter().filter(|r| r.is_err()).count();
    assert_eq!(attempts, 1, "认证失败不得自动重试,实际 {attempts} 次");
}

/// 跑一次 ZRLE 会话取首帧像素;失败时返回错误文案。
async fn zrle_first_frame(mode: ZrleMode) -> Result<Vec<u8>, String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(serve_rfb_with(listener, Security::NoneZrle, "", mode));

    let cfg = VncConnectConfig {
        host: "127.0.0.1".into(),
        port,
        password: String::new(),
        has_password: false,
        view_only: false,
        shared: true,
        clipboard: false,
        follow_tab_size: false,
    };
    let status = Arc::new(crate::rdp::embedded::status::RemoteStatus::new(
        crate::rdp::embedded::status::RemoteState::Connecting,
    ));
    let closed = Arc::new(AtomicBool::new(false));
    let (events_tx, mut events_rx) = mpsc::unbounded_channel();
    let (_trust_tx, trust_rx) = mpsc::unbounded_channel();
    let (_input_tx, input_rx) = mpsc::unbounded_channel();
    let deps = DriverDeps {
        status: status.clone(),
        closed: closed.clone(),
        events: events_tx,
        trust: trust_rx,
    };
    let driver = tokio::spawn(run_with_input(cfg, deps, input_rx));

    let mut frame = None;
    let mut failure = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        match tokio::time::timeout_at(deadline, events_rx.recv()).await {
            Ok(Some(RemoteEvent::Frame(buf))) => {
                frame = Some(buf.as_bytes().to_vec());
                break;
            }
            Ok(Some(RemoteEvent::State { state, message })) => {
                if state.state() == RemoteState::Failed {
                    failure = message.clone();
                    break;
                }
            }
            Ok(Some(_)) => continue,
            _ => break,
        }
    }
    closed.store(true, Ordering::Relaxed);
    let _ = driver.await;
    let _ = server.await;

    frame.ok_or_else(|| format!("{failure:?}"))
}

/// 取缓冲里的某像素。注意:驱动交给 UI 的**已经是 RGBA**(VncFramebuffer::to_rgba
/// 在 blit 时就把线序 [B,G,R,A] 换成了 [R,G,B,A]),所以这里不再换序 —— 之前多换
/// 一次导致四个 ZRLE 用例全部假失败。
fn rgba_at(buf: &[u8], x: usize, y: usize) -> [u8; 4] {
    let off = (y * usize::from(W) + x) * 4;
    [buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]
}

/// A = 线序 [0x10,0x20,0x30] → RGBA [0x30,0x20,0x10];B = [0x40,0x50,0x60] →
/// RGBA [0x60,0x50,0x40]。
const A_RGBA: [u8; 4] = [0x30, 0x20, 0x10, 0xff];
const B_RGBA: [u8; 4] = [0x60, 0x50, 0x40, 0xff];

/// (#vnc-encodings 2026-11-17) ZRLE 四种 tile 子编码都必须解对。
///
/// 背景:曾断言"ZRLE 解码是坏的"(理由是 RFC 没有长度前缀)——**引用错了章节**
/// (§7.7.4 是 Hextile,ZRLE 是 §7.7.6,而原文确实规定 4 字节长度),据此把编码
/// 退成只请求 Raw,白白牺牲带宽。改正后重新请求 ZRLE,并用这组用例把结论钉死:
/// 只测 solid 不够 —— 服务器还会用调色板、游程、裸像素。
#[tokio::test]
async fn zrle_solid_tile_decodes() {
    let buf = zrle_first_frame(ZrleMode::Solid).await.expect("solid tile");
    assert_eq!(rgba_at(&buf, 0, 0), A_RGBA);
    assert_eq!(rgba_at(&buf, usize::from(W) - 1, usize::from(H) - 1), A_RGBA);
}

#[tokio::test]
async fn zrle_raw_tile_decodes_pixel_by_pixel() {
    let buf = zrle_first_frame(ZrleMode::Raw).await.expect("raw tile");
    assert_eq!(rgba_at(&buf, 0, 0), A_RGBA, "第 0 行应为 A");
    assert_eq!(rgba_at(&buf, 0, 1), B_RGBA, "第 1 行应为 B");
    assert_eq!(rgba_at(&buf, usize::from(W) - 1, 0), A_RGBA, "第 0 行末像素也应是 A");
}

#[tokio::test]
async fn zrle_palette_tile_decodes_indices_in_the_right_order() {
    let buf = zrle_first_frame(ZrleMode::Palette).await.expect("palette tile");
    // packing 顺序错了的话,同一行里会掺 B —— 逐像素验第 0 行
    for x in 0..usize::from(W) {
        assert_eq!(rgba_at(&buf, x, 0), A_RGBA, "第 0 行第 {x} 个像素索引错了");
    }
    assert_eq!(rgba_at(&buf, 0, 1), B_RGBA);
    assert_eq!(rgba_at(&buf, usize::from(W) - 1, usize::from(H) - 1), B_RGBA);
}

#[tokio::test]
async fn zrle_rle_tile_decodes_runs() {
    let buf = zrle_first_frame(ZrleMode::Rle).await.expect("rle tile");
    for x in 0..usize::from(W) {
        assert_eq!(rgba_at(&buf, x, 0), A_RGBA, "第 0 行第 {x} 个像素(游程)错了");
    }
    for x in [0usize, 1, usize::from(W) - 1] {
        assert_eq!(rgba_at(&buf, x, usize::from(H) - 1), B_RGBA, "末行游程错了");
    }
}
