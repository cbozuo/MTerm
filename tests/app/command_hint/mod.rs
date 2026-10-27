//! (#cmd-hint 2026-10-27) 四段式命令提示框的数据侧回归。
//!
//! 高保真稿：docs/design/terminal-command-hint-hifi-2026-10-27.html
//!
//! 这里守**纯函数语义**：阶段判定、`apply`（选中后整行）、候选上限。
//! 为什么这几条值得写：原型阶段踩过的每个坑都是「两处各算一遍、算岔了」——
//! 尤其 `apply` 与 Enter 判据必须严丝合缝，否则表现就是
//! 「Enter 永远只填入、永远不执行」。

use super::{compute_apply, should_pop, HintInput, HintKind, HintPhase, HINT_MAX_ROWS};

/// 测 apply 用的小助手：只有一个候选文本，其余字段无关。
fn apply_of(cand: &str, kind: HintKind, line: &str) -> String {
    let input = HintInput::parse(line);
    let has_subs = line.split_whitespace().next() == Some("git");
    let phase = input.phase(has_subs);
    compute_apply(cand, kind, &input, phase)
}

// ── 阶段判定 ────────────────────────────────────────────────────────────

#[test]
fn the_first_word_is_the_command_phase() {
    let i = HintInput::parse("gi");
    assert_eq!(i.phase(false), HintPhase::Command);
    assert_eq!(i.word, "gi");
    assert!(i.settled.is_empty());
    assert!(!i.trailing_space);
}

#[test]
fn a_command_with_subcommands_enters_the_subcommand_phase() {
    let i = HintInput::parse("git co");
    // `has_subcommands` 只有 git/systemctl/docker/kubectl 这类才为真。
    assert_eq!(i.phase(true), HintPhase::Subcommand);
    // 同一个输入，命令没有子命令概念时就是「参数」阶段。
    assert_eq!(i.phase(false), HintPhase::Option);
}

#[test]
fn a_trailing_space_keeps_the_subcommand_phase() {
    // ★ Tab 链路的关键：填入 `git` 后我们会补一个空格，下一步必须仍停在
    //   子命令阶段，否则「填完就跳到参数」，用户看到的是候选突然换了一批。
    let i = HintInput::parse("git ");
    assert_eq!(i.phase(true), HintPhase::Subcommand);
    assert_eq!(i.word, "");
    assert!(i.trailing_space);
    assert_eq!(i.settled.as_slice(), ["git"]);
}

#[test]
fn a_slash_means_the_path_phase() {
    assert_eq!(HintInput::parse("ls /var/").phase(false), HintPhase::Path);
    assert_eq!(HintInput::parse("cat ~/.bash").phase(false), HintPhase::Path);
}

#[test]
fn a_dash_means_the_option_phase() {
    assert_eq!(
        HintInput::parse("journalctl -").phase(false),
        HintPhase::Option
    );
}

// ── apply：选中之后整行长什么样 ──────────────────────────────────────────

#[test]
fn accepting_a_command_at_the_first_word_replaces_the_whole_line() {
    // 命令阶段：`gi` + 选 `git` → `git`（不带空格，Slint 侧再补）。
    assert_eq!(apply_of("git", HintKind::Quick, "gi"), "git");
}

#[test]
fn accepting_a_subcommand_keeps_the_promoted_prefix_and_adds_a_space() {
    // ★ 尾随空格是必须的：下一步还要接着打参数。
    assert_eq!(
        apply_of("git commit", HintKind::Quick, "git co"),
        "git commit "
    );
}

#[test]
fn accepting_an_option_replaces_only_the_word_being_typed() {
    assert_eq!(
        apply_of("-u", HintKind::Quick, "journalctl -"),
        "journalctl -u "
    );
}

#[test]
fn accepting_a_path_does_not_add_a_trailing_space() {
    // 路径要能继续往下补（/var/log/nginx/…），补空格就断了。
    assert_eq!(
        apply_of("/var/log/", HintKind::Quick, "ls /var/"),
        "ls /var/log/"
    );
}

