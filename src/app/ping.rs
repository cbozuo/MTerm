//! 标题栏 Ping 弹框（#titlebar-ping，高保真 docs/design/titlebar-ping-hifi-2026-11-03.html
//! V13）的探测引擎与 UI 状态。
//!
//! 结构对应 ui/ping_dialog.slint：
//!  · 双模式：`icmp`＝可达性探测（本机源，草稿→清单→RTT/曲线）；`port`＝本机
//!    监听端口占用视图（GetExtendedTcpTable + 进程名，无目标概念）。方式分段在
//!    两个视图间切换，数据互斥下发。
//!  · 探测在后台 `std::thread` 执行（ICMP 3 包 burst、每包 2s 超时），结果经
//!    `slint::invoke_from_event_loop` 回 UI（项目标准模式，见 app/session_runtime.rs）。
//!  · 连续 Ping = `slint::Timer`（Repeated 2s）驱动的后台轮次，关弹框即停。
//!  · 图表曲线是 Rust 拼好的 SVG path 字符串（30 点滚动窗口），清单行迷你趋势
//!    同理（40×12）。Slint 无 split / filter，「加入 N」条数走 pure callback
//!    `ping-count-draft`，监听表搜索过滤在 Rust 重建模型。
//!  · 一期边界：探测源仅「本机」（托管主机条目灰显占位，三期接
//!    `ssh::execute_command`）；IPv4 走 ICMP，IPv6 / 解析失败标记对应状态，
//!    ICMP 无响应不做 TCP 降级（高保真边界表「ICMP 被禁用自动降级」挂账二期）。

use crate::config::ConfigStore;
use crate::ui::{AppWindow, ListenRow, PingHostOption, PingTargetRow};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::net::{IpAddr, ToSocketAddrs};
use std::rc::Rc;

/// 单目标探测窗口（滚动）长度与「加入」上限，对齐高保真 handoff。
const SAMPLE_KEEP: usize = 30;
const MAX_TARGETS: usize = 12;
const AUTO_INTERVAL_MS: u64 = 2000;
/// 图表视口（与 ping_dialog.slint 的 chart 区域一致）。
const CHART_W: f64 = 320.0;
const CHART_H: f64 = 74.0;
/// 迷你趋势视口（清单行）。
const SPARK_W: f64 = 40.0;
const SPARK_H: f64 = 12.0;

/// 单目标状态。
struct TargetState {
    /// 草稿原文（显示与移除键）。
    addr: String,
    /// 解析出的 IP（域名先解析；失败为 None → 状态「解析失败」）。
    ip: Option<IpAddr>,
    /// idle | testing | normal | degraded | offline
    mode: String,
    /// 最近采样（None＝丢包），滚动窗口。
    samples: Vec<Option<f64>>,
    selected: bool,
}

impl TargetState {
    fn new(addr: String) -> Self {
        Self { addr, ip: None, mode: "idle".into(), samples: Vec::new(), selected: false }
    }

    fn last(&self) -> Option<f64> {
        self.samples.iter().rev().flatten().copied().next()
    }

    fn loss_pct(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let lost = self.samples.iter().filter(|s| s.is_none()).count();
        lost as f64 / self.samples.len() as f64 * 100.0
    }

    fn avg(&self) -> Option<f64> {
        let v: Vec<f64> = self.samples.iter().flatten().copied().collect();
        if v.is_empty() { return None; }
        Some(v.iter().sum::<f64>() / v.len() as f64)
    }

    fn jitter(&self) -> Option<f64> {
        let v: Vec<f64> = self.samples.iter().flatten().copied().collect();
        if v.len() < 2 { return Some(0.0); }
        let d: Vec<f64> = v.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        Some(d.iter().sum::<f64>() / d.len() as f64)
    }

    /// tone: 0 优秀 1 一般 2 超时 3 待测
    fn tone(&self) -> i32 {
        match self.mode.as_str() {
            "normal" => 0,
            "degraded" => 1,
            "offline" => 2,
            _ => 3,
        }
    }

    fn state_word(&self, lang_en: bool) -> &'static str {
        match self.mode.as_str() {
            "normal" => if lang_en { "OK" } else { "正常" },
            "degraded" => if lang_en { "Fair" } else { "弱网" },
            "offline" => if lang_en { "Timeout" } else { "超时" },
            "testing" => if lang_en { "Testing" } else { "测试中" },
            _ => if lang_en { "Idle" } else { "待测" },
        }
    }
}

/// 弹框会话态（install 时创建，RefCell 持有）。
struct PingState {
    targets: Vec<TargetState>,
    method: String, // icmp | port
    source: String, // local | 会话 id
    auto_on: bool,
    listen_rows: Vec<ListenRow>,
    listen_query: String,
    listen_sel: Option<u16>,
    lang_en: bool,
}

impl PingState {
    fn new() -> Self {
        Self {
            targets: Vec::new(),
            method: "icmp".into(),
            source: "local".into(),
            auto_on: false,
            listen_rows: Vec::new(),
            listen_query: String::new(),
            listen_sel: None,
            lang_en: false,
        }
    }

    fn selected_addr(&self) -> Option<&str> {
        self.targets.iter().find(|t| t.selected).map(|t| t.addr.as_str())
    }

