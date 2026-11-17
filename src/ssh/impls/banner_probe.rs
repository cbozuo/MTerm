//! 探查「登录 banner / MOTD 客户端到底收到了什么」(#ssh-login-banner 2026-11-17)。
//!
//! 用户报:iShellPro 连 202 会显示欢迎信息(allowed users / Last login / System
//! information…),MTerm 不显示。服务器侧已确认(`Banner /etc/issue.net` +
//! `session optional pam_motd.so` + `/etc/profile.d/system-info.sh`),这些字节
//! 都会写到 pty 上 —— 所以问题在客户端这一侧:要么没收到,要么被吞了。
//!
//! 本探针走**和应用完全同一个** `spawn_session`,把每个 `SessionEvent::Output`
//! 原文打出来(带毫秒时间戳和 CR/LF 标记),从而分辨:
//!   * 一个字节都没收到 → 服务端对我们这个会话没有输出(pty/login-shell 差异);
//!   * 收到了 → 被注入/抑制逻辑吃掉(那就在 client 里修)。
//!
//! 跑法(默认忽略,因为它要连真服务器):
//! ```text
//! set MTERM_PROBE_SESSION=<session-id 或会话名>
//! cargo test --lib ssh::impls::banner_probe -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use super::ssh::spawn_session;
use crate::config::ConfigStore;

/// 把 CR/LF/ESC 变成可见标记:chunk 边界和转义序列正是这个探针要看的东西。
fn escape_for_display(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '\r' => out.push_str("<CR>"),
            '\n' => out.push_str("<LF>\n"),
            '\x1b' => out.push_str("<ESC>"),
            '\x07' => out.push_str("<BEL>"),
            other => out.push(other),
        }
    }
    out
}

#[test]
#[ignore = "needs a live SSH server; set MTERM_PROBE_SESSION and run with --ignored"]
fn probe_login_banner() {
    let key = std::env::var("MTERM_PROBE_SESSION").unwrap_or_else(|_| "202-2".to_string());
    let store = ConfigStore::load().expect("load store");
    let base = store
        .get(&key)
        .or_else(|| store.sessions().iter().find(|s| s.name == key))
        .unwrap_or_else(|| panic!("no session with id/name {key}"))
        .clone();

    // 两趟对比:关掉 shell 集成 = 服务端的**原始**字节流(没有被注入/抑制吞过);
    // 按用户配置 = 应用实际看到的东西。两趟之差就是客户端吃掉的量。
    let mut raw = base.clone();
    raw.disable_shell_integration = true;
    dump_pass("禁用 shell 集成(服务端原始流)", raw);
    dump_pass("按会话配置(应用实际路径)", base);
}

fn dump_pass(label: &str, session: crate::config::Session) {
    println!("\n======== {label} ========");
    println!(
        "session {} -> {}@{}:{} encoding={} integration_disabled={}",
        session.name,
        session.user,
        session.host,
        session.port,
        session.encoding,
        session.disable_shell_integration
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let (_handle, mut events) = spawn_session(
        runtime.handle(),
        "probe-tab".to_string(),
        session,
        Vec::new(),
        120,
        40,
    );

    let started = Instant::now();
    let mut chunks = 0usize;
    let mut bytes = 0usize;
    while started.elapsed() < Duration::from_secs(6) {
        match events.try_recv() {
            Ok(crate::ssh::SessionEvent::Output(text)) => {
                chunks += 1;
                bytes += text.len();
                println!(
                    "\n--- Output #{} @ {:>6} ms, {} bytes ---\n{}",
                    chunks,
                    started.elapsed().as_millis(),
                    text.len(),
                    escape_for_display(&text)
                );
            }
            Ok(_other) => {}
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
    println!("\nchunks={chunks} bytes={bytes}");
}
