// (#sync-dialog 2026-10-08) 同步输入对话框的全部 Rust 回调：从 app.rs 拆出
//（app.rs 过臃肿）。装配入口 = wire_sync_input（app.rs 一行调用）。
//
// 交互模型（WindTerm 式）：
//   打开(open-dialog) → 填充 sync-sessions 模型；行级增删(add/remove-row/
//   add-all/remove-all)只改**内存模型**；确定(apply) → 一次性按槽持久化
//   （先 detach 全部涉及会话再写入，语义 = 换频道移动），并全量刷新 tab 行
//   （根 + 每窗格嵌套模型都要写，见 #tab-32-fix2）与频道状态条。
//
// (#sync-dialog-v2 2026-10-12) v2 高保真落地（docs/design/sync-dialog-hifi-
// 2026-10-12.html）：
//   · 模型打开时**一次装全所有窗口**的行（registry.for_each_id 进程内遍历，
//     单进程多窗口无需跨进程）；「当前窗口/全部窗口」作用域是纯 UI 过滤
//     （Slint 按 win-id），切换不重灌、未应用的编辑不丢。
//   · welcome tab（空 session_id）不入列：没有可同步输入的终端，且 apply
//     按 session_id 去重时空 id 会互相冲突。
//   · 行带 host/user/auth/group-name/group-color/paused/win-id：行显主机、
//     字母药丸、窗口徽标、悬浮卡全字段都从模型直读。
//   · 左栏候选数 left-count 由 recount 按作用域+排除当前频道一并算。
//   · tab_id 加 "w{win}:" 前缀 —— tab id 是每窗口的 UUID，天然唯一，前缀
//     只是可读性；同会话多开仍按 session_id 一起翻槽。
//
// (#sync-dialog-v2-fix3 2026-10-12) **左右两栏由 Rust 灌两个独立模型**
// （left-rows / right-rows），每次增删/切页签/切作用域都整模型重灌；
// 判定规则抽成纯函数 [`row_side`]。此前分栏在 Slint 侧做（0 高折叠行），
// 行数据改了但 repetition 的 height 绑定不重算 —— 用户实测「点加入只更新了
// 悬浮卡，列表没动」。换模型是 Slint 最可靠的刷新路径。
use super::*;

// (#sync-dialog-hifi 2026-10-11) 每频道成员数重算：对话框页签成员数徽标 /
// 双栏计数 / 空态都读 slot-counts。open/add/remove/add-all/remove-all 之后
// 成员数可能变化，必须重算写入。
// (#sync-dialog-v2) left-count = 左栏候选数：排除当前频道成员 + 按作用域
// （0=当前窗口只数本窗口行，1=全部窗口全数）。
//
// (#sync-dialog-v2-fix3 2026-10-12) **左/右两栏改为两个独立的模型**
// （left-rows / right-rows），不再靠 Slint 侧 `height: 0px` 折叠行来过滤。
// 原写法失效的原因：`for r in sessions: Rectangle { height: r.slot == ... }`
// —— Rust `set_row_data` 改了行数据（悬浮卡读到的频道是对的、计数也重算对了），
// 但 repetition 内**直接依赖 `r.slot` 的 height 绑定不跟着重算**，行既没从左栏
// 消失、也没在右栏出现（用户实测：点「+」只有悬浮卡变了）。改为每次增删后由
// Rust **重新灌两个新模型**：换模型是 Slint 最可靠的刷新路径，且顺带把行首
// 色点按当前编辑态频道色纠正（此前 `color` 是开框时的已生效频道色，编辑后陈旧）。
/// 一行归左栏（右栏）、另一行归右栏，还是两栏都不出现。
///
/// 纯函数判定，不碰 UI —— 见 [`sync_list_models`] 的分栏规则说明。
/// scope：0 = 当前窗口（只收本窗口行），1 = 全部窗口。
fn row_side(row_slot: i32, active_slot: i32, row_win: i32, self_win: i32, scope: i32) -> Side {
    if row_slot == active_slot {
        Side::Right
    } else if scope == 1 || row_win == self_win {
        Side::Left
    } else {
        Side::Hidden
    }
}

/// 行的归属。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
    Hidden,
}

