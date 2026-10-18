//! The connection state machine shared by the built-in RDP and VNC clients
//! (P0-10), and the events the UI renders from it.

use std::sync::atomic::{AtomicU8, Ordering};

/// One of the five states a remote-desktop tab can be in.
///
/// The numbering is what the Slint side stores in `RemoteState.state`, so the
/// two must stay in step; `#[repr(u8)]` plus the `as_u8` mapping below keep the
/// translation in one place instead of at every `set_state` call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RemoteState {
    /// Dialing / handshaking. First connection or a reconnect attempt.
    Connecting = 0,
    /// Handshake done, credentials being exchanged.
    Authenticating = 1,
    /// Frames are arriving.
    Connected = 2,
    /// Waiting out a backoff before dialing again.
    Reconnecting = 3,
    /// Gone, and not retrying (either the user closed it or it failed).
    Disconnected = 4,
    /// Gave up. `RemoteState::message` carries the human-readable reason.
    Failed = 5,
}

impl RemoteState {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Slint renders the dot colour from this, so the UI never has to know the
    /// numbering: connecting/reconnecting are amber, connected green,
    /// disconnected/failed red.
    pub fn as_str(self) -> &'static str {
        match self {
            RemoteState::Connecting => "connecting",
            RemoteState::Authenticating => "authenticating",
            RemoteState::Connected => "connected",
            RemoteState::Reconnecting => "reconnecting",
            RemoteState::Disconnected => "disconnected",
            RemoteState::Failed => "failed",
        }
    }

    /// True while the tab should still be considered "live" — i.e. it may yet
    /// produce frames. Drives the sidebar's online dot.
    pub fn is_live(self) -> bool {
        matches!(
            self,
            RemoteState::Connecting | RemoteState::Authenticating | RemoteState::Connected
                | RemoteState::Reconnecting
        )
    }
}

/// The state a driver publishes when its attempt loop ends, if any.
///
/// `already_failed` is `true` when the loop published [`RemoteState::Failed`]
/// with its reason on the way out. The driver must then publish **nothing**:
/// the tail's bare `Disconnected` would replace that reason, and the view clears
/// the error text for every state but `Failed` — which is what turned every
/// refused connection (a rejected certificate, a server that hangs up during the
/// handshake) into a silent "disconnected" with no explanation.
///
/// Shared by both drivers rather than written twice: the rule is about the state
/// machine, and two copies would eventually disagree about it.
pub fn closing_state(already_failed: bool) -> Option<RemoteState> {
    if already_failed {
        None
    } else {
        Some(RemoteState::Disconnected)
    }
}

/// State plus an optional human-readable note, published atomically so the I/O
/// task can hand it to the UI thread without a lock.
#[derive(Debug)]
pub struct RemoteStatus {
    state: AtomicU8,
    message: std::sync::Mutex<Option<String>>,
}

impl Default for RemoteStatus {
    fn default() -> Self {
        Self::new(RemoteState::Connecting)
    }
}

impl RemoteStatus {
    pub fn new(initial: RemoteState) -> Self {
        Self {
            state: AtomicU8::new(initial.as_u8()),
            message: std::sync::Mutex::new(None),
        }
    }

    pub fn set(&self, state: RemoteState, message: Option<String>) {
        if let Ok(mut slot) = self.message.lock() {
            *slot = message;
        }
        self.state.store(state.as_u8(), Ordering::Release);
    }

    pub fn state(&self) -> RemoteState {
        match self.state.load(Ordering::Acquire) {
            0 => RemoteState::Connecting,
            1 => RemoteState::Authenticating,
            2 => RemoteState::Connected,
            3 => RemoteState::Reconnecting,
            4 => RemoteState::Disconnected,
            _ => RemoteState::Failed,
        }
    }

    pub fn message(&self) -> Option<String> {
        self.message.lock().ok().and_then(|slot| slot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trips_through_its_numeric_form() {
        for state in [
            RemoteState::Connecting,
            RemoteState::Authenticating,
            RemoteState::Connected,
            RemoteState::Reconnecting,
            RemoteState::Disconnected,
            RemoteState::Failed,
        ] {
            let back = match state.as_u8() {
                0 => RemoteState::Connecting,
                1 => RemoteState::Authenticating,
                2 => RemoteState::Connected,
                3 => RemoteState::Reconnecting,
                4 => RemoteState::Disconnected,
                _ => RemoteState::Failed,
            };
            assert_eq!(back, state);
        }
    }

    #[test]
    fn a_failed_tab_is_not_live_so_the_right_sidebar_dot_goes_out() {
        assert!(RemoteState::Connected.is_live());
        assert!(RemoteState::Reconnecting.is_live());
        assert!(!RemoteState::Failed.is_live());
        assert!(!RemoteState::Disconnected.is_live());
    }

    #[test]
    fn set_carries_the_failure_reason_to_the_reader() {
        let status = RemoteStatus::new(RemoteState::Connecting);
        status.set(RemoteState::Failed, Some("boom".into()));
        assert_eq!(status.state(), RemoteState::Failed);
        assert_eq!(status.message().as_deref(), Some("boom"));
    }

    #[test]
    fn a_published_failure_is_not_replaced_by_a_bare_disconnect() {
        assert_eq!(
            closing_state(true),
            None,
            "the reason has to survive the end of the loop"
        );
        assert_eq!(closing_state(false), Some(RemoteState::Disconnected));
    }
}
