//! The ironrdp session driver for MTerm's built-in RDP client.
//!
//! Structure mirrors the reference implementation's `run_session_loop`: build a
//! config, run `RdpClient`, then pump three sources — the client's output
//! events, our own input channel, and a periodic frame flush — until the session
//! ends. What differs is the transport of the results: instead of a Tauri IPC
//! channel and a JSON-encoded JPEG packet, frames become an `image::RgbaImage`
//! handed to the frame queue that feeds the tab.
//!
//! Three ironrdp 0.17 details worth writing down, because they differ from the
//! reference code and from older versions:
//!
//! * `RdpClient::run()` owns the whole connect/authenticate/active loop *and*
//!   reconnects internally, so this driver never re-creates the client.
//! * `RdpOutputEvent` carries whole frames (`Image`), not dirty-region updates,
//!   so the decoded picture is authoritative and is cloned per frame rather than
//!   re-assembled from regions.
//! * There is no certificate-verifier hook; trust is decided by a pre-flight TLS
//!   probe before the client is even built (see `impls::verifier`).
//!
//! And one detail that is ours rather than ironrdp's (#rdp-resize-drop
//! 2026-10-27): a resize may restart the whole connection, so input — including
//! the `Close` that a closing tab sends — is held while one is in flight. The
//! note above the main `loop` explains why in full.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use image::RgbaImage;
use tokio::sync::mpsc;

use ironrdp::client::config::{ConfigBuilder, Destination, TransportKind};
use ironrdp::client::rdp::{RdpClient, RdpInputEvent as IronRdpInput, RdpOutputEvent};
use ironrdp::input::{
    Database as IronRdpInputDatabase, MouseButton, MousePosition, Operation as IronRdpOperation,
    Scancode, WheelRotations,
};
use ironrdp::pdu::rdp::capability_sets::MajorPlatformType;
use ironrdp::session::{GracefulDisconnectReason, SessionResult};

use crate::rdp::embedded::config::EmbeddedRdpConfig;
use crate::rdp::embedded::input::{button, RemoteInput};
use crate::rdp::embedded::status::{closing_state, RemoteState, RemoteStatus};
use crate::remote::pump::FrameQueue;
use crate::remote::pane::{RemoteEvent, RemoteHandle, TrustRequest};

/// Don't repaint more often than this. A desktop redraws on every mouse move;
/// 60 Hz is already more than any renderer needs, and the cap is what stops a
/// busy session from saturating the UI thread.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Ticks that carry no work still let `cancel` be noticed promptly.
const INPUT_TICK: Duration = Duration::from_millis(20);

/// How long a trust prompt may stay unanswered before the attempt is abandoned.
///
/// Bounded so a tab left open overnight with a prompt nobody answered cannot
/// hold a socket open forever.
const TRUST_TIMEOUT: Duration = Duration::from_secs(180);

/// How long one desktop resize may keep the session stopped before the resize
/// is given up on (#rdp-resize-drop 2026-10-27).
///
/// A fast-path resize is a one-round-trip exchange; the reconnect path is a
/// whole connection. Anything that takes longer than this is not a slow
/// server — it is a hung one — and the tab must not stay on "重连中…" forever.
const RESIZE_WAIT: Duration = Duration::from_secs(45);

/// Everything the driver needs that is not in [`EmbeddedRdpConfig`].
pub struct DriverDeps {
    pub status: Arc<RemoteStatus>,
    pub closed: Arc<AtomicBool>,
    pub events: mpsc::UnboundedSender<RemoteEvent>,
    /// Answers to [`TrustRequest`]s, as `(request_id, accepted, remember)`.
    pub trust: mpsc::UnboundedReceiver<(u64, bool, bool)>,
    /// Current desktop size, kept in sync with the UI's idea of it.
    pub size: Arc<Mutex<(u16, u16)>>,
}

/// A desktop resize the session has been asked for and has not yet answered.
///
/// Only the size and the deadline matter; the rest of the decision lives in the
/// loop that owns the session (it needs the input sender, the applied size and
/// the caller's `dirty` flag). `Copy` so the deadline branch can inspect it at
/// the same moment the loop knows it is armed.
#[derive(Clone, Copy)]
struct InFlightResize {
    /// The size that was sent, so a timeout can give it back.
    size: (u16, u16),
    started: tokio::time::Instant,
}

/// The deadline of the resize in flight, or a future that never resolves.
///
/// A helper rather than an inline `sleep_until` so that `tokio::select!` polls
/// the *same* future on every turn: an expression inside `select!` is created
/// afresh each time, and a freshly created deadline measured from the start has
/// already expired — it would fire immediately and forever.
async fn resize_deadline(in_flight: Option<InFlightResize>) {
    match in_flight {
        Some(resize) => tokio::time::sleep_until(resize.started + RESIZE_WAIT).await,
        None => std::future::pending().await,
    }
}

/// Whether the session may be sent input right now.
///
/// False while a resize is in flight: the client drains its input channel across
/// a Deactivate-Reactivation and across a resume-at-a-new-size reconnect, so
/// anything sent in that window is executed on the far side of it. See the note
/// above the main loop.
fn accepts_input(resize_in_flight: Option<InFlightResize>) -> bool {
    resize_in_flight.is_none()
}

