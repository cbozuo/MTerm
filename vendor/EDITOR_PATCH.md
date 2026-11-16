# Native editor patch (Slint 1.18.1) — MTerm

Ported into MTerm from the same-shaped patch in
[`yituorou/meatshell`](https://github.com/yituorou/meatshell) (MIT OR Apache-2.0),
adapted to MTerm's vendored crates. The three crates below are the published
1.18.1 crates (upstream license files retained) selected by `[patch.crates-io]`.
They must be upgraded together; do not remove only one override. Every change is
marked with an `MTerm editor patch` (previously `MeatShell editor patch`) comment.

Why this exists: MTerm's first built-in editor drew the document itself — one
`Rectangle` per line, one absolutely positioned `Text` per highlighted span at
`x = col * char_width`, plus a self-drawn caret and selection. That model is a
terminal's, and it cannot be made correct for free-form text: a monospace face
has no CJK glyphs, so CJK falls back to a proportional font whose advance is not
two grid cells, and every cell after the first CJK character drifts. Porting
this patch hands the text back to Slint's own shaper: geometry (wrap, cursor,
selection, IME, hit-testing) is the stock `TextInput`'s, and the only thing the
patch adds is per-range glyph **colour**.

## i-slint-compiler

- `builtin_elements.rs` (`item! { TextInput { … } }`) declares three opt-in
  properties: `syntax-source`, `syntax-spans`, `line-number-width`. Existing
  inputs keep empty/zero defaults, so nothing else changes.

## i-slint-core

- `items/text.rs` adds the three `TextInput` properties behind the declarations
  above, plus `TextInput::syntax_colors()`, which returns the spans **only** when
  `syntax-source` equals the displayed text, no IME preedit is active and the
  field is not a password — so a debounced highlight result can never colour the
  wrong text.
- `item_rendering.rs` adds `RenderString::syntax_colors()` (default `None`).
- `textlayout/sharedparley/shaping.rs` adds
  `create_text_paragraphs_with_syntax()` and `parse_syntax_spans()`. The
  paragraph split, byte ranges and font metrics are exactly those of plain text,
  so native shaping, wrap, cursor, clipboard, selection and IME are unchanged;
  only glyph colours differ. Because shaping reads these properties, Slint's own
  `TextLayoutCache` reshapes when they change and reuses the shaped text
  otherwise (typing reshapes once; scrolling, selecting and cursor blinking do
  not).
- `textlayout/sharedparley.rs` adds `draw_line_numbers()`: a positive
  `line-number-width` turns an input into a virtual gutter that shares the
  editor's exact source, font, width and wrapping, so each logical line's number
  sits on the first visual baseline of its paragraph. Only the numbers inside the
  clip are shaped and drawn, instead of one Slint item per logical line.

### Wire format

Syntax spans are sorted, non-overlapping UTF-8 **byte** ranges encoded as
`start:end:AARRGGBB;`. Ranges that are out of order, overlapping, out of bounds
or mid-character are dropped at the renderer boundary even though the
application is trusted.

## MTerm-local hunks kept in the same files

- `items/text.rs` also carries MTerm's `#tip-r17` patch: gate caret painting on
  `text_cursor_width() > 0` so the send-command editor's own overlay caret does
  not double-draw. Re-apply it when rebasing.

## Scope note

MTerm enables these properties only for the built-in editor's `TextInput`s
(`ui/editor_window.slint`). Every other text input keeps the empty defaults, so
no existing UI behaviour changes. The renderer does not parse programming
languages: language detection and lexical tokenization live in
`src/editor/impls/highlight_line.rs` / `highlight_ts.rs` (tree-sitter), and the
host encodes their output into the span string.

Validation: `cargo test --lib editor` covers the tokenizer/encoder, and
`cargo run --profile fast --example editor_shot` renders the editor for a visual
check against `docs/design/sftp-editor-hifi-2026-11-14.html`.
