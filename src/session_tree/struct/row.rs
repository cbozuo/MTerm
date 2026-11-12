//! (#left-panel-v3 2026-11-06) 会话树的**领域类型与行几何**（零 Slint 依赖）。
//!
//! `TreeRow` 是拍平后的「行即节点」：组头 / 会话 / 空组占位三类。桥层把它
//! 逐字段转成 `ui::SessionInfo`（Slint 生成类型）；组色在这里只保留 hex 原文，
//! `slint::Color` 的解析在桥层（`session_models::parse_hex_color`）。

use crate::config::Session;

/// 行类型。`as_str()` 的三个值与 Slint 侧 `SessionInfo.row-kind` 逐字对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// 组头行：name = 组名，id 为空。
    Group,
    /// 会话行：name = 会话名。
    Session,
    /// 空组占位行（组头下「空分组」小字），id 为空。
    Empty,
}

impl RowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RowKind::Group => "group",
            RowKind::Session => "session",
            RowKind::Empty => "empty",
        }
    }
}

/// 拍平后的单行。字段语义与 `ui::SessionInfo` 一一对应（见 left_sidebar.slint
/// struct 注释），除 `color_hex`（桥层再解析成 `slint::Color`）与 `row_kind`
/// （这里用枚举，桥层转字符串）。
#[derive(Debug, Clone)]
pub struct TreeRow {
    pub row_kind: RowKind,
    pub id: String,
    pub name: String,
    /// 协议："ssh" | "serial" | "telnet" | "local" | "rdp"。组头/占位行 = ""。
    pub kind: String,
    pub host: String,
    pub port: i32,
    pub user: String,
    /// 认证方式原始值（password / keyboard-interactive / key），悬停卡用。
    pub auth: String,
    /// 稳定组 id：组头行 = 自身组 id；成员/占位行 = 所属组 id。
    pub group_id: String,
    /// 显示组名（组头行与成员行同值）。
    pub group: String,
    /// 树深度：组头行 = 组自身深度（根级 0）；成员/占位行 = 所属组深度 + 1。
    pub depth: i32,
    /// 组头行的折叠态（成员行恒 false）。折叠时子层行不进模型。
    pub collapsed: bool,
    /// 内置本地终端（system 组成员）。
    pub builtin: bool,
    /// 组内序（0..n，桥层回写 conn_state 前的原值恒 0）。
    pub group_index: i32,
    /// 直接成员数（过滤后）。
    pub group_size: i32,
    /// 组头行统计：含子树的会话总数（右对齐数字）。
    pub group_total: i32,
    /// 组头行统计：直接子组数（组名后「N组」）。
    pub group_subs: i32,
    /// 会话备注（悬停卡「备注」行）。
    pub note: String,
    /// 有效色 hex 原文（"" = 无色 → 桥层回落次级前景色）。
    pub color_hex: String,
}

impl TreeRow {
    pub fn row_kind_str(&self) -> &'static str {
        self.row_kind.as_str()
    }
}

/// (#left-panel-v3) 面板排序方式（面板设置「排序方式」三档）。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupSort {
    /// 存储序：组 = record Vec 顺序（用户拖动维护）；会话 = sessions Vec 内
    /// 同组相对顺序。默认。
    #[default]
    Storage,
    /// 名称：组与会话各自按名称排序（小写比较；无 locale 依赖）。
    Name,
    /// 最近：按 last_used 降序（ISO 字符串字典序即时间序），空值垫底。
    Recent,
}

// ---------------------------------------------------------------------------
// (#tree-geom 2026-11-06) 行几何 —— Slint 侧 ui/left_sidebar.slint 的行布局
// 唯一 Rust 真源。取值 = 高保真 V4.x 终版覆盖（left-panel-session-hifi
// §V4.1：父组 30 / 子组 27 / 深层 26，会话行 34；组间 6、成员间 2）。
// 拖拽 pitch = 会话行高 + MEMBER_GAP = 36（Slint 侧 drag-pitch 同值互指）。
// ---------------------------------------------------------------------------

