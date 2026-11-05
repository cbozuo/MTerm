use super::*;
// (#left-panel-v3 2026-11-06) 树形分组:组从组名字符串升级为一等实体
// (GroupRecord),本文件重写为「递归拍平 → 带 depth/row-kind 扁平行」的
// 模型层;旧 named_display_groups / display_group_of 的组名路径退役。
// (#tab-group-bar 2026-09-14) 标签底部色条要用与左侧会话行同一套组色语义。
// (#group-color 2026-09-14) 分组颜色(旧:组名表;新:record 有效色)。
use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;

/// (#group-color 2026-09-14) 解析 `"#RRGGBB"` / `"#RGB"`(前导 `#` 可省)为颜色。
/// Slint 的 `Color` **没有实现 `FromStr`**,所以这里手工解析。其他形式一律视为
/// 无效 → 该组回落"无色"。
pub(super) fn parse_hex_color(hex: &str) -> Option<slint::Color> {
    let h = hex.trim().trim_start_matches('#');
    let (r, g, b) = match h.len() {
        3 => {
            // `#abc` 简写:每位重复一次(a → 0xaa)。
            let dup = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
            (dup(0)?, dup(1)?, dup(2)?)
        }
        6 => (
            u8::from_str_radix(&h[0..2], 16).ok()?,
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
        ),
        _ => return None,
    };
    Some(slint::Color::from_rgb_u8(r, g, b))
}

/// (#group-color 2026-09-14) 把用户输入的 hex 归一到 `#rrggbb`(小写、带 `#`)。
/// 存储形式统一后,面板里"当前选中色"的判定就是一次字符串相等,不必反复解析。
/// 非法输入返回 `None`。
pub(super) fn normalize_hex(hex: &str) -> Option<String> {
    let c = parse_hex_color(hex)?;
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        c.red(),
        c.green(),
        c.blue()
    ))
}

/// (#left-panel-v3) 面板排序方式(设置弹层「排序方式」三档)。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum GroupSort {
    /// 存储序:组 = record Vec 顺序(用户拖动维护);会话 = sessions Vec 内
    /// 同组相对顺序。默认。
    #[default]
    Storage,
    /// 名称:组与会话各自按名称排序(小写比较;无 locale 依赖)。
    Name,
    /// 最近:按 last_used 降序(ISO 字符串字典序即时间序),空值垫底。
    Recent,
}

/// (#tab-group-bar 2026-09-14) 标签底部色条用的组色:`(绘制色, hex)`。
///
/// **必须与左侧会话行同源** —— 走 `group_effective_color`(本组 → 最近祖先),
/// 与树渲染同一条颜色链。builtin 本地终端恒归 `system` 组。
/// hex 为空 = 该组无有效色 → Slint 侧不绘制色条。
pub(super) fn tab_group_color(store: &ConfigStore, session: &Session) -> (slint::Color, String) {
    let builtin = builtin_local_sessions(store.wsl_profiles());
    let gid = if builtin.iter().any(|b| b.id == session.id) {
        crate::config::GROUP_ID_SYSTEM.to_string()
    } else if session.group_id.is_empty() {
        crate::config::GROUP_ID_DEFAULT.to_string()
    } else {
        session.group_id.clone()
    };
    match store
        .group_effective_color(&gid)
        .filter(|hex| !hex.trim().is_empty())
    {
        Some(hex) => (
            parse_hex_color(&hex).unwrap_or_default(),
            hex.trim().to_string(),
        ),
        None => (slint::Color::default(), String::new()),
    }
}

pub(super) fn wsl_profile_model(store: &ConfigStore) -> ModelRc<WslProfileInfo> {
    let rows = store
        .wsl_profiles()
        .iter()
        .map(|profile| WslProfileInfo {
            id: profile.id.clone().into(),
            name: profile.name.clone().into(),
            distribution: profile.distribution.clone().into(),
            directory: profile.directory.clone().into(),
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn parse_batch_import(text: &str) -> Vec<Session> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // splitn(5) so the last field (name) may itself contain '|'.
        let parts: Vec<&str> = line.splitn(5, '|').map(str::trim).collect();
        let host = parts.first().copied().unwrap_or("");
        // Skip blank hosts and a header row like "host|port|username|...".
        if host.is_empty() || host.eq_ignore_ascii_case("host") {
            continue;
        }
        let port = parts
            .get(1)
            .and_then(|p| p.parse::<u16>().ok())
            .filter(|&p| p > 0)
            .unwrap_or(22);
        let user = parts
            .get(2)
            .copied()
            .filter(|s| !s.is_empty())
            .unwrap_or("root");
        let password = parts.get(3).copied().unwrap_or("");
        let name = parts
            .get(4)
            .copied()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{user}@{host}"));
        let mut sess = Session {
            name,
            host: host.to_string(),
            port,
            user: user.to_string(),
            auth: AuthMethod::Password,
            ..Session::new_empty()
        };
        if !password.is_empty() {
            sess.password = Secret::new(password.to_string());
        }
        out.push(sess);
    }
    out
}

/// (#group-dot 2026-09-14 / V3) 会话对话框「分组」下拉项:组 id + 名称 + 颜色
/// + 树深度。顺序 = 默认组首位,其后用户组按树形 DFS(record Vec 顺序)。
/// 未设色的组 hex = "" → Slint 侧只留空位、不画点。
pub(super) fn session_groups_model(store: &ConfigStore) -> ModelRc<GroupEntry> {
    let records = store.group_records();
    let mut entries: Vec<GroupEntry> = Vec::new();
    let count_of = |gid: &str| {
        store
            .sessions()
            .iter()
            .filter(|s| s.group_id == gid)
            .count() as i32
    };
    let push = |entries: &mut Vec<GroupEntry>,
                record: &crate::config::GroupRecord,
                depth: usize| {
        let hex = record.color.clone().unwrap_or_default();
        entries.push(GroupEntry {
            id: record.id.as_str().into(),
            name: record.name.as_str().into(),
            hex: hex.clone().into(),
            color: parse_hex_color(&hex).unwrap_or_default(),
            count: count_of(&record.id),
            depth: depth as i32,
        });
    };
    // default 恒首位(未分组兜底目标);system 不进下拉(存不了普通会话)。
    if let Some(default) = records.iter().find(|r| r.id == crate::config::GROUP_ID_DEFAULT) {
        push(&mut entries, default, 0);
    }
    // 用户组 DFS:同组内子组在前(与树渲染同序),深度即缩进。
    fn walk<'a>(
        records: &'a [crate::config::GroupRecord],
        parent: &str,
        depth: usize,
        entries: &mut Vec<GroupEntry>,
        push: &impl Fn(&mut Vec<GroupEntry>, &'a crate::config::GroupRecord, usize),
    ) {
        for record in records.iter().filter(|r| r.parent_id == parent && !r.reserved) {
            push(entries, record, depth);
            walk(records, &record.id, depth + 1, entries, push);
        }
    }
    walk(records, "", 0, &mut entries, &push);
    ModelRc::from(Rc::new(VecModel::from(entries)))
}

/// (#move-to-groups 2026-09-14 / V3) 成员右键菜单「移动到」的组清单:显示名
/// 列表(默认组恒居首位,用户组树形序;保留组 system 排除)。**临时兼容**:
/// S4 接「移动到分组」弹窗后回调改走 id,届时本函数退役。
pub(super) fn move_target_groups_model(store: &ConfigStore) -> ModelRc<SharedString> {
    let records = store.group_records();
    let mut names: Vec<SharedString> = records
        .iter()
        .find(|r| r.id == crate::config::GROUP_ID_DEFAULT)
        .map(|r| vec![SharedString::from(r.name.as_str())])
        .unwrap_or_default();
    fn walk<'a>(records: &'a [crate::config::GroupRecord], parent: &str, out: &mut Vec<SharedString>) {
        for record in records.iter().filter(|r| r.parent_id == parent && !r.reserved) {
            out.push(SharedString::from(record.name.as_str()));
            walk(records, &record.id, out);
        }
    }
    walk(records, "", &mut names);
    ModelRc::from(Rc::new(VecModel::from(names)))
}

