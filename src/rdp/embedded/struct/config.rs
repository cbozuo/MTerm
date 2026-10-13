//! Connection parameters for the built-in RDP client.
//!
//! One value carries everything the ironrdp session loop needs, so a reconnect
//! after a transport drop reuses the exact same settings instead of threading
//! ten arguments through every layer.

use crate::config::Session;

/// Everything needed to (re)establish a built-in RDP session.
#[derive(Debug, Clone)]
pub struct EmbeddedRdpConfig {
    /// Tab id, so state and frames can be routed back to the right desktop.
    pub tab_id: String,
    /// Saved-session id (the sidebar/tab grouping key).
    pub session_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: String,
    pub password: String,
    /// Desktop size requested on connect.
    pub width: u16,
    pub height: u16,
    /// Ask the server to resize the desktop when the tab is resized.
    pub follow_tab_size: bool,
    /// Colour depth requested from the server (16/24/32).
    pub color_depth: u8,
    /// How to treat an unknown or changed server certificate:
    /// `"prompt"` (default), `"accept-new"` (ask only for unknown hosts) or
    /// `"strict"` (refuse anything not already trusted).
    pub certificate_policy: String,
}

impl EmbeddedRdpConfig {
    /// Build the connect parameters from a saved session.
    ///
    /// Account fields are shared verbatim with the system-client path — the
    /// built-in client must not invent its own parsing, or switching the mode
    /// in the edit dialog would quietly change the meaning of the fields.
    pub fn from_session(session: &Session, tab_id: &str) -> Result<Self, String> {
        let host = session.host.trim().to_string();
        if host.is_empty() {
            return Err(crate::i18n::t("主机地址为空", "host is empty").to_string());
        }
        let port = if session.port == 0 {
            crate::rdp::RDP_DEFAULT_PORT
        } else {
            session.port
        };
        let (user, implied_domain) = super::split_user_domain(session.user.trim());
        let domain = if session.rdp_domain.trim().is_empty() {
            implied_domain
        } else {
            session.rdp_domain.trim().to_string()
        };
        // "follow the tab" has no size of its own to read: the tab has not been
        // measured yet when this runs, so the connect goes out at the stored
        // display settings as a *guess*. The view corrects it — `report-size` in
        // `ui/remote_desktop_view.slint` sends the real tab size in device pixels
        // once the session is connected, and the driver resizes from there.
        let (width, height) = desktop_size(session);
        Ok(Self {
            tab_id: tab_id.to_string(),
            session_id: session.id.clone(),
            host,
            port,
            username: user,
            domain,
            password: session.password.as_str().to_string(),
            width,
            height,
            follow_tab_size: session.rdp_follow_tab_size,
            color_depth: 32,
            certificate_policy: "prompt".to_string(),
        })
    }
}

/// The desktop size to request for `session`.
///
/// `rdp_fullscreen` and the resolution dropdown feed `rdp_width` / `rdp_height`
/// exactly as they do for the system client, so the same three choices
/// (full screen / preset / custom) mean the same thing in both modes.
pub fn desktop_size(session: &Session) -> (u16, u16) {
    if session.rdp_fullscreen {
        return (1920, 1080);
    }
    (
        session.rdp_width.max(crate::rdp::MIN_DESKTOP_SIDE),
        session.rdp_height.max(crate::rdp::MIN_DESKTOP_SIDE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthMethod, SessionKind};

    #[test]
    fn embedded_config_splits_user_domain_like_the_system_client() {
        let mut session = Session::new_empty();
        session.kind = SessionKind::Rdp;
        session.host = "10.0.0.9".into();
        session.port = 0;
        session.user = "CONTOSO\\alice".into();
        session.auth = AuthMethod::Password;
        let cfg = EmbeddedRdpConfig::from_session(&session, "tab-1").unwrap();
        // port 0 means "use the protocol default", exactly as launch() does.
        assert_eq!(cfg.port, crate::rdp::RDP_DEFAULT_PORT);
        assert_eq!(cfg.username, "alice");
        assert_eq!(cfg.domain, "CONTOSO");
        assert_eq!(cfg.tab_id, "tab-1");
    }

    #[test]
    fn an_explicit_domain_beats_the_one_implied_by_the_username() {
        let mut session = Session::new_empty();
        session.host = "10.0.0.9".into();
        session.user = "alice@contos.com".into();
        session.rdp_domain = "OTHER".into();
        let cfg = EmbeddedRdpConfig::from_session(&session, "t").unwrap();
        assert_eq!(cfg.domain, "OTHER");
    }

    #[test]
    fn a_blank_host_is_rejected_before_any_socket_is_opened() {
        let session = Session::new_empty();
        assert!(EmbeddedRdpConfig::from_session(&session, "t").is_err());
    }
}
