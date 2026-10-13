//! Turning a Slint key event into a protocol input event.
//!
//! Lives in its own module because it is the one place where "what the UI said
//! happened" becomes "what the protocol needs". Both drivers depend on it, and
//! the decisions it encodes are protocol-independent: which events are
//! modifiers, that a bare modifier press is forwarded as itself, and that text
//! with no physical key must go out as a Unicode/keysym event rather than being
//! dropped.

use crate::rdp::embedded::input::SlintKey;
use crate::rdp::embedded::keys::{translate as translate_rdp, ScanCode};

/// What the UI reported, before any protocol-specific interpretation.
pub struct UiKey {
    /// `KeyEvent.text` verbatim: one character for a printable key, a name or an
    /// escape sequence for a named one.
    pub text: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Physical Ctrl on macOS, where Slint swaps the two modifier names.
    pub meta: bool,
    pub down: bool,
}

impl UiKey {
    /// First character of `text`, or `'\0'` when it is not a single character.
    ///
    /// Both views of the string are needed: the protocol tables are `char`-keyed
    /// for printable keys and `str`-keyed for named ones.
    fn first_char(&self) -> char {
        let mut chars = self.text.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) => ch,
            _ => '\0',
        }
    }

    /// True when the key is one of the modifiers themselves.
    ///
    /// A modifier is forwarded as itself rather than as a modifier *state*, so
    /// the remote desktop tracks its own left/right modifier state — which is
    /// also what makes a bare "press and release shift" meaningful there.
    ///
    /// `\u{10}`..`\u{16}` are Slint's own codes for Shift/Control/Alt/AltGr and
    /// their right-hand twins (`i-slint-common/key_codes.rs`); the `\u{ffe1}`
    /// keysym range this once tested is never produced by the backend.
    pub fn is_modifier(&self) -> bool {
        matches!(
            self.text.as_str(),
            "Shift" | "Control" | "Alt" | "AltGr" | "Super" | "Meta"
        ) || matches!(self.first_char(), '\u{10}'..='\u{16}')
    }

    /// True when `ch` is something worth sending as text.
    ///
    /// Slint encodes every named key as a control code (`\u{10}` Shift) or a
    /// private-use code point (`\u{F700}` Up), and it reports IME and pasted
    /// text as ordinary characters. If a code was not recognised by the protocol
    /// table, sending it as a Unicode key event would type a private-use glyph
    /// on the remote desktop — so a control or private-use code that reached
    /// this point is dropped, and only real text is forwarded.
    fn is_sendable_text(ch: char) -> bool {
        let cp = ch as u32;
        let is_control = cp < 0x20 || (0x7f..=0x9f).contains(&cp);
        // The three private-use areas: U+E000..U+F8FF, and planes 15 and 16.
        let is_private_use = (0xe000..=0xf8ff).contains(&cp) || (0x000f_0000..=0x0010_ffff).contains(&cp);
        !is_control && !is_private_use
    }

    fn as_slint_key(&self) -> SlintKey {
        SlintKey {
            name: self.text.clone(),
            text: self.first_char(),
            ctrl: self.ctrl,
            alt: self.alt,
            shift: self.shift,
            meta: self.meta,
            down: self.down,
        }
    }
}

/// The protocol-neutral verdict for one key event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyAction {
    /// A physical key: send `code` with `down`, plus the shift bit the keyboard
    /// event must carry (RDP folds shift into the fast-path flags; VNC ignores
    /// it because the keysym already encodes it).
    Physical { code: u32, down: bool, shift: bool },
    /// Text with no useful physical key (IME commit, dead-key result, pasted
    /// character). The driver sends it as a Unicode event pair, which is the only
    /// way non-ASCII input reaches a remote desktop.
    Text { ch: char },
    /// Nothing to send: the key maps to no code and is not a single character
    /// either, so guessing would send garbage to the server.
    Ignore,
}

/// RDP verdict: a set-1 scancode, or text for a character with no key.
pub fn for_rdp(key: &UiKey) -> KeyAction {
    // The extended (E0) flag is folded into bit 8 of the code. ironrdp's
    // `Scancode::from_u8` takes it as a separate bool, but the event travels
    // through a `u32` shared with the VNC keysym path, so carrying it in the
    // code keeps one event type for both.
    match translate_rdp(&key.as_slint_key()) {
        Some((ScanCode { code, extended }, shift)) => KeyAction::Physical {
            code: u32::from(code) | if extended { 0x100 } else { 0 },
            down: key.down,
            shift,
        },
        None => match key.first_char() {
            ch if UiKey::is_sendable_text(ch) => KeyAction::Text { ch },
            // Not a key we know and not text either: a named key with no
            // scancode in the table. Sending the raw code point would type a
            // private-use glyph on the remote desktop, so nothing is sent.
            _ => KeyAction::Ignore,
        },
    }
}

