//! (#cmd-hint 2026-10-27) 命令提示框的**领域层**。
//!
//! 高保真稿：`docs/design/terminal-command-hint-hifi-2026-10-27.html`
//!
//! 高保真把提示框定为**四段式**：上下文条 / 候选列表 / 详情区 / 键盘脚注。
//! 原来的 `suggest_models()` 只产出「展示文本 + 类型」两列，够画一个扁平列表，
//! 画不了详情区（语法签名、一句话说明、来源）和阶段胶囊。这里补上那几列，
//! 并且**保持原函数不动** —— 完整历史面板还在读它。
//!
//! 三条与高保真对齐的硬规则：
//!   1. 阶段由**当前输入**判定（命令 / 子命令 / 参数 / 路径），不是猜的；
//!   2. 候选上限 = `HINT_MAX_ROWS`，选中项不可能越出渲染出来的行；
//!   3. 空 query 不返回任何候选（高保真 §4「空串处短路」），避免一聚焦就刷屏。
//!
//! P1（终端内联）用的是同一份生成器：只要把「终端里正在敲的那一行」喂进来，
//! 阶段的判定与候选就完全一致，两处宿主不会长出两套语义。
//!
//! # 分层
//!
//! 对齐项目既有约定（参照 `src/terminal/`、`src/config/` 的 `struct/` + `impls/`）：
//!
//! ```text
//! struct/model.rs      —— 纯数据：阶段 / 来源 / 输入切分 / 候选行 / 整块模型
//! struct/knowledge.rs  —— 内置子命令静态表（数据，与算法解耦）
//! impls/suggest.rs     —— 纯逻辑：触发阈值 / 相关度打分 / 排序 / 阶段分流
//! ```
//!
//! **宿主桥不在这里**：把 `HintModel` 写成 Slint 属性的 `apply_*` 放在
//! `src/app/command_hint_ui.rs`。这样这一层完全脱离 UI，可被单元测试直接
//! 驱动，也不再需要 `use super::*` 那种把 app 层全部导入面吞进来的写法。

#[path = "struct/model.rs"]
mod model;
#[path = "struct/knowledge.rs"]
mod knowledge;
#[path = "impls/suggest.rs"]
mod suggest;

pub(crate) use knowledge::subcommands_for;
pub(crate) use model::{HintInput, HintKind, HintModel, HintPhase, HintRowData};
pub(crate) use suggest::hint_model;

// 只给单元测试用（`tests/app/command_hint/mod.rs` 直接测纯函数）。
// 放 `#[cfg(test)]` 是因为产品代码走 `hint_model`，不直接碰这两个。
#[cfg(test)]
pub(crate) use model::compute_apply;
#[cfg(test)]
pub(crate) use suggest::should_pop;

/// 候选列表上限（用户实测反馈：5 条太少，放宽到 8 条）。
/// `ui/command_hint.slint` 的列表行数与可见行数按同一个数排，
/// 两边必须一致，否则选中项会落在看不见的行上。
pub(crate) const HINT_MAX_ROWS: usize = 50;

// 测试仍留在 `tests/app/command_hint/`（那里是「按特性归档」的既有习惯，
// 同一批还有 command_history / terminal_input …）。路径是相对本文件所在目录，
// 与搬到本模块之前同深（src/command_hint/ ↔ src/app/），所以路径不用改。
#[cfg(test)]
#[path = "../../tests/app/command_hint/mod.rs"]
mod command_hint_tests;
