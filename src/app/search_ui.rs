//! (#double-shift-v1 2026-11-11) 全局搜索的**宿主桥**：Slint 回调接线、
//! 索引/查询驱动、执行语义、双击 Shift 触发器状态。
//!
//! 领域层在 `src/search/`（纯逻辑，无 UI）；本文件是它与 AppWindow 的
//! 唯一接触面 —— 对齐 `src/app/command_hint_ui.rs` 的分工。
//!
//! 状态模型（全部挂 AppWindow 根属性 `qs-*`，Rust 只在回调里重灌）：
//!   qs-rows    扁平行模型（kind 0=组头 / 1=行；qi = 扁平结果索引）
//!   qs-hi      选中行（跳组头由 Rust 维护，Slint 只读）
//!   qs-cat     分类 0 全部 / 1 会话 / 2 命令 / 3 设置 / 4 动作
//!   qs-preview 预览区整块（选中变化即重灌）
//!
//! 执行语义（高保真 §03「Enter / Ctrl+Enter」表）：
//!   会话   未打开：连接（新标签）；已打开：Enter 切换标签，Ctrl+Enter
//!          复制 SSH 命令。
//!   命令   Enter 发送到当前会话；Ctrl+Enter 只填入命令栏；无活动终端
//!          降级为复制命令。
//!   设置   Enter 打开设置到对应页；Ctrl+Enter 复制路径。
//!   动作   Enter 执行；Ctrl+Enter 轻提示快捷键。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::{ComponentHandle as _, Model as _, ModelRc, SharedString, VecModel};

use crate::config::ConfigStore;
use crate::search::{
    build_index, group_and_cap, recent_items, search, Category, DoubleShiftTracker,
    GroupedMatches, SearchIndex, SearchItem, SessionView,
};
use crate::ui::{AppWindow, QsMeta, QsPreview, QsRow};

/// 分类 int → 过滤器（0 = 全部）。
fn cat_from_int(v: i32) -> Option<Category> {
    match v {
        1 => Some(Category::Session),
        2 => Some(Category::Command),
        3 => Some(Category::Setting),
        4 => Some(Category::Action),
        _ => None,
    }
}

fn category_int(cat: Category) -> i32 {
    match cat {
        Category::Session => 1,
        Category::Command => 2,
        Category::Setting => 3,
        Category::Action => 4,
    }
}

/// 复制到剪贴板 —— `clipboard_set_text` 只能在子线程调（arboard 约束）。
fn copy_async(text: String) {
    std::thread::Builder::new()
        .name("qs-copy".into())
        .spawn(move || crate::app::clipboard_set_text(text))
        .ok();
}

/// 两次查询之间驻留的快照：索引 + 当前扁平结果（执行/预览按 qi 回查）。
#[derive(Default)]
struct QsState {
    index: SearchIndex,
    flat: Vec<SearchItem>,
}

/// 进程内的最近搜索 / 最近使用（v1 不持久化；高保真「UI 层」口径）。
#[derive(Default)]
struct QsRecent {
    searches: Vec<String>,
    ids: Vec<String>,
}

impl QsRecent {
    fn remember_search(&mut self, term: &str) {
        let term = term.trim();
        if term.is_empty() {
            return;
        }
        self.searches.retain(|s| s != term);
        self.searches.insert(0, term.to_string());
        self.searches.truncate(4);
    }
    fn remember_item(&mut self, id: &str) {
        self.ids.retain(|s| s != id);
        self.ids.insert(0, id.to_string());
        self.ids.truncate(8);
    }
    /// 最近使用排名（1 = 最新）。
    fn rank_map(&self) -> std::collections::HashMap<String, i32> {
        self.ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), (i + 1) as i32))
            .collect()
    }
}

/// 已打开会话 id 集合（窗口 tabs 按 session_id 聚合；断开的 tab 仍算
/// 「已打开」—— 动作是「切换到标签」，断开标签同样可切）。
fn open_session_ids(window: &AppWindow) -> std::collections::HashSet<String> {
    let tabs = window.get_tabs();
    let mut out = std::collections::HashSet::new();
    for i in 0..tabs.row_count() {
        if let Some(t) = tabs.row_data(i) {
            if !t.session_id.is_empty() {
                out.insert(t.session_id.to_string());
            }
        }
    }
    out
}

