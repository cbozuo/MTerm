// 非 Windows：FreeRDP 客户端启动 + 凭据 stdin 喂入 + 早退检测。
use super::{Account, freerdp};
use crate::i18n::t;
use std::process::Command;


/// Feed the credentials to a running client over its stdin: a pipe cannot be
/// read out of the process list, and nothing is written to disk.
fn write_credentials(child: &mut std::process::Child, account: &Account<'_>) {
    use std::io::Write as _;
    let Some(mut stdin) = child.stdin.take() else {
        return;
    };
    for line in freerdp::credential_lines(account) {
        let _ = writeln!(stdin, "{line}");
    }
    let _ = stdin.flush();
}

/// Give a freshly started client a moment to fail, collecting what it printed.
///
/// A missing display, an unknown flag or a client that needs different
/// arguments makes it exit at once, and knowing that beats announcing a window
/// that never appears. `Some(stderr)` means it exited; the text is for the log,
/// not for the UI. `None` means it is still running, which is what we want.
fn early_exit(child: &mut std::process::Child) -> Option<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(700);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            _ => return None,
        }
    }
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        use std::io::Read as _;
        let _ = pipe.take(4096).read_to_string(&mut stderr);
    }
    Some(stderr.trim().to_string())
}

/// Linux / macOS / BSD: drive FreeRDP's `xfreerdp3` / `xfreerdp`.
pub(super) fn start_client(account: &Account<'_>, _session_id: &str) -> Result<String, String> {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err(t(
            "当前环境没有图形显示（DISPLAY / WAYLAND_DISPLAY），无法打开远程桌面窗口",
            "no graphical display here (DISPLAY / WAYLAND_DISPLAY) — cannot open a remote desktop window",
        )
        .to_string());
    }
    for client in freerdp::CLIENTS {
        // Unknown version (or a client that does not answer): assume the older
        // dialect, whose flag is accepted by both.
        let dialect = freerdp::probe_dialect(client).unwrap_or(freerdp::Dialect::V2);
        let mut command = Command::new(client);
        command
            .args(freerdp::args(account, dialect))
            // An AppImage would otherwise hand its own library directory down to
            // the client, which then loads our bundled glib/OpenSSL and breaks.
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .env_remove("APPDIR")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        let Ok(mut child) = command.spawn() else {
            continue;
        };
        write_credentials(&mut child, account);
        if let Some(stderr) = early_exit(&mut child) {
            // The client's own words go to the log, not into the UI: FreeRDP
            // prefixes every line with a timestamp and an internal module name,
            // which tells the user nothing. Everything else about the failure
            // (that it exited, and which binary it was) still reaches them, so a
            // broken connection is never announced as a started session.
            tracing::warn!("{client} exited right away: {stderr}");
            return Err(format!(
                "{} ({client})",
                t(
                    "RDP 客户端启动失败，详情见日志",
                    "the RDP client failed to start — see the log for details"
                )
            ));
        }
        // Detached reaper: `Child` does not wait on drop, so without this every
        // connect would leave a zombie behind for as long as MTerm runs.
        // The stderr pipe is drained in the same thread — a full pipe would
        // block the client.
        let stderr = child.stderr.take();
        std::thread::spawn(move || {
            if let Some(mut pipe) = stderr {
                let _ = std::io::copy(&mut pipe, &mut std::io::sink());
            }
            let _ = child.wait();
        });
        return Ok(format!("{}:{} ({client})", account.host, account.port));
    }
    Err(if freerdp::in_flatpak_sandbox() {
        t(
            "Flatpak 版自带 FreeRDP 客户端，找不到说明安装损坏，请重新安装 MTerm 的 Flatpak 包",
            "the Flatpak bundle ships its own FreeRDP client — it is missing, so please reinstall the MTerm Flatpak",
        )
    } else {
        t(
            "未找到 FreeRDP 客户端；请先安装：Debian/Ubuntu `sudo apt install freerdp3-x11`，Fedora `sudo dnf install freerdp`，Arch `sudo pacman -S freerdp`，macOS `brew install freerdp`",
            "no FreeRDP client found; install it first: Debian/Ubuntu `sudo apt install freerdp3-x11`, Fedora `sudo dnf install freerdp`, Arch `sudo pacman -S freerdp`, macOS `brew install freerdp`",
        )
    }
    .to_string())
}
