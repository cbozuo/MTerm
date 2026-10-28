//! 历史命令面板的**数据侧**回归（纯函数，不建窗口）。
//!
//! 高保真稿：docs/design/history-panel-hifi-2026-10-27.html
//!
//! 这一组里最重要的是 `consecutive_runs_keep_the_last_one_first`：
//! 原型里踩过的真 bug —— 按 `last_used` 排序时，连续执行的多条命令时间戳
//! 相同（秒级并列），稳定排序退回存储序（最旧在前），于是**最早执行的那条
//! 反而排第一**。正确的实现是「存储序反转」，位置本身编码 recency，零并列。

use super::*;

fn entry(cmd: &str, uses: u32, last_used: i64, pinned: bool) -> HistoryEntry {
    HistoryEntry {
        cmd: cmd.to_string(),
        last_used,
        uses,
        pinned,
    }
}

fn cmds(rows: &[HistoryRowData]) -> Vec<String> {
    rows.iter().map(|r| r.cmd.clone()).collect()
}

#[test]
fn default_order_is_most_recent_first() {
    // 存储序 = 最旧在前；显示序必须反过来。
    let entries = vec![
        entry("oldest", 1, 100, false),
        entry("middle", 1, 200, false),
        entry("newest", 1, 300, false),
    ];
    let rows = history_rows(&entries, "", HIST_FILTER_ALL, HIST_SORT_RECENT, 400);
    assert_eq!(cmds(&rows), ["newest", "middle", "oldest"]);
}

#[test]
fn consecutive_runs_keep_the_last_one_first() {
    // 三条命令在同一秒执行 —— last_used 完全并列。
    // 正确顺序只看存储位置：最后 push 的 run-C 排第一。
    let entries = vec![
        entry("before", 1, 50, false),
        entry("run-A", 1, 500, false),
        entry("run-B", 1, 500, false),
        entry("run-C", 1, 500, false),
    ];
    let rows = history_rows(&entries, "", HIST_FILTER_ALL, HIST_SORT_RECENT, 500);
    assert_eq!(rows[0].cmd, "run-C", "最后执行的那条必须排第一");
    assert_eq!(rows[1].cmd, "run-B");
    assert_eq!(rows[2].cmd, "run-A");
    assert_eq!(rows[3].cmd, "before");
}

#[test]
fn freq_order_breaks_ties_by_recency() {
    let entries = vec![
        entry("rare-old", 1, 100, false),
        entry("often-older", 9, 110, false),
        entry("often-newer", 9, 120, false),
    ];
    let rows = history_rows(&entries, "", HIST_FILTER_ALL, HIST_SORT_FREQ, 200);
    assert_eq!(
        cmds(&rows),
        ["often-newer", "often-older", "rare-old"],
        "次数降序；同次数按存储位置（越靠后越近）"
    );
}

#[test]
fn filters_narrow_by_pin_and_today() {
    let now = 1_000_000;
    // 存储序必须「最旧在前」：fresh(now-20) 在 pinned(now-10) 之前。
    // 夹具自身要自洽，否则测出来的「顺序」是在测一个不存在的状态。
    let entries = vec![
        entry("fresh", 2, now - 20, false),
        entry("pinned", 1, now - 10, true),
        entry("stale", 3, now - 90_000, false),
    ];

    let rows = history_rows(&entries, "", HIST_FILTER_PINNED, HIST_SORT_RECENT, now);
    assert_eq!(cmds(&rows), ["pinned"]);

    let rows = history_rows(&entries, "", HIST_FILTER_TODAY, HIST_SORT_RECENT, now);
    assert_eq!(cmds(&rows), ["pinned", "fresh"], "stale 超过 24h 被排除");
}

#[test]
fn search_is_case_insensitive_substring() {
    let entries = vec![
        entry("Git Status", 1, 10, false),
        entry("ls -la", 1, 20, false),
    ];
    let rows = history_rows(&entries, "GIT", HIST_FILTER_ALL, HIST_SORT_RECENT, 30);
    assert_eq!(cmds(&rows), ["Git Status"]);
}

/// 命中高亮三段：pre + hit + post 拼回 = cmd，hit 保留**原文大小写**。
/// Slint 侧没有 substring，这三段就是行内高亮的唯一数据来源（高保真 §8）。
#[test]
fn search_splits_the_row_into_highlight_segments() {
    let entries = vec![entry("GIT status --short", 1, 10, false)];
    let rows = history_rows(&entries, "git", HIST_FILTER_ALL, HIST_SORT_RECENT, 30);
    assert_eq!(rows[0].pre, "");
    assert_eq!(rows[0].hit, "GIT", "命中段必须是原文大小写，不能拿小写串画");
    assert_eq!(rows[0].post, " status --short");

    // 无命中的行根本不会出现在结果里（过滤先行）；无搜索时三段全空，
    // UI 直接画 cmd 整串。
    let rows = history_rows(&entries, "zzz", HIST_FILTER_ALL, HIST_SORT_RECENT, 30);
    assert!(rows.is_empty(), "不匹配的行不该出现");
    let rows = history_rows(&entries, "", HIST_FILTER_ALL, HIST_SORT_RECENT, 30);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].hit.is_empty() && rows[0].pre.is_empty() && rows[0].post.is_empty());
}

#[test]
fn migrated_entries_show_unknown_time_not_a_fake_timestamp() {
    // 从旧 Vec<String> 迁移来的条目 last_used = 0：显示「较早」，
    // 绝不编一个假时间。元信息拆成「次数 / 时间」两段（高保真 §4），
    // 次数随排序档变换视觉权重 —— 所以两段必须各自独立成字段。
    let entries = vec![entry("legacy", 1, 0, false)];
    let rows = history_rows(&entries, "", HIST_FILTER_ALL, HIST_SORT_RECENT, 100);
    assert!(rows[0].time_label.contains("较早"), "time = {}", rows[0].time_label);
    assert!(rows[0].uses_label.contains('1'), "uses = {}", rows[0].uses_label);
    assert!(rows[0].time_label.starts_with(" · "), "分隔符要在 Rust 侧拼好");
}

/// Slint 结构体映射：字段漏一个不会报错，但界面会静默变空 —— 所以单独守。
#[test]
fn row_model_maps_fields_to_the_slint_struct() {
    use slint::Model;

    let entries = vec![
        entry("older", 1, 10, false),
        entry("newest", 3, 20, true),
    ];
    let model = history_row_model(&entries, "", HIST_FILTER_ALL, HIST_SORT_RECENT);
    assert_eq!(model.row_count(), 2);

    let first = model.row_data(0).expect("首行");
    assert_eq!(first.cmd, "newest", "显示序 = 存储序反转");
    assert!(first.pinned, "置顶标记要带过去");
    assert!(first.uses_label.contains('3'), "uses = {}", first.uses_label);
    // history_row_model 用真实当前时间（夹具时间戳是远古），只断结构不断字面：
    // 分隔符在 Rust 侧拼好、相对词非空即可。
    assert!(first.time_label.starts_with(" · "), "time = {}", first.time_label);
    assert_eq!(first.hit, "", "无搜索时 hit 必须为空");
}

