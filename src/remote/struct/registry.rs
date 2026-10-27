//! How a remote-desktop event reaches the tab that shows it.
//!
//! The frame pumps run on their own threads, and a Slint `VecModel` is not
//! `Send`, so no pump may hold one. The split this module enforces is:
//!
//! * **Pump thread** — holds a `u64` window id and an event, then hops onto the
//!   Slint event loop (see [`crate::remote::pump`]).
//! * **UI thread** — resolves that id to a [`RemotePanes`] and applies the row
//!   update. The resolution goes through a thread-local table because the
//!   `invoke_from_event_loop` closure *is* the UI thread, which is exactly when
//!   the table may be touched.
//!
//! The table is populated by `open_window`, which creates the `VecModel` and
//! owns it for the window's lifetime — so the model and the table can never
//! disagree about which rows exist.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use slint::{Model, ModelRc, VecModel};

use crate::rdp::embedded::status::RemoteState;
use crate::remote::pane::RemoteEvent;
use crate::remote::render;
use crate::ui::RemotePaneState;

/// One window's remote-desktop rows.
pub struct RemotePanes {
    /// Behind an `Rc` because that is what a `ModelRc` is built from; the model
    /// itself is not `Clone`, so this handle is the only way to hand the same
    /// rows to Slint as well.
    rows: Rc<VecModel<RemotePaneState>>,
    /// Tab ids whose session has ended. The tab stays (P0-10: a dropped
    /// connection must not destroy the tab) but the view offers a reconnect.
    finished: RefCell<std::collections::HashSet<String>>,
    /// Tab id → the request id of the trust prompt that tab is showing.
    ///
    /// The answer **must** echo this id. The driver re-checks it before letting
    /// the connection proceed, and the trust store is keyed by it, so an answer
    /// carrying anything else is neither applied nor remembered. It lives here
    /// rather than in the Slint row because the view has no use for it — only
    /// the Rust-side decide handler does, and that handler runs on this thread.
    trust_requests: RefCell<HashMap<String, u64>>,
    /// Tab id → the last desktop size the view asked for.
    ///
    /// Written by the same callback that drives `RemoteHandle::resize`, and read
    /// only when a session has to be brought back to the size the tab is at
    /// (a reconnect): the handle cannot remember it across sessions, and the
    /// view only reports on *change* (#rdp-resize-drop 2026-10-27).
    reported: RefCell<HashMap<String, (u16, u16)>>,
}

impl RemotePanes {
    /// Wrap the rows model created by the window.
    pub fn new(rows: Rc<VecModel<RemotePaneState>>) -> Self {
        Self {
            rows,
            finished: RefCell::new(std::collections::HashSet::new()),
            trust_requests: RefCell::new(HashMap::new()),
            reported: RefCell::new(HashMap::new()),
        }
    }

    /// The rows as the `ModelRc` a Slint property takes.
    ///
    /// Cloning the `Rc` and converting it gives a second handle to the *same*
    /// model rather than a copy of the rows, so a write through either side is
    /// visible through the other.
    pub fn model(&self) -> ModelRc<RemotePaneState> {
        self.rows.clone().into()
    }

    fn index_of(&self, tab_id: &str) -> Option<usize> {
        (0..self.rows.row_count()).find(|i| {
            self.rows
                .row_data(*i)
                .map(|row| row.id.as_str() == tab_id)
                .unwrap_or(false)
        })
    }

