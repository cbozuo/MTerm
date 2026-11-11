//! (#double-shift-v1 2026-11-11) 双击 Shift 全局搜索的**领域层**（SearchService）。
//!
//! 高保真稿：`docs/design/double-shift-hifi-2026-11-06-v1.html`。
//!
//! # 分层
//!
//! 对齐 `src/command_hint/` 的既有约定（`struct/` 纯数据 + `impls/` 行为）：
//!
//! ```text
//! struct/model.rs    —— 纯数据：Category / SearchItem / SearchIndex / MatchRow
//! struct/catalog.rs  —— 设置项与全局动作的静态清单（别名+拼音+ifd-page）
//! impls/index.rs     —— 五类数据摊平建索引（打开面板时重建，无 I/O 查询路径）
//! impls/query.rs     —— 分词 / token AND / 分层评分
//! impls/rank.rs      —— 稳定排序 / 分组截断 / 最近使用
//! impls/trigger.rs   —— 双击 Shift 状态机 / IME 污染过滤
//! ```
//!
//! **宿主桥不在这里**：把模型写进 Slint 属性、执行语义、触发链接线都在
//! `src/app/search_ui.rs`。这一层完全脱离 UI，可被单元测试直接驱动。
//!
//! # 性能口径（高保真 §05）
//!
//! 查询全同步：几百条索引一次内存遍历；不防抖、不骨架屏、不异步。
//! 打开耗时 / 查询耗时由宿主桥打点推给脚注。

#[path = "struct/model.rs"]
mod model;
#[path = "struct/catalog.rs"]
mod catalog;
#[path = "impls/index.rs"]
mod index;
#[path = "impls/query.rs"]
mod query;
#[path = "impls/rank.rs"]
mod rank;
#[path = "impls/trigger.rs"]
mod trigger;

pub(crate) use index::{build_index, SessionView};
pub(crate) use model::{Category, Counts, GroupedMatches, MatchRow, Payload, SearchIndex, SearchItem};
pub(crate) use query::{search, tokenize, CatFilter};
pub(crate) use rank::{group_and_cap, recent_items, CAT_ORDER, MAX_PER_GROUP, MAX_TOTAL, MAX_RECENT};
pub(crate) use trigger::{classify, DoubleShiftTracker, KeyEvent};

#[cfg(test)]
#[path = "../../tests/app/search/mod.rs"]
mod search_tests;
