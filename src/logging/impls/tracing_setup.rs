//! 进程级 tracing 初始化（原在 main.rs，#lib-bin-split 迁入）。

use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

use super::{path, CappedFile, CappedWriter};

/// 双层输出：
/// * stderr 层跟随 RUST_LOG（默认 info），开发期实时可见；
/// * 文件层固定 WARN+，写入 50 MiB 封顶、写满自动回卷的 `error.log`，
///   用户不用设任何环境变量就能直接把日志文件发来做诊断（#86）。
pub(crate) fn init_tracing() {
    /// 压制第三方经 `log` → tracing 转发的噪音：ICU4X 的数据错误告警
    /// （icu_provider 依赖链）与 fontdb 对"畸形但无害"字体的误报（如
    /// Windows 的 mstmc.ttf）。fontdb 保留 error 级，真实失败仍要冒出来。
    fn quiet_noise(mut f: EnvFilter) -> EnvFilter {
        for d in [
            "icu_provider=off",
            "icu_segmenter=off",
            "icu_normalizer=off",
            "fontdb=error",
        ] {
            if let Ok(dir) = d.parse() {
                f = f.add_directive(dir);
            }
        }
        f
    }

    let env_filter =
        quiet_noise(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);

    let file_layer = path()
        .and_then(|p| CappedFile::open(p, 50 * 1024 * 1024).ok())
        .map(|cf| {
            fmt::layer()
                .with_ansi(false)
                .with_writer(CappedWriter::new(cf))
                .with_filter(quiet_noise(EnvFilter::new("warn")))
        });

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();
}