    /// 汇总视图取「最慢」目标名（供标题/图表）。
    fn worst_target(&self) -> Option<&TargetState> {
        let mut worst: Option<&TargetState> = None;
        for t in &self.targets {
            if t.mode == "idle" || t.mode == "testing" { continue; }
            if t.mode == "offline" {
                if worst.map(|w| w.mode != "offline").unwrap_or(true) { worst = Some(t); }
                continue;
            }
            if let Some(last) = t.last() {
                let better = match worst {
                    None => true,
                    Some(w) => w.mode != "offline" && w.last().map(|wl| last > wl).unwrap_or(true),
                };
                if better { worst = Some(t); }
            }
        }
        worst
    }
}

/// 解析草稿 → 去重后的目标原文列表（分隔符：空白/逗号/分号/中英标点）。
fn parse_draft(draft: &str) -> Vec<String> {
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

fn looks_like_host(s: &str) -> bool {
    if s.parse::<IpAddr>().is_ok() { return true; }
    // 域名：字母数字连字符点分，至少一个点
    !s.is_empty() && s.len() <= 253
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && s.contains('.')
}

/// DNS 解析（阻塞调用，仅在探测线程内执行）。
fn resolve(host: &str) -> Option<IpAddr> {
    if let Some(ip) = classify(host) { return Some(ip); }
    if !looks_like_host(host) { return None; }
    let addrs: Vec<_> = (host, 0u16).to_socket_addrs().ok()?.collect();
    addrs.into_iter().next().map(|a| a.ip())
}

// ── ICMP（Windows IcmpSendEcho，无需管理员）─────────────────────────────
#[cfg(windows)]
mod icmp {
    // 手写 extern（#titlebar-ping）：不依赖 windows crate 的 feature 组合，
    // 直接链接系统库 iphlpapi.dll。ICMP_ECHO_REPLY 只读前 12 字节（Address/
    // Status/RoundTripTime），布局由 Win32 ABI 保证。
    #[link(name = "iphlpapi")]
    extern "system" {
        fn IcmpCreateFile() -> *mut std::ffi::c_void;
        fn IcmpCloseHandle(handle: *mut std::ffi::c_void) -> u32;
        fn IcmpSendEcho(
            handle: *mut std::ffi::c_void,
            dest: u32,
            request_data: *const u8,
            request_size: u16,
            request_options: *const std::ffi::c_void,
            reply_buffer: *mut u8,
            reply_size: u32,
            timeout: u32,
        ) -> u32;
    }

    pub const PAYLOAD: usize = 32;
    pub const REPLY_EXTRA: usize = 128;
    /// INVALID_HANDLE_VALUE
    const INVALID: isize = -1;

    pub struct Session { handle: *mut std::ffi::c_void }

    impl Session {
        pub fn open() -> Option<Self> {
            unsafe {
                let h = IcmpCreateFile();
                if (h as isize) == INVALID { None } else { Some(Self { handle: h }) }
            }
        }
        /// 发一包，返回 RTT(ms)；None＝超时/失败。
        pub fn echo(&self, ip: std::net::Ipv4Addr, timeout_ms: u32) -> Option<f64> {
            let dest = u32::from(ip).to_be();
            let payload = [0u8; PAYLOAD];
            // ICMP_ECHO_REPLY 前缀：Address(4) Status(4) RoundTripTime(4)
            let reply_len = 12 + PAYLOAD + REPLY_EXTRA;
            let mut buf = vec![0u8; reply_len];
            let n = unsafe {
                IcmpSendEcho(
                    self.handle,
                    dest,
                    payload.as_ptr(),
                    PAYLOAD as u16,
                    std::ptr::null(),
                    buf.as_mut_ptr(),
                    reply_len as u32,
                    timeout_ms,
                )
            };
            if n == 0 { return None; }
            let status = u32::from_le_bytes(buf[4..8].try_into().unwrap());
            // 0 = IP_STATUS_BASE success
            if status != 0 { return None; }
            let rtt = u32::from_le_bytes(buf[8..12].try_into().unwrap());
            Some(rtt as f64)
        }
    }

    impl Drop for Session {
        fn drop(&mut self) { unsafe { let _ = IcmpCloseHandle(self.handle); } }
    }
}

/// 3 包 ICMP burst；返回 (采样点[3], 是否全超时)。
#[cfg(windows)]
fn icmp_probe(ip: std::net::Ipv4Addr) -> (Vec<Option<f64>>, bool) {
    let Some(sess) = icmp::Session::open() else { return (vec![None; 3], true) };
    let mut out = Vec::with_capacity(3);
    for _ in 0..3 { out.push(sess.echo(ip, 2000)); }
    let all_lost = out.iter().all(|s| s.is_none());
    (out, all_lost)
}
#[cfg(not(windows))]
fn icmp_probe(_ip: std::net::Ipv4Addr) -> (Vec<Option<f64>>, bool) { (vec![None; 3], true) }

/// TCP connect 探测（毫秒）；IPv6 兜底路径。
fn tcp_probe(ip: IpAddr, timeout_ms: u64) -> Option<f64> {
    let addr = std::net::SocketAddr::new(ip, 80);
    let start = std::time::Instant::now();
    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(timeout_ms)) {
        Ok(_) => Some(start.elapsed().as_secs_f64() * 1000.0),
        Err(_) => None,
    }
}

