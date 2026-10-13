//! Slint key code → X11 keysym, for the VNC driver.
//!
//! RFB's key protocol ([RFC 6143] §7.5.4) sends an X11 keysym, not a scancode.
//! For Latin-1 the keysym is simply the character code; everything else lives in
//! the Unicode/private-use ranges X11 assigned long ago.
//!
//! Slint's winit backend reports every named key as a control or private-use
//! code point in `KeyEvent.text` — `\u{F700}` is `Key.Up`, `\u{0010}` is
//! `Key.Shift` (`i-slint-common/key_codes.rs`). Those codes are the ones that
//! actually arrive; the name and escape-sequence spellings are still accepted so
//! the mapping does not depend on the backend, but they are the fallback, not
//! the primary path.

use crate::rdp::embedded::input::keysym;

/// Keysym for the key Slint reported as `text`, or `None` when it has none.
///
/// `None` means the caller should fall back to a text event, which is how
/// non-Latin input still reaches the server.
pub fn keysym_for(text: &str) -> Option<u32> {
    // A single character is the common case, and Latin-1 covers all of ASCII.
    // The control codes Slint uses for named keys are below 0x20 and the
    // private-use ones above 0xFF, so neither can be mistaken for a character.
    let mut chars = text.chars();
    if let (Some(ch), None) = (chars.next(), chars.next()) {
        if let Some(code) = crate::rdp::embedded::latin1_keysym(ch) {
            return Some(code);
        }
    }
    let code = match text {
        // --- Slint's own codes for named keys ------------------------------
        // The control codes come first: Slint reports Return as U+000A and
        // Backspace as U+0008, so without these the Enter, Backspace, Tab and
        // Escape keys had no keysym and were dropped.
        "\u{8}" => keysym::BACKSPACE,
        "\u{9}" => keysym::TAB,
        "\u{a}" | "\r" => keysym::RETURN,
        "\u{1b}" => keysym::ESCAPE,
        "\u{10}" => keysym::SHIFT_L,
        "\u{15}" => keysym::SHIFT_R,
        "\u{11}" => keysym::CONTROL_L,
        "\u{16}" => keysym::CONTROL_R,
        "\u{12}" => keysym::ALT_L,
        "\u{13}" => keysym::ALT_R,
        "\u{14}" => keysym::CAPS_LOCK,
        "\u{17}" => keysym::SUPER_L,
        "\u{18}" => keysym::SUPER_R,
        // Shift+Tab has its own keysym rather than being Tab with a modifier.
        "\u{19}" => keysym::ISO_LEFT_TAB,
        "\u{7f}" => keysym::DELETE,
        "\u{F700}" => keysym::UP,
        "\u{F701}" => keysym::DOWN,
        "\u{F702}" => keysym::LEFT,
        "\u{F703}" => keysym::RIGHT,
        // F1..F12 are a contiguous block from 0xffbe. Spelled out because Rust
        // has no range pattern over strings.
        "\u{F704}" => keysym::F1,
        "\u{F705}" => keysym::F1 + 1,
        "\u{F706}" => keysym::F1 + 2,
        "\u{F707}" => keysym::F1 + 3,
        "\u{F708}" => keysym::F1 + 4,
        "\u{F709}" => keysym::F1 + 5,
        "\u{F70A}" => keysym::F1 + 6,
        "\u{F70B}" => keysym::F1 + 7,
        "\u{F70C}" => keysym::F1 + 8,
        "\u{F70D}" => keysym::F1 + 9,
        "\u{F70E}" => keysym::F1 + 10,
        "\u{F70F}" => keysym::F1 + 11,
        "\u{F727}" => keysym::INSERT,
        "\u{F729}" => keysym::HOME,
        "\u{F72B}" => keysym::END,
        "\u{F72C}" => keysym::PAGE_UP,
        "\u{F72D}" => keysym::PAGE_DOWN,
        "\u{F72F}" => keysym::SCROLL_LOCK,
        "\u{F735}" => keysym::MENU,
        // --- name spellings, for a backend that sends them ------------------
        "Backspace" => keysym::BACKSPACE,
        "Tab" => keysym::TAB,
        "Return" | "Enter" | "NumpadEnter" => keysym::RETURN,
        "Escape" | "Esc" => keysym::ESCAPE,
        "Delete" | "NumpadEnd" => keysym::DELETE,
        "Home" | "\u{1b}[H" | "\u{1b}OH" => keysym::HOME,
        "End" | "\u{1b}[F" | "\u{1b}OF" => keysym::END,
        "Insert" | "\u{1b}[2~" => keysym::INSERT,
        "PageUp" | "\u{1b}[5~" => keysym::PAGE_UP,
        "PageDown" | "\u{1b}[6~" => keysym::PAGE_DOWN,
        "Left" | "\u{1b}[D" | "\u{1b}OD" => keysym::LEFT,
        "Right" | "\u{1b}[C" | "\u{1b}OC" => keysym::RIGHT,
        "Up" | "\u{1b}[A" | "\u{1b}OA" => keysym::UP,
        "Down" | "\u{1b}[B" | "\u{1b}OB" => keysym::DOWN,
        "F1" => keysym::F1,
        "F2" => keysym::F1 + 1,
        "F3" => keysym::F1 + 2,
        "F4" => keysym::F1 + 3,
        "F5" => keysym::F1 + 4,
        "F6" => keysym::F1 + 5,
        "F7" => keysym::F1 + 6,
        "F8" => keysym::F1 + 7,
        "F9" => keysym::F1 + 8,
        "F10" => keysym::F1 + 9,
        "F11" => keysym::F1 + 10,
        "F12" => keysym::F1 + 11,
        // Modifiers have their own keysyms so the server can tell left from
        // right, which a plain character cannot express.
        "Shift" | "\u{ffe1}" => keysym::SHIFT_L,
        "\u{ffe2}" => keysym::SHIFT_R,
        "Control" | "\u{ffe3}" => keysym::CONTROL_L,
        "\u{ffe4}" => keysym::CONTROL_R,
        "Alt" | "\u{ffe9}" => keysym::ALT_L,
        "AltGr" | "\u{ffea}" => keysym::ALT_R,
        "Super" | "Meta" | "\u{ffeb}" => keysym::SUPER_L,
        "\u{ffec}" => keysym::SUPER_R,
        // Unicode arrows, for a backend that sends the glyph rather than a name.
        "\u{2190}" => keysym::LEFT,
        "\u{2192}" => keysym::RIGHT,
        "\u{2191}" => keysym::UP,
        "\u{2193}" => keysym::DOWN,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_characters_are_their_own_keysym() {
        assert_eq!(keysym_for("a"), Some(0x61));
        assert_eq!(keysym_for("A"), Some(0x41));
        assert_eq!(keysym_for("0"), Some(0x30));
        assert_eq!(keysym_for(" "), Some(0x20));
    }

    #[test]
    fn the_editing_keys_slint_sends_as_control_codes_have_their_own_keysyms() {
        // Slint reports Return as U+000A and Backspace as U+0008, so these are
        // the codes that actually arrive. They must resolve to the named key's
        // keysym — 0xff0d for Return, not the bare 0x0a newline RFB reads
        // specially — or Enter, Backspace, Tab and Escape do nothing at all.
        assert_eq!(keysym_for("\n"), Some(0xff0d));
        assert_eq!(keysym_for("\u{8}"), Some(0xff08));
        assert_eq!(keysym_for("\u{9}"), Some(0xff09));
        assert_eq!(keysym_for("\u{1b}"), Some(0xff1b));
        assert_eq!(keysym_for("\u{7f}"), Some(0xffff));
        // The name spellings still agree with them.
        assert_eq!(keysym_for("Return"), keysym_for("\n"));
        assert_eq!(keysym_for("Backspace"), keysym_for("\u{8}"));
    }

    #[test]
    fn the_private_use_codes_slint_sends_agree_with_the_name_spellings() {
        for code in ["\u{F700}", "\u{F701}", "\u{F702}", "\u{F703}"] {
            assert!(keysym_for(code).is_some(), "{code} must resolve");
        }
        assert_eq!(keysym_for("\u{F700}"), keysym_for("Up"));
        assert_eq!(keysym_for("\u{F72C}"), keysym_for("PageUp"));
        assert_eq!(keysym_for("\u{F72D}"), keysym_for("PageDown"));
        assert_eq!(keysym_for("\u{F729}"), keysym_for("Home"));
        assert_eq!(keysym_for("\u{F727}"), keysym_for("Insert"));
        assert_eq!(keysym_for("\u{F704}"), keysym_for("F1"));
        assert_eq!(keysym_for("\u{F70F}"), keysym_for("F12"));
        // The modifier codes too: without them VNC never saw Ctrl or Alt.
        assert_eq!(keysym_for("\u{11}"), keysym_for("Control"));
        assert_eq!(keysym_for("\u{16}"), Some(keysym::CONTROL_R));
        assert_eq!(keysym_for("\u{12}"), keysym_for("Alt"));
    }

    #[test]
    fn named_navigation_keys_match_x11() {
        assert_eq!(keysym_for("Up"), Some(0xff52));
        assert_eq!(keysym_for("Down"), Some(0xff54));
        assert_eq!(keysym_for("Left"), Some(0xff51));
        assert_eq!(keysym_for("Right"), Some(0xff53));
        assert_eq!(keysym_for("Home"), Some(0xff50));
        assert_eq!(keysym_for("End"), Some(0xff57));
        assert_eq!(keysym_for("PageUp"), Some(0xff55));
        assert_eq!(keysym_for("PageDown"), Some(0xff56));
        assert_eq!(keysym_for("Insert"), Some(0xff63));
        assert_eq!(keysym_for("Delete"), Some(0xffff));
    }

    #[test]
    fn escape_sequence_spellings_agree_with_the_names() {
        // The same key arrives under two spellings depending on the backend, and
        // both must resolve identically.
        assert_eq!(keysym_for("\u{1b}[A"), keysym_for("Up"));
        assert_eq!(keysym_for("\u{1b}OD"), keysym_for("Left"));
        assert_eq!(keysym_for("\u{1b}[5~"), keysym_for("PageUp"));
    }

    #[test]
    fn f_keys_are_a_contiguous_block() {
        assert_eq!(keysym_for("F1"), Some(0xffbe));
        assert_eq!(keysym_for("F12"), Some(0xffc9));
        assert_eq!(keysym_for("F13"), None);
    }

    #[test]
    fn left_and_right_modifiers_stay_distinct() {
        assert_ne!(keysym_for("Shift"), keysym_for("\u{ffe2}"));
        assert_ne!(keysym_for("Control"), keysym_for("\u{ffe4}"));
    }

    #[test]
    fn a_non_latin_character_has_no_keysym_so_it_falls_back_to_text() {
        assert_eq!(keysym_for("\u{4e2d}"), None);
    }
}
