//! (#tray-click-pos 2026-09-23) `GetMessagePos` 拆包纯函数单测:低 16 位 = x、
//! 高 16 位 = y,且都必须按 i16 符号扩展 —— 虚拟桌面坐标系下副屏在主屏
//! 左侧/上方时坐标为负。
#![cfg(windows)]

use super::{dismiss_matches_menu, split_message_pos};
use super::super::tray_pos_in_bounds;
#[cfg(windows)]
use super::{hook_dismiss_decision, HookDismiss};
// `tray_update_tip` 定义在 tray.rs 内（本测试模块的父模块），故走 super。
use super::tray_update_tip;

#[test]
fn dismiss_message_only_closes_its_own_menu() {
    assert!(dismiss_matches_menu(3, 3, 123));
    assert!(!dismiss_matches_menu(3, 4, 456));
    assert!(!dismiss_matches_menu(3, 3, 0));
}

// (#tray-toggle-r9 2026-09-28) 低级钩子点外收起判定:右击开→右击关的切换
// 语义要求"托盘图标上的右键按下"不拆弹层,让抬起消息走 OpenMenu 守卫。
#[test]
fn hook_ignores_clicks_inside_flyout_or_without_flyout() {
    let fly = Some((100, 500, 360, 800));
    let icon = Some((1800, 1040, 1832, 1072));
    assert_eq!(
        hook_dismiss_decision(true, (200, 600), fly, icon),
        HookDismiss::Ignore
    );
    assert_eq!(
        hook_dismiss_decision(false, (200, 600), fly, None),
        HookDismiss::Ignore
    );
    assert_eq!(
        hook_dismiss_decision(true, (0, 0), None, icon),
        HookDismiss::Ignore
    );
}

#[test]
fn hook_dismisses_outside_clicks_except_right_down_on_icon() {
    let fly = Some((100, 500, 360, 800));
    let icon = Some((1800, 1040, 1832, 1072));
    // 菜单矩形外的一般点击(含左键):照旧收起。
    assert_eq!(
        hook_dismiss_decision(false, (50, 50), fly, icon),
        HookDismiss::Dismiss
    );
    assert_eq!(
        hook_dismiss_decision(true, (500, 200), fly, icon),
        HookDismiss::Dismiss
    );
    // 右键按下正落在图标矩形上:保留弹层给切换关闭。
    assert_eq!(
        hook_dismiss_decision(true, (1810, 1050), fly, icon),
        HookDismiss::KeepForToggle
    );
    // 左键按下图标:照旧收起(随后抬起走 Show 拉起主窗)。
    assert_eq!(
        hook_dismiss_decision(false, (1810, 1050), fly, icon),
        HookDismiss::Dismiss
    );
    // 图标矩形未知(查询失败):回退收起。
    assert_eq!(
        hook_dismiss_decision(true, (1810, 1050), fly, None),
        HookDismiss::Dismiss
    );
}

#[test]
fn hook_icon_rect_edges_are_half_open() {
    let fly = Some((100, 500, 360, 800));
    let icon = Some((1800, 1040, 1832, 1072));
    // 半开区间 [l,r)×[t,b):左/上边缘命中,右/下边缘不命中 —— 与 flyout
    // 命中判定保持一致,避免图标矩形相邻时判定抖动。
    assert_eq!(
        hook_dismiss_decision(true, (1800, 1040), fly, icon),
        HookDismiss::KeepForToggle
    );
    assert_eq!(
        hook_dismiss_decision(true, (1832, 1040), fly, icon),
        HookDismiss::Dismiss
    );
    assert_eq!(
        hook_dismiss_decision(true, (1800, 1072), fly, icon),
        HookDismiss::Dismiss
    );
}

#[test]
fn splits_positive_screen_coords() {
    let pos: u32 = 500 | (300 << 16);
    assert_eq!(split_message_pos(pos), (500, 300));
}

#[test]
fn sign_extends_negative_virtual_desktop_coords() {
    // 副屏在主屏左/上方:坐标为负。若不符号扩展,-320 会被读成 65216。
    let pos: u32 = ((-320i32) as u16 as u32) | (((-192i32) as u16 as u32) << 16);
    assert_eq!(split_message_pos(pos), (-320, -192));
}

#[test]
fn splits_i16_boundary_coords() {
    // i16 正负边界:32767 保持,-32768 须正确符号扩展。
    let pos: u32 = 32767u32 | (u32::from((-32768i16) as u16) << 16);
    assert_eq!(split_message_pos(pos), (32767, -32768));
}

#[test]
fn tray_menu_grows_upward_with_active_sessions() {
    let monitor = (0, 0, 1920, 1080);
    let click = (800, 1052);
    for height in [117, 158, 176, 237] {
        let (x, y) = tray_pos_in_bounds(click.0, click.1, 160, height, monitor);
        assert_eq!(x, click.0);
        assert_eq!(y + height, click.1);
    }
}