    /// Register a pane for `tab_id`, or reset it when the id is reused.
    pub fn open(&self, tab_id: &str, title: &str, kind: &str, view_only: bool) {
        self.finished.borrow_mut().remove(tab_id);
        // A fresh session has no prompt outstanding: a stale id would let an
        // answer to the old prompt resolve the new one.
        self.trust_requests.borrow_mut().remove(tab_id);
        if let Some(index) = self.index_of(tab_id) {
            if let Some(mut row) = self.rows.row_data(index) {
                row.title = title.into();
                row.kind = kind.into();
                row.state = i32::from(RemoteState::Connecting.as_u8());
                row.status = status_for(RemoteState::Connecting, None).into();
                row.error = "".into();
                row.image = Default::default();
                row.frame_w = 0;
                row.frame_h = 0;
                row.view_only = view_only;
                row.trust_open = false;
                row.trust_fingerprint = "".into();
                self.rows.set_row_data(index, row);
            }
            return;
        }
        self.rows.push(RemotePaneState {
            id: tab_id.into(),
            title: title.into(),
            kind: kind.into(),
            state: i32::from(RemoteState::Connecting.as_u8()),
            status: status_for(RemoteState::Connecting, None).into(),
            error: "".into(),
            image: Default::default(),
            frame_w: 0,
            frame_h: 0,
            view_only,
            trust_open: false,
            trust_protocol: "".into(),
            trust_host: "".into(),
            trust_fingerprint: "".into(),
            trust_subject: "".into(),
            trust_issuer: "".into(),
            trust_validity: "".into(),
            trust_changed: false,
        });
    }

    /// Reset a pane for a **reconnect**: same tab, same title, a fresh session.
    ///
    /// Distinct from [`RemotePanes::open`], which is for a tab that is being
    /// created or reused for a different session and therefore rewrites the
    /// title, the kind and `view_only`. A reconnect keeps all three — the user
    /// is re-dialling the same box — and only clears what belonged to the dead
    /// session: its row state, its failure text, its last picture and any
    /// prompt it was still showing.
    ///
    /// Clearing the picture is not cosmetic. State `>= 3` still draws the
    /// status overlay over the whole tab, but the moment the new session
    /// reports `Connected` that overlay goes away — and the desktop on screen
    /// must then be the new session's, not the old one's last frame.
    pub fn restart(&self, tab_id: &str) {
        self.finished.borrow_mut().remove(tab_id);
        // A fresh session has no prompt outstanding; a stale id would let an
        // answer to the old prompt resolve the new one.
        self.trust_requests.borrow_mut().remove(tab_id);
        let Some(index) = self.index_of(tab_id) else {
            return;
        };
        let Some(mut row) = self.rows.row_data(index) else {
            return;
        };
        row.state = i32::from(RemoteState::Connecting.as_u8());
        row.status = status_for(RemoteState::Connecting, None).into();
        row.error = "".into();
        row.image = Default::default();
        row.frame_w = 0;
        row.frame_h = 0;
        row.trust_open = false;
        row.trust_fingerprint = "".into();
        self.rows.set_row_data(index, row);
    }

    /// Show a failure on a pane **without a session behind it**.
    ///
    /// The one case is a reconnect whose session could not even be started
    /// (a configuration error). Without this the row would sit on "连接中..."
    /// forever with nothing driving it, which is worse than the error.
    pub fn fail(&self, tab_id: &str, reason: &str) {
        let Some(index) = self.index_of(tab_id) else {
            return;
        };
        let Some(mut row) = self.rows.row_data(index) else {
            return;
        };
        self.finished.borrow_mut().insert(tab_id.to_string());
        row.state = i32::from(RemoteState::Failed.as_u8());
        row.status = status_for(RemoteState::Failed, Some(reason)).into();
        row.error = reason.into();
        row.trust_open = false;
        self.rows.set_row_data(index, row);
    }