/// 根级组头行高。
pub const ROW_GROUP_D0: f32 = 30.0;
/// 一级子组头行高。
pub const ROW_GROUP_D1: f32 = 27.0;
/// 二级及更深组头行高。
pub const ROW_GROUP_D2: f32 = 26.0;
/// 会话行高（单行化：主机/用户只在悬停卡）。
pub const ROW_HEIGHT_SESSION: f32 = 34.0;
/// 会话行高·紧凑档（#panel-settings 2026-11-12 面板设置「行密度」；Slint 侧
/// SessionRow 高度与 drag-pitch 同步读 `panel_row_compact` 配置）。
pub const ROW_HEIGHT_SESSION_COMPACT: f32 = 30.0;
/// 空组占位行高。
pub const ROW_HEIGHT_EMPTY: f32 = 22.0;
/// 组头行前留白（组间距；首行无）。
pub const GROUP_GAP: f32 = 6.0;
/// 会话/占位行前留白（成员间距）。
pub const MEMBER_GAP: f32 = 2.0;

/// 拍平布局的行度量：`(padding-top, 行高)`。定位滚动坐标（`locate_center_y`）
/// 与 Slint 侧行 wrapper 的 padding-top/height 用同一张表；任何一侧改行高或
/// 间距，另一侧必须同步（两侧注释 #tree-geom 互指）。
pub fn row_metrics(row_kind: &str, depth: i32) -> (f32, f32) {
    match row_kind {
        "group" => (
            GROUP_GAP,
            if depth <= 0 {
                ROW_GROUP_D0
            } else if depth == 1 {
                ROW_GROUP_D1
            } else {
                ROW_GROUP_D2
            },
        ),
        "session" => (MEMBER_GAP, ROW_HEIGHT_SESSION),
        _ => (MEMBER_GAP, ROW_HEIGHT_EMPTY),
    }
}

/// 搜索查询归一：去首尾空白 + 小写。
pub fn normalized_query(query: &str) -> String {
    query.trim().to_lowercase()
}

/// (V3) 会话匹配口径（设计稿 T2）：名称 / 主机 / 用户 / 备注 / 协议 / 认证方式。
/// 组名的命中（整组带出）由 flatten 的祖先链回溯承担。
pub fn session_matches_normalized_query(session: &Session, query: &str) -> bool {
    query.is_empty()
        || session.name.to_lowercase().contains(query)
        || session.host.to_lowercase().contains(query)
        || session.user.to_lowercase().contains(query)
        || session.note.to_lowercase().contains(query)
        || session.kind.as_str().to_lowercase().contains(query)
}

/// (V3) last_used 排序键：ISO 字符串字典序即时间序；空值恒垫底。
pub fn last_used_rank(s: &Session) -> (bool, String) {
    match &s.last_used {
        Some(v) if !v.trim().is_empty() => (true, v.clone()),
        _ => (false, String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SessionKind;

    fn session(name: &str, host: &str) -> Session {
        let mut value = Session::new_empty();
        value.name = name.into();
        value.host = host.into();
        value
    }

    #[test]
    fn metrics_follow_the_hifi_row_heights() {
        // (#tree-geom) 三档组头 + 会话 + 占位，与 Slint 侧逐值对齐。
        assert_eq!(row_metrics("group", 0), (GROUP_GAP, 30.0));
        assert_eq!(row_metrics("group", 1), (GROUP_GAP, 27.0));
        assert_eq!(row_metrics("group", 3), (GROUP_GAP, 26.0));
        assert_eq!(row_metrics("session", 0), (MEMBER_GAP, 34.0));
        assert_eq!(row_metrics("empty", 1), (MEMBER_GAP, 22.0));
    }

    #[test]
    fn query_matching_is_trimmed_and_case_insensitive() {
        let value = session("Prod API", "DB.EXAMPLE.COM");
        let q = normalized_query("  Prod  ");
        assert!(session_matches_normalized_query(&value, &q));
        assert!(session_matches_normalized_query(&value, &normalized_query("example.com")));
        assert!(!session_matches_normalized_query(&value, &normalized_query("staging")));
    }

    #[test]
    fn row_kind_strings_match_the_slint_contract() {
        assert_eq!(RowKind::Group.as_str(), "group");
        assert_eq!(RowKind::Session.as_str(), "session");
        assert_eq!(RowKind::Empty.as_str(), "empty");
    }

    #[test]
    fn protocol_kind_drives_the_row_icon_source() {
        for (kind, expected) in [
            (SessionKind::Ssh, "ssh"),
            (SessionKind::Serial, "serial"),
            (SessionKind::Telnet, "telnet"),
            (SessionKind::Local, "local"),
        ] {
            let mut value = Session::new_empty();
            value.kind = kind;
            assert_eq!(value.kind.as_str(), expected);
        }
    }
}
