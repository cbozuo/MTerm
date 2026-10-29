//! (#find-bar 2026-10-27) 终端查找栏的行为侧（高保真稿：
//! `docs/design/find-bar-hifi-2026-10-23.html` v3.6）。
//!
//! 全部实现挂在 `TermBuffer` 上：匹配消费的是 history + live + `cell_prefix`，
//! 本来就是 terminal 域的东西；app 侧只做「回调 → 委托这里 → 回传 UI」。
//!
//! v3.6 硬规则与落点：
//!   ① 最近命中起点、逐键不重跳      → `find_set_query` / `find_reanchor_if_needed`
//!   ② 绝对行号锚定                  → `FindHits.hits` 的 abs_row
//!   ③ 循环查找                      → `find_step` 的取模
//!   ④ 正则写错保旧结果              → `find_set_query` 的 invalid 分支
//!   ⑤ 计数限 alt 屏                 → `find_step` 的 is_alt 早退
//!   ⑥ 跳转居中                      → `find_center_scroll_target`

use crate::terminal::{build_row, cell_prefix, FindHits, FindOptions, FindState, TermBuffer};
use crate::ui::TermMatch;

/// 编译查询为匹配器。正则失败返回 Err（调用方保旧结果）。
enum Matcher {
    /// 纯文本：折叠/原样的 char 序列。
    Plain { q: Vec<char>, folded: bool },
    Regex(regex::Regex),
}

fn build_matcher(query: &str, opts: FindOptions) -> Result<Matcher, regex::Error> {
    if opts.use_regex {
        let mut builder = regex::RegexBuilder::new(query);
        builder.case_insensitive(!opts.match_case);
        builder.size_limit(1 << 22); // 4MB 编译上限：防病态模式打爆内存
        return Ok(Matcher::Regex(builder.build()?));
    }
    let q: Vec<char> = if opts.match_case {
        query.chars().collect()
    } else {
        query.chars().map(|c| c.to_ascii_lowercase()).collect()
    };
    Ok(Matcher::Plain { folded: !opts.match_case, q })
}

/// 单行匹配 → 行内命中的 (网格列, 网格长) 列表。
///
/// 列位映射复用 `cell_prefix`（CJK 每字两格，#132），与选区/高亮同一坐标。
fn match_line(line: &str, m: &Matcher) -> Vec<(i32, i32)> {
    if line.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = line.chars().collect();
    let prefix = cell_prefix(&chars);
    let mut out: Vec<(i32, i32)> = Vec::new();
    match m {
        Matcher::Plain { q, folded } => {
            if q.is_empty() {
                return out;
            }
            // hay：Aa 开 → 原字符；Aa 关 → 折叠（q 侧已同步折叠）。
            let hay: Vec<char> = if *folded {
                chars.iter().map(|c| c.to_ascii_lowercase()).collect()
            } else {
                chars.clone()
            };
            let n = q.len();
            let mut i = 0usize;
            while i + n <= hay.len() {
                if hay[i..i + n] == q[..] {
                    let col = prefix[i] as i32;
                    let len = (prefix[i + n] - prefix[i]) as i32;
                    out.push((col, len));
                    i += n; // 不允许重叠命中（与现网口径一致）
                } else {
                    i += 1;
                }
            }
        }
        Matcher::Regex(re) => {
            // regex 工作在 char 边界（&str），列位用 prefix 映射。
            // captures_at + 空匹配前跳，防 `a*` 一类死循环。
            let mut at = 0usize;
            while at <= line.len() {
                match re.captures_at(line, at) {
                    Some(cap) => match cap.get(0) {
                        Some(hit) if !hit.is_empty() => {
                            let cs = line[..hit.start()].chars().count();
                            let ce = line[..hit.end()].chars().count();
                            if ce <= chars.len() && ce > cs {
                                let col = prefix[cs] as i32;
                                let len = (prefix[ce] - prefix[cs]) as i32;
                                out.push((col, len));
                            }
                            at = hit.end();
                        }
                        Some(_) => {
                            // 空匹配：前进一个 char。
                            match line[at..].chars().next() {
                                Some(c) => at += c.len_utf8(),
                                None => break,
                            }
                        }
                        None => break,
                    },
                    None => break,
                }
            }
        }
    }
    out
}