    /// Apply one event from a session. UI thread only.
    pub fn apply(&self, event: RemoteEvent, tab_id: &str) {
        let Some(index) = self.index_of(tab_id) else {
            // The tab was closed while a frame was in flight. Dropping it is
            // correct: there is nothing left to draw it on.
            return;
        };
        let Some(mut row) = self.rows.row_data(index) else {
            return;
        };
        match event {
            RemoteEvent::Frame(image) => {
                let (w, h) = image.dimensions();
                let slint_image = render::to_image(image);
                row.image = slint_image;
                row.frame_w = i32::from(w.min(u32::from(u16::MAX)) as u16);
                row.frame_h = i32::from(h.min(u32::from(u16::MAX)) as u16);
            }
            RemoteEvent::Size { width, height } => {
                // (#rdp-resize-drop 2026-10-27) The desktop size the session is
                // moving to, before the first frame of that size exists. Only
                // the geometry changes: the picture stays exactly as it was
                // until the server's next frame replaces it, and `apply` writes
                // the whole row back, so dropping the image here would be the
                // same "frozen until reconnect" hole the driver just closed.
                if width > 0 && height > 0 {
                    row.frame_w = i32::from(width);
                    row.frame_h = i32::from(height);
                }
            }
            RemoteEvent::State { state, message } => {
                // The status object is the driver's own view; the event carries
                // the snapshot it published, so the UI reads that rather than
                // reaching back across the thread boundary.
                let kind = state.state();
                let note = message.or_else(|| state.message());
                if kind != RemoteState::Connected {
                    self.finished.borrow_mut().insert(tab_id.to_string());
                }
                row.state = i32::from(kind.as_u8());
                row.status = status_for(kind, note.as_deref()).into();
                // The driver's text, verbatim: this is the only place the exact
                // failure survives intact, so it is kept as-is and shown by the
                // view behind a details toggle (#failure-wrap 2026-10-13).
                row.error = if kind == RemoteState::Failed {
                    note.unwrap_or_default().into()
                } else {
                    "".into()
                };
            }
            RemoteEvent::TrustRequest(request) => {
                // Recorded before anything else so the decide handler can echo
                // it back; without it that handler has no id to send and the
                // driver refuses the answer (and the store never learns the
                // certificate).
                self.trust_requests
                    .borrow_mut()
                    .insert(tab_id.to_string(), request.request_id);
                row.trust_open = true;
                row.trust_protocol = request.protocol.into();
                row.trust_host = format!("{}:{}", request.host, request.port).into();
                row.trust_fingerprint = request.fingerprint.into();
                row.trust_subject = request.subject.clone().unwrap_or_default().into();
                row.trust_issuer = request.issuer.clone().unwrap_or_default().into();
                row.trust_validity = format!(
                    "{} — {}",
                    request.valid_from.clone().unwrap_or_default(),
                    request.valid_to.clone().unwrap_or_default()
                )
                .into();
                // A changed fingerprint is the one case that may be an
                // impersonation, so it gets its own wording and styling.
                row.trust_changed = request.known_status == "changed";
            }
            RemoteEvent::Clipboard(text) => {
                // P1 groundwork: the event is delivered and the UI has somewhere
                // to put it; the clipboard bridge lands with P1-1.
                tracing::debug!("remote clipboard update ({} bytes)", text.len());
            }
        }
        self.rows.set_row_data(index, row);
    }

    /// Forget a pane. Called when its tab closes.
    pub fn close(&self, tab_id: &str) {
        self.finished.borrow_mut().remove(tab_id);
        self.trust_requests.borrow_mut().remove(tab_id);
        self.reported.borrow_mut().remove(tab_id);
        if let Some(index) = self.index_of(tab_id) {
            self.rows.remove(index);
        }
    }

    /// Take a pane's row out, preserving its picture.
    pub fn take_row(&self, tab_id: &str) -> Option<RemotePaneState> {
        let index = self.index_of(tab_id)?;
        let row = self.rows.row_data(index)?;
        self.rows.remove(index);
        Some(row)
    }

    /// Install a row that arrived from another window.
    pub fn put_row(&self, row: RemotePaneState) {
        if let Some(index) = self.index_of(&row.id) {
            self.rows.remove(index);
        }
        self.rows.push(row);
    }

    /// The request id of the trust prompt `tab_id` is showing, if any.
    ///
    /// Read by the decide handler on the UI thread, immediately before it
    /// answers. `None` means there is no prompt to answer — reading it is not
    /// what consumes the id; [`RemotePanes::dismiss_trust`] is.
    pub fn trust_request_id(&self, tab_id: &str) -> Option<u64> {
        self.trust_requests.borrow().get(tab_id).copied()
    }

    /// Close the trust overlay without answering.
    pub fn dismiss_trust(&self, tab_id: &str) {
        self.trust_requests.borrow_mut().remove(tab_id);
        if let Some(index) = self.index_of(tab_id) {
            if let Some(mut row) = self.rows.row_data(index) {
                row.trust_open = false;
                row.trust_fingerprint = "".into();
                self.rows.set_row_data(index, row);
            }
        }
    }

