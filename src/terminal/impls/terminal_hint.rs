//! (#cmd-hint-p1 2026-10-27) 终端内联提示框的**行缓冲**。
//!
//! 高保真稿：`docs/design/terminal-command-hint-hifi-2026-10-27.html` §7 P1。
//!
//! ## 为什么要单独一个模块
//!
//! P1 要回答的问题只有一个：**「用户现在在提示符后面敲了什么」**。
//! MTerm 原来完全没有这个信息 —— 按键在 `send-key` 里逐字节发往 PTY，
//! `ime-input` 用完即清（`ui/terminal_view.slint`），也没有
//! OSC 133 这种行边界标记（只有命令**跑完**才回来的 OSC 697）。
//!
//! ## 做法：不信按键，信回显
//!
//! 最容易想到也最容易错的做法是「自己记按键序列」（按键记账）。它会在
//! 这些地方崩：远端 tab 补全会改写整行、readline 的 Ctrl+W/Ctrl+U 删词、
//! ↑ 调历史、行内移动光标……任何一处都会让记账与实际显示永久错位，
//! 而**错位不会自愈**。
//!
//! 所以这里反过来：**以 vt100 网格的回显为准**。每次按键之后，取光标所在行
//! 的文本，从光标列往前找一个「命令起点」，把 `起点..光标` 当作当前命令行。
//! 这天然包含了远端做的一切改写（补全、删词、历史回溯），不需要理解它们。
//!
//! 「命令起点」是本模块唯一需要猜的东西，三条保守规则：
//!   1. 行里最后一个提示符终止符（`$` `#` `%` `>` `❯` `➜`）之后；
//!   2. 找不到终止符时，退回到「行首连续空白之后」；
//!   3. 两者都给不出**确定的**起点 → 返回 `None`，调用方**不弹提示框**。
//!
//! 第三条是刻意设计的：P1 的定位就是「识别不出来就别弹，绝不猜」
//! （高保真 §4 抑制表最后一行）。宁可少弹，也不要弹一个内容错位的框。
//!
//! ## 密码/非回显输入
//!
//! `observe()` 会比对「按键后网格是否变了」：如果用户敲了可打印字符但
//! 光标行文本没变、光标列也没动，说明远端**没回显**（`sudo` / `ssh` /
//! `passwd` 的密码提示就是这种）。此时立刻把状态标成 `Muted`，并让调用方
//! 清空提示框 —— 绝不在密码旁边显示历史命令。

use super::line::{LineSnapshot, LineState};

/// 提示符终止符。收得比较全但不含中文全角符号（那些在 CJK 提示符里位置不定）。
const PROMPT_TERMINATORS: &[char] = &['$', '#', '%', '>', '❯', '➜', 'λ', '→'];

/// 看上去像「要你输密码」的词。
///
/// ★ 这是**首选**判定，不能只靠「按键没回显」：
///   `Password: ` 这种提示符里**没有** `$`/`#` 这类终止符，按规则 2 会被
///   整行当成命令行，于是第一下按键就会弹出历史命令 —— 而它就贴在密码框旁边。
///   靠「连续两次无回显」兜底太晚（第一次已经弹出来了）。
///   所以先看文本：命中即 `Muted`，一个字都不弹。
const PASSWORD_WORDS: &[&str] = &[
    "password",
    "passphrase",
    "passwd",
    "密码",
    "口令",
    "verify",
];

/// 整行（或整段可见输出）是否像密码提示。
fn looks_like_password_prompt(row_text: &str) -> bool {
    let lower = row_text.to_lowercase();
    PASSWORD_WORDS.iter().any(|w| lower.contains(w))
}

/// 从一整行网格文本里，截出「光标所在命令行」。
///
/// `row_text` 是光标所在行的完整文本（右侧补空格已由调用方 `trim_end`），
/// `cursor_col` 是光标在该行的**字符**列（不是显示格，CJK 已在调用方换算）。
///
/// 返回 `None` 表示定位不出命令起点 —— 调用方应视为 `Muted`。
pub(crate) fn command_line(row_text: &str, cursor_col: usize) -> Option<String> {
    // 光标之后的内容不参与（readline 允许光标停在行中间）。
    let chars: Vec<char> = row_text.chars().collect();
    if cursor_col > chars.len() {
        return None;
    }
    let before: String = chars[..cursor_col].iter().collect();

    // 规则 1：最后一个提示符终止符之后。要求终止符**后面**跟空格或直接接内容，
    // 避免把 `echo $PATH` 里的 `$` 当成提示符（那样会丢掉命令名）。
    // 判据：终止符是「它前面没有非空白字符」，或者「它紧跟着一个空格」。
    let mut start: Option<usize> = None;
    for (i, ch) in before.chars().enumerate() {
        if !PROMPT_TERMINATORS.contains(&ch) {
            continue;
        }
        let prefix: String = before.chars().take(i).collect();
        let next_is_space = before.chars().nth(i + 1).map(|c| c == ' ').unwrap_or(true);
        // `echo $PATH`：`$` 前面有非空白 → 只有当它后面紧跟空格才算提示符，
        // 而 `$PATH` 后面不是空格，于是被排除。
        if prefix.trim().is_empty() || next_is_space {
            start = Some(i + 1);
        }
    }
    if let Some(s) = start {
        let cmd: String = before.chars().skip(s).collect();
        let cmd = cmd.trim_start().to_string();
        return Some(cmd);
    }

    // 规则 2：没有终止符。只有当整行形如「提示符样子的前缀 + 空格 + 命令」时
    // 才敢用 —— 也就是**行首那段不含命令的样子**我们无法判断，所以退一步：
    // 若整行没有终止符，就把整行当命令行（很多自定义 PS1 没有 `$`）。
    // 但如果行首有 `[`、`(`、`{` 这类明显的提示符包裹，仍然保守返回 None。
    let trimmed = before.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('(') || trimmed.starts_with('{') {
        // 形如 `[root@host ~]# ls` 这种已经由规则 1 处理；
        // 走到这里说明既没终止符又以包裹符开头 —— 无法确定边界。
        return None;
    }
    Some(trimmed.to_string())
}

