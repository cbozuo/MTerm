//! RDP keyboard translation: Slint `KeyEvent` → set-1 scancode.
//!
//! [MS-RDPBCGR] 2.2.9.1.1.3.1.2.1 "Keyboard Event (Key Down)" carries a
//! set-1 scancode, optionally in the E0 (extended) prefix space for keys the
//! original PC/AT keyboard did not have. Slint gives us a Unicode character and
//! modifiers instead, so the mapping has to be written out by hand.
//!
//! Two rules drive the table below:
//!   * the character determines the *unshifted* key, and Shift is a separate
//!     scancode (so `a` is 0x1E and `A` is 0x1E + shift) — this is what lets a
//!     remote desktop distinguish left/right shift, which a pure characters
//!     table would collapse;
//!   * modifier keys map to themselves, because the remote side tracks its own
//!     modifier state from the same scancodes.

use crate::rdp::embedded::input::SlintKey;

/// A scancode plus whether it lives in the extended (E0) space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCode {
    pub code: u8,
    pub extended: bool,
}

impl ScanCode {
    const fn new(code: u8, extended: bool) -> Self {
        Self { code, extended }
    }
}

/// Scancode for the physical key that produces `ch` on a US layout, ignoring
/// modifiers.
///
/// `None` means "this character has no dedicated key" — the caller should send
/// it as a Unicode key event instead (IME text, dead-key results, pasted
/// characters).
pub fn scancode_for_char(ch: char) -> Option<ScanCode> {
    let code = match ch.to_ascii_lowercase() {
        '1' => 0x02,
        '2' => 0x03,
        '3' => 0x04,
        '4' => 0x05,
        '5' => 0x06,
        '6' => 0x07,
        '7' => 0x08,
        '8' => 0x09,
        '9' => 0x0a,
        '0' => 0x0b,
        '-' => 0x0c,
        '=' => 0x0d,
        'q' => 0x10,
        'w' => 0x11,
        'e' => 0x12,
        'r' => 0x13,
        't' => 0x14,
        'y' => 0x15,
        'u' => 0x16,
        'i' => 0x17,
        'o' => 0x18,
        'p' => 0x19,
        '[' => 0x1a,
        ']' => 0x1b,
        'a' => 0x1e,
        's' => 0x1f,
        'd' => 0x20,
        'f' => 0x21,
        'g' => 0x22,
        'h' => 0x23,
        'j' => 0x24,
        'k' => 0x25,
        'l' => 0x26,
        ';' => 0x27,
        '\'' => 0x28,
        '`' => 0x29,
        '\\' => 0x2b,
        'z' => 0x2c,
        'x' => 0x2d,
        'c' => 0x2e,
        'v' => 0x2f,
        'b' => 0x30,
        'n' => 0x31,
        'm' => 0x32,
        ',' => 0x33,
        '.' => 0x34,
        '/' => 0x35,
        ' ' => 0x39,
        _ => return None,
    };
    Some(ScanCode::new(code, false))
}

