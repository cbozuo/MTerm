//! (#double-shift-v1 2026-11-11) 全局搜索的**数据类型**。
//!
//! 高保真稿：`docs/design/double-shift-hifi-2026-11-06-v1.html`。
//! 纯数据 + 只依赖自身字段的小逻辑；不依赖 Slint、不依赖 app 层
//! （分层说明见 `src/search/mod.rs`，与 `src/command_hint/` 同构）。
//!
//! 五类数据（会话 / 命令 = 快捷命令+历史 / 设置 / 动作）在打开面板时摊平成
//! 同一种 `SearchItem`，预计算小写检索串——查询路径上没有 I/O。

use crate::i18n::t;

/// 结果分类。组序（「全部」视图）= 会话 → 命令 → 设置 → 动作。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Category {
    Session,
    Command,
    Setting,
    Action,
}

impl Category {
    /// 分类条 / 组头文案。
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Session => t("会话", "Sessions"),
            Self::Command => t("命令", "Commands"),
            Self::Setting => t("设置", "Settings"),
            Self::Action => t("动作", "Actions"),
        }
    }

    /// 行首图标（`ui/icons.slint` 既有 glyph，不新增 svg）。
    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::Session => "terminal",
            Self::Command => "quick-command",
            Self::Setting => "settings",
            Self::Action => "bolt",
        }
    }
}

/// 执行器需要的载荷。`None` = 纯展示（理论上不出现，兜底用）。
#[derive(Clone, Debug)]
pub(crate) enum Payload {
    None,
    /// 会话：`session_id` 供连接/切换；`ssh_cmd` 供 Ctrl+Enter 复制。
    Session { session_id: String, ssh_cmd: String },
    /// 命令文本（Enter 发送到当前会话 / Ctrl+Enter 填入命令栏）。
    Command { cmd: String, from_history: bool },
    /// 设置项：`page` = settings 弹框 `ifd-page` 页名。
    Setting { page: &'static str, key: &'static str },
    /// 动作：`act` = 执行器分派键；`shortcut` = 展示快捷键（空 = 无）。
    Action { act: &'static str, shortcut: &'static str },
}

/// 一条可搜索的本地数据。索引构建时把所有检索串预转小写。
#[derive(Clone, Debug)]
pub(crate) struct SearchItem {
    /// 稳定 id：`session:<sid>` / `cmd:<i>` / `hist:<i>` / `set:<key>` / `act:<key>`。
    /// 最近使用（空查询视图）按它回查。
    pub(crate) id: String,
    pub(crate) cat: Category,
    pub(crate) title: String,
    /// 行副标题（会话 = user@host:port · 分组；命令 = 命令文本；设置 = 路径）。
    pub(crate) sub: String,
    /// 预览区描述。
    pub(crate) desc: String,
    /// 会话已打开（已打开标签排前并标「已打开」）。
    pub(crate) open: bool,
    /// 最近使用排名（1 = 最新；0 = 不在名单）。评分加成 `max(0, 12-rank)`。
    pub(crate) recent_rank: i32,
    pub(crate) payload: Payload,
    // —— 预计算检索串（小写）——
    pub(crate) hay_title: String,
    pub(crate) hay_sub: String,
    /// 关键词全串：标题/别名/拼音/分类词。
    pub(crate) hay_kw: String,
    /// `hay_title + hay_sub + hay_kw` 的串联（token AND 的门面判断）。
    pub(crate) hay_all: String,
}

impl SearchItem {
    /// 统一构造：收一份关键词串，内部完成小写化与串联。
    pub(crate) fn new(
        id: String,
        cat: Category,
        title: String,
        sub: String,
        desc: String,
        keywords: String,
        open: bool,
        recent_rank: i32,
        payload: Payload,
    ) -> Self {
        let hay_title = title.to_lowercase();
        let hay_sub = sub.to_lowercase();
        let hay_kw = keywords.to_lowercase();
        let hay_all = format!("{hay_title} {hay_sub} {hay_kw}");
        Self {
            id,
            cat,
            title,
            sub,
            desc,
            open,
            recent_rank,
            payload,
            hay_title,
            hay_sub,
            hay_kw,
            hay_all,
        }
    }
}

/// 分类计数（分类条徽章 + 脚注「索引 N 项」）。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Counts {
    pub(crate) all: usize,
    pub(crate) session: usize,
    pub(crate) command: usize,
    pub(crate) setting: usize,
    pub(crate) action: usize,
}

/// 打开面板时建好的内存索引。
#[derive(Clone, Debug, Default)]
pub(crate) struct SearchIndex {
    pub(crate) items: Vec<SearchItem>,
    pub(crate) counts: Counts,
}

/// 一条命中（查询产物，评分与条目配对）。
pub(crate) struct MatchRow<'a> {
    pub(crate) item: &'a SearchItem,
    pub(crate) score: i32,
}

/// 「全部」视图的一个分类分组（组头 + 该组截取后的条目）。
pub(crate) struct GroupedMatches<'a> {
    pub(crate) cat: Category,
    pub(crate) items: Vec<MatchRow<'a>>,
}