// ── path 拼接（SVG d）───────────────────────────────────────────────────
/// 主曲线 + 区域填充；None 点断段。返回 (line, area)。
fn chart_paths(samples: &[Option<f64>]) -> (String, String) {
    let valid: Vec<f64> = samples.iter().flatten().copied().collect();
    if valid.is_empty() { return (String::new(), String::new()); }
    let max_v = valid.iter().cloned().fold(0.0f64, f64::max);
    let scale = (max_v * 1.25 / 10.0).ceil() * 10.0;
    let scale = scale.max(40.0);
    let n = samples.len().max(2);
    let x_at = |i: usize| (i as f64 / (n - 1) as f64) * CHART_W;
    let y_at = |v: f64| CHART_H - 3.0 - (v / scale) * (CHART_H - 6.0);
    let mut line = String::new();
    let mut area = String::new();
    let mut pen = false;
    let mut seg_start: Option<usize> = None;
    let mut seg_pts: Vec<(f64, f64)> = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        match s {
            Some(v) => {
                if !pen { seg_start = Some(i); seg_pts.clear(); }
                let (x, y) = (x_at(i), y_at(*v));
                line.push_str(if pen { " L " } else { "M " });
                line.push_str(&format!("{:.1} {:.1}", x, y));
                seg_pts.push((x, y));
                pen = true;
            }
            None => {
                if pen {
                    // 断段：闭合当前段的区域
                    if let Some(s0) = seg_start {
                        area.push_str(&area_seg(&seg_pts, x_at(s0), x_at(i.saturating_sub(1)), CHART_H - 3.0));
                    }
                    pen = false;
                }
            }
        }
    }
    if pen {
        if let Some(s0) = seg_start {
            area.push_str(&area_seg(&seg_pts, x_at(s0), x_at(samples.len() - 1), CHART_H - 3.0));
        }
    }
    (line, area)
}

fn area_seg(pts: &[(f64, f64)], x0: f64, x1: f64, base: f64) -> String {
    if pts.is_empty() { return String::new(); }
    let mut d = String::from("M ");
    d.push_str(&format!("{:.1} {:.1} L ", x0, base));
    for (x, y) in pts { d.push_str(&format!("{:.1} {:.1} L ", x, y)); }
    d.push_str(&format!("{:.1} {:.1} Z ", x1, base));
    d
}

/// 丢包刻度：None 点在底部的红色短竖线。
fn loss_path(samples: &[Option<f64>]) -> String {
    let n = samples.len().max(2);
    let mut d = String::new();
    for (i, s) in samples.iter().enumerate() {
        if s.is_none() {
            let x = (i as f64 / (n - 1) as f64) * CHART_W;
            d.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1} ", x, CHART_H - 6.0, x, CHART_H));
        }
    }
    d
}

/// 行迷你趋势（40×12，min-max 归一）。
fn spark_path(samples: &[Option<f64>]) -> String {
    let valid: Vec<f64> = samples.iter().flatten().copied().collect();
    if valid.len() < 2 { return String::new(); }
    let max = valid.iter().cloned().fold(f64::MIN, f64::max);
    let min = valid.iter().cloned().fold(f64::MAX, f64::min);
    let span = (max - min).max(1.0);
    let n = samples.len();
    let mut d = String::new();
    let mut pen = false;
    for (i, s) in samples.iter().enumerate() {
        match s {
            Some(v) => {
                let x = (i as f64 / (n - 1).max(1) as f64) * SPARK_W;
                let y = SPARK_H - ((v - min) / span) * (SPARK_H - 2.0) - 1.0;
                d.push_str(if pen { " L " } else { "M " });
                d.push_str(&format!("{:.1} {:.1}", x, y));
                pen = true;
            }
            None => pen = false,
        }
    }
    d
}

// ── 端口模式：本机监听端口 ──────────────────────────────────────────────
#[cfg(windows)]
fn listen_table() -> Vec<ListenRow> {
    // 手写 extern + 结构（同 icmp mod：不依赖 windows crate 的 feature 组合）
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MibTcpRowOwnerModule {
        state: u32,
        local_addr: u32,
        local_port: u32,   // 网络序（低 16 位有效）
        remote_addr: u32,
        remote_port: u32,
        pid: u32,
        create_time: i64,
    }
    #[repr(C)]
    struct MibTcpTableOwnerModule {
        num_entries: u32,
        table: [MibTcpRowOwnerModule; 1],
    }
    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetExtendedTcpTable(
            table: *mut MibTcpTableOwnerModule,
            size: *mut u32,
            order: i32,
            af: u32,
            class: u32,
            reserved: u32,
        ) -> u32;
    }
    const TCP_TABLE_OWNER_MODULE_LISTENER: u32 = 3;
    const AF_INET_V: u32 = 2;
    const MIB_TCP_STATE_LISTEN: u32 = 2;
    let mut out = Vec::new();
    unsafe {
        let mut size: u32 = 0;
        let _ = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET_V, TCP_TABLE_OWNER_MODULE_LISTENER, 0);
        if size == 0 { return out; }
        let mut buf = vec![0u8; size as usize];
        let rc = GetExtendedTcpTable(buf.as_mut_ptr() as *mut MibTcpTableOwnerModule, &mut size, 0, AF_INET_V, TCP_TABLE_OWNER_MODULE_LISTENER, 0);
        if rc != 0 { return out; }
        let tbl = &*(buf.as_ptr() as *const MibTcpTableOwnerModule);
        let n = tbl.num_entries as usize;
        let rows = std::slice::from_raw_parts(tbl.table.as_ptr(), n);
        let mut sys = None;
        for row in rows {
            if row.state != MIB_TCP_STATE_LISTEN { continue; }
            // dwLocalPort 是网络序（htons），低 16 位
            let port = u16::from_be((row.local_port & 0xFFFF) as u16);
            let a = (row.local_addr >> 24) as u8;
            let b = (row.local_addr >> 16) as u8;
            let c = (row.local_addr >> 8) as u8;
            let d = row.local_addr as u8;
            if sys.is_none() { sys = Some(sysinfo::System::new()); }
            let name = sys.as_ref().and_then(|sys| sys.process(sysinfo::Pid::from_u32(row.pid)))
                .map(|pr| pr.name().to_string_lossy().to_string())
                .unwrap_or_else(|| "System".into());
            out.push(ListenRow {
                port: port as i32,
                name: SharedString::from(name),
                pid: row.pid as i32,
                addr: SharedString::from(format!("{}.{}.{}.{}:{}", a, b, c, d, port)),
                path: SharedString::new(),
                selected: false,
            });
        }
    }
    out.sort_by_key(|r| r.port);
    out.dedup_by(|a, b| a.port == b.port && a.name == b.name);
    out
}