fn sync_list_models(w: &AppWindow) {
    let g = w.global::<crate::ui::SyncInputState>();
    let rows: Vec<crate::ui::SyncSessionInfo> = g.get_sessions().iter().collect();
    let slot = g.get_slot();
    let scope = g.get_scope();
    let win_id = g.get_win_id();
    let colors = w.global::<crate::ui::Theme<'_>>().get_channel_colors();
    let mut counts = [0i32; 4];
    let mut left: Vec<crate::ui::SyncSessionInfo> = Vec::new();
    let mut right: Vec<crate::ui::SyncSessionInfo> = Vec::new();
    for mut r in rows {
        // 行首色点读 `color`：按**编辑态** slot 现算，避免开框后陈旧。
        r.color = if (0..4).contains(&r.slot) {
            colors.row_data(r.slot as usize).unwrap_or_default()
        } else {
            slint::Color::default()
        };
        if (0..4).contains(&r.slot) {
            counts[r.slot as usize] += 1;
        }
        match row_side(r.slot, slot, r.win_id, win_id, scope) {
            Side::Right => right.push(r),
            Side::Left => left.push(r),
            Side::Hidden => {}
        }
    }
    g.set_slot_counts(ModelRc::from(Rc::new(VecModel::from(counts.to_vec()))));
    g.set_left_count(left.len() as i32);
    // 作用域内的 SSH 会话总数：0 = 「根本没有 SSH 会话」，>0 且 left=0 = 「都加进来了」
    g.set_ssh_total((left.len() + right.len()) as i32);
    g.set_left_rows(ModelRc::from(Rc::new(VecModel::from(left))));
    g.set_right_rows(ModelRc::from(Rc::new(VecModel::from(right))));
}

// (#sync-dialog-v2-fix3 2026-10-12) 分栏判定的回归测试。
#[cfg(test)]
#[path = "../../tests/app/sync_dialog_split.rs"]
mod sync_split_tests;

/// 一行的会话附加信息（主机/用户/认证/组名/组色）。会话在配置里找不到
/// （内建本地终端 system:*）时主机/用户/认证/组名留空，组名留空 = 卡片
/// 不显示该行 —— 色是身份，名给不出就不硬造。
fn session_extras(
    store: &ConfigStore,
    sid: &str,
) -> (String, String, String, String, slint::Color, String) {
    let Some(sess) = store.sessions().iter().find(|sess| sess.id.as_str() == sid) else {
        return (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            slint::Color::default(),
            String::new(),
        );
    };
    let host = if sess.id.starts_with("system:") {
        String::new()
    } else if sess.port > 0 {
        format!("{}:{}", sess.host, sess.port)
    } else {
        sess.host.clone()
    };
    let builtin = sess.id.starts_with("system:");
    let group_name = if builtin {
        String::new()
    } else {
        crate::config::display_group_of(sess)
    };
    let group_color = crate::app::session_models::tab_group_color(store, sess).0;
    let note = sess.note.clone();
    (
        host,
        sess.user.clone(),
        sess.auth.as_str().to_string(),
        group_name,
        group_color,
        note,
    )
}

