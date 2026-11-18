//! MTerm's built-in VNC (RFB) client.
//!
//! VNC has no "system client" path — there is no `vncviewer` on a Windows box to
//! hand an account to — so this module is the only way MTerm speaks it, and it
//! always hosts the desktop in a tab. Kept as its own top-level module rather
//! than folded into `crate::rdp` because it is a different protocol with
//! different account fields, matching `SessionKind::Vnc` being its own variant.
//!
//! Module layout follows the project's `struct/` + `impls/` convention:
//!   struct/config.rs     — connect parameters
//!   struct/framebuffer.rs— decoded pixels + the two RFB blit operations
//!   impls/client.rs      — the vnc-rs session driver

// `struct` is a Rust keyword, so these are re-exported under their plain names
// below rather than addressed as `crate::vnc::struct::x`.
#[path = "struct/config.rs"]
pub(crate) mod config;
#[path = "struct/framebuffer.rs"]
pub(crate) mod framebuffer;

#[path = "impls/client.rs"]
pub(crate) mod client;
#[path = "impls/keysym.rs"]
mod keysym;

pub(crate) use keysym::keysym_for;

// (#vnc-connect-probe 2026-11-17) 进程内最小 RFB 服务器:握手/认证/首帧的端到端探查。
#[cfg(test)]
mod rfb_mock_tests;