/// Scancode for a non-printable key, keyed by the character Slint reports it as.
///
/// Slint does **not** send key names. Its winit backend assigns every named key
/// a control or private-use code point and delivers that as `KeyEvent.text`
/// (`i-slint-common/key_codes.rs` is the table; `Key.Up` is `\u{F700}`,
/// `Key.Shift` is `\u{0010}`). The codes below are copied from it verbatim.
///
/// Getting one wrong is not a cosmetic mistake: a code point that collides with
/// a printable character silently rebinds that character. `'\u{21}'` is `!`, not
/// PageUp, so an entry that puts a navigation scancode there makes `Shift`+`1`
/// press Up on the remote desktop. The table is therefore exhaustive over what
/// the backend can produce, and anything outside it falls through to
/// [`scancode_for_char`].
pub fn scancode_for_named(ch: char) -> Option<ScanCode> {
    let code = match ch {
        // --- control codes (< 0x20, or DEL) --------------------------------
        '\u{8}' => ScanCode::new(0x0e, false),     // Backspace
        '\t' => ScanCode::new(0x0f, false),        // Tab
        '\n' | '\r' => ScanCode::new(0x1c, false), // Return
        '\u{1b}' => ScanCode::new(0x01, false),    // Escape
        '\u{19}' => ScanCode::new(0x0f, false),    // Backtab; Tab plus Shift
        '\u{7f}' => ScanCode::new(0x53, true),     // Delete
        // --- modifier keys -------------------------------------------------
        // Slint reports the modifiers themselves as key events, and forwarding
        // them as physical keys is what lets the remote desktop see Ctrl and Alt
        // as keys it can hold — Ctrl+Alt+Del, Alt+Tab, Ctrl+Shift+Esc — rather
        // than inferring them from a flag on some other key.
        '\u{10}' => ScanCode::new(0x2a, false), // Shift (left)
        '\u{11}' => ScanCode::new(0x1d, false), // Control (left)
        '\u{12}' => ScanCode::new(0x38, false), // Alt (left)
        '\u{13}' => ScanCode::new(0x38, true),  // AltGr (right Alt)
        '\u{14}' => ScanCode::new(0x3a, false), // CapsLock
        '\u{15}' => ScanCode::new(0x36, false), // Shift (right)
        '\u{16}' => ScanCode::new(0x1d, true),  // Control (right)
        '\u{17}' => ScanCode::new(0x5b, true),  // Meta / Super (left)
        '\u{18}' => ScanCode::new(0x5c, true),  // Meta / Super (right)
        // --- navigation and editing ----------------------------------------
        // These live in the extended (E0) block. The *same* numeric code with
        // extended=false is a different key on the numeric keypad, which is
        // exactly why the flag is not optional.
        '\u{F700}' => ScanCode::new(0x48, true), // Up
        '\u{F701}' => ScanCode::new(0x50, true), // Down
        '\u{F702}' => ScanCode::new(0x4b, true), // Left
        '\u{F703}' => ScanCode::new(0x4d, true), // Right
        '\u{F727}' => ScanCode::new(0x52, true), // Insert
        '\u{F729}' => ScanCode::new(0x47, true), // Home
        '\u{F72B}' => ScanCode::new(0x4f, true), // End
        '\u{F72C}' => ScanCode::new(0x49, true), // PageUp
        '\u{F72D}' => ScanCode::new(0x51, true), // PageDown
        // --- function keys --------------------------------------------------
        // F1..F10 are contiguous; F11 and F12 jump to 0x57/0x58. None of them is
        // extended: they are single-byte codes in set 1.
        '\u{F704}'..='\u{F70D}' => ScanCode::new(0x3b + (ch as u32 - 0xf704) as u8, false),
        '\u{F70E}' => ScanCode::new(0x57, false), // F11
        '\u{F70F}' => ScanCode::new(0x58, false), // F12
        // --- lock and menu keys ---------------------------------------------
        '\u{F72F}' => ScanCode::new(0x46, false), // ScrollLock
        '\u{F735}' => ScanCode::new(0x5d, true),  // Menu (context menu)
        // PrintScreen, Pause and Break are deliberately absent: each one is a
        // multi-byte E1/E0 sequence in set 1, not a single scancode, and sending
        // half of one is worse than sending nothing. The caller drops unmapped
        // non-text codes (see `keyinput`), so these simply do nothing.
        //
        // Unicode glyph spellings, for a backend that sends the character rather
        // than the platform code.
        '\u{2190}' => ScanCode::new(0x4b, true), // ←
        '\u{2192}' => ScanCode::new(0x4d, true), // →
        '\u{2191}' => ScanCode::new(0x48, true), // ↑
        '\u{2193}' => ScanCode::new(0x50, true), // ↓
        _ => return None,
    };
    Some(code)
}

/// Keys a *different* backend might report by name instead of by code point.
///
/// Slint's winit backend — the one this app uses — never sends these; it sends
/// the control/private-use codes [`scancode_for_named`] handles. The table is
/// kept because it costs nothing and makes the mapping independent of which
/// backend is in use, and because the escape-sequence spellings (`"\u{1b}[5~"`)
/// are what a terminal-style source would produce.
fn named_fallback(name: &str) -> Option<ScanCode> {
    let code = match name {
        "F1" => ScanCode::new(0x3b, false),
        "F2" => ScanCode::new(0x3c, false),
        "F3" => ScanCode::new(0x3d, false),
        "F4" => ScanCode::new(0x3e, false),
        "F5" => ScanCode::new(0x3f, false),
        "F6" => ScanCode::new(0x40, false),
        "F7" => ScanCode::new(0x41, false),
        "F8" => ScanCode::new(0x42, false),
        "F9" => ScanCode::new(0x43, false),
        "F10" => ScanCode::new(0x44, false),
        // F11/F12 are single-byte codes in set 1 like the rest of the row; they
        // are not in the extended block.
        "F11" => ScanCode::new(0x57, false),
        "F12" => ScanCode::new(0x58, false),
        "Home" | "\u{1b}[H" | "\u{1b}OH" => ScanCode::new(0x47, true),
        "End" | "\u{1b}[F" | "\u{1b}OF" => ScanCode::new(0x4f, true),
        "Insert" | "\u{1b}[2~" => ScanCode::new(0x52, true),
        "Delete" | "\u{1b}[3~" => ScanCode::new(0x53, true),
        // PageUp/PageDown sit *between* the arrow keys in the E0 block: Home 47,
        // Up 48, PageUp 49 … End 4F, Down 50, PageDown 51. They are not Up/Down.
        "PageUp" | "\u{1b}[5~" => ScanCode::new(0x49, true),
        "PageDown" | "\u{1b}[6~" => ScanCode::new(0x51, true),
        "Left" | "\u{1b}[D" | "\u{1b}OD" => ScanCode::new(0x4b, true),
        "Right" | "\u{1b}[C" | "\u{1b}OC" => ScanCode::new(0x4d, true),
        "Up" | "\u{1b}[A" | "\u{1b}OA" => ScanCode::new(0x48, true),
        "Down" | "\u{1b}[B" | "\u{1b}OB" => ScanCode::new(0x50, true),
        // The numeric keypad's Enter: the same numeric code as the main Enter,
        // but in the extended (E0) space, so the two must not be collapsed.
        "NumpadEnter" | "\u{1b}OM" => ScanCode::new(0x1c, true),
        "NumpadEnd" | "\u{1b}Oo" => ScanCode::new(0x53, false),
        "Shift" => ScanCode::new(0x2a, false),
        "Control" => ScanCode::new(0x1d, false),
        "Alt" | "AltGr" => ScanCode::new(0x38, false),
        "Super" | "Meta" => ScanCode::new(0x5b, true),
        "Escape" | "Esc" => ScanCode::new(0x01, false),
        "Tab" => ScanCode::new(0x0f, false),
        "Backspace" => ScanCode::new(0x0e, false),
        "Return" | "Enter" => ScanCode::new(0x1c, false),
        _ => return None,
    };
    Some(code)
}

