//! Connection parameters for the built-in VNC client.

use crate::config::Session;

/// Default RFB port. 5900 is display `:0`; `:1` and up follow from it.
pub const VNC_DEFAULT_PORT: u16 = 5900;

/// Everything needed to (re)establish a VNC session.
#[derive(Debug, Clone)]
pub struct VncConnectConfig {
    pub tab_id: String,
    pub session_id: String,
    pub host: String,
    pub port: u16,
    /// RFB password, empty when the session has none (very common — many
    /// devices ship with authentication disabled).
    pub password: String,
    /// Whether a password was configured at all. Distinguishing "no password"
    /// from "empty password" matters: the former must not be turned into an
    /// authentication attempt with an empty secret.
    pub has_password: bool,
    /// `""` (auto), `"none"` or `"vnc-auth"`.
    pub security: String,
    /// Reject all input at the server (view-only session).
    pub view_only: bool,
    /// Whether to let other clients share the desktop.
    pub shared: bool,
    /// Mirror the clipboard between this machine and the remote one.
    pub clipboard: bool,
    /// Whether the remote desktop tracks the tab size.
    pub follow_tab_size: bool,
    /// Desktop size to request initially; servers that ignore it keep their own.
    pub width: u16,
    pub height: u16,
}

impl VncConnectConfig {
    /// Build the connect parameters from a saved session.
    ///
    /// VNC reuses `rdp_width` / `rdp_height` for its display size rather than
    /// adding a parallel pair: they mean exactly the same thing ("how big should
    /// the remote desktop be") and duplicating them would give the user two
    /// fields with one meaning.
    pub fn from_session(session: &Session, tab_id: &str) -> Result<Self, String> {
        let host = session.host.trim().to_string();
        if host.is_empty() {
            return Err(crate::i18n::t("主机地址为空", "host is empty").to_string());
        }
        let password = session.password.as_str().to_string();
        Ok(Self {
            tab_id: tab_id.to_string(),
            session_id: session.id.clone(),
            host,
            port: if session.port == 0 {
                VNC_DEFAULT_PORT
            } else {
                session.port
            },
            has_password: !password.is_empty(),
            password,
            security: session.vnc_security.trim().to_string(),
            view_only: session.vnc_view_only,
            // Sharing on by default: taking the desktop away from whoever is
            // already looking at it is the surprising behaviour.
            shared: true,
            clipboard: session.vnc_clipboard,
            follow_tab_size: session.rdp_follow_tab_size,
            width: session.rdp_width.max(200),
            height: session.rdp_height.max(200),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SessionKind;

    #[test]
    fn a_blank_host_is_rejected_before_any_socket_is_opened() {
        let session = Session::new_empty();
        assert!(VncConnectConfig::from_session(&session, "t").is_err());
    }

    #[test]
    fn port_zero_means_the_rfb_default() {
        let mut session = Session::new_empty();
        session.kind = SessionKind::Vnc;
        session.host = "10.0.0.30".into();
        session.port = 0;
        let cfg = VncConnectConfig::from_session(&session, "t").unwrap();
        assert_eq!(cfg.port, 5900);
    }

    #[test]
    fn an_absent_password_is_distinguished_from_an_empty_one() {
        let mut session = Session::new_empty();
        session.host = "h".into();
        let cfg = VncConnectConfig::from_session(&session, "t").unwrap();
        assert!(!cfg.has_password, "an unset password must not become an auth attempt");
    }

    #[test]
    fn display_size_is_reused_from_the_rdp_fields() {
        let mut session = Session::new_empty();
        session.host = "h".into();
        session.rdp_width = 0;
        session.rdp_height = 0;
        let cfg = VncConnectConfig::from_session(&session, "t").unwrap();
        // Clamped: a zero-sized framebuffer is not a thing.
        assert_eq!((cfg.width, cfg.height), (200, 200));
    }
}
