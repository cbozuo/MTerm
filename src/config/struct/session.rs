use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Secret;

/// Which transport a session uses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    /// SSH shell + SFTP (the original and default behaviour).
    #[default]
    Ssh,
    /// Local serial port (COM3 / /dev/ttyUSB0) for switches, routers, MCUs (#14).
    Serial,
    /// Plain Telnet over TCP, for legacy network gear (#17).
    Telnet,
    /// Local shell process on this machine (PowerShell/CMD/WSL/$SHELL).
    Local,
    /// Remote Desktop session. Two ways to open it (see `RdpClientMode`): the
    /// system client (`mstsc` / `xfreerdp`) opens the desktop in its own native
    /// window, or MTerm's built-in client speaks RDP itself inside a tab.
    Rdp,
    /// VNC/RFB remote desktop, always handled by MTerm's built-in client
    /// inside a tab. A separate variant rather than an RDP mode because the
    /// protocol differs outright: there is no Windows logon domain, and the
    /// challenge/response authentication and account fields are their own.
    Vnc,
}

impl SessionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionKind::Ssh => "ssh",
            SessionKind::Serial => "serial",
            SessionKind::Telnet => "telnet",
            SessionKind::Local => "local",
            SessionKind::Rdp => "rdp",
            SessionKind::Vnc => "vnc",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "serial" => SessionKind::Serial,
            "telnet" => SessionKind::Telnet,
            "local" => SessionKind::Local,
            "rdp" => SessionKind::Rdp,
            "vnc" => SessionKind::Vnc,
            _ => SessionKind::Ssh,
        }
    }
}

/// Which client opens an RDP session.
///
/// Both variants describe the *same* connection (one host, one account, one
/// desktop) and differ only in who speaks the protocol, so this is a field on
/// `Session` instead of a second `SessionKind`: switching it must not split one
/// connection into two entries, and every existing `match session.kind` stays
/// untouched.
///
/// `#[serde(default)]` on the field makes every session file written before
/// this existed deserialize to `SystemClient` — today's mstsc behaviour —
/// with no migration step.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RdpClientMode {
    /// Hand the account to the system client (`mstsc` / `xfreerdp`), which
    /// owns its own window. The behaviour MTerm shipped before the built-in
    /// client, and still the default.
    #[default]
    SystemClient,
    /// MTerm's own RDP client: a tab holds the desktop, frames are rendered by
    /// Slint and input is forwarded to the session.
    Embedded,
}

impl RdpClientMode {
    /// Stable identifier used by the session dialog's segmented control.
    pub fn as_str(&self) -> &'static str {
        match self {
            RdpClientMode::SystemClient => "system",
            RdpClientMode::Embedded => "embedded",
        }
    }

    /// Anything unrecognised (including an empty string) means the system
    /// client, so a hand-edited config can never silently switch a session
    /// into the embedded client.
    pub fn from_str(s: &str) -> Self {
        match s {
            "embedded" => RdpClientMode::Embedded,
            _ => RdpClientMode::SystemClient,
        }
    }
}

impl SessionKind {
    /// True for the transports that are *always* rendered inside a tab by MTerm
    /// itself, whatever `rdp_mode` says.
    ///
    /// This deliberately answers only half the question. An RDP session in
    /// `RdpClientMode::Embedded` is also a tab, and that cannot be decided from
    /// `SessionKind` alone. Callers that need the real answer — anything that
    /// creates a tab, or picks a renderer — must use `Session::uses_tab`, which
    /// can see both fields. Use this one only for genuinely kind-level
    /// questions (e.g. "does this protocol have a tab-hosted flavour at all?").
    pub fn is_tab_transport(&self) -> bool {
        matches!(self, SessionKind::Vnc)
    }
}

fn default_baud() -> u32 {
    115_200
}
fn default_data_bits() -> u8 {
    8
}
fn default_stop_bits() -> u8 {
    1
}
fn default_parity() -> String {
    "none".to_string()
}

fn default_flow() -> String {
    "none".to_string()
}

fn default_encoding() -> String {
    "UTF-8".to_string()
}

fn default_vt100_drawing() -> bool {
    false
}

