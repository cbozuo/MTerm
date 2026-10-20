//! 输出高亮（#hl-settings-hifi）：预设表 + claim 式命中引擎 + 彩虹括号。
//!
//! 旧 API（`highlight_plain_output(runs, preset, custom)` + log/devops 两档
//! 打包）已换成逐条可开关的预设表：
//! * `compile_highlight_rules(enabled, disabled_presets, custom)` 出规则表
//!   （自定义在前、预设按表序；总开关关 = 空表）；
//! * `highlight_plain_output(runs, rules, rainbow)` 只染**程序没上过色**的
//!   run，先命中先着色；
//! * 彩虹括号是独立开关（非正则），`rainbow` 参数单独传。
use super::*;
use crate::config::OutputHighlightRule;
use crate::terminal::{
    compile_highlight_rules, preset_enabled, rainbow_parens_enabled, HIGHLIGHT_PRESETS,
    RAINBOW_PRESET_ID,
};

fn plain_run(text: &str, col: i32) -> HistSpan {
    HistSpan {
        text: text.to_string(),
        fg: vt100::Color::Default,
        bg: vt100::Color::Default,
        bold: false,
        inverse: false,
        col,
        cells: text.chars().count() as i32,
    }
}

fn custom_rule(
    pattern: &str,
    regex: bool,
    case_sensitive: bool,
    whole_line: bool,
    color: &str,
) -> OutputHighlightRule {
    OutputHighlightRule {
        name: String::new(),
        pattern: pattern.to_string(),
        regex,
        case_sensitive,
        whole_line,
        color: color.to_string(),
        enabled: true,
    }
}

/// 全预设开 + 无自定义
fn all_presets() -> Vec<CompiledOutputRule> {
    compile_highlight_rules(true, &[], &[])
}