// ---------------------------------------------------------------------------
// (#conn-path 2026-10-04 高保真 #proxy-fields) 代理四字段 ↔ 存储 URL
// ---------------------------------------------------------------------------

/// Parse a stored proxy URL back into the dialog's fields:
/// `(path_type, proto, host, port, user, pass)`.
///
/// `""` → direct。scheme 识别与旧 split_proxy 一致（http/https → http、
/// socks5h/socks5/socks → socks5、无前缀 → socks5，旧配置兼容）。userinfo 从
/// 最后一个 `@` 切、user 取首个 `:` 前 —— 密码里含 `@`/`:` 都能还原。
pub(super) fn split_proxy_parts(url: &str) -> (String, String, String, String, String, String) {
    const DIRECT: &str = "direct";
    let s = url.trim().trim_end_matches('/');
    if s.is_empty() {
        return (
            DIRECT.to_string(),
            "socks5".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );
    }
    let lower = s.to_ascii_lowercase();
    // 先长后短（socks5h 必须在 socks5 之前试）。
    let prefixes = [
        ("https://", "http"),
        ("http://", "http"),
        ("socks5h://", "socks5"),
        ("socks5://", "socks5"),
        ("socks://", "socks5"),
    ];
    let mut proto = "socks5";
    let mut rest = s;
    for (p, name) in prefixes {
        if lower.starts_with(p) {
            proto = name;
            rest = &s[p.len()..];
            break;
        }
    }
    let (user, pass, hostport) = match rest.rsplit_once('@') {
        Some((userinfo, hp)) => match userinfo.split_once(':') {
            Some((u, p)) => (u.to_string(), p.to_string(), hp.to_string()),
            None => (userinfo.to_string(), String::new(), hp.to_string()),
        },
        None => (String::new(), String::new(), rest.to_string()),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.to_string()),
        None => (hostport, String::new()),
    };
    let path_type = if host.is_empty() { DIRECT } else { "proxy" };
    (
        path_type.to_string(),
        proto.to_string(),
        host,
        port,
        user,
        pass,
    )
}

/// Assemble the stored proxy URL from the dialog's four fields
/// (`path_type == "proxy"` 且 host 非空才产出，否则空串 = 不用代理）。
///
/// 代理密码不回显（#10 惯例）：`proxy_pass` 留空且存在旧代理 URL 时复用旧
/// pass，这样「编辑 → 什么都不动 → 保存」不会把代理密码洗掉。
pub(super) fn assemble_proxy(draft: &SessionDraft, existing_proxy: &str) -> String {
    if draft.path_type != "proxy" {
        return String::new();
    }
    let host = draft.proxy_host.trim();
    if host.is_empty() {
        return String::new();
    }
    let pass = if draft.proxy_pass.is_empty() {
        split_proxy_parts(existing_proxy).5
    } else {
        draft.proxy_pass.to_string()
    };
    let mut url = format!(
        "{}://",
        if draft.proxy_type == "http" { "http" } else { "socks5" }
    );
    if !draft.proxy_user.trim().is_empty() {
        url.push_str(draft.proxy_user.trim());
        url.push(':');
        url.push_str(&pass);
        url.push('@');
    }
    url.push_str(host);
    let port = draft.proxy_port.trim();
    if !port.is_empty() {
        url.push(':');
        url.push_str(port);
    }
    url
}

/// Build the jump-host picker's parallel label/id lists for the session dialog
/// (#211). Index 0 is always the "no jump host" entry (empty id); the rest are
/// the saved SSH sessions except `exclude_id` (a session can't jump through
/// itself). Returns `(labels, ids, selected_index)` where `selected_index`
/// points at `current_jump_id` (0 if unset / dangling).
pub(super) fn jump_candidates(
    store: &ConfigStore,
    exclude_id: &str,
    current_jump_id: &str,
) -> (ModelRc<SharedString>, ModelRc<SharedString>, i32) {
    let mut labels: Vec<SharedString> = vec![t("无（直接连接）", "None (direct)").into()];
    let mut ids: Vec<SharedString> = vec!["".into()];
    let mut selected: i32 = 0;
    for s in store.sessions() {
        if s.kind != SessionKind::Ssh || s.id == exclude_id {
            continue;
        }
        let label = if s.name.trim().is_empty() {
            if s.user.trim().is_empty() {
                s.host.clone()
            } else {
                format!("{}@{}", s.user, s.host)
            }
        } else {
            format!("{} ({}@{})", s.name, s.user, s.host)
        };
        if s.id == current_jump_id {
            selected = ids.len() as i32;
        }
        labels.push(label.into());
        ids.push(s.id.clone().into());
    }
    (
        ModelRc::from(Rc::new(VecModel::from(labels))),
        ModelRc::from(Rc::new(VecModel::from(ids))),
        selected,
    )
}