/// 观测器：每个 tab 一个，随按键与输出更新。
///
/// 它**不做按键记账**（理由见模块注释），只在每次按键后读一次网格。
#[derive(Debug, Default)]
pub(crate) struct HintObserver {
    /// 上一次观测到的光标行文本，用于判「远端有没有回显」。
    last_row: String,
    /// 上一次观测到的光标列。
    last_col: u16,
    /// 连续多少次可打印按键没有引起任何网格变化。
    /// 达到阈值即认定「非回显输入」（密码），置 `Muted`。
    silent_keystrokes: u8,
    /// 当前是否处于 `Muted`（密码输入 / 全屏程序）。
    muted: bool,
}

/// 连续这么多次可打印按键都没有回显 → 认作密码类输入。
const SILENT_KEY_LIMIT: u8 = 2;

impl HintObserver {
    /// 每次按键**之后**调用一次。
    ///
    /// * `row_text` —— 光标所在行的完整网格文本（调用方负责 `trim_end`）
    /// * `cursor_col` —— 光标列
    /// * `printable_keystroke` —— 这次按键是否是可打印字符（用于密码判定）
    /// * `alt_screen` —— 是否在全屏程序里（vim/htop：直接 `Muted`）
    pub(crate) fn observe(
        &mut self,
        row_text: &str,
        cursor_col: u16,
        printable_keystroke: bool,
        alt_screen: bool,
    ) -> LineSnapshot {
        // 全屏程序里没有「shell 行」这回事。
        if alt_screen {
            self.muted = true;
            self.last_row.clear();
            self.last_col = cursor_col;
            return LineSnapshot::muted();
        }

        let changed = row_text != self.last_row || cursor_col != self.last_col;
        self.last_row = row_text.to_string();
        self.last_col = cursor_col;

        // ★ 先按**文本**判密码提示（首选判据）：
        //   `Password: ` 里没有 `$`/`#`，按规则 2 会被整行当成命令行，
        //   第一下按键就会弹出历史命令 —— 而它就贴在密码框旁边。
        if looks_like_password_prompt(row_text) {
            self.muted = true;
            return LineSnapshot::muted();
        }

        // 兜底（只在未静默时计数）：可打印按键但网格毫无变化 → 远端没回显
        // （密码的另一种形态）。
        if !self.muted {
            if printable_keystroke && !changed {
                self.silent_keystrokes = self.silent_keystrokes.saturating_add(1);
                if self.silent_keystrokes >= SILENT_KEY_LIMIT {
                    self.muted = true;
                }
            } else if changed {
                self.silent_keystrokes = 0;
            }
        }

        // ★ 静默解除（§4 修订 2026-10-30）：**有回显（changed）就恢复观测**。
        //   密码的特征是「持续无回显」；而快速连打时每次按键的**首读**都落在
        //   回显之前 —— 原实现只在 on_submit（回车）解除，实测连打 `git co`
        //   六个键就有 2 次首读无变化 → 误判密码 → 提示框永久哑掉。
        //   120ms 重读（schedule_hint_reread）时回显必然已到 → changed=true
        //   → 这里自愈。alt_screen 在函数开头已单独拦截。
        //   ⚠ 解除判定用的 changed 就是**开头**算的那个 —— 不能在更新
        //     last_row 之后重算（那样恒为 false，解除永远不触发，实测踩过）。
        if self.muted && changed {
            self.muted = false;
            self.silent_keystrokes = 0;
        }
        if self.muted {
            return LineSnapshot::muted();
        }

        let Some(line) = command_line(row_text, cursor_col as usize) else {
            return LineSnapshot::muted();
        };
        if line.trim().is_empty() {
            return LineSnapshot::empty();
        }
        // 行尾判定（§5 鬼影显示前提）：光标右侧（trim 后）无可见字符。
        // row_text 已被调用方 trim_end，光标列之后还有字符 = 光标在行中。
        let cursor_at_eol = (cursor_col as usize) >= row_text.chars().count();
        LineSnapshot { state: LineState::Ready, line, cursor_at_eol }
    }

