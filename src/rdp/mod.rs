//! Launching RDP sessions in a remote desktop client that already speaks RDP.
//!
//! RDP follows FinalShell's model: MTerm never implements the protocol
//! itself. It only stores the account details — host, port, user, password,
//! domain, resolution — and hands them over, so the session opens in the
//! client's own native window instead of one of our tabs.
//!
//! * Windows: with no saved password, start `mstsc /v:host` so its existing
//!   credentials and sign-in settings work just as they do when launched by
//!   hand. With a saved password, use a generated `.rdp` file: `mstsc` refuses
//!   passwords on its command line, so the file holds a DPAPI-encrypted blob
//!   for the current user. A failed encryption makes the client prompt instead.
//! * Linux / macOS / BSD: FreeRDP's `xfreerdp3` / `xfreerdp`, with the password
//!   handed over on stdin so it never shows up in the process list. The Flatpak
//!   bundle builds FreeRDP into `/app` (see `packaging/flatpak`), so no separate
//!   installation is needed there.
//!
//! 模块组织（对齐 ssh/terminal 的 impls + struct 惯例）：
//!   struct/account.rs —— 凭据/显示参数结构 + 端口常量
//!   impls/mstsc.rs —— Windows：mstsc 启动、.rdp 文件、DPAPI 密码加密
//!   impls/freerdp.rs —— FreeRDP 2/3 方言与命令行（跨平台编译）
//!   impls/freerdp_start.rs —— 非 Windows 启动、stdin 凭据、早退检测
use crate::config::Session;
use crate::i18n::t;

#[path = "struct/account.rs"]
mod account;
#[cfg(windows)]
#[path = "impls/mstsc.rs"]
mod mstsc;
#[cfg_attr(windows, allow(dead_code))]
#[path = "impls/freerdp.rs"]
mod freerdp;
#[cfg(not(windows))]
#[path = "impls/freerdp_start.rs"]
mod freerdp_start;

#[cfg(windows)]
use mstsc::start_client;
#[cfg(not(windows))]
use freerdp_start::start_client;
pub(crate) use account::{Account, MIN_DESKTOP_SIDE, RDP_DEFAULT_PORT};

/// Start the system remote desktop client for `session`.
///
/// `Ok` carries a short description of what was launched (shown as a hint on
/// the welcome page), `Err` a human-readable reason. This call is fire and
/// forget: the client owns its own window and lifetime.
pub(crate) fn launch(session: &Session) -> Result<String, String> {
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
    start_client(&account, &session.id)
}

/// Split `DOMAIN\user` / `user@domain` into its parts; plain names pass through.
fn split_user_domain(user: &str) -> (String, String) {
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
    use super::*;

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
        assert_eq!(
            split_user_domain(""),
            (String::new(), String::new())
        );
    }
}
