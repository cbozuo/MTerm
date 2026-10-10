/// The port RDP is assigned by IANA/MS-RDPBCGR.
pub(crate) const RDP_DEFAULT_PORT: u16 = 3389;

/// Smallest desktop side written into a connection file. Servers reject (or
/// clamp in surprising ways) tiny desktops, and the dialog can hand us anything.
pub(crate) const MIN_DESKTOP_SIDE: u16 = 200;

/// The account details passed to the system client.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Account<'a> {
    pub(crate) host: &'a str,
    pub(crate) port: u16,
    /// User name without the domain part.
    pub(crate) user: &'a str,
    /// Windows logon domain, or "" when there is none.
    pub(crate) domain: &'a str,
    pub(crate) password: &'a str,
    /// Start full screen (at the local monitor resolution).
    pub(crate) fullscreen: bool,
    /// Desktop size for a windowed session, in pixels.
    pub(crate) width: u16,
    pub(crate) height: u16,
}