    /// 用户按了回车（命令被提交）→ 行结束，密码静默状态解除。
    pub(crate) fn on_submit(&mut self) {
        self.last_row.clear();
        self.last_col = 0;
        self.silent_keystrokes = 0;
        self.muted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_classic_bash_prompt_yields_the_command() {
        // `root@web-01:/var/log$ ls -l` 且光标在行尾
        let row = "root@web-01:/var/log$ ls -l";
        assert_eq!(command_line(row, row.chars().count()).as_deref(), Some("ls -l"));
    }

    #[test]
    fn an_empty_prompt_yields_an_empty_command() {
        let row = "root@web-01:/var/log$ ";
        assert_eq!(command_line(row, row.chars().count()).as_deref(), Some(""));
    }

    #[test]
    fn a_hash_prompt_for_root_is_handled() {
        let row = "[root@web-01 ~]# systemctl status nginx";
        assert_eq!(
            command_line(row, row.chars().count()).as_deref(),
            Some("systemctl status nginx")
        );
    }

    #[test]
    fn a_dollar_inside_the_command_is_not_mistaken_for_a_prompt() {
        // ★ 这条是最容易错的：`echo $PATH` 里的 `$` 后面不是空格，
        //   不能被当成提示符（否则命令行会被截成 `PATH`）。
        let row = "user@host:~$ echo $PATH";
        assert_eq!(
            command_line(row, row.chars().count()).as_deref(),
            Some("echo $PATH")
        );
    }

    #[test]
    fn the_cursor_in_the_middle_only_looks_at_text_before_it() {
        // `git commit` 里光标停在 `co` 之后 → 命令是 `git co`
        let row = "u@h:~$ git commit";
        let col = "u@h:~$ git co".chars().count();
        assert_eq!(command_line(row, col).as_deref(), Some("git co"));
    }

    #[test]
    fn a_fancy_prompt_without_a_terminator_still_works() {
        let row = "my-prompt ls -l";
        // 没有终止符、也不是包裹符开头 → 整行当命令行（保守但可用）。
        assert_eq!(command_line(row, row.chars().count()).as_deref(), Some("my-prompt ls -l"));
    }

    #[test]
    fn a_prompt_opening_with_a_bracket_and_no_terminator_is_refused() {
        // 没有任何终止符、又以包裹符开头 → 定位不出边界，必须拒绝（不猜）。
        let row = "[2026-10-27 12:00] ls -l";
        assert_eq!(command_line(row, row.chars().count()), None);
    }

    #[test]
    fn alt_screen_is_always_muted() {
        let mut o = HintObserver::default();
        let s = o.observe("anything", 5, true, true);
        assert_eq!(s.state, LineState::Muted);
    }

    #[test]
    fn a_password_prompt_is_muted_on_the_very_first_observation() {
        // ★ 首选判据是**文本**，不是「按键没回显」：
        //   `Password: ` 没有 `$`/`#`，按规则 2 会被整行当命令行，
        //   第一下按键就会弹出历史命令 —— 而那正是最不该出现的时刻。
        let mut o = HintObserver::default();
        let row = "Password: ";
        let col = row.chars().count() as u16;
        let s = o.observe(row, col, false, false);
        assert_eq!(s.state, LineState::Muted, "密码提示必须第一帧就静音");
    }

    #[test]
    fn a_silent_keystroke_falls_back_to_muting() {
        // 文本里没有 password 字样，但敲了字符网格毫无变化 →
        // 两次之后兜底判 Muted（远端确实没回显）。
        let mut o = HintObserver::default();
        let row = "u@h:~$ ";
        let col = row.chars().count() as u16;
        o.observe(row, col, false, false);
        let s1 = o.observe(row, col, true, false);
        assert_eq!(s1.state, LineState::Empty, "第一次静默还不足以判定");
        let s2 = o.observe(row, col, true, false);
        assert_eq!(s2.state, LineState::Muted, "连续两次无回显必须判 Muted");
    }

    #[test]
    fn an_echoed_keystroke_keeps_the_buffer_alive() {
        let mut o = HintObserver::default();
        let prompt = "u@h:~$ ";
        o.observe(prompt, prompt.chars().count() as u16, false, false);
        // 敲了 `l`：网格里真的多了一个字符，光标右移一格。
        let after = "u@h:~$ l";
        let s = o.observe(after, after.chars().count() as u16, true, false);
        assert_eq!(s.state, LineState::Ready);
        assert_eq!(s.line, "l");
    }

    #[test]
    fn submitting_a_command_clears_the_muted_state() {
        let mut o = HintObserver::default();
        let row = "Password: ";
        let col = row.chars().count() as u16;
        assert_eq!(o.observe(row, col, false, false).state, LineState::Muted);
        // 回车之后回到可用状态（下一次 observe 会重新判定）。
        o.on_submit();
        let after = "u@h:~$ ls";
        let s = o.observe(after, after.chars().count() as u16, false, false);
        assert_eq!(s.state, LineState::Ready);
    }
}
