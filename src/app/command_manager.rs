//! 命令管理弹框（#quick-cmd-manager，高保真 2026-10-24）的模型构建与回调。
//!
//! 结构对应 ui/command_manager.slint：
//!  · 左栏组行 / 右栏卡片全部由 [`refresh`] 重建下发（过滤、计数、can-up/down
//!    都在 Rust 算——Slint 的字符串没有 contains，数组没有 filter）。
//!  · 卡片用弹框会话内的**稳定 uid** 定位：`QuickCommand` 没有稳定 id，
//!    orig-index 在任何一次增删移后都会漂移，勾选/拖拽/编辑拿它做键会串行。
//!    uid 只活在弹框会话内（打开时重建，改动时与 commands 同步搬移），
//!    不进配置文件。
//!  · 所有落盘动作统一走「改 store → save → 刷新命令栏模型 → 刷新弹框模型」，
//!    与分组管理同一节奏；底栏结果文案由 Slint 端组织（toast）。

use super::quick_commands::quick_cmd_model;
use crate::config::ConfigStore;
use crate::ui::{AppWindow, CmCard, CmGroupRow};
use slint::{ComponentHandle, Model, ModelRc, SharedString};
use super::clipboard_set_text;
use std::cell::RefCell;
use std::rc::Rc;

/// 左栏「全部命令」行的 key（存储名空间外的哨兵值）。
const CM_ALL: &str = "__all";
/// 默认分组的显示名（存储值为空串）。
const CM_DEFAULT_DISPLAY: &str = "默认分组";

/// 弹框会话态：uid 表（与 store.quick_commands() 等长同序）+ 勾选集 +
/// Shift 范围勾选锚点。
pub(super) struct CmState {
    uids: Vec<u64>,
    next: u64,
    picked: std::collections::HashSet<u64>,
    anchor: Option<u64>,
}

impl CmState {
    fn new() -> Self {
        Self { uids: Vec::new(), next: 1, picked: std::collections::HashSet::new(), anchor: None }
    }

    /// 打开弹框时重建：uid 表按当前命令表分配；勾选/锚点复位。
    fn reseat(&mut self, len: usize) {
        self.uids.clear();
        for _ in 0..len {
            self.uids.push(self.next);
            self.next += 1;
        }
        self.picked.clear();
        self.anchor = None;
    }
}

fn uid_key(uid: u64) -> SharedString {
    SharedString::from(uid.to_string())
}

fn parse_key(key: &str) -> Option<u64> {
    key.parse::<u64>().ok()
}

/// 组存储值归一：空串 / "default"（不分大小写）→ 默认分组（空串）。
fn normalize_group(g: &str) -> String {
    let t = g.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("default") {
        String::new()
    } else {
        t.to_string()
    }
}

fn group_display(g: &str) -> &str {
    if g.is_empty() || g.eq_ignore_ascii_case("default") {
        CM_DEFAULT_DISPLAY
    } else {
        g
    }
}

/// 自定义组序列（有序）：显式 `quick_groups`（用户排定的顺序）在前，只被
/// 命令引用而未登记的隐含组按字母序追加在后。左栏顺序、编辑卡下拉、
/// 命令栏面板分组顺序同源（quick_cmd_model 用同一口径）。
fn cm_group_seq(store: &ConfigStore) -> Vec<String> {
    let mut seq: Vec<String> = store
        .quick_groups()
        .iter()
        .map(|g| normalize_group(g))
        .filter(|g| !g.is_empty())
        .collect();
    let mut implied: Vec<String> = store
        .quick_commands()
        .iter()
        .map(|c| normalize_group(&c.group))
        .filter(|g| !g.is_empty() && !seq.contains(g))
        .collect();
    implied.sort_by_key(|g| g.to_lowercase());
    implied.dedup();
    seq.extend(implied);
    seq
}

