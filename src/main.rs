//! MTerm 二进制入口（薄壳）。
//!
//! 采用主流的 lib + bin 分离：全部逻辑在 `mterm` 库 crate（`src/lib.rs`），
//! 本文件只做三件事 —— Windows 子系统声明、全局分配器注册、转发到
//! `mterm::run()`。加新启动模式 / 新初始化步骤请改 `src/lib.rs::run()`
//! 与各业务模块，不要往这里塞。

// release 构建隐藏控制台窗口（debug 保留，方便看 panic 输出）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 全局分配器必须在 bin crate 声明（写在 lib 依赖里不会传递到最终二进制）；
/// 具体类型按平台在 `mterm::allocator` 里选：Windows=mimalloc、
/// Unix 系=jemalloc、其它=System。
#[global_allocator]
static GLOBAL: mterm::allocator::Allocator = mterm::allocator::Allocator;

fn main() -> anyhow::Result<()> {
    mterm::run()
}
