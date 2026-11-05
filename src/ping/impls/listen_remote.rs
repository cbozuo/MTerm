//! 端口模式：远端监听端口快照——SSH 到探测源执行 ss/netstat 并解析。
//!
//! 复用 `crate::ssh::execute_command`（一次性 exec；代理/跳板/认证路径与
//! 交互会话全同，未知主机密钥/缺凭证 fail-closed 返回 Err，无 UI 提示通道）。
//! 进程名是尽力解析：非 root 的 ss 只见自己进程，其余 name 显示 "-"。

use super::model::ListenEntry;
use crate::config::Session;

/// 探测源主机的监听 TCP 端口表（按端口升序、同端口去重）。
/// 命令回退链：Linux ss → Linux netstat → Windows netstat -ano → macOS/BSD
/// netstat -an。不带 `2>/dev/null`——execute_command 分开收集 stderr，前级
/// 失败输出直接丢弃；`||` 靠退出码短路（Windows cmd 同样支持 `||`）。
pub(crate) async fn listen_table_remote(
    session: Session,
    jumps: Vec<Session>,
) -> Result<Vec<ListenEntry>, String> {
    let out = crate::ssh::execute_command(
        session,
        jumps,
        "ss -tlnp || netstat -tlnp || netstat -ano || netstat -an",
        std::time::Duration::from_secs(10),
        256 * 1024,
    )
    .await
    .map_err(|e| format!("{:#}", e))?;
    Ok(parse_listen_output(&out.stdout))
}

/// 解析四种 ss/netstat 输出为 ListenEntry 列表。
/// 行结构（状态列的位置是唯一稳定的区分点）：
///   ss:      `LISTEN 0 128 0.0.0.0:22 0.0.0.0:* users:(("sshd",pid=987,fd=3))`
///   linux:   `tcp 0 0 0.0.0.0:22 0.0.0.0:* LISTEN 987/sshd`
///   windows: `TCP 0.0.0.0:80 0.0.0.0:0 LISTENING 4`
///   macos:   `tcp4 0 0 *.80 *.* LISTEN`
pub(crate) fn parse_listen_output(stdout: &str) -> Vec<ListenEntry> {
    let mut out: Vec<ListenEntry> = Vec::new();
    for line in stdout.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        // 状态列（ss 在行首，netstat 系在尾部区域）
        let Some(state_idx) = tokens
            .iter()
            .position(|t| *t == "LISTEN" || *t == "LISTENING")
        else {
            continue;
        };
        // 本地地址：ss（state 行首）固定第 4 列；netstat 系在 state 前第 2 列
        let local = if state_idx == 0 {
            tokens.get(3)
        } else {
            tokens.get(state_idx - 2)
        };
        let Some(local) = local else { continue };
        // 端口：IPv4/IPv6 取最后一个 `:` 后段（[::]:22 同样成立）；macOS `*.80`
        // 无冒号，取最后一个 `.` 后段
        let port_text = if local.contains(':') {
            local.rsplit(':').next()
        } else if local.contains('.') {
            local.rsplit('.').next()
        } else {
            None
        };
        let Some(port_text) = port_text.and_then(|p| p.trim_end_matches(']').parse::<u16>().ok())
        else {
            continue;
        };
        let (pid, name) = if state_idx == 0 {
            // ss：users:(("sshd",pid=987,fd=3))——取首个引号名与 pid=
            let users = tokens.iter().rev().find(|t| t.starts_with("users:"));
            match users {
                Some(u) => {
                    let pid = u
                        .split("pid=")
                        .nth(1)
                        .and_then(|p| {
                            p.split(|c| c == ',' || c == ')').next()
                        })
                        .and_then(|p| p.trim().parse::<u32>().ok())
                        .unwrap_or(0);
                    let name = u.split('"').nth(1).unwrap_or("-").to_string();
                    (pid, name)
                }
                None => (0, "-".to_string()),
            }
        } else {
            match tokens.get(state_idx + 1) {
                // Linux：`987/sshd`
                Some(t) if t.contains('/') => {
                    let mut it = t.splitn(2, '/');
                    let pid = it.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
                    (pid, it.next().unwrap_or("-").to_string())
                }
                // Windows：裸 pid 数字列
                Some(t) if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) => {
                    (t.parse::<u32>().unwrap_or(0), "-".to_string())
                }
                // macOS（不带 -v）无进程列
                _ => (0, "-".to_string()),
            }
        };
        let name = if name.is_empty() { "-".to_string() } else { name };
        out.push(ListenEntry {
            port: port_text,
            name,
            pid,
            addr: (*local).to_string(),
        });
    }
    out.sort_by_key(|e| e.port);
    out.dedup_by_key(|e| e.port);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_listen_four_formats() {
        // ss -tlnp（Linux，含 IPv6 与 users 进程段）
        let ss = "\
State   Recv-Q  Send-Q   Local Address:Port   Peer Address:Port  Process\n\
LISTEN  0       128          0.0.0.0:22           0.0.0.0:*      users:((\"sshd\",pid=987,fd=3))\n\
LISTEN  0       100          [::]:3306            [::]:*         users:((\"mysqld\",pid=55,fd=31))\n";
        let rows = parse_listen_output(ss);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].port, 22);
        assert_eq!(rows[0].pid, 987);
        assert_eq!(rows[0].name, "sshd");
        assert_eq!(rows[0].addr, "0.0.0.0:22");
        assert_eq!(rows[1].port, 3306);
        assert_eq!(rows[1].addr, "[::]:3306");

        // Linux netstat -tlnp（root 可见 pid/name）
        let linux = "\
Active Internet connections (only servers)\n\
tcp        0      0 0.0.0.0:22        0.0.0.0:*       LISTEN      987/sshd\n\
tcp6       0      0 :::80             :::*            LISTEN      1001/nginx\n";
        let rows = parse_listen_output(linux);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].port, 22);
        assert_eq!(rows[0].name, "sshd");
        assert_eq!(rows[1].port, 80);

        // Windows netstat -ano（裸 pid，无进程名）
        let win = "\
  协议  本地地址          外部地址        状态           PID\n\
  TCP    0.0.0.0:80            0.0.0.0:0              LISTENING       4\n\
  TCP    [::]:445              [::]:0                 LISTENING    1968\n";
        let rows = parse_listen_output(win);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].port, 80);
        assert_eq!(rows[0].pid, 4);
        assert_eq!(rows[0].name, "-");
        assert_eq!(rows[1].port, 445);
        assert_eq!(rows[1].pid, 1968);

        // macOS netstat -an（`*.端口`，无进程列）
        let mac = "\
tcp4       0      0  *.8080            *.*        LISTEN\n\
tcp6       0      0  *.22              *.*        LISTEN\n";
        let rows = parse_listen_output(mac);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].port, 22);
        assert_eq!(rows[0].pid, 0);
        assert_eq!(rows[0].name, "-");
        assert_eq!(rows[1].port, 8080);

        // 混入噪声行 / 空串 / 同端口去重（排序升序）
        let mixed = "LISTEN 0 128 0.0.0.0:8080 0.0.0.0:* users:((\"x\",pid=1,fd=1))\n\nnoise line here\nLISTEN 0 128 [::]:8080 [::]:* users:((\"x\",pid=1,fd=2))\n";
        let rows = parse_listen_output(mixed);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].port, 8080);
    }
}