/// 当前视图成员在扁平命令表中的下标序列：
///  · selected == "__all" → 0..len（全部命令视图 = 扁平表序）
///  · 组视图 → 该组成员（保持扁平表相对序）
fn cm_view_indices(store: &ConfigStore, selected: &str) -> Vec<usize> {
    let cmds = store.quick_commands();
    if selected == CM_ALL {
        return (0..cmds.len()).collect();
    }
    let g = normalize_group(selected);
    cmds.iter()
        .enumerate()
        .filter(|(_, c)| normalize_group(&c.group) == g)
        .map(|(i, _)| i)
        .collect()
}

fn cmd_matches(c: &crate::config::QuickCommand, group_disp: &str, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    // 名称 / 备注 / 命令 / 分组名四路匹配（稿子 §搜索）
    c.name.to_lowercase().contains(q)
        || c.note.to_lowercase().contains(q)
        || c.command.to_lowercase().contains(q)
        || group_disp.to_lowercase().contains(q)
}

/// 弹框数据刷新：左栏组行 + 当前视图卡片 + 下拉数据 + 头部/计数汇总。
pub(super) fn refresh(window: &AppWindow, store: &ConfigStore, cm: &CmState) {
    let cmds = store.quick_commands();
    let query = window.get_cm_query().to_lowercase();
    let selected = {
        let s = window.get_cm_selected().to_string();
        if s.is_empty() && !window.get_quick_cmd_manage_open() {
            CM_ALL.to_string()
        } else {
            s
        }
    };
    let is_all = selected == CM_ALL;

    // ── 左栏组行 ──
    let mut rows: Vec<CmGroupRow> = Vec::new();
    let all_hit = if query.is_empty() {
        cmds.len() as i32
    } else {
        cmds.iter().filter(|c| cmd_matches(c, group_display(&c.group), &query)).count() as i32
    };
    rows.push(CmGroupRow {
        key: CM_ALL.into(),
        display: if window.get_lang_en() { "All commands" } else { "全部命令" }.into(),
        count: cmds.len() as i32,
        hit: all_hit,
        kind: 0,
    });
    let default_hit = if query.is_empty() {
        cmds.iter().filter(|c| normalize_group(&c.group).is_empty()).count() as i32
    } else {
        cmds.iter()
            .filter(|c| normalize_group(&c.group).is_empty())
            .filter(|c| cmd_matches(c, CM_DEFAULT_DISPLAY, &query))
            .count() as i32
    };
    rows.push(CmGroupRow {
        key: "".into(),
        display: CM_DEFAULT_DISPLAY.into(),
        count: cmds.iter().filter(|c| normalize_group(&c.group).is_empty()).count() as i32,
        hit: default_hit,
        kind: 1,
    });
    let custom = cm_group_seq(store);
    for g in &custom {
        let members: Vec<usize> = cmds
            .iter()
            .enumerate()
            .filter(|(_, c)| normalize_group(&c.group) == *g)
            .map(|(i, _)| i)
            .collect();
        let hit = if query.is_empty() {
            members.len() as i32
        } else {
            members
                .iter()
                .filter(|&&i| cmd_matches(&cmds[i], g, &query))
                .count() as i32
        };
        rows.push(CmGroupRow {
            key: g.as_str().into(),
            display: g.as_str().into(),
            count: members.len() as i32,
            hit,
            kind: 2,
        });
    }

    // ── 当前视图卡片（过滤后）──
    let view = cm_view_indices(store, &selected);
    let visible: Vec<usize> = view
        .iter()
        .copied()
        .filter(|&i| cmd_matches(&cmds[i], group_display(&normalize_group(&cmds[i].group)), &query))
        .collect();
    // can-up/down：搜索态禁排序；全部命令 = 扁平相邻；组视图 = 组内相邻
    let mut cards: Vec<CmCard> = Vec::with_capacity(visible.len());
    for (pos, &i) in visible.iter().enumerate() {
        let uid = cm.uids.get(i).copied().unwrap_or(0);
        let c = &cmds[i];
        let g_disp = group_display(&normalize_group(&c.group)).to_string();
        cards.push(CmCard {
            key: uid_key(uid),
            name: c.name.clone().into(),
            note: c.note.clone().into(),
            command: c.command.clone().into(),
            group_key: normalize_group(&c.group).into(),
            group_display: g_disp.into(),
            send_enter: c.send_enter,
            picked: cm.picked.contains(&uid),
            can_up: query.is_empty() && pos > 0,
            can_down: query.is_empty() && pos + 1 < visible.len(),
            show_group: is_all || !query.is_empty(),
        });
    }

    // ── 头部/计数汇总 ──
    let total = cmds.len() as i32;
    let view_count = cards.len() as i32;
    let picked_count = cards.iter().filter(|c| c.picked).count() as i32;
    // 全部勾上 = 当前可见卡全部 picked（空列表不算）
    let all_picked = !cards.is_empty() && cards.iter().all(|c| c.picked);
    let cur_name = if is_all {
        if window.get_lang_en() { "All commands" } else { "全部命令" }.to_string()
    } else {
        group_display(&selected).to_string()
    };
    let cur_count = if is_all {
        total
    } else {
        view.len() as i32
    };
    let cur_custom = !is_all && !normalize_group(&selected).is_empty();
    let cur_index = rows.iter().position(|r| r.key == selected.as_str()).unwrap_or(0) as i32;
    let subtitle = format!(
        "{}{}{}{}",
        custom.len() + 1,
        if window.get_lang_en() { " groups · " } else { " 组 · " },
        total,
        if window.get_lang_en() { "" } else { " 条" }
    );

    // ── 搜索空态分支数据 ──
    let search_hits = if query.is_empty() { 0 } else { all_hit };
    let search_jump = if query.is_empty() || all_hit == 0 {
        String::new()
    } else {
        // 第一个「别处有命中」的组（跳转引导备用；当前空态只引导去「全部命令」）
        rows.iter()
            .skip(1)
            .find(|r| r.hit > 0 && r.key != selected.as_str())
            .map(|r| r.display.to_string())
            .unwrap_or_default()
    };

    // ── 下拉数据（默认分组 + 自定义组；编辑卡 / 移动至菜单 / 删除条去向共用）──
    let mut menu: Vec<CmGroupRow> = vec![CmGroupRow {
        key: "".into(),
        display: CM_DEFAULT_DISPLAY.into(),
        count: cmds.iter().filter(|c| normalize_group(&c.group).is_empty()).count() as i32,
        hit: 0,
        kind: 1,
    }];
    for g in &custom {
        let n = cmds.iter().filter(|c| normalize_group(&c.group) == *g).count() as i32;
        menu.push(CmGroupRow {
            key: g.as_str().into(),
            display: g.as_str().into(),
            count: n,
            hit: 0,
            kind: 2,
        });
    }

    window.set_cm_groups(ModelRc::from(Rc::new(slint::VecModel::from(rows))));
    window.set_cm_menu_groups(ModelRc::from(Rc::new(slint::VecModel::from(menu))));
    window.set_cm_cards(ModelRc::from(Rc::new(slint::VecModel::from(cards))));
    window.set_cm_picked_count(picked_count);
    window.set_cm_all_picked(all_picked);
    window.set_cm_custom_count(custom.len() as i32);
    window.set_cm_total_count(total);
    window.set_cm_cur_name(cur_name.into());
    window.set_cm_cur_count(cur_count);
    window.set_cm_cur_custom(cur_custom);
    window.set_cm_cur_index(cur_index);
    window.set_cm_view_count(view_count);
    window.set_cm_subtitle(subtitle.into());
    window.set_cm_search_hits(search_hits);
    window.set_cm_search_jump(search_jump.into());
    window.set_cm_has_any(total > 0);
}