/// (#conn-path 2026-10-04 高保真 #proxy-fields) Session → 跳板链条目(信息卡
/// 数据源):label 沿用 jump_candidates 的命名规则,auth 存原始值由 slint 侧
/// auth-label() 转显示名。
pub(super) fn jump_entry(s: &Session, group_display: &str) -> JumpEntry {
    let label = if s.name.trim().is_empty() {
        if s.user.trim().is_empty() {
            s.host.clone()
        } else {
            format!("{}@{}", s.user, s.host)
        }
    } else {
        format!("{} ({}@{})", s.name, s.user, s.host)
    };
    JumpEntry {
        id: s.id.clone().into(),
        label: label.into(),
        name: s.name.clone().into(),
        host: s.host.clone().into(),
        port: s.port as i32,
        user: s.user.clone().into(),
        auth: s.auth.as_str().into(),
        group: group_display.into(),
        note: s.note.clone().into(),
    }
}

/// (#conn-path) 跳板链候选池:全部已保存 SSH 会话,排除 `exclude_id`(自身)
/// 与 `skip`(已在链中的 id 集合 —— 防环:下拉只列未入链项)。
pub(super) fn build_jump_pool(store: &ConfigStore, exclude_id: &str, skip: &HashSet<String>) -> Vec<JumpEntry> {
    let mut out = Vec::new();
    let group_name_of = |gid: &str| -> String {
        store
            .group_records()
            .iter()
            .find(|r| r.id == gid)
            .map(|r| r.name.clone())
            .unwrap_or_default()
    };
    for s in store.sessions() {
        if s.kind != SessionKind::Ssh || s.id == exclude_id || skip.contains(&s.id) {
            continue;
        }
        let gid = if s.group_id.is_empty() {
            crate::config::GROUP_ID_DEFAULT
        } else {
            s.group_id.as_str()
        };
        out.push(jump_entry(s, &group_name_of(gid)));
    }
    out
}

fn normalized_query(query: &str) -> String {
    query.trim().to_lowercase()
}

#[cfg(test)]
fn session_matches_query(session: &Session, query: &str) -> bool {
    let query = normalized_query(query);
    session_matches_normalized_query(session, &query)
}

/// (V3) 匹配口径(设计稿 T2):名称 / 主机 / 用户 / 备注 / 协议 / 认证方式。
/// 组名的命中(整组带出)由 `FlatCtx::matches` 的祖先链回溯承担。
fn session_matches_normalized_query(session: &Session, query: &str) -> bool {
    query.is_empty()
        || session.name.to_lowercase().contains(query)
        || session.host.to_lowercase().contains(query)
        || session.user.to_lowercase().contains(query)
        || session.note.to_lowercase().contains(query)
        || session.kind.as_str().to_lowercase().contains(query)
}

/// (V3) last_used 排序键:ISO 字符串字典序即时间序;空值恒垫底。
fn last_used_rank(s: &Session) -> (bool, String) {
    match &s.last_used {
        Some(v) if !v.trim().is_empty() => (true, v.clone()),
        _ => (false, String::new()),
    }
}

