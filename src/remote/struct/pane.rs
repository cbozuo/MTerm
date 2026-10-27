//! Per-tab state for a remote desktop hosted in a tab.
//!
//! One of these exists for every open RDP (built-in client) or VNC tab. It is
//! owned by the UI thread — the I/O task only ever reaches it through a
//! `RemoteHandle`, whose channels are the single crossing point between the two
//! worlds (P0-5/P0-6).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::rdp::embedded::input::RemoteInput;
use crate::rdp::embedded::status::RemoteStatus;

/// Frames may be queued this many deep before the oldest is dropped.
///
/// A desktop redraws far faster than the UI thread can upload a texture, so an
/// unbounded queue would grow without bound and show the desktop seconds behind
/// the user's input. Two frames of slack keeps the pipeline full; anything older
/// than that is already stale by definition.
pub const FRAME_QUEUE_DEPTH: usize = 2;

/// What the I/O task produced, handed across to the UI thread.
#[derive(Debug)]
pub enum RemoteEvent {
    /// A new frame, already converted to something Slint can draw.
    Frame(image::RgbaImage),
    /// The connection state changed (or the session ended). The status object
    /// is shared rather than copied so the UI can read the driver's own view
    /// (state + message) without a second channel carrying the same facts.
    State {
        state: Arc<RemoteStatus>,
        /// Free-form note; the UI turns it into the status line and, on
        /// failure, the reason text.
        message: Option<String>,
    },
    /// A server certificate or key needs a trust decision. The tab stays in
    /// `Authenticating` until the UI answers through
    /// [`RemoteHandle::resolve_trust`].
    TrustRequest(TrustRequest),
    /// The session's own picture size changed — an RDP desktop resize (asked
    /// for with [`RemoteHandle::resize`], or forced onto the session by a
    /// reconnect at a new size).
    ///
    /// It has to travel on its own because `Frame` carries the pixels and no
    /// owner: the driver drops its last picture the moment it asks for a
    /// resize, and the frame that follows carries the new size anyway. This
    /// event is what keeps the row's `frame-w`/`frame-h` — and therefore the
    /// view's letterbox and its pointer mapping — truthful during that gap,
    /// while the tab is stopped on "reconnect…" and no frame will arrive for
    /// a while (#rdp-resize-drop 2026-10-27).
    Size { width: u16, height: u16 },
    /// The remote clipboard changed (VNC's server-cut-text).
    Clipboard(String),
}

/// A pending certificate/key trust decision (P0-12).
#[derive(Debug, Clone)]
pub struct TrustRequest {
    /// Correlates the prompt with the answer.
    pub request_id: u64,
    /// `"rdp"` or `"vnc"`, so the UI can word the prompt for the protocol.
    pub protocol: &'static str,
    pub host: String,
    pub port: u16,
    /// `"SHA256:..."` of the server certificate / key.
    pub fingerprint: String,
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    /// `"unknown"` (never seen), `"changed"` (seen with a different
    /// fingerprint) or `"match"` (already trusted).
    pub known_status: &'static str,
}

/// Which protocol a tab's session speaks.
///
/// Recorded on the handle rather than looked up from the tab model: the input
/// callbacks receive only a tab id, and a lookup would have to go through the UI
/// model from the UI thread while the answer is also needed by the driver. One
/// field on the handle is both cheaper and impossible to get out of step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteProtocol {
    /// Built-in RDP (ironrdp).
    Rdp,
    /// VNC (vnc-rs).
    Vnc,
}

impl RemoteProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            RemoteProtocol::Rdp => "rdp",
            RemoteProtocol::Vnc => "vnc",
        }
    }
}

/// The UI-thread side of one remote-desktop tab.
///
/// Cheap to clone: it is only a set of channel senders plus a few atomics, and
/// the callbacks capture clones.
///
/// The driver thread is **not** per-clone: it lives behind an `Arc<Mutex<…>>`
/// and exactly one clone — the one the window keeps in its handle map — will ever
/// take it, which is what [`RemoteHandle::wait_for_driver`] documents. Cloning
/// therefore copies a handle to the same session, never a second session.
#[derive(Clone)]
pub struct RemoteHandle {
    tab_id: String,
    protocol: RemoteProtocol,
    input_tx: mpsc::UnboundedSender<RemoteInput>,
    event_tx: mpsc::UnboundedSender<RemoteEvent>,
    status: Arc<RemoteStatus>,
    closed: Arc<AtomicBool>,
    /// Answer a pending [`TrustRequest`].
    trust_tx: mpsc::UnboundedSender<(u64, bool, bool)>,
    /// Last size we asked the server for, so a resize to the same size is not
    /// re-sent on every relayout.
    last_size: Arc<std::sync::Mutex<(u16, u16)>>,
    /// Whether the remote desktop should track the tab size (P0-9).
    follow_tab_size: bool,
    /// Whether the driver thread is still running, and — once it has finished —
    /// the thread itself, so [`RemoteHandle::wait_for_driver`] can wait for the
    /// session to be really gone (#remote-close-leftover 2026-10-27).
    ///
    /// `None` until the session starter attaches it. That is also the shape a
    /// handle has when there is no thread to wait for at all (the driver was
    /// never spawned, or a test built the handle directly), and
    /// [`RemoteHandle::wait_for_driver`] treats the two alike: nothing to wait
    /// for.
    driver: Arc<std::sync::Mutex<Option<crate::remote::session::DriverThread>>>,
}