/// (#sync-dialog-v2) 填充模型：全部窗口的打开 tab + 离线持久成员。
///
/// (#sync-dialog-v2-fix8 2026-10-12) 行的 `win_id` 存**显示位次**（存活窗口按
/// registry id 升序的序号，与窗口标题「MTerm · 窗口 N」同源，见
/// [`super::window_ranks`]）——用户实测：关掉 1 号窗口后标题顺延成「窗口
/// 1/2」，弹框的「窗口 N」徽标与悬浮卡却还挂原始 registry id（旧 3 号窗口
/// 仍显示「窗口 3」）。`tab_id` 仍用 registry id（稳定身份：增删行回调按
/// 它匹配，位次会随开关窗漂移）。
fn fill_sync_rows(
    w: &AppWindow,
    store: &ConfigStore,
    registry: &WindowRegistry<AppWindow>,
    self_win_id: u64,
) {
    let theme = w.global::<crate::ui::Theme<'_>>();
    let colors = theme.get_channel_colors();
    let slot_color = |slot: usize| colors.row_data(slot).unwrap_or_default();
    let members = store.channel_members();
    let ranks = super::window_ranks(registry);
    let mut rows: Vec<crate::ui::SyncSessionInfo> = Vec::new();
    // (#sync-ssh-only 2026-10-22) 同步输入只支持 SSH（用户明确）：RDP/VNC 是整幅
    // 画面、SFTP 是本机文件面板、本机 shell 没有远端通道 —— 这些会话既不进候选，
    // 也不该被写进频道。广播侧（app.rs 的 send_key 路径）与 tab 频道徽标同口径。
    let is_ssh = |sid: &str| -> bool {
        store
            .get(sid)
            .map(|s| s.kind == SessionKind::Ssh)
            .unwrap_or(false)
    };
    // 打开的 tab：每个窗口各遍历一遍；welcome tab（空 session_id）跳过。
    registry.for_each_id(|win_id, win| {
        // 位次缺失（理论上不会——rank 表覆盖全部存活窗口）时退回原 id。
        let rank = ranks.get(&win_id).copied().unwrap_or(win_id) as i32;
        for t in win.get_tabs().iter() {
            let sid = t.session_id.to_string();
            if sid.is_empty() || !is_ssh(&sid) {
                continue;
            }
            let slot = members
                .iter()
                .position(|m| m.iter().any(|m| m == &sid));
            let (host, user, auth, group_name, group_color, note) =
                session_extras(store, &sid);
            let paused = store.channel_is_paused(&sid);
            rows.push(crate::ui::SyncSessionInfo {
                tab_id: format!("w{win_id}:{}", t.id).into(),
                session_id: sid.into(),
                title: t.title.clone(),
                host: host.into(),
                user: user.into(),
                auth: auth.into(),
                group_name: group_name.into(),
                group_color,
                note: note.into(),
                slot: slot.map(|i| i as i32).unwrap_or(-1),
                committed_slot: slot.map(|i| i as i32).unwrap_or(-1),
                color: slot.map(slot_color).unwrap_or_default(),
                online: true,
                paused,
                win_id: rank,
            });
        }
    });
    // 离线的持久成员：不出现在任何 tab 里，补行（置灰、标题带标注）。
    // 归属记到打开弹框的窗口（持久成员是配置级的，没有窗口归属）。
    // (#sync-dialog-v2-fix8) 同样记**位次**：与打开 tab 的行同一套编号。
    let self_rank = ranks.get(&self_win_id).copied().unwrap_or(self_win_id) as i32;
    for (slot, members) in members.iter().enumerate() {
        for m in members.iter().filter(|m| !m.is_empty()) {
            if rows.iter().any(|r| r.session_id.as_str() == m.as_str()) || !is_ssh(m) {
                continue;
            }
            let title = store
                .sessions()
                .iter()
                .find(|sess| sess.id.as_str() == m.as_str())
                .map(|sess| sess.name.to_string())
                .unwrap_or_else(|| m.to_string());
            let (host, user, auth, group_name, group_color, note) =
                session_extras(store, m);
            rows.push(crate::ui::SyncSessionInfo {
                tab_id: format!("offline:{slot}:{m}").into(),
                session_id: m.clone().into(),
                title: format!("{title}（离线）").into(),
                host: host.into(),
                user: user.into(),
                auth: auth.into(),
                group_name: group_name.into(),
                group_color,
                note: note.into(),
                slot: slot as i32,
                committed_slot: slot as i32,
                color: slot_color(slot),
                online: false,
                paused: store.channel_is_paused(m),
                win_id: self_rank,
            });
        }
    }
    let g = w.global::<crate::ui::SyncInputState>();
    g.set_sessions(ModelRc::from(Rc::new(VecModel::from(rows))));
    g.set_win_id(self_rank);
}