/// 落盘 + 刷新（命令栏模型 + 弹框模型）——每个改数据的回调收尾都调它。
fn commit(
    window: &AppWindow,
    store: &Rc<RefCell<ConfigStore>>,
    collapsed: &Rc<RefCell<std::collections::HashSet<String>>>,
    cm: &Rc<RefCell<CmState>>,
) {
    {
        let s = store.borrow_mut();
        if let Err(err) = s.save() {
            tracing::warn!("failed to save config: {err:#}");
        }
    }
    window.set_quick_commands(quick_cmd_model(&store.borrow(), &collapsed.borrow()));
    refresh(window, &store.borrow(), &cm.borrow());
}

/// uid → 扁平表下标。
fn uid_index(cm: &CmState, uid: u64) -> Option<usize> {
    cm.uids.iter().position(|&u| u == uid)
}

/// 拖拽/勾选的「整块」：拖的卡已勾选且勾了多条 → 全部已勾选（store 序），
/// 否则只有它自己。返回扁平表下标（保持相对顺序）。
fn moving_indices(cm: &CmState, key: &str) -> Vec<usize> {
    let Some(uid) = parse_key(key) else { return Vec::new() };
    let is_picked = cm.picked.contains(&uid);
    if is_picked && cm.picked.len() > 1 {
        cm.uids
            .iter()
            .enumerate()
            .filter(|(_, &u)| cm.picked.contains(&u))
            .map(|(i, _)| i)
            .collect()
    } else {
        uid_index(cm, uid).into_iter().collect()
    }
}

