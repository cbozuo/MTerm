//! 终端行缓冲的**状态类型**：一次按键之后「用户正在敲的那一行」的判定结果。
//!
//! 判定逻辑在 `src/terminal/impls/terminal_hint.rs`；这里只放状态与快照。

/// 行缓冲的状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineState {
    /// 有可用的命令行文本（`line` 字段有效）。
    Ready,
    /// 提示符后面什么都没有 —— 不弹框（高保真 §4：空输入不端出候选）。
    Empty,
    /// 判定不可靠（密码输入 / 全屏程序 / 无法定位命令起点）—— 不弹框。
    Muted,
}

/// 一次按键之后，终端里「用户正在敲的那一行」是什么。
#[derive(Debug, Clone)]
pub(crate) struct LineSnapshot {
    pub(crate) state: LineState,
    /// 命令起点到光标之间的文本。`Muted` 时为空。
    pub(crate) line: String,
    /// ★ 光标是否停在行尾（§5 鬼影的显示前提）：行内光标右侧没有可见字符。
    ///   `Muted` 恒 false（无行可言）。
    pub(crate) cursor_at_eol: bool,
}

impl LineSnapshot {
    pub(crate) fn muted() -> Self {
        Self { state: LineState::Muted, line: String::new(), cursor_at_eol: false }
    }
    pub(crate) fn empty() -> Self {
        Self { state: LineState::Empty, line: String::new(), cursor_at_eol: true }
    }
}
