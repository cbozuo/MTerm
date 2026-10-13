//! MTerm's built-in remote-desktop clients (RDP and VNC) hosted inside a tab.
//!
//! The same protocol has two ways to open in RDP: the system client
//! (`mstsc` / `xfreerdp`) keeps its own window and is reached through
//! `crate::rdp::launch`, while this module is the built-in alternative that
//! renders the desktop into a tab. VNC has no system-client path, so it always
//! comes through here.
//!
//! Module layout follows the project's `struct/` + `impls/` convention:
//!   struct/config.rs  — connect parameters (built-in RDP)
//!   struct/status.rs  — the five-state connection machine, shared by both
//!   struct/input.rs   — protocol-independent input events + key translation
//!   impls/client.rs   — the ironrdp session driver
//!   impls/keys.rs     — Slint key → RDP set-1 scancode
//!   impls/verifier.rs — certificate trust (pre-flight TLS probe; see the file
//!                      header for why ironrdp 0.17 needs this shape)
//!   impls/credentials.rs — Windows Credential Manager lookup for a blank
//!                      password field (`TERMSRV/<host>`, what mstsc reads)

// The files live under `struct/` and `impls/` to match the project's layout
// convention, but `struct` is a Rust keyword, so a `crate::…::struct::x` path
// would not parse. Each submodule is therefore re-exported here under its plain
// name, and that is the path the rest of the crate uses.
#[path = "struct/config.rs"]
pub(crate) mod config;
#[path = "struct/status.rs"]
pub(crate) mod status;
#[path = "struct/input.rs"]
pub(crate) mod input;

#[path = "impls/client.rs"]
pub(crate) mod client;
#[path = "impls/keys.rs"]
pub(crate) mod keys;
#[path = "impls/verifier.rs"]
pub(crate) mod verifier;
#[path = "impls/credentials.rs"]
pub(crate) mod credentials;

pub(crate) use config::{desktop_size, EmbeddedRdpConfig};
pub(crate) use status::{RemoteState, RemoteStatus};
pub(crate) use input::{button, keysym, latin1_keysym, RemoteInput, SlintKey};

/// Split `DOMAIN\user` / `user@domain` into its parts; plain names pass through.
///
/// Lives here rather than in `mod.rs` because both clients need it: the system
/// client parses the user field for `mstsc`, and `EmbeddedRdpConfig` parses the
/// same field for ironrdp. One implementation is the point — if the two
/// disagreed, switching a session's connection mode would silently change which
/// account it authenticates as.
pub(crate) fn split_user_domain(user: &str) -> (String, String) {
    if let Some((domain, name)) = user.split_once('\\') {
        return (name.to_string(), domain.to_string());
    }
    if let Some((name, domain)) = user.split_once('@') {
        return (name.to_string(), domain.to_string());
    }
    (user.to_string(), String::new())
}

#[cfg(test)]
mod tests {
    use super::split_user_domain;

    #[test]
    fn user_and_domain_are_split_from_the_user_field() {
        assert_eq!(
            split_user_domain("CONTOSO\\alice"),
            ("alice".to_string(), "CONTOSO".to_string())
        );
        assert_eq!(
            split_user_domain("alice@contos.com"),
            ("alice".to_string(), "contos.com".to_string())
        );
        assert_eq!(
            split_user_domain("alice"),
            ("alice".to_string(), String::new())
        );
        assert_eq!(split_user_domain(""), (String::new(), String::new()));
    }
}
