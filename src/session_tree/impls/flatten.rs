//! (#left-panel-v3 2026-11-06) 树形拍平：`group_records` 树 → 扁平行序列。
//!
//! 关键决策（设计稿 §06 迁移顺序 #2）：
//!  - Slint 只吃扁平行，递归布局一律不做；
//!  - 同组内**子组在前**、成员在后（成员块连续，组内拖拽几何才成立）；
//!  - 折叠（非搜索态）时子层不进模型，组头行保留（计数徽章仍可见）；
//!  - 搜索时强制展开且**不写回**折叠态；组名命中带出全部子孙；组头计数 = 匹配数；
//!  - 根层顺序：system（本地终端，可被 hide_system 整组剔除）→ default
//!    （仅当有成员）→ 用户根组（record 顺序）——与 V2 行序一致，迁移零 surprise。

use std::collections::{HashMap, HashSet};

use super::row::{
    last_used_rank, normalized_query, session_matches_normalized_query, GroupSort, RowKind, TreeRow,
};
use crate::config::{ConfigStore, Session};

/// (V3) 树形拍平上下文：递归展开 group_records 树。
struct FlatCtx<'a> {
    records: &'a [crate::config::GroupRecord],
    by_group: HashMap<&'a str, Vec<&'a Session>>,
    /// 有效色预计算：id -> hex（"" = 无色）。
    colors: HashMap<&'a str, String>,
    name_hit: HashSet<&'a str>,
    searching: bool,
    sort: GroupSort,
}

impl<'a> FlatCtx<'a> {
    fn children(&self, parent: &str) -> Vec<&'a crate::config::GroupRecord> {
        self.records.iter().filter(|r| r.parent_id == parent).collect()
    }

    /// 会话自身字段 + 祖先组名链的联合匹配。
    fn matches_with(&self, s: &Session, query: &str) -> bool {
        if !self.searching {
            return true;
        }
        if session_matches_normalized_query(s, query) {
            return true;
        }
        let mut cur = if s.group_id.is_empty() {
            crate::config::GROUP_ID_DEFAULT.to_string()
        } else {
            s.group_id.clone()
        };
        for _ in 0..16 {
            if self.name_hit.contains(cur.as_str()) {
                return true;
            }
            match self.records.iter().find(|r| r.id == cur) {
                Some(r) if !r.parent_id.is_empty() => cur = r.parent_id.clone(),
                _ => break,
            }
        }
        false
    }

    /// 子树会话总数（不过滤）。
    fn subtree_total(&self, gid: &str) -> usize {
        let own = self.by_group.get(gid).map(|v| v.len()).unwrap_or(0);
        own + self
            .children(gid)
            .iter()
            .map(|r| self.subtree_total(&r.id))
            .sum::<usize>()
    }

    /// 子树匹配会话数（搜索态组头计数）。
    fn subtree_matches(&self, gid: &str, query: &str) -> usize {
        let own = self
            .by_group
            .get(gid)
            .map(|v| v.iter().filter(|s| self.matches_with(s, query)).count())
            .unwrap_or(0);
        own + self
            .children(gid)
            .iter()
            .map(|r| self.subtree_matches(&r.id, query))
            .sum::<usize>()
    }

    fn color_of(&self, gid: &str) -> String {
        self.colors.get(gid).cloned().unwrap_or_default()
    }

    /// 递归展开一个组：组头行 +（未折叠时）子组 + 成员 + 空组占位。
    fn emit_group(&self, gid: &str, depth: usize, query: &str, out: &mut Vec<TreeRow>) {
        let Some(record) = self.records.iter().find(|r| r.id == gid) else {
            return;
        };
        let mut members: Vec<&Session> = self.by_group.get(gid).cloned().unwrap_or_default();
        let kids = self.children(gid);
        let force = self.name_hit.contains(gid);
        let match_count = self.subtree_matches(gid, query);
        // 搜索可见性：组名未命中且子树零匹配 → 整组不出现。
        if self.searching && !force && match_count == 0 {
            return;
        }
        sort_members(&mut members, self.sort);
        let shown: Vec<&&Session> = members
            .iter()
            .filter(|s| self.matches_with(s, query))
            .collect();

        let hex = self.color_of(gid);
        let total = if self.searching { match_count } else { self.subtree_total(gid) };
        let subs = kids.iter().filter(|r| !r.reserved).count();
        let collapsed = record.collapsed && !self.searching;
        out.push(TreeRow {
            row_kind: RowKind::Group,
            id: String::new(),
            name: record.name.clone(),
            kind: String::new(),
            host: String::new(),
            port: 0,
            user: String::new(),
            auth: String::new(),
            group_id: record.id.clone(),
            group: record.name.clone(),
            depth: depth as i32,
            collapsed,
            builtin: record.id == crate::config::GROUP_ID_SYSTEM,
            group_index: 0,
            group_size: 0,
            group_total: total as i32,
            group_subs: if self.searching { 0 } else { subs as i32 },
            note: String::new(),
            color_hex: hex.clone(),
        });
        if collapsed {
            return;
        }
        // 子组在前（设计稿 §06：成员块必须连续，组内拖拽几何才成立）。
        for kid in &kids {
            self.emit_group(&kid.id, depth + 1, query, out);
        }
        if shown.is_empty() {
            if !self.searching || force {
                // 空组占位（非搜索态；搜索时组名命中也显示）。
                out.push(TreeRow {
                    row_kind: RowKind::Empty,
                    id: String::new(),
                    name: String::new(),
                    kind: String::new(),
                    host: String::new(),
                    port: 0,
                    user: String::new(),
                    auth: String::new(),
                    group_id: record.id.clone(),
                    group: record.name.clone(),
                    depth: (depth + 1) as i32,
                    collapsed: false,
                    builtin: false,
                    group_index: 0,
                    group_size: 0,
                    group_total: 0,
                    group_subs: 0,
                    note: String::new(),
                    color_hex: hex.clone(),
                });
            }
            return;
        }
        let group_size = shown.len() as i32;
        for (i, s) in shown.iter().enumerate() {
            let s = **s;
            out.push(TreeRow {
                row_kind: RowKind::Session,
                id: s.id.clone(),
                name: s.name.clone(),
                kind: s.kind.as_str().to_string(),
                host: s.host.clone(),
                port: s.port as i32,
                user: s.user.clone(),
                // (#conn-path) 认证方式原始值：悬浮信息卡「认证」行。
                auth: s.auth.as_str().to_string(),
                note: s.note.clone(),
                group_id: record.id.clone(),
                group: record.name.clone(),
                depth: (depth + 1) as i32,
                collapsed: false,
                builtin: record.id == crate::config::GROUP_ID_SYSTEM,
                group_index: i as i32,
                group_size,
                group_total: 0,
                group_subs: 0,
                color_hex: hex.clone(),
            });
        }
    }
}

