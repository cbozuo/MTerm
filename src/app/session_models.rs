use super::*;
// (#left-panel-v3 2026-11-06) 树形分组:组从组名字符串升级为一等实体
// (GroupRecord)。(#left-panel-v3-r2 2026-11-06 引擎提取) 拍平/匹配/排序/
// builtin 本地终端集整体迁往 crate::session_tree 三段式引擎(struct 零 Slint),
// 本文件降级为**宿主桥**:TreeRow → ui::SessionInfo 转换、模型下发、会话
// 对话框/跳板/代理/RDP 等桥层助手。
// (#tab-group-bar 2026-09-14) 标签底部色条要用与左侧会话行同一套组色语义。
// (#group-color 2026-09-14) 分组颜色(旧:组名表;新:record 有效色)。
use std::collections::HashSet;

// (#left-panel-v3-r2) 引擎层类型经桥层转呈 app.rs(glob 导入 `use
// self::session_models::*` 的既有调用点不变)。
pub(super) use crate::session_tree::{builtin_local_sessions, GroupSort};

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
        // (#group-default-zh 2026-11-10) 默认组的存储名是保留字 "default",
        // 下拉直出英文(用户截图)。下发展示名「默认组」/"Default",与左栏
        // 树、移动到菜单同一口径;选中后 value 即展示名,补全/色点/✓ 都按
        // 它匹配;提交链 upsert 会把保留名变体归一到 grp-default,不新建组。
        let name = if record.id == crate::config::GROUP_ID_DEFAULT {
            crate::i18n::t("默认组", "Default group")
        } else {
            record.name.as_str()
        };
        entries.push(GroupEntry {
            id: record.id.as_str().into(),
            name: name.into(),
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
/// (#move-group-dialog 2026-11-12) 「移动到分组」弹框的树形行模型(高保真
/// left-panel-session-hifi-2026-10-28-v3 §Move to Group):DFS 序 + depth
/// 缩进 + 组色圆点 + 组内会话数 + 「当前」徽章。`session_id` 用于标出
/// 该会话当前所在组(不可选)。保留组 system(本地终端)不出现在目标里。
pub(super) fn move_group_rows_model(
    store: &ConfigStore,
    session_id: &str,
    query: &str,
) -> ModelRc<MoveGroupRow> {
    let records = store.group_records().to_vec();
    let current_id = store
        .get(session_id)
        .map(|s| s.group_id.clone())
        .unwrap_or_default();
    // 组内直接会话数(与侧栏 group_total 的"含子树求和"不同 —— 弹框数字
    // 表达"这个组里有多少个会话",父子各自独立)。
    let mut counts: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    for s in store.sessions() {
        *counts.entry(s.group_id.clone()).or_insert(0) += 1;
    }
    let mut rows: Vec<MoveGroupRow> = Vec::new();
    let needle = query.trim().to_lowercase();
    fn walk(
        records: &[crate::config::GroupRecord],
        counts: &std::collections::HashMap<String, i32>,
        current_id: &str,
        needle: &str,
        parent: &str,
        depth: i32,
        rows: &mut Vec<MoveGroupRow>,
    ) {
        for record in records.iter().filter(|r| r.parent_id == parent) {
            // system(本地终端)保留组不可移入,不出现在目标列表。
            if record.id == crate::config::GROUP_ID_SYSTEM {
                continue;
            }
            // (#group-default-zh 2026-11-13) 默认组显示名统一「默认组」(此前
            // move 弹框漏改,直出存储名 "default";confirm 传回显示名由
            // resolve_group_target 归一,链路不受影响)。
            let display = if record.id == crate::config::GROUP_ID_DEFAULT {
                crate::i18n::t("默认组", "Default group").to_string()
            } else {
                record.name.clone()
            };
            // (#move-group-dialog) 搜索过滤在 Rust 侧(Slint string 无
            // contains);命中=组名不区分大小写包含。子组命中不受父组
            // 是否命中影响(DFS 继续下钻,父组行照常出树形骨架)。
            let hit = needle.is_empty()
                || display.to_lowercase().contains(needle)
                || needle.contains(&display.to_lowercase());
            if hit {
                let hex = record.color.clone().unwrap_or_default();
                let (color, has_color) = match parse_hex_color(&hex) {
                    Some(c) => (c, true),
                    None => (Default::default(), false),
                };
                rows.push(MoveGroupRow {
                    name: display.as_str().into(),
                    depth,
                    color,
                    has_color,
                    count: counts.get(&record.id).copied().unwrap_or(0),
                    current: record.id == current_id,
                });
            }
            walk(records, counts, current_id, needle, &record.id, depth + 1, rows);
        }
    }
    walk(&records, &counts, &current_id, &needle, "", 0, &mut rows);
    ModelRc::from(Rc::new(VecModel::from(rows)))
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

// ---------------------------------------------------------------------------
// (引擎迁移) normalized_query / session_matches_* / last_used_rank 已迁
// crate::session_tree::row —— 本文件不再重复定义。
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// (引擎迁移 #left-panel-v3-r2 2026-11-06) FlatCtx 拍平上下文 / sort_members /
// build_session_rows 整体迁往 crate::session_tree::flatten(引擎层,零 Slint)。
// 本文件只保留 TreeRow → ui::SessionInfo 的转换与模型下发(见下 sync_*)。
// ---------------------------------------------------------------------------

/// 引擎行 → Slint 行的逐字段转换(桥层唯一职责)。`slint::Color` 在这里解析
/// (引擎只持 hex 原文);conn_state 恒 0,由 refresh_session_markers_win 回写。
fn to_session_info(row: crate::session_tree::TreeRow) -> SessionInfo {
    let hex = row.color_hex.clone();
    SessionInfo {
        row_kind: row.row_kind_str().into(),
        id: row.id.into(),
        name: row.name.into(),
        kind: row.kind.into(),
        host: row.host.into(),
        port: row.port,
        user: row.user.into(),
        auth: row.auth.into(),
        group_id: row.group_id.into(),
        group: row.group.into(),
        depth: row.depth,
        collapsed: row.collapsed,
        builtin: row.builtin,
        conn_state: 0,
        group_index: row.group_index,
        group_size: row.group_size,
        group_total: row.group_total,
        group_subs: row.group_subs,
        note: row.note.into(),
        group_color: parse_hex_color(&hex).unwrap_or_default(),
        group_color_hex: hex.into(),
    }
}

pub(super) fn sync_sessions_to_model_with_filter(
    store: &ConfigStore,
    model: &VecModel<SessionInfo>,
    query: &str,
) {
    // (#panel-settings 2026-11-12) 排序档位读配置(0 存储序 / 1 名称 / 2 最近)
    // —— 面板设置「排序方式」的单一来源,所有模型刷新路径统一走它。
    sync_sessions_to_model_with_sort(store, model, query, sort_from_store(store));
}

/// (#panel-settings) 落盘档位 → 引擎排序枚举(越界回落存储序)。
pub(super) fn sort_from_store(store: &ConfigStore) -> GroupSort {
    match store.panel_sort_mode() {
        1 => GroupSort::Name,
        2 => GroupSort::Recent,
        _ => GroupSort::Storage,
    }
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
    // (#left-panel-v3-r2) 引擎拍平 → 桥层转换 → 模型下发。
    let rows = crate::session_tree::build_tree_rows(
        store,
        &builtin_sessions,
        query,
        store.system_group_hidden(),
        sort,
    )
    .into_iter()
    .map(to_session_info)
    .collect::<Vec<_>>();
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
    let rows = crate::session_tree::build_tree_rows(
        store,
        &builtin_sessions,
        query,
        store.system_group_hidden(),
        sort_from_store(store),
    )
    .into_iter()
    .map(to_session_info)
    .collect::<Vec<_>>();
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

// (引擎迁移) builtin_local_sessions / builtin_local_session / wsl_available
// 已迁 crate::session_tree::flatten;本文件经上方 `pub(super) use` 转呈
// app.rs 的 glob 导入,既有调用点(bare 调用)不变。

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
/// The resolution a new RDP session starts with.
///
/// (#rdp-follow-size 2026-11-13) "follow" — the remote desktop takes the
/// tab's size. A built-in session lives *inside* a tab, so a fixed desktop
/// there is scaled to fit: it looks soft when the tab is larger, and it
/// costs a full-frame resample on every repaint. The stored width/height
/// stay as the placeholder for the system client, which has a window of its
/// own.
pub(super) const RDP_RESOLUTION_DEFAULT: &str = "follow";
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

// (引擎迁移) search_tests / drag_order_tests / row_display_tests 三个 V2
// 拍平测试模块已随引擎迁走并以 ConfigStore 重写为
// session_tree::{flatten,locate} 的测试(旧版基于 group_header/组名字符串
// 的用例在 V3 数据模型下无意义,保留只会让 cargo test 编译失败)。
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
            rdp_resolution: RDP_RESOLUTION_DEFAULT.into(),
            rdp_width: 1280,
            rdp_height: 720,
            rdp_mode: "embedded".into(),
            rdp_follow_tab_size: true,
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
    ///
    /// (#rdp-follow-size 2026-11-13) The draft's *default* now follows the
    /// tab, so the flag is cleared explicitly here — the point of the test is
    /// that a fixed resolution turns the follow off.
    #[test]
    fn neither_source_means_no_follow() {
        let mut draft = draft();
        draft.rdp_follow_tab_size = false;
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
