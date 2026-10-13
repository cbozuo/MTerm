/// The account details passed to the system client.
///
/// Built by [`crate::rdp::launch`] from a [`crate::config::Session`] and
/// consumed by the `mstsc` / FreeRDP launchers in this package. The embedded
/// client shares the *fields* verbatim (see `EmbeddedRdpConfig`) but not this
/// struct — it has its own connect parameters.
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