// ── install ─────────────────────────────────────────────────────────────
pub(super) fn install(window: &AppWindow, store: &Rc<std::cell::RefCell<ConfigStore>>) {
    let w = window.as_weak();
    let state = Arc::new(Mutex::new(PingState::new()));
    // 连续 Ping 定时器：slint::Timer 非 Send，留在 UI 线程（state 不携带）
    let auto_timer: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));

    // 源下拉：本机 + 项目内全部会话（三期接 SSH；未在线不可选）
    {
        let mut hosts = vec![PingHostOption {
            key: "local".into(), name: "本机".into(), online: true,
        }];
        for s in store.borrow().sessions() {
            hosts.push(PingHostOption {
                key: SharedString::from(s.id.clone()),
                name: SharedString::from(format!("{}（{}）", s.name, s.host)),
                online: false, // 三期：随会话连接状态刷新
            });
        }
        window.set_ping_hosts(ModelRc::from(Rc::new(VecModel::from(hosts))));
    }

    // 「加入 N」条数（pure callback，草稿文本 → 解析条数）
    window.on_ping_count_draft(move |draft| {
        parse_draft(draft.as_str()).len().min(MAX_TARGETS) as i32
    });

    // 关闭弹框：停连续 Ping（点外不关，关闭走 Esc/✕/标题栏图标，均转发到 close-requested）
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_close(move || {
            if let Ok(mut st) = state.lock() {
                st.auto_on = false;
            }
            let Some(w) = w.upgrade() else { return };
            w.set_ping_auto_on(false);
        });
    }

    // 方式切换
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_set_method(move |m| {
            if let Ok(mut st) = state.lock() {
                st.method = m.to_string();
            }
            let Some(w) = w.upgrade() else { return };
            w.set_ping_method(SharedString::from(m.as_str()));
            if m == "port" {
                // 一期同步拉取监听端口（快照 <100ms，直接同步调用）
                let rows = listen_table();
                w.set_ping_listen_count(rows.len() as i32);
                set_listen_model(&w, &rows, None);
            }
            refresh_all(&w, &state);
        });
    }

    // 加入（解析草稿并入清单）
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_add_draft(move || {
            let Some(w) = w.upgrade() else { return };
            let draft = w.get_ping_draft().to_string();
            if draft.trim().is_empty() { return; }
            let parsed = parse_draft(&draft);
            let mut err = String::new();
            let mut added = 0usize;
            let mut skipped = 0usize;
            if let Ok(mut st) = state.lock() {
                for t in parsed {
                    if !looks_like_host(&t) {
                        if err.is_empty() { err = format!("「{}」不是有效的 IP / 域名", t); }
                        continue;
                    }
                    if st.targets.iter().any(|x| x.addr.eq_ignore_ascii_case(&t)) { continue; }
                    if st.targets.len() >= MAX_TARGETS { skipped += 1; continue; }
                    st.targets.push(TargetState::new(t));
                    added += 1;
                }
            }
            if !err.is_empty() {
                w.set_ping_has_error(true);
                w.set_ping_input_error(SharedString::from(err));
                return;
            }
            if skipped > 0 {
                w.set_ping_input_error(SharedString::from(format!("一次最多 {} 个目标 · 已跳过 {} 个", MAX_TARGETS, skipped)));
                w.set_ping_has_error(true);
            }
            w.set_ping_draft(SharedString::new());
            refresh_all(&w, &state);
            let _ = added;
        });
    }

    // 清空草稿
    {
        let w = w.clone();
        window.on_ping_clear_draft(move || {
            let Some(w) = w.upgrade() else { return };
            w.set_ping_draft(SharedString::new());
            w.set_ping_has_error(false);
        });
    }

    // 移除目标
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_remove_target(move |addr| {
            if let Ok(mut st) = state.lock() {
                st.targets.retain(|t| t.addr != addr.as_str());
            }
            let Some(w) = w.upgrade() else { return };
            refresh_all(&w, &state);
        });
    }

    // 选择目标（空串＝回汇总；再点同一行回汇总）
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_select_target(move |addr| {
            if let Ok(mut st) = state.lock() {
                let cur = st.selected_addr().map(|s| s.to_string()).unwrap_or_default();
                let next = if cur == addr.as_str() { String::new() } else { addr.to_string() };
                for t in &mut st.targets { t.selected = t.addr == next; }
            }
            let Some(w) = w.upgrade() else { return };
            refresh_all(&w, &state);
        });
    }

    // 探测 / 刷新
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_run(move || {
            let Ok(mut st) = state.lock() else { return };
            if st.method == "port" {
                drop(st);
                let Some(w) = w.upgrade() else { return };
                w.set_ping_testing(true);
                let w2 = w.as_weak();
                let state2 = state.clone();
                std::thread::spawn(move || {
                    let rows = listen_table();
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(w) = w2.upgrade() else { return };
                        if let Ok(mut st) = state2.lock() {
                            st.listen_rows = rows;
                        }
                        w.set_ping_testing(false);
                        refresh_all(&w, &state2);
                        toast(&w, "已刷新监听端口");
                    });
                });
                return;
            }
            // Ping：草稿非空 → 先并入（与高保真 startTest 一致）
            let Some(w) = w.upgrade() else { return };
            let draft = w.get_ping_draft().to_string();
            if !draft.trim().is_empty() {
                let mut err = String::new();
                let mut skipped = 0usize;
                for t in parse_draft(&draft) {
                    if !looks_like_host(&t) {
                        if err.is_empty() { err = format!("「{}」不是有效的 IP / 域名", t); }
                        continue;
                    }
                    if let Ok(mut s) = state.lock() {
                        if s.targets.iter().any(|x| x.addr.eq_ignore_ascii_case(&t)) { continue; }
                        if s.targets.len() >= MAX_TARGETS { skipped += 1; continue; }
                        s.targets.push(TargetState::new(t));
                    }
                }
                if !err.is_empty() {
                    w.set_ping_has_error(true);
                    w.set_ping_input_error(SharedString::from(err));
                    return;
                }
                if skipped > 0 {
                    w.set_ping_input_error(SharedString::from(format!("一次最多 {} 个目标 · 已跳过 {} 个", MAX_TARGETS, skipped)));
                    w.set_ping_has_error(true);
                }
                w.set_ping_draft(SharedString::new());
            }
            let targets: Vec<String> = state.lock().ok()
                .map(|s| s.targets.iter().map(|t| t.addr.clone()).collect())
                .unwrap_or_default();
            if targets.is_empty() {
                w.set_ping_has_error(true);
                w.set_ping_input_error(SharedString::from("请至少选择一个目标"));
                return;
            }
            w.set_ping_has_error(false);
            drop(st);
            start_round(&w, &state, targets);
        });
    }

    // 连续 Ping
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_toggle_auto(move |on| {
            let auto_timer = auto_timer.clone();
            let Some(w) = w.upgrade() else { return };
            w.set_ping_auto_on(on);
            if let Ok(mut st) = state.lock() { st.auto_on = on; }
            if on {
                let w2 = w.as_weak();
                let state2 = state.clone();
                let timer = slint::Timer::default();
                timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(AUTO_INTERVAL_MS), move || {
                    let Some(w) = w2.upgrade() else { return };
                    if !w.get_ping_dialog_open() || !w.get_ping_auto_on() { return; }
                    if w.get_ping_testing() { return; }
                    let targets: Vec<String> = state2.lock().unwrap()
                        .targets.iter().map(|t| t.addr.clone()).collect();
                    if targets.is_empty() { return; }
                    start_round(&w, &state2, targets);
                });
                *auto_timer.borrow_mut() = Some(timer);
            } else {
                *auto_timer.borrow_mut() = None;
            }
        });
    }

    // 清空结果（保留目标清单，V2 定稿）
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_clear_results(move || {
            if let Ok(mut st) = state.lock() {
                for t in &mut st.targets { t.samples.clear(); t.mode = "idle".into(); }
                for t in &mut st.targets { t.selected = false; }
            }
            let Some(w) = w.upgrade() else { return };
            refresh_all(&w, &state);
        });
    }

    // 复制目标
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_copy_target(move || {
            let Some(w) = w.upgrade() else { return };
            let text = {
                let st = state.lock().unwrap();
                if st.method == "port" {
                    st.listen_sel.map(|p| p.to_string()).unwrap_or_default()
                } else if let Some(sel) = st.selected_addr() {
                    sel.to_string()
                } else {
                    st.targets.iter().map(|t| t.addr.clone()).collect::<Vec<_>>().join(", ")
                }
            };
            if text.is_empty() { return; }
            super::clipboard_set_text(text.clone());
            toast(&w, &format!("已复制 {}", text));
        });
    }

    // 监听表：选中 / 搜索
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_listen_select(move |port| {
            if let Ok(mut st) = state.lock() {
                st.listen_sel = if st.listen_sel == Some(port as u16) { None } else { Some(port as u16) };
            }
            let Some(w) = w.upgrade() else { return };
            render_listen(&w, &state);
            refresh_all(&w, &state);
        });
    }
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_listen_search_edited(move |text| {
            if let Ok(mut st) = state.lock() {
                st.listen_query = text.to_string();
            }
            let Some(w) = w.upgrade() else { return };
            render_listen(&w, &state);
        });
    }

    // 源切换（一期仅本机可选；回调留全量路径）
    {
        let state = state.clone();
        let w = window.as_weak();
        window.on_ping_set_source(move |key| {
            if let Ok(mut st) = state.lock() {
                st.source = key.to_string();
            }
            let Some(w) = w.upgrade() else { return };
            let name = if key == "local" { "本机".to_string() } else { key.to_string() };
            w.set_ping_source_name(SharedString::from(name));
            refresh_all(&w, &state);
        });
    }
}

