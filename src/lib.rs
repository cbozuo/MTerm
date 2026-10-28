//! MTerm 库 crate（lib + bin 分离的"lib"侧）。
//!
//! 顶层模块清单与启动流程骨架集中在本文件；二进制目标 `MTerm`
//! （`src/main.rs`）只做全局分配器注册 + 转发到 [`run`]。
//! 集成测试与 `cargo doc` 通过 `use mterm::…` 直接访问本 crate。

pub mod allocator;
pub mod app;
pub mod automation;
pub mod cli;
// (#cmd-hint 2026-10-27) 命令提示框领域层（阶段判定 / 候选生成 / 内置命令表）。
pub mod command_hint;
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

/// 测试用的可写临时目录。
///
/// 为什么不能直接用 `std::env::temp_dir()`：它只回答"系统认为临时目录在哪"，
/// 不回答"我们能不能往那儿写"。两者在很多环境里并不一致 —— 沙箱化构建、
/// 受限服务账户、策略收紧的 Windows 上，`%TEMP%` 可读但不可写，连
/// `create_dir_all` 都返回 `PermissionDenied`。于是凡是 `temp_dir().join(…)`
/// 再 `.unwrap()` 的测试都会红，而失败信息看起来像被测代码坏了（实际被测的
/// 只是写文件那一步），排查成本很高。
///
/// 这里按"先试再用"挑一个真能写的根：
///   1. `MTerm_TEST_TMP`（显式覆盖，CI 上最省事）；
///   2. `temp_dir()`，绝大多数机器走这条；
///   3. 工作区内的 `target/test-tmp`，沙箱里必然可写（cargo 本来就在写它）。
///
/// 三个都不可写才 panic —— 那是环境彻底坏掉，而不是某条断言不成立。
///
/// 仅 `cfg(test)`：生产路径（`data_dir`、`log_dir`、外部编辑器暂存等）各有自己
/// 的目录决议，不走这里，那是产品行为而不是测试便利。
#[cfg(test)]
pub(crate) fn test_tmp_root() -> std::path::PathBuf {
    use std::sync::OnceLock;
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Some(dir) = std::env::var_os("MTerm_TEST_TMP") {
            candidates.push(std::path::PathBuf::from(dir));
        }
        candidates.push(std::env::temp_dir());
        // `env!("CARGO_MANIFEST_DIR")` 是编译期常量，测试二进制在哪跑都能定位
        // 到工作区；`target/` 下的写入 cargo 自己就在做，权限必然够。
        candidates.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp"));

        for dir in candidates {
            // 真的写一次才算数：`create_dir_all` 成功也可能是个只读挂载点。
            let probe = dir.join(format!(".mterm-tmp-probe-{}", std::process::id()));
            if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&probe, b"").is_ok() {
                let _ = std::fs::remove_file(&probe);
                return dir;
            }
        }
        panic!(
            "no writable temp directory for tests: tried MTerm_TEST_TMP, %TEMP%, target/test-tmp"
        );
    })
    .clone()
}

/// `<test_tmp_root>/<name>-<uuid>`：测试专属的临时子目录。
///
/// 每次都新建、默认带随机后缀，所以并发跑的同名测试互不干扰；调用方负责用完
/// 删（`remove_dir_all` 失败可以忽略 —— 临时目录，不是产品状态的来源）。
#[cfg(test)]
pub(crate) fn test_tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = test_tmp_root().join(format!("{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("test temp dir");
    dir
}

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