/// 校验组名（行内改名 / 新建分组 / 编辑卡共享的口径）。
/// 返回 "" = 合法；空名也返回 ""（commit 闸门里静默放弃，不报红）。
fn validate_group_name(store: &ConfigStore, text: &str, orig: &str) -> String {
    let n = text.trim();
    if n.is_empty() {
        return String::new();
    }
    if n == CM_DEFAULT_DISPLAY || n.eq_ignore_ascii_case("default") {
        return "该名称与默认分组重名".into();
    }
    let seq = cm_group_seq(store);
    if seq.iter().any(|g| g != orig && g.eq_ignore_ascii_case(n)) {
        return "已存在同名分组（不区分大小写）".into();
    }
    String::new()
}

pub(super) fn install(
    window: &AppWindow,
    store: &Rc<RefCell<ConfigStore>>,
    collapsed: &Rc<RefCell<std::collections::HashSet<String>>>,
) {
    let cm: Rc<RefCell<CmState>> = Rc::new(RefCell::new(CmState::new()));

    // ── 打开：重置视图（全部命令）/ 搜索 / 勾选，重建 uid 表 ──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let cm = cm.clone();
        window.on_open_command_manager(move || {
            let Some(w) = weak.upgrade() else { return };
            let len = store_rc.borrow().quick_commands().len();
            cm.borrow_mut().reseat(len);
            w.set_cm_selected(CM_ALL.into());
            w.set_cm_query("".into());
            refresh(&w, &store_rc.borrow(), &cm.borrow());
            w.set_quick_cmd_manage_open(true);
        });
    }

    // ── 搜索 / 切组：只刷新模型（搜索不清勾选——勾选跨视图保留，切组清）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let cm = cm.clone();
        window.on_cm_search(move |_q: SharedString| {
            if let Some(w) = weak.upgrade() {
                refresh(&w, &store_rc.borrow(), &cm.borrow());
            }
        });
    }
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let cm = cm.clone();
        window.on_cm_select(move |_g: SharedString| {
            if let Some(w) = weak.upgrade() {
                cm.borrow_mut().picked.clear();
                cm.borrow_mut().anchor = None;
                refresh(&w, &store_rc.borrow(), &cm.borrow());
            }
        });
    }

    // ── 组名实时预校验（只读，不落盘）──
    {
        let store_rc = store.clone();
        window.on_cm_validate_group(move |text: SharedString, orig: SharedString| -> SharedString {
            validate_group_name(&store_rc.borrow(), &text, &orig).into()
        });
    }

    // ── 改名：保位（顺序不动），当前选择跟着换名 ──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_rename_group(move |orig: SharedString, name: SharedString| -> SharedString {
            let n = name.trim().to_string();
            if orig.as_str() == CM_ALL || normalize_group(&orig).is_empty() {
                return "该分组不可改名".into();
            }
            if n.is_empty() {
                return "名称不能为空".into();
            }
            {
                let mut s = store_rc.borrow_mut();
                let err = validate_group_name(&s, &n, &orig);
                if !err.is_empty() {
                    return err.into();
                }
                s.rename_quick_group(&orig.to_string(), n.clone());
            }
            if let Some(w) = weak.upgrade() {
                if w.get_cm_selected() == orig {
                    w.set_cm_selected(n.as_str().into());
                }
                commit(&w, &store_rc, &collapsed, &cm);
            }
            SharedString::new()
        });
    }

    // ── 新建：建完选中新组 ──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_create_group(move |name: SharedString| -> SharedString {
            let n = name.trim().to_string();
            if n.is_empty() {
                return "名称不能为空".into();
            }
            {
                let mut s = store_rc.borrow_mut();
                let err = validate_group_name(&s, &n, "");
                if !err.is_empty() {
                    return err.into();
                }
                s.add_quick_group(n.clone());
            }
            if let Some(w) = weak.upgrade() {
                w.set_cm_selected(n.as_str().into());
                commit(&w, &store_rc, &collapsed, &cm);
            }
            SharedString::new()
        });
    }

    // ── 删除组：组员整体移入所选去向；视图切到去向 ──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_remove_group(move |name: SharedString, target: SharedString| {
            if normalize_group(&name).is_empty() || name.as_str() == CM_ALL {
                return;
            }
            let t = normalize_group(&target);
            {
                let mut s = store_rc.borrow_mut();
                s.remove_quick_group_into(&name.to_string(), &t);
            }
            if let Some(w) = weak.upgrade() {
                cm.borrow_mut().picked.clear();
                w.set_cm_selected(t.as_str().into());
                commit(&w, &store_rc, &collapsed, &cm);
            }
        });
    }

    // ── 组排序：↑↓ 相邻一步（dir = -1/+1）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_move_group(move |name: SharedString, dir: i32| {
            let d = if dir < 0 { -1 } else { 1 };
            let mut changed = false;
            {
                let mut s = store_rc.borrow_mut();
                let mut seq = cm_group_seq(&s);
                if let Some(i) = seq.iter().position(|g| g == name.as_str()) {
                    let j = i as i32 + d;
                    if j >= 0 && (j as usize) < seq.len() {
                        seq.swap(i, j as usize);
                        // 整段序列落盘：隐含组也顺势登记为显式组（顺序完全受控）
                        s.set_quick_groups(seq);
                        changed = true;
                    }
                }
            }
            if changed {
                if let Some(w) = weak.upgrade() {
                    commit(&w, &store_rc, &collapsed, &cm);
                }
            }
        });
    }

    // ── 组排序：拖拽落点（插到目标前/后）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_reorder_group(
            move |drag: SharedString, target: SharedString, after: bool| {
                let mut changed = false;
                {
                    let mut s = store_rc.borrow_mut();
                    let mut seq = cm_group_seq(&s);
                    let from = seq.iter().position(|g| g == drag.as_str());
                    let to = seq.iter().position(|g| g == target.as_str());
                    if let (Some(from), Some(to)) = (from, to) {
                        if from != to {
                            let g = seq.remove(from);
                            let mut at = seq.iter().position(|x| *x == target.as_str()).unwrap_or(0);
                            if after {
                                at += 1;
                            }
                            seq.insert(at, g);
                            s.set_quick_groups(seq);
                            changed = true;
                        }
                    }
                }
                if changed {
                    if let Some(w) = weak.upgrade() {
                        commit(&w, &store_rc, &collapsed, &cm);
                    }
                }
            },
        );
    }

    // ── 保存命令（新建 / 编辑共用）──
    // 分组字段是显示名：「默认分组」/ 空 → 默认分组；不存在的组名就地新建。
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_save_cmd(
            move |key: SharedString,
                  name: SharedString,
                  note: SharedString,
                  command: SharedString,
                  group: SharedString,
                  send_enter: bool| -> SharedString {
                let name = name.trim().to_string();
                let command = command.trim_end().to_string();
                if name.is_empty() {
                    return "名称不能为空".into();
                }
                if command.trim().is_empty() {
                    return "命令内容不能为空".into();
                }
                let g = normalize_group(if group.trim() == CM_DEFAULT_DISPLAY { "" } else { &group });
                {
                    let mut s = store_rc.borrow_mut();
                    if !g.is_empty() {
                        s.add_quick_group(g.clone());   // 不存在则就地建组（保序追加）
                    }
                    let entry = crate::config::QuickCommand {
                        name,
                        command,
                        group: g,
                        note: note.trim().to_string(),
                        send_enter,
                    };
                    let mut c = cm.borrow_mut();
                    if key.is_empty() {
                        let mut v = s.quick_commands().to_vec();
                        v.push(entry);
                        s.set_quick_commands(v);
                        let uid = c.next;
                        c.next += 1;
                        c.uids.push(uid);
                    } else if let Some(uid) = parse_key(&key) {
                        if let Some(i) = uid_index(&c, uid) {
                            let mut v = s.quick_commands().to_vec();
                            if i < v.len() {
                                v[i] = entry;
                                s.set_quick_commands(v);
                            }
                        }
                    }
                }
                if let Some(w) = weak.upgrade() {
                    commit(&w, &store_rc, &collapsed, &cm);
                }
                SharedString::new()
            },
        );
    }

    // ── 复制：命令内容进剪贴板（不是创建副本条目）──
    {
        let store_rc = store.clone();
        let cm = cm.clone();
        window.on_cm_copy_cmd(move |key: SharedString| {
            let uid = parse_key(&key);
            let i = {
                let c = cm.borrow();
                uid.and_then(|u| uid_index(&c, u))
            };
            if let Some(i) = i {
                let text = store_rc.borrow().quick_commands().get(i).map(|c| c.command.clone());
                if let Some(text) = text {
                    std::thread::spawn(move || clipboard_set_text(text));
                }
            }
        });
    }

    // ── 单删（二段式确认后的执行）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_del_cmd(move |key: SharedString| {
            let mut changed = false;
            {
                let mut s = store_rc.borrow_mut();
                let mut c = cm.borrow_mut();
                if let Some(uid) = parse_key(&key) {
                    if let Some(i) = uid_index(&c, uid) {
                        let mut v = s.quick_commands().to_vec();
                        if i < v.len() {
                            v.remove(i);
                            s.set_quick_commands(v);
                            c.uids.remove(i);
                            c.picked.remove(&uid);
                            c.anchor = None;
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                if let Some(w) = weak.upgrade() {
                    commit(&w, &store_rc, &collapsed, &cm);
                }
            }
        });
    }

    // ── 切换发送方式（卡片右上图标一点即切，落盘）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_toggle_send(move |key: SharedString| {
            let mut changed = false;
            {
                let mut s = store_rc.borrow_mut();
                let c = cm.borrow();
                if let Some(uid) = parse_key(&key) {
                    if let Some(i) = uid_index(&c, uid) {
                        let mut v = s.quick_commands().to_vec();
                        if let Some(e) = v.get_mut(i) {
                            e.send_enter = !e.send_enter;
                            s.set_quick_commands(v);
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                if let Some(w) = weak.upgrade() {
                    commit(&w, &store_rc, &collapsed, &cm);
                }
            }
        });
    }

    // ── 勾选（点卡片任意处；Shift = 范围，锚点 = 上次点击的卡）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let cm = cm.clone();
        window.on_cm_toggle_pick(move |key: SharedString, shift: bool| {
            let Some(w) = weak.upgrade() else { return };
            let Some(uid) = parse_key(&key) else { return };
            {
                let mut c = cm.borrow_mut();
                if shift {
                    if let Some(anchor) = c.anchor {
                        // 范围 = 当前模型里 anchor..key 的整段，统一置为 key 的目标态
                        let keys: Vec<String> = w
                            .get_cm_cards()
                            .iter()
                            .map(|card| card.key.to_string())
                            .collect();
                        let a = keys.iter().position(|k| parse_key(k) == Some(anchor));
                        let b = keys.iter().position(|k| k == key.as_str());
                        if let (Some(a), Some(b)) = (a, b) {
                            let on = !c.picked.contains(&uid);
                            for k in &keys[a.min(b)..=a.max(b)] {
                                if let Some(u) = parse_key(k) {
                                    if on {
                                        c.picked.insert(u);
                                    } else {
                                        c.picked.remove(&u);
                                    }
                                }
                            }
                        }
                    }
                } else if !c.picked.remove(&uid) {
                    c.picked.insert(uid);
                }
                c.anchor = Some(uid);
            }
            refresh(&w, &store_rc.borrow(), &cm.borrow());
        });
    }

    // ── 清空勾选 / 全选（当前视图）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let cm = cm.clone();
        window.on_cm_clear_picks(move || {
            if let Some(w) = weak.upgrade() {
                cm.borrow_mut().picked.clear();
                cm.borrow_mut().anchor = None;
                refresh(&w, &store_rc.borrow(), &cm.borrow());
            }
        });
    }
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let cm = cm.clone();
        window.on_cm_set_all_picked(move |on: bool| {
            if let Some(w) = weak.upgrade() {
                let keys: Vec<u64> = w
                    .get_cm_cards()
                    .iter()
                    .filter_map(|card| parse_key(&card.key))
                    .collect();
                // ⚠️ 借用必须圈在块里：`RefMut` 有 Drop，NLL 会把它活到作用域
                // 末尾。写成 `let mut c = cm.borrow_mut();` 直接带到下面的
                // `refresh(..., &cm.borrow(), ...)`，就是"already mutably
                // borrowed" panic —— release 是 `panic = "abort"`，进程当场
                // 静默退出（用户实测：点「全选」程序直接没了，无提示无进程）。
                {
                    let mut c = cm.borrow_mut();
                    for u in keys {
                        if on {
                            c.picked.insert(u);
                        } else {
                            c.picked.remove(&u);
                        }
                    }
                }
                refresh(&w, &store_rc.borrow(), &cm.borrow());
            }
        });
    }

    // ── 批量移动（移动至菜单 / 拖整块共用语义；返回实际变动数）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_move_picked(move |target: SharedString| -> i32 {
            let t = normalize_group(&target);
            let mut moved = 0i32;
            {
                let mut s = store_rc.borrow_mut();
                let c = cm.borrow();
                let mut v = s.quick_commands().to_vec();
                for (i, e) in v.iter_mut().enumerate() {
                    if let Some(&uid) = c.uids.get(i) {
                        if c.picked.contains(&uid) && normalize_group(&e.group) != t {
                            e.group = t.clone();
                            moved += 1;
                        }
                    }
                }
                if moved > 0 {
                    s.set_quick_commands(v);
                }
            }
            if moved > 0 {
                if let Some(w) = weak.upgrade() {
                    commit(&w, &store_rc, &collapsed, &cm);
                }
            }
            moved
        });
    }

    // ── 批量删除（内联确认后的执行；返回删除数）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_del_picked(move || -> i32 {
            let mut removed = 0i32;
            {
                let mut s = store_rc.borrow_mut();
                let mut c = cm.borrow_mut();
                let mut v = s.quick_commands().to_vec();
                let mut uids = c.uids.clone();
                let mut i = 0;
                while i < v.len() {
                    let picked = uids.get(i).map(|&u| c.picked.contains(&u)).unwrap_or(false);
                    if picked {
                        v.remove(i);
                        uids.remove(i);
                        removed += 1;
                    } else {
                        i += 1;
                    }
                }
                if removed > 0 {
                    s.set_quick_commands(v);
                    c.uids = uids;
                    c.picked.clear();
                    c.anchor = None;
                }
            }
            if removed > 0 {
                if let Some(w) = weak.upgrade() {
                    commit(&w, &store_rc, &collapsed, &cm);
                }
            }
            removed
        });
    }

    // ── 单步移动（卡片 ↑↓）：视图相邻交换（全部命令 = 扁平相邻；组 = 组内）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_step_cmd(move |key: SharedString, dir: i32| {
            let Some(w) = weak.upgrade() else { return };
            let selected = w.get_cm_selected().to_string();
            let mut changed = false;
            {
                let mut s = store_rc.borrow_mut();
                let mut c = cm.borrow_mut();
                let Some(uid) = parse_key(&key) else { return };
                let Some(i) = uid_index(&c, uid) else { return };
                let view = cm_view_indices(&s, &selected);
                let Some(pos) = view.iter().position(|&x| x == i) else { return };
                let to = if dir >= 0 { pos + 1 } else { pos.saturating_sub(1) };
                if to >= view.len() || to == pos {
                    return;
                }
                let j = view[to];
                let mut v = s.quick_commands().to_vec();
                if i < v.len() && j < v.len() {
                    v.swap(i, j);
                    s.set_quick_commands(v);
                    c.uids.swap(i, j);
                    changed = true;
                }
            }
            if changed {
                commit(&w, &store_rc, &collapsed, &cm);
            }
        });
    }

    // ── 拖卡到左栏组行 = 移动分组（整块 = 拖的卡已勾选且勾了多条）──
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_drop_group(move |key: SharedString, target: SharedString| -> i32 {
            let t = normalize_group(&target);
            let mut moved = 0i32;
            {
                let mut s = store_rc.borrow_mut();
                let c = cm.borrow();
                let idxs = moving_indices(&c, &key);
                let mut v = s.quick_commands().to_vec();
                for i in idxs {
                    if let Some(e) = v.get_mut(i) {
                        if normalize_group(&e.group) != t {
                            e.group = t.clone();
                            moved += 1;
                        }
                    }
                }
                if moved > 0 {
                    s.set_quick_commands(v);
                }
            }
            if moved > 0 {
                if let Some(w) = weak.upgrade() {
                    commit(&w, &store_rc, &collapsed, &cm);
                }
            }
            moved
        });
    }

    // ── 拖卡到卡之间 = 排序（四向插入；top/left = 之前）──
    // 视图成员序列重排后按原槽位写回：全部命令 = 全局扁平序、组视图 = 组内
    // 相对序（组外成员不动）。uid 表同步搬移。
    {
        let store_rc = store.clone();
        let weak = window.as_weak();
        let collapsed = collapsed.clone();
        let cm = cm.clone();
        window.on_cm_drop_cards(move |key: SharedString, before: SharedString, after: bool| -> i32 {
            let Some(w) = weak.upgrade() else { return 0 };
            let selected = w.get_cm_selected().to_string();
            let moved;
            {
                let mut s = store_rc.borrow_mut();
                let mut c = cm.borrow_mut();
                let Some(before_uid) = parse_key(&before) else { return 0 };
                let moving: Vec<usize> = moving_indices(&c, &key);
                if moving.is_empty() {
                    return 0;
                }
                let view = cm_view_indices(&s, &selected);
                // 视图序 → 新视图序：moving 按原序整块取出，插到目标前/后
                let mut seq: Vec<usize> = view
                    .iter()
                    .copied()
                    .filter(|i| !moving.contains(i))
                    .collect();
                let before_idx = c
                    .uids
                    .iter()
                    .position(|&u| u == before_uid)
                    .and_then(|i| seq.iter().position(|&x| x == i));
                let at = match before_idx {
                    Some(p) => {
                        let mut p = p;
                        if after {
                            p += 1;
                        }
                        p
                    }
                    None => seq.len(),
                };
                for (off, &m) in moving.iter().enumerate() {
                    seq.insert(at + off, m);
                }
                // 按槽位写回（从旧表取值，避免链式覆盖；cmds + uids 同步）
                let old: Vec<_> = s.quick_commands().to_vec();
                let old_uids = c.uids.clone();
                let mut v = old.clone();
                for (&slot, &src) in view.iter().zip(seq.iter()) {
                    if slot < v.len() && src < old.len() {
                        v[slot] = old[src].clone();
                        c.uids[slot] = old_uids[src];
                    }
                }
                s.set_quick_commands(v);
                moved = moving.len() as i32;
            }
            commit(&w, &store_rc, &collapsed, &cm);
            moved
        });
    }
}