    /// The last desktop size the view asked for, so a reconnected session can be
    /// brought to the size the tab is at now.
    ///
    /// The view reports its size on every size/state change, but it de-duplicates
    /// per handle (`RemoteHandle::resize`) and only talks to the *current*
    /// session — after a reconnect the new handle starts with nothing, and a tab
    /// that has not changed size since will never report again. This is the row's
    /// copy, written by [`RemotePanes::remember_size`] from that same callback.
    pub fn reported_size(&self, tab_id: &str) -> Option<(u16, u16)> {
        self.reported.borrow().get(tab_id).copied()
    }

    /// Record the size the view asked this tab's session for.
    pub fn remember_size(&self, tab_id: &str, width: u16, height: u16) {
        if width == 0 || height == 0 {
            return;
        }
        self.reported
            .borrow_mut()
            .insert(tab_id.to_string(), (width, height));
    }

    /// Whether the session has ended, so the view can offer a reconnect.
    pub fn is_finished(&self, tab_id: &str) -> bool {
        self.finished.borrow().contains(tab_id)
    }

    /// Forget every tab at once, for a window that is going away.
    ///
    /// The per-tab [`RemotePanes::close`] is what a closing *tab* calls; a
    /// closing window has no tab events left to receive. What must not outlive
    /// it is the three maps, which are keyed by tab id: a tab id reused after a
    /// reconnect would otherwise find a stale prompt id or size, and
    /// `is_finished` would answer for a session that no longer exists.
    pub fn close_all(&self) {
        self.finished.borrow_mut().clear();
        self.trust_requests.borrow_mut().clear();
        self.reported.borrow_mut().clear();
        self.rows.set_vec(Vec::new());
    }
}

// ---------------------------------------------------------------------------
// The UI-thread table: window id → panes
// ---------------------------------------------------------------------------

thread_local! {
    // `HashMap::new` is not a `const fn`, so the default is built lazily.
    static PANES: RefCell<Option<HashMap<u64, Rc<RemotePanes>>>> = const { RefCell::new(None) };
}

/// Register a window's panes. Called once per window from `open_window`.
pub fn register(window_id: u64, panes: Rc<RemotePanes>) {
    PANES.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(HashMap::new)
            .insert(window_id, panes);
    });
}

/// Drop a window's panes. Called when the window closes.
pub fn unregister(window_id: u64) {
    PANES.with(|slot| {
        if let Some(table) = slot.borrow_mut().as_mut() {
            table.remove(&window_id);
        }
    });
}

/// The panes of window `window_id`, or `None` if it is gone.
///
/// UI thread only: `PANES` is a thread-local, which is the whole point — a pump
/// thread cannot read it, and must hop onto the event loop first.
pub fn panes_of(window_id: u64) -> Option<Rc<RemotePanes>> {
    PANES.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|table| table.get(&window_id).cloned())
    })
}

/// Apply an event to a window's panes, from the UI thread.
pub fn apply_to(window_id: u64, tab_id: &str, event: RemoteEvent) {
    if let Some(panes) = panes_of(window_id) {
        panes.apply(event, tab_id);
    }
}

