//! The vnc-rs session driver for MTerm's built-in VNC client.
//!
//! Same shape as the RDP driver: build a connector, finish the handshake, then
//! pump events, input and a periodic frame flush. The differences that matter:
//!
//! * vnc-rs does **not** reconnect on its own, so the retry loop lives here.
//! * vnc-rs 0.6's connector API is much smaller than the reference code
//!   assumes: there is no `set_credentials_available`, `set_username`,
//!   `set_security_policy`, `set_limits` or `set_server_key_verifier`. Security
//!   type selection is therefore implicit (the library picks from what the
//!   server offers and asks the auth callback for the password), which is also
//!   why VNC's trust flow reduces to "the password was accepted or not" rather
//!   than a key fingerprint.
//! * There is no `VncSecurityPolicy` in this version, so `VncConnectConfig::security`
//!   is accepted and recorded but the library negotiates; keeping the field means
//!   the UI can offer it without a config migration when support lands.
//! * (#vnc-encodings 2026-11-17) Incremental updates go through
//!   `client.input(X11Event::Refresh)` (the library maps it to a
//!   `FramebufferUpdateRequest` at its own desktop size); the old driver
//!   hand-wrote those bytes over a *second* `TcpStream` cloned from the same
//!   socket — two tokio IO-driver registrations on one handle, pure risk for
//!   zero benefit.
//! * (#vnc-encodings) Encodings: **ZRLE + CopyRect + Raw**. An earlier round
//!   claimed vnc-rs's ZRLE decoder was broken "because it reads a 4-byte length
//!   prefix" — that claim was **wrong**: it cited Hextile (RFC 6143 §7.7.4)
//!   while ZRLE is §7.7.6, and the RFC does specify the length ("the rectangle
//!   begins with a 4-byte length field"). A conformant ZRLE stream decodes fine
//!   (see `rfb_mock_tests::a_zrle_coded_rect_decodes_to_the_expected_colour`),
//!   so ZRLE is requested again — that is where VNC bandwidth comes from.
//!   **Tight stays out**: only its zlib path is implemented, so a server that
//!   picks JPEG for photographic regions would break the whole frame.
//! * (#vnc-blank-password) A session without a password never sends a made-up
//!   response: the driver probes the server's security types read-only first
//!   and fails with an actionable message instead, so a password-requiring
//!   server (TigerVNC) does not count a failed handshake per attempt.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use vnc::{ClientKeyEvent, ClientMouseEvent, VncClient, VncConnector, VncEncoding, VncEvent};

use crate::rdp::embedded::input::RemoteInput;
use crate::rdp::embedded::status::{closing_state, RemoteState, RemoteStatus};
use crate::remote::pump::FrameQueue;
use crate::remote::pane::RemoteEvent;
use crate::vnc::config::VncConnectConfig;
use crate::vnc::framebuffer::{Rect, VncFramebuffer};

/// How often to ask the server for an incremental update.
///
/// RFB is server-push but client-paced: the client sends
/// `FramebufferUpdateRequest` and the server answers. 16 ms is roughly 60 Hz and
/// is also the interval at which frames are handed to the UI, so the two rates
/// match and no frame is produced that nobody will draw.
const UPDATE_REQUEST_INTERVAL: Duration = Duration::from_millis(16);

/// Cap on how long a TCP connect may take before we report a failure.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Cap on the handshake. A server that cannot finish RFB negotiation in this
/// time is not going to become useful.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// Transport-level retries before giving up (P1-10 makes this configurable).
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// Backoff before retry N.
fn reconnect_delay(attempt: u32) -> Duration {
    match attempt {
        0 | 1 => Duration::from_secs(1),
        2 => Duration::from_secs(2),
        3 => Duration::from_secs(4),
        _ => Duration::from_secs(8),
    }
}

/// Everything the driver needs that is not in [`VncConnectConfig`].
pub struct DriverDeps {
    pub status: Arc<RemoteStatus>,
    pub closed: Arc<AtomicBool>,
    pub events: mpsc::UnboundedSender<RemoteEvent>,
    /// Trust answers. VNC has no key fingerprint in this vnc-rs version, so
    /// the only trust-shaped question is whether to proceed at all when the
    /// server offers no authentication; the channel is kept so the UI path is
    /// identical for both protocols.
    pub trust: mpsc::UnboundedReceiver<(u64, bool, bool)>,
}

/// Run the session on the current runtime, retrying transport failures.
pub async fn run_with_input(
    config: VncConnectConfig,
    deps: DriverDeps,
    input_rx: mpsc::UnboundedReceiver<RemoteInput>,
) {
    let DriverDeps {
        status,
        closed,
        events,
        mut trust,
    } = deps;
    // The receiver is re-broadcast so every retry attempt can borrow it: the
    // driver outlives each attempt, but only one may hold the receiver at a time.
    let (input_tx, mut input_rx) = forward_input_channel(input_rx);

    let mut attempt: u32 = 0;
    // Set when the loop below has already published `Failed` with its reason, so
    // the tail does not replace it with a reasonless "Disconnected".
    let mut failure_reported = false;
    loop {
        if closed.load(Ordering::Relaxed) {
            break;
        }
        let state = if attempt == 0 {
            RemoteState::Connecting
        } else {
            RemoteState::Reconnecting
        };
        push_state(&status, &events, state, None);

        match run_once(
            &config,
            &status,
            &closed,
            &events,
            &input_tx,
            &mut input_rx,
            &mut trust,
        )
        .await
        {
            Ok(()) => break,
            Err((kind, reason, retryable)) => {
                if closed.load(Ordering::Relaxed) {
                    break;
                }
                if !auto_retry_allowed(kind, retryable, attempt) {
                    push_state(&status, &events, RemoteState::Failed, Some(reason));
                    failure_reported = true;
                    break;
                }
                attempt += 1;
                let delay = reconnect_delay(attempt);
                push_state(
                    &status,
                    &events,
                    RemoteState::Reconnecting,
                    Some(format!(
                        "{} ({kind}); retrying in {}s",
                        reason,
                        delay.as_secs()
                    )),
                );
                tokio::time::sleep(delay).await;
            }
        }
    }

    closed.store(true, Ordering::Release);
    drop(input_tx);
    // A session that ran and then ended is "disconnected"; one that never got
    // through keeps the failure and its reason.
    if let Some(state) = closing_state(failure_reported) {
        push_state(&status, &events, state, None);
    }
}

