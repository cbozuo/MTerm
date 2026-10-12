#[cfg(test)]
#[path = "../../tests/app/window_management/launch_intent.rs"]
mod launch_intent_tests;

/// What this process launch is supposed to do, parsed from argv before any
/// window exists. `--new-window` is issued by the OS entry points (Windows
/// jump list, macOS dock menu, Linux desktop action); when an instance is
/// already running it is forwarded over the single-instance socket instead
/// of opening a second process.
///
/// (#sync-dialog-v2-fix 2026-10-12) 二次启动（无论是否带 `--new-window`）现在
/// **统一转发**到主实例开新窗，`new_window` 不再影响行为 —— 字段保留只为
/// 继续解析/接受该 CLI 旗标（jump list 等入口不能因未知参数报错）。
pub struct LaunchIntent {
    #[allow(dead_code)]
    pub new_window: bool,
}

pub fn parse(args: &[String]) -> LaunchIntent {
    LaunchIntent {
        new_window: args.iter().any(|a| a == "--new-window"),
    }
}