/// (#sync-dialog-v2-fix8 2026-10-12) 结构变化后刷新**所有打开中**的同步输入
/// 弹框（逐窗判断 `dialog-open`，只刷真开着的）。
///
/// 触发点：开/关窗口（窗口集合与位次变化）、任何窗口改频道归属（右键加入/
/// 离开/暂停、关频道、他窗 apply）。用户实测两类滞后：
///   ① 关掉 1 号窗口后，弹框仍列着已关窗口的行，且「窗口 N」徽标/悬浮卡
///      还是旧编号（标题已顺延）；
///   ② 在 2 号窗口右键把 tab 加入频道 A，1 号窗口已打开的弹框里频道状态
///      不变，要关开弹框才刷新。
///
/// 重灌时按 session_id **保留未确定的编辑**（`slot != committed-slot` 的行，
/// 含"待确认移出"的 -1）：刷新不该丢用户手里的编排。
pub(super) fn refresh_open_sync_dialogs(
    registry: &WindowRegistry<AppWindow>,
    store: &ConfigStore,
) {
    registry.for_each_id(|win_id, win| {
        let g = win.global::<crate::ui::SyncInputState>();
        if !g.get_dialog_open() {
            return;
        }
        // 未确定的编辑快照（session_id -> 编排态 slot；-1 = 待确认移出）。
        let pending: Vec<(String, i32)> = g
            .get_sessions()
            .iter()
            .filter(|r| r.slot != r.committed_slot)
            .map(|r| (r.session_id.to_string(), r.slot))
            .collect();
        fill_sync_rows(win, store, registry, win_id);
        if !pending.is_empty() {
            let model = g.get_sessions();
            if let Some(vec_model) = model
                .as_any()
                .downcast_ref::<slint::VecModel<crate::ui::SyncSessionInfo>>()
            {
                for i in 0..vec_model.row_count() {
                    if let Some(mut r) = vec_model.row_data(i) {
                        if let Some((_, slot)) =
                            pending.iter().find(|(sid, _)| sid == r.session_id.as_str())
                        {
                            r.slot = *slot;
                            vec_model.set_row_data(i, r);
                        }
                    }
                }
            }
        }
        sync_list_models(win);
        tracing::info!(
            "[SYNC_DIAG] refreshed open dialog of window {win_id} ({} pending edits kept)",
            pending.len()
        );
    });
}