/// Re-broadcast the UI's input channel onto one this function can lend out
/// across retries.
///
/// The receiver has one owner and the driver outlives every attempt, so the
/// first attempt gets a sender and later attempts reuse it — otherwise the UI's
/// events would be dropped on the floor between attempts.
fn forward_input_channel(
    input_rx: mpsc::UnboundedReceiver<RemoteInput>,
) -> (
    mpsc::UnboundedSender<RemoteInput>,
    mpsc::UnboundedReceiver<RemoteInput>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    let relay = tx.clone();
    tokio::spawn(async move {
        let mut input_rx = input_rx;
        while let Some(event) = input_rx.recv().await {
            if relay.send(event).is_err() {
                break;
            }
        }
    });
    (tx, rx)
}

/// Why an attempt ended, and whether trying again could help.
type AttemptError = (&'static str, String, bool);

/// 安全类型预检的结论(只读列表得出,不选类型、不发认证数据)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthPreflight {
    /// 服务器允许无认证(None)—— 空密码也能连。
    NoAuthNeeded,
    /// 服务器要密码(至少提供标准 VNC 认证)。
    NeedsPassword,
    /// 服务器只提供我们说不了的类型(VeNCrypt / TLS / Tight…)。
    Unsupported,
}

/// 安全类型列表 → 结论。有 `None` 就当作无需认证。
fn classify_types(types: &[u8]) -> AuthPreflight {
    if types.contains(&1) {
        AuthPreflight::NoAuthNeeded
    } else if types.contains(&2) {
        AuthPreflight::NeedsPassword
    } else {
        AuthPreflight::Unsupported
    }
}

/// 只读探测:TCP 连接 → 读服务器版本 → 回自己的版本 → 读安全类型列表 → 断开。
///
/// **全程不发安全类型选择、不发认证数据**,所以服务器不会把它计为失败握手 ——
/// 这正是它存在的理由:库在咨询认证回调**之前**就把选中的安全类型发出去了,
/// 所以"空密码 → 回调报错"那条路径仍会让服务器看到一次未完成的握手,TigerVNC
/// 会把它计入失败并封禁地址(用户实测:反复重试之后连类型列表都拿不到)。
/// 版本差异:RFB 3.3 的 security-type 是 u32(0=拒绝+原因串 / 1=None / 2=VncAuth),
/// 3.7/3.8 才是 "数量 + 类型字节"。
async fn preflight_security(config: &VncConnectConfig) -> Result<AuthPreflight, AttemptError> {
    /// 探测主体:Ok(Ok(types)) = 拿到类型列表;Ok(Err(reason)) = 服务器拒绝并给了原因。
    async fn probe(s: &mut TcpStream) -> std::io::Result<Result<Vec<u8>, String>> {
        let mut ver = [0u8; 12];
        s.read_exact(&mut ver).await?;
        s.write_all(b"RFB 003.008\n").await?;

        let text = String::from_utf8_lossy(&ver).into_owned();
        let minor: u32 = text
            .trim()
            .rsplit('.')
            .next()
            .and_then(|m| m.parse().ok())
            .unwrap_or(8);

        async fn read_reason(s: &mut TcpStream) -> std::io::Result<String> {
            let mut raw = [0u8; 4];
            s.read_exact(&mut raw).await?;
            let n = (u32::from_be_bytes(raw) as usize).min(512);
            let mut buf = vec![0u8; n];
            if n > 0 {
                s.read_exact(&mut buf).await?;
            }
            Ok(String::from_utf8_lossy(&buf).into_owned())
        }

        if minor <= 3 {
            let mut raw = [0u8; 4];
            s.read_exact(&mut raw).await?;
            let t = u32::from_be_bytes(raw);
            if t == 0 {
                return Ok(Err(read_reason(s).await?));
            }
            return Ok(Ok(vec![t as u8]));
        }

        let mut count = [0u8; 1];
        s.read_exact(&mut count).await?;
        if count[0] == 0 {
            return Ok(Err(read_reason(s).await?));
        }
        let mut types = vec![0u8; count[0] as usize];
        s.read_exact(&mut types).await?;
        Ok(Ok(types))
    }

    let mut stream = match tokio::time::timeout(
        CONNECT_TIMEOUT,
        TcpStream::connect((config.host.as_str(), config.port)),
    )
    .await
    {
        Err(_) => return Err(("transport", "VNC connect timed out".into(), true)),
        Ok(Err(err)) => {
            let retryable = err.kind() != std::io::ErrorKind::ConnectionRefused;
            return Err(("transport", format!("VNC TCP connect failed: {err}"), retryable));
        }
        Ok(Ok(s)) => s,
    };

    match tokio::time::timeout(HANDSHAKE_TIMEOUT, probe(&mut stream)).await {
        Err(_) => Err((
            "handshake",
            crate::i18n::t(
                "读取服务器安全类型超时(对端没有回应 RFB 版本协商)",
                "timed out reading the server's security types (no answer to the RFB version exchange)",
            )
            .to_string(),
            true,
        )),
        Ok(Err(io)) => {
            // (#vnc-probe-dropped 2026-11-17) 只读探测(没选类型、没发凭据)都被断开:
            // 服务端是在"认证方式"这一步之前就拒绝本机,和密码无关。不自动重试 ——
            // 若原因是防爆破封禁,重试只会延长时间;用户可手动点「重新连接」。
            let err = vnc::VncError::IoError(io);
            if is_dropped_handshake(&err) {
                Err((
                    "handshake",
                    crate::i18n::t(
                        "连只读探测都被断开:服务器还没问认证方式就关闭了连接。通常是服务端已暂时拒绝本机地址(防爆破封禁),或该 VNC 服务不可用 —— 请稍候几分钟或在对端重启 VNC 服务,再手动点「重新连接」",
                        "even a read-only probe was dropped: the server closed the connection before asking for any authentication. Usually this address is temporarily blocked by the server's anti-bruteforce policy, or the VNC service is unusable — wait a few minutes or restart the VNC server on the remote host, then press Reconnect",
                    )
                    .to_string(),
                    false,
                ))
            } else {
                Err(handshake_failure(&err))
            }
        }
        // 服务器明确拒绝(例如 "Too many security failures"):复用同一条原因串映射
        Ok(Ok(Err(reason))) => Err(handshake_failure(&vnc::VncError::General(reason))),
        Ok(Ok(Ok(types))) => Ok(classify_types(&types)),
    }
}

