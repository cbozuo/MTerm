//! Naming a failure in one line a person can read.
//!
//! The drivers hand the UI the *raw* failure text, and that part is deliberate:
//! it says exactly what went wrong, and it is what a bug report needs. What it
//! is not is a first impression. The RDP driver flattens a `ConnectorError`
//! into `Kind: cause: cause` — a line that opens with the word `CredSSP` and
//! closes with an NTSTATUS code — and the VNC driver reports things like
//! `VNC TCP connect failed: ...`. Leading with that put a protocol trace in
//! the middle of an otherwise empty pane, which is what this module exists to
//! stop: the view now shows a readable headline here, and keeps the raw text
//! one "details" toggle away (see `ui/remote_desktop_view.slint`).
//!
//! Classification is by substring, on purpose. The text is built inside a
//! dependency, crosses the event channel as a plain `String`, and arrives with
//! no error type attached — there is nothing to match on but the words. A
//! pattern that stops matching therefore degrades to the generic headline
//! rather than to a wrong one, and the unclaimed text still reaches the user
//! under the toggle.
//!
//! Three rules keep that honest:
//!
//! * A line whose text already names a reason (an NTSTATUS code, a socket
//!   error) is classified even when it also carries Chinese — Windows
//!   localises its own socket errors, so `VNC TCP connect failed: 由于目标计算机
//!   积极拒绝… (os error 10061)` is a hybrid, and the code is the reliable part.
//! * A line this crate wrote itself is passed through untouched when nothing
//!   classifies: those lines (`"连接超时"`, the certificate-policy refusals in
//!   the RDP driver) were written for this spot and already read as sentences.
//! * An unrecognised failure gets the generic headline. It is never answered
//!   by echoing the raw text back — that is the behaviour being replaced.

use crate::i18n::t;

/// What the failure text says, as far as we can tell.
///
/// Deliberately small: every variant is something the user can act on, and
/// anything else folds into the generic line. Kept separate from the wording
/// so the classification is testable without depending on the UI language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    /// The logon was refused: wrong user name or password.
    Credentials,
    AccountLocked,
    AccountDisabled,
    /// Policy on the account itself (as opposed to a wrong password).
    AccountRestricted,
    /// Policy on *where* the account may sign in from.
    WorkstationRestricted,
    /// Policy on *when* the account may sign in.
    LogonHours,
    PasswordExpired,
    PasswordMustChange,
    /// Nothing answered the logon request.
    NoLogonServers,
    Timeout,
    Refused,
    Unreachable,
    Reset,
    Dns,
    /// TLS or CredSSP never completed. Kept behind the concrete reasons above:
    /// nearly every RDP failure text mentions CredSSP, so it has to be the
    /// last specific rule rather than the first.
    SecurityNegotiation,
    Certificate,
    /// (#vnc-auth-reasons 2026-10-27) VNC-specific, from vnc-rs's handshake:
    /// the server offered only security types this client cannot speak
    /// (RA2/TLS/SASL…), so connecting can never succeed no matter the retry.
    AuthMethodUnsupported,
    /// The server demands authentication, but the session has no password
    /// configured to answer it with.
    PasswordRequired,
}

/// One readable line naming the failure.
///
/// The caller shows this where the raw text used to go; the raw text itself
/// stays available to the view and is never withheld by this function.
pub(crate) fn explain(raw: &str) -> String {
    explain_optional(raw).unwrap_or_else(|| t("连接失败", "Connection failed").to_owned())
}

/// [`explain`], but `None` when the text names nothing this module can classify
/// and is not already a sentence.
///
/// For a failure that *is* the answer — "the connection failed" — the generic
/// headline is the right thing to say. For a session that had already connected
/// and then ended, it is not: "已断线 — 连接失败" says less than "已断线" did, so
/// the caller asks for the reason alone and adds nothing when there is none.
/// The raw text is never lost either way; it stays in the log and, on `Failed`,
/// behind the details toggle.
pub(crate) fn explain_optional(raw: &str) -> Option<String> {
    if let Some(reason) = classify(raw) {
        return Some(headline(reason).to_owned());
    }
    // Nothing classified. If it is already a sentence in the user's language,
    // it is one of ours and needs no second translation.
    if raw.chars().any(is_cjk) {
        return Some(raw.to_owned());
    }
    None
}

