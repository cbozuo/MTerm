//! (#cmd-hint 2026-10-27) 命令提示框的 **Slint 宿主桥**。
//!
//! 领域规则（阶段判定 / 打分 / 排序 / 候选生成）在 `crate::command_hint`；
//! 这里只负责把 `HintModel` 写进两套 Slint 属性：
//!
//!   · `hint-*`      —— 命令栏那条浮出条（P0）
//!   · `term-hint-*` —— 终端内联浮层（P1）
//!
//! 两处互不干扰：命令栏读 `hint-*`，终端内联读 `term-hint-*`。它们可以同时
//! 存在（终端里敲字的同时命令栏也有半行草稿），但浮层只有当前聚焦的那个会显示。
//!
//! 单独放在 app 层的理由：这两个函数需要 `AppWindow`（Slint 生成类型），
//! 而领域层刻意不依赖 UI —— 否则领域规则就没法脱离 UI 单独测试了。

use crate::command_hint::HintModel;
use crate::ui::{AppWindow, HintRow};
use slint::{ModelRc, VecModel};
use std::rc::Rc;

/// 把 `HintModel` 写成**终端内联**那一组属性（P1）。
pub(crate) fn apply_terminal_hint_model(win: &AppWindow, model: &HintModel) {
    let rows = hint_rows_model(model);
    win.set_term_hint_rows(rows);
    win.set_term_hint_phase(model.phase.label().into());
    win.set_term_hint_context(model.context.clone().into());
    win.set_term_hint_total(model.total as i32);
    win.set_term_hint_open(model.open);
}

/// 把 `HintModel` 写成命令栏那一组属性（P0）。
///
/// 单点写入：hint-* 六列必须**一起**更新，否则会出现「列表换了、阶段胶囊还是
/// 上一次的」这种半新半旧的一帧。所有调用点都走这里。
pub(crate) fn apply_hint_model(win: &AppWindow, model: &HintModel) {
    win.set_hint_rows(hint_rows_model(model));
    win.set_hint_phase(model.phase.label().into());
    win.set_hint_context(model.context.clone().into());
    win.set_hint_total(model.total as i32);
}

/// `HintModel.rows` → Slint 模型。两个 apply 共用，避免字段名抄两遍。
fn hint_rows_model(model: &HintModel) -> ModelRc<HintRow> {
    let rows: Vec<HintRow> = model
        .rows
        .iter()
        .map(|r| HintRow {
            text: r.text.clone().into(),
            kind: r.kind as i32,
            subtitle: r.subtitle.clone().into(),
            source: r.source.clone().into(),
            signature: r.signature.clone().into(),
            description: r.description.clone().into(),
            apply: r.apply.clone().into(),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}