impl TermBuffer {
    /// live 屏的行文本（含尾随空行——与 render() 的滚动窗口同口径，
    /// 保证 live 视图与滚动视图的绝对行号连续（#119-followup））。
    fn find_live_rows(&self) -> Vec<String> {
        let s = self.parser.screen();
        let (rows, cols) = s.size();
        (0..rows)
            .map(|r| build_row(s, r, cols).0.trim_end().to_string())
            .collect()
    }

    /// 全缓冲扫描（v3.6 §05「全缓冲匹配」新增项）：
    /// history（老→新）+ live 屏一次遍历。视口外命中同样入表——
    /// 计数和 next/prev 都要全量，画矩形时由调用方按视口过滤。
    pub(crate) fn find_scan_all(&self, query: &str, opts: FindOptions) -> FindHits {
        let mut out = FindHits::default();
        if query.is_empty() {
            return out;
        }
        let matcher = match build_matcher(query, opts) {
            Ok(m) => m,
            Err(_) => {
                out.invalid_regex = true;
                return out;
            }
        };
        for (abs, line) in self.history.iter().enumerate() {
            for (col, len) in match_line(&line.0, &matcher) {
                out.hits.push((abs, col, len));
            }
        }
        let live = self.find_live_rows();
        let hist_len = self.history.len();
        for (r, line) in live.iter().enumerate() {
            for (col, len) in match_line(line, &matcher) {
                out.hits.push((hist_len + r, col, len));
            }
        }
        out
    }

    /// 视口内命中的高亮矩形（TermMatch 的 row 是**可视行** 0..rows）。
    /// 由 `rebuild_tab_display` 在每次渲染时调用——视口变了矩形跟着变，
    /// 但 current 指针（绝对序）不动，主题色始终落在同一条命中上。
    /// (§06⑤) invalid_regex 时**不清**：hits 还是上一次有效扫描的结果，
    /// 旧高亮保留，仅计数位转红「正则无效」。
    pub(crate) fn find_visible_rects(&self, st: &FindState) -> Vec<TermMatch> {
        if st.query.is_empty() {
            return Vec::new();
        }
        let rows = self.parser.screen().size().0 as usize;
        let (_, live_used) = self.live_rows();
        let top = self.view_top_abs(live_used);
        let mut out = Vec::new();
        for &(abs, col, len) in &st.hits.hits {
            if abs < top {
                continue;
            }
            let vis = abs - top;
            if vis >= rows {
                break; // hits 按行升序，后面只会更远
            }
            out.push(TermMatch { row: vis as i32, col, len });
        }
        out
    }

    /// 当前匹配的可视矩形（换主题色那一条）；不在视口 → None。
    pub(crate) fn find_current_rect(&self, st: &FindState) -> Option<TermMatch> {
        let idx = st.current?;
        let &(abs, col, len) = st.hits.hits.get(idx)?;
        let rows = self.parser.screen().size().0 as usize;
        let (_, live_used) = self.live_rows();
        let top = self.view_top_abs(live_used);
        if abs < top {
            return None;
        }
        let vis = abs - top;
        if vis >= rows {
            return None;
        }
        Some(TermMatch { row: vis as i32, col, len })
    }