fn default_rdp_width() -> u16 {
    1280
}

fn default_rdp_height() -> u16 {
    720
}

/// How a session authenticates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    Password,
    #[serde(rename = "keyboard-interactive")]
    KeyboardInteractive,
    Key,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthMethod::Password => "password",
            AuthMethod::KeyboardInteractive => "keyboard-interactive",
            AuthMethod::Key => "key",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "keyboard-interactive" | "keyboard" | "interactive" => AuthMethod::KeyboardInteractive,
            "key" => AuthMethod::Key,
            _ => AuthMethod::Password,
        }
    }
}

/// A single saved SSH target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthMethod,
    #[serde(default)]
    pub password: Secret,
    #[serde(default)]
    pub private_key_path: String,
    #[serde(default)]
    pub private_key_inline: Secret,
    /// Optional outbound proxy, e.g. "socks5://127.0.0.1:1080" or
    /// "http://user:pass@host:8080". Empty = use $ALL_PROXY, else direct.
    #[serde(default)]
    pub proxy: String,
    /// Optional SSH jump host (bastion): the id of another saved SSH session to
    /// tunnel this connection through, like OpenSSH's ProxyJump. Empty = direct.
    /// Single hop only; the jump session supplies its own host/user/auth (#211).
    #[serde(default)]
    pub jump_session_id: String,
    /// (#conn-path 2026-10-04) 多级跳板链(有序会话 id,依次经过)。单跳时代只
    /// 写 jump_session_id;加载时若本字段为空而 jump_session_id 非空则迁移为
    /// 链首一项;保存时 jump_session_id 同步为链首(旧版本读兼容)。
    #[serde(default)]
    pub jump_chain: Vec<String>,
    #[serde(default)]
    pub last_used: Option<String>,
    /// Optional folder/group name to organize sessions in the list (#41).
    /// Empty = ungrouped. Sessions are grouped by this in Quick Connect.
    #[serde(default)]
    pub group: String,

    // --- Transport ----------------------------------------------------------
    /// SSH (default), Serial, or Telnet. Absent in old config files → Ssh.
    #[serde(default)]
    pub kind: SessionKind,

    /// WSL distribution and startup directory for generated local sessions.
    /// The directory defaults to the selected distribution user's home (`~`).
    #[serde(default)]
    pub local_distribution: String,
    #[serde(default)]
    pub local_working_dir: String,

    // --- Serial-only fields (ignored unless kind == Serial) -----------------
    /// Serial device path, e.g. "COM3" (Windows) or "/dev/ttyUSB0" (Linux).
    #[serde(default)]
    pub serial_port: String,
    #[serde(default = "default_baud")]
    pub baud_rate: u32,
    #[serde(default = "default_data_bits")]
    pub data_bits: u8,
    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,
    /// "none" | "odd" | "even".
    #[serde(default = "default_parity")]
    pub parity: String,
    /// "none" | "hardware" | "software".
    #[serde(default = "default_flow")]
    pub flow_control: String,

    // --- RDP-only fields (ignored unless kind == Rdp) -----------------------
    /// Windows logon domain (or "." for the local machine); empty = none. A
    /// "DOMAIN\user" style username is split into this when the connection is
    /// handed to the system RDP client.
    #[serde(default)]
    pub rdp_domain: String,
    /// Ask the system client to start the desktop full screen. It then uses the
    /// local monitor resolution, which keeps the image pixel-exact; the size
    /// below is only used when this is false.
    #[serde(default)]
    pub rdp_fullscreen: bool,
    /// Remote desktop size in pixels for a windowed session.
    #[serde(default = "default_rdp_width")]
    pub rdp_width: u16,
    #[serde(default = "default_rdp_height")]
    pub rdp_height: u16,
    /// Which client opens an RDP session. Absent in old config files, where it
    /// means `SystemClient` — the mstsc behaviour that shipped first.
    #[serde(default)]
    pub rdp_mode: RdpClientMode,
    /// Built-in client only: resize the remote desktop to follow the tab
    /// instead of using the fixed `rdp_width` x `rdp_height` above. Servers that
    /// refuse a mid-session resize fall back to scaling the fixed size to fit.
    #[serde(default)]
    pub rdp_follow_tab_size: bool,
    /// Hide MTerm's own chrome and give the tab the whole monitor. Only
    /// meaningful for the built-in client; the system client has its own
    /// full-screen handling driven by `rdp_fullscreen`.
    #[serde(default)]
    pub rdp_embedded_fullscreen: bool,

    // --- VNC-only fields (ignored unless kind == Vnc) -----------------------
    /// Reject mouse and keyboard input at the server (view-only session).
    #[serde(default)]
    pub vnc_view_only: bool,
    /// Preferred VNC security type; empty = let the client pick. VNC Auth
    /// (DES challenge/response) is the common one; VeNCrypt/TLS need the
    /// session to offer it.
    #[serde(default)]
    pub vnc_security: String,
    /// Mirror the local clipboard into the remote session and back.
    #[serde(default = "default_true")]
    pub vnc_clipboard: bool,

    /// Character encoding used by the interactive terminal stream (#338).
    /// UTF-8 remains the default for existing and newly created sessions.
    #[serde(default = "default_encoding")]
    pub encoding: String,

    /// Honor VT100 line drawing (DEC Special Graphics via `ESC ( 0` and SO/SI)
    /// even when the encoding is UTF-8 (#376). PuTTY's "Enable VT100 line
    /// drawing even in UTF-8 mode"; opt-in, off for new/existing sessions.
    #[serde(default = "default_vt100_drawing")]
    pub vt100_drawing: bool,

    // --- SSH port forwarding / tunnels (#56) --------------------------------
    /// Tunnels established automatically when this SSH session connects.
    #[serde(default)]
    pub forwards: Vec<PortForward>,

    /// Expect/send rules evaluated against interactive terminal output (#212).
    #[serde(default)]
    pub triggers: Vec<SessionTrigger>,

    /// Skip the shell-integration setup (the cwd-follow PROMPT_COMMAND hook + the
    /// remote resource monitor). Those assume a POSIX shell; on a Windows server
    /// whose shell is pwsh/cmd the injected hook breaks the shell. Turn this on
    /// for such servers (#140).
    #[serde(default)]
    pub disable_shell_integration: bool,
    /// Free-form note for this session — somewhere to stash extra info (jump-host
    /// details, credentials hints, owner, etc.). Shown only in the edit dialog.
    /// (B站 suggestion)
    #[serde(default)]
    pub note: String,
}

