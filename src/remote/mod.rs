//! Hosting for the built-in remote-desktop tabs (RDP and VNC).
//!
//! This module owns everything that is *not* protocol-specific: the per-tab
//! state the UI renders, the frame hand-over from the I/O task to the UI thread,
//! and the trust store. Both protocol drivers (`crate::rdp::embedded` and
//! `crate::vnc`) produce the same [`RemoteEvent`] stream and consume the same
//! [`RemoteInput`] events, which is what lets one view, one tab lifecycle and
//! one status machine serve both.
//!
//! Module layout follows the project's `struct/` + `impls/` convention:
//!   struct/pane.rs    — `RemoteHandle` (the UI-side handle) + event types
//!   struct/registry.rs— UI-thread row updates against the window's own model
//!   struct/trust.rs   — persistent certificate/key trust decisions
//!   impls/render.rs   — frame → Slint image, and the letterbox coordinate map
//!   impls/pump.rs     — the I/O → UI crossing point
//!   impls/session.rs  — start / stop a session for a session definition
//!   impls/failure.rs  — raw failure text → one readable headline
//!
//! A **reconnect** is not a message to a driver: by the time the tab offers one,
//! the driver has ended and closed its sockets. It is a new session started in
//! the tab that already exists (`app::respawn_remote_tab`), which is why
//! nothing in this module owns it.

// `struct` is a Rust keyword, so these files cannot be addressed as
// `crate::remote::struct::x`. Each submodule is re-exported under its plain
// name below, and that is the path the rest of the crate uses.
#[path = "struct/pane.rs"]
pub(crate) mod pane;
#[path = "struct/trust.rs"]
pub(crate) mod trust;

#[path = "impls/failure.rs"]
pub(crate) mod failure;
#[path = "impls/keyinput.rs"]
pub(crate) mod keyinput;
#[path = "impls/pump.rs"]
pub(crate) mod pump;
#[path = "impls/render.rs"]
pub(crate) mod render;
#[path = "impls/session.rs"]
pub(crate) mod session;

pub(crate) use pane::{
    RemoteEvent, RemoteHandle, RemoteProtocol, TrustRequest, FRAME_QUEUE_DEPTH,
};
#[path = "struct/registry.rs"]
pub(crate) mod registry;
pub(crate) use registry::{
    apply_to, panes_of, register, status_for, unregister, RemotePanes,
};
pub(crate) use session::{
    close_session, remote_resolve_trust, start, start_rdp_session, start_vnc_session, tab_kind,
};
pub(crate) use session::is_tab_hosted;
pub(crate) use trust::{
    authorize as authorize_trust, open_decision, resolve_decision, sha256_fingerprint,
    trust_store_path, TrustOutcome, TrustStore,
};