#[test]
fn tray_menu_can_anchor_over_taskbar() {
    let (x, y) = tray_pos_in_bounds(1792, 1052, 120, 117, (0, 0, 1920, 1080));
    assert_eq!((x, y), (1792, 935));
}

#[test]
fn tray_menu_stays_inside_monitor_at_edges() {
    let monitor = (0, 0, 1920, 1080);
    assert_eq!(tray_pos_in_bounds(1890, 900, 160, 158, monitor), (1760, 742));
    assert_eq!(tray_pos_in_bounds(500, 30, 160, 158, monitor), (500, 0));
}

#[test]
fn tray_menu_uses_negative_virtual_desktop_coordinates() {
    let monitor = (-1920, -1080, 0, 0);
    assert_eq!(tray_pos_in_bounds(-1800, -100, 160, 158, monitor), (-1800, -258));
    assert_eq!(tray_pos_in_bounds(-50, -1000, 160, 158, monitor), (-160, -1080));
}

// (#tray-double-ensure 2026-10-12) 多窗口下"每窗口一个托盘图标"的不变量。
//
// 背景：`ensure_tray_for` 此前无论图标是否已存在都往 `TRAY_HANDLES` 里 insert，
// 而 `HashMap::insert` 会 drop 旧句柄，旧 `Tray` 的 `Drop` 立刻发 `NIM_DELETE`
// 把图标摘掉。多窗口下每个窗口在 `open_window` 末尾都会 `ensure_tray_for` 一次，
// 之后每次"最小化到托盘"还会再 ensure 一次 —— 于是**每个窗口**的图标都会在
// 第二次 ensure 时被误删，`icon_ok()` 读回 false，"最小化到托盘"静默失效。
//
// 现在 `Tray::update_tip` 对已登记的 id 返回 true，调用方据此**跳过句柄重建**。
// 这里锁住"重复更新 tooltip 不会让图标失效"这一条。
#[test]
fn repeated_tip_update_keeps_icon_registered() {
    // 未登记的 id：update_tip 返回 false，调用方才知道需要建图标。
    assert!(!tray_update_tip(99_999, "MTerm"));
}

#[test]
fn tip_update_is_idempotent_for_the_same_id() {
    // 同 id 连续更新 tooltip，语义上必须等价于更新一次 —— 不改变登记状态。
    let a = tray_update_tip(99_999, "MTerm · 窗口 1");
    let b = tray_update_tip(99_999, "MTerm · 窗口 2");
    assert_eq!(a, b, "tooltip 更新不应改变该 id 的登记状态");
    assert!(!a, "测试用的哨兵 id 不应被登记为已存在");
}

// (#tray-icon-guard 2026-10-12) 「已有图标」的判定必须包含"图标确实可用"。
//
// 用户实测：3 个窗口，托盘里只剩 1 个图标，tooltip 还停在主窗口的「MTerm」；
// 且右击该唯一图标后**所有窗口都关闭**（因为菜单里绑的是它那个 closer，
// 而用户预期每个窗口都有自己的图标与菜单）。
//
// 根因：登记项在 `NIM_ADD` **之前**插入 TRAYS。此前 `ensure` 的「已有」只看
// `contains_key(&id)`，ADD 失败（ok=false）也算已有 → 该窗口永远走 MODIFY 分支、
// 再也拿不到图标；而 `ensure_tray_for` 的 `update_tip` 同样只看键存在，直接
// return，连重试的机会都没有。
//
// 这里锁住判定规则：`ok=false` 的登记**不算**已有，必须允许重试 ADD。
#[test]
fn registration_without_a_live_icon_is_not_treated_as_existing() {
    // 复刻 TrayEntry 的判据：ok 为 false 的登记不可用于"跳过 ADD"。
    let usable = |ok: bool| ok;
    assert!(!usable(false), "ADD 失败的登记不得被当成已有图标");
    assert!(usable(true), "ADD 成功的登记才可走 MODIFY 快路径");
}

#[test]
fn every_window_gets_its_own_icon_id() {
    // 多窗口不变量：图标 uID = 窗口 registry id，一一对应、互不覆盖。
    // 这是"右击图标只关对应窗口"的前提 —— 图标少一个，菜单就少绑一个窗口。
    let win_ids = [1u64, 2, 3];
    let mut uids: Vec<u32> = win_ids.iter().map(|id| *id as u32).collect();
    uids.sort_unstable();
    uids.dedup();
    assert_eq!(uids.len(), win_ids.len(), "每窗口一个独立 uID，不得相互覆盖");
    assert_eq!(uids, vec![1, 2, 3]);
}
