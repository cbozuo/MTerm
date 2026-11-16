// (#editor-native 2026-11-16) 查找:全文同步扫,产出 **UTF-8 字节区间**。
//
// 字节区间正是 TextInput 的坐标系:`set-selection-offsets(start, end)`、`syntax-spans`
// 都用它,因此中文/emoji 前后的命中都精确(旧实现给的是「行内字符列」,还要再经一层
// 网格换算才能落到屏幕上)。大小写不敏感用原始文本逐字符比较实现 —— 不能把小写化后的
// 字符串当干草堆,因为 Unicode 小写化可能改变字节长度('İ' → 'i̇'),偏移会整体错位。
pub(crate) struct Hit {
    /// 命中起始字节(含)
    pub start: usize,
    /// 命中结束字节(不含)
    pub end: usize,
    /// 0 基行号(滚动条命中标记用)
    pub line: usize,
}

/// 全文查找,命中区间互不重叠(与旧行为一致:命中后从命中尾继续)。
pub(crate) fn find_all(text: &str, query: &str, case_sensitive: bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    if query.is_empty() {
        return hits;
    }
    if case_sensitive {
        let mut byte = 0usize;
        while byte <= text.len() {
            let Some(off) = text[byte..].find(query) else { break };
            let s = byte + off;
            let e = s + query.len();
            hits.push(Hit { start: s, end: e, line: 0 });
            byte = e;
        }
    } else {
        let qchars: Vec<char> = query.chars().collect();
        let hay: Vec<(usize, char)> = text.char_indices().collect();
        let mut j = 0usize;
        while j + qchars.len() <= hay.len() {
            let matched = (0..qchars.len()).all(|k| {
                let (_, c) = hay[j + k];
                let (_, q) = (0, qchars[k]);
                // 逐字符忽略大小写:先直接比,再比小写化后的单字符
                c == q || c.to_lowercase().eq(q.to_lowercase())
            });
            if matched {
                let start = hay[j].0;
                let end = if j + qchars.len() < hay.len() {
                    hay[j + qchars.len()].0
                } else {
                    text.len()
                };
                hits.push(Hit { start, end, line: 0 });
                j += qchars.len();
            } else {
                j += 1;
            }
        }
    }
    // 行号:命中已按字节升序,一次线性推进即可
    let mut line = 0usize;
    let mut scan = 0usize;
    for h in hits.iter_mut() {
        let seg = &text[scan..h.start.min(text.len())];
        line += seg.bytes().filter(|b| *b == b'\n').count();
        h.line = line;
        scan = h.start.min(text.len());
    }
    hits
}

/// 字节偏移 → (0 基行, 0 基字符列)。列以字符计(状态栏口径,与旧版一致)。
pub(crate) fn line_col(text: &str, byte: usize) -> (usize, usize) {
    let byte = byte.min(text.len());
    let mut line_start = 0usize;
    let mut line = 0usize;
    for (i, c) in text.char_indices() {
        if i >= byte {
            break;
        }
        if c == '\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    let col = text[line_start..byte].chars().count();
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_and_case() {
        let t = "sftp sftp Sftp\nxx sftp";
        assert_eq!(find_all(t, "sftp", true).len(), 3);
        assert_eq!(find_all(t, "sftp", false).len(), 4);
    }

    #[test]
    fn hit_offsets_are_char_boundaries() {
        let t = "第一行 hit\n第二行\n三个: hit";
        for h in find_all(t, "hit", false) {
            assert!(t.is_char_boundary(h.start), "起点非字符边界 {:?}", h.start);
            assert!(t.is_char_boundary(h.end), "终点非字符边界 {:?}", h.end);
            assert_eq!(&t[h.start..h.end], "hit");
        }
    }

    #[test]
    fn cjk_query_offsets_are_exact() {
        // 纯中文查询:字节区间必须精确覆盖查询本身(旧实现按字符列换算)
        let t = "配置: 超时 = 30\n说明: 超时 上限";
        let hits = find_all(t, "超时", false);
        assert_eq!(hits.len(), 2);
        for h in &hits {
            assert_eq!(&t[h.start..h.end], "超时");
            assert_eq!(h.end - h.start, "超时".len());
        }
    }

    #[test]
    fn hit_line_numbers_are_reported() {
        let t = "a\nb hit\nc";
        let hits = find_all(t, "hit", true);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    fn line_col_accounts_for_cjk_columns() {
        let t = "ab中文cd\n第二行";
        // 字节 = "ab中" 的 2 + 3 = 5 → 第 0 行第 3 字符列
        assert_eq!(line_col(t, 5), (0, 3));
        // 第二行起点
        assert_eq!(line_col(t, "ab中文cd\n".len()), (1, 0));
    }

    #[test]
    fn empty_query_finds_nothing() {
        assert!(find_all("abc", "", true).is_empty());
    }
}
