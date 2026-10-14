//! 进程级 rustls CryptoProvider 安装（原在 main.rs，#lib-bin-split 迁入）。
//!
//! 本二进制同时启用了 rustls 的两个 provider feature：
//! * `ring` —— 本 crate 与 `ureq`（WebDAV）要求；
//! * `aws_lc_rs` —— `ironrdp → ironrdp-client → ironrdp-tls` 要求。
//!
//! 两个都在时 rustls 拒绝自动选择：`from_crate_features()` 返回 `None`，
//! 任何 `ClientConfig::builder()` 直接 panic。release 配置是
//! `panic = "abort"`，实测表现为"点 RDP 会话 → 无窗口无日志、整个应用
//! 瞬间消失"（这就是当初的报障形态）。

/// 把 `ring` 装为进程级 provider，必须早于任何 `ClientConfig::builder()`。
///
/// 选 `ring` 而非 `aws_lc_rs`：WebDAV 路径已在用 `ring`
/// （`builder_with_provider`），保持全进程单一 provider。
///
/// 装在全局而不是只改自己的调用点：`ironrdp-tls` 内部也会
/// `ClientConfig::builder()`，那条链在我们够不着的依赖里，只有
/// `install_default()` 能同时覆盖（含 `tokio-rustls` / `ureq` /
/// `rdp::embedded::verifier` 的证书探测）。
///
/// 失败刻意忽略：`install_default` 唯一的失败原因是"已经装过"，
/// 而那正是本函数要达成的状态（main 与测试都可能到达这里）。
pub(crate) fn install_rustls_crypto_provider() {
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::debug!("a rustls crypto provider was already installed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// provider 没装的话 `ClientConfig::builder()` 直接 panic，而两条 TLS
    /// 路径都不指定 provider、只走 `builder()`。
    ///
    /// 测试不经 `run()`，所以必须自己调安装 —— 同理，任何构建 TLS 配置
    /// 的测试都要显式确保 provider 已装。
    #[test]
    fn rustls_gets_a_default_crypto_provider() {
        install_rustls_crypto_provider();
        assert!(
            rustls::crypto::CryptoProvider::get_default().is_some(),
            "no provider was installed, so any ClientConfig::builder() would panic"
        );
        // 能走到下一行就是断言成功：builder() 正是缺 provider 时会 panic 的调用。
        let _ = rustls::ClientConfig::builder();
        // 第二次安装必须安静地无操作，而不是把第一次的撤销掉。
        install_rustls_crypto_provider();
        let _ = rustls::ClientConfig::builder();
    }
}
