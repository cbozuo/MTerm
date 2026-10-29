//! (#find-bar 2026-10-27) 终端查找栏的数据类型（高保真稿：
//! `docs/design/find-bar-hifi-2026-10-23.html` v3.6）。
//!
//! 归 `struct/`：纯数据定义，不含行为；匹配/扫描/步进在
//! `impls/terminal_find.rs`（挂在 TermBuffer 上）。

/// 查找匹配规则（与 UI 的 Aa / (.·)* 两个开关一一对应）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FindOptions {
    /// true = Aa 区分大小写；false = 忽略大小写（默认）。
    pub(crate) match_case: bool,
    /// true = 按正则匹配；false = 纯文本包含。
    pub(crate) use_regex: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self { match_case: false, use_regex: false }
    }
}

/// 一次全缓冲扫描的产物。
///
/// `hits` 的行号是**绝对行号**（0 = history 最老一行，history.len() 起是
/// live 屏）——v3.6 §04「新输出时当前匹配不动」的锚：历史增长只是把同一
/// 绝对行号往数组深处推，命中本身不漂移。col/len 是**网格列**（CJK 占两格，
/// #132 口径），画矩形/对齐都直接可用。
#[derive(Clone, Debug, Default)]
pub(crate) struct FindHits {
    /// (abs_row, col, len)，行序优先、行内按列升序。
    pub(crate) hits: Vec<(usize, i32, i32)>,
    /// 正则编译失败：true 时 hits 无意义，调用方**保旧结果**、只点亮
    /// 计数位的红字「正则无效」（v3.6 §06⑤）。
    pub(crate) invalid_regex: bool,
}

impl FindHits {
    pub(crate) fn total(&self) -> usize {
        self.hits.len()
    }
}

/// 查找栏在单个终端缓冲上的全部状态。
///
/// 挂在 `TermBuffer` 上（替代原来的裸 `find_query: String`）：一个终端
/// 一份，切 tab 天然隔离（v3.6 §06⑩）。UI 侧的 find-active / 面板几何
/// 仍是 Slint 局部状态，不进这里。
#[derive(Clone, Debug, Default)]
pub(crate) struct FindState {
    /// 当前查询词（空串 = 无查找）。
    pub(crate) query: String,
    /// 匹配规则（Aa / (.·)*）。
    pub(crate) options: FindOptions,
    /// 最近一次**成功**的全缓冲扫描结果。正则写错时原样保留（§06⑤）。
    pub(crate) hits: FindHits,
    /// 正则编译失败标志（§06⑤）：与 hits 解耦——写错的瞬间旧命中还在
    /// 屏上，只是计数位转红「正则无效」。修正后自动清除。
    pub(crate) invalid_regex: bool,
    /// 当前匹配在 `hits.hits` 里的下标；无命中/未初始化 = None。
    pub(crate) current: Option<usize>,
    /// 上次全量扫描完成的时间。配合 `TermBuffer::last_output_at` 判定
    /// 「输出停顿后的静默重算」（v3.6 §04 新输出行）：扫描之后来过新输出、
    /// 且输出已停 ≥200ms 的下一次查询提交才重扫——刷屏期间的每次提交都在
    /// 幂等分支直接返回。
    pub(crate) scanned_at: Option<std::time::Instant>,
}