/// Act on one input event, moving the session's size bookkeeping with it.
///
/// Extracted from the select loop because a deferred event (held during a
/// resize) has to go through exactly the same path once the session can take it
/// again, and because the resize branch is the one place where a few pieces of
/// loop state have to move together.
#[allow(clippy::too_many_arguments)]
fn apply_input(
    deps: &DriverDeps,
    input_db: &mut IronRdpInputDatabase,
    iron_input: &mpsc::UnboundedSender<IronRdpInput>,
    dirty: &mut bool,
    size: &mut (u16, u16),
    applied_size: &mut (u16, u16),
    resize_in_flight: &mut Option<InFlightResize>,
    event: RemoteInput,
) {
    match event {
        RemoteInput::Resize { width, height } => {
            let Some((w, h)) = crate::remote::render::clamp_desktop_size(width, height) else {
                return;
            };
            // Compared against the size the session is *at*, not against the
            // last requested one: asking for the current size back after a
            // resize that timed out is a real request, not a no-op.
            if (w, h) == *applied_size || (w, h) == *size {
                return;
            }
            *size = (w, h);
            if let Ok(mut slot) = deps.size.lock() {
                *slot = *size;
            }
            // The picture stays until a new one replaces it (#rdp-resize-drop
            // 2026-10-27): the server may be about to reconnect, and a blank tab
            // in between is exactly the "frozen until reconnect" symptom. The
            // outgoing frame is still worth flushing, so the UI shows the
            // desktop up to the moment the resize lands rather than up to the
            // last tick.
            *dirty = true;
            // Tell the view where the pointer now maps to, before any pixel of
            // that size exists.
            let _ = deps.events.send(RemoteEvent::Size {
                width: w,
                height: h,
            });
            *resize_in_flight = Some(InFlightResize {
                size: (w, h),
                started: tokio::time::Instant::now(),
            });
            let _ = iron_input.send(IronRdpInput::Resize {
                width: w,
                height: h,
                scale_factor: 100,
                physical_size: None,
            });
        }
        // Input is pointless before the desktop exists, and the client drains
        // its channel during the handshake anyway — sending now would only queue
        // it up to be applied after the connection, possibly out of order with
        // the first frame.
        _other if deps.status.state() != RemoteState::Connected => {}        other => forward_input(input_db, iron_input, other),
    }
}