/// One connection attempt, from TCP connect to session end.
async fn run_once(
    config: &VncConnectConfig,
    status: &Arc<RemoteStatus>,
    closed: &Arc<AtomicBool>,
    events: &mpsc::UnboundedSender<RemoteEvent>,
    input_tx: &mpsc::UnboundedSender<RemoteInput>,
    input_rx: &mut mpsc::UnboundedReceiver<RemoteInput>,
    trust: &mut mpsc::UnboundedReceiver<(u64, bool, bool)>,
) -> Result<(), AttemptError> {
    push_state(status, events, RemoteState::Connecting, None);

    // (#vnc-no-burn 2026-11-17) 没有密码时,先用一条**只读**探测连接问清服务器要
    // 什么认证,再决定是否值得发起真正的会话 —— 否则"选了 VncAuth 再断开"会被
    // TigerVNC 这类服务器计为一次失败握手,反复重试就把地址封了。
    if !config.has_password {
        match preflight_security(config).await? {
            AuthPreflight::NoAuthNeeded => {}
            AuthPreflight::NeedsPassword => {
                return Err((
                    "auth",
                    crate::i18n::t(
                        "该服务器要求 VNC 密码,但会话里没有填写。请在会话中填入 VNC 密码后重试(未发起握手,服务器不会记失败)",
                        "this server requires a VNC password but the session has none. Fill in the VNC password and retry (no handshake was started, so the server records no failure)",
                    )
                    .to_string(),
                    false,
                ));
            }
            AuthPreflight::Unsupported => {
                return Err(("handshake", unsupported_types_message(), false));
            }
        }
    }

    let tcp = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((config.host.as_str(), config.port)))
        .await
    {
        Err(_) => return Err(("transport", "VNC connect timed out".into(), true)),
        // (#vnc-refused-is-final 2026-10-13) A refused connection is the host
        // saying "nothing listens on this port" — retrying cannot change that,
        // and the retry loop would hold the reason back behind a "reconnecting"
        // spinner for ~7 seconds. That is exactly how "the VNC service is not
        // running" stayed invisible: the user saw 重连中… and no cause. Every
        // other transport error (timeouts, resets, unreachable) still retries.
        Ok(Err(err)) => {
            let retryable = err.kind() != std::io::ErrorKind::ConnectionRefused;
            return Err(("transport", format!("VNC TCP connect failed: {err}"), retryable));
        }
        Ok(Ok(stream)) => stream,
    };
    let _ = tcp.set_nodelay(true);
    // RFB 是单条双向流,库独占它(内部自带网络线程 + 解码线程);增量刷新请求
    // 经 client.input(X11Event::Refresh) 走同一条流。旧实现把同一个 socket
    // 注册两次进 tokio IO 驱动再手写字节,纯风险无收益(本机实测可用,已废)。

    push_state(status, events, RemoteState::Authenticating, None);

    let password = config.password.clone();
    let has_password = config.has_password;
    let connector = VncConnector::new(tcp)
        // (#vnc-blank-password 2026-11-17) 服务器要认证而我们没有密码时,**直接以
        // NoPassword 中止**,不发送任何伪造响应。旧写法无论有没有密码都返回
        // Ok(password),于是空密码会话会发出 8 个零字节的 DES 响应 —— 服务器
        // 每次都记一次失败,几次之后就用原因串 "Too many security failures" 把
        // 这个地址暂时封掉(用户实测症状)。
        // 注:回调在库读 16 字节挑战之前被 poll,但库**已经先发了选中的安全类型**,
        // 所以这条路径本身仍算一次未完成的握手 —— 因此没有密码时改用
        // `preflight_security()` 在选类型之前就拦下(见 run_once 开头)。
        .set_auth_method(async move {
            if !has_password {
                return Err(vnc::VncError::NoPassword);
            }
            Ok(password)
        })
        .set_pixel_format(vnc::PixelFormat::bgra())
        .add_encoding(requested_encodings()[0])
        .add_encoding(requested_encodings()[1])
        .allow_shared(config.shared)
        .build()
        .map_err(|err| ("protocol", format!("{err}"), false))?;

    let client = match tokio::time::timeout(HANDSHAKE_TIMEOUT, connector.try_start())
        .await
    {
        Err(_) => return Err(("transport", "VNC handshake timed out".into(), true)),
        Ok(Err(err)) => return Err(classify_attempt(&err, has_password)),
        Ok(Ok(handshake)) => handshake
            .finish()
            .map_err(|err| ("protocol", format!("{err}"), true))?,
    };

    // No password configured on a server that demanded one: say so plainly
    // rather than failing with the library's generic auth error.
    if !has_password {
        tracing::debug!("VNC session has no stored password; relying on the server's own policy");
    }

    let mut framebuffer: Option<VncFramebuffer> = None;
    let mut dirty: Option<Rect> = None;
    let mut last_request = tokio::time::Instant::now() - UPDATE_REQUEST_INTERVAL;
    let mut last_emit = tokio::time::Instant::now() - UPDATE_REQUEST_INTERVAL;
    let mut frames = FrameQueue::new();
    let mut held_keys: Vec<u32> = Vec::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(8));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        if closed.load(Ordering::Relaxed) {
            release_keys(&client, &mut held_keys).await;
            let _ = client.close().await;
            return Ok(());
        }

        tokio::select! {
            event = client.poll_event() => {
                match event {
                    Ok(Some(ev)) => {
                        if let Err(err) = handle_event(ev, &mut framebuffer, &mut dirty, status, events, config).await {
                            release_keys(&client, &mut held_keys).await;
                            let _ = client.close().await;
                            return Err(err);
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        release_keys(&client, &mut held_keys).await;
                        let _ = client.close().await;
                        return Err(("transport", format!("{err}"), is_io_error(&err)));
                    }
                }
            }

            input = input_rx.recv() => {
                let Some(event) = input else { break };
                send_input(&client, event, &mut held_keys, config).await;
            }

            // Nothing to read and nothing to send: this is where the periodic
            // update request and the frame hand-over happen.
            _ = ticker.tick() => {
                if framebuffer.is_some() {
                    if last_request.elapsed() >= UPDATE_REQUEST_INTERVAL {
                        last_request = tokio::time::Instant::now();
                        // 库把 Refresh 映射为「全桌面 + incremental」的
                        // FramebufferUpdateRequest(尺寸由库自己跟踪)。
                        let _ = client.input(vnc::X11Event::Refresh).await;
                    }
                }
                if dirty.take().is_some() {
                    if last_emit.elapsed() >= UPDATE_REQUEST_INTERVAL {
                        if let Some(fb) = framebuffer.as_ref() {
                            frames.push_frame(RemoteEvent::Frame(crate::remote::render::to_buffer_exact(&fb.to_rgba())));
                            last_emit = tokio::time::Instant::now();
                        }
                    }
                }
                for event in frames.drain() {
                    if events.send(event).is_err() {
                        closed.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
    }
    release_keys(&client, &mut held_keys).await;
    let _ = client.close().await;
    Ok(())
}

/// Fold one RFB event into the framebuffer and the dirty rectangle.
async fn handle_event(
    event: VncEvent,
    framebuffer: &mut Option<VncFramebuffer>,
    dirty: &mut Option<Rect>,
    status: &Arc<RemoteStatus>,
    events: &mpsc::UnboundedSender<RemoteEvent>,
    config: &VncConnectConfig,
) -> Result<(), AttemptError> {
    match event {
        VncEvent::SetResolution(screen) => {
            // 只建帧缓冲,**不置 dirty**:库自己已经发了首个全帧请求,像素回来前
            // 若被 8ms tick 抢先,会把一张全黑帧交给 UI(首帧闪黑)。首帧只应由
            // 真正的像素更新触发。后续服务端 resize 后也会自带全量更新。
            let fb = VncFramebuffer::new(screen.width, screen.height)
                .map_err(|err| ("protocol", err, false))?;
            *framebuffer = Some(fb);
        }
        VncEvent::RawImage(rect, data) => {
            let fb = framebuffer.get_or_insert_with(|| {
                // Some servers send an image before announcing the geometry;
                // growing to fit the rectangle is better than dropping the
                // first frame entirely.
                let w = rect.x.saturating_add(rect.width);
                let h = rect.y.saturating_add(rect.height);
                // A 1x1 framebuffer is the degenerate fallback: it satisfies the
                // non-zero invariant and the next SetResolution replaces it.
                VncFramebuffer::new(w.max(1), h.max(1))
                    .unwrap_or_else(|_| VncFramebuffer::new(1, 1).expect("1x1 framebuffer is always valid"))
            });
            let bpp = 4; // the client requested PixelFormat::bgra()
            fb.blit_raw(
                Rect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                },
                &data,
                bpp,
            )
            .map_err(|err| ("protocol", err, false))?;
            *dirty = Some(union(*dirty, to_rect(rect)));
            if !matches!(status.state(), RemoteState::Connected) {
                push_state(status, events, RemoteState::Connected, None);
            }
        }
        VncEvent::Copy(dst, src) => {
            if let Some(fb) = framebuffer.as_mut() {
                fb.copy_rect(to_rect(dst), to_rect(src))
                    .map_err(|err| ("protocol", err, false))?;
                *dirty = Some(union(*dirty, to_rect(dst)));
            }
        }
        VncEvent::JpegImage(_rect, data) => {
            // Tight-encoded JPEG. Decoding it would need a second image pipeline
            // and it only reaches here when the server prefers Tight over the
            // raw encodings we asked for first; the raw path keeps the frame
            // correct, just less efficiently.
            tracing::debug!("ignoring a Tight JPEG update of {} bytes", data.len());
        }
        VncEvent::Text(text) => {
            if config.clipboard {
                let _ = events.send(RemoteEvent::Clipboard(text));
            }
        }
        VncEvent::SetCursor(_, _) => {}
        VncEvent::Bell => {}
        VncEvent::SetPixelFormat(_) => {
            // The client sets the format explicitly, so the server has nothing to
            // tell us; a notification here means it overrode us anyway.
            tracing::debug!("server overrode the requested VNC pixel format");
        }
        VncEvent::Error(message) => {
            return Err(("protocol", message, false));
        }
        other => {
            tracing::debug!("unhandled VNC event: {other:?}");
        }
    }
    Ok(())
}

/// Send one input event, tracking held keys so they can be released on blur.
async fn send_input(
    client: &VncClient,
    event: RemoteInput,
    held_keys: &mut Vec<u32>,
    config: &VncConnectConfig,
) {
    // View-only is enforced at the sender as well as the view: a stale UI state
    // must not turn into a click on someone else's machine.
    if config.view_only && !matches!(event, RemoteInput::ReleaseAll | RemoteInput::Resize { .. }) {
        return;
    }
    match event {
        RemoteInput::Key { code, down } => {
            if down {
                if !held_keys.contains(&code) {
                    held_keys.push(code);
                }
            } else {
                held_keys.retain(|k| *k != code);
            }
            let _ = client
                .input(vnc::X11Event::KeyEvent(ClientKeyEvent { keycode: code, down }))
                .await;
        }
        RemoteInput::Text { ch } => {
            // X11 keysyms for Latin-1 are the character code; anything else has
            // no keysym this protocol can express.
            if let Some(keysym) = crate::rdp::embedded::input::latin1_keysym(ch) {
                let _ = client
                    .input(vnc::X11Event::KeyEvent(ClientKeyEvent {
                        keycode: keysym,
                        down: true,
                    }))
                    .await;
                let _ = client
                    .input(vnc::X11Event::KeyEvent(ClientKeyEvent {
                        keycode: keysym,
                        down: false,
                    }))
                    .await;
            }
        }
        RemoteInput::Pointer { x, y, mask } => {
            let _ = client
                .input(vnc::X11Event::PointerEvent(ClientMouseEvent {
                    position_x: x,
                    position_y: y,
                    bottons: mask,
                }))
                .await;
        }
        RemoteInput::ReleaseAll => release_keys(client, held_keys).await,
        // VNC servers pick their own resolution; a mid-session resize needs the
        // DesktopSize pseudo-encoding, which this client does not negotiate.
        // vnc-rs 0.6 支持 ExtendedDesktopSize 协商(resize_desktop);仅当会话
        // 跟随标签页尺寸时请求,服务器不支持时忽略(桌面保持原尺寸)。
        RemoteInput::Resize { width, height } => {
            if config.follow_tab_size {
                let _ = client.resize_desktop(width, height).await;
            }
        }
        RemoteInput::Wheel { x, y, dx, dy } => {
            // RFB expresses the wheel as button 4/5 presses (and 6/7 for the
            // horizontal axis), not a delta. 单位/方向统一走 `wheel_notches`
            // (#rdp-wheel-sign 2026-11-17):Slint 给的是像素(一格 60)、正 = 上,
            // 旧实现除 120 会让触摸板的半格增量变成 0(完全不动)。
            let notches = i32::from(crate::rdp::embedded::wheel_notches(dy));
            let h_notches = i32::from(crate::rdp::embedded::wheel_notches(dx));
            for _ in 0..notches.abs().min(20) {
                let button = if notches > 0 { 4 } else { 5 };
                let _ = client
                    .input(vnc::X11Event::PointerEvent(ClientMouseEvent {
                        position_x: x,
                        position_y: y,
                        bottons: button,
                    }))
                    .await;
            }
            for _ in 0..h_notches.abs().min(20) {
                let button = if h_notches > 0 { 7 } else { 6 };
                let _ = client
                    .input(vnc::X11Event::PointerEvent(ClientMouseEvent {
                        position_x: x,
                        position_y: y,
                        bottons: button,
                    }))
                    .await;
            }
        }
    }
}

/// Release every key the client still holds down.
async fn release_keys(client: &VncClient, held_keys: &mut Vec<u32>) {
    for keysym in held_keys.drain(..) {
        let _ = client
            .input(vnc::X11Event::KeyEvent(ClientKeyEvent {
                keycode: keysym,
                down: false,
            }))
            .await;
    }
}

fn to_rect(rect: vnc::Rect) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// Smallest rectangle covering both inputs, so several updates in one tick
/// become one frame instead of several.
fn union(a: Option<Rect>, b: Rect) -> Rect {
    let Some(a) = a else {
        return b;
    };
    let x1 = a.x.min(b.x);
    let y1 = a.y.min(b.y);
    let x2 = (a.x.saturating_add(a.width)).max(b.x.saturating_add(b.width));
    let y2 = (a.y.saturating_add(a.height)).max(b.y.saturating_add(b.height));
    Rect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1),
        height: y2.saturating_sub(y1),
    }
}

