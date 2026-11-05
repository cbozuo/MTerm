//! Ping 模式：远端可达性探测——SSH 到探测源执行 ping 并解析时延样本。
//!
//! 与本地引擎同产出形状（`Vec<Option<f64>>`，None=丢包），桥层可无差别套用
//! mode 判定/渲染。复用 `crate::ssh::execute_command`（fail-closed）。

use crate::config::Session;

/// 从探测源主机 ping 目标，返回逐包时延样本（None=丢包）。
/// 命令回退链：Linux/macOS `ping -c 4` → Windows `ping -n 4`（`-c` 在
/// Windows 非法、退出非零，`||` 短路）。target 已过 `looks_like_host`
/// 校验（字母数字 `.` `-` `_`），无 shell 注入面。
pub(crate) async fn ping_remote(
    session: Session,
    jumps: Vec<Session>,
    target: &str,
) -> Result<Vec<Option<f64>>, String> {
    let out = crate::ssh::execute_command(
        session,
        jumps,
        &format!("ping -c 4 {target} || ping -n 4 {target}"),
        // 目标不可达时 iputils 最后一包要等满宽限期（~10s），加上连接建立，
        // 12s 会拿到半截输出——放宽到 25s（并发限 4，手动探测可接受）
        std::time::Duration::from_secs(25),
        64 * 1024,
    )
    .await
    .map_err(|e| format!("{:#}", e))?;
    match parse_ping_output(&out.stdout) {
        Some(samples) => Ok(samples),
        // 无可解析输出：多半是 DNS 解析失败（信息在 stderr）或意外语言
        None => {
            let hint: String = out
                .stderr
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .chars()
                .take(80)
                .collect();
            if hint.is_empty() {
                Err("无法解析 ping 输出".into())
            } else {
                Err(format!("目标不可达：{hint}"))
            }
        }
    }
}

/// 解析 ping 输出为逐包样本：`time=/时间=/time<` 每处一个 Some；超时标记
/// 行每行一个 None；两类都没有（DNS 失败/语言意外）→ None。
fn parse_ping_output(stdout: &str) -> Option<Vec<Option<f64>>> {
    static TIME_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let time_re = TIME_RE.get_or_init(|| {
        // time=0.032 ms / time<1ms / 时间=33ms / TIME 100 ms 内均取首个数值
        regex::Regex::new(r"(?i)(?:time|时间)\s*[=<>=]\s*([0-9.]+)\s*ms").unwrap()
    });
    let mut samples: Vec<Option<f64>> = Vec::new();
    for line in stdout.lines() {
        if let Some(caps) = time_re.captures(line) {
            if let Ok(v) = caps[1].parse::<f64>() {
                samples.push(Some(v));
                continue;
            }
        }
        let low = line.to_lowercase();
        // 汇总行兜底（全丢时无逐包行）：中英 Linux/Windows 各变体 + 明示 0 接收
        let lost = ["timed out", "request timeout", "unreachable", "expired",
            "100% packet loss", "100% loss", "100% 丢失", "100% 丢包",
            "0 received", "0 接收", "超时", "无法访问"]
            .iter()
            .any(|m| low.contains(m));
        if lost {
            samples.push(None);
        }
    }
    if samples.is_empty() {
        None
    } else {
        Some(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ping_variants() {
        // Linux 英文（含丢包标记行）
        let linux = "\
PING www.example.com (93.184.216.34) 56(84) bytes of data.\n\
64 bytes from 93.184.216.34: icmp_seq=1 ttl=56 time=1.23 ms\n\
64 bytes from 93.184.216.34: icmp_seq=2 ttl=56 time=1.50 ms\n\
From 192.168.1.1 icmp_seq=3 Destination Host Unreachable\n\
64 bytes from 93.184.216.34: icmp_seq=4 ttl=56 time=2.05 ms\n";
        let s = parse_ping_output(linux).unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], Some(1.23));
        assert_eq!(s[2], None);
        assert_eq!(s[3], Some(2.05));

        // 中文 Windows（时间=33ms / 请求超时）
        let win = "\
正在 Ping www.example.com [93.184.216.34] 具有 32 字节的数据:\n\
来自 93.184.216.34 的回复: 字节=32 时间=33ms TTL=54\n\
请求超时。\n\
来自 93.184.216.34 的回复: 字节=32 时间=41ms TTL=54\n\
请求超时。\n";
        let s = parse_ping_output(win).unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], Some(33.0));
        assert_eq!(s[1], None);
        assert_eq!(s[2], Some(41.0));
        assert_eq!(s[3], None);

        // 全超时（Linux 汇总行兜底出 None，offline 判定成立）
        let all_lost = "\
PING 10.255.255.1 (10.255.255.1) 56(84) bytes of data.\n\n\
--- 10.255.255.1 ping statistics ---\n\
4 packets transmitted, 0 received, 100% packet loss, time 3024ms\n";
        let s = parse_ping_output(all_lost).unwrap();
        assert!(s.iter().all(|v| v.is_none()));

        // 中文 Linux 汇总（「丢包」不是「丢失」——线上踩过的标记缺口）
        let all_lost_zh = "发出 4 个包，收到 0 个，100% 丢包，时间 3024ms\n";
        assert!(parse_ping_output(all_lost_zh).is_some());

        // DNS 失败（无可解析行）→ None，调用方转 Err 带出 stderr
        assert!(parse_ping_output("ping: nosuchhost: Name or service not known").is_none());

        // time<1ms 紧凑格式
        let fast = "Reply from 127.0.0.1: bytes=32 time<1ms TTL=128\n";
        let s = parse_ping_output(fast).unwrap();
        assert_eq!(s[0], Some(1.0));
    }
}
