//! Starting, stopping and steering a remote-desktop session for a tab.
//!
//! This is the single place that knows how a `Session` definition becomes a live
//! tab: it builds the protocol parameters, creates the channels, spawns the
//! driver thread and the pump thread, and publishes the routes both of them read.
//! The two protocol drivers stay unaware of Slint, and the UI stays unaware of
//! ironrdp/vnc-rs.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::config::{Session, SessionKind};
use crate::remote::pump::{self, RemoteRoutes};
use crate::remote::pane::{RemoteEvent, RemoteHandle, RemoteProtocol};
use crate::remote::registry::RemotePanes;
use crate::rdp::embedded::config::EmbeddedRdpConfig;
use crate::rdp::embedded::status::RemoteStatus;

/// Everything the starter needs from the window that owns the tab.
pub struct StartCtx {
    pub routes: RemoteRoutes,
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
    std::thread::Builder::new()
        .name(format!("rdp-client-{tab_id}"))
        .spawn(move || {
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
        })
        .map_err(|err| format!("{err}"))?;

    Ok(handle)
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

    pump::spawn_pump(tab_id.to_string(), events_rx, ctx.routes.clone(), closed.clone());

    let deps = crate::vnc::client::DriverDeps {
        status,
        closed,
        events: events_tx,
        trust: trust_rx,
    };

    std::thread::Builder::new()
        .name(format!("vnc-client-{tab_id}"))
        .spawn(move || {
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
        })
        .map_err(|err| format!("{err}"))?;

    Ok(handle)
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
pub fn close_session(handle: &RemoteHandle, panes: &RemotePanes) {
    handle.close();
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
}
