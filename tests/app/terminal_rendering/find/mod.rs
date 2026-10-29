//! (#find-bar 2026-10-27 v3.6) 查找栏数据侧测试。
//! 验收基准：高保真 `docs/design/find-bar-hifi-2026-10-23.html` v3.6
//! §03 规则汇总 + §06 边界口径。

use super::*;
use crate::terminal::{FindOptions, FindState};

fn make_buf(rows: u16, cols: u16, history: &[&str], live: &[&str], view_offset: usize) -> TermBuffer {
    let mut parser = vt100::Parser::new(rows, cols, 0);
    parser.process(live.join("\r\n").as_bytes());
    TermBuffer {
        parser,
        find: FindState::default(),
        is_dark: false,
        palette: crate::theme::palette_or_default("", false),
        highlight_rules: Vec::new(),
        rainbow_parens: false,
        json_format_output: false,
        vt100_drawing: true,
        charset: crate::terminal::CharsetTracker::default(),
        interactive_echo_until: std::time::Instant::now(),
        last_output_at: std::time::Instant::now(),
        sel_anchor: None,
        sel_focus: None,
        sel_ranges: Vec::new(),
        mouse_tracked: false,
        history: history.iter().map(|s| (s.to_string(), Vec::new(), false)).collect(),
        prev: Vec::new(),
        view_offset,
        scroll_accum: 0.0,
        displayed_text: Vec::new(),
        csi_state: CsiState::Normal,
        csi_pending: Vec::new(),
        raw: std::collections::VecDeque::new(),
    }
}

const IGNORE_CASE: FindOptions = FindOptions { match_case: false, use_regex: false };

#[test]
fn find_scan_covers_history_and_live_with_grid_columns_past_cjk() {
    // 高亮列必须是网格列（CJK 占两格），不是 char 下标（#132 同口径）。
    let buf = make_buf(3, 40, &["old java line"], &["提示: java", "no match here"], 0);
    let hits = buf.find_scan_all("java", IGNORE_CASE);
    assert_eq!(hits.total(), 2, "history + live 各一条");
    // history 行
    assert_eq!(hits.hits[0], (0, 4, 4));
    // live 行：`提示: java` → 「提示」4 格 + ": " 2 格 = 网格列 6
    assert_eq!(hits.hits[1], (1, 6, 4));
}

#[test]
fn find_scan_cjk_query_spans_two_cells_per_glyph() {
    let buf = make_buf(3, 40, &[], &["提示: java"], 0);
    let hits = buf.find_scan_all("提示", IGNORE_CASE);
    assert_eq!(hits.total(), 1);
    assert_eq!(hits.hits[0], (0, 0, 4), "两个宽字符 = 4 格");
}

#[test]
fn find_match_case_toggle_respects_uppercase() {
    let buf = make_buf(3, 40, &["Java java JAVA"], &[], 0);
    let hits = buf.find_scan_all("java", IGNORE_CASE);
    assert_eq!(hits.total(), 3, "忽略大小写 → 全中");
    let hits = buf.find_scan_all("java", FindOptions { match_case: true, use_regex: false });
    assert_eq!(hits.total(), 1, "Aa 开 → 只有小写 java");
    assert_eq!(hits.hits[0], (0, 5, 4));
}

#[test]
fn find_regex_mode_and_invalid_keeps_old_hits() {
    let mut buf = make_buf(3, 40, &["error=1 warn=2"], &[], 0);
    let ok = buf.find_set_query(r"\w+=\d", IGNORE_CASE.with_regex());
    assert!(ok);
    // 视口（live 屏）= abs 1..4，两条命中都在 history（abs 0）→
    // 「视口之前最后一个」= 第 2 条（离视口最近的上方命中，v3.6 §03①）。
    assert_eq!(buf.find_counts(), (2, Some(2)), "最近命中口径：靠视口最近的第 2 条");

    // 正则写错（悬空括号）：find_set_query 返回 false 且**保旧结果**（§06⑤）。
    let ok = buf.find_set_query("error=(", IGNORE_CASE.with_regex());
    assert!(!ok, "无效正则应被拒绝");
    assert!(buf.find_invalid_regex());
    assert_eq!(buf.find_counts(), (2, Some(2)), "旧命中与旧指针原样保留");
}