/// Run the session on the current runtime until it ends or `closed` is set.
///
/// Takes the input receiver rather than creating it: the receiver has exactly one
/// owner, and the caller (which created the sender the UI holds) is that owner.
/// This also keeps `tokio::spawn` out of the driver, so it can run on the
/// dedicated current-thread runtime its caller sets up.
pub async fn run_with_input(
    config: EmbeddedRdpConfig,
    mut deps: DriverDeps,
    mut input_rx: mpsc::UnboundedReceiver<RemoteInput>,
) {
    // Certificate first: ironrdp connects to whatever it is pointed at, so the
    // trust decision has to happen before it gets the chance.
    if let Err(reason) = authorize_certificate(&config, &mut deps).await {
        push_state(&deps, RemoteState::Failed, Some(reason.clone()));
        return;
    }

    let (output_tx, mut output_rx) = mpsc::channel::<RdpOutputEvent>(8);

    let iron_config = match build_config(&config) {
        Ok(cfg) => cfg,
        Err(err) => {
            push_state(&deps, RemoteState::Failed, Some(err.clone()));
            deps.status.set(RemoteState::Failed, Some(err));
            return;
        }
    };
    let client = RdpClient::new(iron_config, output_tx);
    let iron_input = client.input_sender();

    deps.status.set(RemoteState::Connecting, None);
    push_state(&deps, RemoteState::Connecting, None);

    // The protocol future owns the client for its whole life, but it is driven
    // from this task instead of being spawned: ironrdp's `run` future is not
    // provably `Send` (the compiler hits a higher-ranked lifetime limit inside
    // the library's own async fns), while this driver already has a dedicated
    // current-thread runtime to itself, so a second task would buy nothing.
    let mut session = Box::pin(client.run());
    let mut session_done = false;
    // Set when a failure was published with its reason and the loop is about to
    // break, so the tail below does not replace it with a reasonless
    // "Disconnected".
    let mut failure_reported = false;

    let mut frames = FrameQueue::new();
    let mut input_db = IronRdpInputDatabase::new();
    let mut framebuffer: Option<RgbaImage> = None;
    let mut last_emit = tokio::time::Instant::now() - FRAME_INTERVAL;
    let mut dirty = false;
    let mut size = deps
        .size
        .lock()
        .map(|slot| *slot)
        .unwrap_or((config.width, config.height));
    // (#rdp-resize-drop 2026-10-27) The size the session is known to be running
    // at. It starts as the size that was *performed* (ironrdp itself rewrites a
    // reconnect's desktop size into this field, see `RdpClient::run`), and is
    // overwritten by the shape of every frame the server sends. `size` above is
    // the size the *UI* wants; the two are kept apart because a resize can fail
    // and leave the UI asking for a desktop the session never adopted.
    //
    // They used to be the same variable, which is what made a failed resize
    // permanent: `size` was moved to the requested value before the server had
    // answered, so nothing could restore it — the next resize to the old size
    // was then dropped as a no-op, and after a drop-and-reconnect the session
    // came back at the failed size and stayed there.
    let mut applied_size = size;
    // Some(resize) while a resize is in flight and the session may not be
    // driven; see the long note above `loop`.
    let mut resize_in_flight: Option<InFlightResize> = None;
    // Input that arrived while a resize was in flight (see `deferred_input`).
    let mut deferred_input: std::collections::VecDeque<RemoteInput> = std::collections::VecDeque::new();
    let mut tick = tokio::time::interval(INPUT_TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // (#rdp-resize-drop 2026-10-27) What a desktop resize is, here, and why the
    // session is not driven while one is in flight.
    //
    // ironrdp performs a mid-session resize in one of two ways (`RdpClient`):
    // the conventional "Deactivate-Reactivation Sequence" inside the active
    // session (`encode_resize`), or — when the server refuses that — by **ending
    // the active session and running the whole connection loop again** at the
    // new size (`RdpControlFlow::ReconnectWithNewSize`). The second one produces
    // no `Terminated` event; to this driver it is just a quiet stretch. Two
    // things follow, and both used to be wrong:
    //
    // * The picture cannot be thrown away at the request any more. Nothing may
    //   arrive for as long as the reconnect takes, so a dropped picture left the
    //   tab blank until then; it is now replaced only by a frame, and the new
    //   size travels ahead of it as `RemoteEvent::Size`.
    // * Input may not be delivered while that reconnect is in flight. The
    //   client's input channel is *drained* both during the connection phase
    //   (`connect_*` never reads it) and inside the Deactivate-Reactivation loop
    //   (`single_sequence_step_read` does not select on it), so anything sent in
    //   that window — a `Resize` answering the maximised window, a `Close` sent
    //   by closing the tab, keystrokes — sits in the channel and is executed on
    //   the *other* side of the reactivation. A stale `Resize` then restarts the
    //   session again at a size nobody asked for, which is what a
    //   "reconnects and drops on every maximise" looks like from the user's
    //   side. Input is therefore held while a resize is in flight, and its
    //   deadline is what ends such a loop.
    loop {
        if deps.closed.load(Ordering::Relaxed) {
            // (#remote-graceful-close 2026-10-21) 关 tab 时先让远端的 RDP 会话
            // **正常断开**，而不是靠 drop 掉 future 关 socket。
            //
            // 用户反馈「关掉 tab 再连，服务器说另一个会话已登录」。落在一台
            // 单会话策略的 Windows 上时，这条提示来自**同一账号已存在的会话**：
            // drop future 只是让 TCP 断开，服务器要等传输层发现才会把会话标成
            // Disconnected；这个窗口内重连，新旧两个会话同时在册，于是弹「是否
            // 强制登录」。发 `Close` 会走 ironrdp 的 graceful_shutdown（MCS
            // DisconnectProviderUltimatum），服务器立刻把会话挂起。
            //
            // 最多等 400ms：服务器不答（或正在握手）也不能拖住关 tab，超时就
            // 照旧 drop。
            //
            // 正在 resize 收尾时 Close 会被摁在 input 里、等到 reactivation
            // 之后才执行，只会白等 400ms（#rdp-resize-drop 2026-10-27）。这时
            // 直接 drop future：那一步本来就是兜底，会话已经不再发任何东西。
            if !session_done && resize_in_flight.is_none() {
                let _ = iron_input.send(IronRdpInput::Close);
                let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
                // 结果只看「是否在窗口内收尾」：这之后的 drop 就是兜底，
                // 所以不用再回写 session_done（马上 break）。
                let _ = tokio::time::timeout_at(deadline, &mut session).await;
            }
            break;
        }

        tokio::select! {
            // When the protocol future ends, its output sender is already gone:
            // the branch below will see the channel close. It is drained first
            // so the final state event is not lost, and `session_done` keeps the
            // completed future from being polled a second time.
            _ = &mut session, if !session_done => {
                session_done = true;
            }

            // The resize deadline. Deliberately a call into a named helper
            // rather than an inline `sleep_until`: `select!` creates each branch
            // expression afresh on every loop turn, so a deadline computed from
            // the start would be re-created already expired and fire non-stop.
            _ = resize_deadline(resize_in_flight), if resize_in_flight.is_some() => {
                let pending = resize_in_flight.take().map(|r| r.size);
                tracing::warn!(
                    ?pending,
                    "RDP desktop resize did not complete in time; keeping the session as it is"
                );
                // If the protocol future is already gone, nothing will ever
                // answer: the loop is about to end on the closed output channel,
                // so the held input (a `Close`, usually) is not worth flushing.
                if session_done {
                    break;
                }
                // The request is given back, so the same size can be asked for
                // again later: a resize that timed out once is usually a server
                // hiccup, not a permanent refusal.
                applied_size = pending.unwrap_or(applied_size);
                if let Ok(mut slot) = deps.size.lock() {
                    *slot = applied_size;
                }
                match deps.status.state() {
                    RemoteState::Connected => {}
                    // The handshake may genuinely still be running (a
                    // reconnect after a drop); the frame or the failure that
                    // follows will say what happened, so this reports nothing
                    // rather than guessing.
                    RemoteState::Connecting | RemoteState::Authenticating | RemoteState::Reconnecting => {
                        push_state(&deps, RemoteState::Connecting, None);
                    }
                    _ => {
                        push_state(
                            &deps,
                            RemoteState::Failed,
                            Some(
                                crate::i18n::t(
                                    "远程桌面调整分辨率时失去了响应",
                                    "the remote desktop stopped responding while it was being resized",
                                )
                                .to_string(),
                            ),
                        );
                        failure_reported = true;
                    }
                }
            }

            _ = tick.tick() => {
                if dirty && last_emit.elapsed() >= FRAME_INTERVAL {
                    last_emit = tokio::time::Instant::now();
                    dirty = false;
                    if let Some(fb) = framebuffer.as_ref() {
                        frames.push_frame(RemoteEvent::Frame(fb.clone()));
                    }
                }
                for event in frames.drain() {
                    if deps.events.send(event).is_err() {
                        deps.closed.store(true, Ordering::Relaxed);
                    }
                }
            }

            maybe_out = output_rx.recv() => {
                let Some(out) = maybe_out else { break };
                match out {
                    RdpOutputEvent::Image { buffer, width, height } => {
                        let dimensions = (width.get(), height.get());
                        // A frame is the only proof of which size the session is
                        // actually at, and it ends any resize in flight — either
                        // because it is the frame that resize asked for, or
                        // because a differently-sized one means the resize was
                        // never going to happen and waiting any longer is moot.
                        applied_size = dimensions;
                        if let Ok(mut slot) = deps.size.lock() {
                            *slot = dimensions;
                        }
                        if resize_in_flight.is_some() {
                            tracing::info!(?dimensions, "RDP desktop resize completed");
                            resize_in_flight = None;
                        }
                        framebuffer = Some(to_rgba(&buffer, dimensions.0, dimensions.1));
                        dirty = true;
                        if !matches!(deps.status.state(), RemoteState::Connected) {
                            push_state(&deps, RemoteState::Connected, None);
                        }
                    }
                    RdpOutputEvent::ConnectionFailure(err) => {
                        let reason = describe_failure(&err);
                        deps.status.set(RemoteState::Failed, Some(reason.clone()));
                        push_state(&deps, RemoteState::Failed, Some(reason));
                        failure_reported = true;
                        break;
                    }
                    RdpOutputEvent::Terminated(reason) => {
                        tracing::info!("RDP session terminated: {reason:?}");
                        // The reason is the only thing that tells "the server
                        // hung up", "someone took the session over" and "the
                        // transport died" apart, and the UI has a slot for it:
                        // `status_for` appends it to the state (#remote-
                        // reconnect 2026-10-13). It used to be logged here and
                        // thrown away, which is why every ending read "已断线".
                        let note = termination_note(&reason);
                        deps.status.set(RemoteState::Disconnected, note.clone());
                        push_state(&deps, RemoteState::Disconnected, note);
                        break;
                    }
                    RdpOutputEvent::PointerDefault
                    | RdpOutputEvent::PointerHidden
                    | RdpOutputEvent::PointerPosition { .. }
                    | RdpOutputEvent::PointerBitmap(_) => {
                        // Remote cursor shapes are a P1 refinement; leaving the
                        // local cursor visible is the honest behaviour rather
                        // than a cursor drawn at the wrong place.
                    }
                }
            }

            maybe_input = input_rx.recv() => {
                let Some(event) = maybe_input else { break };
                if !accepts_input(resize_in_flight) {
                    // Held, not dropped: see the resize note above `loop`. A key
                    // pressed during the reactivation is news the session wants
                    // on the other side of it; a `Resize` is re-evaluated there
                    // because the resize may have completed by then.
                    deferred_input.push_back(event);
                    continue;
                }
                apply_input(
                    &deps,
                    &mut input_db,
                    &iron_input,
                    &mut dirty,
                    &mut size,
                    &mut applied_size,
                    &mut resize_in_flight,
                    event,
                );
            }

        }

        // Whatever was held back while a resize was in flight, in the order it
        // arrived, now that the session can take it. A `Resize` is looked at
        // against the size the session settled on, so one that was answered by
        // the frame it was waiting for is recognised as a no-op.
        //
        // Bounded on purpose: `apply_input` may arm a *new* resize, and the
        // queue has to stay finite — what is left is picked up on the next turn
        // of the loop, where the select would run again anyway.
        let mut drained = 0usize;
        while accepts_input(resize_in_flight) && drained < deferred_input.len() {
            let Some(event) = deferred_input.pop_front() else {
                break;
            };
            drained += 1;
            apply_input(
                &deps,
                &mut input_db,
                &iron_input,
                &mut dirty,
                &mut size,
                &mut applied_size,
                &mut resize_in_flight,
                event,
            );
        }
    }

    deps.closed.store(true, Ordering::Release);
    drop(iron_input);
    // Dropping the pinned future is this driver's version of `abort()`: the
    // client stops at its next await point and its sockets close with it.
    drop(session);
    // A session that ran and then ended is "disconnected"; one that never got
    // through keeps the failure and its reason.
    if let Some(state) = closing_state(failure_reported) {
        push_state(&deps, state, None);
    }
}

/// Decide whether to connect at all, prompting the user if needed.
///
/// Returns `Err` only when the connection must not proceed (a fingerprint
/// mismatch under a strict policy, an unanswered prompt, or a probe that could
/// not complete). A pending prompt is not an error: the prompt goes to the UI
/// and this call blocks on the answer, so a rejected certificate never reaches
/// ironrdp at all.
async fn authorize_certificate(
    config: &EmbeddedRdpConfig,
    deps: &mut DriverDeps,
) -> Result<(), String> {
    use crate::remote::trust::{authorize, open_decision, resolve_decision, TrustOutcome};

    // Best-effort, and deliberately not fatal: see `verifier`'s module header.
    // Some healthy servers simply have no certificate to show — one that answers
    // the connection initiation with standard RDP security runs no TLS at all —
    // and a probe can also fail for reasons that say nothing about trust. So a
    // failure here means "no fingerprint to compare", not "untrustworthy", and
    // refusing to connect over it would break those sessions outright. ironrdp's
    // own TLS layer accepts any certificate regardless, so the prompt is a bonus
    // rather than a gate.
    let probe =
        match crate::rdp::embedded::verifier::probe_certificate(&config.host, config.port).await {
            Ok(probe) => probe,
            Err(reason) => {
                tracing::warn!(
                    "RDP certificate pre-flight probe for {}:{} failed ({reason}); \
                     connecting without a known-hosts check",
                    config.host,
                    config.port
                );
                return Ok(());
            }
        };
    let outcome = authorize(
        "rdp",
        &config.host,
        config.port,
        &probe.fingerprint,
        &config.certificate_policy,
    );
    if outcome == TrustOutcome::Trusted {
        return Ok(());
    }
    if outcome == TrustOutcome::Reject {
        return Err(crate::i18n::t(
            "服务器证书与已信任的指纹不符，已拒绝连接",
            "server certificate does not match the trusted fingerprint; connection refused",
        )
        .to_string());
    }

    let request_id = open_decision(
        "rdp",
        &config.host,
        config.port,
        &probe.fingerprint,
        probe.subject.clone(),
        probe.issuer.clone(),
    );
    let known_status = match outcome {
        TrustOutcome::PromptChanged => "changed",
        _ => "unknown",
    };
    deps.status.set(
        RemoteState::Authenticating,
        Some(
            crate::i18n::t("等待证书信任确认...", "Waiting for a certificate trust decision...")
                .to_string(),
        ),
    );
    let _ = deps.events.send(RemoteEvent::TrustRequest(TrustRequest {
        request_id,
        protocol: "rdp",
        host: config.host.clone(),
        port: config.port,
        fingerprint: probe.fingerprint,
        subject: probe.subject,
        issuer: probe.issuer,
        valid_from: probe.valid_from,
        valid_to: probe.valid_to,
        known_status,
    }));

    // Bounded, so a tab left open overnight with an unanswered prompt cannot
    // hold a socket open forever.
    let answer = tokio::time::timeout(TRUST_TIMEOUT, deps.trust.recv()).await;
    let decision = match answer {
        Ok(Some(answer)) => answer,
        _ => {
            let _ = resolve_decision(request_id, false, false);
            return Err(crate::i18n::t(
                "证书信任确认超时",
                "timed out waiting for a certificate trust decision",
            )
            .to_string());
        }
    };
    // The id is re-checked: an answer meant for another session's prompt must
    // not be applied here, and must not be consumed silently either.
    if decision.0 != request_id {
        return Err(crate::i18n::t(
            "证书信任确认未送达该连接",
            "the certificate trust decision did not reach this connection",
        )
        .to_string());
    }
    let _ = resolve_decision(request_id, decision.1, decision.2);
    if decision.1 {
        Ok(())
    } else {
        Err(crate::i18n::t(
            "已拒绝该服务器的证书",
            "the server certificate was rejected",
        )
        .to_string())
    }
}

/// Build the ironrdp config for one connection attempt.
fn build_config(config: &EmbeddedRdpConfig) -> Result<ironrdp::client::config::Config, String> {
    // `Destination::new` parses "host:port"; going through it rather than
    // splitting by hand means an IPv6 literal takes the same code path as a
    // hostname.
    let destination = Destination::new(format!("{}:{}", config.host, config.port))
        .map_err(|err| format!("{err:#}"))?;
    let password = password_for(config);
    ConfigBuilder::new()
        .with_destination(destination)
        .with_transport(TransportKind::Direct)
        .with_username(config.username.clone())
        .with_domain(config.domain.clone())
        .with_password(password.clone())
        .with_desktop_width(config.width)
        .with_desktop_height(config.height)
        .with_desktop_scale_factor(100)
        .with_color_depth(u32::from(config.color_depth.clamp(16, 32)))
        // (#rdp-blank-password 2026-10-13) NLA only when there *is* a password —
        // and `password_for` has already asked Windows for one when the field was
        // blank. CredSSP authenticates before the session exists, so with no
        // password at all there is no "ask later": the server can only refuse the
        // connection (seen against a real host as `0xc000006d`
        // STATUS_LOGON_FAILURE, with the remote logon screen never appearing).
        // No password therefore means what the dialog promises — leave blank and
        // the server's own logon screen collects the secret — and that screen
        // only exists on the plain TLS path. A server that *requires* NLA still
        // turns a TLS-only offer down; the refusal is reported verbatim.
        .with_credssp(use_credssp(&password))
        .with_tls(true)
        .with_platform(MajorPlatformType::WINDOWS)
        .with_client_build(client_build())
        .with_client_dir("C:\\Windows\\System32\\mstscax.dll")
        .with_client_name(client_name())
        .build()
        .map_err(|err| format!("{err:#}"))
}

/// The password this attempt authenticates with.
///
/// The session's own value wins whenever the field is set: the field is the
/// user's statement of intent, and `#blank-password-overwrites` settled that a
/// blank field means "the password is blank", not "keep the old one". A blank
/// field then asks Windows for the same entry the system client ends up using
/// (#credential-store): with no saved password MTerm starts `mstsc /v:host`, and
/// `mstsc` reads `TERMSRV/<host>` out of the Credential Manager on its own — so
/// consulting that entry here is what makes the two connection modes agree.
/// When the store has nothing the result is an empty password: the old
/// behaviour, not an error.
fn password_for(config: &EmbeddedRdpConfig) -> String {
    if !config.password.is_empty() {
        return config.password.clone();
    }
    crate::rdp::embedded::credentials::stored_password(&config.host, config.port).unwrap_or_default()
}

/// Whether to negotiate CredSSP (NLA) with `password`.
///
/// NLA authenticates before the session is created, so it needs the password up
/// front; with a blank one the only way to reach a session at all is the plain
/// TLS path, where the server's logon screen collects the secret. The dialog
/// says exactly that ("leave blank — prompt on connect"), and an empty password
/// is the only signal that distinguishes it from "authenticate me silently".
///
/// Note that this is *not* a silent downgrade: a server configured to require
/// NLA still refuses a TLS-only offer, and that refusal is what the user sees.
fn use_credssp(password: &str) -> bool {
    !password.is_empty()
}

/// Version triple of MTerm, encoded the way `client_build` expects.
fn client_build() -> u32 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .take(3)
        .fold(0_u32, |acc, part| {
            acc.saturating_mul(100)
                .saturating_add(part.parse::<u32>().unwrap_or(0))
        })
}