/// Whether an error is worth retrying. Auth failures are not: retrying a wrong
/// password three times only locks accounts.
fn is_auth_error(err: &vnc::VncError) -> bool {
    matches!(
        err,
        vnc::VncError::NoPassword | vnc::VncError::WrongPassword
    )
}

fn is_io_error(err: &vnc::VncError) -> bool {
    matches!(err, vnc::VncError::IoError(_))
}

/// Read a handshake failure: what to call it, what to tell the user, and
/// whether another attempt could help.
///
/// Two things are easy to get wrong at the call site.
///
/// **The sense of the retry flag.** `is_auth_error` answers "was this a
/// credential problem", so a rejected password must map to *not* retried, not
/// to retried — three rejected passwords walk an account towards a lockout,
/// which is exactly what the predicate exists to prevent.
///
/// **The wording.** A server that has decided not to serve this client closes
/// the socket part-way through the RFB security handshake. The library reports
/// that as `IoError(UnexpectedEof)`, whose `Display` is tokio's
/// "failed to fill whole buffer" — a sentence that names neither the stage nor
/// anything the user could change. That case gets a message about the stage
/// instead. It stays retryable: a socket dropped by a proxy or an overloaded
/// server looks identical from here, and only a repeat tells the two apart.
/// 是否允许再自动重试一次。
///
/// **握手阶段(已经进入 RFB 版本/安全协商)最多重试一次**(#vnc-retry-cap 2026-11-17):
/// VNC 服务端会把重复的失败握手计入封禁 —— TigerVNC 数够次数后就用
/// "Too many security failures" 或直接关闭连接拒绝该地址,越试封得越久(用户实测:
/// 界面一直闪烁正是因为自动重试在反复握手)。判断"代理抖一下"还是"服务器在拒绝"
/// 的代价不该由用户的封禁状态来付:UI 上就有「重新连接」按钮。
/// 传输类失败(TCP 连不上/超时/重置)照旧按 `MAX_RECONNECT_ATTEMPTS` 退避重试。
fn auto_retry_allowed(kind: &str, retryable: bool, attempt: u32) -> bool {
    if !retryable || attempt >= MAX_RECONNECT_ATTEMPTS {
        return false;
    }
    if kind == "handshake" && attempt >= 1 {
        return false;
    }
    true
}

