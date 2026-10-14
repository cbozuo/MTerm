//! MTerm 库 crate（lib + bin 分离的"lib"侧）。
//!
//! 顶层模块清单与启动流程骨架集中在本文件；二进制目标 `MTerm`
//! （`src/main.rs`）只做全局分配器注册 + 转发到 [`run`]。
//! 集成测试与 `cargo doc` 通过 `use mterm::…` 直接访问本 crate。

pub mod allocator;
pub mod app;
pub mod automation;
pub mod cli;
pub mod config;
pub mod datastore;
pub mod i18n;
pub mod layout;
pub mod logging;
pub mod mcp;
pub mod picker;
pub mod rdp;
pub mod remote;
pub mod resource;
pub mod session;
pub mod sftp;
pub mod ssh;
pub mod terminal;
pub mod theme;
pub mod tunnel;
pub mod ui;
pub mod vnc;
pub mod wallpaper;
pub mod webdav;

/// 进程主流程：解析参数 → 初始化日志 → 安装 TLS provider → 分发三条主线。
///
/// 顺序敏感，不要重排：
/// 1. Version 分支提前返回，不初始化任何子系统（`--version` 必须零副作用）；
/// 2. tracing 先装，后续所有 `tracing::` 宏的输出才有去处；
/// 3. rustls crypto provider 必须早于任何 `ClientConfig::builder()`
///    —— 原因见 [`rdp::install_rustls_crypto_provider`] 的文档。
pub fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mode = cli::StartMode::detect(&args);
    if matches!(mode, cli::StartMode::Version) {
        println!("MTerm {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    logging::init_tracing();
    rdp::install_rustls_crypto_provider();

    match mode {
        cli::StartMode::Mcp => mcp::run_stdio(),
        cli::StartMode::Cli => cli::run(&args),
        cli::StartMode::App => {
            // macOS 默认走 Slint 软件渲染器：两个 GPU 后端都有机型级的"文字
            // 整体消失"事故（femtovg #108 / skia #129），所以软件渲染是兼容
            // 性默认。FemtoVG / Skia 在「设置 → 界面 → 渲染」里可选，
            // SLINT_BACKEND=winit-skia 环境变量诊断覆盖仍然生效且优先。
            //
            // IME 策略：这里刻意**不调** ImmDisableIME —— 中文合成输入走
            // terminal_view.slint 的隐藏 ime-input TextInput，提交文本经
            // edited 回调转发给 PTY；vim/Shift 副作用由 app::on_send_key
            // 的 C0-marker + Backspace 三层过滤处理。禁用 IME 会直接让
            // 中文输入完全不可用（历史事故，勿再引入）。
            app::run(app::launch::parse(&args))
        }
        cli::StartMode::Version => unreachable!("handled above"),
    }
}
