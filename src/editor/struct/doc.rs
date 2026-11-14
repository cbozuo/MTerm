// (#editor-hifi) 编辑器文档:一份打开的文件 = 一个 EditorDoc(rope + 光标/选区 +
// 撤销栈 + 修改标记)。多 tab 见 tabs.rs;编辑/撤销/坐标换算的实现在 doc_impls.rs。
use super::Lang;
use ropey::Rope;

/// 光标/选区端点。line/col 均 0 基;col 以 unicode 字符计(渲染侧等宽换算像素,
/// CJK 宽度由 unicode-width 在像素换算时处理)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pos {
    pub line: usize,
    pub col: usize,
}

impl Pos {
    pub(crate) fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// 撤销快照:rope + marks + 光标一起回滚,避免 marks 在行数变化后漂移。
pub(crate) struct UndoSnap {
    pub rope: Rope,
    pub marks: std::collections::BTreeSet<usize>,
    pub cursor: Pos,
}

/// 单份文档。大文件(>16MB 或 >200 万行)进只读模式(big=true),不建撤销栈。
pub(crate) struct EditorDoc {
    pub rope: Rope,
    pub lang: Lang,
    /// 打开时的编码显示名(UTF-8/GBK/GB18030…),仅状态栏展示与切换重开用。
    pub encoding: String,
    pub name: String,
    /// 远端绝对路径(SFTP 保存回传目标)。
    pub path: String,
    /// 所属会话 tab id(SFTP 连接复用键)。
    pub session_tab_id: String,
    /// 编辑器窗口内的 tab id(uuid)。
    pub tab_id: String,
    pub readonly: bool,
    /// 大文件模式(设计稿②):只读 + 行级高亮 + 不建撤销栈。
    pub big: bool,
    pub dirty: bool,
    pub cursor: Pos,
    /// 选区(锚点, 活动点);None = 无选区。活动点即 cursor。
    pub sel_anchor: Option<Pos>,
    /// 修改过的行号(0 基)集合:gutter 3px 琥珀标记;保存成功后清空。
    pub edit_marks: std::collections::BTreeSet<usize>,
    /// tree-sitter 语法树缓存(ts 语言才用);编辑时 edit_version+1,惰性重 parse。
    pub tree_cache: Option<tree_sitter::Tree>,
    pub edit_version: u64,
    pub ts_version: u64,
    /// 行级 spans 缓存(行号 → spans),编辑时清空(ts 模式下重 parse 时清空)。
    pub spans_cache: std::collections::HashMap<usize, Vec<super::highlight::Span>>,
    /// 本 tab 的滚动状态(px;多 tab 各自保留)。
    pub scroll_y: usize,
    pub scroll_x: usize,
    /// 查找状态(per tab):全量命中 + 当前命中索引(usize::MAX = 无)。
    pub find_hits: Vec<crate::editor::find::Hit>,
    pub find_cur: usize,
    pub(super) undo: Vec<UndoSnap>,
    pub(super) redo: Vec<UndoSnap>,
}

impl EditorDoc {
    /// 从已解码文本建文档(编码检测在 encode.rs 完成后再走到这里)。
    pub(crate) fn new(
        text: &str,
        name: String,
        path: String,
        session_tab_id: String,
        encoding: String,
        force_readonly: bool,
    ) -> Self {
        let rope = Rope::from_str(text);
        let first_line = rope.line(0).to_string();
        let lang = Lang::detect(&name, Some(&first_line));
        // 大文件判定(设计稿「大文件模式」:>16MB 或 >200 万行 → 只读)
        let big = rope.len_bytes() > 16 * 1024 * 1024 || rope.len_lines() > 2_000_000;
        let readonly = force_readonly || big;
        Self {
            rope,
            lang,
            encoding,
            name,
            path,
            session_tab_id,
            tab_id: uuid::Uuid::new_v4().to_string(),
            readonly,
            big,
            dirty: false,
            cursor: Pos::new(0, 0),
            sel_anchor: None,
            edit_marks: std::collections::BTreeSet::new(),
            tree_cache: None,
            edit_version: 0,
            ts_version: 0,
            spans_cache: std::collections::HashMap::new(),
            scroll_y: 0,
            scroll_x: 0,
            find_hits: Vec::new(),
            find_cur: usize::MAX,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub(crate) fn push_undo(&mut self) {
        if self.big {
            return; // 大文件只读,不会走到编辑路径
        }
        self.undo.push(UndoSnap {
            rope: self.rope.clone(),
            marks: self.edit_marks.clone(),
            cursor: self.cursor,
        });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub(crate) fn undo_len(&self) -> usize {
        self.undo.len()
    }
}
