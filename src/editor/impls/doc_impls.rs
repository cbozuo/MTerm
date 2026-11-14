// (#editor-hifi) EditorDoc 的编辑/撤销/坐标换算实现。列以 unicode 字符计,
// 像素宽度换算用 unicode-width(CJK≈2 格),渲染侧只拿行文本+span 字符区间。
use ropey::{Rope, RopeSlice};
use super::*;

impl EditorDoc {
    // ── 行读取 ────────────────────────────────────────────────
    /// 行内容(不含行尾换行)。
    pub(crate) fn line_text(&self, line: usize) -> String {
        if line >= self.len_lines() {
            return String::new();
        }
        let l = self.rope.line(line);
        let eol = line_ending_len(&l);
        l.slice(0..l.len_chars() - eol).to_string()
    }

    pub(crate) fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// 行的像素宽度(char_px=单格宽;CJK 按 2 格计)——横向滚动内容宽用。
    pub(crate) fn line_pixel_width(&self, line: usize, char_px: f64) -> f64 {
        chars_px(&self.line_text(line), char_px)
    }

    /// 全文最大行宽(打开/编辑后重算一次,滚动复用;rope 分块迭代 ~100ms/100MB)。
    pub(crate) fn max_line_pixel_width(&self, char_px: f64) -> f64 {
        let mut max = 0.0f64;
        for i in 0..self.len_lines() {
            let w = self.line_pixel_width(i, char_px);
            if w > max {
                max = w;
            }
        }
        max
    }

    // ── 坐标换算 ──────────────────────────────────────────────
    /// (行,字符列) → 全文字节偏移。越界自动钳制(列钳到行内可写位置=不含行尾)。
    pub(crate) fn pos_to_byte(&self, p: Pos) -> usize {
        let line = p.line.min(self.len_lines().saturating_sub(1));
        let l = self.rope.line(line);
        let eol = line_ending_len(&l);
        let col = p.col.min(l.len_chars() - eol);
        self.rope.line_to_byte(line) + l.slice(0..col).len_bytes()
    }

    /// 全文字节偏移 → (行,字符列)。
    pub(crate) fn byte_to_pos(&self, byte: usize) -> Pos {
        let byte = byte.min(self.rope.len_bytes());
        let line = self.rope.byte_to_line(byte);
        let l = self.rope.line(line);
        let in_line = byte - self.rope.line_to_byte(line);
        let mut col = 0usize;
        let mut acc = 0usize;
        for ch in l.chars() {
            if acc >= in_line {
                break;
            }
            acc += ch.len_utf8();
            col += 1;
        }
        Pos::new(line, col)
    }

    /// 钳到可编辑范围(末行末列)。
    pub(crate) fn clamp_pos(&self, p: Pos) -> Pos {
        let line = p.line.min(self.len_lines().saturating_sub(1));
        let l = self.rope.line(line);
        let eol = line_ending_len(&l);
        Pos::new(line, p.col.min(l.len_chars() - eol))
    }

    /// 光标像素 x(该行前 col 个字符的宽度合计,char_px=单格像素宽)。
    pub(crate) fn cursor_x_px(&self, p: Pos, char_px: f64) -> f64 {
        let t = self.line_text(p.line);
        let mut w = 0.0f64;
        for (i, c) in t.chars().enumerate() {
            if i >= p.col {
                break;
            }
            w += char_px_one(c) * char_px;
        }
        w
    }

    // ── 编辑 ──────────────────────────────────────────────────
    /// 在 at 处插入文本;返回插入后的光标落点。维护 marks 平移与 dirty。
    pub(crate) fn insert_text(&mut self, at: Pos, text: &str) -> Pos {
        self.push_undo();
        let byte = self.pos_to_byte(at);
        self.rope.insert(byte, text);
        let added_lines = text.matches('\n').count();
        let tail_cols = text.rsplit('\n').next().map(|s| s.chars().count()).unwrap_or(0);
        // marks:≥ at.line 的行整体 +added_lines;编辑起始行本身加标记
        if added_lines > 0 {
            self.edit_marks = self
                .edit_marks
                .iter()
                .map(|m| if *m >= at.line { m + added_lines } else { *m })
                .collect();
        }
        self.edit_marks.insert(at.line);
        self.dirty = true;
        self.edit_version += 1;
        self.spans_cache.clear();
        Pos::new(at.line + added_lines, tail_cols)
    }