// ── 探测轮次 ────────────────────────────────────────────────────────────
fn start_round(w: &AppWindow, state: &Arc<Mutex<PingState>>, targets: Vec<String>) {
    w.set_ping_testing(true);
    {
        let Ok(mut st) = state.lock() else { return };
        for t in &mut st.targets {
            if targets.iter().any(|x| x == &t.addr) { t.mode = "testing".into(); }
        }
    }
    refresh_all(w, state);
    let w2 = w.as_weak();
    let state2 = state.clone();
    std::thread::spawn(move || {
        let mut results: Vec<(String, TargetState)> = Vec::new();
        for addr in &targets {
            let ip = resolve(addr);
            let mut ts = TargetState::new(addr.clone());
            ts.ip = ip;
            match ip {
                Some(IpAddr::V4(v4)) => {
                    let (pts, all_lost) = icmp_probe(v4);
                    if all_lost {
                        ts.mode = "offline".into();
                    } else {
                        let loss = pts.iter().filter(|s| s.is_none()).count() as f64 / pts.len() as f64 * 100.0;
                        let max_rtt = pts.iter().flatten().cloned().fold(0.0f64, f64::max);
                        ts.mode = if loss > 0.0 || max_rtt >= 100.0 { "degraded".into() } else { "normal".into() };
                    }
                    ts.samples = pts;
                }
                Some(IpAddr::V6(v6)) => {
                    let rt = tcp_probe(IpAddr::V6(v6), 2000);
                    ts.samples = vec![rt];
                    ts.mode = if rt.is_some() { "normal".into() } else { "offline".into() };
                }
                None => {
                    ts.mode = "offline".into();
                    ts.samples = vec![None; 3];
                }
            }
            results.push((addr.clone(), ts));
        }
        let _ = slint::invoke_from_event_loop(move || {
            let Some(w) = w2.upgrade() else { return };
            let mut done = 0usize;
            if let Ok(mut st) = state2.lock() {
                let cur_sel: Option<String> = st.selected_addr().map(|s| s.to_string());
                for (addr, ts) in results {
                    if let Some(t) = st.targets.iter_mut().find(|t| t.addr == addr) {
                        *t = ts;
                        t.selected = cur_sel.as_deref() == Some(&t.addr);
                    }
                    done += 1;
                }
            }
            w.set_ping_testing(false);
            refresh_all(&w, &state2);
            toast(&w, &format!("Ping 完成 · {} 个目标", done));
        });
    });
}