/// 组头行。
fn group_head(rows: &mut Vec<QsRow>, title: String) {
    rows.push(QsRow {
        kind: 0,
        cat: 0,
        title: title.into(),
        sub: "".into(),
        icon: "".into(),
        badge: "".into(),
        open: false,
        shortcut: "".into(),
        qi: -1,
    });
}

/// 结果行。`badge`：全部视图带分类徽章；单分类视图留空（右列显示 ↵）。
fn result_row(item: &SearchItem, qi: usize, badge: Option<&str>) -> QsRow {
    let (badge, shortcut) = match (&item.payload, badge) {
        // 动作行右侧是快捷键键帽（设计稿 pl-keys）。
        (crate::search::Payload::Action { shortcut, .. }, _) => {
            ("".to_string(), shortcut.to_string())
        }
        _ => (badge.unwrap_or("").to_string(), "".to_string()),
    };
    QsRow {
        kind: 1,
        cat: category_int(item.cat),
        title: item.title.clone().into(),
        sub: item.sub.clone().into(),
        icon: item.cat.icon().into(),
        badge: badge.into(),
        open: item.open,
        shortcut: shortcut.into(),
        qi: qi as i32,
    }
}

/// 把查询结果（分组 + 扁平）压成行模型，返回（rows, 扁平快照, 内容高 px）。
fn rows_from_groups(
    groups: &[GroupedMatches<'_>],
    flat: Vec<SearchItem>,
    all_view: bool,
) -> (Vec<QsRow>, Vec<SearchItem>, i32) {
    let mut rows = Vec::new();
    let mut height = 0i32;
    let mut qi = 0usize;
    for g in groups {
        group_head(&mut rows, format!("{} · {}", g.cat.label(), g.items.len()));
        height += 24;
        for m in &g.items {
            let badge = if all_view { Some(g.cat.label()) } else { None };
            rows.push(result_row(m.item, qi, badge));
            height += 49;
            qi += 1;
        }
    }
    let _ = flat;
    (rows, Vec::new(), height)
}

/// 预览区整块构造（store 补会话详情）。
fn build_preview(store: &ConfigStore, item: &SearchItem) -> QsPreview {
    let mut metas: Vec<QsMeta> = Vec::new();
    let mut code = String::new();
    let t = crate::i18n::t;
    let (primary, secondary) = match &item.payload {
        crate::search::Payload::Session { session_id, .. } => {
            if let Some(s) = store.get(session_id) {
                metas.push(QsMeta {
                    label: t("主机", "Host").into(),
                    value: format!("{}@{}:{}", s.user, s.host, s.port).into(),
                    mono: true,
                });
                metas.push(QsMeta {
                    label: t("分组", "Group").into(),
                    value: crate::config::display_group_of(&s).into(),
                    mono: false,
                });
                metas.push(QsMeta {
                    label: t("认证", "Auth").into(),
                    value: s.auth.as_str().into(),
                    mono: false,
                });
            }
            metas.push(QsMeta {
                label: t("状态", "Status").into(),
                value: if item.open { t("已打开", "Open") } else { t("未连接", "Not connected") }.into(),
                mono: false,
            });
            (
                if item.open { t("Enter 切换", "Enter switch") } else { t("Enter 连接", "Enter connect") }.to_string(),
                if item.open {
                    t("Ctrl+Enter 复制 SSH 命令", "Ctrl+Enter copy SSH command").to_string()
                } else {
                    t("Ctrl+Enter 新标签连接", "Ctrl+Enter connect in new tab").to_string()
                },
            )
        }
        crate::search::Payload::Command { cmd, from_history } => {
            metas.push(QsMeta {
                label: t("来源", "Source").into(),
                value: if *from_history { t("命令历史", "History") } else { t("快捷命令", "Quick command") }.into(),
                mono: false,
            });
            metas.push(QsMeta {
                label: t("发送后回车", "Sends Enter").into(),
                value: t("是", "Yes").into(),
                mono: false,
            });
            code = cmd.clone();
            (
                t("Enter 发送", "Enter send").to_string(),
                t("Ctrl+Enter 填入命令栏", "Ctrl+Enter fill command bar").to_string(),
            )
        }
        crate::search::Payload::Setting { .. } => {
            metas.push(QsMeta {
                label: t("路径", "Path").into(),
                value: item.sub.clone().into(),
                mono: false,
            });
            (
                t("Enter 打开设置", "Enter open settings").to_string(),
                t("Ctrl+Enter 复制路径", "Ctrl+Enter copy path").to_string(),
            )
        }
        crate::search::Payload::Action { shortcut, .. } => {
            metas.push(QsMeta {
                label: t("快捷键", "Shortcut").into(),
                value: if shortcut.is_empty() { "—" } else { shortcut }.into(),
                mono: false,
            });
            (
                t("Enter 执行", "Enter run").to_string(),
                t("Ctrl+Enter 查看快捷键", "Ctrl+Enter show shortcut").to_string(),
            )
        }
        crate::search::Payload::None => (t("Enter 执行", "Enter run").to_string(), String::new()),
    };
    QsPreview {
        has_item: true,
        icon: item.cat.icon().into(),
        title: item.title.clone().into(),
        cat: item.cat.label().into(),
        desc: item.desc.clone().into(),
        metas: ModelRc::from(Rc::new(VecModel::from(metas))),
        code: code.into(),
        primary: primary.into(),
        secondary: secondary.into(),
    }
}

/// 空预览。
fn empty_preview() -> QsPreview {
    QsPreview::default()
}

/// 按 qs-hi 重灌预览。
fn push_preview(win: &AppWindow, store: &ConfigStore, state: &QsState) {
    let hi = win.get_qs_hi();
    let rows = win.get_qs_rows();
    let qi = rows
        .row_data(hi.max(0) as usize)
        .map(|r| r.qi)
        .unwrap_or(-1);
    let preview = if qi >= 0 {
        state
            .flat
            .get(qi as usize)
            .map(|item| build_preview(store, item))
            .unwrap_or_else(empty_preview)
    } else {
        empty_preview()
    };
    win.set_qs_preview(preview);
}

/// 首个可选行（组头 kind=0 跳过）。
fn first_selectable(win: &AppWindow) -> i32 {
    let rows = win.get_qs_rows();
    (0..rows.row_count())
        .find(|i| rows.row_data(*i as usize).map(|r| r.kind != 0).unwrap_or(false))
        .unwrap_or(0) as i32
}

/// 动作执行（动作行的 Enter 主操作）。
fn run_action(win: &AppWindow, act: &str) -> Option<String> {
    let t = crate::i18n::t;
    match act {
        "new-window" => {
            win.invoke_new_window_clicked();
            Some(t("已新建窗口", "New window created").to_string())
        }
        "new-session" => {
            win.invoke_new_session_clicked();
            Some(t("已打开新建会话向导", "New-session dialog opened").to_string())
        }
        "toggle-sidebar" => {
            let v = !win.get_left_sidebar_collapsed();
            win.set_left_sidebar_collapsed(v);
            win.invoke_set_left_sidebar_collapsed(v);
            Some(if v { t("左侧栏已收起", "Left sidebar collapsed") } else { t("左侧栏已展开", "Left sidebar expanded") }.to_string())
        }
        "toggle-right" => {
            let v = !win.get_right_sidebar_collapsed();
            win.set_right_sidebar_collapsed(v);
            win.invoke_set_right_sidebar_collapsed(v);
            Some(if v { t("右侧栏已收起", "Right sidebar collapsed") } else { t("右侧栏已展开", "Right sidebar expanded") }.to_string())
        }
        "zen" => {
            win.invoke_toggle_zen_key();
            Some(t("专注模式已切换", "Zen mode toggled").to_string())
        }
        "toggle-theme" => {
            win.invoke_cycle_theme();
            Some(t("主题已切换", "Theme toggled").to_string())
        }
        "font-inc" => {
            win.invoke_zoom_term_font(win.get_active_tab_id().clone(), 1, false);
            Some(t("终端字体已放大", "Terminal font increased").to_string())
        }
        "font-dec" => {
            win.invoke_zoom_term_font(win.get_active_tab_id().clone(), -1, false);
            Some(t("终端字体已缩小", "Terminal font decreased").to_string())
        }
        "font-reset" => {
            win.invoke_zoom_term_font(win.get_active_tab_id().clone(), 0, false);
            Some(t("终端字体已重置", "Terminal font reset").to_string())
        }
        "open-settings" => {
            win.set_interface_open(true);
            win.set_ifd_page("theme".into());
            Some(t("已打开：设置", "Opened: Settings").to_string())
        }
        "open-command-manager" => {
            win.invoke_open_command_manager();
            Some(t("已打开：命令管理", "Opened: Command manager").to_string())
        }
        "open-send-window" => {
            win.invoke_open_send_window("".into());
            Some(t("已打开发送命令窗口", "Send-command window opened").to_string())
        }
        "open-history" => {
            win.set_qs_term_history_seq(win.get_qs_term_history_seq() + 1);
            Some(t("已打开：命令历史", "Opened: Command history").to_string())
        }
        "terminal-find" => {
            win.set_qs_term_find_seq(win.get_qs_term_find_seq() + 1);
            Some(t("已打开：终端查找", "Opened: Terminal find").to_string())
        }
        "ping" => {
            win.invoke_ping_open();
            Some(t("已打开：Ping 检测", "Opened: Ping").to_string())
        }
        "system-info" => {
            win.invoke_open_system_info();
            Some(t("已打开：系统信息", "Opened: System info").to_string())
        }
        "processes" => {
            win.invoke_open_processes();
            Some(t("已打开：进程管理", "Opened: Process manager").to_string())
        }
        "about" => {
            win.set_about_open(true);
            Some(t("已打开：关于", "Opened: About").to_string())
        }
        _ => None,
    }
}

/// 五类执行语义（返回 toast 文案；None = 无反馈）。
fn execute_item(
    win: &AppWindow,
    store: &ConfigStore,
    item: &SearchItem,
    secondary: bool,
) -> Option<String> {
    let t = crate::i18n::t;
    match &item.payload {
        crate::search::Payload::Session { session_id, ssh_cmd } => {
            if item.open {
                if secondary && !ssh_cmd.is_empty() {
                    copy_async(ssh_cmd.clone());
                    return Some(t("已复制 SSH 命令", "SSH command copied").to_string());
                }
                // 切换到该会话的标签：从 tabs/panes 模型找 tab → pane。
                let tabs = win.get_tabs();
                let mut tab_id = String::new();
                for i in 0..tabs.row_count() {
                    if let Some(tab) = tabs.row_data(i) {
                        if tab.session_id.as_str() == session_id.as_str() {
                            tab_id = tab.id.to_string();
                            break;
                        }
                    }
                }
                if tab_id.is_empty() {
                    return None;
                }
                let panes = win.get_panes();
                for i in 0..panes.row_count() {
                    if let Some(pane) = panes.row_data(i) {
                        if pane.tabs.iter().any(|tb| tb.id.as_str() == tab_id) {
                            win.invoke_pane_tab_selected(pane.id, SharedString::from(tab_id.as_str()));
                            break;
                        }
                    }
                }
                Some(t("已切换到 {name}", "Switched to {name}").replace("{name}", &item.title))
            } else {
                win.invoke_connect_session(SharedString::from(session_id.as_str()));
                Some(t("已连接 {name}", "Connected to {name}").replace("{name}", &item.title))
            }
        }
        crate::search::Payload::Command { cmd, .. } => {
            let active = win.get_active_tab_id().to_string();
            let live = !active.is_empty() && active != "welcome";
            if !live {
                copy_async(cmd.clone());
                return Some(t("无活动会话，已复制命令", "No active session; command copied").to_string());
            }
            if secondary {
                win.set_quick_external_command(SharedString::from(cmd.as_str()));
                win.set_quick_external_send_enter(false);
                win.set_quick_external_sequence(win.get_quick_external_sequence() + 1);
                Some(t("已填入命令栏", "Filled into command bar").to_string())
            } else {
                win.invoke_run_command(
                    SharedString::from(active.as_str()),
                    SharedString::from(cmd.as_str()),
                    "".into(),
                    "".into(),
                );
                Some(t("已发送到当前会话", "Sent to the current session").to_string())
            }
        }
        crate::search::Payload::Setting { page, .. } => {
            if secondary {
                copy_async(item.sub.clone());
                Some(t("已复制设置路径", "Settings path copied").to_string())
            } else {
                win.set_interface_open(true);
                win.set_ifd_page(SharedString::from(*page));
                Some(t("已打开：{name}", "Opened: {name}").replace("{name}", &item.title))
            }
        }
        crate::search::Payload::Action { act, shortcut } => {
            if secondary {
                // 高保真：查看快捷键并关闭（快捷键随 toast 显示）。
                Some(
                    t("快捷键：{keys}", "Shortcut: {keys}").replace(
                        "{keys}",
                        if shortcut.is_empty() { "—" } else { shortcut },
                    ),
                )
            } else {
                run_action(win, act)
            }
        }
        crate::search::Payload::None => None,
    }
}

/// 安装全部回调；返回共享的双击 Shift 状态机（`on_send_key` 与
/// `search-shift-press` 两个宿主共用同一个实例）。
pub(crate) fn install(
    window: &AppWindow,
    store: &Rc<RefCell<ConfigStore>>,
) -> Arc<Mutex<DoubleShiftTracker>> {
    let state: Rc<RefCell<QsState>> = Rc::new(RefCell::new(QsState::default()));
    let recent: Rc<RefCell<QsRecent>> = Rc::new(RefCell::new(QsRecent::default()));
    let tracker = Arc::new(Mutex::new(DoubleShiftTracker::new()));
    let weak = window.as_weak();
    let weak_shift = window.as_weak();

    // ── 纯回调：文案拼装（Slint 无 format）──────────────────────────
    {
        window.on_empty_title(move || {
            let q = EMPTY_QUERY.with(|c| c.borrow().clone());
            SharedString::from(
                crate::i18n::t("没有找到 “{q}”", "No results for “{q}”").replace("{q}", &q),
            )
        });
    }
    {
        let weak2 = weak.clone();
        window.on_index_count_text(move || {
            let Some(win) = weak2.upgrade() else { return Default::default() };
            let all = win.get_qs_count_all();
            SharedString::from(
                crate::i18n::t("索引 {n} 项", "Index of {n}").replace("{n}", &all.to_string()),
            )
        });
    }
    {
        let weak3 = weak.clone();
        window.on_timing_text(move || {
            let Some(win) = weak3.upgrade() else { return Default::default() };
            let q = win.get_qs_query_ms().to_string();
            let o = win.get_qs_open_ms().to_string();
            SharedString::from(
                crate::i18n::t("查询 {q} · 打开 {o}", "query {q} · open {o}")
                    .replace("{q}", &q)
                    .replace("{o}", &o),
            )
        });
    }

    // ── 空结果建议（静态三条，语言切换由 t() 原子变量承担）──────────
    {
        let hints: Vec<SharedString> = vec![
            crate::i18n::t("1  检查拼写，确认没有多余空格", "1  Check spelling and stray spaces").into(),
            crate::i18n::t("2  试试更短的关键词", "2  Try a shorter keyword").into(),
            crate::i18n::t("3  按 Tab 切到具体分类再搜", "3  Press Tab to narrow by category").into(),
        ];
        window.set_qs_empty_hints(ModelRc::from(Rc::new(VecModel::from(hints))));
    }

    // ── 打开编排：重建索引 + 推计数 + 最近搜索 + 空查询视图 ─────────
    {
        let store_rc = store.clone();
        let state_rc = state.clone();
        let recent_rc = recent.clone();
        let weak = weak.clone();
        window.on_qs_open(move || {
            let Some(win) = weak.upgrade() else { return };
            let t0 = std::time::Instant::now();
            let open_ids = open_session_ids(&win);
            let rank_map = recent_rc.borrow().rank_map();
            let index = build_index(
                &store_rc.borrow(),
                &SessionView { open_ids: &open_ids, recent_rank: &rank_map },
            );
            let counts = index.counts;
            let recent_view = {
                let ids = recent_rc.borrow().ids.clone();
                recent_items(&index, &ids)
            };
            let flat: Vec<SearchItem> = recent_view.iter().map(|r| r.item.clone()).collect();
            let groups = if flat.is_empty() {
                vec![]
            } else {
                vec![GroupedMatches { cat: Category::Session, items: recent_view }]
            };
            let (mut rows, _flat, height) = rows_from_groups(&groups, Vec::new(), false);
            // 空查询视图的组头固定叫「最近使用」（不是分类名）。
            if let Some(head) = rows.first_mut() {
                head.title = crate::i18n::t("最近使用", "Recent items").into();
            }
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            win.set_qs_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            win.set_qs_content_px(height);
            win.set_qs_row_count(flat.len() as i32);
            *state_rc.borrow_mut() = QsState { index, flat };
            win.set_qs_hi(first_selectable(&win));
            win.set_qs_count_all(counts.all as i32);
            win.set_qs_count_session(counts.session as i32);
            win.set_qs_count_command(counts.command as i32);
            win.set_qs_count_setting(counts.setting as i32);
            win.set_qs_count_action(counts.action as i32);
            let searches: Vec<SharedString> = recent_rc
                .borrow()
                .searches
                .iter()
                .map(|s| SharedString::from(s.as_str()))
                .collect();
            win.set_qs_recent_searches(ModelRc::from(Rc::new(VecModel::from(searches))));
            win.set_qs_open_ms(SharedString::from(format!("{ms:.1} ms")));
            win.set_qs_query_ms("0.0 ms".into());
            let st = state_rc.borrow();
            let store_ref = store_rc.borrow();
            push_preview(&win, &store_ref, &st);
        });
    }

    // ── 查询：按 qs-search-text + qs-cat ────────────────────────────
    {
        let store_rc = store.clone();
        let state_rc = state.clone();
        let recent_rc = recent.clone();
        let weak = weak.clone();
        window.on_qs_refresh(move || {
            let Some(win) = weak.upgrade() else { return };
            let t0 = std::time::Instant::now();
            let query = win.get_qs_search_text().to_string();
            let cat = cat_from_int(win.get_qs_cat());
            EMPTY_QUERY.with(|c| *c.borrow_mut() = query.clone());
            let index_snapshot = state_rc.borrow().index.clone();
            let has_query = !query.trim().is_empty();
            let (rows, flat, height) = if has_query {
                let hits = search(&index_snapshot, &query, cat).unwrap_or_default();
                let (groups, flat_refs) = group_and_cap(hits, cat);
                let all_view = cat.is_none();
                let mut rows = Vec::new();
                let mut height = 0i32;
                let mut qi = 0usize;
                for g in &groups {
                    group_head(&mut rows, format!("{} · {}", g.cat.label(), g.items.len()));
                    height += 24;
                    for m in &g.items {
                        let badge = if all_view { Some(g.cat.label()) } else { None };
                        rows.push(result_row(m.item, qi, badge));
                        height += 49;
                        qi += 1;
                    }
                }
                let flat: Vec<SearchItem> = flat_refs.iter().map(|r| r.item.clone()).collect();
                (rows, flat, height)
            } else {
                // 空查询：最近使用（不分类过滤）。
                let ids = recent_rc.borrow().ids.clone();
                let recent_view = recent_items(&index_snapshot, &ids);
                let flat: Vec<SearchItem> = recent_view.iter().map(|r| r.item.clone()).collect();
                let groups = if flat.is_empty() {
                    vec![]
                } else {
                    vec![GroupedMatches { cat: Category::Session, items: recent_view }]
                };
                let (mut rows, _f, height) = rows_from_groups(&groups, Vec::new(), false);
                if let Some(head) = rows.first_mut() {
                    head.title = crate::i18n::t("最近使用", "Recent items").into();
                }
                (rows, flat, height)
            };
            win.set_qs_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            win.set_qs_content_px(height);
            win.set_qs_row_count(flat.len() as i32);
            {
                let mut st = state_rc.borrow_mut();
                st.flat = flat;
            }
            win.set_qs_hi(first_selectable(&win));
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            win.set_qs_query_ms(SharedString::from(format!("{ms:.1} ms")));
            let st = state_rc.borrow();
            let store_ref = store_rc.borrow();
            push_preview(&win, &store_ref, &st);
        });
    }

    // ── ↑↓ / PageUp/Down 跳组头移动（照 on_lib_move 循环）───────────
    {
        let store_rc = store.clone();
        let state_rc = state.clone();
        let weak = weak.clone();
        window.on_qs_move(move |delta: i32| {
            let Some(win) = weak.upgrade() else { return };
            let rows = win.get_qs_rows();
            let n = rows.row_count() as i32;
            if n == 0 {
                return;
            }
            let mut hi = win.get_qs_hi();
            loop {
                hi += delta;
                if hi < 0 || hi >= n {
                    return;
                }
                if rows.row_data(hi as usize).map(|r| r.kind != 0).unwrap_or(false) {
                    win.set_qs_hi(hi);
                    let st = state_rc.borrow();
                    let store_ref = store_rc.borrow();
                    push_preview(&win, &store_ref, &st);
                    return;
                }
            }
        });
    }

    // ── 鼠标点行 / Home / End：绝对选中 ─────────────────────────────
    {
        let store_rc = store.clone();
        let state_rc = state.clone();
        let weak = weak.clone();
        window.on_qs_sel(move |idx: i32| {
            let Some(win) = weak.upgrade() else { return };
            let rows = win.get_qs_rows();
            let n = rows.row_count() as i32;
            if n == 0 {
                return;
            }
            let target = idx.clamp(0, n - 1);
            let mut hi = target;
            // 组头不可选中：向右找第一个可选行；超出即向左回退。
            if rows.row_data(hi as usize).map(|r| r.kind == 0).unwrap_or(false) {
                hi = (hi..n).find(|i| rows.row_data(*i as usize).map(|r| r.kind != 0).unwrap_or(false))
                    .unwrap_or((0..=hi).rev().find(|i| rows.row_data(*i as usize).map(|r| r.kind != 0).unwrap_or(false)).unwrap_or(0))
                    as i32;
            }
            win.set_qs_hi(hi);
            let st = state_rc.borrow();
            let store_ref = store_rc.borrow();
            push_preview(&win, &store_ref, &st);
        });
    }

    // ── 执行（Enter / Ctrl+Enter / 双击 / 预览按钮）─────────────────
    {
        let store_rc = store.clone();
        let state_rc = state.clone();
        let recent_exec = recent.clone();
        let weak = weak.clone();
        window.on_qs_execute(move |qi: i32, secondary: bool| {
            let Some(win) = weak.upgrade() else { return };
            let item = {
                let st = state_rc.borrow();
                (qi >= 0)
                    .then(|| st.flat.get(qi as usize))
                    .flatten()
                    .cloned()
            };
            let Some(item) = item else { return };
            // 最近使用 / 最近搜索记录（高保真 executeItem rememberSearch）。
            recent_exec.borrow_mut().remember_item(&item.id);
            let q = win.get_qs_search_text().to_string();
            recent_exec.borrow_mut().remember_search(&q);
            let toast = execute_item(&win, &store_rc.borrow(), &item, secondary);
            if let Some(msg) = toast {
                win.set_quick_search_toast(SharedString::from(msg));
            }
            // 执行即关面板（高保真语义；复制类同样关闭）。
            win.set_quick_search_open(false);
            // 焦点还给活动终端（粘贴确认卡同款机制）。
            win.set_terminal_focus_sequence(win.get_terminal_focus_sequence() + 1);
        });
    }

    // ── 双击 Shift 触发器（非终端焦点宿主；终端链路见 app.rs on_send_key）
    {
        let tracker_shift = tracker.clone();
        window.on_search_shift_press(move || {
            let Some(win) = weak_shift.upgrade() else { return };
            let opened = tracker_shift
                .lock()
                .unwrap()
                .on_key(
                    crate::search::KeyEvent::Shift,
                    std::time::Instant::now(),
                    win.get_quick_search_open(),
                );
            if opened {
                win.set_quick_search_open(true);
            }
        });
    }

    tracker
}

thread_local! {
    /// 空结果态标题里的用户查询词（pure callback 无参数，借 thread_local 传递）。
    static EMPTY_QUERY: RefCell<String> = RefCell::new(String::new());
}