fn sort_members(members: &mut Vec<&Session>, sort: GroupSort) {
    match sort {
        GroupSort::Storage => {}
        GroupSort::Name => members.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
        }),
        GroupSort::Recent => members.sort_by(|a, b| last_used_rank(b).cmp(&last_used_rank(a))),
    }
}

/// 拍平入口：ConfigStore → 扁平行（`Vec<TreeRow>`，桥层转 Slint 模型）。
pub(crate) fn build_tree_rows(
    store: &ConfigStore,
    builtin_sessions: &[Session],
    query: &str,
    // (#hide-system-group 2026-09-13) 隐藏"本地终端"保留组：**模型层过滤**。
    // Slint 的 `visible:false` 只隐藏不回收空间,列表上方会残留一整块空白。
    hide_system: bool,
    sort: GroupSort,
) -> Vec<TreeRow> {
    let query = normalized_query(query);
    let searching = !query.is_empty();
    let records = store.group_records();

    // 组名命中集(搜索:组名匹配 → 整组带出,设计稿 T2)。
    let name_hit: HashSet<&str> = if searching {
        records
            .iter()
            .filter(|r| r.name.to_lowercase().contains(&query))
            .map(|r| r.id.as_str())
            .collect()
    } else {
        HashSet::new()
    };

    // 会话按组分桶(builtin 归 system,与旧版一致)。存储序保持。
    let mut by_group: HashMap<&str, Vec<&Session>> = HashMap::new();
    for s in store.sessions() {
        let gid = if s.group_id.is_empty() {
            crate::config::GROUP_ID_DEFAULT
        } else {
            s.group_id.as_str()
        };
        by_group.entry(gid).or_default().push(s);
    }
    for b in builtin_sessions {
        by_group
            .entry(crate::config::GROUP_ID_SYSTEM)
            .or_default()
            .push(b);
    }

    // 有效色预计算(本组 → 最近祖先 → 无)。
    let mut colors: HashMap<&str, String> = HashMap::new();
    for r in records {
        let hex = store
            .group_effective_color(&r.id)
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_default();
        colors.insert(r.id.as_str(), hex);
    }

    let ctx = FlatCtx {
        records,
        by_group,
        colors,
        name_hit,
        searching,
        sort,
    };

    let mut rows: Vec<TreeRow> = Vec::new();
    // 1) system(本地终端):builtin 恒存在时显示;hide_system 整组剔除。
    if !hide_system && records.iter().any(|r| r.id == crate::config::GROUP_ID_SYSTEM) {
        ctx.emit_group(crate::config::GROUP_ID_SYSTEM, 0, &query, &mut rows);
    }
    // 2) default(默认组):有成员(或搜索命中)才显示,与旧版 has_default 同语义。
    let default_hit = if searching {
        ctx.name_hit.contains(crate::config::GROUP_ID_DEFAULT)
            || ctx.subtree_matches(crate::config::GROUP_ID_DEFAULT, &query) > 0
    } else {
        ctx.subtree_total(crate::config::GROUP_ID_DEFAULT) > 0
    };
    if default_hit {
        ctx.emit_group(crate::config::GROUP_ID_DEFAULT, 0, &query, &mut rows);
    }
    // 3) 用户根组(record 顺序;空组保留显示 —— 显式建组就该可见)。
    for kid in ctx.children("") {
        if kid.reserved {
            continue;
        }
        ctx.emit_group(&kid.id, 0, &query, &mut rows);
    }
    rows
}