/// Reported client machine name. Servers log it, and a real name makes the
/// session recognisable in the remote machine's event log.
fn client_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "MTerm".to_string())
}

/// ironrdp hands back one `u32` per pixel packed as `0x00RRGGBB`; unpack it into
/// the RGBA bytes Slint wants.
///
/// The packing is fixed by the decoder, so there is no per-session pixel-format
/// negotiation on this side.
fn to_rgba(buffer: &[u32], width: u16, height: u16) -> RgbaImage {
    let mut img = RgbaImage::new(u32::from(width), u32::from(height));
    {
        let dst = img.as_mut();
        let count = (u32::from(width) * u32::from(height)) as usize;
        // `RgbaImage::as_mut` yields the pixel bytes in RGBA order, tightly
        // packed, so one pixel is four bytes: the stride is derived from the
        // buffer rather than assumed, and the four bytes are written as a slice
        // instead of through the `Rgba` wrapper.
        let dst = dst.as_mut();
        let stride = dst.len() / count.max(1);
        for (i, src) in buffer.iter().take(count).enumerate() {
            let [_, r, g, b] = src.to_be_bytes();
            let px = &mut dst[i * stride..i * stride + 4];
            px.copy_from_slice(&[r, g, b, 0xff]);
        }
    }
    img
}

