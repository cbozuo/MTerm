//! (#double-shift-v1 2026-11-11) 查询：分词 → token AND → 分层评分。
//!
//! 评分分层移植自高保真稿 §03 的打分表（精确 / 前缀 / 词界 / 子串 / 子序列
//! × title / keyword / sub 三层）。全同步、无 I/O：几百条索引一次内存遍历。

use crate::search::{Category, MatchRow, SearchIndex, SearchItem};

/// 「全部」之外的单分类过滤（None = 全部）。
pub(crate) type CatFilter = Option<Category>;

/// 空白分词（小写）。空 query → 空 vec（上层转「最近使用」视图）。
pub(crate) fn tokenize(query: &str) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    q.split_whitespace().map(str::to_string).collect()
}

/// 子序列判定（`token` 的字符按序全在 `text` 中）。
fn is_subsequence(text: &str, token: &str) -> bool {
    let mut it = text.chars();
    token.chars().all(|tc| it.any(|c| c == tc))
}

/// 单层打分。`tier`：0 = title / 1 = keyword / 2 = sub（分值表见高保真 §03）。
fn score_text(hay: &str, token: &str, tier: u8) -> i32 {
    if hay.is_empty() {
        return 0;
    }
    // 精确
    if hay == token {
        return match tier {
            1 => 900,
            2 => 840,
            _ => 1200,
        };
    }
    // 前缀
    if hay.starts_with(token) {
        return match tier {
            1 => 650,
            2 => 700,
            _ => 1000,
        };
    }
    // 子串（title 层区分词界：命中点前是分隔符或开头 → 更高）
    if let Some(idx) = hay.find(token) {
        if tier == 0 {
            let prev = hay[..idx].chars().next_back();
            let boundary = idx == 0
                || prev
                    .is_none_or(|c| matches!(c, ' ' | '-' | '_' | '/' | '.' | ':' | '·' | '›'));
            return if boundary { 850 } else { 700 };
        }
        return match tier {
            1 => 550,
            _ => 490,
        };
    }
    // 子序列（两字符以上才算，避免单字母误伤面过大）
    if token.chars().count() >= 2 && is_subsequence(hay, token) {
        return match tier {
            1 => 323,
            2 => 266,
            _ => 380,
        };
    }
    0
}

/// 整条打分：每个 token 必须命中（AND），总分 = 各 token 最佳层之和。
/// 返回 -1 = 不匹配。
pub(crate) fn score_item(item: &SearchItem, tokens: &[String]) -> i32 {
    let mut total = 0i32;
    for token in tokens {
        // 门面：全串不含该 token 时先做子序列预判（title/kw/sub 任一命中才
        // 进入打分）—— 子序列可散落在 title 里而 hay_all 不含完整 token
        // （`doc-keeper` 命中 `docker`），直接判负会把子序列层整个挡掉。
        if !item.hay_all.contains(token.as_str())
            && !is_subsequence(&item.hay_title, token)
            && !is_subsequence(&item.hay_kw, token)
            && !is_subsequence(&item.hay_sub, token)
        {
            return -1;
        }
        let best = score_text(&item.hay_title, token, 0)
            .max(score_text(&item.hay_kw, token, 1))
            .max(score_text(&item.hay_sub, token, 2));
        if best == 0 {
            return -1;
        }
        total += best;
    }
    // 已打开标签加成 + 最近使用加成（高保真 scoreItem 尾部两条）。
    if item.open {
        total += 40;
    }
    if item.recent_rank > 0 {
        total += (12 - item.recent_rank).max(0);
    }
    total
}

/// 查询：命中列表（未分组、未截断；排序在 rank.rs）。
/// 空 tokens 返回 None（上层转「最近使用」视图），而不是全量。
pub(crate) fn search<'a>(
    index: &'a SearchIndex,
    query: &str,
    cat: CatFilter,
) -> Option<Vec<MatchRow<'a>>> {
    let tokens = tokenize(query);
    if tokens.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for item in &index.items {
        if let Some(c) = cat {
            if item.cat != c {
                continue;
            }
        }
        let score = score_item(item, &tokens);
        if score >= 0 {
            out.push(MatchRow { item, score });
        }
    }
    Some(out)
}
