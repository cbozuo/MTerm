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

use crate::config::{ConfigStore, SessionKind};
use crate::ping::{chart_paths, icmp_probe, listen_table, listen_table_remote, loss_path,
    looks_like_host, mode_from_samples, parse_draft, ping_remote, resolve, spark_path, tcp_probe,
    ListenEntry, PingState, TargetState, AUTO_INTERVAL_MS, MAX_TARGETS};
use crate::ui::{AppWindow, ListenRow, PingHostInfo, PingHostOption, PingTargetRow};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::net::IpAddr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

use super::session_runtime::resolve_jump_chain;

// ── install ─────────────────────────────────────────────────────────────
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

pub(super) fn install(window: &AppWindow, store: &Rc<std::cell::RefCell<ConfigStore>>,
    runtime: Arc<Runtime>,
) {
    let w = window.as_weak();
    let state = Arc::new(Mutex::new(PingState::new()));
    // 连续 Ping 定时器：slint::Timer 非 Send，留在 UI 线程（state 不携带）
    let auto_timer: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));

    // ── 下拉数据（2026-11-04 用户定稿）──────────────────────────────────
    // 源下拉 = 本机 + 已打开 tab 的会话（连接态动态）；＋ 下拉 = 本机 + 全部
    // 已保存会话（「本软件维护的集合」，不管有没有开 tab，key=会话 host 即
    // 探测地址）。两处行都带悬浮信息卡字段（tab 卡同款；远程源仍仅展示，
    // 真正从远端探测要三期 SSH 通道，见文件头）。proto 文案 kind_proto（模块级）。

    // HostEntry = (key, 名称, tab 连接, proto, host, user, auth, group, note,
    // **exec**=该源可执行命令（本机恒真；tab 仅 SSH 会话）——源下拉两种模式下
    // 的可点规则都靠它（Serial/Telnet 控制台流、RDP/VNC 无命令通道、Local 与
    // 本机同机重复 → 置灰不可作源；作为目标不受限）。
    type HostEntry = (String, String, bool, String, String, String, String, String, String, bool);

    /// 源下拉全量：本机 + 当前窗口已打开 tab（按会话去重）。tab 的卡片字段
    /// 直接从 TabInfo 透传（Rust 建 tab 时已填好 proto/host/user/auth/group/note）。
    fn build_hosts(window: &AppWindow, store: &Rc<std::cell::RefCell<ConfigStore>>) -> Vec<HostEntry> {
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<HostEntry> =
            vec![("local".to_string(), "本机".to_string(), false,
                  String::new(), "127.0.0.1".to_string(), String::new(), String::new(),
                  String::new(), String::new(), true)];
        for i in 0..window.get_tabs().row_count() {
            let Some(t) = window.get_tabs().row_data(i) else { continue };
            let sid = t.session_id.to_string();
            if sid.is_empty() || seen.contains(&sid) { continue; }
            seen.push(sid.clone());
            let (name, kind_ssh) = {
                let store = store.borrow();
                match store.get(&sid) {
                    Some(s) => (s.name.clone(), s.kind == SessionKind::Ssh),
                    None => (t.title.to_string(), false),
                }
            };
            out.push((sid, name, t.connected,
                t.proto.to_string(), t.host.to_string(), t.user.to_string(),
                t.auth.to_string(), t.group_name.to_string(), t.note.to_string(),
                kind_ssh));
        }
        out
    }

    fn push_hosts(window: &AppWindow, entries: &[HostEntry]) {
        let hosts: Vec<PingHostOption> = entries.iter()
            .map(|(k, name, conn, proto, host, user, auth, group, note, exec)| PingHostOption {
                key: SharedString::from(k.clone()),
                name: SharedString::from(name.clone()),
                online: k == "local" || *conn,
                proto: SharedString::from(proto.clone()),
                host: SharedString::from(host.clone()),
                user: SharedString::from(user.clone()),
                auth: SharedString::from(auth.clone()),
                group: SharedString::from(group.clone()),
                note: SharedString::from(note.clone()),
                exec: *exec,
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

    // ＋ 下拉全量：本机 + 全部已保存会话（打开下拉时重建，增删会话不陈旧）。
    // selected = key 已在清单（toggle 填入的勾选态，2026-11-04 定稿）。
    {
        let w = w.clone();
        let store = Rc::clone(store);
        let state = state.clone();
        window.on_ping_refresh_targets(move || {
            let Some(w) = w.upgrade() else { return };
            let store = store.borrow();
            rebuild_target_opts(&w, &store, &state, w.get_lang_en());
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
                    .filter(|(k, name, _, _, _, _, _, _, _, _)| {
                        q.is_empty()
                            || name.to_lowercase().contains(&q)
                            || k.to_lowercase().contains(&q)
                    })
                    .map(|(k, name, conn, proto, host, user, auth, group, note, exec)| PingHostOption {
                        key: SharedString::from(k.clone()),
                        name: SharedString::from(name.clone()),
                        online: k == "local" || *conn,
                        proto: SharedString::from(proto.clone()),
                        host: SharedString::from(host.clone()),
                        user: SharedString::from(user.clone()),
                        auth: SharedString::from(auth.clone()),
                        group: SharedString::from(group.clone()),
                        note: SharedString::from(note.clone()),
                        exec: *exec,
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
            w.set_ping_probe_error(SharedString::new());
            w.set_ping_listen_count(0);
            w.set_ping_listen_selected(0);
            refresh_all(&w, &state);
        });
    }

    // 打开弹框：默认探测源 = 当前 tab 的会话（2026-11-05 用户定稿）——
    // 仅当该 tab **已连接**且会话是 SSH（可作源的类型）时采用；断线 / 未打开 /
    // 非 SSH（RDP/VNC/串口/本地终端）一律回落「本机」。
    {
        let w = w.clone();
        let state = state.clone();
        let store = store.clone();
        window.on_ping_open(move || {
            let Some(w) = w.upgrade() else { return };
            let active_id = w.get_active_tab_id().to_string();
            // 找活动 tab 的会话 id（connected && state==1 才算在线）
            let active_sid = {
                let tabs = w.get_tabs();
                let mut sid = String::new();
                for i in 0..tabs.row_count() {
                    let Some(t) = tabs.row_data(i) else { continue };
                    if t.id.as_str() == active_id.as_str()
                        && t.connected && t.state == 1 && !t.session_id.is_empty() {
                        sid = t.session_id.to_string();
                    }
                }
                sid
            };
            let usable = !active_sid.is_empty()
                && store.borrow().get(&active_sid)
                    .map(|s| s.kind == SessionKind::Ssh)
                    .unwrap_or(false);
            // 源下拉数据同步刷新（tab 列表可能已变）
            let hosts = build_hosts(&w, &store);
            let (key, name) = if usable {
                let name = hosts.iter().find(|(k, ..)| *k == active_sid)
                    .map(|(_, n, ..)| n.clone())
                    .unwrap_or_else(|| active_sid.clone());
                (active_sid.clone(), name)
            } else {
                ("local".to_string(), if w.get_lang_en() { "Local".to_string() } else { "本机".to_string() })
            };
            if let Ok(mut st) = state.lock() {
                st.source = key.clone();
                st.hosts_full = hosts.clone();
            }
            push_hosts(&w, &hosts);
            w.set_ping_source_key(SharedString::from(key));
            w.set_ping_source_name(SharedString::from(name));
            refresh_all(&w, &state);
        });
    }

    // 方式切换
    {
        let w = w.clone();
        let state = state.clone();
        let store = store.clone();
        let runtime = runtime.clone();
        window.on_ping_set_method(move |m| {
            if let Ok(mut st) = state.lock() {
                st.method = m.to_string();
            }
            let Some(w) = w.upgrade() else { return };
            w.set_ping_method(SharedString::from(m.as_str()));
            // 换模式等于换视图：上一模式的探测错误不再适用
            w.set_ping_probe_error(SharedString::new());
            if m == "port" {
                // 按探测源取端口表（本机=本地 API；SSH tab=远端 ss/netstat，
                // 异步）。fetch_listen 自带全部路径的 refresh_all
                fetch_listen(&w, &store, &state, &runtime);
                return;
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
            // 告警显隐收口：有错报错、超限提示保留，正常加入清掉旧告警
            //（2026-11-05 用户反馈「重新输入后告警应同步消失」）
            w.set_ping_has_error(skipped > 0);
            w.set_ping_input_error(SharedString::from(if skipped > 0 {
                format!("一次最多 {} 个目标 · 已跳过 {} 个", MAX_TARGETS, skipped)
            } else { String::new() }));
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

    // 草稿重新输入时撤告警（2026-11-05 用户反馈：报错后改输入，页脚红字同步消失）
    {
        let w = w.clone();
        window.on_ping_clear_error(move || {
            if let Some(w) = w.upgrade() { w.set_ping_has_error(false); }
        });
    }

    // ＋ 下拉行点击 / 清单勾选框 = 参与/不参与切换（2026-11-05 用户定稿：
    // 空框=不参与但**行保留置灰**，不再直接移除；勾选态同步下拉模型）
    {
        let w = w.clone();
        let state = state.clone();
        let store = Rc::clone(store);
        window.on_ping_toggle_target(move |addr| {
            let addr = addr.trim().to_string();
            if addr.is_empty() { return; }
            let Some(w) = w.upgrade() else { return };
            let mut skipped = false;
            if let Ok(mut st) = state.lock() {
                if let Some(t) = st.targets.iter_mut().find(|x| x.addr.eq_ignore_ascii_case(&addr)) {
                    t.enabled = !t.enabled;   // 勾/空切换，行保留
                } else if st.targets.len() >= MAX_TARGETS {
                    skipped = true;
                } else {
                    st.targets.push(TargetState::new(addr));
                }
            }
            if skipped {
                w.set_ping_input_error(SharedString::from(format!("一次最多 {} 个目标", MAX_TARGETS)));
                w.set_ping_has_error(true);
            }
            refresh_all(&w, &state);
            // 下拉勾选态同步：勾选来自清单 enabled 成员关系，重建下拉模型即可
            rebuild_target_opts(&w, &store.borrow(), &state, w.get_lang_en());
        });
    }

    // 表头全选（2026-11-05）：全开→全关；混合/全关→全开（Rust 按 state 裁决，
    // Slint 只报点击）。勾选态同步 ＋ 下拉模型（与 toggle-target 同套）
    {
        let w = w.clone();
        let state = state.clone();
        let store = Rc::clone(store);
        window.on_ping_toggle_all_targets(move || {
            if let Ok(mut st) = state.lock() {
                let all_on = st.targets.iter().all(|t| t.enabled);
                let next = !all_on;
                for t in &mut st.targets { t.enabled = next; }
            }
            let Some(w) = w.upgrade() else { return };
            refresh_all(&w, &state);
            rebuild_target_opts(&w, &store.borrow(), &state, w.get_lang_en());
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

    // 探测 / 刷新（连续 Ping 定时器随探测启动——2026-11-04 用户定稿：
    // 开关只切状态不触发，点「探测」才跑）
    {
        let w = w.clone();
        let state = state.clone();
        let store = store.clone();
        let runtime = runtime.clone();
        let auto_timer = auto_timer.clone();
        window.on_ping_run(move || {
            let is_port = state.lock().map(|s| s.method == "port").unwrap_or(false);
            let Some(w) = w.upgrade() else { return };
            if is_port {
                // 按探测源取端口表（本机同步 / 远端异步），含刷新中反馈与守卫
                fetch_listen(&w, &store, &state, &runtime);
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
                .map(|s| s.targets.iter().filter(|t| t.enabled)
                    .map(|t| t.addr.clone()).collect())
                .unwrap_or_default();
            if targets.is_empty() {
                w.set_ping_has_error(true);
                w.set_ping_input_error(SharedString::from("请至少勾选一个目标"));
                return;
            }
            w.set_ping_has_error(false);
            // 新一轮探测：清掉上一轮的探测类错误（probe-error 只属于上次轮次）
            w.set_ping_probe_error(SharedString::new());
            // 连续 Ping 开着：确保定时器在跑（开关切换不启动定时器，这里随
            // 探测启动；重复 start 是幂等重置——重置节拍从本次点击起算）
            if w.get_ping_auto_on() { start_auto_timer(&w, &state, &auto_timer, &store, &runtime); }
            start_round(&w, &state, &store, &runtime, targets);
        });
    }

    // 连续 Ping（开关只切状态不触发，2026-11-04 用户定稿：切换瞬间不探测、
    // 不起定时器；定时器随「探测」按钮启动/重置——探测期间开关关闭则停表）
    {
        let w = w.clone();
        let state = state.clone();
        window.on_ping_toggle_auto(move |on| {
            let Some(w) = w.upgrade() else { return };
            w.set_ping_auto_on(on);
            if let Ok(mut st) = state.lock() { st.auto_on = on; }
            if !on {
                // 关闭即停表（在飞的那一轮照常落地）
                *auto_timer.borrow_mut() = None;
            }
            // 开启时不做任何探测动作（含定时器）——下一次点「探测」时启动
        });
    }

    /// 连续 Ping 定时器（随「探测」启动；Repeated 2s，每跳探测一轮）。
    /// 供 on_ping_run 调用；独立函数避免闭包内再借 auto_timer（RefCell 冲突）。
    fn start_auto_timer(
        w: &AppWindow,
        state: &Arc<Mutex<PingState>>,
        auto_timer: &Rc<RefCell<Option<slint::Timer>>>,
        store: &Rc<RefCell<ConfigStore>>,
        runtime: &Arc<Runtime>,
    ) {
        let timer = slint::Timer::default();
        let w2 = w.as_weak();
        let state2 = state.clone();
        let store = store.clone();
        let runtime = runtime.clone();
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(AUTO_INTERVAL_MS), move || {
            let Some(w) = w2.upgrade() else { return };
            if !w.get_ping_dialog_open() || !w.get_ping_auto_on() { return; }
            // 轮次在飞 / 弹框关闭中：跳过这一跳（round_active 防重入在
            // start_round 里兜底，双保险）
            if w.get_ping_testing() { return; }
            let targets: Vec<String> = match state2.lock() {
                Ok(st) => {
                    // 远端源连续 Ping 禁用（每 2s 重连 SSH 不可接受，持久通道
                    // 后续优化）——开关已置灰，这里双保险
                    if st.method != "icmp" || st.source != "local" { return; }
                    st.targets.iter().filter(|t| t.enabled)
                        .map(|t| t.addr.clone()).collect()
                }
                Err(_) => return,
            };
            if targets.is_empty() { return; }
            start_round(&w, &state2, &store, &runtime, targets);
        });
        *auto_timer.borrow_mut() = Some(timer);
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
            // 清结果同时撤探测错误（错误属于刚被清掉的那轮结果）
            w.set_ping_probe_error(SharedString::new());
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

    // 源切换（本机 / SSH tab；两种模式语义一致：数据从所选源来）
    {
        let state = state.clone();
        let store = store.clone();
        let runtime = runtime.clone();
        let w = window.as_weak();
        window.on_ping_set_source(move |key| {
            let name = {
                let Ok(st) = state.lock() else { return };
                match key.as_str() {
                    "local" => "本机".to_string(),
                    _ => st.hosts_full.iter().find(|(k, _, _, _, _, _, _, _, _, _)| k == key.as_str())
                        .map(|(_, n, _, _, _, _, _, _, _, _)| n.clone())
                        .unwrap_or_else(|| key.to_string()),
                }
            };
            if let Ok(mut st) = state.lock() {
                st.source = key.to_string();
                // 结果反映「源 → 目标」链路：源变了旧结果全部作废（2026-11-05
                // 用户反馈「切换源还保持上一个的样子」）。清采样回 idle，清单
                // 翻「待测」、右栏/汇总随 idle 重渲染；选中保留（聚焦不跳）。
                for t in &mut st.targets {
                    t.samples.clear();
                    t.mode = "idle".into();
                }
            }
            let Some(w) = w.upgrade() else { return };
            w.set_ping_source_key(SharedString::from(key.as_str()));
            w.set_ping_source_name(SharedString::from(name));
            // 换源等于换链路：上次源的探测错误不再适用
            w.set_ping_probe_error(SharedString::new());
            // 端口模式：切源即重取该源的端口表（远端异步）；Ping 模式仅重渲染
            let is_port = state.lock().map(|s| s.method == "port").unwrap_or(false);
            if is_port {
                fetch_listen(&w, &store, &state, &runtime);
            } else {
                refresh_all(&w, &state);
            }
        });
    }
}

// ── 探测轮次 ────────────────────────────────────────────────────────────
fn start_round(w: &AppWindow, state: &Arc<Mutex<PingState>>, store: &Rc<RefCell<ConfigStore>>,
    runtime: &Arc<Runtime>, targets: Vec<String>,
) {
    // 轮次标记（2026-11-04 用户定稿「清单一直在跳动」根因修复：探测期间
    // 行/右栏**保持上次数据**——不再把目标集体翻成 testing 模式渲染成
    // 「测试中/—」再翻回来；按钮文字用 round_active 渲染「测试中…」做在飞
    // 反馈。防重入：轮次在飞时直接忽略新触发。）
    // 顺带记下发起源/方式：落地时已切换则本轮作废（结果反映发起源链路，
    // 2026-11-05 源切换清结果配套——不然切源瞬间在飞的旧源结果照落）。
    let src_at_start = {
        let Ok(mut st) = state.lock() else { return };
        if st.round_active { return; }
        st.round_active = true;
        (st.source.clone(), st.method.clone())
    };
    // 探测源为 SSH tab → 远端真实探测：在源主机执行 ping 并解析（tokio
    // 并发限 4，避免 12 目标打满 sshd MaxStartups）
    let remote = if src_at_start.0 == "local" {
        None
    } else {
        let sess = store.borrow().get(&src_at_start.0).cloned();
        sess.filter(|s| s.kind == SessionKind::Ssh).map(|s| {
            let jumps = resolve_jump_chain(store, &s);
            (s, jumps)
        })
    };
    refresh_all(w, state);
    let w2 = w.as_weak();
    let state2 = state.clone();
    match remote {
        Some((session, jumps)) => {
            runtime.spawn(async move {
                let sem = Arc::new(tokio::sync::Semaphore::new(4));
                let mut tasks = Vec::new();
                for addr in targets {
                    let permit = Arc::clone(&sem);
                    let sess = session.clone();
                    let jp = jumps.clone();
                    tasks.push(tokio::spawn(async move {
                        let _permit = permit.acquire_owned().await;
                        let res = ping_remote(sess, jp, &addr).await;
                        (addr, res)
                    }));
                }
                let mut results = Vec::new();
                for t in tasks {
                    if let Ok(r) = t.await { results.push(r); }
                }
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(w) = w2.upgrade() else { return };
                    let mut done = 0usize;
                    let mut err_first = String::new();
                    let mut err_all = true;
                    if let Ok(mut st) = state2.lock() {
                        if st.source == src_at_start.0 && st.method == src_at_start.1 {
                            let cur_sel: Option<String> = st.selected_addr().map(|s| s.to_string());
                            for (addr, res) in results {
                                if let Some(t) = st.targets.iter_mut().find(|t| t.addr == addr) {
                                    match res {
                                        Ok(samples) => {
                                            t.samples = samples.clone();
                                            t.mode = mode_from_samples(&samples);
                                            err_all = false;
                                        }
                                        Err(e) => {
                                            t.samples = vec![None; 3];
                                            t.mode = "offline".into();
                                            if err_first.is_empty() { err_first = e; }
                                        }
                                    }
                                    t.selected = cur_sel.as_deref() == Some(&t.addr);
                                }
                                done += 1;
                            }
                        }
                        st.round_active = false;
                    }
                    refresh_all(&w, &state2);
                    if err_all {
                        // 探测类错误 → 只进页脚（probe-error），不染草稿输入框
                        w.set_ping_probe_error(SharedString::from(if err_first.is_empty() {
                            "探测失败：无返回".to_string()
                        } else { format!("远端探测失败：{err_first}") }));
                    } else if done > 0 {
                        w.set_ping_probe_error(SharedString::new());
                        toast(&w, &format!("Ping 完成 · {} 个目标", done));
                    }
                });
            });
        }
        None => {
            std::thread::spawn(move || {
                let mut results: Vec<(String, TargetState)> = Vec::new();
                for addr in &targets {
                    let ip = resolve(addr);
                    let mut ts = TargetState::new(addr.clone());
                    ts.ip = ip;
                    match ip {
                        Some(IpAddr::V4(v4)) => {
                            let (pts, all_lost) = icmp_probe(v4);
                            // 与远端 ping 同一套判定（mode_from_samples）
                            ts.mode = if all_lost { "offline".into() } else { mode_from_samples(&pts) };
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
                        if st.source == src_at_start.0 && st.method == src_at_start.1 {
                            let cur_sel: Option<String> = st.selected_addr().map(|s| s.to_string());
                            for (addr, ts) in results {
                                if let Some(t) = st.targets.iter_mut().find(|t| t.addr == addr) {
                                    *t = ts;
                                    t.selected = cur_sel.as_deref() == Some(&t.addr);
                                }
                                done += 1;
                            }
                        }
                        st.round_active = false;
                    }
                    refresh_all(&w, &state2);
                    if done > 0 { toast(&w, &format!("Ping 完成 · {} 个目标", done)); }
                });
            });
        }
    }
}

// ── 端口表按源拉取 ─────────────────────────────────────────────────────
/// 源 key → 显示名（无锁环境用；refresh_all 持锁期间用内联版）
fn source_display(state: &Arc<Mutex<PingState>>, key: &str) -> String {
    let st = state.lock().unwrap();
    match key {
        "local" => (if st.lang_en { "Local" } else { "本机" }).to_string(),
        k => st.hosts_full.iter()
            .find(|(hk, ..)| hk == k)
            .map(|(_, n, ..)| n.clone())
            .unwrap_or_else(|| k.to_string()),
    }
}

/// 端口模式按探测源取监听端口表：本机=GetExtendedTcpTable 同步快照（<100ms）；
/// SSH tab=远端执行 ss/netstat（异步 10s）。结果落地守卫=(source, method) 不变
/// （探测中途切源/切模式则丢弃）；连接失败（主机密钥未信任/认证失败/超时）
/// 进页脚错误槽。全路径自带 refresh_all。
fn fetch_listen(
    w: &AppWindow,
    store: &Rc<RefCell<ConfigStore>>,
    state: &Arc<Mutex<PingState>>,
    runtime: &Arc<Runtime>,
) {
    let source = state.lock().ok().map(|s| s.source.clone()).unwrap_or_default();
    if source == "local" {
        let rows = listen_table();
        if let Ok(mut s) = state.lock() {
            // 存入 state：refresh_all → render_listen_locked 从 st.listen_rows
            // 渲染，不存会被空表覆盖成 0 行
            s.listen_rows = rows.clone();
        }
        w.set_ping_listen_count(rows.len() as i32);
        set_listen_model(w, &rows, None);
        refresh_all(w, state);
        return;
    }
    // 远端：会话 + 跳板链（非 SSH 源在源下拉已置灰，这里防御性兜底）。
    // 以下错误都是探测/源问题 → probe-error（页脚红字），不染草稿输入框
    let sess = store.borrow().get(&source).cloned();
    let Some(sess) = sess else {
        w.set_ping_probe_error(SharedString::from(format!("源「{source}」不是已保存会话")));
        refresh_all(w, state);
        return;
    };
    if sess.kind != SessionKind::Ssh {
        w.set_ping_probe_error(SharedString::from(
            format!("「{}」不支持端口枚举（仅 SSH 会话可作为源）", sess.name)));
        refresh_all(w, state);
        return;
    }
    let jumps = resolve_jump_chain(store, &sess);
    let src_name = source_display(state, &source);
    w.set_ping_testing(true);
    refresh_all(w, state); // 立即反馈「刷新中…」
    let w2 = w.as_weak();
    let state2 = state.clone();
    runtime.clone().spawn(async move {
        let res = listen_table_remote(sess, jumps).await;
        let _ = slint::invoke_from_event_loop(move || {
            let Some(w) = w2.upgrade() else { return };
            let valid = state2.lock().ok()
                .map(|st| st.source == source && st.method == "port")
                .unwrap_or(false);
            if valid {
                match res {
                    Ok(rows) => {
                        if let Ok(mut st) = state2.lock() { st.listen_rows = rows; }
                        w.set_ping_probe_error(SharedString::new());
                    }
                    Err(e) => {
                        w.set_ping_probe_error(SharedString::from(
                            format!("无法连接「{src_name}」：{e}")));
                    }
                }
            }
            w.set_ping_testing(false);
            refresh_all(&w, &state2);
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
            enabled: t.enabled,
        });
    }
    w.set_ping_targets(ModelRc::from(Rc::new(VecModel::from(rows))));

    // 发起源显示名（src_disp）：owner 解析行前缀 + 源 chip 共用——两种模式的
    // 数据都真实来自所选源（端口=远端枚举、Ping=远端执行，2026-11-05 做真），
    // 「本机 · ip」/「tab名 · ip」/端口模式源标签都按它显示
    let src_disp = if st.source == "local" {
        if lang { "Local".to_string() } else { "本机".to_string() }
    } else {
        st.hosts_full.iter().find(|(hk, _, _, _, _, _, _, _, _, _)| *hk == st.source)
            .map(|(_, n, _, _, _, _, _, _, _, _)| n.clone())
            .unwrap_or_else(|| st.source.clone())
    };

    if st.method == "port" {
        // 已持锁 → 用持锁版渲染（render_listen 会二次 lock，重入即死锁）。
        // 源 chip/结果头标签按 src_disp 显示（远端枚举做真后数据即来自所选源）
        w.set_ping_source_name(SharedString::from(src_disp.clone()));
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

    // 在飞反馈（2026-11-04 用户定稿：探测期间行/右栏保持上次数据，不集体
    // 翻「测试中」；只把按钮文字换成「测试中…」+ 禁点）
    let testing = st.round_active;
    w.set_ping_testing(testing);
    // has_any / 计数只看参与探测的行（空框=不参与，2026-11-05 定稿）
    let has_any = st.targets.iter().any(|t| t.enabled && t.mode != "idle");
    let enabled_count = st.targets.iter().filter(|t| t.enabled).count();
    // 表头全选三态用（Slint 按 enabled==target-count 判全勾）
    w.set_ping_enabled_count(enabled_count as i32);
    // 空态：无选中、无数据 → 图表占位框/三格统计整块隐藏（右栏不再悬着
    // 「—」孤块和空占位框，2026-11-04 用户反馈截图）
    w.set_ping_empty_view(!has_any);

    let (rtt_label, rtt_value, rtt_unit, q_text, q_tone, q_hint, title, chart_on, chart_src, loss_src, owner, stats): (
        String, String, String, String, String, String, String, bool, Vec<Option<f64>>, String, Option<(String, String)>, [String; 6]
    ) = if let Some(addr) = &sel_addr {
        let Some(t) = st.targets.iter().find(|t| &t.addr == addr) else {
            return;
        };
        // 未探测的目标（新增/源切换清空后）：右栏保持「待测」中性态。不拦
        // 的话会落进下面按 mode 推导的尾部——idle 被渲染成「优秀/连接稳定」+
        // 上一个目标的解析卡残留（2026-11-05 用户反馈「新增即优秀」）。
        if t.mode == "idle" {
            (
                (if lang { "Current RTT" } else { "当前 RTT" }).to_string(),
                "—".to_string(),
                "ms".to_string(),
                (if lang { "Idle" } else { "待测" }).to_string(),
                "neutral".to_string(),
                (if lang { "Not probed yet" } else { "尚未探测" }).to_string(),
                t.addr.clone(),
                false,
                Vec::new(),
                String::new(),
                None,
                [
                    (if lang { "Avg" } else { "平均" }).to_string(),
                    "—".into(),
                    (if lang { "Jitter" } else { "抖动" }).to_string(),
                    "—".into(),
                    (if lang { "Loss" } else { "丢包" }).to_string(),
                    "—".into(),
                ],
            )
        } else {
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
            Some((t.addr.clone(), format!("{} · {}", src_disp, t.ip.map(|i| i.to_string()).unwrap_or_else(|| t.addr.clone())))),
            stats,
        )
        }
    } else if has_any {
        // 汇总：最慢 + 最快 + 平均抖动 + 丢包目标
        let mut worst_v: Option<f64> = None;
        let mut fast_v: Option<f64> = None;
        let mut bad_n = 0usize;
        let mut loss_n = 0usize;
        let mut jit_sum = 0.0f64;
        let mut jit_n = 0usize;
        let mut worst_name = String::new();
        let mut idle_n = 0usize;
        for t in &st.targets {
            if !t.enabled { continue; }   // 空框行不进汇总
            if t.mode == "idle" { idle_n += 1; }
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
        // 图表与「最慢」数字**同源**（2026-11-05 用户问「全部态图示怎么展示」）：
        // 曾用 worst_target()——它偏袒 offline 目标当最慢，最慢是超时目标时采样
        // 全空、图表只剩红丢包刻度近乎空白，与「最慢 14ms」数字（只统计有响应
        // 目标）张冠李戴。改为取循环跟踪的最慢（非 offline）目标采样；全 offline
        // 时回退到任一有采样目标（红刻度=丢包史仍有意义），真没有则图表隐藏。
        let chart_src = st.targets.iter().find(|t| t.addr == worst_name)
            .map(|t| t.samples.clone())
            .unwrap_or_else(|| st.targets.iter()
                .find(|t| t.enabled && !t.samples.is_empty())
                .map(|t| t.samples.clone())
                .unwrap_or_default());
        (
            if lang { "Slowest" } else { "最慢" }.to_string(),
            worst_v.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
            "ms".to_string(),
            if bad_n > 0 { format!("{} {}", bad_n, if lang { "bad" } else { "个异常" }) }
                else if idle_n > 0 { format!("{} {}", idle_n, if lang { "pending" } else { "个待测" }) }
                else { if lang { "All normal" } else { "全部正常" }.to_string() },
            if bad_n > 0 { "fair".to_string() } else if idle_n > 0 { "neutral".to_string() } else { "good".to_string() },
            format!("{} {} · {} {}",
                enabled_count, if lang { "targets" } else { "个目标" },
                if lang { "slowest" } else { "最慢" },
                if worst_name.is_empty() { "—".into() } else { worst_name.clone() }),
            format!("{} {}", enabled_count, if lang { "targets" } else { "个目标" }),
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

    // 页脚（testing=round_active，上面已下发；摘要保持上次结果文案——
    // 在飞时不再整条换「正在探测…」，只有按钮文字给在飞反馈）
    w.set_ping_run_text(SharedString::from(if testing {
        (if lang { "Testing…" } else { "测试中…" }).into()
    } else { format!("探测 {} 个", enabled_count) }));
    // 连续 Ping 远端源禁用（每 2s 重连 SSH 不可接受，持久通道后续优化）——
    // 开关置灰 + 定时器 tick 双保险
    let auto_disabled = testing || !has_any
        || (st.method == "icmp" && st.source != "local");
    w.set_ping_auto_disabled(auto_disabled);
    let idle_n = st.targets.iter().filter(|t| t.enabled && t.mode == "idle").count();
    let summary = if st.targets.is_empty() {
        (if lang { "No targets" } else { "勾选目标，或在上方输入 IP 后加入" }).to_string()
    } else if enabled_count == 0 {
        (if lang { "No enabled targets" } else { "全部未勾选 · 点行首空框重新参与" }).to_string()
    } else {
        let bad = st.targets.iter().filter(|t| t.enabled && (t.mode == "offline" || t.mode == "degraded")).count();
        if lang {
            format!("{} targets · {}", enabled_count,
                if bad > 0 { format!("{} bad", bad) }
                else if idle_n > 0 { format!("{} pending", idle_n) }
                else { "all normal".into() })
        } else {
            format!("{} 个目标 · {}", enabled_count,
                if bad > 0 { format!("{} 个异常", bad) }
                else if idle_n > 0 { format!("{} 个待测", idle_n) }
                else { "全部正常".into() })
        }
    };
    w.set_ping_summary(SharedString::from(summary));

    // 源 chip 显示名（src_disp 上面已算好——本地引擎一期，UI 语义按所选源）
    w.set_ping_source_name(SharedString::from(src_disp));
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

// ── ＋ 下拉模型重建 ─────────────────────────────────────────────────────
/// 全量重建 ＋ 下拉（本机 + 全部已保存会话），selected = key 已在清单。
/// 打开下拉（on_ping_refresh_targets）与 toggle 填入（on_ping_add_target）
/// 共用——勾选态的唯一来源是清单成员关系。
fn rebuild_target_opts(
    w: &AppWindow,
    store: &std::cell::Ref<ConfigStore>,
    state: &Arc<Mutex<PingState>>,
    lang_en: bool,
) {
    // 勾选 = 参与探测的行（空框=不参与，2026-11-05 定稿）
    let in_list: Vec<String> = state.lock().ok()
        .map(|st| st.targets.iter().filter(|t| t.enabled)
            .map(|t| t.addr.clone()).collect())
        .unwrap_or_default();
    let is_sel = |k: &str| in_list.iter().any(|a| a.eq_ignore_ascii_case(k));
    // 探测源的地址（2026-11-05 用户反馈）：源主机自身不进候选——自己 ping
    // 自己无意义。按**地址**剔（不只按会话 id）：同 host 的重复会话（如
    // 188 / 188 (1)）一并剔掉；源=本机时剔掉本机行与任何指向 127.0.0.1 的会话。
    let source_host: String = state.lock().ok().map(|st| {
        if st.source == "local" {
            "127.0.0.1".to_string()
        } else {
            store.sessions().iter()
                .find(|s| s.id == st.source)
                .map(|s| s.host.clone())
                .unwrap_or_default()
        }
    }).unwrap_or_default();
    let is_source_host = |host: &str| {
        !source_host.is_empty() && host.trim().eq_ignore_ascii_case(source_host.trim())
    };
    let mut opts: Vec<PingHostInfo> = Vec::new();
    if !is_source_host("127.0.0.1") {
        opts.push(PingHostInfo {
            key: "127.0.0.1".into(), name: if lang_en { "Local" } else { "本机" }.into(),
            proto: kind_proto(crate::config::SessionKind::Local, lang_en).into(),
            host: "127.0.0.1".into(),
            user: "".into(), auth: "".into(), group: "".into(), note: "".into(),
            selected: is_sel("127.0.0.1"),
        });
    }
    for s in store.sessions() {
        if s.host.trim().is_empty() { continue; }
        if is_source_host(&s.host) { continue; }
        let group = crate::config::display_group_of(s);
        opts.push(PingHostInfo {
            key: SharedString::from(s.host.clone()),
            name: SharedString::from(s.name.clone()),
            proto: SharedString::from(kind_proto(s.kind, lang_en)),
            host: SharedString::from(s.host.clone()),
            user: SharedString::from(s.user.clone()),
            auth: SharedString::from(s.auth.as_str()),
            group: SharedString::from(group),
            note: SharedString::from(s.note.clone()),
            selected: is_sel(&s.host),
        });
    }
    w.set_ping_target_opts(ModelRc::from(Rc::new(VecModel::from(opts))));
}
