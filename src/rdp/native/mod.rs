//! The system-client path: hand the session to `mstsc` (Windows) or `xfreerdp`
//! (everywhere else) and let the external program own its own window. MTerm
//! never speaks the protocol here — it only builds the account details and the
//! command line, so this half of the fork has no session state at all.
//!
//! The counterpart is [`crate::rdp::embedded`], where MTerm speaks RDP itself.
//! Both live under `rdp/` as sibling packages so the two ways to open an RDP
//! session read as one glance at the directory, and [`crate::rdp::launch`] is
//! the only place they meet.
//!
//! Layout is flat on purpose: four files do not need a `struct/` + `impls/`
//! split, and `Account` has exactly one consumer group (this one).

mod account;
#[cfg_attr(windows, allow(dead_code))] // only `freerdp_start` consumes it, and that is non-Windows
mod freerdp;
#[cfg(windows)]
mod mstsc;
#[cfg(not(windows))]
mod freerdp_start;

pub(crate) use account::Account;
// Re-exported through `rdp/` so the protocol constants have a single
// definition site (they are shared with the embedded client); `super::` paths
// inside this package resolve to the same items the rest of the crate sees.
pub(crate) use crate::rdp::{MIN_DESKTOP_SIDE, RDP_DEFAULT_PORT};

#[cfg(windows)]
pub(crate) use mstsc::start_client;
#[cfg(not(windows))]
pub(crate) use freerdp_start::start_client;
