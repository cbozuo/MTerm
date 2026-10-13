//! Input events the UI sends towards a remote-desktop session, and the
//! keyboard translation from Slint's `KeyEvent` to the protocol's own codes.
//!
//! Slint reports a key as a Unicode string plus modifiers; RDP wants a
//! set-1 scancode and VNC wants an X11 keysym. Neither is derivable from the
//! other, so the translation tables live here — one per protocol — and the UI
//! only has to say "this key went down/up with these modifiers".

/// A decoded remote-desktop input event, protocol independent.
///
/// `code` means whatever the destination protocol calls it: an RDP set-1
/// scancode, or an X11 keysym. Each protocol's driver knows which.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RemoteInput {
    /// Physical key press/release. `code` = scancode (RDP) or keysym (VNC).
    Key { code: u32, down: bool },
    /// Character that has no useful physical key (IME commit, dead-key result,
    /// paste into a text field). RDP sends it as a Unicode key event pair;
    /// VNC maps it back to a keysym where one exists.
    Text { ch: char },
    /// Pointer at remote-desktop coordinates, with the button bitmask in the
    /// RDP sense (bit 0 left, 1 middle, 2 right, 3/4 wheel).
    Pointer { x: u16, y: u16, mask: u8 },
    /// Wheel / trackpad scroll at a position, in notches (positive = up/right).
    Wheel { x: u16, y: u16, dx: f64, dy: f64 },
    /// Drop every key the client currently holds down. Sent when the tab loses
    /// focus, so a key held while alt-tabbing away cannot stay stuck down on
    /// the remote desktop.
    ReleaseAll,
    /// Ask the server to resize the desktop (P0-9). Only produced when the
    /// session is set to follow the tab size; the I/O side clamps the value,
    /// since a degenerate size can also arrive from a window minimise.
    Resize { width: u16, height: u16 },
}

/// Where a Slint key event came from, in the form both mappings need.
///
/// Modifiers are passed through separately rather than folded into the key
/// because each protocol needs them differently: RDP resolves Ctrl/Alt/Shift
/// into scancodes that travel with the key, while VNC wants a keysym that
/// already encodes the modifier.
#[derive(Debug, Clone)]
pub struct SlintKey {
    /// `KeyEvent.text` verbatim.
    ///
    /// Both the string and its first character are kept because Slint reports a
    /// key either way depending on the key and the backend: a printable key
    /// arrives as one character (`"a"`), while a named key arrives as its name
    /// (`"F1"`, `"PageUp"`) or as an escape sequence (`"\u{1b}[A"`). A protocol
    /// mapping needs the string to recognise the named forms and the character
    /// to look up the printable ones.
    pub name: String,
    /// First character of `name`, or `\0` when it is not a single character.
    pub text: char,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Physical Ctrl on macOS, where Slint swaps the two names.
    pub meta: bool,
    /// True for `key-pressed`, false for `key-released`.
    pub down: bool,
}

impl RemoteInput {
    /// Convenience constructor for a key press/release.
    pub fn key(code: u32, down: bool) -> Self {
        RemoteInput::Key { code, down }
    }
}

/// X11 keysym for a printable Latin-1 character.
///
/// VNC's key protocol is keysym-based, and for plain Latin-1 the keysym is just
/// the character code. Shared by the VNC driver.
///
/// The range stops at 0x7E and resumes at 0xA0 on purpose: 0x7F (DEL) and the C1
/// block 0x80–0x9F are control characters, not text, and X11 has no keysym that
/// means "that character". Leaving 0x7F in made the Delete *key* — which Slint
/// reports as `\u{7f}` — resolve to keysym 0x7F instead of falling through to
/// the real `XK_Delete` (0xFFFF).
pub fn latin1_keysym(ch: char) -> Option<u32> {
    let code = ch as u32;
    let printable = (0x20..=0x7e).contains(&code) || (0xa0..=0xff).contains(&code);
    printable.then_some(code)
}

/// Special keysyms the UI needs beyond ASCII, in the private-use range the
/// X11/VNC convention reserves for them.
pub mod keysym {
    pub const BACKSPACE: u32 = 0xff08;
    pub const TAB: u32 = 0xff09;
    pub const RETURN: u32 = 0xff0d;
    /// Shift+Tab, which X11 gives its own keysym instead of a modifier.
    pub const ISO_LEFT_TAB: u32 = 0xfe20;
    pub const SCROLL_LOCK: u32 = 0xff14;
    pub const ESCAPE: u32 = 0xff1b;
    pub const HOME: u32 = 0xff50;
    pub const LEFT: u32 = 0xff51;
    pub const UP: u32 = 0xff52;
    pub const RIGHT: u32 = 0xff53;
    pub const DOWN: u32 = 0xff54;
    pub const PAGE_UP: u32 = 0xff55;
    pub const PAGE_DOWN: u32 = 0xff56;
    pub const END: u32 = 0xff57;
    pub const INSERT: u32 = 0xff63;
    pub const MENU: u32 = 0xff67;
    pub const DELETE: u32 = 0xffff;
    pub const F1: u32 = 0xffbe;
    pub const SHIFT_L: u32 = 0xffe1;
    pub const SHIFT_R: u32 = 0xffe2;
    pub const CONTROL_L: u32 = 0xffe3;
    pub const CONTROL_R: u32 = 0xffe4;
    pub const CAPS_LOCK: u32 = 0xffe5;
    pub const ALT_L: u32 = 0xffe9;
    pub const ALT_R: u32 = 0xffea;
    pub const SUPER_L: u32 = 0xffeb;
    pub const SUPER_R: u32 = 0xffec;
}

/// Pointer button bitmask bits, in the RDP convention both drivers use.
pub mod button {
    pub const LEFT: u8 = 0x01;
    pub const MIDDLE: u8 = 0x02;
    pub const RIGHT: u8 = 0x04;
    pub const BACK: u8 = 0x08;
    pub const FORWARD: u8 = 0x10;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin1_covers_printable_and_excludes_control_and_unicode() {
        assert_eq!(latin1_keysym('a'), Some(0x61));
        assert_eq!(latin1_keysym(' '), Some(0x20));
        assert_eq!(latin1_keysym('~'), Some(0x7e));
        // Printable Latin-1 supplement is still covered…
        assert_eq!(latin1_keysym('é'), Some(0xe9));
        // …but DEL and the C1 controls are not text and have no keysym.
        assert_eq!(latin1_keysym('\u{7f}'), None);
        assert_eq!(latin1_keysym('\u{85}'), None);
        assert_eq!(latin1_keysym('\n'), None);
        assert_eq!(latin1_keysym('\u{4e2d}'), None);
    }

    #[test]
    fn button_bits_do_not_overlap() {
        let all = button::LEFT | button::MIDDLE | button::RIGHT | button::BACK | button::FORWARD;
        assert_eq!(all.count_ones(), 5);
        assert_eq!(button::LEFT & button::RIGHT, 0);
    }
}