#[test]
fn log_levels_are_colored_case_insensitively_and_bolded() {
    // 大写 / 小写级别词都命中（旧引擎就认大写，新表不能退步）
    let runs = highlight_plain_output(
        vec![plain_run("request ERROR failed", 0)],
        &all_presets(),
        false,
    );
    let hits: Vec<&HistSpan> = runs
        .iter()
        .filter(|r| matches!(r.fg, vt100::Color::Idx(9)))
        .collect();
    assert_eq!(hits.len(), 2, "ERROR 与 failed 都该染 err 色");
    assert_eq!(hits[0].text, "ERROR");
    assert_eq!(hits[1].text, "failed");
    assert!(hits.iter().all(|r| r.bold));
    // 列号按字符数重算（全宽单字节）
    assert_eq!(hits[0].col, 8);
    assert_eq!(hits[0].cells, 5);
    assert_eq!(hits[1].col, 14);

    let runs = highlight_plain_output(
        vec![plain_run(r#"{"level":"warn","message":"disk nearly full"}"#, 4)],
        &all_presets(),
        false,
    );
    let level = runs
        .iter()
        .find(|run| run.text == "warn")
        .expect("结构化级别词应该被高亮");
    assert!(matches!(level.fg, vt100::Color::Idx(11)));

    for (word, idx) in [
        ("success", 10u8),
        ("debug", 8),
        ("information", 12),
        ("caution", 11),
    ] {
        let runs = highlight_plain_output(vec![plain_run(word, 0)], &all_presets(), false);
        let hit = runs
            .iter()
            .find(|r| r.text == word)
            .unwrap_or_else(|| panic!("{word} should be highlighted"));
        assert!(matches!(hit.fg, vt100::Color::Idx(i) if i == idx), "{word}");
    }
}

#[test]
fn structured_forms_keep_their_ansi_slots() {
    // 具体形态优先于宽泛数字：日期/IP/UUID/路径各自的槽位
    let cases: &[(&str, u8)] = &[
        ("http://example.com/a", 12),          // url
        ("192.168.0.1", 10),                   // ipv4
        ("fe80::1", 14),                       // ipv6
        ("aa:bb:cc:dd:ee:ff", 12),             // mac
        ("user@example.com", 13),              // email
        ("2026-07-14", 11),                    // iso 日期
        ("Jul 14 10:20:30", 11),               // syslog 日期（月份先行）
        ("550e8400-e29b-41d4-a716-446655440000", 13), // uuid
        ("12.5 GB", 12),                       // 大小
        ("/var/log/syslog", 13),               // 路径
        ("+-------+------+", 8),               // 表格分隔线
        ("port 8080 up", 14),                  // 数字垫底
    ];
    for (text, idx) in cases {
        let runs = highlight_plain_output(vec![plain_run(text, 0)], &all_presets(), false);
        let hit = runs
            .iter()
            .find(|r| matches!(r.fg, vt100::Color::Idx(_)))
            .unwrap_or_else(|| panic!("{text:?} should be highlighted"));
        assert!(
            matches!(hit.fg, vt100::Color::Idx(i) if i == *idx),
            "{text:?} -> {:?} (text {:?}), expected Idx({idx})",
            hit.fg,
            hit.text
        );
        assert!(hit.bold, "{text:?} should be bold");
    }
}

#[test]
fn master_switch_off_leaves_everything_untouched() {
    assert!(compile_highlight_rules(false, &[], &[]).is_empty());
    let run = plain_run("ERROR 192.168.0.1 /var/log/x", 0);
    let runs = highlight_plain_output(vec![run], &[], false);
    assert_eq!(runs.len(), 1);
    assert!(matches!(runs[0].fg, vt100::Color::Default));
    assert!(!runs[0].bold);
}

#[test]
fn disabled_preset_is_skipped() {
    let rules = compile_highlight_rules(true, &["err".into()], &[]);
    let runs = highlight_plain_output(vec![plain_run("request ERROR failed", 0)], &rules, false);
    assert_eq!(runs.len(), 1);
    assert!(matches!(runs[0].fg, vt100::Color::Default));
    // 其余预设照旧
    let runs = highlight_plain_output(vec![plain_run("192.168.0.1", 0)], &rules, false);
    assert!(matches!(runs[0].fg, vt100::Color::Idx(10)));
}

#[test]
fn custom_rule_wins_over_preset_and_is_case_insensitive() {
    let rules = compile_highlight_rules(true, &[], &[custom_rule("error", false, false, false, "green")]);
    let runs = highlight_plain_output(vec![plain_run("ERROR then error", 0)], &rules, false);
    let hits: Vec<&HistSpan> = runs
        .iter()
        .filter(|run| matches!(run.fg, vt100::Color::Idx(10)))
        .collect();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].text, "ERROR");
    assert_eq!(hits[1].text, "error");
    // err 预设（Idx 9）被自定义整体挤占，不再出现
    assert!(!runs.iter().any(|run| matches!(run.fg, vt100::Color::Idx(9))));
}

#[test]
fn custom_regex_whole_line_does_not_overwrite_ansi() {
    let rules =
        compile_highlight_rules(true, &[], &[custom_rule("timeout|denied", true, false, true, "magenta")]);
    let mut ansi = plain_run(" ANSI", 18);
    ansi.fg = vt100::Color::Idx(2);
    let runs = highlight_plain_output(vec![plain_run("request timeout   ", 0), ansi], &rules, false);
    assert!(matches!(runs[0].fg, vt100::Color::Idx(13)));
    assert!(runs[0].bold);
    // 程序自己上过色的 run 永远不碰
    assert!(matches!(runs[1].fg, vt100::Color::Idx(2)));
    assert!(!runs[1].bold);
}

#[test]
fn existing_ansi_styles_are_preserved() {
    let mut coloured = plain_run("ERROR", 0);
    coloured.fg = vt100::Color::Idx(2);
    let runs = highlight_plain_output(vec![coloured], &all_presets(), false);
    assert_eq!(runs.len(), 1);
    assert!(matches!(runs[0].fg, vt100::Color::Idx(2)));
    assert!(!runs[0].bold);
}

#[test]
fn custom_unicode_match_preserves_terminal_grid_columns() {
    let rules = compile_highlight_rules(true, &[], &[custom_rule("错误", false, true, false, "red")]);
    let text = "前缀错误 done";
    let mut run = plain_run(text, 0);
    run.cells = text_cell_width(text);
    let runs = highlight_plain_output(vec![run], &rules, false);
    let hit = runs
        .iter()
        .find(|run| run.text == "错误")
        .expect("CJK keyword should be highlighted");
    assert_eq!(hit.col, 4);
    assert_eq!(hit.cells, 4);
}

