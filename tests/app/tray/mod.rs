//! (#tray-click-pos 2026-09-23) `GetMessagePos` 拆包纯函数单测:低 16 位 = x、
//! 高 16 位 = y,且都必须按 i16 符号扩展 —— 虚拟桌面坐标系下副屏在主屏
//! 左侧/上方时坐标为负。
#![cfg(windows)]

use super::{dismiss_matches_menu, split_message_pos};
use super::super::tray_pos_in_bounds;
#[cfg(windows)]
use super::{hook_dismiss_decision, HookDismiss};

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
