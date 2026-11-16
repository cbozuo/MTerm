// (#editor-native 2026-11-16) 编辑器多 tab 容器。
//
// 文本权威在 Slint 侧 TextInput(原生 shaping / 光标 / 选择 / IME / 撤销)。
// 这里只做宿主侧的镜像与元数据:非活动 tab 的正文暂存在 `text`,切回时下发;
// 活动 tab 的实时正文从 Slint 属性读(save / highlight / find / format 用),
// 因此不存在「Rust 副本过期」的问题(旧 rope 模型每次编辑都要回写,撤销还会脱队)。
use super::Lang;

/// 大文件只读档位:超过此体积不进可编辑态。
/// 原生渲染要把整篇文本交给 Slint 并参与排版(行高 = 全文行盒累加),
/// 体积线性抬升内存与重排成本;旧 rope 模型此处同口径(16MB)。
/// 旧的「>200 万行」那档用不上 —— SFTP 侧 `validate_editor_text` 已直接拒收。
const BIG_FILE_BYTES: usize = 16 * 1024 * 1024;

/// 单份打开的文档。
pub(crate) struct TabDoc {
    /// 编辑器窗口内的 tab id(uuid)。Slint 侧 tab 条目按此对齐。
    pub id: String,
    /// 文件名(标题/tab 显示)。
    pub name: String,
    /// 远端绝对路径(SFTP 保存回传目标)。
    pub path: String,
    /// 所属会话 tab id(SFTP 连接复用键)。
    pub session_tab_id: String,
    /// 打开时的编码显示名(UTF-8/GBK…),状态栏展示与另存编码用。
    pub encoding: String,
    pub lang: Lang,
    /// 只读(查看模式 / 大文件)。
    pub readonly: bool,
    /// 大文件档(→ 只读 + 提示条;与「查看模式」只读区分开,便于状态栏说法)。
    pub big: bool,
    /// 非活动时的正文暂存(活动 tab 以 Slint 为准)。
    pub text: String,
    /// 相对上次保存是否有改动。
    pub dirty: bool,
}

impl TabDoc {
    pub(crate) fn new(
        text: &str,
        name: String,
        path: String,
        session_tab_id: String,
        encoding: String,
        readonly: bool,
    ) -> Self {
        // 语言:优先远端路径扩展名,shebang 兜底
        let lang = Lang::detect(&path, first_line_of(text));
        // 大文件 → 强制只读(查看模式传进来的 readonly 也保留)
        let big = text.len() > BIG_FILE_BYTES;
        Self {
            id: format!("editor-{}", next_tab_seq()),
            name,
            path,
            session_tab_id,
            encoding,
            lang,
            readonly: readonly || big,
            big,
            text: text.to_string(),
            dirty: false,
        }
    }
}

/// shebang 行(`Lang::detect` 内部按 `#!` 前缀判断)。
fn first_line_of(text: &str) -> Option<&str> {
    text.lines().next()
}

fn next_tab_seq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

pub(crate) struct EditorTabs {
    pub docs: Vec<TabDoc>,
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

    pub(crate) fn active_doc(&self) -> Option<&TabDoc> {
        self.docs.get(self.active)
    }

    pub(crate) fn active_doc_mut(&mut self) -> Option<&mut TabDoc> {
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
    pub(crate) fn open(&mut self, doc: TabDoc) -> usize {
        self.docs.push(doc);
        self.active = self.docs.len() - 1;
        self.active
    }

    /// 关闭;返回被关文档(调用方负责 dirty 确认后调用)。索引平移保证 active 仍指向合理 tab。
    pub(crate) fn close(&mut self, idx: usize) -> Option<TabDoc> {
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
    pub(crate) fn close_others(&mut self, keep: usize) -> Vec<TabDoc> {
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
        self.active = self.docs.len().saturating_sub(1);
        closed
    }

    /// 关闭全部。返回被关文档(逆序)。
    pub(crate) fn close_all(&mut self) -> Vec<TabDoc> {
        let mut closed = Vec::new();
        while !self.docs.is_empty() {
            if let Some(d) = self.close(0) {
                closed.push(d);
            }
        }
        closed
    }

    /// 关闭右侧(idx 之后全部)。返回被关文档(逆序)。
    pub(crate) fn close_right(&mut self, idx: usize) -> Vec<TabDoc> {
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

    fn doc(name: &str) -> TabDoc {
        TabDoc::new(
            &format!("// {name}"),
            name.into(),
            format!("/tmp/{name}"),
            "s1".into(),
            "UTF-8".into(),
            false,
        )
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

    #[test]
    fn tab_ids_are_unique() {
        let a = doc("a");
        let b = doc("b");
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn big_file_forces_readonly() {
        // 16MB 以上进只读档(与旧 rope 模型同口径)
        let big_text = "x".repeat(BIG_FILE_BYTES + 1);
        let d = TabDoc::new(&big_text, "big.log".into(), "/t/big.log".into(), "s".into(), "UTF-8".into(), false);
        assert!(d.big, "超过阈值应标记为大文件");
        assert!(d.readonly, "大文件必须只读");
        // 阈值内保持可编辑
        let small = TabDoc::new("hello", "a.txt".into(), "/t/a.txt".into(), "s".into(), "UTF-8".into(), false);
        assert!(!small.big && !small.readonly);
    }
}