    /// v3.6 §03①：离当前视口最近的命中下标。
    /// 视口内有 → 视口内第一个；没有 → 视口之后最近的；再没有 → 视口之前最后一个。
    fn find_nearest_to_view(&self, hits: &FindHits) -> Option<usize> {
        if hits.hits.is_empty() {
            return None;
        }
        let (_, live_used) = self.live_rows();
        let top = self.view_top_abs(live_used);
        let rows = self.parser.screen().size().0 as usize;
        let view = top..top + rows;
        // 视口内第一个
        if let Some(i) = hits.hits.iter().position(|&(abs, _, _)| view.contains(&abs)) {
            return Some(i);
        }
        // 视口之后最近
        if let Some(i) = hits.hits.iter().position(|&(abs, _, _)| abs >= view.end) {
            return Some(i);
        }
        // 视口之前最后一个
        Some(hits.hits.len() - 1)
    }

    /// 设置查询词并全量重扫（输入去抖后调用）。
    ///
    /// 返回 false = 正则无效：**状态保旧**（旧 hits/旧 current 不动），
    /// 调用方把 invalid 标志点亮即可（§06⑤）。
    ///
    /// 起始当前匹配 = 离视口最近命中（§03①），**逐键不重跳**由
    /// `find_reanchor_if_needed` 保证：这里只在「新词」时定锚。
    pub(crate) fn find_set_query(&mut self, st_query: &str, opts: FindOptions) -> bool {
        if st_query == self.find.query && st_opts_eq(&self.find.options, &opts) {
            // 同词同规则：逐键去抖的幂等底座。唯一的例外是「输出停顿后的
            // 静默重算」（§04 新输出行）——扫描之后来过新输出、且输出已停
            // ≥200ms 的这次提交才真正重扫；刷屏期间的每次提交都在这里返回。
            // 不比 history.len()：回滚区到 MAX_HISTORY 饱和后长度不再变化，
            // 时间戳比较才能覆盖「新行进、旧行出」的 tail -f 场景。
            let new_output = self
                .find
                .scanned_at
                .map_or(false, |t| self.last_output_at > t);
            let quiet =
                self.last_output_at.elapsed() >= std::time::Duration::from_millis(200);
            if !new_output || !quiet {
                return true;
            }
            // 落入下方重扫；fresh_query = false → current 按「原位置附近最近
            // 命中」重锚（§06⑥ 同源），输出流不会把当前匹配拽走。
        }
        let fresh_query = st_query != self.find.query;
        let hits = self.find_scan_all(st_query, opts);
        if hits.invalid_regex {
            // §06⑤：保旧 hits / 旧 current，词照记（输入框显示的是它），
            // 只点红标志；修正后走正常路径自动清除。
            self.find.query = st_query.to_string();
            self.find.options = opts;
            self.find.invalid_regex = true;
            return false;
        }
        let had_hits = !self.find.hits.hits.is_empty();
        let old_current_abs = self.find.current.and_then(|i| self.find.hits.hits.get(i).copied());
        self.find.query = st_query.to_string();
        self.find.options = opts;
        self.find.invalid_regex = false;
        let empty_now = hits.hits.is_empty();
        self.find.hits = hits;
        self.find.scanned_at = Some(std::time::Instant::now());
        self.find.current = if empty_now {
            None
        } else if fresh_query {
            // 新词：定锚到离视口最近的命中。
            self.find_nearest_to_view(&self.find.hits)
        } else {
            // 只切了规则（Aa/正则）：§06⑥ 原位置附近的最近命中。
            match old_current_abs {
                Some(anchor) if had_hits => self
                    .find
                    .hits
                    .hits
                    .iter()
                    .position(|&h| h >= anchor)
                    .or(Some(self.find.hits.hits.len() - 1)),
                _ => self.find_nearest_to_view(&self.find.hits),
            }
        };
        true
    }

    /// 新输出/滚动后调用：若「当前匹配仍在命中表里」就不动（§04 新输出行）；
    /// 仅当 current 失效（历史截断挤出表 / 表被重扫过）才重定锚到最近命中。
    /// 永远不因为「视口变了」而改 current——逐键不重跳同源。
    pub(crate) fn find_reanchor_if_needed(&mut self) {
        if self.find.query.is_empty() || self.find.hits.hits.is_empty() {
            if !self.find.hits.hits.is_empty() {
                self.find.current = None;
            }
            return;
        }
        let in_range = self.find.current.is_some_and(|i| i < self.find.hits.hits.len());
        if !in_range {
            self.find.current = self.find_nearest_to_view(&self.find.hits);
        }
    }

