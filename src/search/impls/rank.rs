//! (#double-shift-v1 2026-11-11) 排序与分组截断。
//!
//! 稳定序：评分降序 → 平手 title 字典序（高保真 cmp）。「全部」视图按
//! 会话 → 命令 → 设置 → 动作 分组，每组取前 [`MAX_PER_GROUP`] 条、总数
//! 封顶 [`MAX_TOTAL`]；单分类视图直接取前 [`MAX_TOTAL`] 条。

use super::{CatFilter, MatchRow};
use crate::search::{Category, GroupedMatches, SearchIndex};

/// 「全部」视图每分类组的条数上限（高保真 buildModel：每组 8）。
pub(crate) const MAX_PER_GROUP: usize = 8;
/// 结果总上限（单分类视图同样封顶）。
pub(crate) const MAX_TOTAL: usize = 40;

/// 组序（「全部」视图的分组顺序）。
pub(crate) const CAT_ORDER: [Category; 4] = [
    Category::Session,
    Category::Command,
    Category::Setting,
    Category::Action,
];

/// 排序：评分降序，平手 title 字典序（稳定）。
pub(crate) fn sort_rows<'a>(rows: &mut [MatchRow<'a>]) {
    rows.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.item.title.cmp(&b.item.title))
            .then_with(|| a.item.id.cmp(&b.item.id))
    });
}

/// 分组 + 截断。`cat = None` → 「全部」分组视图；`Some(c)` → 单分类平铺一个组。
/// 返回（分组序列, 扁平全量——供预览/键盘导航按扁平索引取条目）。
pub(crate) fn group_and_cap<'a>(
    mut rows: Vec<MatchRow<'a>>,
    cat: CatFilter,
) -> (Vec<GroupedMatches<'a>>, Vec<MatchRow<'a>>) {
    sort_rows(&mut rows);
    let mut groups: Vec<GroupedMatches<'a>> = Vec::new();
    match cat {
        Some(c) => {
            rows.truncate(MAX_TOTAL);
            if !rows.is_empty() {
                let items = rows
                    .iter()
                    .map(|r| MatchRow { item: r.item, score: r.score })
                    .collect();
                groups.push(GroupedMatches { cat: c, items });
            }
            (groups, rows)
        }
        None => {
            let mut flat: Vec<MatchRow<'a>> = Vec::with_capacity(rows.len());
            for group_cat in CAT_ORDER {
                if flat.len() >= MAX_TOTAL {
                    break;
                }
                let take: Vec<MatchRow<'a>> = rows
                    .iter()
                    .filter(|r| r.item.cat == group_cat)
                    .take(MAX_PER_GROUP.min(MAX_TOTAL - flat.len()))
                    .map(|r| MatchRow { item: r.item, score: r.score })
                    .collect();
                if take.is_empty() {
                    continue;
                }
                flat.extend(take.iter().map(|r| MatchRow { item: r.item, score: r.score }));
                groups.push(GroupedMatches { cat: group_cat, items: take });
            }
            (groups, flat)
        }
    }
}

/// 空查询视图：最近使用条目（按 recent_ids 回查；不足补最近会话）。
/// `recent_ids` 由宿主桥维护（执行成功时记住）。
pub(crate) fn recent_items<'a>(
    index: &'a SearchIndex,
    recent_ids: &[String],
) -> Vec<MatchRow<'a>> {
    let mut out: Vec<MatchRow<'a>> = Vec::new();
    for id in recent_ids {
        if let Some(item) = index.items.iter().find(|it| &it.id == id) {
            out.push(MatchRow { item, score: 0 });
        }
        if out.len() >= MAX_RECENT {
            break;
        }
    }
    if out.len() < MAX_RECENT {
        for item in index.items.iter().filter(|it| it.cat == Category::Session) {
            if out.iter().any(|r| r.item.id == item.id) {
                continue;
            }
            out.push(MatchRow { item, score: 0 });
            if out.len() >= MAX_RECENT {
                break;
            }
        }
    }
    out
}

/// 最近使用上限。
pub(crate) const MAX_RECENT: usize = 5;