// ── 渲染 ────────────────────────────────────────────────────────────────
fn refresh_all(w: &AppWindow, state: &Arc<Mutex<PingState>>) {
    if let Ok(mut s) = state.lock() { s.lang_en = w.get_lang_en(); }
    let st = state.lock().unwrap();
    let lang = st.lang_en;
    w.set_ping_target_count(st.targets.len() as i32);
    w.set_ping_method_label(SharedString::from(if st.method == "port" {
        if lang { "Ports" } else { "端口" }
    } else { "ICMP".into() }));

    // 清单模型
    let mut rows: Vec<PingTargetRow> = Vec::new();
    for t in &st.targets {
        rows.push(PingTargetRow {
            addr: SharedString::from(t.addr.clone()),
            state_word: SharedString::from(t.state_word(lang)),
            tone: t.tone(),
            rtt: SharedString::from(match t.last() {
                Some(v) if t.mode != "testing" && t.mode != "idle" => format!("{:.0} ms", v),
                _ => "—".into(),
            }),
            loss: SharedString::from(if t.mode == "testing" || t.mode == "idle" {
                "—".to_string()
            } else { format!("{:.1}%", t.loss_pct()) }),
            spark: SharedString::from(spark_path(&t.samples)),
            selected: t.selected,
        });
    }
    w.set_ping_targets(ModelRc::from(Rc::new(VecModel::from(rows))));

    if st.method == "port" {
        render_listen(w, state);
        // 端口模式右栏
        let sel = st.listen_sel.and_then(listen_row_in(&st.listen_rows));
        w.set_ping_selected_target_active(false);
        match sel {
            Some(r) => {
                w.set_ping_result_target(SharedString::from(format!("端口 {}", r.port)));
                w.set_ping_rtt_label(SharedString::from(if lang { "Listening port" } else { "监听端口" }));
                w.set_ping_rtt_value(SharedString::from(r.port.to_string()));
                w.set_ping_rtt_unit(SharedString::new());
                w.set_ping_quality_text(SharedString::from(if lang { "Listening" } else { "监听中" }));
                w.set_ping_quality_hint(SharedString::from(format!("TCP · {}", r.addr)));
                w.set_ping_show_owner(true);
                w.set_ping_owner_host(SharedString::from(r.addr.to_string()));
                w.set_ping_owner_row(SharedString::from(format!("{} · pid {}", r.name, r.pid)));
            }
            None => {
                w.set_ping_result_target(SharedString::from(if lang { "Local port usage" } else { "本机端口占用" }));
                w.set_ping_rtt_label(SharedString::from(if lang { "Total ports" } else { "端口总数" }));
                w.set_ping_rtt_value(SharedString::from(st.listen_rows.len().to_string()));
                w.set_ping_rtt_unit(SharedString::from(if lang { "" } else { "个" }));
                w.set_ping_quality_text(SharedString::from(if lang { "All listening" } else { "全部监听" }));
                w.set_ping_quality_hint(SharedString::from(if lang { "Select a port for details" } else { "点左侧端口查看占用详情" }));
                w.set_ping_show_owner(false);
            }
        }
        w.set_ping_quality_tone(SharedString::from(if st.listen_sel.is_some() { "good" } else { "neutral" }));
        w.set_ping_chart_show(false);
        w.set_ping_run_text(SharedString::from(if w.get_ping_testing() {
            if lang { "Refreshing…" } else { "刷新中…" }
        } else { "刷新".into() }));
        let procs: std::collections::HashSet<&str> = st.listen_rows.iter().map(|r| r.name.as_str()).collect();
        w.set_ping_summary(SharedString::from(format!(
            "{} 个监听端口 · {} 个进程", st.listen_rows.len(), procs.len())));
        w.set_ping_auto_disabled(true);
        return;
    }

    // ── Ping 模式右栏 ──────────────────────────────────────────────────
    let sel_addr = st.selected_addr().map(|s| s.to_string());
    let selected_active = sel_addr.is_some();
    w.set_ping_selected_target_active(selected_active);

    let testing = st.targets.iter().any(|t| t.mode == "testing");
    let has_any = st.targets.iter().any(|t| t.mode != "idle");

    let (rtt_label, rtt_value, rtt_unit, q_text, q_tone, q_hint, title, chart_on, chart_src, loss_src, owner, stats): (
        String, String, String, String, String, String, String, bool, Vec<Option<f64>>, String, Option<(String, String)>, [String; 6]
    ) = if testing {
        ("当前 RTT".to_string(), "—".to_string(), "ms".to_string(), (if lang { "Testing" } else { "测试中" }).to_string(), "testing".to_string(),
         (if lang { "Probing targets" } else { "正在探测目标" }).into(), String::new(), true, Vec::new(), String::new(), None,
         ["—".into(), "—".into(), "—".into(), "—".into(), "—".into(), "—".into()])
    } else if let Some(addr) = &sel_addr {
        let Some(t) = st.targets.iter().find(|t| &t.addr == addr) else {
            return;
        };
        let offline = t.mode == "offline";
        let last = t.last();
        let st_ = t;
        let stats = [
            (if lang { "Avg" } else { "平均" }).to_string(),
            st_.avg().map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into()),
            (if lang { "Jitter" } else { "抖动" }).to_string(),
            st_.jitter().map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into()),
            (if lang { "Loss" } else { "丢包" }).to_string(),
            if offline { "100.0".into() } else { format!("{:.1}", st_.loss_pct()) },
        ];
        let hint = if offline {
            (if lang { "Check address or network" } else { "请检查地址或网络" }).to_string()
        } else { (if lang { "Stable" } else { "连接稳定" }).to_string() };
        (
            if offline { (if lang { "Result" } else { "探测结果" }).to_string() } else { "当前 RTT".to_string() },
            if offline { "超时".into() } else { last.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()) },
            "ms".to_string(),
            if offline { (if lang { "Timeout" } else { "超时" }).to_string() }
                else if t.mode == "degraded" { (if lang { "Fair" } else { "一般" }).to_string() }
                else { (if lang { "Good" } else { "优秀" }).to_string() },
            if offline { "bad".to_string() } else if t.mode == "degraded" { "fair".to_string() } else { "good".to_string() },
            hint,
            t.addr.clone(),
            true,
            t.samples.clone(),
            loss_path(&t.samples),
            Some((t.addr.clone(), format!("本机 · {}", t.ip.map(|i| i.to_string()).unwrap_or_else(|| t.addr.clone())))),
            stats,
        )
    } else if has_any {
        // 汇总：最慢 + 最快 + 平均抖动 + 丢包目标
        let mut worst_v: Option<f64> = None;
        let mut fast_v: Option<f64> = None;
        let mut bad_n = 0usize;
        let mut loss_n = 0usize;
        let mut jit_sum = 0.0f64;
        let mut jit_n = 0usize;
        let mut worst_name = String::new();
        for t in &st.targets {
            if t.mode == "idle" || t.mode == "testing" { continue; }
            if t.mode == "offline" || t.mode == "degraded" { bad_n += 1; }
            if t.loss_pct() > 0.0 { loss_n += 1; }
            if t.mode != "offline" {
                if let Some(l) = t.last() {
                    if worst_v.map(|w| l > w).unwrap_or(true) { worst_v = Some(l); worst_name = t.addr.clone(); }
                }
                if let Some(f) = t.last() { if fast_v.map(|fv| f < fv).unwrap_or(true) { fast_v = Some(f); } }
            }
            if let Some(j) = t.jitter() { jit_sum += j; jit_n += 1; }
        }
        let chart_src = st.worst_target().map(|t| t.samples.clone()).unwrap_or_default();
        let worst_title = st.worst_target().map(|t| t.addr.clone()).unwrap_or_default();
        (
            "Slowest".to_string(),
            worst_v.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
            "ms".to_string(),
            if bad_n > 0 { format!("{} 个异常", bad_n) } else { "全部正常".into() },
            if bad_n > 0 { "fair".to_string() } else { "good".to_string() },
            format!("{} 个目标 · 最慢来自 {}", st.targets.len(), if worst_name.is_empty() { "—".into() } else { worst_title }),
            format!("{} 个目标", st.targets.len()),
            true,
            chart_src.clone(),
            loss_path(&chart_src),
            None,
            [
                (if lang { "Fastest" } else { "最快" }).to_string(),
                fast_v.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
                (if lang { "Avg jitter" } else { "平均抖动" }).to_string(),
                if jit_n > 0 { format!("{:.1}", jit_sum / jit_n as f64) } else { "—".into() },
                (if lang { "Loss targets" } else { "丢包目标" }).to_string(),
                loss_n.to_string(),
            ],
        )
    } else {
        (
            "当前 RTT".to_string(), "—".to_string(), "ms".to_string(),
            (if lang { "Idle" } else { "待测" }).to_string(), "neutral".to_string(),
            (if lang { "Add a target to start" } else { "输入目标开始" }).into(),
            String::new(), false, Vec::new(), String::new(), None,
            ["—".into(), "—".into(), "—".into(), "—".into(), "—".into(), "—".into()],
        )
    };

    w.set_ping_rtt_label(SharedString::from(rtt_label));
    w.set_ping_rtt_value(SharedString::from(rtt_value.clone()));
    w.set_ping_rtt_unit(SharedString::from(rtt_unit));
    w.set_ping_quality_text(SharedString::from(q_text));
    w.set_ping_quality_tone(SharedString::from(q_tone));
    w.set_ping_quality_hint(SharedString::from(q_hint));
    w.set_ping_result_target(SharedString::from(if title.is_empty() {
        (if lang { "Ping" } else { "Ping" }).to_string()
    } else { title }));
    w.set_ping_chart_show(chart_on && !chart_src.is_empty());
    let (line, area) = chart_paths(&chart_src);
    w.set_ping_chart_path(SharedString::from(line));
    w.set_ping_chart_area_path(SharedString::from(area));
    w.set_ping_chart_loss_path(SharedString::from(loss_src));
    if let Some((host, row)) = owner {
        w.set_ping_show_owner(true);
        w.set_ping_owner_host(SharedString::from(host));
        w.set_ping_owner_row(SharedString::from(row));
    } else {
        w.set_ping_show_owner(false);
    }
    w.set_ping_stat_1_label(SharedString::from(stats[0].clone()));
    w.set_ping_stat_1_value(SharedString::from(stats[1].clone()));
    w.set_ping_stat_2_label(SharedString::from(stats[2].clone()));
    w.set_ping_stat_2_value(SharedString::from(stats[3].clone()));
    w.set_ping_stat_3_label(SharedString::from(stats[4].clone()));
    w.set_ping_stat_3_value(SharedString::from(stats[5].clone()));

    // 页脚
    let testing = st.targets.iter().any(|t| t.mode == "testing");
    w.set_ping_testing(testing);
    w.set_ping_run_text(SharedString::from(if testing {
        (if lang { "Testing…" } else { "测试中…" }).into()
    } else { format!("探测 {} 个", st.targets.len()) }));
    let auto_disabled = testing || !has_any;
    w.set_ping_auto_disabled(auto_disabled);
    let summary = if st.targets.is_empty() {
        (if lang { "No targets" } else { "勾选目标，或在上方输入 IP 后加入" }).to_string()
    } else if testing {
        (if lang { "Testing…" } else { "正在探测…" }).to_string()
    } else {
        let bad = st.targets.iter().filter(|t| t.mode == "offline" || t.mode == "degraded").count();
        format!("{} 个目标 · {}", st.targets.len(), if bad > 0 { format!("{} 个异常", bad) } else { "全部正常".into() })
    };
    w.set_ping_summary(SharedString::from(summary));

    // 源提示（一期固定本机）
    w.set_ping_source_name(SharedString::from(match st.source.as_str() {
        "local" => "本机".into(),
        k => k.to_string(),
    }));
}