/// One SSH tunnel (#56). `kind` is "local" (-L), "remote" (-R) or
/// "dynamic" (-D / SOCKS5). For local/remote, `host:host_port` is the target;
/// for dynamic it is ignored (the SOCKS client picks the destination).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortForward {
    pub kind: String,
    /// Optional label to tell rules apart (#100). Empty = unnamed.
    #[serde(default)]
    pub name: String,
    /// Listener bind address (local side for L/D, remote side for R).
    /// Empty → 127.0.0.1.
    #[serde(default)]
    pub bind_addr: String,
    pub bind_port: u16,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub host_port: u16,
}

/// Automatically send a response when literal terminal output is observed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionTrigger {
    pub expect: String,
    #[serde(default)]
    pub response: Secret,
    #[serde(default = "default_true")]
    pub append_enter: bool,
    /// False means the rule is consumed after its first match.
    #[serde(default)]
    pub repeat: bool,
}

fn default_true() -> bool {
    true
}

impl Session {
    /// True when this session's desktop is drawn by MTerm inside one of its
    /// own tabs, i.e. when connecting it must create a tab rather than hand the
    /// account to an external client.
    ///
    /// This is the single predicate every caller should use — it is the only
    /// place that can read both the transport and `rdp_mode`, so no call site
    /// can silently forget the mode and mis-classify `Rdp + Embedded` as an
    /// external-client session (which would connect fine and show no tab).
    ///
    /// Both `RdpClientMode` arms are spelled out rather than using
    /// `!= SystemClient`, so a future third mode has to be classified here
    /// instead of defaulting into the tab path.
    pub fn uses_tab(&self) -> bool {
        match self.kind {
            SessionKind::Vnc => true,
            SessionKind::Rdp => match self.rdp_mode {
                RdpClientMode::Embedded => true,
                RdpClientMode::SystemClient => false,
            },
            _ => false,
        }
    }