/// `Some` when the text names a reason worth reporting on its own.
fn classify(raw: &str) -> Option<Reason> {
    let text = raw.to_lowercase();

    // Logon verdicts first, and by NTSTATUS rather than by wording: a rejected
    // logon is the common case, its text also names CredSSP, and the symbolic
    // names (`STATUS_LOGON_FAILURE`) are printed next to the code but are the
    // part a future sspi could reword. The hex is the stable half.
    if contains(&text, &["0xc000006d", "status_logon_failure"]) {
        return Some(Reason::Credentials);
    }
    // (#vnc-auth-reasons 2026-10-27) VNC has no status codes; its verdicts are
    // these two library strings. Before everything transport-shaped, and before
    // the negotiation rule, for the same reason the NTSTATUS rules sit first.
    if contains(&text, &["wrong password"]) {
        return Some(Reason::Credentials);
    }
    if contains(&text, &["auth is required but no password provided"]) {
        return Some(Reason::PasswordRequired);
    }
    if contains(&text, &["security type apart from vnc auth"]) {
        return Some(Reason::AuthMethodUnsupported);
    }
    if contains(&text, &["0xc0000234", "status_account_locked_out"]) {
        return Some(Reason::AccountLocked);
    }
    if contains(&text, &["0xc0000072", "status_account_disabled"]) {
        return Some(Reason::AccountDisabled);
    }
    if contains(&text, &["0xc000006e", "status_account_restriction"]) {
        return Some(Reason::AccountRestricted);
    }
    if contains(&text, &["0xc0000070", "status_invalid_workstation"]) {
        return Some(Reason::WorkstationRestricted);
    }
    if contains(&text, &["0xc000006f", "status_invalid_logon_hours"]) {
        return Some(Reason::LogonHours);
    }
    if contains(&text, &["0xc0000071", "status_password_expired"]) {
        return Some(Reason::PasswordExpired);
    }
    if contains(&text, &["0xc0000224", "status_password_must_change"]) {
        return Some(Reason::PasswordMustChange);
    }
    if contains(&text, &["0xc000005e", "status_no_logon_servers"]) {
        return Some(Reason::NoLogonServers);
    }

    // Certificates before the transport rules: a rustls refusal mentions both
    // the certificate and the handshake, and the certificate is the part the
    // user has to decide about.
    if contains(&text, &["certificate"]) {
        return Some(Reason::Certificate);
    }

    // Transport. The `os error` numbers are the Windows socket codes
    // (WSAETIMEDOUT 10060, WSAECONNREFUSED 10061, WSAECONNRESET 10054,
    // WSAEHOSTUNREACH 10065, WSAENETUNREACH 10051, WSAHOST_NOT_FOUND 11001);
    // the phrasing around them is localised, the number is not. Each number
    // sits with its own meaning — 10061 is a refusal, not a timeout.
    if contains(&text, &["refused", "os error 10061"]) {
        return Some(Reason::Refused);
    }
    if contains(&text, &["timed out", "timeout", "os error 10060"]) {
        return Some(Reason::Timeout);
    }
    if contains(&text, &["connection reset", "os error 10054"]) {
        return Some(Reason::Reset);
    }
    if contains(
        &text,
        &["unreachable", "no route to host", "os error 10065", "os error 10051"],
    ) {
        return Some(Reason::Unreachable);
    }
    if contains(
        &text,
        &[
            "failed to lookup",
            "name or service not known",
            "os error 11001",
            "os error 11004",
        ],
    ) {
        return Some(Reason::Dns);
    }

    // Everything else that mentions the security negotiation. Last, because
    // `CredSSP` appears in the text of failures this module already classified.
    if contains(&text, &["credssp", "nla", "negotiat"]) {
        return Some(Reason::SecurityNegotiation);
    }

    None
}

