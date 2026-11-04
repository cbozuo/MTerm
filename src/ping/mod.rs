//! 标题栏 Ping 弹框（#titlebar-ping）的**探测引擎**。
//!
//! 分层与 `crate::command_hint` / `crate::resource` 同款：类型与领域规则在
//! `struct/`，平台实现（Win32 ICMP / TCP 表 / SVG path 拼接）在 `impls/`，
//! Slint 宿主桥（回调接线、模型下发、渲染）在 `crate::app::ping`。
//! 本层不依赖 Slint 类型 —— 监听行用自有 `ListenEntry`，转 Slint struct 在桥层。
//!
//! 结构对应 ui/ping_dialog.slint：
//!  · 双模式：`icmp`＝可达性探测（本机源，草稿→清单→RTT/曲线）；`port`＝本机
//!    监听端口占用视图（GetExtendedTcpTable + 进程名，无目标概念）。
//!  · 探测在后台 `std::thread` 执行（ICMP 3 包 burst、每包 2s 超时），结果经
//!    `slint::invoke_from_event_loop` 回 UI（桥层职责）。
//!  · 一期边界：探测源仅「本机」（托管主机条目灰显占位，三期接
//!    `ssh::execute_command`）；IPv4 走 ICMP，IPv6 / 解析失败标记对应状态，
//!    ICMP 无响应不做 TCP 降级（高保真边界表「ICMP 被禁用自动降级」挂账二期）。

#[path = "struct/model.rs"]
pub(crate) mod model;
#[path = "impls/net.rs"]
pub(crate) mod net;
#[path = "impls/listen.rs"]
pub(crate) mod listen;
#[path = "impls/chart.rs"]
pub(crate) mod chart;
#[path = "impls/local_ip.rs"]
pub(crate) mod local_ip;

pub(crate) use model::{parse_draft, looks_like_host, resolve, ListenEntry, PingState, TargetState,
    AUTO_INTERVAL_MS, MAX_TARGETS};
pub(crate) use net::{icmp_probe, tcp_probe};
pub(crate) use listen::listen_table;
pub(crate) use chart::{chart_paths, loss_path, spark_path};
pub(crate) use local_ip::local_ipv4_list;