    pub fn new_empty() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            host: String::new(),
            port: 22,
            user: "root".into(),
            auth: AuthMethod::Password,
            password: Secret::default(),
            private_key_path: String::new(),
            private_key_inline: Secret::default(),
            proxy: String::new(),
            jump_session_id: String::new(),
            jump_chain: Vec::new(),
            last_used: None,
            group: String::new(),
            kind: SessionKind::Ssh,
            local_distribution: String::new(),
            local_working_dir: String::new(),
            serial_port: String::new(),
            baud_rate: default_baud(),
            data_bits: default_data_bits(),
            stop_bits: default_stop_bits(),
            parity: default_parity(),
            flow_control: default_flow(),
            rdp_domain: String::new(),
            rdp_fullscreen: false,
            rdp_width: default_rdp_width(),
            rdp_height: default_rdp_height(),
            rdp_mode: RdpClientMode::default(),
            rdp_follow_tab_size: false,
            rdp_embedded_fullscreen: false,
            vnc_view_only: false,
            vnc_security: String::new(),
            vnc_clipboard: true,
            encoding: default_encoding(),
            vt100_drawing: default_vt100_drawing(),
            forwards: Vec::new(),
            triggers: Vec::new(),
            disable_shell_integration: false,
            note: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The zero-regression proof for the built-in RDP client (P0-4): a session
    /// file written before the feature existed has none of the new fields, and
    /// must load as a system-client session — the `mstsc` behaviour that shipped
    /// first — with no migration step.
    ///
    /// Going through `Session::new_empty` would not show this: that is the
    /// constructor, not serde. The new keys are stripped from a serialized
    /// session instead, so what is parsed is exactly the JSON an older MTerm
    /// wrote.
    #[test]
    fn a_session_file_from_before_the_built_in_client_loads_as_a_system_client() {
        let mut value = serde_json::to_value(Session::new_empty()).expect("serializable");
        let object = value.as_object_mut().expect("a session is a JSON object");
        for field in [
            "rdp_mode",
            "rdp_follow_tab_size",
            "rdp_embedded_fullscreen",
            "vnc_view_only",
            "vnc_security",
            "vnc_clipboard",
        ] {
            assert!(
                object.remove(field).is_some(),
                "{field} should be present in a freshly serialized session"
            );
        }

        let session: Session = serde_json::from_value(value).expect("an old entry must still load");

        assert_eq!(session.rdp_mode, RdpClientMode::SystemClient);
        assert!(!session.rdp_follow_tab_size);
        assert!(!session.rdp_embedded_fullscreen);
        assert!(!session.uses_tab(), "an old entry must not be given a tab");
        assert!(session.vnc_clipboard, "the documented default is on");
    }

    #[test]
    fn only_vnc_and_embedded_rdp_are_tab_hosted() {
        // The rest of the app asks this one predicate, so it is tested directly
        // rather than incidentally through a caller.
        let mut session = Session::new_empty();
        assert!(!session.uses_tab(), "SSH has no tab-hosted desktop");

        session.kind = SessionKind::Rdp;
        assert!(!session.uses_tab(), "the system client owns its own window");
        session.rdp_mode = RdpClientMode::Embedded;
        assert!(session.uses_tab());

        session.kind = SessionKind::Vnc;
        session.rdp_mode = RdpClientMode::SystemClient;
        assert!(session.uses_tab(), "VNC has no system client to fall back to");
    }

    /// A hand-edited config must never switch a session into the built-in client
    /// by accident, so anything unrecognised means the system client.
    #[test]
    fn an_unrecognised_mode_falls_back_to_the_system_client() {
        assert_eq!(RdpClientMode::from_str("embedded"), RdpClientMode::Embedded);
        assert_eq!(RdpClientMode::from_str("system"), RdpClientMode::SystemClient);
        assert_eq!(RdpClientMode::from_str(""), RdpClientMode::SystemClient);
        assert_eq!(RdpClientMode::from_str("nonsense"), RdpClientMode::SystemClient);
    }
}