/// The readable line for a reason. `t` returns the variant for the current
/// language, so both sides stay in step with the rest of the UI's text.
fn headline(reason: Reason) -> &'static str {
    match reason {
        Reason::Credentials => t("用户名或密码不正确", "Wrong username or password"),
        Reason::AccountLocked => t("账户已被锁定", "The account is locked out"),
        Reason::AccountDisabled => t("账户已被禁用", "The account is disabled"),
        Reason::AccountRestricted => {
            t("该账户不允许远程登录", "This account may not sign in remotely")
        }
        Reason::WorkstationRestricted => t(
            "该账户不允许从这台电脑登录",
            "This account may not sign in from this computer",
        ),
        Reason::LogonHours => t(
            "该账户当前时段不允许登录",
            "This account may not sign in at this time",
        ),
        Reason::PasswordExpired => t("密码已过期", "The password has expired"),
        Reason::PasswordMustChange => t("需要先修改密码", "The password must be changed first"),
        Reason::NoLogonServers => {
            t("找不到可用的登录服务器", "No logon server could be reached")
        }
        Reason::Timeout => t("连接超时", "Connection timed out"),
        Reason::Refused => t("目标主机拒绝了连接", "The host refused the connection"),
        Reason::Unreachable => t("无法访问目标主机", "The host is unreachable"),
        Reason::Reset => t("连接被重置", "The connection was reset"),
        Reason::Dns => t("无法解析主机名", "Could not resolve the host name"),
        Reason::SecurityNegotiation => t(
            "与服务器的安全协商失败",
            "Security negotiation with the server failed",
        ),
        Reason::Certificate => t("服务器证书未被接受", "The server certificate was not accepted"),
        Reason::AuthMethodUnsupported => t(
            "服务器要求的认证方式不受支持（需要 VNC 密码认证或无认证）",
            "The server's authentication type is unsupported (VNC password auth or none required)",
        ),
        Reason::PasswordRequired => t(
            "该服务器要求认证，但会话未配置密码",
            "The server demands authentication, but no password is configured",
        ),
    }
}