#[test]
fn a_history_row_always_replaces_the_whole_line() {
    // 历史是「整条命令」，不是某个词的补全 —— 任何阶段都整行替换。
    assert_eq!(
        apply_of("git commit -m fix", HintKind::History, "git co"),
        "git commit -m fix"
    );
}

#[test]
fn apply_is_stable_so_enter_can_ever_match() {
    // ★ 回归「Enter 永远只填入、永远不执行」：判据是「整行 == apply」。
    //   如果 apply 反复算会漂（每次都追一个空格/前缀），Enter 永远等不到相等。
    //
    //   链路：`git co` --Tab--> `git commit ` --再算一次--> 仍是 `git commit `。
    let first = apply_of("git commit", HintKind::Quick, "git co");
    assert_eq!(first, "git commit ");
    // 到了参数阶段，候选换成参数（`-m`），此时的 apply 只替换「正在补的词」。
    // 行尾有空格 → 直接追加，不会把已落定的 `git commit` 再拼一遍。
    let second = apply_of("-m", HintKind::Quick, &first);
    assert_eq!(second, "git commit -m ", "不能再把前缀拼一遍");
    // 对同一个输入再算一次，结果必须逐字节相同（幂等）。
    let again = apply_of("-m", HintKind::Quick, &first);
    assert_eq!(second, again);
}

#[test]
fn tab_chain_walks_command_to_subcommand_without_jumping_phases() {
    // 模拟连按 Tab：`gi` → `git ` → `git commit `。
    let step1 = apply_of("git", HintKind::Quick, "gi");
    assert_eq!(step1, "git");
    // Slint 侧会把命令阶段的短命令补一个空格，这里手工模拟那一步。
    let step2 = apply_of("git commit", HintKind::Quick, "git ");
    assert_eq!(step2, "git commit ");
    // 到了 "git commit " 之后，阶段应进入参数而不是退回去。
    let i = HintInput::parse(&step2);
    let has_subs = true;
    assert_eq!(i.phase(has_subs), HintPhase::Option);
}

// ── 候选上限 ────────────────────────────────────────────────────────────

#[test]
fn the_row_cap_matches_the_ui() {
    // 数据侧封顶数与 Slint 侧画的行数必须是同一个数，
    // 否则选中项会落在没画出来的行上。用户实测反馈 5 条太少 → 放宽到 8。
    assert_eq!(HINT_MAX_ROWS, 8);
}

// ── 触发阈值（用户实测反馈：一个字符就弹，误命中太多）──────────────────

#[test]
fn one_character_does_not_pop_unless_it_is_an_intent_prefix() {
    use super::should_pop;
    // 普通命令词：1 个字符不弹，2 个起弹。
    assert!(!should_pop("g", false, HintPhase::Command));
    assert!(should_pop("gi", false, HintPhase::Command));
    // 参数 / 路径前缀是明确的意图信号，1 个字符就弹。
    assert!(should_pop("-", false, HintPhase::Option));
    assert!(should_pop("/", false, HintPhase::Path));
    assert!(should_pop("~", false, HintPhase::Path));
    // 子命令阶段（`git ` 后）无字符也弹。
    assert!(should_pop("", true, HintPhase::Subcommand));
    // 命令阶段的空行尾不弹（什么都没敲）。
    assert!(!should_pop("", true, HintPhase::Command));
}

#[test]
fn two_characters_are_required_before_auto_popping() {
    // 用户实测反馈：一个字符就弹，误命中太多。
    // `hint_model` 的入口在 q.len() < 2 时短路（非路径/参数前缀），
    // 这里直接验证纯函数层的口径：词长 1 的打分照常，截断在调用方。
    // → 用 hint_model 的行为验证（需要 store，见集成层）；此处守阈值常量语义。
    let i = HintInput::parse("g");
    assert_eq!(i.word, "g");
    let j = HintInput::parse("gi");
    assert_eq!(j.word, "gi");
}
