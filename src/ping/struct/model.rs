//! Ping 弹框的**数据类型**：目标态 / 弹框会话态 / 草稿与主机名规则。
//!
//! 纯数据 + 只依赖自身字段的小逻辑（统计、tone、worst 选取）。
//! 不依赖 Slint、不依赖 app 层 —— 分层说明见 `src/ping/mod.rs`。

use std::net::{IpAddr, ToSocketAddrs};

/// 「加入」上限，对齐高保真 handoff。
pub(crate) const MAX_TARGETS: usize = 12;
pub(crate) const AUTO_INTERVAL_MS: u64 = 2000;

/// 监听端口行（引擎自有类型；转 Slint `ListenRow` 在 app 桥层）。
#[derive(Clone)]
pub(crate) struct ListenEntry {
    pub(crate) port: u16,
    pub(crate) name: String,
    pub(crate) pid: u32,
    pub(crate) addr: String,
}

/// 单目标状态。
pub(crate) struct TargetState {
    /// 草稿原文（显示与移除键）。
    pub(crate) addr: String,
    /// 解析出的 IP（域名先解析；失败为 None → 状态「解析失败」）。
    pub(crate) ip: Option<IpAddr>,
    /// idle | testing | normal | degraded | offline
    pub(crate) mode: String,
    /// 最近采样（None＝丢包），滚动窗口。
    pub(crate) samples: Vec<Option<f64>>,
    pub(crate) selected: bool,
    /// 参与探测（2026-11-05 用户定稿：清单勾选框在勾/空间切换——空框=不
    /// 参与但**行保留置灰**，不再直接移除；探测集/计数/下拉勾选都以此为准）
    pub(crate) enabled: bool,
}

impl TargetState {
    pub(crate) fn new(addr: String) -> Self {
        Self { addr, ip: None, mode: "idle".into(), samples: Vec::new(), selected: false, enabled: true }
    }

    pub(crate) fn last(&self) -> Option<f64> {
        self.samples.iter().rev().flatten().copied().next()
    }

    pub(crate) fn loss_pct(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let lost = self.samples.iter().filter(|s| s.is_none()).count();
        lost as f64 / self.samples.len() as f64 * 100.0
    }    pub(crate) fn avg(&self) -> Option<f64> {
        let v: Vec<f64> = self.samples.iter().flatten().copied().collect();
        if v.is_empty() { return None; }
        Some(v.iter().sum::<f64>() / v.len() as f64)
    }

    pub(crate) fn jitter(&self) -> Option<f64> {
        let v: Vec<f64> = self.samples.iter().flatten().copied().collect();
        if v.len() < 2 { return Some(0.0); }
        let d: Vec<f64> = v.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        Some(d.iter().sum::<f64>() / d.len() as f64)
    }

    /// tone: 0 优秀 1 一般 2 超时 3 待测
    pub(crate) fn tone(&self) -> i32 {
        match self.mode.as_str() {
            "normal" => 0,
            "degraded" => 1,
            "offline" => 2,
            _ => 3,
        }
    }

    pub(crate) fn state_word(&self, lang_en: bool) -> &'static str {
        match self.mode.as_str() {
            "normal" => if lang_en { "OK" } else { "正常" },
            "degraded" => if lang_en { "Fair" } else { "弱网" },
            "offline" => if lang_en { "Timeout" } else { "超时" },
            "testing" => if lang_en { "Testing" } else { "测试中" },
            _ => if lang_en { "Idle" } else { "待测" },
        }
    }
}

/// 采样 → 模式判定（本地 ICMP / 远端 ping 共用同一规则）：空或全丢=offline；
/// 有丢包或最大 RTT ≥100ms=degraded；否则 normal。
pub(crate) fn mode_from_samples(samples: &[Option<f64>]) -> String {
    if samples.is_empty() || samples.iter().all(|s| s.is_none()) {
        return "offline".into();
    }
    let loss = samples.iter().filter(|s| s.is_none()).count() as f64
        / samples.len() as f64 * 100.0;
    let max_rtt = samples.iter().flatten().copied().fold(0.0f64, f64::max);
    if loss > 0.0 || max_rtt >= 100.0 { "degraded".into() } else { "normal".into() }
}

/// 弹框会话态（install 时创建，Arc<Mutex> 持有；锁全部短作用域——
/// std Mutex 不可重入，UI 线程持锁期间二次 lock 即整窗「未响应」）。
pub(crate) struct PingState {
    pub(crate) targets: Vec<TargetState>,
    pub(crate) method: String, // icmp | port
    pub(crate) source: String, // local | 会话 id
    pub(crate) auto_on: bool,
    /// 探测轮次在飞（2026-11-04 用户定稿：只有点「探测」才触发；连续 Ping
    /// 的每 2s 轮次也算一轮）。按钮态/防重入用它，行不再集体翻「测试中」
    /// （有数据的目标轮询期间保持原状——清单不再每 2s 跳动）。
    pub(crate) round_active: bool,
    pub(crate) listen_rows: Vec<ListenEntry>,
    pub(crate) listen_query: String,
    pub(crate) listen_sel: Option<u16>,
    pub(crate) lang_en: bool,
    /// 源下拉全量（key, 显示名, tab 是否连接, 卡片字段×6, **exec**=可执行命令
    /// ——本机/SSH 会话真）——本机 + 已打开 tab 的会话（2026-11-04 定稿）；
    /// 过滤搜索在 Rust 重建模型（Slint 无 filter）。
    pub(crate) hosts_full: Vec<(String, String, bool, String, String, String, String, String, String, bool)>,}

impl PingState {
    pub(crate) fn new() -> Self {
        Self {
            targets: Vec::new(),
            method: "icmp".into(),
            source: "local".into(),
            auto_on: false,
            round_active: false,
            listen_rows: Vec::new(),
            listen_query: String::new(),
            listen_sel: None,
            lang_en: false,
            hosts_full: Vec::new(),
        }
    }

    pub(crate) fn selected_addr(&self) -> Option<&str> {
        self.targets.iter().find(|t| t.selected).map(|t| t.addr.as_str())
    }
}

/// 解析草稿 → 去重后的目标原文列表（分隔符：空白/逗号/分号/中英标点）。
pub(crate) fn parse_draft(draft: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in draft.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '；' || c == '、') {
        let p = part.trim();
        if p.is_empty() || out.iter().any(|o| o.eq_ignore_ascii_case(p)) { continue; }
        out.push(p.to_string());
    }
    out
}

/// 域名 / IPv4 / IPv6 的宽松校验（IPv6 只要形似）。
fn classify(s: &str) -> Option<IpAddr> {
    if let Ok(ip) = s.parse::<IpAddr>() { return Some(ip); }
    None
}

pub(crate) fn looks_like_host(s: &str) -> bool {
    if s.parse::<IpAddr>().is_ok() { return true; }
    // 域名：字母数字连字符点分，至少一个点
    !s.is_empty() && s.len() <= 253
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && s.contains('.')
}

/// DNS 解析（阻塞调用，仅在探测线程内执行）。
pub(crate) fn resolve(host: &str) -> Option<IpAddr> {
    if let Some(ip) = classify(host) { return Some(ip); }
    if !looks_like_host(host) { return None; }
    let addrs: Vec<_> = (host, 0u16).to_socket_addrs().ok()?.collect();
    addrs.into_iter().next().map(|a| a.ip())
}
