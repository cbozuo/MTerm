//! Starting, stopping and steering a remote-desktop session for a tab.
//!
//! This is the single place that knows how a `Session` definition becomes a live
//! tab: it builds the protocol parameters, creates the channels, spawns the
//! driver thread and the pump thread, and publishes the routes both of them read.
//! The two protocol drivers stay unaware of Slint, and the UI stays unaware of
//! ironrdp/vnc-rs.
//!
//! It also owns the *end* of that lifecycle: a driver thread is kept, not
//! forgotten, so a closing tab can wait for its session to be really gone
//! (#remote-close-leftover 2026-10-27 — see [`close_session`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::config::{Session, SessionKind};
use crate::remote::pump::{self, RemoteRoutes};
use crate::remote::pane::{RemoteEvent, RemoteHandle, RemoteProtocol};
use crate::remote::registry::RemotePanes;
use crate::rdp::embedded::config::EmbeddedRdpConfig;
use crate::rdp::embedded::status::RemoteStatus;

/// How long [`close_session`] waits for a driver thread to finish its own
/// shutdown before returning (#remote-close-leftover 2026-10-27).
///
/// A graceful close is a disconnect PDU and a socket close; the driver bounds
/// its own graceful step at 400 ms, so this is that bound plus room for a slow
/// machine. It is a wait, not a kill: nothing is forced, and a session with no
/// thread behind it any more passes through at once.
const CLOSE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// A driver thread, kept so the tab can wait for it to finish.
///
/// Both facts here used to be free — the thread was spawned and forgotten — and
/// the tab lifecycle turns out to need both
/// (#remote-close-leftover 2026-10-27):
///
/// * **It is alive.** A tab whose session has ended (or failed, and is about to
///   offer a reconnect) still owns a handle, and the handle has to be able to
///   answer "is anything still running for me?". The driver itself cannot: it
///   sets `closed` on the way out, but a closed tab sets the same flag, so the
///   flag says nothing about the thread.
/// * **The thread.** Held only so `close_session` can wait for it; nothing else
///   ever joins it.
///
/// What went wrong without this: closing a tab sent `Close`, the server hung the
/// session up, and the window came back while the thread that owned the runtime
/// and the socket was still finishing. Reconnecting in the same window then
/// found the server holding the old, half-closed session ("用户的另一个会话被阻止
/// 并且通知了本地会话管理器"), and the dying thread could still draw one more
/// event into a pane row nobody wanted.
pub(crate) struct DriverThread {
    alive: Arc<AtomicBool>,
    /// `None` once the thread has been joined, or after it was detached.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl DriverThread {
    /// Whether the driver is still running.
    pub(crate) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// Wait for the driver to return.
    ///
    /// The `Option::take` is what lets a type with a `Drop` impl move its own
    /// `JoinHandle` out; the `Drop` below handles the case where this was never
    /// called (the thread is then detached rather than leaked into a panic).
    pub(crate) fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for DriverThread {
    fn drop(&mut self) {
        // Dropping the last handle is letting the session go: nobody is left to
        // draw the frames or answer a prompt, so the driver is told to stop even
        // when the closing path did not call `close()` first (a window torn
        // down, a map cleared). The thread is not joined here — a drop can
        // happen on the UI thread — and `wait_for_driver` is where a deliberate
        // close makes that wait explicit.
        self.alive.store(false, Ordering::Release);
        // A `JoinHandle` detached by `drop` is the default anyway; taking it is
        // just what keeps this an explicit statement rather than an accident.
        self.thread.take();
    }
}

/// Start a driver on its own thread and record that it is running.
///
/// One helper for both protocols: the liveness flag has to be dropped exactly
/// when the driver returns, whichever driver it is, and duplicating that around
/// two `spawn`s is how the two would drift apart.
fn spawn_driver(
    name: String,
    alive: Arc<AtomicBool>,
    body: impl FnOnce() + Send + 'static,
) -> Result<DriverThread, String> {
    alive.store(true, Ordering::Release);
    let flag = alive.clone();
    let thread = std::thread::Builder::new()
        .name(name)
        .spawn(move || {
            // Runs even if the driver unwinds: a panicking session must not
            // read as a live one for the rest of the process's life.
            let _done = DriverDone(flag);
            body();
        })
        .map_err(|err| format!("{err}"))?;
    Ok(DriverThread {
        alive,
        thread: Some(thread),
    })
}

/// Clears the liveness flag when the driver returns.
struct DriverDone(Arc<AtomicBool>);

impl Drop for DriverDone {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Everything the starter needs from the window that owns the tab.
pub struct StartCtx {
    pub routes: RemoteRoutes,
    /// The window the tab lives in.
    ///
    /// The starter registers the tab's route with it *itself*, before the pump
    /// thread exists, because the driver publishes its first state (「连接中…」)
    /// on its own thread the moment it is spawned. The pump resolves its route
    /// per event, so an event that arrives before the caller has inserted one is
    /// routed nowhere and lost — which is exactly how a fast failure could leave
    /// a tab with no visible state at all (#remote-close-leftover 2026-10-27).
    ///
    /// `None` keeps the old behaviour (the caller owns the route): used by call
    /// sites that have already registered it, and by tests that have no window.
    pub window_id: Option<u64>,
}

/// Start the session for `session` in tab `tab_id`.
///
/// Returns the handle the UI holds for the lifetime of the tab. A configuration
/// error is returned instead of a handle so the caller can put the reason on the
/// tab it is about to create — a tab that exists but can never connect is worse
/// than a clear error at the point of connection.
pub fn start(session: &Session, tab_id: &str, ctx: &StartCtx) -> Result<RemoteHandle, String> {
    match session.kind {
        SessionKind::Rdp => start_rdp_session(session, tab_id, ctx),
        SessionKind::Vnc => start_vnc_session(session, tab_id, ctx),
        other => Err(format!(
            "{} is not a remote-desktop protocol",
            other.as_str()
        )),
    }
}

/// Whether `session` should be hosted in a tab by MTerm's own client.
///
/// The single decision point for "does this get a tab or does it get handed to
/// the system client". `SessionKind::Rdp` in system mode answers `false` here and
/// therefore keeps its pre-existing path untouched.
///
/// Thin wrapper over [`Session::uses_tab`], which is the only place that can read
/// both the transport and `rdp_mode`. Deliberately not a second copy of that
/// logic: two copies that drifted would mis-classify `Rdp + Embedded` as an
/// external-client session, which connects fine and then shows no tab.
pub fn is_tab_hosted(session: &Session) -> bool {
    session.uses_tab()
}

/// `TabInfo.kind` for a hosted session — the tab bar and the theme branch both
/// read it, so it is derived here rather than spelled out at each call site.
pub fn tab_kind(session: &Session) -> &'static str {
    match session.kind {
        SessionKind::Vnc => "vnc",
        _ => "rdp",
    }
}

/// Register the tab's route with the window that owns it, when the caller has
/// not already done so.
///
/// Idempotent by design: a caller that registered the route itself (or a tab
/// that was dragged to another window) already has the entry it wants, and
/// overwriting it with the same data is harmless.
fn publish_route(tab_id: &str, ctx: &StartCtx) {
    let Some(window_id) = ctx.window_id else {
        return;
    };
    if let Ok(mut routes) = ctx.routes.lock() {
        routes.insert(
            tab_id.to_string(),
            Arc::new(Mutex::new(pump::RemoteRoute { window_id })),
        );
    }
}

/// Start the built-in RDP client for `session`.
pub fn start_rdp_session(
    session: &Session,
    tab_id: &str,
    ctx: &StartCtx,
) -> Result<RemoteHandle, String> {
    let config = EmbeddedRdpConfig::from_session(session, tab_id)?;
    let status = Arc::new(RemoteStatus::new(
        crate::rdp::embedded::status::RemoteState::Connecting,
    ));
    let closed = Arc::new(AtomicBool::new(false));
    // (#remote-close-leftover 2026-10-27) Whether the driver thread is still
    // running, and the thread itself: `close_session` has to be able to answer
    // "is the tab really gone?" instead of hoping.
    let driver_alive = Arc::new(AtomicBool::new(true));
    let (events_tx, events_rx) = pump::event_channel();
    let (trust_tx, trust_rx) = mpsc::unbounded_channel::<(u64, bool, bool)>();
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let size = Arc::new(Mutex::new((config.width, config.height)));

    let handle = RemoteHandle::new(
        tab_id,
        input_tx,
        events_tx.clone(),
        status.clone(),
        closed.clone(),
        trust_tx,
        config.follow_tab_size,
        RemoteProtocol::Rdp,
    );

    // The pump owns the receiver; the driver owns the sender.
    let shutdown = closed.clone();
    // Before the pump starts: see `StartCtx::window_id`.
    publish_route(tab_id, ctx);
    pump::spawn_pump(tab_id.to_string(), events_rx, ctx.routes.clone(), shutdown);

    let deps = crate::rdp::embedded::client::DriverDeps {
        status,
        closed,
        events: events_tx,
        trust: trust_rx,
        size,
    };

    // The driver needs the input receiver; the handle owns the sender, so the
    // receiver is handed over here and never surfaces again.
    let driver = spawn_driver(
        format!("rdp-client-{tab_id}"),
        driver_alive,
        move || {
            // A current-thread runtime inside this thread: ironrdp's client task
            // then cannot starve the runtime serving SSH and the terminals.
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(crate::rdp::embedded::client::run_with_input(
                config,
                deps,
                input_rx,
            ));
        },
    )?;

    Ok(handle.with_driver(driver))
}

/// Start the built-in VNC client for `session`.
pub fn start_vnc_session(
    session: &Session,
    tab_id: &str,
    ctx: &StartCtx,
) -> Result<RemoteHandle, String> {
    let config = crate::vnc::config::VncConnectConfig::from_session(session, tab_id)?;
    let status = Arc::new(RemoteStatus::new(
        crate::rdp::embedded::status::RemoteState::Connecting,
    ));
    let closed = Arc::new(AtomicBool::new(false));
    let driver_alive = Arc::new(AtomicBool::new(true));
    let (events_tx, events_rx) = pump::event_channel();
    let (trust_tx, trust_rx) = mpsc::unbounded_channel::<(u64, bool, bool)>();
    let (input_tx, input_rx) = mpsc::unbounded_channel();

    let handle = RemoteHandle::new(
        tab_id,
        input_tx,
        events_tx.clone(),
        status.clone(),
        closed.clone(),
        trust_tx,
        config.follow_tab_size,
        RemoteProtocol::Vnc,
    );

    publish_route(tab_id, ctx);
    pump::spawn_pump(tab_id.to_string(), events_rx, ctx.routes.clone(), closed.clone());

    let deps = crate::vnc::client::DriverDeps {
        status,
        closed,
        events: events_tx,
        trust: trust_rx,
    };

    let driver = spawn_driver(
        format!("vnc-client-{tab_id}"),
        driver_alive,
        move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(crate::vnc::client::run_with_input(
                config,
                deps,
                input_rx,
            ));
        },
    )?;