/// 本驱动请求的编码,顺序即优先级。
///
/// **ZRLE + CopyRect + Raw**(#vnc-encodings 2026-11-17):ZRLE 是带宽最优的通用
/// 编码(RFC 6143 §7.7.6:矩形以 4 字节长度开头 + zlib 流),Raw 兜底保证任何
/// 服务器都能出画。曾一度只请求 Raw —— 当时误判库的 ZRLE 解码坏了(把 §7.7.4
/// 的 Hextile 当成了 ZRLE),实测合规 ZRLE 流可正常解出,故恢复。
/// **Tight 仍不请求**:JPEG 子编码库不解,服务器一选照片区域整帧就废。
///
/// (以下为当时的误判记录,保留以免重犯)
/// (旧)**只有 Raw 与 CopyRect**(#vnc-connect-fix 2026-11-17):
pub(crate) fn requested_encodings() -> [VncEncoding; 3] {
    [VncEncoding::Zrle, VncEncoding::CopyRect, VncEncoding::Raw]
}

/// 认证失败的**可行动**文案:库给的是英文且只说"是什么",用户需要知道"怎么办"。
fn auth_failure_message(err: &vnc::VncError) -> String {
    match err {
        vnc::VncError::NoPassword => crate::i18n::t(
            "该服务器要求 VNC 密码,但会话里没有填写。请在会话中填入 VNC 密码后重试",
            "this server requires a VNC password but the session has none. Fill in the VNC password and retry",
        )
        .to_string(),
        vnc::VncError::WrongPassword => crate::i18n::t(
            "VNC 密码不正确(服务器拒绝了认证)",
            "wrong VNC password (the server rejected the authentication)",
        )
        .to_string(),
        other => format!("{other}"),
    }
}