/// Apply one input event to the ironrdp input sender.
///
/// Keys go through the `Database` so ironrdp can derive the fast-path flags,
/// including the left/right modifier disambiguation the protocol needs.
fn forward_input(
    db: &mut IronRdpInputDatabase,
    sender: &mpsc::UnboundedSender<IronRdpInput>,
    event: RemoteInput,
) {
    let ops: Vec<IronRdpOperation> = match event {
        RemoteInput::Key { code, down } => {
            // The shared input path folds the extended (E0) flag into bit 8 so
            // one `u32` event type serves both protocols; ironrdp wants it as a
            // separate bool, so it is split back out here.
            let extended = code & 0x100 != 0;
            let scancode = Scancode::from_u8(extended, (code & 0xff) as u8);
            vec![if down {
                IronRdpOperation::KeyPressed(scancode)
            } else {
                IronRdpOperation::KeyReleased(scancode)
            }]
        }
        RemoteInput::Text { ch } => vec![
            IronRdpOperation::UnicodeKeyPressed(ch),
            IronRdpOperation::UnicodeKeyReleased(ch),
        ],
        RemoteInput::Pointer { x, y, mask } => {
            // Every button is stated explicitly, pressed or released: the RFB
            // and RDP pointer protocols both describe the *current* button mask
            // rather than a delta, so inferring from a bitmask would leave a
            // button stuck when the mask was unchanged.
            let mut ops = vec![IronRdpOperation::MouseMove(MousePosition { x, y })];
            for (bit, mouse_button) in [
                (button::LEFT, MouseButton::Left),
                (button::MIDDLE, MouseButton::Middle),
                (button::RIGHT, MouseButton::Right),
                (button::BACK, MouseButton::X1),
                (button::FORWARD, MouseButton::X2),
            ] {
                ops.push(if mask & bit != 0 {
                    IronRdpOperation::MouseButtonPressed(mouse_button)
                } else {
                    IronRdpOperation::MouseButtonReleased(mouse_button)
                });
            }
            ops
        }
        RemoteInput::Wheel { x, y, dx, dy } => {
            let mut ops = vec![IronRdpOperation::MouseMove(MousePosition { x, y })];
            if dx.abs() > 0.001 {
                ops.push(IronRdpOperation::WheelRotations(WheelRotations {
                    is_vertical: false,
                    rotation_units: clamp_i16(-dx),
                }));
            }
            if dy.abs() > 0.001 {
                ops.push(IronRdpOperation::WheelRotations(WheelRotations {
                    is_vertical: true,
                    rotation_units: clamp_i16(-dy),
                }));
            }
            ops
        }
        RemoteInput::ReleaseAll => {
            let fast = db.release_all();
            if !fast.is_empty() {
                let _ = sender.send(IronRdpInput::FastPath(fast));
            }
            return;
        }
        RemoteInput::Resize { .. } => return,
    };
    if ops.is_empty() {
        return;
    }
    let fast = db.apply(ops);
    if !fast.is_empty() {
        let _ = sender.send(IronRdpInput::FastPath(fast));
    }
}

