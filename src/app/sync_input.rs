// (#sync-dialog 2026-10-08) 同步输入对话框的全部 Rust 回调：从 app.rs 拆出
//（app.rs 过臃肿）。装配入口 = wire_sync_input（app.rs 一行调用）。
//
// 交互模型（WindTerm 式）：
//   打开(open-dialog) → 填充 sync-sessions 模型（全部打开 tab + 离线持久
//   成员）；行级增删(add/remove-row/add-all/remove-all)只改**内存模型**；
//   确定(apply) → 一次性按槽持久化（先 detach 全部涉及会话再写入，
//   语义 = 换频道移动），并全量刷新 tab 行（根 + 每窗格嵌套模型都要写，
//   见 #tab-32-fix2）与频道状态条。
use super::*;

pub(super) fn wire_sync_input(
    window: &AppWindow,
    store: Rc<RefCell<ConfigStore>>,
    tabs_model: Rc<slint::VecModel<crate::ui::TabInfo>>,
    panes_model: Rc<slint::VecModel<crate::ui::PaneInfo>>,
) {
    // ── (#sync-dialog 2026-10-08) 同步输入对话框（WindTerm 式）：打开时填充
    // 全部会话行（打开 tab + 离线持久成员），行级增删只改内存模型，确定时
    // 一次性按槽持久化（先 detach 全部涉及会话再写入，语义 = 换频道移动）。
    {
        let weak = window.as_weak();
        let store = store.clone();
                window.global::<crate::ui::SyncInputState>().on_open_dialog(move || {
            tracing::info!("[SYNC_DIAG] open-dialog callback fired");
            let Some(w) = weak.upgrade() else {
                tracing::info!("[SYNC_DIAG] weak upgrade failed");
                return;
            };
            // 语言快照：对话框打开期间文案跟随当前界面语言。
            w.global::<crate::ui::SyncInputState>()
                .set_lang_en(w.get_lang_en());
            let s = store.borrow();
            let theme = w.global::<crate::ui::Theme<'_>>();
            let colors = theme.get_channel_colors();
            let slot_color = |slot: usize| colors.row_data(slot).unwrap_or_default();
            let mut rows: Vec<crate::ui::SyncSessionInfo> = Vec::new();
            // 打开的 tab：每 tab 一行（同一会话多开 = 多行，apply 按会话去重）。
            for t in w.get_tabs().iter() {
                let sid = t.session_id.to_string();
                let slot = s
                    .channel_members()
                    .iter()
                    .position(|m| m.iter().any(|m| m == &sid));
                rows.push(crate::ui::SyncSessionInfo {
                    tab_id: t.id.clone(),
                    session_id: sid.into(),
                    title: t.title.clone(),
                    slot: slot.map(|i| i as i32).unwrap_or(-1),
                    color: slot.map(slot_color).unwrap_or_default(),
                    online: true,
                });
            }
            // 离线的持久成员：不出现在任何 tab 里，补行（置灰、标题带标注）。
            for (slot, members) in s.channel_members().iter().enumerate() {
                for m in members.iter().filter(|m| !m.is_empty()) {
                    if rows.iter().any(|r| r.session_id.as_str() == m.as_str()) {
                        continue;
                    }
                    let title = s
                        .sessions()
                        .iter()
                        .find(|sess| sess.id.as_str() == m.as_str())
                        .map(|sess| sess.name.to_string())
                        .unwrap_or_else(|| m.to_string());
                    rows.push(crate::ui::SyncSessionInfo {
                        tab_id: format!("offline:{slot}:{m}").into(),
                        session_id: m.clone().into(),
                        title: format!("{title}（离线）").into(),
                        slot: slot as i32,
                        color: slot_color(slot),
                        online: false,
                    });
                }
            }
            let row_count = rows.len();
            w.global::<crate::ui::SyncInputState>().set_sessions(ModelRc::from(Rc::new(VecModel::from(rows))));
            w.global::<crate::ui::SyncInputState>().set_slot(0);
            tracing::info!("[SYNC_DIAG] sessions filled ({} rows), set dialog-open=true", row_count);
            w.global::<crate::ui::SyncInputState>().set_dialog_open(true);
        });
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
            // 成员按**会话稳定 id** 记账：同一会话多开的行一起翻槽。
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
        });
        }
        {
            let weak = weak.clone();
            window.global::<crate::ui::SyncInputState>().on_add_all(move || {
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
                    r.slot = slot;
                    vec_model.set_row_data(i, r);
                }
            }
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
        });
        }
    }
    {
        let weak = window.as_weak();
        let store = store.clone();
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
            apply_rows(&w.get_tabs());
            for pi in 0..panes_model.row_count() {
                if let Some(pane) = panes_model.row_data(pi) {
                    apply_rows(&pane.tabs);
                }
            }
            refresh_channel_bars(&w, &store.borrow(), &panes_model, &tabs_model);
            w.global::<crate::ui::SyncInputState>().set_dialog_open(false);
        });
    }
}

