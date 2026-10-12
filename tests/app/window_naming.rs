//! (#window-title-fix2 2026-10-12) 窗口显示名 / 托盘 tooltip 编号规则的回归测试。
//!
//! 用户定稿（修订）：**多开时全部窗口按位次编号**「MTerm · 窗口 N」（含最早
//! 打开的 → 窗口 1）；**只剩一个窗口时退回素「MTerm」**。编号取"存活窗口中
//! 按 registry id 升序的位次"，因此关掉靠前的窗口后，其余窗口位次前移、
//! 标题跟着刷新。
//!
//! 标题与托盘 tooltip 共用同一套纯函数（同一个 rank + 总数必须给出同一个串）。
use super::{ranks_of, window_title_for};

/// 托盘 tooltip 与状态栏标题同源：同一个 rank 必须给出同一个串。
/// 这是用户反馈的「托盘悬浮提示没有和状态栏中的名称同步」的判据。
fn titles_for(live_ids: &[u64]) -> Vec<(u64, String)> {
    let ranks = ranks_of(live_ids.iter().copied());
    let total = live_ids.len();
    live_ids
        .iter()
        .map(|id| (*id, window_title_for(ranks[id], total)))
        .collect()
}

#[test]
fn single_window_is_plain_mterm() {
    assert_eq!(window_title_for(1, 1), "MTerm");
    assert_eq!(titles_for(&[1])[0].1, "MTerm");
}

#[test]
fn all_windows_are_numbered_when_multiple_are_open() {
    let live = [1u64, 2, 3];
    let titles = titles_for(&live);
    assert!(titles[0].1.ends_with("窗口 1"), "首个窗口也编号：{}", titles[0].1);
    assert!(titles[1].1.ends_with("窗口 2"), "{}", titles[1].1);
    assert!(titles[2].1.ends_with("窗口 3"), "{}", titles[2].1);
}

#[test]
fn closing_a_window_renumbers_the_rest() {
    // 关掉位次1的窗口后，原窗口2/3 位次前移 → 编号顺延为 窗口 1 / 窗口 2。
    let titles = titles_for(&[2, 3]);
    assert!(titles[0].1.ends_with("窗口 1"), "{}", titles[0].1);
    assert!(titles[1].1.ends_with("窗口 2"), "{}", titles[1].1);
}

#[test]
fn ranks_follow_open_order_not_registry_insertion_order() {
    // for_each_id 遍历 HashMap，顺序随机 —— 位次必须按 id 升序（= 打开顺序），
    // 否则同一个状态每次刷新可能给出不同的名字。
    let ranks = ranks_of([3u64, 1, 2]);
    assert_eq!(ranks[&1], 1);
    assert_eq!(ranks[&2], 2);
    assert_eq!(ranks[&3], 3);
}

#[test]
fn rank_never_drifts_when_a_later_window_closes() {
    // 用户定稿是"后面打开的窗口**按序修改**" —— 关掉中间的窗口后，其后的窗口
    // 要**重编号**顶上来（窗口 3 变第二个 → 「窗口 2」），而不是保留旧号留空档。
    let before = titles_for(&[1, 2, 3, 4]);
    assert!(before[0].1.ends_with("窗口 1"));
    assert!(before[1].1.ends_with("窗口 2"));
    assert!(before[2].1.ends_with("窗口 3"));
    assert!(before[3].1.ends_with("窗口 4"));

    let after = titles_for(&[1, 3, 4]);
    // (#window-title-fix2) 多开时**全部编号**（含首个窗口 → 窗口 1）：
    // 关掉中间窗口后，其后窗口位次前移、编号顺延。
    assert!(after[0].1.ends_with("窗口 1"), "{}", after[0].1);
    assert!(after[1].1.ends_with("窗口 2"), "窗口 3 顶上来成第二个：{}", after[1].1);
    assert!(after[2].1.ends_with("窗口 3"), "窗口 4 顶上来成第三个：{}", after[2].1);
}

#[test]
fn tray_tip_and_window_title_agree_for_every_window() {
    // 同一存活集合下，托盘 tooltip 与状态栏标题必须逐窗口相等。
    for live in [
        vec![1u64],
        vec![1, 2],
        vec![1, 2, 3],
        vec![2, 5, 9],
    ] {
        let ranks = ranks_of(live.iter().copied());
        let total = live.len();
        for id in live {
            let title = window_title_for(ranks[&id], total);
            let tip = window_title_for(ranks[&id], total); // tray_tip_for 走同一函数
            assert_eq!(title, tip, "窗口 {id} 的标题与 tooltip 必须一致");
        }
    }
}

#[test]
fn sync_dialog_badge_reads_the_same_rank_as_the_title() {
    // (#sync-dialog-v2-fix8) 同步输入弹框的「窗口 N」徽标/悬浮卡与标题**同源**
    // （都读 ranks_of）：关掉 1 号窗口后，原 2/3 号窗口的行必须显示 窗口 1 /
    // 窗口 2，而不是停留在原始 registry id（用户实测：标题顺延了，弹框还挂
    // 旧号「窗口 3」）。
    let ranks = ranks_of([2u64, 3]);
    assert_eq!(ranks[&2], 1, "原 2 号窗口顺延成 窗口 1");
    assert_eq!(ranks[&3], 2, "原 3 号窗口顺延成 窗口 2");
    // 徽标与标题读同一个位次表 → 串尾数字必须一致。
    assert!(window_title_for(ranks[&3], 2).ends_with("窗口 2"));
    assert!(window_title_for(ranks[&2], 2).ends_with("窗口 1"));
}