/// 认证方式不受支持时的文案(握手失败路径与预检共用一份)。
fn unsupported_types_message() -> String {
    crate::i18n::t(
        "服务器的认证方式内置客户端不支持(仅支持 None / VNC 认证)。请在服务端启用 VNC 密码认证,或改用下载 / 外部客户端",
        "the server's authentication types are not supported by the built-in client (None / VNC auth only). Enable VNC password auth on the server, or use download / an external viewer",
    )
    .to_string()
}

/// (#vnc-none-then-drop 2026-11-17) 把一次连接尝试的失败分类,知道会话有没有密码。
///
/// 有密码时直接交给 [`handshake_failure`]。**没有密码时**我们能多知道一件事:
/// 没有密码就必然先跑过 `preflight_security()`,而它会提前返回
/// (要求密码 / 类型不支持/探测本身失败),**能走到真正的连接就说明预检已经答过
/// "服务器广告的是 None(免认证)"**。既然如此,握手第一步就 EOF 就不是密码问题、
/// 也不是客户端问题 —— 是服务端自己把连接掐了。
///
/// 实测(2026-11-17,对端 TigerVNC):它广告 `[1]`(None),客户端一旦选定类型就
/// 立刻 FIN;选它没广告的 VncAuth 同样 FIN;3.3 的 u32 写法也一样。也就是说
/// **没有任何认证方式能走通**,与密码、协议版本、客户端实现都无关。所以这句
/// 话必须说成"服务端状态异常",并给出可执行的下一步(重启对端服务)。
fn classify_attempt(err: &vnc::VncError, has_password: bool) -> AttemptError {
    if !has_password && is_dropped_handshake(err) {
        // 不可重试:服务端已经在拒,反复重试只会喂它的防爆破计数器。
        return ("handshake", none_then_dropped_message(), false);
    }
    handshake_failure(err)
}

fn none_then_dropped_message() -> String {
    // (#failure-dedupe 2026-11-17) 这句话会**独占**失败画面(没有原始错误可展开,
    // 见 UI 的去重规则),所以它必须先是结论、再是动作,不能是三条猜测罗列 ——
    // 用户截图里的三行红字就是旧版太长的证据。
    crate::i18n::t(
        "服务端状态异常:它声明「无需认证」,却在握手第一步就断开(重启对端 VNC 服务可清除;若服务端设了密码,把密码填进本会话)",
        "the server is in a bad state: it advertises no authentication, then hangs up on the first handshake step (restart the VNC server on the remote host; if it has a password, put it in this session)",
    )
    .to_string()
}