fn clamp_i16(value: f64) -> i16 {
    if value.is_nan() {
        return 0;
    }
    value.clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

/// Publish a state change to the UI.
///
/// Sent outside the frame queue on purpose: a state change is the one event that
/// must never be dropped, and routing it through the drop-oldest frame queue
/// would let a busy desktop hide it. The status object is authoritative, so it is
/// updated too rather than trusting the two to stay in step.
fn push_state(deps: &DriverDeps, state: RemoteState, message: Option<String>) {
    deps.status.set(state, message.clone());
    let _ = deps.events.send(RemoteEvent::State {
        state: deps.status.clone(),
        message,
    });
}

/// Describe a connection failure in words a user can act on.
///
/// Neither built-in rendering works for a tab. `Display` on the error prints
/// `[context @ file:line] kind` — a build location, then the bare kind, which for
/// a CredSSP refusal is the word "CredSSP" and nothing else — while `Debug` dumps
/// the whole struct. The server's own reason sits one `source()` hop down and has
/// a readable `Display` there: a rejected NLA logon reads
/// `InvalidToken: CredSSP server returned an error status; status is
/// STATUS_LOGON_FAILURE [0xc000006d]`, which names the actual problem instead of
/// looking like a client bug.
///
/// The cause can hang off either the error itself (`with_source`, which is what
/// `custom` sets) or off the kind (`ConnectorErrorKind::Credssp` keeps the `sspi`
/// error inside the variant). `ironrdp_error::Error::source` already tries the
/// kind first and falls back to `with_source`, so the first lookup is normally
/// enough — the `or_else` only keeps this working if that precedence ever
/// changes underneath us, which is a dependency's implementation detail.
fn describe_failure(err: &ironrdp::connector::ConnectorError) -> String {
    let mut out = err.kind().to_string();
    // The flattened text leads with the kind, not with `[context @ file:line]`:
    // the file and line are build details and would bury the reason.
    let mut cause =
        std::error::Error::source(err).or_else(|| std::error::Error::source(err.kind()));
    // Bounded: a malformed chain must not spin, and nothing actionable lives
    // past a handful of hops.
    for _ in 0..8 {
        let Some(current) = cause else { break };
        let text = current.to_string();
        if !text.is_empty() && !out.contains(text.as_str()) {
            out.push_str(": ");
            out.push_str(&text);
        }
        cause = current.source();
    }
    out
}

/// A human line for why the session ended, or `None` when there is nothing to
/// say beyond "it ended".
///
/// The graceful half is worth translating: the server names whether the
/// disconnect was asked for by the user sitting in front of it or by the server
/// itself, and that is the difference between "you were taken over" and "the
/// box went down". The error half goes through the shared classifier so it
/// reads like every other failure, and a text it cannot place yields `None`
/// rather than the generic "connection failed" — the state line already says
/// the session is gone (#remote-reconnect 2026-10-13).
fn termination_note(reason: &SessionResult<GracefulDisconnectReason>) -> Option<String> {
    match reason {
        Ok(GracefulDisconnectReason::UserInitiated) => Some(
            crate::i18n::t("远端用户断开了连接", "The remote user disconnected").to_string(),
        ),
        Ok(GracefulDisconnectReason::ServerInitiated) => Some(
            crate::i18n::t("服务器断开了连接", "The server closed the connection").to_string(),
        ),
        // `Other(description)` is the protocol's own wording (a disconnect PDU
        // description) — run it through the classifier like any other text.
        Ok(GracefulDisconnectReason::Other(description)) => {
            crate::remote::failure::explain_optional(description)
        }
        Err(err) => crate::remote::failure::explain_optional(&err.to_string()),
    }
}

/// Counter used to hand out trust request ids. Process-global because a session
/// has exactly one verifier and the id only has to be unique among the requests
/// actually pending.
///
/// **Dead, and dangerous to revive as-is.** The ids the driver actually sends and
/// re-checks come from [`crate::remote::trust::open_decision`], which is also the
/// only counter the trust store knows about; this one would hand out ids that
/// collide with it, and the driver refuses an answer whose id it did not raise.
/// Left in place (with its test) rather than deleted, so removing it stays a
/// deliberate cleanup instead of an accident of this change.
static NEXT_TRUST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Allocate the next trust request id.
pub fn next_trust_id() -> u64 {
    NEXT_TRUST_ID.fetch_add(1, Ordering::Relaxed)
}

/// Unused today, but the handle type is what the UI holds and the driver keeps
/// the same shape for when reconnect policy becomes configurable (P1-10).
#[allow(dead_code)]
fn assert_handle_is_send_sync(_: &RemoteHandle) {}

#[cfg(test)]
mod tests {
    use super::*;
    use ironrdp::connector::ConnectorErrorExt;

    #[test]
    fn a_failure_keeps_the_servers_reason_and_drops_the_build_location() {
        // Mirrors what a rejected NLA logon reports: the readable status lives in
        // the *cause*, which the `{:?}` dump this replaced buried inside a struct
        // — `0xc000006d` with no hint that it means "wrong credentials".
        #[derive(Debug)]
        struct Rejected;
        impl std::fmt::Display for Rejected {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "InvalidToken: CredSSP server returned an error status; \
                     status is STATUS_LOGON_FAILURE [0xc000006d]"
                )
            }
        }
        impl std::error::Error for Rejected {}

        let err = ironrdp::connector::ConnectorError::custom("CredSSP", Rejected);
        let text = describe_failure(&err);
        assert!(text.contains("STATUS_LOGON_FAILURE"), "{text}");
        assert!(
            !text.contains(".rs:"),
            "a build location must not reach the user: {text}"
        );
    }

    #[test]
    fn a_failure_without_a_cause_reads_as_its_reason() {
        // `reason` carries the text in the kind itself: nothing to append, and
        // nothing to lose.
        let err = ironrdp::connector::ConnectorError::reason("negotiation", "server hung up");
        assert_eq!(describe_failure(&err), "reason: server hung up");
    }

    /// (#rdp-resize-drop 2026-10-27) A resize asks for a desktop the session is
    /// not at, and reports the size to the UI *before* any frame of that size
    /// exists — the view's pointer mapping is wrong for the whole reconnect
    /// otherwise. Both facts are asserted here because they are the two halves of
    /// the same mistake: the size used to be compared against a value it had
    /// already overwritten, and nothing at all was told to the UI.
    #[tokio::test]
    async fn a_resize_reports_the_new_size_and_arms_the_hold() {
        let (events_tx, mut events_rx) = mpsc::unbounded_channel();
        let (iron_tx, mut iron_rx) = mpsc::unbounded_channel();
        let status = Arc::new(RemoteStatus::new(RemoteState::Connected));
        let deps = DriverDeps {
            status,
            closed: Arc::new(AtomicBool::new(false)),
            events: events_tx,
            trust: mpsc::unbounded_channel().1,
            size: Arc::new(std::sync::Mutex::new((1280, 720))),
        };
        let mut db = IronRdpInputDatabase::new();
        let mut dirty = false;
        let mut size = (1280, 720);
        let mut applied = (1280, 720);
        let mut in_flight = None;

        apply_input(
            &deps,
            &mut db,
            &iron_tx,
            &mut dirty,
            &mut size,
            &mut applied,
            &mut in_flight,
            RemoteInput::Resize {
                width: 2560,
                height: 1440,
            },
        );

        assert_eq!(size, (2560, 1440), "the UI's target moved with the request");
        assert_eq!(applied, (1280, 720), "the session is still running at 720p");
        assert!(in_flight.is_some(), "input must be held until the server answers");
        assert!(dirty, "the picture on screen must be flushed, not discarded");

        let announced = events_rx.try_recv().expect("the view is told the new size");
        assert!(matches!(
            announced,
            RemoteEvent::Size { width: 2560, height: 1440 }
        ));
        assert!(matches!(
            iron_rx.try_recv(),
            Ok(IronRdpInput::Resize { width: 2560, height: 1440, .. })
        ));
    }

    /// The same resize asked for twice in a row (a maximised window reports its
    /// size once per relayout) must not queue a second one: a stale `Resize`
    /// executed after the reactivation is a second reconnect.
    #[tokio::test]
    async fn asking_for_the_size_already_pending_is_not_a_second_resize() {
        let (events_tx, _events_rx) = mpsc::unbounded_channel();
        let (iron_tx, mut iron_rx) = mpsc::unbounded_channel();
        let deps = DriverDeps {
            status: Arc::new(RemoteStatus::new(RemoteState::Connected)),
            closed: Arc::new(AtomicBool::new(false)),
            events: events_tx,
            trust: mpsc::unbounded_channel().1,
            size: Arc::new(std::sync::Mutex::new((1280, 720))),
        };
        let mut db = IronRdpInputDatabase::new();
        let mut dirty = false;
        let mut size = (1280, 720);
        let mut applied = (1280, 720);
        let mut in_flight = None;
        for _ in 0..2 {
            apply_input(
                &deps,
                &mut db,
                &iron_tx,
                &mut dirty,
                &mut size,
                &mut applied,
                &mut in_flight,
                RemoteInput::Resize {
                    width: 1920,
                    height: 1080,
                },
            );
        }
        assert!(matches!(
            iron_rx.try_recv(),
            Ok(IronRdpInput::Resize { width: 1920, height: 1080, .. })
        ));
        assert!(
            iron_rx.try_recv().is_err(),
            "the second identical request must not reach ironrdp"
        );
    }

    /// Input is refused while a resize is in flight, and accepted again the moment
    /// the server's frame settles it.
    #[test]
    fn input_is_held_while_a_resize_is_in_flight() {
        assert!(accepts_input(None));
        assert!(!accepts_input(Some(InFlightResize {
            size: (1920, 1080),
            started: tokio::time::Instant::now(),
        })));
    }

    #[test]
    fn pixels_unpack_to_opaque_rgba_in_channel_order() {
        let buffer = vec![0x0012_34FFu32, 0x00FF_0000];
        let img = to_rgba(&buffer, 2, 1);
        assert_eq!(img.dimensions(), (2, 1));
        let px = img.as_raw();
        // ironrdp packs 0x00RRGGBB, and the bytes written are the RGBA order
        // `image` and Slint's pixel buffer both use, with alpha forced opaque.
        assert_eq!(&px[0..4], &[0x12, 0x34, 0xff, 0xff]);
        assert_eq!(&px[4..8], &[0xff, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn a_short_buffer_leaves_the_remaining_pixels_black_rather_than_panicking() {
        // A frame one pixel short must still produce a drawable image; panicking
        // here would take the whole tab down.
        let img = to_rgba(&[0x00FF_FFFF], 2, 1);
        assert_eq!(img.dimensions(), (2, 1));
        assert_eq!(&img.as_raw()[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn wheel_deltas_are_clamped_instead_of_wrapping() {
        assert_eq!(clamp_i16(1e9), i16::MAX);
        assert_eq!(clamp_i16(-1e9), i16::MIN);
        assert_eq!(clamp_i16(f64::NAN), 0);
        assert_eq!(clamp_i16(3.7), 3);
    }

    #[test]
    fn trust_ids_are_unique() {
        assert_ne!(next_trust_id(), next_trust_id());
    }

    #[test]
    fn the_version_triple_encodes_as_a_build_number() {
        assert_eq!(client_build(), 703, "0.7.3 -> 703");
    }

    #[test]
    fn a_blank_password_drops_nla_and_keeps_the_tls_path() {
        assert!(!use_credssp(&password_for(&config_with_password(""))));
        assert!(use_credssp(&password_for(&config_with_password("secret"))));
    }

    /// Only a genuinely empty field means "no password". Spaces are characters
    /// a password may contain, so the check must not grow a `trim()`.
    #[test]
    fn only_an_empty_field_counts_as_no_password() {
        assert!(use_credssp(&password_for(&config_with_password(" "))));
    }

    /// The blank-password path has to produce a config the client can actually
    /// run with, not merely a different flag.
    #[test]
    fn both_password_states_build_a_usable_client_config() {
        assert!(build_config(&config_with_password("")).is_ok());
        assert!(build_config(&config_with_password("secret")).is_ok());
    }

    /// A session that ends says which ending it was; a session that dies of an
    /// unrecognisable error says nothing rather than "connection failed", which
    /// says less than the bare state line already did.
    #[test]
    fn a_termination_names_its_reason_only_when_it_has_one() {
        use ironrdp::session::SessionErrorExt;

        let user = termination_note(&Ok(GracefulDisconnectReason::UserInitiated));
        let server = termination_note(&Ok(GracefulDisconnectReason::ServerInitiated));
        assert!(user.is_some() && server.is_some());
        assert_ne!(user, server, "the two graceful endings are not the same news");

        // An error the classifier knows reads like every other failure.
        let reset =
            ironrdp::session::SessionError::reason("active stage", "connection reset by peer");
        assert_eq!(
            termination_note(&Err(reset)),
            Some(crate::i18n::t("连接被重置", "The connection was reset").to_string())
        );

        // One it does not know must not come back as a headline.
        let odd =
            ironrdp::session::SessionError::reason("active stage", "something nobody has seen before");
        assert_eq!(termination_note(&Err(odd)), None);
    }

    fn config_with_password(password: &str) -> EmbeddedRdpConfig {
        EmbeddedRdpConfig {
            tab_id: "tab-1".to_string(),
            session_id: "session-1".to_string(),
            // RFC 5737 TEST-NET-1: no real host, so the credential store can
            // never hold an entry for it and a blank field stays blank.
            host: "192.0.2.9".to_string(),
            port: 3389,
            username: "Administrator".to_string(),
            domain: String::new(),
            password: password.to_string(),
            width: 1280,
            height: 720,
            follow_tab_size: false,
            color_depth: 32,
            certificate_policy: "prompt".to_string(),
        }
    }
}
