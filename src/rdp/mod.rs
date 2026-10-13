//! Launching RDP sessions — in a system client, or in a tab.
//!
//! RDP has two ways to open, and this module is the fork between them:
//!
//! * **System client** (the behaviour MTerm shipped with, and still the
//!   default): MTerm never speaks the protocol. It stores the account details —
//!   host, port, user, password, domain, resolution — and hands them over, so
//!   the session opens in the client's own native window instead of one of our
//!   tabs.
//!   * Windows: with no saved password, start `mstsc /v:host` so its existing
//!     credentials and sign-in settings work just as they do when launched by
//!     hand. With a saved password, use a generated `.rdp` file: `mstsc` refuses
//!     passwords on its command line, so the file holds a DPAPI-encrypted blob
//!     for the current user. A failed encryption makes the client prompt instead.
//!   * Linux / macOS / BSD: FreeRDP's `xfreerdp3` / `xfreerdp`, with the
//!     password handed over on stdin so it never shows up in the process list.
//!     The Flatpak bundle builds FreeRDP into `/app` (see `packaging/flatpak`),
//!     so no separate installation is needed there.
//!
//! * **Built-in client** (`RdpClientMode::Embedded`): MTerm speaks RDP itself
//!   with ironrdp and renders the desktop into a tab. The protocol work lives in
//!   `embedded/`, and the tab hosting it needs lives in [`crate::remote`] —
//!   which VNC shares, since the two produce the same events and consume the
//!   same input. A blank password field is resolved against the Credential
//!   Manager (`TERMSRV/<host>`, the entry the system client gets for free by
//!   handing the connection to `mstsc`) before the session is built, so both
//!   modes read a blank field the same way (#credential-store 2026-10-13).
//!
//! The fork is [`Session::rdp_mode`], and it is the *only* place the two meet:
//! [`launch`] keeps its signature and its system-client body byte-for-byte, so a
//! session file written before the built-in client existed still resolves to
//! `SystemClient` and behaves exactly as it did.
//!
//! 模块组织（两种客户端平级，`launch` 是唯一交汇点）：
//!   native/ —— 系统客户端：mstsc / FreeRDP 启动、.rdp 文件、DPAPI 密码加密、
//!     stdin 凭据、早退检测
//!   embedded/ —— 内置客户端（ironrdp）：配置、状态机、按键映射、证书信任
//! 两个协议常量（端口、最小桌面边长）定义在本文件：内置客户端经顶层再导出
//! 共用，不归属任何一侧。
use crate::config::{RdpClientMode, Session};
use crate::i18n::t;

pub(crate) mod embedded;
pub(crate) mod native;

pub(crate) use embedded::split_user_domain;
pub(crate) use native::Account;

/// The port RDP is assigned by IANA/MS-RDPBCGR.
pub(crate) const RDP_DEFAULT_PORT: u16 = 3389;

/// Smallest desktop side written into a connection file. Servers reject (or
/// clamp in surprising ways) tiny desktops, and the dialog can hand us anything.
pub(crate) const MIN_DESKTOP_SIDE: u16 = 200;

/// True when this RDP session is opened by MTerm's own client in a tab, rather
/// than by `mstsc` / `xfreerdp` in a separate window.
///
/// The single predicate the rest of the app asks, so the "which client" decision
/// is never re-derived (and never re-derived *differently*) at several call
/// sites.
pub(crate) fn is_embedded(session: &Session) -> bool {
    session.kind == crate::config::SessionKind::Rdp
        && session.rdp_mode == RdpClientMode::Embedded
}

/// Start the system remote desktop client for `session`.
///
/// This is the pre-existing entry point and its contract is unchanged: it is
/// called for a system-client session, `Ok` carries a short description of what
/// was launched (shown as a hint on the welcome page) and `Err` a
/// human-readable reason. The call is fire and forget — the client owns its own
/// window and lifetime.
///
/// Built-in-client sessions must **not** come here; they are routed to a tab by
/// `on_connect_session` instead. The guard below makes that a no-op rather than
/// a silent mstsc launch if a caller ever gets it wrong, which would be the one
/// failure mode that could quietly reopen the behaviour this feature replaced.
pub(crate) fn launch(session: &Session) -> Result<String, String> {
    if is_embedded(session) {
        return Err(t(
            "内置客户端会话应由标签页承载，不走系统客户端",
            "an embedded-client session is hosted in a tab, not by the system client",
        )
        .to_string());
    }
    let host = session.host.trim();
    if host.is_empty() {
        return Err(t("主机地址为空", "host is empty").to_string());
    }
    let port = if session.port == 0 {
        RDP_DEFAULT_PORT
    } else {
        session.port
    };
    // mstsc accepts both "DOMAIN\user" and "user@domain" in its user field; the
    // dialog's own domain field wins when it is filled in.
    let (user, implied_domain) = split_user_domain(session.user.trim());
    let domain = if session.rdp_domain.trim().is_empty() {
        implied_domain
    } else {
        session.rdp_domain.trim().to_string()
    };
    let account = Account {
        host,
        port,
        user: &user,
        domain: &domain,
        password: session.password.as_str(),
        fullscreen: session.rdp_fullscreen,
        width: session.rdp_width.max(MIN_DESKTOP_SIDE),
        height: session.rdp_height.max(MIN_DESKTOP_SIDE),
    };
    native::start_client(&account, &session.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RdpClientMode, SessionKind};

    /// The zero-regression guarantee, stated as a test: a session saved before
    /// the built-in client existed has no `rdp_mode` field, and it must still
    /// take the system-client path — the one `launch` serves.
    #[test]
    fn a_default_session_is_a_system_client_session() {
        let session = Session::new_empty();
        assert_eq!(session.rdp_mode, RdpClientMode::SystemClient);
        assert!(!is_embedded(&session));
    }

    #[test]
    fn only_an_explicitly_embedded_rdp_session_bypasses_the_system_client() {
        let mut session = Session::new_empty();
        session.kind = SessionKind::Rdp;
        session.rdp_mode = RdpClientMode::Embedded;
        assert!(is_embedded(&session));

        // The same field must not turn a *VNC* session into an RDP one.
        session.kind = SessionKind::Vnc;
        assert!(!is_embedded(&session));
    }

    #[test]
    fn launch_refuses_an_embedded_session_rather_than_opening_mstsc() {
        // Misrouting an embedded session to mstsc would silently restore the
        // exact behaviour this feature exists to make optional, so it is an
        // error instead of a fallback.
        let mut session = Session::new_empty();
        session.kind = SessionKind::Rdp;
        session.host = "10.0.0.9".into();
        session.rdp_mode = RdpClientMode::Embedded;
        assert!(launch(&session).is_err());
    }
}