/// Read a handshake failure: what to call it, what to tell the user, and
/// whether another attempt could help.
///
/// **The sense of the retry flag.** `is_auth_error` answers "was this a
/// credential problem", so a rejected password must map to *not* retried, not
/// to retried — three rejected passwords walk an account towards a lockout,
/// which is exactly what the predicate exists to prevent. `auto_retry_allowed`
/// then caps handshake-stage retries at one: VNC servers (TigerVNC and friends)
/// count repeated failed handshakes and block the client, so a client that
/// retries in a loop is the reason the user sees the address locked out (and a
/// flickering status line).
///
/// **The wording.** A server that has decided not to serve this client closes
/// the socket part-way through the RFB security handshake. The library reports
/// that as `IoError(UnexpectedEof)`, whose `Display` is tokio's
/// "failed to fill whole buffer" — a sentence that names neither the stage nor
/// anything the user could change. That case gets a message that names the two
/// realistic causes (a temporary block after failed attempts, or an auth type
/// we cannot speak) and what to do about them.
fn handshake_failure(err: &vnc::VncError) -> AttemptError {
    // 认证类失败一律不可重试:同样的凭据再试一次只会把服务器侧的失败计数推向
    // 封禁(VNC 的 "Too many security failures")。
    if is_auth_error(err) {
        return ("auth", auth_failure_message(err), false);
    }
    // 服务器已经把话说白了:原因串告诉我们它数满失败次数并暂时拒绝本地址。
    // 这不是"连不上",是"先别再试" —— 必须说清楚,否则用户会一直点重新连接
    // 把封禁时间越拖越长。
    if let vnc::VncError::General(msg) = err {
        let lower = msg.to_ascii_lowercase();
        if lower.contains("too many") || lower.contains("security failures") {
            return (
                "auth",
                crate::i18n::t(
                    "服务器因多次认证失败已暂时拒绝本地址(VNC 防爆破策略)。请先等待几分钟,核对 VNC 密码后再试 —— 现在反复重试会延长封禁",
                    "the server has temporarily blocked this address after too many failed authentications. Wait a few minutes, check the VNC password, then retry — retrying now extends the block",
                )
                .to_string(),
                false,
            );
        }
        if msg.contains("Security type") {
            return ("handshake", unsupported_types_message(), false);
        }
    }
    if is_dropped_handshake(err) {
        return (
            "handshake",
            crate::i18n::t(
                "服务端在 RFB 安全握手阶段关闭了连接。常见原因:①本地址因之前多次认证失败被 VNC 服务端暂时拒绝(TigerVNC 等会这样做,重启服务或等待几分钟即清除);②服务端要求的认证方式不受支持。请先确认 VNC 密码已正确填写,必要时重启服务端再试",
                "the server closed the connection during the RFB security handshake. Usually either this address is temporarily blocked after repeated failed authentications (TigerVNC does this — restart the server or wait a few minutes to clear it), or the server's authentication types are unsupported. Check the VNC password, restart the server if needed, then retry",
            )
            .to_string(),
            true,
        );
    }
    // Everything else that killed the socket is worth another attempt;
    // everything that did not is a decision the server has already made.
    ("handshake", format!("{err}"), is_io_error(err))
}

/// Whether the server hung up in the middle of the handshake instead of
/// answering it.
fn is_dropped_handshake(err: &vnc::VncError) -> bool {
    matches!(err, vnc::VncError::IoError(io) if io.kind() == std::io::ErrorKind::UnexpectedEof)
}