// ---------------------------------------------------------------------------
// 内置本地终端(builtin)会话集 —— 纯 config 域,无 Slint。
// ---------------------------------------------------------------------------

pub(crate) fn builtin_local_sessions(wsl_profiles: &[crate::config::WslProfile]) -> Vec<Session> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        out.push(builtin_local_session("system:ps", "PowerShell", "powershell"));
        out.push(builtin_local_session("system:cmd", "CMD", "cmd"));
        if wsl_available() {
            if wsl_profiles.is_empty() {
                let mut session = builtin_local_session("system:wsl", "WSL", "wsl");
                session.local_working_dir = "~".to_string();
                out.push(session);
            } else {
                for profile in wsl_profiles {
                    let mut session = builtin_local_session(
                        &format!("system:wsl:{}", profile.id),
                        profile.name.clone(),
                        "wsl",
                    );
                    session.local_distribution = profile.distribution.clone();
                    session.local_working_dir = if profile.directory.trim().is_empty() {
                        "~".to_string()
                    } else {
                        profile.directory.clone()
                    };
                    out.push(session);
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let name = std::path::Path::new(&shell)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Shell")
            .to_string();
        out.push(builtin_local_session("system:shell", name, "shell"));
    }
    out
}

pub(crate) fn builtin_local_session(id: &str, name: impl Into<String>, host: &str) -> Session {
    let mut s = Session::new_empty();
    s.id = id.to_string();
    s.name = name.into();
    s.host = host.to_string();
    s.user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    s.group = "system".to_string();
    s.kind = crate::config::SessionKind::Local;
    s
}

#[cfg(windows)]
pub(crate) fn wsl_available() -> bool {
    use std::os::windows::process::CommandExt;

    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("wsl.exe")
            .arg("--status")
            .creation_flags(0x08000000)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GROUP_ID_DEFAULT;

    fn temp_store() -> ConfigStore {
        let dir = crate::test_tmp_dir("ms-tree-test");
        ConfigStore {
            path: dir.join("config.json"),
            cache: crate::config::ConfigFile::default(),
            key: [7u8; 32],
        }
    }

    /// load/migrate 路径会补建两条保留组;测试直构 store 时手动补齐。
    fn bootstrap_reserved(store: &mut ConfigStore) {
        for (id, name) in [
            (crate::config::GROUP_ID_DEFAULT, "default"),
            (crate::config::GROUP_ID_SYSTEM, "system"),
        ] {
            if !store
                .group_records()
                .iter()
                .any(|r| r.id == id)
            {
                store
                    .cache
                    .group_records
                    .push(crate::config::GroupRecord::reserved(id, name));
            }
        }
    }

    fn session(id: &str, name: &str, host: &str, group_id: &str) -> Session {
        let mut value = Session::new_empty();
        value.id = id.into();
        value.name = name.into();
        value.host = host.into();
        value.group_id = group_id.into();
        value
    }

    #[test]
    fn search_keeps_only_matching_subtrees_and_force_expands() {
        let mut store = temp_store();
        store.add_group("prod".into(), "");
        let prod_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "prod")
            .map(|r| r.id.clone())
            .unwrap();
        store.upsert(session("1", "Prod API", "10.0.0.8", &prod_id));
        store.set_group_collapsed(&prod_id, true);

        let rows = build_tree_rows(&store, &[], "prod", false, GroupSort::Storage);
        assert_eq!(rows.len(), 2, "组头 + 成员,折叠被搜索强制展开");
        assert_eq!(rows[0].row_kind_str(), "group");
        assert!(!rows[0].collapsed, "搜索态强制展开且不写回");
        assert_eq!(rows[1].row_kind_str(), "session");
        assert_eq!(rows[1].name, "Prod API");

        let rows = build_tree_rows(&store, &[], "staging", false, GroupSort::Storage);
        assert!(rows.is_empty(), "零匹配整树不出现");
    }

    #[test]
    fn default_group_hides_when_empty_and_builtin_lands_in_system() {
        let mut store = temp_store();
        bootstrap_reserved(&mut store);
        let builtin = builtin_local_session("sys:1", "PowerShell", "powershell");
        let rows = build_tree_rows(&store, &[builtin], "", false, GroupSort::Storage);
        assert_eq!(rows.len(), 2, "system 组头 + 1 builtin 成员");
        assert_eq!(rows[0].group_id, crate::config::GROUP_ID_SYSTEM);
        assert!(rows[1].builtin);
        assert!(rows
            .iter()
            .all(|r| r.group_id != GROUP_ID_DEFAULT), "空 default 不显示");

        // hide_system 整组剔除。
        let rows = build_tree_rows(
            &store,
            &[builtin_local_session("sys:1", "PowerShell", "powershell")],
            "",
            true,
            GroupSort::Storage,
        );
        assert!(rows.is_empty());
    }

    #[test]
    fn rows_carry_group_totals_and_nested_depth() {
        let mut store = temp_store();
        store.add_group("prod".into(), "");
        let prod_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "prod")
            .map(|r| r.id.clone())
            .unwrap();
        store.add_group("cn".into(), &prod_id);
        let cn_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "cn")
            .map(|r| r.id.clone())
            .unwrap();
        store.upsert(session("1", "api", "10.0.0.1", &cn_id));

        let rows = build_tree_rows(&store, &[], "", false, GroupSort::Storage);
        let prod = rows.iter().find(|r| r.group == "prod").unwrap();
        assert_eq!(prod.depth, 0);
        assert_eq!(prod.group_total, 1, "含子树总数");
        assert_eq!(prod.group_subs, 1, "直接子组数");
        let cn = rows.iter().find(|r| r.group == "cn").unwrap();
        assert_eq!(cn.depth, 1, "子组在前于成员");
        let member = rows.iter().find(|r| r.id == "1").unwrap();
        assert_eq!(member.depth, 2);
        assert_eq!(member.row_kind_str(), "session");
    }

    #[test]
    fn collapse_prunes_children_but_keeps_the_header() {
        let mut store = temp_store();
        store.add_group("prod".into(), "");
        let prod_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "prod")
            .map(|r| r.id.clone())
            .unwrap();
        store.upsert(session("1", "api", "10.0.0.1", &prod_id));
        store.set_group_collapsed(&prod_id, true);

        let rows = build_tree_rows(&store, &[], "", false, GroupSort::Storage);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].row_kind_str(), "group");
        assert!(rows[0].collapsed);
        assert_eq!(rows[0].group_total, 1, "折叠组头仍可见计数");
    }
}