fn contains(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

/// Whether a character is CJK (ideographs, their punctuation, full-width
/// forms). A failure text containing one is a line written for a reader — the
/// drivers localise their own refusals with `t` — rather than a protocol dump.
fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x303F | 0x4E00..=0x9FFF | 0xFF00..=0xFFEF)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim from a real server: `probe2 nla 192.168.0.69 Administrator
    /// <wrong password>` against a host whose account has a password.
    const REJECTED_LOGON: &str = "CredSSP: InvalidToken: CredSSP server returned an error status; \
                                  status is STATUS_LOGON_FAILURE [0xc000006d]";

    #[test]
    fn a_rejected_logon_reads_as_credentials_and_not_as_the_protocol() {
        // The whole point of the ordering: this text mentions CredSSP, and a
        // negotiation-first rule would report the protocol instead of the
        // password the user actually has to fix.
        assert_eq!(classify(REJECTED_LOGON), Some(Reason::Credentials));
    }

    #[test]
    fn the_hex_code_alone_is_enough() {
        // A future sspi may reword the sentence; the NTSTATUS value survives.
        assert_eq!(classify("[0xc000006d]"), Some(Reason::Credentials));
        assert_eq!(classify("status is [0xC000006D]"), Some(Reason::Credentials));
    }

    #[test]
    fn account_policy_codes_keep_their_own_wording() {
        for (code, reason) in [
            ("0xc0000234", Reason::AccountLocked),
            ("0xc0000072", Reason::AccountDisabled),
            ("0xc000006e", Reason::AccountRestricted),
            ("0xc0000070", Reason::WorkstationRestricted),
            ("0xc000006f", Reason::LogonHours),
            ("0xc0000071", Reason::PasswordExpired),
            ("0xc0000224", Reason::PasswordMustChange),
            ("0xc000005e", Reason::NoLogonServers),
        ] {
            assert_eq!(classify(&format!("logon failed [{code}]")), Some(reason), "{code}");
        }
    }

    #[test]
    fn a_localised_socket_error_is_classified_by_its_number() {
        // Windows localises the socket message, so the text is a hybrid; the
        // code is the part that survives translation.
        let refused = "VNC TCP connect failed: 由于目标计算机积极拒绝，无法连接。 (os error 10061)";
        assert_eq!(classify(refused), Some(Reason::Refused));
    }

    #[test]
    fn transport_failures_are_told_apart() {
        assert_eq!(classify("VNC connect timed out"), Some(Reason::Timeout));
        assert_eq!(classify("VNC handshake timed out"), Some(Reason::Timeout));
        assert_eq!(classify("connection reset by peer"), Some(Reason::Reset));
        assert_eq!(classify("No route to host (os error 10065)"), Some(Reason::Unreachable));
        assert_eq!(
            classify("failed to lookup address information: Name or service not known"),
            Some(Reason::Dns)
        );
    }

    #[test]
    fn a_certificate_refusal_is_not_mistaken_for_a_transport_error() {
        // rustls names both the certificate and the handshake in one line.
        let text = "invalid peer certificate: UnknownIssuer";
        assert_eq!(classify(text), Some(Reason::Certificate));
    }

    #[test]
    fn credssp_alone_still_classifies_as_a_negotiation_problem() {
        assert_eq!(
            classify("CredSSP: attempted to feed server request in an unexpected state"),
            Some(Reason::SecurityNegotiation)
        );
    }

    #[test]
    fn vnc_auth_verdicts_classify_by_the_librarys_own_words() {
        // (#vnc-auth-reasons 2026-10-27) vnc-rs's three credential-shaped
        // failures, verbatim (wrapped by its `VNC Error with message:` prefix in
        // one case) — each must reach the user as something they can act on.
        assert_eq!(
            classify("VNC Error with message: Security type apart from Vnc Auth has not been implemented"),
            Some(Reason::AuthMethodUnsupported)
        );
        assert_eq!(classify("Wrong password"), Some(Reason::Credentials));
        assert_eq!(
            classify("Auth is required but no password provided"),
            Some(Reason::PasswordRequired)
        );
    }

    #[test]
    fn an_unrecognised_failure_gets_the_generic_line_and_never_the_raw_text() {
        let raw = "something nobody has seen before";
        let line = explain(raw);
        assert!(!line.is_empty());
        assert!(!line.contains("nobody"), "the raw text must not be echoed: {line}");
        // Either variant of the generic line: the assertion must not depend on
        // which language another test happens to have selected.
        assert!(line == "连接失败" || line == "Connection failed", "{line}");
    }

    #[test]
    fn a_line_this_crate_wrote_is_passed_through_untouched() {
        // The RDP driver's own refusals (see `authorize_certificate`) are
        // already sentences; nothing here should reword them.
        let ours = "服务器证书与已信任的指纹不符，已拒绝连接";
        assert_eq!(explain(ours), ours);
        assert_eq!(explain("已拒绝该服务器的证书"), "已拒绝该服务器的证书");
    }

    #[test]
    fn a_reason_is_offered_only_when_there_is_one() {
        // `explain_optional` is what a *finished* session asks for, and its
        // answer is appended to a state ("已断线 — …"). The generic headline
        // would make that read "已断线 — 连接失败", which says less than the bare
        // state did — so nothing unclassifiable comes back at all.
        assert_eq!(explain_optional("something nobody has seen before"), None);
        // A text that does name a reason comes back, classified.
        assert_eq!(
            explain_optional("connection reset by peer"),
            Some(t("连接被重置", "The connection was reset").to_owned())
        );
        // A line this crate wrote is already a sentence and stays one.
        assert_eq!(explain_optional("连接超时"), Some("连接超时".to_owned()));
    }
}