    /// 删除 [a, b) 区间(a 先于 b)。
    pub(crate) fn delete_range(&mut self, a: Pos, b: Pos) {
        let (a, b) = if (a.line, a.col) <= (b.line, b.col) { (a, b) } else { (b, a) };
        self.push_undo();
        let bytes = self.pos_to_byte(a)..self.pos_to_byte(b);
        self.rope.remove(bytes);
        // marks:移除 a.line..=b.line 中的行,≥b.line 平移 (a.line - b.line - 1)
        let shift = a.line as isize - b.line as isize - 1;
        self.edit_marks = self
            .edit_marks
            .iter()
            .filter_map(|m| {
                if *m >= a.line && *m <= b.line {
                    None
                } else if *m > b.line {
                    Some((( *m as isize) + shift).max(0) as usize)
                } else {
                    Some(*m)
                }
            })
            .collect();
        self.dirty = true;
        self.edit_version += 1;
        self.spans_cache.clear();
    }

    /// 删除光标处前一字符(退格),返回新光标。字符列以 unicode 字符计(CJK 整字符删)。
    pub(crate) fn backspace(&mut self, cur: Pos) -> Pos {
        if cur.line == 0 && cur.col == 0 {
            return cur;
        }
        let (from, to) = if cur.col > 0 {
            (Pos::new(cur.line, cur.col - 1), cur)
        } else {
            // 行首退格:并入上一行末尾
            let prev_len = self.line_text(cur.line - 1).chars().count();
            (Pos::new(cur.line - 1, prev_len), cur)
        };
        self.delete_range(from, to);
        from
    }

    // ── 撤销/重做 ─────────────────────────────────────────────
    pub(crate) fn undo(&mut self) -> bool {
        if self.undo.is_empty() {
            return false;
        }
        let snap = self.undo.pop().unwrap();
        self.redo.push(UndoSnap {
            rope: std::mem::replace(&mut self.rope, snap.rope),
            marks: std::mem::replace(&mut self.edit_marks, snap.marks),
            cursor: std::mem::replace(&mut self.cursor, snap.cursor),
        });
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        if self.redo.is_empty() {
            return false;
        }
        let snap = self.redo.pop().unwrap();
        self.undo.push(UndoSnap {
            rope: std::mem::replace(&mut self.rope, snap.rope),
            marks: std::mem::replace(&mut self.edit_marks, snap.marks),
            cursor: std::mem::replace(&mut self.cursor, snap.cursor),
        });
        true
    }
}

/// 行尾长度(\r\n=2,\n/\r=1,末行无=0)。
pub(crate) fn line_ending_len(l: &ropey::RopeSlice) -> usize {
    let n = l.len_chars();
    if n >= 2 && l.slice(n - 2..n).to_string() == "\r\n" {
        2
    } else if n >= 1 {
        match l.slice(n - 1..n).chars().next() {
            Some('\n') | Some('\r') => 1,
            _ => 0,
        }
    } else {
        0
    }
}

/// 字符串像素宽(CJK 2 格)。
pub(crate) fn chars_px(s: &str, char_px: f64) -> f64 {
    s.chars()
        .map(|c| if unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) >= 2 { 2.0 } else { 1.0 })
        .sum::<f64>()
        * char_px
}

/// 光标 x 的 per-char 宽度回调版(供 cursor_x_px 用,签名简化用不到)。
pub(crate) fn char_px_one(c: char) -> f64 {
    if unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) >= 2 { 2.0 } else { 1.0 }
}