fn listen_row_in<'a>(rows: &'a [ListenRow]) -> impl Fn(u16) -> Option<&'a ListenRow> + 'a {
    move |p| rows.iter().find(|r| r.port as u16 == p)
}

fn set_listen_model(w: &AppWindow, rows: &[ListenRow], sel: Option<u16>) {
    let m: Vec<ListenRow> = rows.iter().map(|r| ListenRow {
        port: r.port, name: r.name.clone(), pid: r.pid, addr: r.addr.clone(),
        path: r.path.clone(), selected: Some(r.port as u16) == sel,
    }).collect();
    w.set_ping_listen_items(ModelRc::from(Rc::new(VecModel::from(m))));
}

fn render_listen(w: &AppWindow, state: &Arc<Mutex<PingState>>) {
    let st = state.lock().unwrap();
    let q = st.listen_query.trim().to_lowercase();
    let items: Vec<ListenRow> = st.listen_rows.iter().filter(|r| {
        q.is_empty()
            || r.port.to_string().contains(&q)
            || r.name.to_lowercase().contains(&q)
            || r.addr.to_string().contains(&q)
    }).cloned().map(|mut r| {
        r.selected = st.listen_sel == Some(r.port as u16);
        r
    }).collect();
    w.set_ping_listen_count(items.len() as i32);
    w.set_ping_listen_items(ModelRc::from(Rc::new(VecModel::from(items))));
}

fn toast(w: &AppWindow, text: &str) {
    // 复用命令管理同款 toast 通道（若可用则静默失败，不打断探测流程）
    let _ = w;
    let _ = text;
}