#[test]
fn find_start_anchor_is_nearest_to_viewport_not_buffer_top() {
    // v3.6 §03① 勘误的核心：起始当前匹配 = 离视口最近，不是全缓冲第一个。
    // 3 行视口，视口 offset=1（顶行 = history[1]）；命中在 history[0] 和
    // history[3]（视口之后）。最近者 = history[3]。
    let buf = make_buf(
        3,
        40,
        &["java top", "filler", "filler", "java bottom"],
        &["live a", "live b", "live c"],
        1,
    );
    let mut buf = buf;
    let ok = buf.find_set_query("java", IGNORE_CASE);
    assert!(ok);
    let (_, cur) = buf.find_counts();
    // 视口 = abs 2..5（history 4 行 + live 3 行，offset=1 → 顶=combined_len-rows-offset=7-3-1=3）。
    // 视口内 3..7 无命中？history[3] = abs 3 在视口内 → 命中它。
    assert_eq!(cur, Some(2), "离视口最近的命中（history[3]，视口内）");
}

#[test]
fn find_step_wraps_around() {
    let mut buf = make_buf(3, 40, &["aa 1", "aa 2", "aa 3"], &[], 0);
    let ok = buf.find_set_query("aa", IGNORE_CASE);
    assert!(ok);
    // 3 条命中。live 视图视口 = abs 3..6，命中全在 history（abs 0..3）
    // →「视口之前最后一个」= 第 3 条（v3.6 §03① 最近命中口径）。
    assert_eq!(buf.find_counts().1, Some(3));
    assert_eq!(buf.find_step(1), Some(0)); // 卷回第 1 条（§04 循环查找）
    assert_eq!(buf.find_step(1), Some(1));
    assert_eq!(buf.find_step(1), Some(2));
    assert_eq!(buf.find_step(-1), Some(1)); // 反向
    assert_eq!(buf.find_step(-1), Some(0));
    assert_eq!(buf.find_step(-1), Some(2)); // 反向也卷
}

#[test]
fn find_center_scroll_target_aims_mid_viewport() {
    // combined = 4 history + 3 live = 7 行，视口 3 行。
    // 命中 history[0]（abs 0）→ 目标顶 = 0 − 3/2 = 0（saturating）。
    let mut buf = make_buf(3, 40, &["java x", "f", "f", "f"], &["l1", "l2", "l3"], 0);
    let ok = buf.find_set_query("java", IGNORE_CASE);
    assert!(ok);
    // 视口在 live 底（offset 0）→ 顶 = 7−3 = 4；最近命中 = history[0]（abs 0，
    // 视口外上方）→ 视口之前最后一个 = 它。
    let target = buf.find_center_scroll_target();
    assert_eq!(target, Some(7 - 3 - 0), "目标顶=abs0 → view_offset=4");
}

#[test]
fn find_clear_buffer_resets_state() {
    let mut buf = make_buf(3, 40, &["java"], &[], 0);
    let ok = buf.find_set_query("java", IGNORE_CASE);
    assert!(ok);
    assert_eq!(buf.find_counts().0, 1);
    buf.release_scrollback();
    assert_eq!(buf.find_counts().0, 0, "释放回滚区后查找状态一并清空");
}

/// Options 字面量辅助：链式切 regex。
trait FindOptsExt {
    fn with_regex(self) -> Self;
}
impl FindOptsExt for FindOptions {
    fn with_regex(mut self) -> Self {
        self.use_regex = true;
        self
    }
}

use crate::terminal::TermBuffer;
use crate::terminal::CsiState;

#[test]
fn debug_bad_regex_compile() {
    let e = regex::Regex::new("error=(").is_err();
    assert!(e, "悬空捕获组应编译失败");
}
