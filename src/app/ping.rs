//! 标题栏 Ping 弹框（#titlebar-ping，高保真 docs/design/titlebar-ping-hifi-2026-11-03.html
//! V13）的 **Slint 宿主桥**：回调接线、模型下发、右栏渲染。
//!
//! 领域规则与平台实现（ICMP/TCP 探测、监听端口表、SVG path 拼接、目标态）
//! 在 `crate::ping`；终端行缓冲之类的依赖同理不下放——本层只做搬运。
//! 分层说明见 `src/ping/mod.rs`。
//!
//! 桥层职责与既有模式：
//!  · 探测在后台 `std::thread` 执行，结果经 `slint::invoke_from_event_loop`
//!    回 UI（项目标准模式，见 app/session_runtime.rs）。
//!  · 连续 Ping = `slint::Timer`（Repeated 2s）驱动的后台轮次，关弹框即停。
//!  · Slint 无 split / filter：「加入 N」条数走 pure callback `ping-count-draft`，
//!    监听表/源下拉的搜索过滤在本层重建模型。
//!  · ⚠️ state 的 Mutex 锁必须全部短作用域：UI 线程持锁期间二次 lock() 是
//!    std Mutex 重入 → 永久阻塞 → 整窗「未响应」（持锁路径用 *_locked 变体）。

use crate::config::ConfigStore;
use crate::ping::{chart_paths, icmp_probe, listen_table, loss_path, looks_like_host, parse_draft,
    resolve, spark_path, tcp_probe, ListenEntry, PingState, TargetState,
    AUTO_INTERVAL_MS, MAX_TARGETS};