    /// next/prev 步进（§03②/§04 跳转行）。dir: +1 下一个 / -1 上一个。
    /// 循环查找：到头卷回（§04 默认行为行）。返回新的 current 下标。
    pub(crate) fn find_step(&mut self, dir: i32) -> Option<usize> {
        if self.find.hits.hits.is_empty() {
            // 没命中但视口外可能有（还没扫过/被截断）——不做隐式重扫，
            // 调用方在查询变化时已经全量扫过。
            return None;
        }
        let n = self.find.hits.hits.len();
        let next = match self.find.current {
            None => self.find_nearest_to_view(&self.find.hits),
            Some(i) => {
                let step = |d: i32| ((i as i32) + d).rem_euclid(n as i32) as usize;
                Some(if dir >= 0 { step(1) } else { step(-1) })
            }
        };
        self.find.current = next;
        next
    }

    /// 「点击高亮成为当前匹配」的入口（§03⑤）：把命中表里**绝对行号+列**
    /// 落在它上面的那条设为当前——点击命中矩形内**任意字符**都算（不必
    /// 对准首列）。没找到（点击的是选区/空白）→ 不动。
    pub(crate) fn find_set_current_at(&mut self, abs_row: usize, col: i32) -> bool {
        if let Some(i) = self
            .find
            .hits
            .hits
            .iter()
            .position(|&(a, c, l)| a == abs_row && col >= c && col < c + l)
        {
            self.find.current = Some(i);
            true
        } else {
            false
        }
    }

    /// 跳转居中的滚动目标（§03②）：把当前匹配的绝对行号换算成
    /// `terminal-scroll-to` 的 view_offset，目标 = 匹配行 − 行数/2。
    /// alt 屏无回滚 → None（UI 保持不动，遮挡交给下移让位）。
    pub(crate) fn find_center_scroll_target(&self) -> Option<usize> {
        let idx = self.find.current?;
        let &(abs, _, _) = self.find.hits.hits.get(idx)?;
        if self.parser.screen().alternate_screen() {
            return None;
        }
        let rows = self.parser.screen().size().0 as usize;
        let (_, live_used) = self.live_rows();
        let top = self.view_top_abs(live_used);
        let combined_len = self.history.len() + live_used.max(rows);
        // 目标视口顶 = 匹配行 − rows/2，夹进 [0, combined_len − rows]。
        let want_top = abs.saturating_sub(rows / 2);
        let max_top = combined_len.saturating_sub(rows);
        let want_top = want_top.min(max_top);
        if want_top == top {
            return None; // 已在目标位，不用滚
        }
        // view_offset = combined_len − rows − top
        Some(combined_len.saturating_sub(rows + want_top))
    }

    /// 当前匹配的绝对行号（遮挡让位判定用：匹配行在可视区的 y）。
    pub(crate) fn find_current_abs_row(&self) -> Option<usize> {
        let idx = self.find.current?;
        self.find.hits.hits.get(idx).map(|&(abs, _, _)| abs)
    }

    /// 纯数据读取：UI 计数条（current 从 1 起显示）。
    pub(crate) fn find_counts(&self) -> (usize, Option<usize>) {
        (self.find.hits.total(), self.find.current.map(|i| i + 1))
    }

    pub(crate) fn find_invalid_regex(&self) -> bool {
        self.find.invalid_regex
    }
}

/// FindOptions 没有 PartialEq 之外的比较需求，手写等值（两个 bool）。
fn st_opts_eq(a: &FindOptions, b: &FindOptions) -> bool {
    a.match_case == b.match_case && a.use_regex == b.use_regex
}
