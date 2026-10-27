use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

use crate::terminal::charset::CharsetTracker;
use crate::terminal::find::FindState;
use crate::ui::TermSpan;

#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CtrlKeySide {
    Left,
    Right,
}

/// Per-terminal state used by normal and alternate-screen rendering.
pub(crate) struct TermBuffer {
    pub(crate) parser: vt100::Parser,
    /// (#find-bar 2026-10-27) 查找栏全部状态（词 / Aa / 正则 / 全缓冲命中 /
    /// 当前匹配）。替代原裸 `find_query: String`。
    pub(crate) find: FindState,
    pub(crate) is_dark: bool,
    /// 当前主题的色表（#theme-split）：ANSI 16 色与默认前景/背景按它查表——
    /// 换主题时 UI 与终端同源换装（presentation.rs 的 render_term_span 读它）。
    pub(crate) palette: &'static crate::theme::palettes::Palette,
    /// 已按优先级排好的高亮规则（自定义在前、预设在后）；空 = 关闭
    pub(crate) highlight_rules: Vec<CompiledOutputRule>,
    /// 彩虹括号（按嵌套层级循环着色，非正则）开关
    pub(crate) rainbow_parens: bool,
    pub(crate) json_format_output: bool,
    /// Honor VT100 line-drawing (DEC Special Graphics) via SCS designators and
    /// SO/SI even when the session encoding is UTF-8 (#376, PuTTY's option).
    pub(crate) vt100_drawing: bool,
    pub(crate) charset: CharsetTracker,
    pub(crate) interactive_echo_until: std::time::Instant,
    pub(crate) sel_anchor: Option<(usize, u16)>,
    pub(crate) sel_focus: Option<(usize, u16)>,
    pub(crate) sel_ranges: Vec<((usize, u16), (usize, u16))>,
    /// Whether the remote application is tracking the mouse (vt100
    /// `mouse_protocol_mode() != None`). When true, clicks and drags are
    /// forwarded to the PTY instead of starting a local drag-selection, so
    /// btop/htop/mc can be operated with the mouse (#terminal-mouse).
    pub(crate) mouse_tracked: bool,
    pub(crate) history: VecDeque<Line>,
    pub(crate) prev: Vec<Line>,
    pub(crate) view_offset: usize,
    /// Fractional scrollback rows not yet applied. Wheel deltas arrive as
    /// pixel fractions of a row (trackpad + macOS momentum decay); keeping
    /// the remainder here lets the momentum tail glide to a stop instead of
    /// stepping a fixed amount per event.
    pub(crate) scroll_accum: f32,
    pub(crate) displayed_text: Vec<String>,
    pub(crate) csi_state: CsiState,
    pub(crate) csi_pending: Vec<u8>,
    pub(crate) raw: VecDeque<u8>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum CsiState {
    Normal,
    Esc,
    Csi,
    /// Buffering an SCS designator (`ESC ( X`, `ESC ) X`, …); payload holds the
    /// designator intro byte. Bytes still pass through to the display verbatim.
    Designate(u8),
    /// Buffering an OSC string (`ESC ] … BEL/ST`) so its payload is never
    /// charset-translated. Bytes still pass through to the display verbatim.
    Osc,
    OscEsc,
}

/// 一条高亮规则最终落到 span 上的颜色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuleColor {
    /// ANSI 槽位（预设规则用它，换主题时跟着主题走）
    Idx(u8),
    /// 绝对 RGB（自定义规则用用户选的色）
    Rgb(u8, u8, u8),
}

#[derive(Clone)]
pub(crate) struct CompiledOutputRule {
    pub(crate) matcher: regex::Regex,
    pub(crate) whole_line: bool,
    pub(crate) color: RuleColor,
    /// 只高亮第几个捕获组（0 = 整个匹配）；path 预设用前置组占位，取 2。
    pub(crate) group: usize,
    /// (#hl-bold-tier) 该规则命中是否加粗（日志级加粗、标识类细体）。
    pub(crate) bold: bool,
}

pub(crate) type TermBufferHandle = Arc<Mutex<TermBuffer>>;
pub(crate) type TermBuffers = Arc<Mutex<HashMap<String, TermBufferHandle>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderWaitResult {
    Settled,
    Closed,
    TimedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenderGatePhase {
    Idle,
    Scheduled,
    Flushing,
}

pub(super) struct RenderGateState {
    pub(super) requested: u64,
    pub(super) settled: u64,
    pub(super) phase: RenderGatePhase,
    pub(super) closed: bool,
    pub(super) last_visible_flush: std::time::Instant,
}

/// Coalesces and acknowledges UI snapshot flushes for one terminal tab.
pub(crate) struct TabRenderGate {
    pub(super) state: Mutex<RenderGateState>,
    pub(super) settled_cv: Condvar,
}

pub(crate) type RenderGates = Arc<Mutex<HashMap<String, Arc<TabRenderGate>>>>;

/// A coloured, cursor-annotated snapshot ready for the Slint terminal grid.
pub(crate) struct BuiltScreen {
    pub(crate) spans: Vec<TermSpan>,
    pub(crate) cursor_row: i32,
    pub(crate) cursor_col: i32,
    pub(crate) rows_used: i32,
    pub(crate) is_alt: bool,
    pub(crate) mouse_tracked: bool,
    pub(crate) scroll_max: i32,
    pub(crate) scroll_offset: i32,
}

/// One coloured run within a terminal line.
#[derive(Clone)]
pub(crate) struct HistSpan {
    pub(crate) text: String,
    pub(crate) fg: vt100::Color,
    pub(crate) bg: vt100::Color,
    pub(crate) bold: bool,
    pub(crate) inverse: bool,
    pub(crate) col: i32,
    pub(crate) cells: i32,
}

pub(crate) type Line = (String, Vec<HistSpan>, bool);