/// Translate one Slint key event into an RDP keyboard action.
///
/// Returns the scancode plus the shift bit the keyboard event must carry.
/// `None` means the character has no physical key; the caller sends it as a
/// Unicode key event instead.
pub fn translate(key: &SlintKey) -> Option<(ScanCode, bool)> {
    // Order matters. A named spelling ("F1", "\u{1b}[5~") is matched first
    // because it is a *whole string* and can never be a single character, so it
    // cannot shadow anything. The non-printable code-point table comes next: it
    // is keyed by the control/private-use codes Slint's own backend produces,
    // and it must be consulted before the printable table so that a code point
    // that happens to look printable can never be reinterpreted as a character
    // (that is exactly how `!` once became PageUp). Printable characters come
    // last, and their shift state comes from the modifier rather than from the
    // character, so `a` and `A` stay the same physical key.
    named_fallback(&key.name)
        .or_else(|| scancode_for_named(key.text))
        .or_else(|| scancode_for_char(key.text))
        .map(|code| (code, key.shift))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: char, shift: bool) -> SlintKey {
        SlintKey {
            name: text.to_string(),
            text,
            ctrl: false,
            alt: false,
            shift,
            meta: false,
            down: true,
        }
    }

    /// A key the UI reports by name rather than as a character.
    fn named_key(name: &str) -> SlintKey {
        SlintKey {
            name: name.to_string(),
            text: name.chars().next().unwrap_or('\0'),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            down: true,
        }
    }

    #[test]
    fn letters_map_to_set1_scancodes_and_carry_shift_separately() {
        // 'a' and 'A' are the same physical key — only the shift bit differs.
        assert_eq!(translate(&key('a', false)), Some((ScanCode::new(0x1e, false), false)));
        assert_eq!(translate(&key('A', true)), Some((ScanCode::new(0x1e, false), true)));
        assert_eq!(translate(&key('z', false)), Some((ScanCode::new(0x2c, false), false)));
    }

    #[test]
    fn digits_and_punctuation_use_their_us_layout_positions() {
        assert_eq!(translate(&key('1', false)), Some((ScanCode::new(0x02, false), false)));
        assert_eq!(translate(&key('0', false)), Some((ScanCode::new(0x0b, false), false)));
        assert_eq!(translate(&key('/', false)), Some((ScanCode::new(0x35, false), false)));
        assert_eq!(translate(&key(';', false)), Some((ScanCode::new(0x27, false), false)));
    }

    #[test]
    fn navigation_keys_are_extended_and_must_not_be_confused_with_the_keypad() {
        let (up, _) = translate(&key('↑', false)).unwrap();
        assert!(up.extended);
        // The keypad's 8 has the same numeric code but is NOT extended.
        let keypad_up = ScanCode::new(0x48, false);
        assert_ne!(up, keypad_up);
    }

    #[test]
    fn enter_has_two_scancodes_one_per_keyboard() {
        let (main, _) = translate(&key('\n', false)).unwrap();
        assert_eq!(main, ScanCode::new(0x1c, false));
        // The keypad's Enter is the main Enter's numeric code in the extended
        // space — a different key on the wire, so it must not collapse.
        let (numpad, _) = translate(&named_key("\u{1b}OM")).unwrap();
        assert_eq!(numpad, ScanCode::new(0x1c, true));
    }

    #[test]
    fn shift_keys_map_to_themselves_so_left_and_right_stay_distinct() {
        assert_eq!(
            translate(&key('\u{10}', false)),
            Some((ScanCode::new(0x2a, false), false)),
            "Slint reports the left Shift as U+0010"
        );
        assert_eq!(
            translate(&key('\u{15}', false)),
            Some((ScanCode::new(0x36, false), false)),
            "…and the right Shift as U+0015"
        );
    }

    #[test]
    fn ctrl_and_alt_are_forwarded_as_physical_keys() {
        // A remote desktop has to hold these to see Ctrl+Alt+Del and Alt+Tab, so
        // the modifiers themselves must map to their own scancodes and not only
        // to a flag on another key.
        assert_eq!(
            translate(&key('\u{11}', false)),
            Some((ScanCode::new(0x1d, false), false))
        );
        assert_eq!(
            translate(&key('\u{12}', false)),
            Some((ScanCode::new(0x38, false), false))
        );
        assert_eq!(
            translate(&key('\u{16}', false)),
            Some((ScanCode::new(0x1d, true), false))
        );
    }

    #[test]
    fn non_latin_characters_have_no_scancode_so_they_go_out_as_unicode() {
        assert_eq!(translate(&key('\u{4e2d}', false)), None);
    }

    #[test]
    fn f_keys_are_single_byte_codes_in_the_main_block() {
        assert_eq!(translate(&key('\u{F704}', false)), Some((ScanCode::new(0x3b, false), false)));
        assert_eq!(translate(&key('\u{F70D}', false)), Some((ScanCode::new(0x44, false), false)));
        // F11/F12 jump the gap to 0x57/0x58 but are still not extended keys.
        assert_eq!(translate(&key('\u{F70E}', false)), Some((ScanCode::new(0x57, false), false)));
        assert_eq!(translate(&key('\u{F70F}', false)), Some((ScanCode::new(0x58, false), false)));
    }

    #[test]
    fn navigation_keys_use_the_codes_slint_actually_sends() {
        // U+F700..U+F703 are the arrows; the table used to key off the Unicode
        // glyphs and the `Key.Up` *names*, neither of which Slint ever sends, so
        // every arrow fell through to a Unicode text event.
        assert_eq!(translate(&key('\u{F700}', false)), Some((ScanCode::new(0x48, true), false)));
        assert_eq!(translate(&key('\u{F701}', false)), Some((ScanCode::new(0x50, true), false)));
        assert_eq!(translate(&key('\u{F702}', false)), Some((ScanCode::new(0x4b, true), false)));
        assert_eq!(translate(&key('\u{F703}', false)), Some((ScanCode::new(0x4d, true), false)));
        assert_eq!(translate(&key('\u{F729}', false)), Some((ScanCode::new(0x47, true), false)));
        assert_eq!(translate(&key('\u{F72B}', false)), Some((ScanCode::new(0x4f, true), false)));
        assert_eq!(translate(&key('\u{F727}', false)), Some((ScanCode::new(0x52, true), false)));
    }

    #[test]
    fn page_up_and_page_down_are_not_the_arrow_keys() {
        // The E0 block interleaves them: Up 48, PageUp 49, Down 50, PageDown 51.
        // Mapping PageUp onto Up is invisible in a unit test that only checks
        // "something is extended", and wrong on the wire.
        assert_eq!(translate(&key('\u{F72C}', false)), Some((ScanCode::new(0x49, true), false)));
        assert_eq!(translate(&key('\u{F72D}', false)), Some((ScanCode::new(0x51, true), false)));
        assert_eq!(translate(&key('↑', false)), Some((ScanCode::new(0x48, true), false)));
        assert_ne!(translate(&key('\u{F72C}', false)), translate(&key('↑', false)));
    }

    #[test]
    fn shifted_punctuation_is_not_mistaken_for_a_navigation_key() {
        // The table once had `'\u{21}' => PageUp` — but U+0021 is `!`, so
        // Shift+1 pressed Up on the remote desktop. These characters have no
        // unshifted key in the table, so they must fall through to the Unicode
        // path rather than resolve to a control key.
        for ch in ['!', '"', '#', '$'] {
            assert_eq!(translate(&key(ch, true)), None, "{ch} must not map to a key");
        }
        // …and the digits they are shifted from still resolve normally.
        assert_eq!(translate(&key('1', false)), Some((ScanCode::new(0x02, false), false)));
    }

    #[test]
    fn the_named_spellings_still_agree_with_the_code_points() {
        // Both spellings must resolve identically, so a backend that sends names
        // cannot disagree with one that sends code points.
        assert_eq!(translate(&named_key("PageUp")), translate(&key('\u{F72C}', false)));
        assert_eq!(translate(&named_key("F12")), translate(&key('\u{F70F}', false)));
        assert_eq!(translate(&named_key("Shift")), translate(&key('\u{10}', false)));
    }
}