fn push_state(
    status: &Arc<RemoteStatus>,
    events: &mpsc::UnboundedSender<RemoteEvent>,
    state: RemoteState,
    message: Option<String>,
) {
    status.set(state, message.clone());
    let _ = events.send(RemoteEvent::State {
        state: status.clone(),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dirty_rectangle_grows_to_cover_both_updates() {
        let a = Rect {
            x: 10,
            y: 10,
            width: 10,
            height: 10,
        };
        let b = Rect {
            x: 100,
            y: 0,
            width: 5,
            height: 5,
        };
        let u = union(Some(a), b);
        assert_eq!((u.x, u.y), (10, 0));
        assert_eq!((u.width, u.height), (95, 20));
    }

    #[test]
    fn the_first_dirty_rectangle_is_kept_as_is() {
        let b = Rect {
            x: 3,
            y: 4,
            width: 5,
            height: 6,
        };
        assert_eq!(union(None, b), b);
    }

    #[test]
    fn backoff_grows_then_plateaus() {
        assert_eq!(reconnect_delay(1), Duration::from_secs(1));
        assert_eq!(reconnect_delay(3), Duration::from_secs(4));
        assert_eq!(reconnect_delay(9), Duration::from_secs(8));
    }

    #[test]
    fn an_auth_failure_is_not_retried_but_a_transport_error_is() {
        assert!(is_auth_error(&vnc::VncError::WrongPassword));
        assert!(!is_auth_error(&vnc::VncError::NoEncoding));
        assert!(is_io_error(&vnc::VncError::IoError(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "reset"
        ))));
    }

    /// The polarity the doc comment on `is_auth_error` promises: a credential
    /// problem must reach the user immediately instead of being replayed against
    /// an account that will lock out.
    #[test]
    fn a_rejected_password_is_reported_without_being_retried() {
        for err in [vnc::VncError::WrongPassword, vnc::VncError::NoPassword] {
            let (kind, _, retryable) = handshake_failure(&err);
            assert_eq!(kind, "auth");
            assert!(!retryable, "{err} must not be tried again");
        }
    }

    #[test]
    fn a_broken_connection_is_retried_and_a_refusal_is_not() {
        let reset = vnc::VncError::IoError(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "reset",
        ));
        assert!(handshake_failure(&reset).2, "a reset is worth one more try");

        // A server that only offers security types we cannot speak has said no;
        // asking again gets the same answer.
        let unsupported = vnc::VncError::General("no shared security type".into());
        assert!(!handshake_failure(&unsupported).2);
    }

    /// A server that hangs up mid-handshake must not reach the user as
    /// "failed to fill whole buffer".
    #[test]
    fn a_server_that_hangs_up_is_described_by_stage() {
        let dropped = vnc::VncError::IoError(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "failed to fill whole buffer",
        ));
        assert!(is_dropped_handshake(&dropped));
        let (kind, message, retryable) = handshake_failure(&dropped);
        assert_eq!(kind, "handshake");
        assert!(retryable);
        assert!(
            !message.contains("buffer"),
            "the library's wording leaked through: {message}"
        );
        // 文案必须给出可行动的两条线索(封禁 / 认证方式),并说明怎么办
        assert!(message.contains("握手"), "{message}");
        assert!(
            message.contains("重启") || message.contains("等待"),
            "应告诉用户如何清除封禁: {message}"
        );
        assert!(message.contains("密码"), "应提醒核对密码: {message}");
    }

    /// (#vnc-none-then-drop 2026-11-17) 没有密码 + 服务器广告 None 却在握手第一步
    /// 断开:必须说成"服务端状态异常 + 怎么办",而不是让用户去猜密码。
    ///
    /// 这条路径能成立,是因为没有密码就一定先跑过只读预检,而预检要么提前返回
    /// (要求密码 / 类型不支持),要么已经证明服务器广告的是 None —— 所以这里的
    /// EOF 一定发生在选完 None 之后。
    #[test]
    fn a_server_that_offers_none_then_hangs_up_is_called_out() {
        let dropped = vnc::VncError::IoError(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "failed to fill whole buffer",
        ));
        let (kind, message, retryable) = classify_attempt(&dropped, false);
        assert_eq!(kind, "handshake");
        assert!(
            !retryable,
            "服务端已在拒绝,自动重试只会喂它的防爆破计数器"
        );
        assert!(
            message.contains("无需认证") || message.contains("no authentication"),
            "{message}"
        );
        assert!(
            message.contains("重启"),
            "必须给出可执行的下一步(重启对端服务): {message}"
        );
        assert!(!message.contains("buffer"), "{message}");

        // 有密码时仍走原来那条(更泛化的)文案:此时我们并不知道服务器广告了什么。
        let (kind2, message2, _) = classify_attempt(&dropped, true);
        assert_eq!(kind2, "handshake");
        assert_ne!(message2, message, "没有密码才敢断言是服务端状态异常");
    }

    /// 库的通用句子必须翻成能行动的文案(不可重试,不外泄库的措辞)。
    #[test]
    fn an_unsupported_security_type_gets_an_actionable_message() {
        let err = vnc::VncError::General(
            "Security type apart from Vnc Auth has not been implemented".into(),
        );
        let (kind, message, retryable) = handshake_failure(&err);
        assert_eq!(kind, "handshake");
        assert!(!retryable, "安全类型不支持,重试无意义");
        assert!(
            message.contains("VNC") || message.contains("认证"),
            "应包含可行动的指引: {message}"
        );
        assert!(!message.contains("has not been implemented"), "{message}");
    }

    /// 服务器数满失败次数后的原因串必须被翻译成「先等待」的指引,且不可重试。
    #[test]
    fn the_too_many_security_failures_reason_is_explained() {
        let err = vnc::VncError::General("Too many security failures".into());
        let (kind, message, retryable) = handshake_failure(&err);
        assert_eq!(kind, "auth");
        assert!(!retryable, "封禁期间重试只会延长封禁");
        assert!(
            message.contains("等待") || message.contains("封"),
            "应给出「先等待」的指引: {message}"
        );
        assert!(!message.contains("Too many"), "不应把英文原话抛给用户: {message}");
    }

    /// 握手阶段最多自动重试一次:VNC 服务端把重复的失败握手计入封禁(#vnc-retry-cap)。
    #[test]
    fn handshake_stage_retries_at_most_once() {
        assert!(auto_retry_allowed("handshake", true, 0));
        assert!(!auto_retry_allowed("handshake", true, 1));
        assert!(auto_retry_allowed("transport", true, 0));
        assert!(auto_retry_allowed("transport", true, 2));
        assert!(!auto_retry_allowed("transport", true, MAX_RECONNECT_ATTEMPTS));
        assert!(!auto_retry_allowed("auth", false, 0));
        assert!(!auto_retry_allowed("handshake", false, 0));
    }

    /// 编码清单契约:ZRLE(带宽)+ Raw(兜底)必须在场;Tight 不得请求
    /// (JPEG 子编码库不解,服务器一选照片区域整帧就废)。
    #[test]
    fn the_encoding_list_keeps_its_safe_set() {
        let encs = requested_encodings();
        assert!(encs.contains(&VncEncoding::Zrle), "ZRLE 是带宽主力");
        assert!(encs.contains(&VncEncoding::Raw), "必须有 Raw 兜底");
        assert!(!encs.contains(&VncEncoding::Tight), "Tight 的 JPEG 子编码不可解");
    }

    /// 安全类型列表 → 结论:有 None 就当无需认证;只有 VncAuth 就是要密码。
    #[test]
    fn security_types_map_to_the_preflight_verdict() {
        assert_eq!(classify_types(&[1]), AuthPreflight::NoAuthNeeded);
        assert_eq!(classify_types(&[19, 2]), AuthPreflight::NeedsPassword);
        assert_eq!(classify_types(&[2, 1]), AuthPreflight::NoAuthNeeded);
        assert_eq!(classify_types(&[19, 18]), AuthPreflight::Unsupported);
    }
}