#[test]
fn path_preset_colors_only_the_path_itself() {
    let runs = highlight_plain_output(vec![plain_run("see /var/log/syslog now", 0)], &all_presets(), false);
    let hit = runs
        .iter()
        .find(|r| matches!(r.fg, vt100::Color::Idx(13)))
        .expect("path should be highlighted");
    assert_eq!(hit.text, "/var/log/syslog");
    assert_eq!(hit.col, 4);
    // 前置空白不在染色区间里（拆出的前段保持默认色）
    assert!(runs.iter().any(|r| r.text == "see " && matches!(r.fg, vt100::Color::Default)));
    // 没有斜杠的单词不算路径
    let runs = highlight_plain_output(vec![plain_run("syslog", 0)], &all_presets(), false);
    assert!(matches!(runs[0].fg, vt100::Color::Default));
}

#[test]
fn rainbow_parens_cycle_by_nesting_depth() {
    // f(g(x), h(y))：外层红 / 内层橙，闭括号与对应开括号同色
    let runs = highlight_plain_output(vec![plain_run("f(g(x), h(y))", 0)], &[], true);
    let parens: Vec<&HistSpan> = runs
        .iter()
        .filter(|r| matches!(r.text.as_str(), "(" | ")"))
        .collect();
    assert_eq!(parens.len(), 6);
    let red = vt100::Color::Rgb(0xF7, 0x54, 0x64);
    let orange = vt100::Color::Rgb(0xF0, 0x88, 0x3E);
    assert!(matches!(parens[0].fg, c if c == red));
    assert!(matches!(parens[1].fg, c if c == orange));
    assert!(matches!(parens[2].fg, c if c == orange));
    assert!(matches!(parens[3].fg, c if c == orange));
    assert!(matches!(parens[4].fg, c if c == orange));
    assert!(matches!(parens[5].fg, c if c == red));
    // 括号里的内容不染色
    assert!(runs.iter().any(|r| r.text == "x" && matches!(r.fg, vt100::Color::Default)));
}

#[test]
fn rainbow_switch_and_disable_flag() {
    assert!(rainbow_parens_enabled(true, &[]));
    assert!(!rainbow_parens_enabled(true, &[RAINBOW_PRESET_ID.to_string()]));
    assert!(!rainbow_parens_enabled(false, &[]));
    assert!(preset_enabled(true, &[], "err"));
    assert!(!preset_enabled(true, &["err".into()], "err"));
    assert!(!preset_enabled(false, &[], "err"));
}

#[test]
fn alternate_screen_does_not_add_highlight_colors() {
    // 高亮在 TermBuffer::render 里做，alt 屏整段跳过（看 term_buffer.rs）；
    // 这里守住 build_row 本身不染色 + 引擎只碰无色 run 的契约。
    let mut parser = vt100::Parser::new(3, 30, 0);
    parser.process(b"\x1b[?1049hERROR");
    assert!(parser.screen().alternate_screen());
    let (_plain, runs, _wrapped) = build_row(parser.screen(), 0, 30);
    let level = runs
        .iter()
        .find(|run| run.text.contains("ERROR"))
        .expect("alternate-screen text should still render");
    assert!(matches!(level.fg, vt100::Color::Default));
    assert!(!level.bold);
    // 显式喂给引擎也不会染：alt 屏的文本由 render 分支根本不进引擎
    let runs = highlight_plain_output(runs, &[], false);
    assert!(matches!(runs[0].fg, vt100::Color::Default));
}

#[test]
fn invalid_regex_is_rejected_before_persistence() {
    assert!(super::terminal_ui::validate_highlight_rule("([").is_err());
    assert!(super::terminal_ui::validate_highlight_rule("").is_err());
    assert!(super::terminal_ui::validate_highlight_rule("literal").is_ok());
    assert!(super::terminal_ui::validate_highlight_rule("timeout|denied").is_ok());
}