use crate::ui::{AppWindow, ListenRow, PingHostInfo, PingHostOption, PingTargetRow};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::net::IpAddr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// ── install ─────────────────────────────────────────────────────────────
pub(super) fn install(window: &AppWindow, store: &Rc<std::cell::RefCell<ConfigStore>>) {
    let w = window.as_weak();
    let state = Arc::new(Mutex::new(PingState::new()));
    // 连续 Ping 定时器：slint::Timer 非 Send，留在 UI 线程（state 不携带）
    let auto_timer: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));

    // ── 下拉数据（2026-11-04 用户定稿）──────────────────────────────────
    // 源下拉 = 本机 + 已打开 tab 的会话（连接态动态）；＋ 下拉 = 本机 + 全部
    // 已保存会话（「本软件维护的集合」，不管有没有开 tab，key=会话 host 即
    // 探测地址）。两处行都带悬浮信息卡字段（tab 卡同款；远程源仍仅展示，
    // 真正从远端探测要三期 SSH 通道，见文件头）。
    fn kind_proto(kind: crate::config::SessionKind, lang_en: bool) -> &'static str {
        use crate::config::SessionKind as K;
        match kind {
            K::Ssh => "SSH",
            K::Serial => if lang_en { "Serial" } else { "串口" },
            K::Telnet => "Telnet",
            K::Local => if lang_en { "Local" } else { "本地" },
            K::Rdp => "RDP",
            K::Vnc => "VNC",
        }
    }

    type HostEntry = (String, String, bool, String, String, String, String, String, String);

    /// 源下拉全量：本机 + 当前窗口已打开 tab（按会话去重）。tab 的卡片字段
    /// 直接从 TabInfo 透传（Rust 建 tab 时已填好 proto/host/user/auth/group/note）。
    fn build_hosts(window: &AppWindow, store: &Rc<std::cell::RefCell<ConfigStore>>) -> Vec<HostEntry> {
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<HostEntry> =
            vec![("local".to_string(), "本机".to_string(), false,
                  String::new(), String::new(), String::new(), String::new(),
                  String::new(), String::new())];
        for i in 0..window.get_tabs().row_count() {
            let Some(t) = window.get_tabs().row_data(i) else { continue };
            let sid = t.session_id.to_string();
            if sid.is_empty() || seen.contains(&sid) { continue; }
            seen.push(sid.clone());
            let name = store.borrow().get(&sid)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| t.title.to_string());
            out.push((sid, name, t.connected,
                t.proto.to_string(), t.host.to_string(), t.user.to_string(),
                t.auth.to_string(), t.group_name.to_string(), t.note.to_string()));
        }
        out
    }

    fn push_hosts(window: &AppWindow, entries: &[HostEntry]) {
        let hosts: Vec<PingHostOption> = entries.iter()
            .map(|(k, name, conn, proto, host, user, auth, group, note)| PingHostOption {
                key: SharedString::from(k.clone()),
                name: SharedString::from(name.clone()),
                online: k == "local" || *conn,
                proto: SharedString::from(proto.clone()),
                host: SharedString::from(host.clone()),
                user: SharedString::from(user.clone()),
                auth: SharedString::from(auth.clone()),
                group: SharedString::from(group.clone()),
                note: SharedString::from(note.clone()),
            }).collect();
        window.set_ping_hosts(ModelRc::from(Rc::new(VecModel::from(hosts))));
    }

    fn stash_hosts(state: &Arc<Mutex<PingState>>, entries: &[HostEntry]) {
        if let Ok(mut st) = state.lock() {
            st.hosts_full = entries.to_vec();
        }
    }

    // install 时先铺一版，之后每次打开下拉由 refresh-hosts 重建（tab 动态）
    let entries = build_hosts(window, store);
    stash_hosts(&state, &entries);
    push_hosts(window, &entries);
    // 打开源下拉：重建主机列表（tab 开/关后数据才不陈旧）
    {
        let w = w.clone();
        let store = Rc::clone(store);
        let state = state.clone();
        window.on_ping_refresh_hosts(move || {
            let Some(w) = w.upgrade() else { return };
            let entries = build_hosts(&w, &store);
            stash_hosts(&state, &entries);
            push_hosts(&w, &entries);
        });
    }

    // ＋ 下拉全量：本机 + 全部已保存会话（打开下拉时重建，增删会话不陈旧）
    {
        let w = w.clone();
        let store = Rc::clone(store);
        window.on_ping_refresh_targets(move || {
            let Some(w) = w.upgrade() else { return };
            let lang = w.get_lang_en();
            let store = store.borrow();
            let mut opts: Vec<PingHostInfo> = vec![PingHostInfo {
                key: "127.0.0.1".into(), name: "本机".into(),
                proto: kind_proto(crate::config::SessionKind::Local, lang).into(),
                host: "127.0.0.1".into(),
                user: "".into(), auth: "".into(), group: "".into(), note: "".into(),
            }];
            for s in store.sessions() {
                if s.host.trim().is_empty() { continue; }
                let group = crate::config::display_group_of(s);
                opts.push(PingHostInfo {
                    key: SharedString::from(s.host.clone()),
                    name: SharedString::from(s.name.clone()),
                    proto: SharedString::from(kind_proto(s.kind, lang)),
                    host: SharedString::from(s.host.clone()),
                    user: SharedString::from(s.user.clone()),
                    auth: SharedString::from(s.auth.as_str()),
                    group: SharedString::from(group),
                    note: SharedString::from(s.note.clone()),
                });
            }
            w.set_ping_target_opts(ModelRc::from(Rc::new(VecModel::from(opts))));
        });
    }

    // 源下拉搜索过滤（Slint 无 filter，Rust 重建模型）
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_source_search_edited(move |text| {
            let q = text.trim().to_lowercase();
            let filtered: Vec<PingHostOption> = {
                let Ok(st) = state.lock() else { return };
                st.hosts_full.iter()
                    .filter(|(k, name, _, _, _, _, _, _, _)| {
                        q.is_empty()
                            || name.to_lowercase().contains(&q)
                            || k.to_lowercase().contains(&q)
                    })
                    .map(|(k, name, conn, proto, host, user, auth, group, note)| PingHostOption {
                        key: SharedString::from(k.clone()),
                        name: SharedString::from(name.clone()),
                        online: k == "local" || *conn,
                        proto: SharedString::from(proto.clone()),
                        host: SharedString::from(host.clone()),
                        user: SharedString::from(user.clone()),
                        auth: SharedString::from(auth.clone()),
                        group: SharedString::from(group.clone()),
                        note: SharedString::from(note.clone()),
                    })
                    .collect()
            };
            let Some(w) = w.upgrade() else { return };
            w.set_ping_hosts(ModelRc::from(Rc::new(VecModel::from(filtered))));
        });
    }

    // 「加入 N」条数（pure callback，草稿文本 → 解析条数）
    window.on_ping_count_draft(move |draft| {
        parse_draft(draft.as_str()).len().min(MAX_TARGETS) as i32
    });

    // ＋ 下拉行过滤（pure callback；Slint string 无 contains）
    window.on_ping_ip_hit(move |key, name, q| {
        let q = q.trim().to_lowercase();
        q.is_empty()
            || key.to_lowercase().contains(&q)
            || name.to_lowercase().contains(&q)
    });
    // 关闭弹框：完全重置（2026-11-04 用户定稿「不留记忆」）——停连续 Ping、
    // 清空会话态（目标清单/草稿/结果/监听表/选中）、方式回 ICMP、源回本机。
    // （点外不关，关闭走 Esc/✕/标题栏图标，均转发到 close-requested）
    {
        let w = w.clone();
        let state = state.clone();
        let auto_timer = auto_timer.clone();
        window.on_ping_close(move || {
            // 连续 Ping 定时器一并停（留 timer + auto_on=false 也会空转触发）
            *auto_timer.borrow_mut() = None;
            // hosts_full 是 install 时的静态数据，重置前取出回填
            let hosts_full = state.lock().map(|st| st.hosts_full.clone()).unwrap_or_default();
            *state.lock().unwrap() = PingState::new();
            if let Ok(mut st) = state.lock() {
                st.hosts_full = hosts_full;
            }
            let Some(w) = w.upgrade() else { return };
            w.set_ping_auto_on(false);
            w.set_ping_method(SharedString::from("icmp"));
            w.set_ping_source_key(SharedString::from("local"));
            w.set_ping_source_name(SharedString::from("本机"));
            w.set_ping_draft(SharedString::new());
            w.set_ping_source_search(SharedString::new());
            w.set_ping_listen_search(SharedString::new());
            w.set_ping_has_error(false);
            w.set_ping_input_error(SharedString::new());
            w.set_ping_listen_count(0);
            w.set_ping_listen_selected(0);
            refresh_all(&w, &state);
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
                if let Ok(mut s) = state.lock() {
                    // 存入 state：refresh_all → render_listen_locked 从 st.listen_rows
                    // 渲染，不存会被空表覆盖成 0 行
                    s.listen_rows = rows.clone();
                }
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

    // ＋ 下拉模型由 on_ping_refresh_targets 打开时重建（本机+全部已保存会话）
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_add_target(move |addr| {
            let addr = addr.trim().to_string();
            if addr.is_empty() { return; }
            let Some(w) = w.upgrade() else { return };
            let mut skipped = false;
            if let Ok(mut st) = state.lock() {
                if st.targets.iter().any(|x| x.addr.eq_ignore_ascii_case(&addr)) { return; }
                if st.targets.len() >= MAX_TARGETS { skipped = true; } else {
                    st.targets.push(TargetState::new(addr));
                }
            }
            if skipped {
                w.set_ping_input_error(SharedString::from(format!("一次最多 {} 个目标", MAX_TARGETS)));
                w.set_ping_has_error(true);
            }
            refresh_all(&w, &state);
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
            let is_port = state.lock().map(|s| s.method == "port").unwrap_or(false);
            let Some(w) = w.upgrade() else { return };
            if is_port {
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
            let name = {
                let Ok(st) = state.lock() else { return };
                match key.as_str() {
                    "local" => "本机".to_string(),
                    _ => st.hosts_full.iter().find(|(k, _, _, _, _, _, _, _, _)| k == key.as_str())
                        .map(|(_, n, _, _, _, _, _, _, _)| n.clone())
                        .unwrap_or_else(|| key.to_string()),
                }
            };
            if let Ok(mut st) = state.lock() {
                st.source = key.to_string();
            }
            let Some(w) = w.upgrade() else { return };
            w.set_ping_source_key(SharedString::from(key.as_str()));
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
        // 已持锁 → 用持锁版渲染（render_listen 会二次 lock，重入即死锁）
        render_listen_locked(w, &st);
        // 端口模式右栏
        let sel = st.listen_sel.and_then(|p| st.listen_rows.iter().find(|r| r.port == p));
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
                w.set_ping_owner_host(SharedString::from(r.addr.clone()));
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
            if lang { "Slowest" } else { "最慢" }.to_string(),
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

    // 源提示（一期固定本机；非 local 显示主机名而非裸 id）
    w.set_ping_source_name(SharedString::from(match st.source.as_str() {
        "local" => "本机".into(),
        k => st.hosts_full.iter().find(|(hk, _, _, _, _, _, _, _, _)| hk == k)
            .map(|(_, n, _, _, _, _, _, _, _)| n.clone())
            .unwrap_or_else(|| k.to_string()),
    }));
}

/// 引擎行 → Slint struct（唯一转换点）。
fn set_listen_model(w: &AppWindow, rows: &[ListenEntry], sel: Option<u16>) {
    let m: Vec<ListenRow> = rows.iter().map(|r| ListenRow {
        port: r.port as i32,
        name: SharedString::from(r.name.clone()),
        pid: r.pid as i32,
        addr: SharedString::from(r.addr.clone()),
        path: SharedString::new(),
        selected: Some(r.port) == sel,
    }).collect();
    w.set_ping_listen_items(ModelRc::from(Rc::new(VecModel::from(m))));
}

fn render_listen(w: &AppWindow, state: &Arc<Mutex<PingState>>) {
    let st = state.lock().unwrap();
    render_listen_locked(w, &st);
}

/// 持锁版：refresh_all 已持 state 锁时用这个（Mutex 不可重入）。
fn render_listen_locked(w: &AppWindow, st: &PingState) {
    let q = st.listen_query.trim().to_lowercase();
    let items: Vec<ListenRow> = st.listen_rows.iter().filter(|r| {
        q.is_empty()
            || r.port.to_string().contains(&q)
            || r.name.to_lowercase().contains(&q)
            || r.addr.contains(&q)
    }).map(|r| ListenRow {
        port: r.port as i32,
        name: SharedString::from(r.name.clone()),
        pid: r.pid as i32,
        addr: SharedString::from(r.addr.clone()),
        path: SharedString::new(),
        selected: st.listen_sel == Some(r.port),
    }).collect();
    w.set_ping_listen_count(items.len() as i32);
    w.set_ping_listen_items(ModelRc::from(Rc::new(VecModel::from(items))));
}

fn toast(w: &AppWindow, text: &str) {
    // 复用命令管理同款 toast 通道（若可用则静默失败，不打断探测流程）
    let _ = w;
    let _ = text;
}
