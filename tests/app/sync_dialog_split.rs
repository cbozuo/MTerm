//! (#sync-dialog-v2-fix3 2026-10-12) 同步输入弹框左右分栏的回归测试。
//!
//! 用户反馈两条：
//!   1. 「弹框中操作加入的频道，只在悬浮信息框展示了，但是没有在列表中显示」
//!      —— 点「+」后，悬浮卡里的「频道：频道 B（本次待确认生效）」变了，
//!      但那一行既没从左栏消失、也没出现在右栏。
//!   2. 切频道页签后两栏内容不跟着换。
//!
//! 根因是分栏**由 UI 侧做**：`for r in sessions: Rectangle { height: r.slot ==
//! ... ? 29px : 0px }`。Rust 的 `set_row_data` 改了行数据（计数重算对了、
//! 悬浮卡读到的也是新值），但 repetition 内直接依赖 `r.slot` 的 `height` 绑定
//! 不跟着重算 —— 行既没折叠也没展开，看起来就是"只更新了悬浮卡"。
//!
//! 改为分栏由 Rust 侧决定并**重新灌两个独立模型**（left-rows / right-rows）。
//! 这里锁定那条判定规则，防止它再退化。
use super::{row_side, Side};

/// 未加入任何频道的行（slot = -1）在当前频道页签下应落到左栏（候选）。
#[test]
fn unjoined_row_lands_in_left_column() {
    assert_eq!(row_side(-1, 0, 1, 1, 0), Side::Left);
}

/// 已加入**当前**频道的行归右栏，且不再出现在左栏（两栏互斥）。
#[test]
fn member_of_active_channel_lands_in_right_column_only() {
    assert_eq!(row_side(2, 2, 1, 1, 0), Side::Right);
    assert_ne!(row_side(2, 2, 1, 1, 0), Side::Left);
}

/// 已加入**其它**频道的行，在当前页签下是候选 → 左栏。
/// 这是「已加入的频道样式」药丸要展示的场景。
#[test]
fn member_of_another_channel_is_a_left_column_candidate() {
    assert_eq!(row_side(0, 1, 1, 1, 0), Side::Left);
}

/// 切换页签后，同一行必须换边 —— 对应用户反馈的"切页签两栏不换"。
#[test]
fn switching_channel_tab_moves_rows_across_columns() {
    let row_slot = 1;
    // 停在频道 A(slot=0)：行属于 B → 左栏。
    assert_eq!(row_side(row_slot, 0, 1, 1, 0), Side::Left);
    // 切到频道 B(slot=1)：同一行变成成员 → 右栏。
    assert_eq!(row_side(row_slot, 1, 1, 1, 0), Side::Right);
}

/// 作用域 = 当前窗口时，别窗口的行两边都不出现（不污染本窗口的候选列表）。
#[test]
fn current_window_scope_hides_other_windows_rows() {
    // 行属于别的窗口(2)，当前窗口是 1
    assert_eq!(row_side(-1, 0, 2, 1, 0), Side::Hidden);
}

/// 作用域 = 全部窗口时，所有窗口的行都参与分栏。
#[test]
fn all_windows_scope_includes_cross_window_rows() {
    assert_eq!(row_side(-1, 0, 2, 1, 1), Side::Left);
    // 跨窗口的**成员**仍归右栏（右栏成员跨窗口常显，不受作用域限制）。
    assert_eq!(row_side(0, 0, 7, 1, 1), Side::Right);
    // 跨窗口的行即使在"当前窗口"作用域下属于当前频道，也照样进右栏。
    assert_eq!(row_side(0, 0, 7, 1, 0), Side::Right);
}

/// 每一行都必须落到某一栏或被显式排除，不允许"既在左栏又在右栏"。
#[test]
fn a_row_never_appears_in_both_columns() {
    for row_slot in -1..=3 {
        for active in 0..4 {
            for scope in 0..=1 {
                let side = row_side(row_slot, active, 1, 1, scope);
                assert!(
                    matches!(side, Side::Left | Side::Right | Side::Hidden),
                    "slot={row_slot} active={active} scope={scope} 落到了非法侧 {side:?}"
                );
                // 同一侧判定是纯函数：重复调用必须一致（UI 靠重灌模型拿到一致结果）。
                assert_eq!(side, row_side(row_slot, active, 1, 1, scope));
            }
        }
    }
}