pub(super) fn wire_sync_input(
    window: &AppWindow,
    store: Rc<RefCell<ConfigStore>>,
    registry: Rc<WindowRegistry<AppWindow>>,
    window_id: u64,
) {
    // ── (#sync-dialog 2026-10-08) 同步输入对话框（WindTerm 式）：打开时填充
    // 全部会话行，行级增删只改内存模型，确定时一次性按槽持久化。
    {
        let weak = window.as_weak();
        let store = store.clone();
        let registry = registry.clone();
        let self_win_id = window_id;
        window.global::<crate::ui::SyncInputState>().on_open_dialog(move || {
            tracing::info!("[SYNC_DIAG] open-dialog callback fired");
            let Some(w) = weak.upgrade() else {
                tracing::info!("[SYNC_DIAG] weak upgrade failed");
                return;
            };
            // 语言快照：对话框打开期间文案跟随当前界面语言。
            w.global::<crate::ui::SyncInputState>()
                .set_lang_en(w.get_lang_en());
            fill_sync_rows(&w, &store.borrow(), &registry, self_win_id);
            let row_count = w.global::<crate::ui::SyncInputState>()
                .get_sessions()
                .row_count();
            w.global::<crate::ui::SyncInputState>().set_slot(0);
            w.global::<crate::ui::SyncInputState>().set_scope(0);
            sync_list_models(&w);
            tracing::info!("[SYNC_DIAG] sessions filled ({} rows), set dialog-open=true", row_count);
            w.global::<crate::ui::SyncInputState>().set_dialog_open(true);
        });
    }
    {
        let weak = window.as_weak();
        {
            let weak = weak.clone();
            // (#sync-dialog-v2-fix3) 频道页签切换：置 slot 后**必须**重灌两个
            // 列表模型 —— 此前 Slint 侧直接 `SyncInputState.slot = s`，只改了
            // 一个 int，两个列表模型不会重灌，切页签后左右栏内容纹丝不动。
            window.global::<crate::ui::SyncInputState>()
                .on_pick_slot(move |s: i32| {
                    let Some(w) = weak.upgrade() else { return };
                    let g = w.global::<crate::ui::SyncInputState>();
                    g.set_slot(s.clamp(0, 3));
                    sync_list_models(&w);
                });
        }
        {
            let weak = weak.clone();
            // 作用域切换同理：当前窗口 / 全部窗口 改的是左栏候选集。
            window.global::<crate::ui::SyncInputState>()
                .on_pick_scope(move |s: i32| {
                    let Some(w) = weak.upgrade() else { return };
                    let g = w.global::<crate::ui::SyncInputState>();
                    g.set_scope(if s == 1 { 1 } else { 0 });
                    sync_list_models(&w);
                });
        }
    }
    {
        let weak = window.as_weak();
        {
            let weak = weak.clone();
            window.global::<crate::ui::SyncInputState>().on_add_row(move |tab_id: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let slot = w.global::<crate::ui::SyncInputState>().get_slot();
            let model = w.global::<crate::ui::SyncInputState>().get_sessions();
            let Some(vec_model) = model
                .as_any()
                .downcast_ref::<slint::VecModel<crate::ui::SyncSessionInfo>>()
            else {
                return;
            };
            // 成员按**会话稳定 id** 记账：同一会话多开（跨窗口也一样）的行一起翻槽。
            let mut target_sid = String::new();
            for i in 0..vec_model.row_count() {
                if let Some(mut r) = vec_model.row_data(i) {
                    if r.tab_id.as_str() == tab_id.as_str() {
                        target_sid = r.session_id.to_string();
                        r.slot = slot;
                        vec_model.set_row_data(i, r);
                    }
                }
            }
            if !target_sid.is_empty() {
                for i in 0..vec_model.row_count() {
                    if let Some(mut r) = vec_model.row_data(i) {
                        if r.session_id.as_str() == target_sid && r.slot != slot {
                            r.slot = slot;
                            vec_model.set_row_data(i, r);
                        }
                    }
                }
            }
            sync_list_models(&w);
        });
        }
        {
            let weak = weak.clone();
            window.global::<crate::ui::SyncInputState>().on_remove_row(move |tab_id: SharedString| {
            let Some(w) = weak.upgrade() else { return };
            let model = w.global::<crate::ui::SyncInputState>().get_sessions();
            let Some(vec_model) = model
                .as_any()
                .downcast_ref::<slint::VecModel<crate::ui::SyncSessionInfo>>()
            else {
                return;
            };
            let mut target_sid = String::new();
            for i in 0..vec_model.row_count() {
                if let Some(mut r) = vec_model.row_data(i) {
                    if r.tab_id.as_str() == tab_id.as_str() {
                        target_sid = r.session_id.to_string();
                        r.slot = -1;
                        vec_model.set_row_data(i, r);
                    }
                }
            }
            if !target_sid.is_empty() {
                for i in 0..vec_model.row_count() {
                    if let Some(mut r) = vec_model.row_data(i) {
                        if r.session_id.as_str() == target_sid && r.slot >= 0 {
                            r.slot = -1;
                            vec_model.set_row_data(i, r);
                        }
                    }
                }
            }
            sync_list_models(&w);
        });
        }
        {
            let weak = weak.clone();
            window.global::<crate::ui::SyncInputState>().on_add_all(move || {
            let Some(w) = weak.upgrade() else { return };
            let g = w.global::<crate::ui::SyncInputState>();
            let slot = g.get_slot();
            let scope = g.get_scope();
            let win_id = g.get_win_id();
            let model = g.get_sessions();
            let Some(vec_model) = model
                .as_any()
                .downcast_ref::<slint::VecModel<crate::ui::SyncSessionInfo>>()
            else {
                return;
            };
            // 添加全部只作用于**左栏可见候选**（排除当前频道成员 + 作用域）。
            for i in 0..vec_model.row_count() {
                if let Some(mut r) = vec_model.row_data(i) {
                    if r.slot != slot && (scope == 1 || r.win_id == win_id) {
                        r.slot = slot;
                        vec_model.set_row_data(i, r);
                    }
                }
            }
            sync_list_models(&w);
        });
        }
        {
            let weak = weak.clone();
            window.global::<crate::ui::SyncInputState>().on_remove_all(move || {
            let Some(w) = weak.upgrade() else { return };
            let slot = w.global::<crate::ui::SyncInputState>().get_slot();
            let model = w.global::<crate::ui::SyncInputState>().get_sessions();
            let Some(vec_model) = model
                .as_any()
                .downcast_ref::<slint::VecModel<crate::ui::SyncSessionInfo>>()
            else {
                return;
            };
            for i in 0..vec_model.row_count() {
                if let Some(mut r) = vec_model.row_data(i) {
                    if r.slot == slot {
                        r.slot = -1;
                        vec_model.set_row_data(i, r);
                    }
                }
            }
            sync_list_models(&w);
        });
        }
    }
    {
        let weak = window.as_weak();
        let store = store.clone();
        let registry = registry.clone();
        window.global::<crate::ui::SyncInputState>().on_apply(move || {
            let Some(w) = weak.upgrade() else { return };
            let model = w.global::<crate::ui::SyncInputState>().get_sessions();
            let rows: Vec<crate::ui::SyncSessionInfo> = model.iter().collect();
            // 每槽目标成员（按会话稳定 id 去重，行序即加入序）。
            let mut new_members: [Vec<String>; 4] = Default::default();
            let mut touched: Vec<String> = Vec::new();
            for r in &rows {
                if !touched.iter().any(|x| x == &r.session_id.as_str()) {
                    touched.push(r.session_id.to_string());
                }
                if (0..4).contains(&r.slot) {
                    let list = &mut new_members[r.slot as usize];
                    if !list.iter().any(|x| x == &r.session_id.as_str()) {
                        list.push(r.session_id.to_string());
                    }
                }
            }
            // 旧的持久成员也要 touch：被移出的会话才能从旧槽摘除。
            for members in store.borrow().channel_members().iter() {
                for m in members.iter().filter(|m| !m.is_empty()) {
                    if !touched.iter().any(|x| x == m) {
                        touched.push(m.clone());
                    }
                }
            }
            {
                let mut s = store.borrow_mut();
                for sid in &touched {
                    s.detach_all_channels(sid);
                }
                for (slot, members) in new_members.iter().enumerate() {
                    for m in members {
                        s.set_channel_member(slot, m);
                    }
                }
                let _ = s.save();
            }
            // 全量刷新 tab 行（根 + 每窗格嵌套模型都要写，见 #tab-32-fix2）。
            // (#sync-dialog-v2) 跨窗口行加入后，**每个窗口**的 tab 行与频道状态
            // 条都要刷新：遍历 registry 逐窗处理（此前只刷发起弹框的窗口，其它
            // 窗口的状态条缺失、（用户实测）。状态条模型从各窗口自身取。
            let theme = w.global::<crate::ui::Theme<'_>>();
            let colors = theme.get_channel_colors();
            let letters = ["A", "B", "C", "D"];
            let apply_rows = |tabs: &slint::ModelRc<crate::ui::TabInfo>| {
                for i in 0..tabs.row_count() {
                    if let Some(mut row) = tabs.row_data(i) {
                        let sid = row.session_id.to_string();
                        let slot = store
                            .borrow()
                            .channel_members()
                            .iter()
                            .position(|m| m.iter().any(|x| x == &sid));
                        let (letter, color) = match slot {
                            Some(idx) => (
                                letters[idx].to_string(),
                                colors.row_data(idx).unwrap_or_default(),
                            ),
                            None => (String::new(), slint::Color::default()),
                        };
                        let paused = !sid.is_empty()
                            && store.borrow().channel_is_paused(&sid);
                        row.channel_letter = letter.into();
                        row.channel_color = color;
                        row.channel_paused = paused;
                        tabs.set_row_data(i, row);
                    }
                }
            };
            registry.for_each_id(|_, win| {
                apply_rows(&win.get_tabs());
                let panes = win.get_panes();
                for pi in 0..panes.row_count() {
                    if let Some(pane) = panes.row_data(pi) {
                        apply_rows(&pane.tabs);
                    }
                }
                super::refresh_channel_bars_for_win(win, &store.borrow());
            });
            w.global::<crate::ui::SyncInputState>().set_dialog_open(false);
            // (#sync-dialog-v2-fix8) 他窗打开中的弹框也要刷新：本窗刚改掉了
            // 频道归属（用户实测：2 号窗加入频道 A，1 号窗已开弹框的频道状态
            // 不动，要关开才变）。本窗弹框上面已关，for_each 里自然跳过。
            refresh_open_sync_dialogs(&registry, &store.borrow());
            // (#sync-dialog-esc) 关框后把键盘焦点还给活动终端 —— 与 UI 侧
            // Esc/取消同一机制（terminal-focus-sequence +1，终端侧 changed
            // focus-sequence 只对 active 的终端生效）。
            let focus_seq = w.get_terminal_focus_sequence();
            w.set_terminal_focus_sequence(focus_seq + 1);
        });
    }
}