/// The status line shown for a state transition.
///
/// One function so the tab title dot, the status line and the failure text can
/// never disagree about what a state means (P0-10).
///
/// On `Failed`, `message` is the driver's raw failure text — an NTSTATUS
/// decode, a rustls line, a socket error — and it is **classified before it is
/// shown** (#failure-wrap 2026-10-13): the line the user reads first comes from
/// [`crate::remote::failure`], while the raw text reaches the view through
/// `error` and stays one details toggle away. Returning it verbatim here is
/// what used to put a protocol trace at the top of an empty pane.
///
/// On `Disconnected`, `message` is a **reason the driver already reduced to a
/// sentence** (#remote-reconnect 2026-10-13), e.g. the peer hung up or the
/// session was taken over. It is appended rather than substituted, so the line
/// still says the connection is gone first. A driver that knows nothing adds
/// nothing, and the bare "已断线" comes back — the state was never untrue, it
/// was only ever uninformative when a reason was sitting right there.
pub fn status_for(state: RemoteState, message: Option<&str>) -> String {
    use crate::i18n::t;
    match state {
        RemoteState::Connecting => t("连接中...", "Connecting...").to_string(),
        RemoteState::Authenticating => t("正在认证...", "Authenticating...").to_string(),
        RemoteState::Connected => t("已连接", "Connected").to_string(),
        RemoteState::Reconnecting => t("重连中...", "Reconnecting...").to_string(),
        RemoteState::Disconnected => match message.map(str::trim).filter(|note| !note.is_empty()) {
            Some(note) => format!("{} — {note}", t("已断线", "Disconnected")),
            None => t("已断线", "Disconnected").to_string(),
        },
        RemoteState::Failed => message
            .map(crate::remote::failure::explain)
            .unwrap_or_else(|| t("连接失败", "Connection failed").to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trust_prompt_records_the_id_its_answer_must_echo() {
        // The UI glue used to send a hard-coded request id, which the driver
        // refuses; the id has to travel from the event to the decide handler.
        let panes = RemotePanes::new(Rc::new(VecModel::default()));
        panes.open("tab-1", "host", "rdp", false);
        assert_eq!(panes.trust_request_id("tab-1"), None, "no prompt yet");

        panes.apply(
            RemoteEvent::TrustRequest(crate::remote::pane::TrustRequest {
                request_id: 7,
                protocol: "rdp",
                host: "10.0.0.9".into(),
                port: 3389,
                fingerprint: "SHA256:bb".into(),
                subject: None,
                issuer: None,
                valid_from: None,
                valid_to: None,
                known_status: "unknown",
            }),
            "tab-1",
        );
        assert_eq!(panes.trust_request_id("tab-1"), Some(7));

        // The prompt belongs to one tab only.
        panes.open("tab-2", "other", "rdp", false);
        assert_eq!(panes.trust_request_id("tab-2"), None);

        // Answering consumes it, so a second click cannot answer twice.
        panes.dismiss_trust("tab-1");
        assert_eq!(panes.trust_request_id("tab-1"), None);
    }

    #[test]
    fn a_restarted_session_forgets_a_stale_prompt_id() {
        let panes = RemotePanes::new(Rc::new(VecModel::default()));
        panes.open("tab-1", "host", "rdp", false);
        panes.apply(
            RemoteEvent::TrustRequest(crate::remote::pane::TrustRequest {
                request_id: 9,
                protocol: "rdp",
                host: "10.0.0.9".into(),
                port: 3389,
                fingerprint: "SHA256:cc".into(),
                subject: None,
                issuer: None,
                valid_from: None,
                valid_to: None,
                known_status: "unknown",
            }),
            "tab-1",
        );
        assert_eq!(panes.trust_request_id("tab-1"), Some(9));
        // Reconnect reuses the tab id; an answer to the old prompt must not be
        // able to resolve the new session's prompt.
        panes.restart("tab-1");
        assert_eq!(panes.trust_request_id("tab-1"), None);
    }

    #[test]
    fn a_frame_size_is_known_before_the_first_bite_of_the_new_desktop() {
        // (#rdp-resize-drop 2026-10-27) The driver reports the size it is
        // moving to as soon as it asks for it, because the next frame may be a
        // whole reconnect away. The old picture stays: a blank tab in between is
        // the symptom being fixed.
        let panes = RemotePanes::new(Rc::new(VecModel::default()));
        panes.open("tab-1", "host", "rdp", false);
        panes.apply(RemoteEvent::Frame(image::RgbaImage::new(1280, 720)), "tab-1");
        panes.apply(RemoteEvent::Size { width: 2560, height: 1400 }, "tab-1");
        let row = panes.rows.row_data(0).expect("the pane is there");
        assert_eq!((row.frame_w, row.frame_h), (2560, 1400));
        assert!(
            row.image.size().width > 0,
            "the previous picture must survive until a new frame replaces it"
        );
        // A degenerate size (a collapsed tab on the way through) changes
        // nothing: it would make the pointer mapping divide by zero.
        panes.apply(RemoteEvent::Size { width: 0, height: 0 }, "tab-1");
        let row = panes.rows.row_data(0).expect("the pane is there");
        assert_eq!((row.frame_w, row.frame_h), (2560, 1400));
    }

    #[test]
    fn every_state_has_a_status_line_and_a_failure_headline_is_not_the_raw_text() {
        for state in [
            RemoteState::Connecting,
            RemoteState::Authenticating,
            RemoteState::Connected,
            RemoteState::Reconnecting,
            RemoteState::Disconnected,
            RemoteState::Failed,
        ] {
            assert!(
                !status_for(state, Some("why")).is_empty(),
                "{state:?} needs a status line"
            );
        }
        // `why` matches nothing the classifier knows, so the headline is the
        // generic line — the raw text goes to `error`, not to the headline
        // (#failure-wrap 2026-10-13).
        assert_eq!(
            status_for(RemoteState::Failed, Some("why")),
            crate::i18n::t("连接失败", "Connection failed")
        );
    }

    #[test]
    fn a_failure_without_a_reason_still_says_something_useful() {
        assert!(!status_for(RemoteState::Failed, None).is_empty());
    }

    #[test]
    fn a_disconnect_names_the_reason_the_driver_gave() {
        // "已断线" on its own was true and useless: the driver knows whether the
        // peer hung up or the session was taken over, and used to throw it away
        // (#remote-reconnect 2026-10-13).
        let line = status_for(RemoteState::Disconnected, Some("服务器断开了连接"));
        assert!(line.starts_with(crate::i18n::t("已断线", "Disconnected")), "{line}");
        assert!(line.ends_with("服务器断开了连接"), "{line}");
    }

    #[test]
    fn a_disconnect_without_a_reason_keeps_the_bare_line() {
        // Nothing to say beyond the state, and a blank or whitespace "reason"
        // is nothing — it must not turn into a dangling " — ".
        let bare = crate::i18n::t("已断线", "Disconnected").to_string();
        assert_eq!(status_for(RemoteState::Disconnected, None), bare);
        assert_eq!(status_for(RemoteState::Disconnected, Some("")), bare);
        assert_eq!(status_for(RemoteState::Disconnected, Some("   ")), bare);
    }

    #[test]
    fn a_reconnect_clears_the_dead_session_and_keeps_the_tab_identity() {
        let panes = RemotePanes::new(Rc::new(VecModel::default()));
        panes.open("tab-1", "生产库", "rdp", true);
        // The dead session's last word: a failure plus the picture it drew.
        let status = std::sync::Arc::new(crate::rdp::embedded::status::RemoteStatus::new(
            RemoteState::Failed,
        ));
        status.set(RemoteState::Failed, Some("boom".into()));
        panes.apply(
            RemoteEvent::State {
                state: status,
                message: Some("boom".into()),
            },
            "tab-1",
        );
        panes.apply(
            RemoteEvent::Frame(image::RgbaImage::new(64, 48)),
            "tab-1",
        );

        panes.restart("tab-1");
        let row = panes.rows.row_data(0).expect("the tab is still there");
        assert_eq!(row.state, i32::from(RemoteState::Connecting.as_u8()));
        assert_eq!(row.error, "", "the stale failure text must not survive");
        assert_eq!(row.frame_w, 0, "the stale picture must not survive");
        assert_eq!(row.id, "tab-1");
        // A reconnect is the *same* session, so its title and read-only flag
        // are the user's, not the reconnect's, to change.
        assert_eq!(row.title, "生产库");
        assert!(row.view_only);
    }

    #[test]
    fn a_reconnect_that_cannot_start_says_so_instead_of_hanging_on_connecting() {
        let panes = RemotePanes::new(Rc::new(VecModel::default()));
        panes.open("tab-1", "host", "rdp", false);
        panes.restart("tab-1");
        panes.fail("tab-1", "credssp 0xc000006d");
        let row = panes.rows.row_data(0).expect("the tab is still there");
        assert_eq!(row.state, i32::from(RemoteState::Failed.as_u8()));
        assert_eq!(row.error, "credssp 0xc000006d");
    }
}
