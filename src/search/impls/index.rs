//! (#double-shift-v1 2026-11-11) 索引构建：五类数据 → 扁平 `SearchItem`。
//!
//! 打开面板时重建（数据量几百条，毫秒级；保证会话/命令/历史始终新鲜）。
//! 会话含系统内置本地终端（`system:` 前缀）与「已打开」判定；命令合并
//! 快捷命令与命令历史；设置/动作来自静态清单（`struct/catalog.rs`）。

use std::collections::{HashMap, HashSet};

use crate::config::{display_group_of, ConfigStore, Session, SessionKind};
use super::catalog::{action_items, setting_items};
use crate::search::{Counts, Payload, SearchIndex, SearchItem};

/// 会话关键词的通用尾串（中英 + 拼音首字母，样表口径见高保真 DATA 段）。
const SESSION_WORDS: &str = "session sessions ssh 会话 连接 huihua hh 机器 主机 host";
const COMMAND_WORDS: &str = "command commands 快捷命令 命令 mingling ml";

/// 最近历史参与索引的条数上限（与索引体积预算对齐；完整历史走历史面板）。
const HISTORY_INDEX_CAP: usize = 200;

/// 「已打开」判定 + 会话 → 最近使用排名的输入视图。
pub(crate) struct SessionView<'a> {
    /// 已打开（任一 tab 连接/连接中）的 session_id 集合。
    pub(crate) open_ids: &'a HashSet<String>,
    /// 最近使用排名：session_id → 1 = 最新。
    pub(crate) recent_rank: &'a HashMap<String, i32>,
}

/// 组名链：group_id 沿 parent_id 上溯，`父 › 子` 拼接；未分组 → "default"。
fn group_path(store: &ConfigStore, session: &Session) -> String {
    let mut names: Vec<String> = Vec::new();
    let mut cur = if session.group_id.is_empty() {
        String::new()
    } else {
        session.group_id.clone()
    };
    for _ in 0..8 {
        if cur.is_empty() {
            break;
        }
        match store.group_records().iter().find(|r| r.id == cur) {
            Some(r) => {
                names.push(r.name.clone());
                cur = r.parent_id.clone();
            }
            None => break,
        }
    }
    if names.is_empty() {
        return display_group_of(session);
    }
    names.reverse();
    names.join(" › ")
}

/// 单条会话 → SearchItem。
fn session_item(store: &ConfigStore, session: &Session, view: &SessionView<'_>) -> SearchItem {
    let id = session.id.clone();
    let open = view.open_ids.contains(&id) || view.open_ids.contains(session.id.as_str());
    let port_part = if session.port > 0 && session.port != 22 {
        format!(":{}", session.port)
    } else {
        String::new()
    };
    let auth_part = match session.auth {
        crate::config::AuthMethod::Key => crate::i18n::t("密钥", "Key").to_string(),
        crate::config::AuthMethod::KeyboardInteractive => {
            crate::i18n::t("交互认证", "Interactive").to_string()
        }
        crate::config::AuthMethod::Password => crate::i18n::t("密码", "Password").to_string(),
    };
    let group = if id.starts_with("system:") {
        crate::i18n::t("本地终端", "Local shell").to_string()
    } else {
        group_path(store, session)
    };
    let sub = format!(
        "{}@{}{} · {} · {}",
        session.user,
        session.host,
        port_part,
        group,
        auth_part
    );
    let desc = match session.kind {
        SessionKind::Ssh => format!("SSH · {}", session.host),
        SessionKind::Serial => format!("{} @{}", session.serial_port, session.baud_rate),
        SessionKind::Telnet => format!("telnet {}:{}", session.host, session.port),
        SessionKind::Local => crate::i18n::t("本地 shell", "Local shell").to_string(),
        SessionKind::Rdp => format!("RDP · {}", session.host),
        SessionKind::Vnc => format!("VNC · {}", session.host),
    };
    let ssh_cmd = if session.kind == SessionKind::Ssh && !id.starts_with("system:") {
        if session.port > 0 && session.port != 22 {
            format!("ssh {}@{} -p {}", session.user, session.host, session.port)
        } else {
            format!("ssh {}@{}", session.user, session.host)
        }
    } else {
        String::new()
    };
    let note = session.note.clone();
    let keywords = format!(
        "{} {} {} {} {} {} {}",
        session.name, session.host, session.user, group, note, SESSION_WORDS,
        session.kind.as_str()
    );
    let recent_rank = view.recent_rank.get(&id).copied().unwrap_or(0);
    SearchItem::new(
        format!("session:{id}"),
        crate::search::Category::Session,
        session.name.clone(),
        sub,
        desc,
        keywords,
        open,
        recent_rank,
        Payload::Session { session_id: id, ssh_cmd },
    )
}

/// 索引构建入口。
pub(crate) fn build_index(store: &ConfigStore, view: &SessionView<'_>) -> SearchIndex {
    let mut items: Vec<SearchItem> = Vec::new();

    // ── 会话：已保存 + 系统内置本地终端 ──────────────────────────────
    let builtin = crate::session_tree::builtin_local_sessions(store.wsl_profiles());
    let mut session_count = 0usize;
    for s in store.sessions() {
        items.push(session_item(store, s, view));
        session_count += 1;
    }
    for s in &builtin {
        items.push(session_item(store, s, view));
        session_count += 1;
    }

    // ── 命令：快捷命令 + 命令历史（存储序最旧在前 → 倒序取最新）──────
    let mut command_count = 0usize;
    for (i, qc) in store.quick_commands().iter().enumerate() {
        let group = if qc.group.trim().is_empty() {
            crate::i18n::t("默认分组", "Default").to_string()
        } else {
            qc.group.clone()
        };
        let note = qc.note.clone();
        let keywords = format!(
            "{} {} {} {} {} {}",
            qc.name, qc.command, group, note, COMMAND_WORDS, "kuaijie kjml"
        );
        items.push(SearchItem::new(
            format!("cmd:{i}"),
            crate::search::Category::Command,
            qc.name.clone(),
            qc.command.clone(),
            note,
            keywords,
            false,
            0,
            Payload::Command { cmd: qc.command.clone(), from_history: false },
        ));
        command_count += 1;
    }
    let hist: Vec<_> = store.history_entries().iter().rev().take(HISTORY_INDEX_CAP).collect();
    for (i, h) in hist.iter().enumerate() {
        // 历史条目的 rank：位置越靠尾（越新）排名越靠前，给查询轻加成。
        let rank = (i as i32) + 1;
        items.push(SearchItem::new(
            format!("hist:{}", hist.len() - 1 - i),
            crate::search::Category::Command,
            h.cmd.clone(),
            crate::i18n::t("命令历史", "History").to_string(),
            String::new(),
            format!("{} {} {}", h.cmd, COMMAND_WORDS, "lishi ls"),
            false,
            rank.min(12),
            Payload::Command { cmd: h.cmd.clone(), from_history: true },
        ));
        command_count += 1;
    }

    // ── 设置 / 动作：静态清单 ────────────────────────────────────────
    let setting_items = setting_items();
    let action_items = action_items();
    let setting_count = setting_items.len();
    let action_count = action_items.len();
    items.extend(setting_items);
    items.extend(action_items);

    SearchIndex {
        counts: Counts {
            all: items.len(),
            session: session_count,
            command: command_count,
            setting: setting_count,
            action: action_count,
        },
        items,
    }
}
