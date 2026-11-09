#[path = "struct/state.rs"]
mod state;
#[path = "struct/find.rs"]
mod find;
// (#cmd-hint-p1) 终端行缓冲的状态类型（LineState / LineSnapshot）。
#[path = "struct/line.rs"]
mod line;

#[path = "impls/input.rs"]
mod input;
// (#send-cmd-p1) 发送命令窗口的文字检测(危险词 + 续行风险):单一出处。
// Slint 的 string 没有逐字符访问,引号扫描/块计数必须在 Rust 侧做。
// (#send-cmd-v2) 升 pub:纯函数检测库,examples/send_check_probe 探针也要用。
#[path = "impls/send_check.rs"]
pub mod send_check;
// (#multiline-paste-hifi 2026-11-10) 多行粘贴确认弹框的分析层:档位/A·B 危险
// 行/控制字符计数/折叠窗口/预览行模型/页脚文案。复用 send_check 的防误报
// 口径,规则表独立。pub:examples/paste_confirm_shot 出图探针也要用。
#[path = "impls/paste_review.rs"]
pub mod paste_review;
#[path = "impls/json_output.rs"]
mod json_output;
#[path = "impls/charset.rs"]
mod charset;
#[path = "impls/encoding.rs"]
mod encoding;
#[path = "impls/local.rs"]
pub(crate) mod local;
#[path = "impls/output_highlight.rs"]
mod output_highlight;
#[path = "impls/presentation.rs"]
mod presentation;
#[path = "impls/render.rs"]
mod render;
#[path = "impls/render_gate.rs"]
mod render_gate;
#[path = "impls/serial.rs"]
pub(crate) mod serial;
#[path = "impls/telnet.rs"]
pub(crate) mod telnet;
#[path = "impls/term_buffer.rs"]
mod term_buffer;
#[path = "impls/terminal_find.rs"]
mod terminal_find;
// (#cmd-hint-p1) 终端内联提示框的行缓冲（不信按键、信回显）。
#[path = "impls/terminal_hint.rs"]
mod terminal_hint;
#[path = "impls/zmodem.rs"]
pub(crate) mod zmodem;

#[cfg(windows)]
pub(crate) use input::c0_letter_key_down;
#[cfg(test)]
pub(crate) use input::normalize_pasted_newlines;
pub(crate) use input::{
    bare_ctrl_marker_workaround_enabled, encode_command_bar_input, encode_mouse_event,
    encode_pasted_text, is_terminal_interrupt, key_to_pty_bytes, paste_requires_large_review,
    should_drop_bare_ctrl_marker,
    terminal_uses_bracketed_paste,
};
pub(crate) use charset::CharsetTracker;
pub(crate) use encoding::TerminalEncoding;
pub(crate) use json_output::format_json_output;
#[cfg(any(target_os = "windows", test))]
pub(crate) use input::windows_process_ctrl_release;
#[cfg(any(target_os = "windows", test))]
pub(crate) use state::CtrlKeySide;
pub(crate) use output_highlight::{
    compile_highlight_rules, parse_color, preset_enabled, rainbow_parens_enabled, rendered_slot,
    HIGHLIGHT_PRESETS, HighlightPreset, RAINBOW_PRESET_ID,
};
pub(crate) use presentation::{highlight_plain_output, render_term_span};
#[cfg(test)]
pub(crate) use presentation::{text_cell_width, vt_span_colors};
pub(crate) use render::{
    build_row, cell_prefix, char_after_cell_end, char_at_cell_start, detect_scroll, MAX_HISTORY,
    RAW_CAP,
};
pub(crate) use state::{
    BuiltScreen, CompiledOutputRule, CsiState, HistSpan, Line, RenderGates, RuleColor,
    TabRenderGate, TermBuffer, TermBufferHandle, TermBuffers,
};
pub(crate) use find::{FindHits, FindOptions, FindState};
// (#cmd-hint-p1) 终端行缓冲：状态类型 + 观测器 + 命令起点定位。
// `LineSnapshot` 是 `observe()` 的返回类型，调用方只用值不命名类型，
// 所以不必再导出（导出它反而会得到 unused import 警告）。
pub(crate) use line::LineState;
pub(crate) use terminal_hint::{command_line, HintObserver};