    Ok(handle.with_driver(driver))
}

/// Deliver a trust decision to a live session and record it if the user asked.
///
/// The write goes through the trust module rather than a driver callback so it
/// happens exactly once, keyed by the prompt id — and so a decision made in a
/// tab that is being closed still reaches the store.
pub fn remote_resolve_trust(
    handle: &RemoteHandle,
    request_id: u64,
    accepted: bool,
    remember: bool,
) {
    handle.resolve_trust(request_id, accepted, remember);
    if remember {
        let _ = crate::remote::trust::resolve_decision(request_id, accepted, remember);
    }
    if !accepted {
        // Nothing downstream will accept a rejected certificate, so stop the
        // session here rather than leaving a task waiting on a socket.
        handle.close();
    }
}

/// Stop a session and drop its pane row.
///
/// **Waits for the driver to finish**, rather than only asking it to
/// (#remote-close-leftover 2026-10-27). `close()` is a flag: it is seen by the
/// driver at its next tick, then the driver sends the disconnect PDU, waits up
/// to 400 ms for the server, drops the client and returns. The tab used to be
/// considered closed the moment the flag was set, so the window came back with
/// the session still tearing down — and a reconnect in that window (or right
/// after a "关闭 tab 再连") met a server that still held the old session:
/// 「用户的另一个会话被阻止并且通知了本地会话管理器」. It also left the old pump
/// delivering one more event into a pane row that had already been removed.
///
/// A session whose driver has already ended — the common case, since the tab
/// only offers a reconnect once it has — is not waited for at all. The bound
/// exists for the other cases; what it is not is a kill switch.
pub fn close_session(handle: &RemoteHandle, panes: &RemotePanes) {
    handle.close();
    if !handle.wait_for_driver(CLOSE_WAIT) {
        tracing::warn!(
            "remote desktop {} did not stop within {:?}; dropping its row anyway",
            handle.tab_id(),
            CLOSE_WAIT
        );
    }
    panes.close(handle.tab_id());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RdpClientMode, SessionKind};

    #[test]
    fn only_the_embedded_rdp_mode_and_vnc_get_a_tab() {
        let mut session = Session::new_empty();
        session.kind = SessionKind::Rdp;
        // The default is the system client, which keeps its own window.
        assert!(!is_tab_hosted(&session));
        session.rdp_mode = RdpClientMode::Embedded;
        assert!(is_tab_hosted(&session));

        session.kind = SessionKind::Vnc;
        session.rdp_mode = RdpClientMode::SystemClient;
        assert!(
            is_tab_hosted(&session),
            "VNC has no system client, so rdp_mode must not affect it"
        );
    }

    #[test]
    fn a_non_remote_session_is_never_tab_hosted() {
        for kind in [
            SessionKind::Ssh,
            SessionKind::Serial,
            SessionKind::Telnet,
            SessionKind::Local,
        ] {
            let mut session = Session::new_empty();
            session.kind = kind;
            assert!(!is_tab_hosted(&session));
        }
    }

    #[test]
    fn the_tab_kind_matches_the_protocol() {
        let mut session = Session::new_empty();
        session.kind = SessionKind::Rdp;
        assert_eq!(tab_kind(&session), "rdp");
        session.kind = SessionKind::Vnc;
        assert_eq!(tab_kind(&session), "vnc");
    }

    #[test]
    fn waiting_for_a_driver_reports_when_it_is_really_gone() {
        let alive = Arc::new(AtomicBool::new(true));
        let mut driver = spawn_driver("test-driver".to_string(), alive, || {
            std::thread::sleep(std::time::Duration::from_millis(30));
        })
        .expect("spawn");
        // The wait itself is what `close_session` does; the thread must have
        // finished by the time it returns "true".
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while driver.is_alive() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(!driver.is_alive(), "the driver must report that it finished");
        driver.join();
    }

    /// A driver that never returns must not be waited for longer than the bound.
    #[test]
    fn a_handle_with_no_driver_needs_no_waiting() {
        let (input_tx, _input_rx) = mpsc::unbounded_channel();
        let (event_tx, _event_rx) = pump::event_channel();
        let (trust_tx, _trust_rx) = mpsc::unbounded_channel();
        let handle = RemoteHandle::new(
            "tab-1",
            input_tx,
            event_tx,
            Arc::new(RemoteStatus::new(
                crate::rdp::embedded::status::RemoteState::Connecting,
            )),
            Arc::new(AtomicBool::new(false)),
            trust_tx,
            true,
            RemoteProtocol::Rdp,
        );
        assert!(
            handle.wait_for_driver(std::time::Duration::from_millis(1)),
            "nothing to wait for must read as finished, not as a timeout"
        );
    }
}
