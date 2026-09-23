//! (#tray-click-pos 2026-09-23) `GetMessagePos` 拆包纯函数单测:低 16 位 = x、
//! 高 16 位 = y,且都必须按 i16 符号扩展 —— 虚拟桌面坐标系下副屏在主屏
//! 左侧/上方时坐标为负。
#![cfg(windows)]

use super::{dismiss_matches_menu, split_message_pos};
use super::super::tray_pos_in_bounds;

#[test]
fn dismiss_message_only_closes_its_own_menu() {
    assert!(dismiss_matches_menu(3, 3, 123));
    assert!(!dismiss_matches_menu(3, 4, 456));
    assert!(!dismiss_matches_menu(3, 3, 0));
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
