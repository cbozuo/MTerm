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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufWriter};
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
                if !retryable || attempt >= MAX_RECONNECT_ATTEMPTS {
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
    // RFB is a single bidirectional stream, but the library owns the read half
    // while the driver writes FramebufferUpdateRequest by hand. tokio's
    // `TcpStream` has no `try_clone`, so the socket round-trips through std —
    // the only way to get two independent handles onto one connection.
    let std_tcp = tcp
        .into_std()
        .map_err(|err| ("transport", format!("into_std failed: {err}"), true))?;
    let request_stream = TcpStream::from_std(
        std_tcp
            .try_clone()
            .map_err(|err| ("transport", format!("socket clone failed: {err}"), true))?,
    )
    .map_err(|err| ("transport", format!("from_std failed: {err}"), true))?;
    let tcp = TcpStream::from_std(std_tcp)
        .map_err(|err| ("transport", format!("from_std failed: {err}"), true))?;

    push_state(status, events, RemoteState::Authenticating, None);

    let password = config.password.clone();
    let has_password = config.has_password;
    let connector = VncConnector::new(tcp)
        // The future is only polled when the server actually asks for
        // authentication, so an empty password is never sent unprompted.
        .set_auth_method(async move { Ok(password) })
        .set_pixel_format(vnc::PixelFormat::bgra())
        .add_encoding(VncEncoding::Zrle)
        .add_encoding(VncEncoding::Tight)
        .add_encoding(VncEncoding::Raw)
        .add_encoding(VncEncoding::CopyRect)
        .allow_shared(config.shared)
        .build()
        .map_err(|err| ("protocol", format!("{err}"), false))?;

    let client = match tokio::time::timeout(HANDSHAKE_TIMEOUT, connector.try_start())
        .await
    {
        Err(_) => return Err(("transport", "VNC handshake timed out".into(), true)),
        Ok(Err(err)) => return Err(handshake_failure(&err)),
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
    let mut request_writer = BufWriter::new(request_stream);
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
            // (#vnc-starve 2026-10-27) This used to be `poll_event`, and that
            // choice is why VNC never connected. `poll_event` is a `try_recv`:
            // with no event queued it resolves *immediately* with `Ok(None)`,
            // so the `select!` always had a ready branch and this loop never
            // yielded. Every vnc-rs worker — the network reader and the frame
            // decoder — is `tokio::spawn`ed onto **this** driver's dedicated
            // current-thread runtime, so they were starved the moment the
            // session loop started spinning: the handshake completed, and then
            // no frame, no state change and no input ever happened again. The
            // tab hung on 连接中/正在认证 for as long as it lived. `recv_event`
            // is the blocking variant (an honest `Receiver::recv`), so the
            // select parks until the runtime's own tasks produce an event.
            event = client.recv_event() => {
                match event {
                    Ok(ev) => {
                        if let Err(err) = handle_event(ev, &mut framebuffer, &mut dirty, status, events, config).await {
                            release_keys(&client, &mut held_keys).await;
                            let _ = client.close().await;
                            return Err(err);
                        }
                    }
                    // `ClientNotRunning` is what the library reports once the
                    // client is stopped: either we closed it below (the `closed`
                    // flag wins, and ending here is the graceful path) or the
                    // internal reader died on a dead socket — a transport
                    // failure worth one retry.
                    Err(err) if matches!(err, vnc::VncError::ClientNotRunning)
                        && closed.load(Ordering::Relaxed) => {}
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
                if let Some(fb) = framebuffer.as_ref() {
                    if last_request.elapsed() >= UPDATE_REQUEST_INTERVAL {
                        let mut msg = [0u8; 10];
                        msg[0] = 3; // FramebufferUpdateRequest
                        msg[1] = 1; // incremental
                        msg[8] = (fb.width >> 8) as u8;
                        msg[9] = fb.width as u8;
                        msg[6] = (fb.height >> 8) as u8;
                        msg[7] = fb.height as u8;
                        if request_writer.write_all(&msg).await.is_err() {
                            return Err(("transport", "sending FramebufferUpdateRequest failed".into(), true));
                        }
                        let _ = request_writer.flush().await;
                        last_request = tokio::time::Instant::now();
                    }
                }
                if let Some(rect) = dirty.take() {
                    if last_emit.elapsed() >= UPDATE_REQUEST_INTERVAL {
                        if let Some(fb) = framebuffer.as_ref() {
                            frames.push_frame(RemoteEvent::Frame(fb.to_rgba()));
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
            let fb = VncFramebuffer::new(screen.width, screen.height)
                .map_err(|err| ("protocol", err, false))?;
            *framebuffer = Some(fb);
            *dirty = Some(Rect {
                x: 0,
                y: 0,
                width: screen.width,
                height: screen.height,
            });
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
        RemoteInput::Resize { .. } => {}
        RemoteInput::Wheel { x, y, dx, dy } => {
            // RFB expresses the wheel as button 4/5 presses (and 6/7 for the
            // horizontal axis), not a delta.
            let notches = (dy / 120.0).round() as i32;
            let h_notches = (dx / 120.0).round() as i32;
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
fn handshake_failure(err: &vnc::VncError) -> AttemptError {
    if is_auth_error(err) {
        return ("auth", format!("{err}"), false);
    }
    if is_dropped_handshake(err) {
        return (
            "handshake",
            crate::i18n::t(
                "服务端在 RFB 安全握手阶段关闭了连接",
                "the server closed the connection during the RFB security handshake",
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
    }
}

/// End-to-end handshake tests against a mock RFB server.
///
/// The mock speaks just enough RFB 3.8 to satisfy vnc-rs — version exchange,
/// security-type negotiation, [`MockAuth`], ServerInit — and then answers every
/// framebuffer update request with one full-width raw rect. That is enough to
/// drive `run_with_input` from TCP connect through to a delivered frame, which
/// is exactly the stretch of the pipeline that no unit test covers.
///
/// These exist because of #vnc-starve: the session loop used `poll_event`
/// (a `try_recv`), which resolved instantly on every turn and starved the
/// vnc-rs worker tasks that live on the same current-thread runtime — the
/// handshake passed, then nothing ever happened again. Only a real, yielding
/// event loop over a real socket can catch that class of bug.
#[cfg(test)]
mod mock_server_tests {
    use super::*;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;

    const WIDTH: u16 = 64;
    const HEIGHT: u16 = 48;

    /// What the mock server offers at the security stage.
    enum MockAuth {
        /// Security type 1, no challenge.
        None,
        /// Security type 2: challenge, accept whatever comes back.
        VncAuth,
        /// Only types vnc-rs cannot speak (RA2=5, Tight-security=16): the
        /// handshake must fail with the library's own reason and no retry.
        Unsupported,
    }

    /// Server side of one RFB session. Panics inside the spawned task are
    /// surfaced as a disconnect; the assertions on the driver's side are what
    /// actually judge the exchange.
    async fn serve(mut stream: tokio::net::TcpStream, auth: MockAuth) {
        // Version handshake: we offer 3.8, the client echoes its own.
        stream.write_all(b"RFB 003.008\n").await.expect("write version");
        let mut version = [0u8; 12];
        stream.read_exact(&mut version).await.expect("read version");

        match auth {
            MockAuth::None => {
                stream.write_all(&[1, 1]).await.expect("write security types");
                let mut chosen = [0u8; 1];
                stream.read_exact(&mut chosen).await.expect("read security choice");
                assert_eq!(chosen[0], 1, "the client must pick None here");
                // RFB 3.8 sends a SecurityResult even for "no auth".
                stream.write_all(&0u32.to_be_bytes()).await.expect("write result");
            }
            MockAuth::VncAuth => {
                stream.write_all(&[1, 2]).await.expect("write security types");
                let mut chosen = [0u8; 1];
                stream.read_exact(&mut chosen).await.expect("read security choice");
                assert_eq!(chosen[0], 2, "the client must pick VncAuth here");
                stream.write_all(&[7u8; 16]).await.expect("write challenge");
                let mut response = [0u8; 16];
                stream.read_exact(&mut response).await.expect("read response");
                // Not verifying the DES answer: vnc-rs owns the cipher, and the
                // contract under test is the *flow*, not the math.
                stream.write_all(&0u32.to_be_bytes()).await.expect("write result");
            }
            MockAuth::Unsupported => {
                stream.write_all(&[2, 5, 16]).await.expect("write security types");
                // vnc-rs refuses before sending a choice; dropping the socket
                // here mirrors a real server walking away.
                return;
            }
        }

        // ClientInit (shared flag), then ServerInit.
        let mut shared = [0u8; 1];
        stream.read_exact(&mut shared).await.expect("read shared flag");
        let mut init = Vec::new();
        init.extend_from_slice(&WIDTH.to_be_bytes());
        init.extend_from_slice(&HEIGHT.to_be_bytes());
        // The pixel format MTerm requests (PixelFormat::bgra): 32bpp, depth 24,
        // little-endian, true-colour, full 8-bit channels, r:g:b = 16:8:0.
        init.extend_from_slice(&[32, 24, 0, 1]);
        init.extend_from_slice(&255u16.to_be_bytes());
        init.extend_from_slice(&255u16.to_be_bytes());
        init.extend_from_slice(&255u16.to_be_bytes());
        init.extend_from_slice(&[16, 8, 0, 0, 0, 0]);
        init.extend_from_slice(&4u32.to_be_bytes());
        init.extend_from_slice(b"mock");
        stream.write_all(&init).await.expect("write ServerInit");

        // Client → SetPixelFormat (1B type + 3B pad + 16B format),
        // SetEncodings (4B + 4B per encoding), then the first
        // FramebufferUpdateRequest (10B).
        let mut pixel_format = [0u8; 20];
        stream.read_exact(&mut pixel_format).await.expect("read SetPixelFormat");
        assert_eq!(pixel_format[0], 0, "SetPixelFormat message type");
        let mut head = [0u8; 4];
        stream.read_exact(&mut head).await.expect("read SetEncodings head");
        assert_eq!(head[0], 2, "SetEncodings message type");
        let encoding_count = u16::from_be_bytes([head[2], head[3]]) as usize;
        let mut encodings = vec![0u8; encoding_count * 4];
        stream.read_exact(&mut encodings).await.expect("read encoding list");
        let mut request = [0u8; 10];
        stream.read_exact(&mut request).await.expect("read first update request");
        assert_eq!(request[0], 3, "FramebufferUpdateRequest message type");

        // Serve one full-frame raw rect per update request until the client
        // stops asking (test over) or the socket dies.
        loop {
            let mut update = Vec::new();
            update.extend_from_slice(&[0, 0]); // FramebufferUpdate + padding
            update.extend_from_slice(&1u16.to_be_bytes()); // one rect
            update.extend_from_slice(&0u16.to_be_bytes()); // x
            update.extend_from_slice(&0u16.to_be_bytes()); // y
            update.extend_from_slice(&WIDTH.to_be_bytes());
            update.extend_from_slice(&HEIGHT.to_be_bytes());
            update.extend_from_slice(&0i32.to_be_bytes()); // Raw
            update.extend(vec![0xA5u8; WIDTH as usize * HEIGHT as usize * 4]);
            if stream.write_all(&update).await.is_err() {
                return;
            }
            if stream.read_exact(&mut request).await.is_ok() && request[0] == 3 {
                continue;
            }
            return;
        }
    }

    /// One driven session: the mock server is already accepting.
    struct Driven {
        events: mpsc::UnboundedReceiver<RemoteEvent>,
        _input: mpsc::UnboundedSender<RemoteInput>,
        closed: Arc<AtomicBool>,
    }

    async fn drive(auth: MockAuth) -> Driven {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                serve(stream, auth).await;
            }
        });

        let config = VncConnectConfig {
            tab_id: "tab-1".to_string(),
            session_id: "session-1".to_string(),
            host: "127.0.0.1".to_string(),
            port,
            password: "secret".to_string(),
            has_password: true,
            security: String::new(),
            view_only: false,
            shared: true,
            clipboard: false,
            follow_tab_size: false,
            width: 1280,
            height: 720,
        };
        let status = Arc::new(RemoteStatus::new(RemoteState::Connecting));
        let closed = Arc::new(AtomicBool::new(false));
        let (events_tx, events) = mpsc::unbounded_channel::<RemoteEvent>();
        let (_trust_tx, trust) = mpsc::unbounded_channel();
        // The input sender is held for the session's life, exactly as the real
        // handle does: dropping it would end the session at the first select.
        let (input_tx, input_rx) = mpsc::unbounded_channel::<RemoteInput>();
        let deps = DriverDeps {
            status,
            closed: closed.clone(),
            events: events_tx,
            trust,
        };
        tokio::spawn(run_with_input(config, deps, input_rx));
        Driven {
            events,
            _input: input_tx,
            closed,
        }
    }

    /// Collect events until `predicate` accepts one, bounded so a starved or
    /// deadlocked loop fails the test instead of hanging it.
    async fn wait_for(
        driven: &mut Driven,
        what: &'static str,
        mut predicate: impl FnMut(&RemoteEvent) -> bool,
    ) -> RemoteEvent {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let event = tokio::time::timeout(remaining, driven.events.recv())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
                .expect("the driver's event sender stays alive for the session");
            if predicate(&event) {
                return event;
            }
        }
    }

    #[tokio::test]
    async fn a_vnc_auth_server_connects_and_delivers_frames() {
        let mut driven = drive(MockAuth::VncAuth).await;
        wait_for(&mut driven, "the Connected state", |event| {
            matches!(event, RemoteEvent::State { state, .. } if state.state() == RemoteState::Connected)
        })
        .await;
        let frame = wait_for(&mut driven, "a frame", |event| {
            matches!(event, RemoteEvent::Frame(_))
        })
        .await;
        let RemoteEvent::Frame(image) = frame else {
            unreachable!("the predicate matched a Frame");
        };
        assert_eq!(image.dimensions(), (u32::from(WIDTH), u32::from(HEIGHT)));
        // The mock fills every source byte with 0xA5; the framebuffer's
        // `to_rgba` re-forces the alpha channel opaque, so each pixel comes
        // back as [0xA5, 0xA5, 0xA5, 0xFF].
        assert!(
            image
                .as_raw()
                .chunks_exact(4)
                .all(|px| px == [0xA5, 0xA5, 0xA5, 0xFF]),
            "unexpected pixel content"
        );
        driven.closed.store(true, Ordering::Relaxed);
    }

    #[tokio::test]
    async fn a_no_auth_server_connects_and_delivers_frames() {
        let mut driven = drive(MockAuth::None).await;
        wait_for(&mut driven, "the Connected state", |event| {
            matches!(event, RemoteEvent::State { state, .. } if state.state() == RemoteState::Connected)
        })
        .await;
        wait_for(&mut driven, "a frame", |event| matches!(event, RemoteEvent::Frame(_))).await;
        driven.closed.store(true, Ordering::Relaxed);
    }

    #[tokio::test]
    async fn unsupported_security_types_fail_without_a_retry() {
        let mut driven = drive(MockAuth::Unsupported).await;
        let event = wait_for(&mut driven, "the Failed state", |event| {
            matches!(event, RemoteEvent::State { state, .. } if state.state() == RemoteState::Failed)
        })
        .await;
        let RemoteEvent::State { message, .. } = event else {
            unreachable!("the predicate matched a State");
        };
        let message = message.unwrap_or_default();
        assert!(
            message.contains("Security type apart from Vnc Auth"),
            "the raw reason must survive into the details text: {message}"
        );
        // A refusal the server has already made is final: no retry loop may
        // spin behind it.
        let retried = driven
            .events
            .recv()
            .await
            .is_some_and(|event| matches!(event, RemoteEvent::State { state, .. } if state.state() == RemoteState::Reconnecting));
        assert!(!retried, "an unsupported-auth refusal must not be retried");
    }
}
