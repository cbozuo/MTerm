#[path = "struct/components.rs"]
mod components;

// (#scrim-unify 2026-10-15) 生成组件对 crate 外开放：tests/ 下的渲染级回归
// （tests/dialog_scrim_drag.rs）要自建离屏平台驱动真的 AppWindow 并读回
// Theme 全局的遮罩色值。仅测试需要，但 glob 比逐条列名省事且无副作用 ——
// 生成类型本就是 UI 的镜像。
pub use components::*;
