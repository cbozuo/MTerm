// (#editor-hifi) 查找:rope 全文扫(可见区即时标 + 后台线程全量扫由宿主桥起线程,
// 数据层只给同步版)。Hit 用 (行, 字符列, 长度) 表达,直接映射滚动条标记与正文高亮。
use super::EditorDoc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hit {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

/// 全文查找(大小写敏感一期,与现有 editor-count-matches 行为一致)。
pub(crate) fn find_all(doc: &EditorDoc, query: &str, case_sensitive: bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    if query.is_empty() {
        return hits;
    }
    let (q, hay) = if case_sensitive {
        (query.to_string(), doc.rope.to_string())
    } else {
        (query.to_lowercase(), doc.rope.to_string().to_lowercase())
    };
    if q.is_empty() {
        return hits;
    }
    let mut byte = 0usize;
    while let Some(off) = hay[byte..].find(&q) {
        let s = byte + off;
        let e = s + q.len();
        let line = doc.rope.byte_to_line(s.min(doc.rope.len_bytes()));
        // 列:行起点到命中的字符数(查询长度按命中字节反推字符)
        let line_start = doc.rope.line_to_byte(line);
        let col = doc.rope.line(line).slice(0..s - line_start).chars().count();
        let len = doc.rope.line(line)
            .slice(0..(e - line_start).min(doc.rope.line(line).len_bytes()))
            .chars()
            .count()
            - col;
        hits.push(Hit { line, col, len });
        byte = e;
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> EditorDoc {
        EditorDoc::new(text, "t".into(), "/t".into(), "s".into(), "UTF-8".into(), false)
    }

    #[test]
    fn basic_and_case() {
        let d = doc("sftp sftp Sftp\nxx sftp");
        assert_eq!(find_all(&d, "sftp", true).len(), 3);
        assert_eq!(find_all(&d, "sftp", false).len(), 4);
    }

    #[test]
    fn hit_coords() {
        let d = doc("line1 hit\nline2\nthree: hit");
        let hits = find_all(&d, "hit", true);
        assert_eq!(hits[0], Hit { line: 0, col: 6, len: 3 });
        assert_eq!(hits[1], Hit { line: 2, col: 7, len: 3 });
    }

    #[test]
    fn empty_query() {
        let d = doc("abc");
        assert!(find_all(&d, "", true).is_empty());
    }
}