impl RemoteHandle {
    /// Assemble a handle from the channels the I/O task created.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tab_id: &str,
        input_tx: mpsc::UnboundedSender<RemoteInput>,
        event_tx: mpsc::UnboundedSender<RemoteEvent>,
        status: Arc<RemoteStatus>,
        closed: Arc<AtomicBool>,
        trust_tx: mpsc::UnboundedSender<(u64, bool, bool)>,
        follow_tab_size: bool,
        protocol: RemoteProtocol,
    ) -> Self {
        Self {
            tab_id: tab_id.to_string(),
            protocol,
            input_tx,
            event_tx,
            status,
            closed,
            trust_tx,
            last_size: Arc::new(std::sync::Mutex::new((0, 0))),
            follow_tab_size,
            driver: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Hand over the thread that drives this session.
    ///
    /// Exactly once, from the starter that spawned it. The handle cannot take it
    /// as a constructor argument because the thread is spawned *after* the
    /// handle exists (the driver is handed clones of its senders).
    pub fn with_driver(self, driver: crate::remote::session::DriverThread) -> Self {
        if let Ok(mut slot) = self.driver.lock() {
            *slot = Some(driver);
        }
        self
    }

    /// Wait (up to `timeout`) for the driver thread to finish, and report
    /// whether it has.
    ///
    /// `true` also means "there was nothing to wait for": a session that has
    /// already ended has no thread left, and treating that as a timeout would
    /// turn the ordinary close of a dead tab into a warning.
    ///
    /// The thread is taken rather than borrowed, because joining consumes it —
    /// and because this is the only place that will ever look at it again. A
    /// driver that is *still* running when the bound expires is deliberately
    /// **not** joined: that would block this UI-thread path for as long as the
    /// server takes to acknowledge the close, which is exactly what the bound
    /// exists to prevent. Its own `Drop` then stops it, and it notices `closed`
    /// on its own.
    pub fn wait_for_driver(&self, timeout: std::time::Duration) -> bool {
        let driver = self.driver.lock().ok().and_then(|mut slot| slot.take());
        let Some(mut driver) = driver else {
            return true;
        };
        let deadline = std::time::Instant::now() + timeout;
        while driver.is_alive() {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // The driver has finished, so joining cannot block for long — and it
        // makes "the tab is closed" mean its sockets are closed with it.
        driver.join();
        true
    }

    pub fn tab_id(&self) -> &str {
        &self.tab_id
    }

    /// The protocol this session speaks. Decides which key and pointer
    /// translation the input path uses.
    pub fn protocol(&self) -> RemoteProtocol {
        self.protocol
    }

    pub fn status(&self) -> Arc<RemoteStatus> {
        self.status.clone()
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// True when the remote desktop tracks the tab size.
    pub fn follows_tab_size(&self) -> bool {
        self.follow_tab_size
    }

    /// Queue one input event. Dropped silently when the session is gone: input
    /// is a firehose of best-effort events and a full or closed channel says
    /// the session is over anyway.
    pub fn send_input(&self, event: RemoteInput) {
        if self.is_closed() {
            return;
        }
        let _ = self.input_tx.send(event);
    }

    /// Tell the server to resize the desktop (P0-9).
    ///
    /// Ignored when the session does not follow the tab, and de-duplicated so a
    /// relayout that produces the same size does not put a `Resize` on the wire.
    /// `None` for a degenerate size (a collapsed tab) is silently dropped
    /// rather than clamped here — the clamp happens on the I/O side, where a
    /// transient 0 can also come from a window minimise.
    pub fn resize(&self, width: u16, height: u16) {
        if !self.follow_tab_size || self.is_closed() || width == 0 || height == 0 {
            return;
        }
        let Ok(mut last) = self.last_size.lock() else {
            return;
        };
        if *last == (width, height) {
            return;
        }
        *last = (width, height);
        drop(last);
        self.send_input(RemoteInput::Resize { width, height });
    }

    /// Answer a trust prompt. `remember` distinguishes "trust and save" from
    /// "this time only"; `accepted == false` blocks the connection.
    pub fn resolve_trust(&self, request_id: u64, accepted: bool, remember: bool) {
        let _ = self.trust_tx.send((request_id, accepted, remember));
    }

    /// Stop the session. Idempotent, so closing a tab twice is harmless.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(follow: bool) -> (RemoteHandle, mpsc::UnboundedReceiver<RemoteInput>) {
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        let (trust_tx, _t) = mpsc::unbounded_channel();
        (
            RemoteHandle::new(
                "tab-1",
                input_tx,
                event_tx,
                Arc::new(RemoteStatus::default()),
                Arc::new(AtomicBool::new(false)),
                trust_tx,
                follow,
                RemoteProtocol::Rdp,
            ),
            input_rx,
        )
    }

    #[tokio::test]
    async fn a_resize_is_only_sent_once_per_size() {
        let (h, mut rx) = handle(true);
        h.resize(1280, 720);
        h.resize(1280, 720);
        h.resize(1281, 721);
        assert_eq!(rx.recv().await, Some(RemoteInput::Resize { width: 1280, height: 720 }));
        assert_eq!(rx.recv().await, Some(RemoteInput::Resize { width: 1281, height: 721 }));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_session_that_does_not_follow_the_tab_ignores_resizes() {
        let (h, mut rx) = handle(false);
        h.resize(1280, 720);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_collapsed_tab_never_reaches_the_wire() {
        let (h, mut rx) = handle(true);
        // A window minimise briefly reports a 0-size content area; asking the
        // server for a 0x0 desktop would get the session dropped.
        h.resize(0, 0);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn input_stops_at_a_closed_session() {
        let (h, mut rx) = handle(true);
        h.close();
        h.send_input(RemoteInput::ReleaseAll);
        assert!(rx.try_recv().is_err());
        assert!(h.is_closed());
    }
}