/// (V3) 树形拍平上下文:递归展开 group_records 树,产出「组头行 → 子组 →
/// 成员」扁平序列。关键决策(设计稿 §06 迁移顺序 #2):
///  - Slint 只吃扁平行,递归布局一律不做;
///  - 同组内**子组在前**、成员在后(成员块连续,组内拖拽几何才成立);
///  - 折叠(非搜索态)时子层不进模型,组头行保留(计数徽章仍可见);
///  - 搜索时强制展开且**不写回**折叠态;组名命中带出全部子孙;组头计数 = 匹配数;
///  - 根层顺序:system(本地终端,可被 hide_system 整组剔除)→ default
///    (仅当有成员)→ 用户根组(record 顺序)——与旧版行序一致,迁移零 surprise。
struct FlatCtx<'a> {
    records: &'a [crate::config::GroupRecord],
    by_group: HashMap<&'a str, Vec<&'a Session>>,
    /// 有效色预计算:id -> (Color, hex)("" = 无色)。
    colors: HashMap<&'a str, (slint::Color, String)>,
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

    /// 子树会话总数(不过滤)。
    fn subtree_total(&self, gid: &str) -> usize {
        let own = self.by_group.get(gid).map(|v| v.len()).unwrap_or(0);
        own + self
            .children(gid)
            .iter()
            .map(|r| self.subtree_total(&r.id))
            .sum::<usize>()
    }

    /// 子树匹配会话数(搜索态组头计数)。
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

    fn color_of(&self, gid: &str) -> (slint::Color, String) {
        self.colors
            .get(gid)
            .cloned()
            .unwrap_or((slint::Color::default(), String::new()))
    }

    /// 递归展开一个组:组头行 + (未折叠时)子组 + 成员 + 空组占位。
    fn emit_group(&self, gid: &str, depth: usize, query: &str, out: &mut Vec<SessionInfo>) {
        let Some(record) = self.records.iter().find(|r| r.id == gid) else {
            return;
        };
        let mut members: Vec<&Session> = self.by_group.get(gid).cloned().unwrap_or_default();
        let kids = self.children(gid);
        let force = self.name_hit.contains(gid);
        let match_count = self.subtree_matches(gid, query);
        // 搜索可见性:组名未命中且子树零匹配 → 整组不出现。
        if self.searching && !force && match_count == 0 {
            return;
        }
        sort_members(&mut members, self.sort);
        let shown: Vec<&&Session> = members
            .iter()
            .filter(|s| self.matches_with(s, query))
            .collect();

        let (gc, hex) = self.color_of(gid);
        let total = if self.searching { match_count } else { self.subtree_total(gid) };
        let subs = kids.iter().filter(|r| !r.reserved).count();
        let collapsed = record.collapsed && !self.searching;
        out.push(SessionInfo {
            id: "".into(),
            name: record.name.as_str().into(),
            kind: "".into(),
            host: "".into(),
            port: 0,
            user: "".into(),
            auth: "".into(),
            group_id: record.id.as_str().into(),
            group: record.name.as_str().into(),
            depth: depth as i32,
            collapsed,
            builtin: record.id == crate::config::GROUP_ID_SYSTEM,
            conn_state: 0,
            group_index: 0,
            group_size: 0,
            group_total: total as i32,
            group_subs: if self.searching { 0 } else { subs as i32 },
            note: "".into(),
            group_color: gc,
            group_color_hex: hex.as_str().into(),
            row_kind: "group".into(),
        });
        if collapsed {
            return;
        }
        // 子组在前(设计稿 §06:成员块必须连续,组内拖拽几何才成立)。
        for kid in &kids {
            self.emit_group(&kid.id, depth + 1, query, out);
        }
        if shown.is_empty() {
            if !self.searching || force {
                // 空组占位(非搜索态;搜索时组名命中也显示)。
                out.push(SessionInfo {
                    id: "".into(),
                    name: "".into(),
                    kind: "".into(),
                    host: "".into(),
                    port: 0,
                    user: "".into(),
                    auth: "".into(),
                    group_id: record.id.as_str().into(),
                    group: record.name.as_str().into(),
                    depth: (depth + 1) as i32,
                    collapsed: false,
                    builtin: false,
                    conn_state: 0,
                    group_index: 0,
                    group_size: 0,
                    group_total: 0,
                    group_subs: 0,
                    note: "".into(),
                    group_color: gc,
                    group_color_hex: hex.as_str().into(),
                    row_kind: "empty".into(),
                });
            }
            return;
        }
        let group_size = shown.len() as i32;
        for (i, s) in shown.iter().enumerate() {
            let s = **s;
            out.push(SessionInfo {
                id: s.id.clone().into(),
                name: s.name.clone().into(),
                kind: s.kind.as_str().into(),
                host: s.host.clone().into(),
                port: s.port as i32,
                user: s.user.clone().into(),
                // (#conn-path) 认证方式原始值:悬浮信息卡「认证」行。
                auth: s.auth.as_str().into(),
                note: s.note.clone().into(),
                group_id: record.id.as_str().into(),
                group: record.name.as_str().into(),
                depth: (depth + 1) as i32,
                collapsed: false,
                builtin: record.id == crate::config::GROUP_ID_SYSTEM,
                conn_state: 0,
                group_index: i as i32,
                group_size,
                group_total: 0,
                group_subs: 0,
                group_color: gc,
                group_color_hex: hex.as_str().into(),
                row_kind: "session".into(),
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


fn build_session_rows(
    store: &ConfigStore,
    builtin_sessions: &[Session],
    query: &str,
    // (#hide-system-group 2026-09-13) 隐藏"本地终端"保留组:**模型层过滤**。
    // Slint 的 `visible:false` 只隐藏不回收空间,列表上方会残留一整块空白。
    hide_system: bool,
    sort: GroupSort,
) -> Vec<SessionInfo> {
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
    let mut colors: HashMap<&str, (slint::Color, String)> = HashMap::new();
    for r in records {
        let hex = store
            .group_effective_color(&r.id)
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_default();
        let color = parse_hex_color(&hex).unwrap_or_default();
        colors.insert(r.id.as_str(), (color, hex));
    }

    let ctx = FlatCtx {
        records,
        by_group,
        colors,
        name_hit,
        searching,
        sort,
    };

    let mut rows: Vec<SessionInfo> = Vec::new();
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

pub(super) fn sync_sessions_to_model_with_filter(
    store: &ConfigStore,
    model: &VecModel<SessionInfo>,
    query: &str,
) {
    sync_sessions_to_model_with_sort(store, model, query, GroupSort::Storage);
}

/// (V3) 带排序档位的模型刷新(面板设置「排序方式」)。group_records 为空时
/// (异常配置)兜底补建保留组,防止树渲染全空。
pub(super) fn sync_sessions_to_model_with_sort(
    store: &ConfigStore,
    model: &VecModel<SessionInfo>,
    query: &str,
    sort: GroupSort,
) {
    let builtin_sessions = builtin_local_sessions(store.wsl_profiles());
    let rows = build_session_rows(store, &builtin_sessions, query, store.system_group_hidden(), sort);
    model.set_vec(rows);
}

/// Same rows as `sync_sessions_to_model_with_filter`, but when the row count
/// is unchanged the rows are written with `set_row_data` instead of `set_vec`:
/// the `for` loop keeps its elements (and a drag's pointer grab) alive. Used
/// for per-hop updates during drag-to-reorder. Returns false when the row
/// count changed and a full `set_vec` rebuild was required — that recreates
/// the rows and drops the dragging row's pointer grab.
pub(super) fn refresh_session_rows_in_place(
    store: &ConfigStore,
    model: &VecModel<SessionInfo>,
    query: &str,
) -> bool {
    use slint::Model as _;
    let builtin_sessions = builtin_local_sessions(store.wsl_profiles());
    let rows = build_session_rows(
        store,
        &builtin_sessions,
        query,
        store.system_group_hidden(),
        GroupSort::Storage,
    );
    if rows.len() == model.row_count() {
        for (i, row) in rows.into_iter().enumerate() {
            model.set_row_data(i, row);
        }
        true
    } else {
        model.set_vec(rows);
        false
    }
}

pub(super) fn sync_sessions_to_model(store: &ConfigStore, model: &VecModel<SessionInfo>) {
    sync_sessions_to_model_with_filter(store, model, "");
}

pub(super) fn builtin_local_sessions(wsl_profiles: &[crate::config::WslProfile]) -> Vec<Session> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        out.push(builtin_local_session(
            "system:powershell",
            "PowerShell",
            "powershell",
        ));
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

pub(super) fn builtin_local_session(id: &str, name: impl Into<String>, host: &str) -> Session {
    let mut s = Session::new_empty();
    s.id = id.to_string();
    s.name = name.into();
    s.host = host.to_string();
    s.user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    s.group = "system".to_string();
    s.kind = SessionKind::Local;
    s
}

#[cfg(windows)]
pub(super) fn wsl_available() -> bool {
    use std::os::windows::process::CommandExt;

    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("wsl.exe")
            .arg("--status")
            .creation_flags(0x08000000)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

// ---------------------------------------------------------------------------
// RDP display settings
// ---------------------------------------------------------------------------

/// Resolution dropdown values shared by the dialog and the stored session.
/// `fullscreen` and `custom` are the two non-preset entries; presets are the
/// `"WxH"` strings below.
///
/// (#rdp-vnc-embedded) `follow` is a third non-preset entry, offered **only** in
/// the built-in client's resolution list: it means "resize the remote desktop
/// to the tab". It is stored as the separate `Session::rdp_follow_tab_size`
/// boolean rather than as a width/height pair, because the size is then decided
/// at runtime and no pair of numbers describes it.
pub(super) const RDP_RESOLUTION_FOLLOW: &str = "follow";
pub(super) const RDP_RESOLUTION_FULLSCREEN: &str = "fullscreen";
pub(super) const RDP_RESOLUTION_CUSTOM: &str = "custom";
pub(super) const RDP_RESOLUTION_DEFAULT: &str = "1280x720";
pub(super) const RDP_RESOLUTION_PRESETS: [&str; 7] = [
    "1280x720",
    "1366x768",
    "1440x900",
    "1600x900",
    "1920x1080",
    "2560x1440",
    "3840x2160",
];

/// Smallest/largest desktop side accepted from the dialog. The system client
/// clamps further on its own, but sizes outside this range are clearly typos.
pub(super) const RDP_MIN_SIDE: i32 = 200;
pub(super) const RDP_MAX_SIDE: i32 = 8192;

/// Whether the draft asks the built-in client to track the tab size.
///
/// (#rdp-follow-lost 2026-10-13) The dialog has exactly one control for this —
/// the resolution dropdown — and it writes only `draft.rdp_resolution`;
/// `draft.rdp_follow_tab_size` is a carry-over the dialog never touches (it
/// exists so a stored config keeps its meaning). **Every** save path must
/// therefore OR the two, or choosing "Follow tab" in the dialog gets silently
/// dropped. Both callers share this function so the rule cannot drift again —
/// that drift is exactly how the feature broke: `app.rs`'s hand-rolled
/// constructor wrote the raw bool and ignored the dropdown.
pub(super) fn follows_tab_size(draft: &SessionDraft) -> bool {
    draft.rdp_follow_tab_size || draft.rdp_resolution.to_string() == RDP_RESOLUTION_FOLLOW
}

/// Turn the dialog's resolution choice into the stored display settings
/// `(fullscreen, width, height)`. A `"WxH"` preset carries its own size;
/// the two free inputs are only trusted for `custom` (non-positive values
/// fall back to the defaults, positive ones are clamped into range).
/// Unrecognised strings fall back to full screen.
pub(super) fn rdp_display_settings(
    resolution: &str,
    custom_width: i32,
    custom_height: i32,
) -> (bool, u16, u16) {
    if resolution == RDP_RESOLUTION_FULLSCREEN {
        return (true, 1280, 720);
    }
    // (#rdp-follow-default 2026-10-13) "Follow tab" has no size of its own —
    // the view decides the size at runtime — so it must not fall through to
    // the full-screen fallback below. That fallback would leak "full screen"
    // into a session later switched to the system client, and would make the
    // built-in client open at a wrong guess. A plain window at the default
    // size is the honest placeholder; `follow_tab_size` carries the intent.
    if resolution == RDP_RESOLUTION_FOLLOW {
        return (false, 1280, 720);
    }
    if resolution == RDP_RESOLUTION_CUSTOM {
        let clamp_side = |value: i32, default: u16| -> u16 {
            if value <= 0 {
                default
            } else {
                value.clamp(RDP_MIN_SIDE, RDP_MAX_SIDE) as u16
            }
        };
        return (
            false,
            clamp_side(custom_width, 1280),
            clamp_side(custom_height, 720),
        );
    }
    if let Some((width, height)) = resolution.split_once('x') {
        if let (Ok(width), Ok(height)) = (width.parse::<u16>(), height.parse::<u16>()) {
            let (wide, high) = (i32::from(width), i32::from(height));
            if (RDP_MIN_SIDE..=RDP_MAX_SIDE).contains(&wide)
                && (RDP_MIN_SIDE..=RDP_MAX_SIDE).contains(&high)
            {
                return (false, width, height);
            }
        }
    }
    (true, 1280, 720)
}

/// Map stored display settings back to the dialog's resolution entry: one of
/// the preset `"WxH"` strings, `fullscreen`, or `custom` for any other size.
pub(super) fn rdp_resolution_choice(fullscreen: bool, width: u16, height: u16) -> &'static str {
    if fullscreen {
        return RDP_RESOLUTION_FULLSCREEN;
    }
    let size = format!("{}x{}", width, height);
    RDP_RESOLUTION_PRESETS
        .iter()
        .copied()
        .find(|preset| *preset == size)
        .unwrap_or(RDP_RESOLUTION_CUSTOM)
}

// ---------------------------------------------------------------------------
// Session callbacks (welcome page + dialog)
// ---------------------------------------------------------------------------

/// Build the effective session represented by the dialog.
///
/// (#blank-password-overwrites 2026-10-13) The password box is authoritative:
/// whatever it holds is what gets saved, **including nothing**. A blank field
/// therefore means "no password", not "keep the saved one" — that is the only
/// way to bring a saved session back to an empty password (the remote host in
/// this project genuinely has none) without adding a "clear password" control,
/// which the user explicitly declined. One box covers every credential the
/// dialog has: SSH password, SSH key passphrase, RDP and VNC all share
/// `SessionDraft::password`, so they all follow this rule.
///
/// This knowingly drops the "a blank field keeps the saved password" half of
/// the #10 convention. #10's other half still stands — the stored password is
/// never echoed back into the dialog — so the box opens empty on every edit and
/// saving an untouched dialog writes an empty password. The user accepted that
/// trade; do **not** "fix" it by pre-filling the field with the secret.
///
/// The pasted private key keeps the old blank-keeps behaviour (#276): it is key
/// material with no "empty" meaning of its own, so blanking it could only ever
/// destroy a key by accident.
pub(super) fn session_from_draft(
    draft: &SessionDraft,
    existing: Option<&Session>,
    forwards: Vec<crate::config::PortForward>,
    triggers: Vec<crate::config::SessionTrigger>,
) -> Session {
    // Blank overwrites: deliberately no `existing` fallback for this field.
    let password = Secret::new(draft.password.to_string());
    let private_key_inline = if draft.private_key_inline_mode {
        if draft.private_key_inline.is_empty() {
            existing
                .map(|s| s.private_key_inline.clone())
                .unwrap_or_default()
        } else {
            Secret::new(draft.private_key_inline.to_string())
        }
    } else {
        Secret::default()
    };
    let private_key_path = if draft.private_key_inline_mode {
        String::new()
    } else {
        draft.private_key_path.to_string().replace('\\', "/")
    };
    let kind = SessionKind::from_str(&draft.kind.to_string());
    let auto_name = match kind {
        SessionKind::Serial => format!("{} @{}", draft.serial_port, draft.baud_rate),
        _ if draft.user.trim().is_empty() => draft.host.to_string(),
        _ => format!("{}@{}", draft.user, draft.host),
    };
    let default_port = match kind {
        SessionKind::Telnet => 23,
        SessionKind::Rdp => 3389,
        SessionKind::Vnc => 5900,
        _ => 22,
    };
    let (rdp_fullscreen, rdp_width, rdp_height) =
        rdp_display_settings(&draft.rdp_resolution.to_string(), draft.rdp_width, draft.rdp_height);
    // (#rdp-vnc-embedded) 「跟随标签页」是内置客户端独有的选项，它不是一个宽高
    // 对（尺寸在运行时才由标签页决定），所以单独落成一个 bool。判据见
    // `follows_tab_size` —— 保存路径必须共用它，否则分辨率下拉里的选择会丢。
    let rdp_follow_tab_size = follows_tab_size(draft);

    Session {
        id: draft.id.to_string(),
        name: if draft.name.is_empty() {
            auto_name
        } else {
            draft.name.to_string()
        },
        host: draft.host.to_string(),
        port: if draft.port <= 0 {
            default_port
        } else {
            draft.port as u16
        },
        user: draft.user.to_string(),
        auth: AuthMethod::from_str(&draft.auth.to_string()),
        password,
        private_key_path,
        private_key_inline,
        // (#conn-path) 由四字段拼装(编辑时代理密码留空 → 复用旧值)。
        proxy: assemble_proxy(
            draft,
            existing.map(|s| s.proxy.as_str()).unwrap_or(""),
        ),
        last_used: None,
        // (#left-panel-v3) draft.group = GroupEntry.id(下拉提交 id,不再是
        // 组名)。legacy group 字段同值兜底(load 迁移会按名/容错解析)。
        group_id: draft.group.to_string(),
        group: draft.group.to_string(),
        kind,
        local_distribution: String::new(),
        local_working_dir: String::new(),
        serial_port: draft.serial_port.to_string(),
        baud_rate: if draft.baud_rate <= 0 {
            115_200
        } else {
            draft.baud_rate as u32
        },
        data_bits: draft.data_bits as u8,
        stop_bits: draft.stop_bits as u8,
        parity: draft.parity.to_string(),
        flow_control: draft.flow_control.to_string(),
        rdp_domain: draft.rdp_domain.to_string(),
        rdp_fullscreen,
        rdp_width,
        rdp_height,
        // (#rdp-vnc-embedded) The segmented control the dialog shows for RDP.
        // Anything the draft does not carry keeps the pre-existing default, so
        // a system-client session saved before this feature existed round-trips
        // byte-identically through save and reload.
        rdp_mode: crate::config::RdpClientMode::from_str(&draft.rdp_mode.to_string()),
        rdp_follow_tab_size,
        rdp_embedded_fullscreen: false,
        vnc_view_only: draft.vnc_view_only,
        vnc_security: draft.vnc_security.to_string(),
        vnc_clipboard: draft.vnc_clipboard,
        encoding: draft.encoding.to_string(),
        vt100_drawing: draft.vt100_drawing,
        forwards,
        triggers,
        disable_shell_integration: draft.disable_shell_integration,
        note: draft.note.to_string(),
        jump_session_id: draft.jump_session_id.to_string(),
        jump_chain: Vec::new(),
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;

    fn session(id: &str, name: &str, host: &str, group: &str) -> Session {
        let mut value = Session::new_empty();
        value.id = id.into();
        value.name = name.into();
        value.host = host.into();
        value.group = group.into();
        value
    }

    #[test]
    fn session_search_matches_name_and_host_case_insensitively() {
        let value = session("1", "Prod API", "DB.EXAMPLE.COM", "prod");
        assert!(session_matches_query(&value, "  prod  "));
        assert!(session_matches_query(&value, "example.com"));
        assert!(!session_matches_query(&value, "staging"));
    }

    #[test]
    fn filtered_rows_hide_empty_groups_and_expand_matches() {
        let saved = vec![session("1", "Prod API", "10.0.0.8", "prod")];
        let builtins = vec![session("local", "Local terminal", "localhost", "system")];
        let groups = vec!["empty".to_string(), "prod".to_string()];
        let collapsed = vec!["prod".to_string(), "system".to_string()];

        let rows = build_session_rows(&saved, &groups, Some(&collapsed), &HashMap::new(), &builtins, "prod", false);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_str(), "Prod API");
        assert_eq!(rows[0].group_header.as_str(), "prod");
        assert!(!rows[0].collapsed);
    }

    #[test]
    fn filtered_rows_include_matching_builtin_sessions() {
        let builtins = vec![session("local", "Local terminal", "localhost", "system")];

        let rows = build_session_rows(
            &[],
            &[],
            Some(&["system".to_string()]),
            &HashMap::new(),
            &builtins,
            "LOCALHOST",
            false,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_str(), "Local terminal");
        assert_eq!(rows[0].group_header.as_str(), "system");
        assert!(rows[0].builtin);
        assert!(!rows[0].collapsed);
    }

    #[test]
    fn filtered_rows_are_empty_when_nothing_matches() {
        let saved = vec![session("1", "Prod API", "10.0.0.8", "prod")];
        let builtins = vec![session("local", "Local terminal", "localhost", "system")];

        let rows = build_session_rows(&saved, &[], None, &HashMap::new(), &builtins, "staging", false);

        assert!(rows.is_empty());
    }

    #[test]
    fn empty_query_restores_saved_groups_and_collapse_state() {
        let saved = vec![session("1", "Prod API", "10.0.0.8", "prod")];
        let groups = vec!["empty".to_string(), "prod".to_string()];
        let collapsed = vec!["prod".to_string()];

        let rows = build_session_rows(&saved, &groups, Some(&collapsed), &HashMap::new(), &[], "", false);

        assert!(rows
            .iter()
            .any(|row| row.group.as_str() == "empty" && row.id.is_empty()));
        assert!(rows
            .iter()
            .any(|row| row.group.as_str() == "prod" && row.collapsed));
    }
}

#[cfg(test)]

mod drag_order_tests {
    use super::*;

    fn sess(id: &str, name: &str, group: &str) -> Session {
        let mut value = Session::new_empty();
        value.id = id.into();
        value.name = name.into();
        value.group = group.into();
        value
    }

    /// (#drag-cross-group-fix 2026-09-06) 显示组序必须等于存储组序(组头拖动
    /// 换位维护的顺序),不得按字母重排——否则跨组拖动的"相邻组"判定与显示
    /// 相邻组不符,向上拖会被判进保留组 system 而静默 no-op(首行卡在组边
    /// 界)。且每一行都要携带真实 group_size:非首行旧实现填 0,拖拽起拖时
    /// drag-size 取自被拖行,组内窗口 dn∈[-gi, size-1-gi] 塌缩,组内排序
    /// 被误判成跨组移动。
    #[test]
    fn group_rows_follow_stored_order_and_carry_group_size() {
        let saved = vec![
            sess("1", "202", "测试"),
            sess("2", "s", "Test"),
            sess("3", "t2", "Test"),
        ];
        // 存储顺序:测试 在 Test 之前(字母序会把它排到后面)。
        let groups = vec!["测试".to_string(), "Test".to_string()];

        let rows = build_session_rows(&saved, &groups, None, &HashMap::new(), &[], "", false);

        let headers: Vec<&str> = rows
            .iter()
            .filter(|r| !r.group_header.is_empty())
            .map(|r| r.group_header.as_str())
            .collect();
        assert_eq!(headers, ["测试", "Test"]);

        for row in rows.iter().filter(|r| r.group.as_str() == "Test") {
            assert_eq!(row.group_size, 2, "every row must carry the real group size");
        }
        let first_test = rows.iter().find(|r| r.group.as_str() == "Test").unwrap();
        assert_eq!(first_test.group_header.as_str(), "Test");
        assert_eq!(first_test.group_index, 0);
        let second_test = rows
            .iter()
            .filter(|r| r.group.as_str() == "Test")
            .nth(1)
            .unwrap();
        assert_eq!(second_test.group_index, 1);
    }
}

#[cfg(test)]
mod row_display_tests {
    use super::*;

    // (#proto-icon 2026-09-17) 每一行的 kind 必须原样带出协议名 —— 它是成员行
    // 图标(Theme.protocol-icon)的唯一数据源。各协议各测一遍。
    #[test]
    fn rows_carry_the_protocol_kind_for_the_row_icon() {
        for (kind, expected) in [
            (SessionKind::Ssh, "ssh"),
            (SessionKind::Serial, "serial"),
            (SessionKind::Telnet, "telnet"),
            (SessionKind::Local, "local"),
        ] {
            let mut session = Session::new_empty();
            session.kind = kind;
            let rows = build_session_rows(&[session], &[], None, &HashMap::new(), &[], "", false);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].kind.as_str(), expected);
        }
    }

    // 内置本地会话(builtin)走另一条构造分支,kind 同样要带出来(local)。
    #[test]
    fn builtin_rows_carry_local_kind() {
        let builtin = builtin_local_session("id", "PowerShell", "powershell");
        let rows = build_session_rows(&[], &[], None, &HashMap::new(), &[builtin], "", false);
        let row = rows
            .iter()
            .find(|r| r.builtin && r.name.as_str() == "PowerShell")
            .expect("builtin row");
        assert_eq!(row.kind.as_str(), "local");
    }

    #[test]
    fn network_rows_keep_their_address_fields() {
        for kind in [SessionKind::Ssh, SessionKind::Telnet] {
            let mut session = Session::new_empty();
            session.kind = kind;
            session.host = "example.com".into();
            session.port = 2222;
            session.user = "alice".into();
            let rows = build_session_rows(&[session], &[], None, &HashMap::new(), &[], "", false);
            assert_eq!(rows[0].host.as_str(), "example.com");
            assert_eq!(rows[0].port, 2222);
            assert_eq!(rows[0].user.as_str(), "alice");
        }
    }
}

#[cfg(test)]
mod rdp_display_tests {
    use super::*;

    #[test]
    fn fullscreen_choice_stores_fullscreen() {
        assert_eq!(rdp_display_settings("fullscreen", 0, 0), (true, 1280, 720));
    }

    #[test]
    fn presets_round_trip_through_the_stored_fields() {
        for preset in RDP_RESOLUTION_PRESETS {
            let (fullscreen, width, height) = rdp_display_settings(preset, 0, 0);
            assert!(!fullscreen, "{preset}");
            assert_eq!(rdp_resolution_choice(fullscreen, width, height), preset);
        }
    }

    #[test]
    fn custom_choice_clamps_positive_sizes_and_defaults_junk() {
        assert_eq!(rdp_display_settings("custom", 0, 99_999), (false, 1280, 8192));
        assert_eq!(rdp_display_settings("custom", -5, 720), (false, 1280, 720));
        assert_eq!(rdp_display_settings("custom", 1000, 1000), (false, 1000, 1000));
        assert_eq!(rdp_resolution_choice(false, 1000, 1000), RDP_RESOLUTION_CUSTOM);
    }

    #[test]
    fn unrecognised_resolution_falls_back_to_fullscreen() {
        assert_eq!(rdp_display_settings("garbage", 1920, 1080), (true, 1280, 720));
        assert_eq!(rdp_display_settings("", 1920, 1080), (true, 1280, 720));
    }
}

#[cfg(test)]
mod blank_password_tests {
    use super::*;

    /// (#blank-password-overwrites 2026-10-13) A whole draft with only the
    /// fields under test differing from the dialog's own defaults.
    fn draft() -> SessionDraft {
        SessionDraft {
            id: "s1".into(),
            name: "".into(),
            kind: "rdp".into(),
            host: "192.168.0.69".into(),
            port: 3389,
            user: "Administrator".into(),
            auth: "password".into(),
            password: "".into(),
            private_key_path: "".into(),
            private_key_inline: "".into(),
            private_key_inline_mode: false,
            proxy: "".into(),
            path_type: "direct".into(),
            proxy_type: "socks5".into(),
            proxy_host: "".into(),
            proxy_port: "".into(),
            proxy_user: "".into(),
            proxy_pass: "".into(),
            group: "".into(),
            serial_port: "".into(),
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: "none".into(),
            flow_control: "none".into(),
            rdp_domain: "".into(),
            rdp_resolution: "1280x720".into(),
            rdp_width: 1280,
            rdp_height: 720,
            rdp_mode: "embedded".into(),
            rdp_follow_tab_size: false,
            vnc_view_only: false,
            vnc_security: "".into(),
            vnc_clipboard: true,
            encoding: "UTF-8".into(),
            vt100_drawing: true,
            disable_shell_integration: false,
            note: "".into(),
            jump_session_id: "".into(),
        }
    }

    fn saved_with_password(password: &str) -> Session {
        let mut session = Session::new_empty();
        session.id = "s1".into();
        session.password = Secret::new(password.to_string());
        session
    }

    /// The whole point of the change: a blank box means "no password", so
    /// clearing it is how a saved password gets taken off a session.
    #[test]
    fn a_blank_box_overwrites_the_saved_password() {
        let existing = saved_with_password("hunter2");
        let session = session_from_draft(&draft(), Some(&existing), Vec::new(), Vec::new());
        assert!(session.password.is_empty());
    }

    /// ...and typing one still replaces it.
    #[test]
    fn a_typed_password_replaces_the_saved_one() {
        let existing = saved_with_password("old");
        let mut draft = draft();
        draft.password = "new".into();
        let session = session_from_draft(&draft, Some(&existing), Vec::new(), Vec::new());
        assert_eq!(session.password.as_str(), "new");
    }

    /// The rule does not depend on edit state: a new session with a blank box
    /// has no password either.
    #[test]
    fn a_new_session_with_a_blank_box_has_no_password() {
        let session = session_from_draft(&draft(), None, Vec::new(), Vec::new());
        assert!(session.password.is_empty());
    }

    /// A single space is a password a user may legitimately have, so nothing
    /// here may grow a `trim()`.
    #[test]
    fn a_single_space_is_still_a_password() {
        let mut draft = draft();
        draft.password = " ".into();
        let session = session_from_draft(&draft, Some(&saved_with_password("x")), Vec::new(), Vec::new());
        assert_eq!(session.password.as_str(), " ");
    }

    /// #276 still holds for the pasted private key: it is key material with no
    /// "empty" meaning of its own, so a blank field must not wipe it.
    #[test]
    fn a_blank_pasted_private_key_still_keeps_the_saved_one() {
        let mut existing = Session::new_empty();
        existing.private_key_inline =
            Secret::new("-----BEGIN OPENSSH PRIVATE KEY-----".to_string());
        let mut draft = draft();
        draft.private_key_inline_mode = true;
        let session = session_from_draft(&draft, Some(&existing), Vec::new(), Vec::new());
        assert!(!session.private_key_inline.is_empty());
    }

    /// (#rdp-follow-lost 2026-10-13) The regression this guards: the dialog
    /// writes only `rdp_resolution`, so any save path that reads the raw
    /// `rdp_follow_tab_size` bool silently drops the dropdown choice.
    #[test]
    fn choosing_follow_in_the_dropdown_survives_the_save() {
        let mut draft = draft();
        draft.rdp_resolution = "follow".into();
        assert!(follows_tab_size(&draft));
        let session = session_from_draft(&draft, None, Vec::new(), Vec::new());
        assert!(session.rdp_follow_tab_size);
    }

    /// The carry-over bool still works for configs that predate the dropdown:
    /// the judgement is an OR, not a replacement.
    #[test]
    fn a_stored_follow_flag_still_counts_with_another_resolution() {
        let mut draft = draft();
        draft.rdp_follow_tab_size = true;
        draft.rdp_resolution = "1920x1080".into();
        assert!(follows_tab_size(&draft));
        let session = session_from_draft(&draft, None, Vec::new(), Vec::new());
        assert!(session.rdp_follow_tab_size);
    }

    /// ...and neither source claiming "follow" means no follow.
    #[test]
    fn neither_source_means_no_follow() {
        let mut draft = draft();
        draft.rdp_resolution = "1920x1080".into();
        assert!(!follows_tab_size(&draft));
        let session = session_from_draft(&draft, None, Vec::new(), Vec::new());
        assert!(!session.rdp_follow_tab_size);
    }

    /// (#rdp-follow-default 2026-10-13) "Follow tab" has no size of its own,
    /// but it must **not** become full screen: a session saved as follow and
    /// later switched to the system client would otherwise launch full screen.
    #[test]
    fn follow_tab_is_a_window_not_full_screen() {
        assert_eq!(rdp_display_settings("follow", 0, 0), (false, 1280, 720));
    }
}
