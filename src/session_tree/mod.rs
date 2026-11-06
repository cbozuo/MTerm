//! 左侧会话面板（#left-panel-v3 2026-11-06）的**引擎层**。
//!
//! 分层与 `crate::ping` / `crate::command_hint` 同款：领域类型与行几何规则在
//! `struct/`，拍平与定位实现在 `impls/`，Slint 宿主桥（`TreeRow` →
//! `ui::SessionInfo` 转换、模型下发、回调接线）在 `crate::app::session_models`
//! 与 `crate::app`。本层不依赖 Slint 类型 —— 组色用 hex 字符串承载，转
//! `slint::Color` 在桥层。
//!
//! 结构对应 ui/left_sidebar.slint：
//!  · `flatten` 把 `ConfigStore::group_records` 树递归拍平为「组头行 → 子组 →
//!    成员 → 空组占位」的扁平行序列（Slint 只吃扁平行，不递归布局 —— 设计稿
//!    §06 P0②）；同组内**子组在前**，成员块连续，组内拖拽几何才成立。
//!  · `locate` 是「定位当前终端」（IDEA Select in Project View 同款）的引擎
//!    半边：展开目标会话的祖先分组并落盘折叠态；滚动坐标由桥层用本模块的
//!    行几何常量对拍平后的行序累计。
//!  · `struct::row` 的行高/间距常量是 Slint 侧行布局的**唯一 Rust 真源**：
//!    Slint 侧改行高/间距必须同步改这里（两侧注释互指 #tree-geom）。

#[path = "struct/row.rs"]
pub(crate) mod row;
#[path = "impls/flatten.rs"]
pub(crate) mod flatten;
#[path = "impls/locate.rs"]
pub(crate) mod locate;

pub(crate) use flatten::{builtin_local_sessions, build_tree_rows};
pub(crate) use locate::expand_ancestors;
pub(crate) use row::{row_metrics, GroupSort, TreeRow};
// locate_center_y 与行高常量(ROW_*)仅供模块内测试/未来桥层使用,不在根转呈,
// 避免未用告警;需要时按 session_tree::locate::locate_center_y 全路径引用。
