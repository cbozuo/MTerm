// (#editor-hifi) 编辑器窗口内多 tab 容器(用户定稿:完整多文件 tab)。
// 同一会话的 SFTP 连接复用:doc.session_tab_id 即 sftp_handles 的键。
use super::EditorDoc;

pub(crate) struct EditorTabs {
    pub docs: Vec<EditorDoc>,
    pub active: usize,
    /// 查找大小写敏感开关(find-icon 切换,回传 Rust 存档)
    pub case_sensitive: bool,
}

impl EditorTabs {
    pub(crate) fn new() -> Self {
        Self { docs: Vec::new(), active: 0, case_sensitive: true }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    pub(crate) fn active_doc(&self) -> Option<&EditorDoc> {
        self.docs.get(self.active)
    }

    pub(crate) fn active_doc_mut(&mut self) -> Option<&mut EditorDoc> {
        self.docs.get_mut(self.active)
    }

    /// 激活指定 tab;越界时钳到最后一个。
    pub(crate) fn activate(&mut self, idx: usize) {
        if !self.docs.is_empty() {
            self.active = idx.min(self.docs.len() - 1);
        }
    }

    /// 按(远端路径, 会话)查已打开的 tab;SFTP 面板「编辑」重复打开时激活而非新建。
    pub(crate) fn index_of_path(&self, path: &str, session_tab_id: &str) -> Option<usize> {
        self.docs
            .iter()
            .position(|d| d.path == path && d.session_tab_id == session_tab_id)
    }

    /// 打开新文档并激活(调用方保证路径未打开过)。
    pub(crate) fn open(&mut self, doc: EditorDoc) -> usize {
        self.docs.push(doc);
        self.active = self.docs.len() - 1;
        self.active
    }

    /// 关闭;返回被关文档(调用方负责 dirty 确认后调用)。索引平移保证 active 仍指向合理 tab。
    pub(crate) fn close(&mut self, idx: usize) -> Option<EditorDoc> {
        if idx >= self.docs.len() {
            return None;
        }
        let doc = self.docs.remove(idx);
        if self.docs.is_empty() {
            self.active = 0;
        } else if self.active >= self.docs.len() {
            self.active = self.docs.len() - 1;
        } else if idx < self.active {
            self.active -= 1;
        }
        Some(doc)
    }

    /// 关闭其他(保留 keep)。返回被关文档(逆序,便于逐个确认)。
    pub(crate) fn close_others(&mut self, keep: usize) -> Vec<EditorDoc> {
        let mut closed = Vec::new();
        let mut i = self.docs.len();
        while i > 0 {
            i -= 1;
            if i != keep {
                if let Some(d) = self.close(i) {
                    closed.push(d);
                }
            }
        }
        self.active = 0.min(self.docs.len().saturating_sub(1));
        closed
    }

    /// 关闭全部。返回被关文档(逆序)。
    pub(crate) fn close_all(&mut self) -> Vec<EditorDoc> {
        let mut closed = Vec::new();
        while !self.docs.is_empty() {
            if let Some(d) = self.close(0) {
                closed.push(d);
            }
        }
        closed
    }

    /// 关闭右侧(idx 之后全部)。返回被关文档(逆序)。
    pub(crate) fn close_right(&mut self, idx: usize) -> Vec<EditorDoc> {
        let mut closed = Vec::new();
        while self.docs.len() > idx + 1 {
            if let Some(d) = self.close(idx + 1) {
                closed.push(d);
            }
        }
        closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(name: &str) -> EditorDoc {
        EditorDoc::new(&format!("// {name}"), name.into(), format!("/tmp/{name}"), "s1".into(), "UTF-8".into(), false)
    }

    #[test]
    fn open_activate_close() {
        let mut t = EditorTabs::new();
        assert!(t.is_empty());
        t.open(doc("a"));
        t.open(doc("b"));
        t.open(doc("c"));
        assert_eq!(t.active, 2);
        t.activate(0);
        assert_eq!(t.active_doc().unwrap().name, "a");
        let c = t.close(2).unwrap();
        assert_eq!(c.name, "c");
        assert_eq!(t.docs.len(), 2);
        t.activate(0);
        let a = t.close(0).unwrap();
        assert_eq!(a.name, "a");
        // active 收敛到有效区间
        assert!(t.active < t.docs.len());
    }

    #[test]
    fn close_others_and_right() {
        let mut t = EditorTabs::new();
        for n in ["a", "b", "c", "d"] {
            t.open(doc(n));
        }
        t.activate(1);
        let closed = t.close_others(1);
        assert_eq!(closed.len(), 3);
        assert_eq!(t.docs.len(), 1);
        assert_eq!(t.active_doc().unwrap().name, "b");

        t.open(doc("e"));
        t.open(doc("f"));
        let closed = t.close_right(0);
        assert_eq!(closed.len(), 2);
        assert_eq!(t.docs.len(), 1);
    }

    #[test]
    fn path_lookup() {
        let mut t = EditorTabs::new();
        t.open(doc("a"));
        assert_eq!(t.index_of_path("/tmp/a", "s1"), Some(0));
        assert_eq!(t.index_of_path("/tmp/a", "s2"), None);
    }
}
