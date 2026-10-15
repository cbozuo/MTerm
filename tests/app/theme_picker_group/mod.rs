//! (#theme-group-mode 2026-10-15) 主题选择器「暗一组 / 亮一组」分组的回归。
//!
//! 用户反馈：「两个地方的下拉中的主题，不要按照现在的暗亮在一起，要按照
//! 暗一组 亮一组，分为两组」。改动落在 `build_theme_entries`：全部主题区从
//! 「按 palettes.rs 顺序（暗亮成对相邻）」改为「先暗后亮两组，各带子组头」。
//!
//! 这里锁四件事：
//!   ① 全部主题区内**所有暗套在所有亮套之前**（分组本身）；
//!   ② 两组各有且仅有一条子组头（「暗色」/「亮色」，英文 Dark/Light）；
//!   ③ 子组头计数 = 该组实际行数（常用区占用的套不进全部主题区）；
//!   ④ 组头全是 kind == "header"（搜索时统一隐藏、键盘游标统一跳过，
//!      不依赖具体是哪一级组头）。
//!
//! 喂列表即可跑（build_theme_entries 只吃收藏 id，不碰 ConfigStore）。

use super::build_theme_entries;

/// 常用区之后、全部主题区的结构切片。
fn all_theme_section(entries: &[crate::ui::ThemeEntry]) -> &[crate::ui::ThemeEntry] {
    // 第二条组头（index 0 是常用区组头）之后即全部主题区。
    let second = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == "header")
        .nth(1)
        .map(|(i, _)| i)
        .expect("应有常用 / 全部主题两条组头");
    &entries[second + 1..]
}

fn theme_rows(section: &[crate::ui::ThemeEntry]) -> Vec<&crate::ui::ThemeEntry> {
    section.iter().filter(|e| e.kind != "header").collect()
}

#[test]
fn all_themes_split_into_dark_then_light_groups() {
    let entries = build_theme_entries(&[]);
    let section = all_theme_section(&entries);

    // 子组头：暗色 → 亮色，各一条。
    let sub_headers: Vec<&str> = section
        .iter()
        .filter(|e| e.kind == "header")
        .map(|e| e.zh.as_str())
        .collect();
    assert_eq!(
        sub_headers,
        vec![
            if crate::i18n::is_en() { "Dark" } else { "暗色" },
            if crate::i18n::is_en() { "Light" } else { "亮色" },
        ],
        "全部主题区应恰有「暗色 / 亮色」两条子组头"
    );

    // 分组：暗套全部在亮套之前（组内不再混插）。
    let modes: Vec<&str> = theme_rows(section).iter().map(|e| e.mode.as_str()).collect();
    let first_light = modes.iter().position(|m| *m == "亮");
    let last_dark = modes.iter().rposition(|m| *m == "暗");
    match (first_light, last_dark) {
        (Some(fl), Some(ld)) => assert!(
            ld < fl,
            "暗组必须整体在亮组之前：最后一个暗在第 {}，第一个亮在第 {}",
            ld,
            fl
        ),
        _ => panic!("两组都应非空：暗 {} / 亮 {}", modes.iter().filter(|m| **m == "暗").count(), modes.iter().filter(|m| **m == "亮").count()),
    }

    // 计数与总量：常用区为空时全部主题区 = 全目录。
    let total = crate::theme::palettes::PALETTES.len();
    assert_eq!(theme_rows(section).len(), total);
    let dark_n = crate::theme::palettes::PALETTES.iter().filter(|p| p.dark).count();
    assert_eq!(theme_rows(section).iter().filter(|e| e.mode == "暗").count(), dark_n);
}

#[test]
fn favorite_themes_move_to_top_and_leave_the_catalog() {
    // 取一套暗、一套亮各做常用（用目录里真实存在的 id）。
    let dark_id = crate::theme::palettes::PALETTES
        .iter()
        .find(|p| p.dark)
        .map(|p| p.id.to_string())
        .unwrap();
    let light_id = crate::theme::palettes::PALETTES
        .iter()
        .find(|p| !p.dark)
        .map(|p| p.id.to_string())
        .unwrap();
    let entries = build_theme_entries(&[dark_id.clone(), light_id.clone()]);

    // 常用区：组头后紧跟这两套（按传入顺序），faved = true。
    let fav_rows: Vec<&str> = entries
        .iter()
        .skip(1)
        .take_while(|e| e.kind != "header")
        .map(|e| e.id.as_str())
        .collect();
    assert_eq!(fav_rows, vec![dark_id.as_str(), light_id.as_str()]);

    // 全部主题区不再包含它们，且计数相应减少。
    let section = all_theme_section(&entries);
    assert!(!theme_rows(section).iter().any(|e| e.id == dark_id || e.id == light_id));
    assert_eq!(
        theme_rows(section).len(),
        crate::theme::palettes::PALETTES.len() - 2
    );
    // 常用区不受分组影响（用户自藏顺序），仍紧挨组头。
    assert_eq!(entries[0].kind, "header");
}

#[test]
fn sub_group_header_counts_match_their_rows() {
    let entries = build_theme_entries(&[]);
    let section = all_theme_section(&entries);
    let mut checked = 0;
    let mut idx = 0usize;
    while idx < section.len() {
        if section[idx].kind == "header" {
            // 组头下面直到下一条组头的行数 = 计数文案里的数字
            let rows = section[idx + 1..]
                .iter()
                .take_while(|e| e.kind != "header")
                .count();
            let expect = if crate::i18n::is_en() {
                format!("{} themes", rows)
            } else {
                format!("{} 套", rows)
            };
            assert_eq!(
                section[idx].mode, expect,
                "子组头「{}」的计数应是其下真实行数",
                section[idx].zh
            );
            checked += 1;
        }
        idx += 1;
    }
    assert_eq!(checked, 2, "应校验暗 / 亮两条子组头");
}