/// VNC verdict: an X11 keysym, or text for a character that has one.
pub fn for_vnc(key: &UiKey) -> KeyAction {
    if let Some(keysym) = crate::vnc::keysym_for(&key.text) {
        return KeyAction::Physical {
            code: keysym,
            down: key.down,
            // The keysym already encodes shift (`A` is 0x41, `a` is 0x61), so
            // there is no separate shift bit to report.
            shift: false,
        };
    }
    match key.first_char() {
        ch if UiKey::is_sendable_text(ch) => KeyAction::Text { ch },
        _ => KeyAction::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui(text: &str) -> UiKey {
        UiKey {
            text: text.to_string(),
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
            down: true,
        }
    }

    #[test]
    fn a_printable_key_becomes_its_scancode() {
        assert_eq!(
            for_rdp(&ui("a")),
            KeyAction::Physical {
                code: 0x1e,
                down: true,
                shift: false
            }
        );
    }

    #[test]
    fn an_uppercase_letter_is_the_same_key_with_shift() {
        let lower = for_rdp(&ui("a"));
        let mut upper_key = ui("A");
        upper_key.shift = true;
        let upper = for_rdp(&upper_key);
        match (lower, upper) {
            (
                KeyAction::Physical { code: a, shift: sa, .. },
                KeyAction::Physical { code: b, shift: sb, .. },
            ) => {
                assert_eq!(a, b, "A and a are the same physical key");
                assert!(!sa);
                assert!(sb);
            }
            other => panic!("expected two physical keys, got {other:?}"),
        }
    }

    #[test]
    fn an_extended_key_carries_the_e0_flag_in_bit_8() {
        // Slint reports the left arrow as U+F702, so that — not the glyph — is
        // what the UI hands over.
        match for_rdp(&ui("\u{F702}")) {
            // Left arrow = 0x4b with extended = 0x100 | 0x4b.
            KeyAction::Physical { code, .. } => assert_eq!(code, 0x14b),
            other => panic!("expected a physical key, got {other:?}"),
        }
    }

    #[test]
    fn arrows_and_function_keys_reach_the_wire_as_scancodes() {
        // These all used to fall through to a Unicode text event, which types a
        // private-use glyph on the remote desktop instead of moving the caret.
        for (text, code) in [
            ("\u{F700}", 0x148), // Up
            ("\u{F701}", 0x150), // Down
            ("\u{F703}", 0x14d), // Right
            ("\u{F729}", 0x147), // Home
            ("\u{F72C}", 0x149), // PageUp
            ("\u{F704}", 0x03b), // F1
        ] {
            match for_rdp(&ui(text)) {
                KeyAction::Physical { code: got, .. } => {
                    assert_eq!(got, code, "{text} should be scancode {code:#x}")
                }
                other => panic!("{text} should be a physical key, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_named_key_with_no_scancode_is_dropped_rather_than_typing_a_glyph() {
        // Pause is deliberately unmapped: set 1 spells it as a multi-byte E1
        // sequence, so there is no single scancode to send. It must be dropped
        // instead of going out as U+F730.
        assert_eq!(for_rdp(&ui("\u{F730}")), KeyAction::Ignore);
        assert_eq!(for_vnc(&ui("\u{F730}")), KeyAction::Ignore);
    }

    #[test]
    fn a_non_latin_character_is_sent_as_text_not_a_scancode() {
        assert_eq!(
            for_rdp(&ui("\u{4e2d}")),
            KeyAction::Text { ch: '\u{4e2d}' }
        );
    }

    #[test]
    fn shifted_punctuation_is_text_not_a_navigation_key() {
        // U+0021 is `!`; it once mapped to PageUp's scancode because the table
        // confused the DOM `keyCode` for the Unicode code point.
        assert_eq!(for_rdp(&ui("!")), KeyAction::Text { ch: '!' });
        assert_eq!(for_rdp(&ui("$")), KeyAction::Text { ch: '$' });
    }

    #[test]
    fn a_multi_character_name_maps_to_nothing_rather_than_guessing() {
        assert_eq!(for_rdp(&ui("SomeUnknownKey")), KeyAction::Ignore);
    }

    #[test]
    fn modifiers_are_recognised_so_they_can_be_forwarded_as_themselves() {
        assert!(ui("Shift").is_modifier());
        assert!(ui("Control").is_modifier());
        // Slint's own code for the left Control key.
        assert!(ui("\u{11}").is_modifier());
        assert!(!ui("a").is_modifier());
    }

    #[test]
    fn vnc_maps_printable_characters_to_their_keysym_directly() {
        // For Latin-1 the keysym is the character code, so this is exact.
        assert_eq!(
            for_vnc(&ui("a")),
            KeyAction::Physical {
                code: 0x61,
                down: true,
                shift: false
            }
        );
        assert_eq!(
            for_vnc(&ui("A")),
            KeyAction::Physical {
                code: 0x41,
                down: true,
                shift: false
            },
            "the keysym itself encodes the shift"
        );
    }

    #[test]
    fn vnc_maps_the_named_keys_to_their_private_use_keysyms() {
        match for_vnc(&ui("Return")) {
            KeyAction::Physical { code, .. } => assert_eq!(code, 0xff0d),
            other => panic!("expected Return's keysym, got {other:?}"),
        }
        match for_vnc(&ui("Left")) {
            KeyAction::Physical { code, .. } => assert_eq!(code, 0xff51),
            other => panic!("expected Left's keysym, got {other:?}"),
        }
    }

    #[test]
    fn vnc_reports_a_non_latin_character_as_text() {
        assert_eq!(
            for_vnc(&ui("\u{4e2d}")),
            KeyAction::Text { ch: '\u{4e2d}' }
        );
    }
}
