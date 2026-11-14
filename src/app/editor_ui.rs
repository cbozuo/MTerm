// (#editor-hifi 2026-11-15) 宿主桥:EditorWindow(独立窗)↔ src/editor 领域层。
// 状态:SharedTabs(Rc<RefCell<EditorTabs>>,单窗口单实例,OnceLock 持有)。
// 渲染模型按「可见行 ± 缓冲」重建,全文不过 Slint 边界;键入/光标/滚动权威在 Rust。
// IME:vendor 补丁(#editor-hifi-ime)让 FocusScope key-pressed 直收 commit 文本,
// input-text 直插 rope,无需输入条(设计稿兜底方案保留:若实机异常切输入条模式)。
use crate::editor::{self, EditorDoc, EditorTabs, Pos};
use crate::editor::find::Hit;
use crate::editor::highlight::SpanKind;
use crate::editor::format as fmt_ext;
use crate::sftp::DownloadConflict;
use crate::sftp::SftpHandles;
use crate::ui::{
    EditorFindResult, EditorLineRow, EditorSpan, EditorSpanKind, EditorTabInfo, EditorWindow,
};
use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) type SharedTabs = Rc<RefCell<EditorTabs>>;

const ROW_PX: f64 = 20.0;
const CHAR_PX: f64 = 7.8;
const CODE_PAD_PX: f64 = 12.0;
const VIEW_BUFFER: usize = 6;

thread_local! {
    static TABS: RefCell<Option<SharedTabs>> = const { RefCell::new(None) };
    static STATUS_TIMER: RefCell<Option<slint::Timer>> = const { RefCell::new(None) };
}

pub(crate) fn tabs() -> SharedTabs {
    TABS.with(|c| c.borrow().as_ref().expect("editor tabs 未初始化(editor_ui::wire)").clone())
}

/// 打开/激活远端文件(SFTP read_text 事件的入口;重复打开=激活已有 tab)。
pub(crate) fn open_remote_file(
    editor: &EditorWindow,
    text: &str,
    name: &str,
    path: &str,
    session_tab_id: &str,
    encoding: &str,
    force_readonly: bool,
) {
    {
        let tabs = tabs();
        let mut t = tabs.borrow_mut();
        if let Some(i) = t.index_of_path(path, session_tab_id) {
            t.activate(i);
        } else {
            let doc = EditorDoc::new(
                text,
                name.to_string(),
                path.to_string(),
                session_tab_id.to_string(),
                encoding.to_string(),
                force_readonly,
            );
            t.open(doc);
        }
    }
    refresh(editor);
    let _ = editor.show();
    super::window::arm_window_icon(editor.window());
    editor.window().with_winit_window(|ww| ww.focus_window());
}

/// 一次性注册全部回调(在 wire_sftp_callbacks 末尾调用,sftp_handles 可用)。
pub(crate) fn wire(editor: &EditorWindow, sftp_handles: SftpHandles) {
    TABS.with(|c| {
        if c.borrow().is_none() {
            *c.borrow_mut() = Some(SharedTabs::new(RefCell::new(EditorTabs::new())));
        }
    });

    // ── 保存(当前 tab 回传远端)──
    {
        let handles = sftp_handles.clone();
        let weak = editor.as_weak();
        editor.on_save_file(move || {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            if doc.readonly || !doc.dirty {
                return;
            }
            let path = doc.path.clone();
            let content = doc.rope.to_string();
            let session = doc.session_tab_id.clone();
            doc.dirty = false;
            doc.edit_marks.clear();
            if let Ok(handles) = handles.lock() {
                if let Some(h) = handles.get(&session) {
                    h.write_text(path, content);
                }
            }
            drop(t);
            refresh(&editor);
        });
    }

    // ── 关闭窗口(直接隐藏;tabs 保留,重开同文件恢复状态)──
    {
        let weak = editor.as_weak();
        editor.on_close_editor(move || {
            let Some(editor) = weak.upgrade() else { return };
            editor.set_editor_find_open(false);
            editor.set_ctx_open(false);
            editor.set_tabmenu_open(false);
            editor.set_dl_open(false);
            let _ = editor.hide();
        });
    }

    // ── tab 操作 ──
    {
        let weak = editor.as_weak();
        editor.on_tab_activate(move |idx| {
            let Some(editor) = weak.upgrade() else { return };
            tabs().borrow_mut().activate(idx as usize);
            refresh(&editor);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_tab_close(move |idx| {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let _ = tabs.borrow_mut().close(idx as usize);
            refresh(&editor);
            if tabs.borrow().is_empty() {
                editor.set_editor_find_open(false);
                editor.set_ctx_open(false);
                editor.set_tabmenu_open(false);
                editor.set_dl_open(false);
                let _ = editor.hide();
            }
        });
    }
    {
        let weak = editor.as_weak();
        let handles = sftp_handles.clone();
        editor.on_tab_menu(move |cmd: SharedString, idx: i32| {
            let Some(editor) = weak.upgrade() else { return };
            editor.set_tabmenu_open(false);
            let idx = idx.max(0) as usize;
            let tabs = tabs();
            match cmd.as_str() {
                "close" => {
                    let _ = tabs.borrow_mut().close(idx);
                }
                "close-others" => {
                    let _ = tabs.borrow_mut().close_others(idx);
                }
                "close-right" => {
                    let _ = tabs.borrow_mut().close_right(idx);
                }
                "close-all" => {
                    let n = tabs.borrow().docs.len();
                    let _ = tabs.borrow_mut().close_right(0);
                    let _ = n;
                }
                "download" => {
                    if let Some(d) = tabs.borrow().docs.get(idx) {
                        editor.set_editor_download_name(d.name.clone().into());
                        editor.set_editor_download_info(
                            format!("来自 {}:{}", d.session_tab_id, d.path).into(),
                        );
                        editor.set_editor_download_dir(default_download_dir().into());
                        editor.set_dl_open(true);
                    }
                }
                "show-panel" => {
                    // 定位 SFTP 面板到该文件目录:复用面板 cd(终端 cd 链已有,#54)
                    if let Some(d) = tabs.borrow().docs.get(idx) {
                        if let Ok(handles) = handles.lock() {
                            if let Some(h) = handles.get(&d.session_tab_id) {
                                if let Some(dir) = d.path.rsplit_once('/').map(|(p, _)| p.to_string()) {
                                    h.list_dir(dir);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            refresh(&editor);
            if tabs.borrow().is_empty() {
                editor.set_editor_find_open(false);
                editor.set_ctx_open(false);
                editor.set_tabmenu_open(false);
                editor.set_dl_open(false);
                let _ = editor.hide();
            }
        });
    }

    // ── 键盘动作 ──
    {
        let weak = editor.as_weak();
        editor.on_key_action(move |cmd: SharedString| {
            let Some(editor) = weak.upgrade() else { return };
            perform_key_action(&editor, &cmd);
            refresh(&editor);
        });
    }
    // ── 字符输入(单键 / IME 提交整串)──
    {
        let weak = editor.as_weak();
        editor.on_input_text(move |text: SharedString| {
            let Some(editor) = weak.upgrade() else { return };
            if text.is_empty() || text.chars().any(|c| (c as u32) < 9 || ((c as u32) > 13 && (c as u32) < 32)) {
                return; // 控制字符(键位符号)不入正文
            }
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            if doc.readonly {
                return;
            }
            let cur = doc.cursor;
            doc.cursor = doc.insert_text(cur, &text);
            drop(t);
            refresh(&editor);
        });
    }
    // ── 点击定位(行内)──
    {
        let weak = editor.as_weak();
        editor.on_click_at(move |row_idx: f32, x_px, shift| {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            let first = doc.scroll_y / ROW_PX as usize;
            let line = first + row_idx.max(0.0) as usize;
            let col = (((x_px as f64) - CODE_PAD_PX) / CHAR_PX).round().max(0.0) as usize;
            let target = doc.clamp_pos(Pos::new(line, col));
            if shift {
                doc.sel_anchor.get_or_insert(doc.cursor);
            } else {
                doc.sel_anchor = None;
            }
            doc.cursor = target;
            drop(t);
            refresh(&editor);
        });
    }
    // ── 滚动 ──
    {
        let weak = editor.as_weak();
        editor.on_wheel(move |dy: i32, shift| {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            if shift {
                doc.scroll_x = (doc.scroll_x as i32 + dy).max(0) as usize;
            } else {
                let max_y = (doc.len_lines() * ROW_PX as usize).saturating_sub(1);
                doc.scroll_y = (doc.scroll_y as i32 + dy).clamp(0, max_y as i32) as usize;
            }
            drop(t);
            refresh(&editor);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_scroll_to_frac(move |frac: f32| {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            let max_y = (doc.len_lines() * ROW_PX as usize).saturating_sub(1);
            doc.scroll_y = ((frac.max(0.0).min(1.0)) as f64 * max_y as f64) as usize;
            drop(t);
            refresh(&editor);
        });
    }

    // ── 查找/替换 ──
    {
        let weak = editor.as_weak();
        editor.on_find_edited(move |case: bool| {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            let q = editor.get_editor_find_query().to_string();
            run_find(doc, &q, case);
            push_find_state(&editor, doc);
            drop(t);
            refresh(&editor);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_find_step(move |next: bool| {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            if doc.find_hits.is_empty() {
                return;
            }
            let n = doc.find_hits.len();
            doc.find_cur = match doc.find_cur {
                c if c == usize::MAX => if next { 0 } else { n - 1 },
                c if next => (c + 1) % n,
                c => (c + n - 1) % n,
            };
            // 跳转:命中行居中 + 选区 = 命中串
            if let Some(h) = doc.find_hits.get(doc.find_cur) {
                let vp = editor.get_viewport_rows().max(1) as usize;
                let target = h.line.saturating_sub(vp / 2) * ROW_PX as usize;
                let max_y = (doc.len_lines() * ROW_PX as usize).saturating_sub(1);
                doc.scroll_y = target.min(max_y);
                doc.cursor = Pos::new(h.line, h.col + h.len);
                doc.sel_anchor = Some(Pos::new(h.line, h.col));
            }
            push_find_state(&editor, doc);
            drop(t);
            refresh(&editor);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_find_close(move || {
            let Some(editor) = weak.upgrade() else { return };
            editor.set_editor_find_open(false);
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            if let Some(doc) = t.active_doc_mut() {
                doc.find_hits.clear();
                doc.find_cur = usize::MAX;
            }
            editor.set_editor_find_hit_lines(ModelRc::new(VecModel::from(Vec::<i32>::new())));
            editor.set_editor_find_count(0);
            editor.set_editor_find_cur(0);
            drop(t);
            refresh(&editor);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_replace_all(move || {
            let Some(editor) = weak.upgrade() else { return };
            let q = editor.get_editor_find_query().to_string();
            let rep = editor.get_editor_replace_text().to_string();
            if q.is_empty() {
                return;
            }
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            if doc.readonly {
                return;
            }
            doc.push_undo();
            let replaced = doc.rope.to_string().replace(q.as_str(), rep.as_str());
            doc.rope = ropey::Rope::from_str(&replaced);
            doc.dirty = true;
            doc.edit_marks.clear();
            doc.edit_version += 1;
            doc.spans_cache.clear();
            run_find(doc, &q, true);
            drop(t);
            refresh(&editor);
        });
    }
    // ── 格式化 ──
    {
        let weak = editor.as_weak();
        editor.on_format_doc(move || {
            let Some(editor) = weak.upgrade() else { return };
            let tabs = tabs();
            let mut t = tabs.borrow_mut();
            let Some(doc) = t.active_doc_mut() else { return };
            if doc.readonly || !editor::format::can_format(doc.lang) {
                set_status(&editor, "该文件不支持格式化(只读或语言不在支持列表)");
                return;
            }
            match editor::format::format_doc(doc.lang, &doc.rope.to_string()) {
                Ok(Some(new_text)) => {
                    doc.push_undo();
                    doc.rope = ropey::Rope::from_str(&new_text);
                    doc.dirty = true;
                    doc.edit_marks.clear();
                    doc.edit_version += 1;
                    doc.spans_cache.clear();
                    doc.cursor = Pos::new(0, 0);
                    doc.sel_anchor = None;
                    drop(t);
                    refresh(&editor);
                    set_status(&editor, "已格式化 · 撤销栈保留(Ctrl+Z 可退)");
                }
                Ok(None) => set_status(&editor, "该语言不支持格式化"),
                Err(msg) => set_status(&editor, &msg),
            }
        });
    }
    // ── 下载弹窗 ──
    {
        let weak = editor.as_weak();
        editor.on_download_browse(move || {
            let weak2 = weak.clone();
            // 系统对话框阻塞当前线程 → 起独立线程,完成后回 UI 线程写属性
            std::thread::spawn(move || {
                let Some(dir) = rfd::FileDialog::new()
                    .set_title("选择下载目录")
                    .pick_folder()
                else {
                    return;
                };
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(editor) = weak2.upgrade() {
                        editor.set_editor_download_dir(
                            dir.to_string_lossy().to_string().into(),
                        );
                    }
                });
            });
        });
    }
    {
        let handles = sftp_handles.clone();
        let weak = editor.as_weak();
        editor.on_download_confirm(move || {
            let Some(editor) = weak.upgrade() else { return };
            editor.set_dl_open(false);
            let tabs = tabs();
            let t = tabs.borrow();
            let Some(doc) = t.active_doc() else { return };
            let dir = editor.get_editor_download_dir().to_string();
            if dir.is_empty() {
                return;
            }
            if let Ok(handles) = handles.lock() {
                if let Some(h) = handles.get(&doc.session_tab_id) {
                    h.download(
                        doc.path.clone(),
                        dir,
                        DownloadConflict::Replace,
                    );
                    set_status(&editor, "已加入下载队列(后台传输)");
                }
            }
        });
    }
    // ── IME 输入条(兜底方案;直插通道为 vendor 补丁)──
    {
        let weak = editor.as_weak();
        editor.on_ime_commit(move || {
            let Some(editor) = weak.upgrade() else { return };
            let text = editor.get_editor_ime_text().to_string();
            editor.set_editor_ime_text("".into());
            editor.set_editor_ime_open(false);
            if !text.is_empty() {
                editor.invoke_input_text(text.into());
            }
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_ime_cancel(move || {
            let Some(editor) = weak.upgrade() else { return };
            editor.set_editor_ime_text("".into());
            editor.set_editor_ime_open(false);
        });
    }
    // ── 旧版兼容回调(离屏 example/过渡期;新渲染不使用)──
    editor.on_editor_recount(|_| {});
    editor.on_editor_count_matches(|_, _| 0);
    editor.on_editor_find_step(|_, _, _, _| EditorFindResult { start: 0, end: 0, count: 0 });
    editor.on_editor_replace_all(|_, _| {});
}

// ═══════════════════════════════════════════════════════════
// 编辑动作分发
// ═══════════════════════════════════════════════════════════
fn perform_key_action(editor: &EditorWindow, cmd: &str) {
    let tabs = tabs();
    let mut t = tabs.borrow_mut();
    let Some(doc) = t.active_doc_mut() else { return };
    if doc.readonly {
        return;
    }
    let cur = doc.cursor;
    match cmd {
        "enter" => {
            doc.cursor = doc.insert_text(cur, "\n");
        }
        "tab" => {
            doc.cursor = doc.insert_text(cur, "    ");
        }
        "backspace" => {
            if let Some(sel) = sel_span(doc) {
                doc.delete_range(sel.0, sel.1);
                doc.cursor = sel.0;
                doc.sel_anchor = None;
            } else {
                doc.cursor = doc.backspace(cur);
            }
        }
        "delete" => {
            if let Some(sel) = sel_span(doc) {
                doc.delete_range(sel.0, sel.1);
                doc.cursor = sel.0;
                doc.sel_anchor = None;
            } else {
                let end = doc.clamp_pos(Pos::new(cur.line, cur.col + 1));
                if end != cur {
                    doc.delete_range(cur, end);
                }
            }
        }
        "left" => {
            let target = if cur.col > 0 {
                Pos::new(cur.line, cur.col - 1)
            } else if cur.line > 0 {
                let len = doc.line_text(cur.line - 1).chars().count();
                Pos::new(cur.line - 1, len)
            } else {
                cur
            };
            move_cursor(doc, target);
        }
        "right" => {
            let line_len = doc.line_text(cur.line).chars().count();
            let target = if cur.col < line_len {
                Pos::new(cur.line, cur.col + 1)
            } else if cur.line + 1 < doc.len_lines() {
                Pos::new(cur.line + 1, 0)
            } else {
                cur
            };
            move_cursor(doc, target);
        }
        "up" => {
            move_cursor(doc, Pos::new(cur.line.saturating_sub(1), cur.col));
        }
        "down" => {
            move_cursor(doc, Pos::new(cur.line + 1, cur.col));
        }
        "home" => move_cursor(doc, Pos::new(cur.line, 0)),
        "end" => move_cursor(doc, Pos::new(cur.line, doc.line_text(cur.line).chars().count())),
        "page-up" => {
            let vp = editor.get_viewport_rows().max(1) as usize;
            move_cursor(doc, Pos::new(cur.line.saturating_sub(vp), cur.col));
        }
        "page-down" => {
            let vp = editor.get_viewport_rows().max(1) as usize;
            move_cursor(doc, Pos::new(cur.line + vp, cur.col));
        }
        "undo" => {
            doc.undo();
        }
        "redo" => {
            doc.redo();
        }
        "select-all" => {
            doc.sel_anchor = Some(Pos::new(0, 0));
            doc.cursor = Pos::new(doc.len_lines().saturating_sub(1), doc.line_text(doc.len_lines().saturating_sub(1)).chars().count());
        }
        "copy" => {
            if let Some((a, b)) = sel_span(doc) {
                let bytes = doc.pos_to_byte(a)..doc.pos_to_byte(b);
                let text = doc.rope.slice(bytes).to_string();
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(text);
                }
            }
        }
        "paste" => {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Ok(text) = cb.get_text() {
                    doc.cursor = doc.insert_text(doc.cursor, &text);
                }
            }
        }
        "find-toggle" => {
            editor.set_editor_find_open(!editor.get_editor_find_open());
        }
        "replace-one" => {
            // 替换单处:以当前命中为范围
            let q = editor.get_editor_find_query().to_string();
            let rep = editor.get_editor_replace_text().to_string();
            if let Some(h) = doc.find_hits.get(doc.find_cur.min(usize::MAX)) {
                let a = Pos::new(h.line, h.col);
                let b = Pos::new(h.line, h.col + h.len);
                doc.push_undo();
                doc.delete_range(a, b);
                doc.cursor = doc.insert_text(a, &rep);
                doc.dirty = true;
                run_find(doc, &q, true);
            }
        }
        "close" => {
            editor.set_editor_find_open(false);
            editor.set_ctx_open(false);
            editor.set_tabmenu_open(false);
            editor.set_dl_open(false);
            let _ = editor.hide();
        }
        _ => {}
    }
}

fn move_cursor(doc: &mut EditorDoc, target: Pos) {
    let target = doc.clamp_pos(target);
    if let Some(anchor) = doc.sel_anchor {
        let _ = anchor; // shift 移动时保持锚点(Slint 侧 shift 由 key-action 无差别,一期 shift 选区=点击 shift)
    }
    doc.cursor = target;
}

fn sel_span(doc: &EditorDoc) -> Option<(Pos, Pos)> {
    let a = doc.sel_anchor?;
    let c = doc.cursor;
    Some(if (a.line, a.col) <= (c.line, c.col) { (a, c) } else { (c, a) })
}

fn run_find(doc: &mut EditorDoc, query: &str, case: bool) {
    doc.find_hits = editor::find::find_all(doc, query, case);
    doc.find_cur = if doc.find_hits.is_empty() { usize::MAX } else { 0 };
}

fn push_find_state(editor: &EditorWindow, doc: &EditorDoc) {
    editor.set_editor_find_count(doc.find_hits.len().min(i32::MAX as usize) as i32);
    editor.set_editor_find_cur(if doc.find_cur == usize::MAX { 0 } else { (doc.find_cur + 1) as i32 });
    let lines: Vec<i32> = doc
        .find_hits
        .iter()
        .map(|h: &Hit| h.line as i32)
        .take(2000)
        .collect();
    editor.set_editor_find_hit_lines(ModelRc::new(VecModel::from(lines)));
}

fn set_status(editor: &EditorWindow, msg: &str) {
    editor.set_editor_status_msg(msg.into());
    STATUS_TIMER.with(|c| {
        let mut slot = c.borrow_mut();
        let timer = slot.get_or_insert_with(slint::Timer::default);
        timer.stop();
        timer.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(3000),
            {
                let weak = editor.as_weak();
                move || {
                    if let Some(editor) = weak.upgrade() {
                        editor.set_editor_status_msg("".into());
                    }
                }
            },
        );
    });
}

// ═══════════════════════════════════════════════════════════
// 窗口模型刷新(可见行 ± 缓冲;全文不过 Slint 边界)
// ═══════════════════════════════════════════════════════════
pub(crate) fn refresh(editor: &EditorWindow) {
    let tabs = tabs();
    let mut t = tabs.borrow_mut();
    // tab 条模型
    let tab_items: Vec<EditorTabInfo> = t
        .docs
        .iter()
        .enumerate()
        .map(|(i, d)| EditorTabInfo {
            id: d.tab_id.clone().into(),
            name: d.name.clone().into(),
            dirty: d.dirty,
            active: i == t.active,
            lang: d.lang.display().into(),
            letter: first_letter(&d.name).into(),
            icon_color: slint::Color::from_rgb_u8(0x7F, 0x84, 0x8B),
        })
        .collect();
    editor.set_editor_tabs(ModelRc::new(VecModel::from(tab_items)));

    let Some(doc) = t.active_doc_mut() else {
        editor.set_editor_lines(ModelRc::new(VecModel::from(Vec::<EditorLineRow>::new())));
        return;
    };
    // 光标可见性(垂直)
    let vp = editor.get_viewport_rows().max(1) as usize;
    let cur_line = doc.cursor.line;
    let first0 = doc.scroll_y / ROW_PX as usize;
    if cur_line < first0 {
        doc.scroll_y = cur_line * ROW_PX as usize;
    } else if cur_line >= first0 + vp {
        doc.scroll_y = ((cur_line + 1) * ROW_PX as usize).saturating_sub(vp * ROW_PX as usize);
    }
    let gutter_w = if doc.len_lines() >= 100_000 { 74 } else { 56 };
    editor.set_editor_gutter_w(gutter_w);
    let content_w = (doc.max_line_pixel_width(CHAR_PX) + CODE_PAD_PX * 2.0 + 20.0) as i32;
    editor.set_editor_content_w(content_w);
    editor.set_editor_content_h((doc.len_lines() as f64 * ROW_PX) as i32);
    editor.set_editor_scroll_y(doc.scroll_y as i32);
    editor.set_editor_name(doc.name.clone().into());
    editor.set_editor_path(doc.path.clone().into());
    editor.set_editor_encoding(doc.encoding.clone().into());
    editor.set_editor_lang_display(doc.lang.display().into());
    editor.set_editor_readonly(doc.readonly);
    editor.set_can_format(!doc.readonly && fmt_ext::can_format(doc.lang));
    editor.set_editor_badge(if doc.dirty { "修改".into() } else { "".into() });
    editor.set_editor_badge_kind(if doc.dirty { "dirty".into() } else { "none".into() });
    editor.set_editor_banner_visible(doc.big);
    if doc.big {
        editor.set_editor_banner_text(format!(
            "已按<b>只读模式</b>打开 · {:.1} MB · {} 行 · 高亮:行级规则(大文件模式)",
            doc.rope.len_bytes() as f64 / (1024.0 * 1024.0),
            doc.len_lines()
        ).into());
    }
    editor.set_editor_cursor_line((doc.cursor.line + 1) as i32);
    editor.set_editor_cursor_col((doc.cursor.col + 1) as i32);

    // 行窗口模型
    let first = doc.scroll_y / ROW_PX as usize;
    let frac = doc.scroll_y % ROW_PX as usize;
    let l0 = first.saturating_sub(1);
    let l1 = (first + vp + 3).min(doc.len_lines() - 1);
    editor.set_editor_first_row(l0 as i32);
    let spans_rows = editor::highlight_ts::highlight_visible(doc, l0, l1);
    let sel = sel_span(doc);
    let mut rows: Vec<EditorLineRow> = Vec::with_capacity(spans_rows.len());
    for (i, r) in (l0..=l1).enumerate() {
        let text = doc.line_text(r);
        let is_cur = doc.cursor.line == r;
        let mut spans_v: Vec<EditorSpan> = spans_rows
            .get(i)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|sp: editor::Span| {
                let seg: String = text
                    .chars()
                    .skip(sp.start)
                    .take(sp.len)
                    .collect();
                EditorSpan {
                    width: (chars_px_width(&seg)) as f32,
                    text: seg.into(),
                    start: sp.start as i32,
                    kind: span_kind_to_slint(sp.kind),
                }
            })
            .collect();
        // 查找命中注入(hl 叠加片段;当前命中 hl-cur)
        let mut spans_v: Vec<EditorSpan> = spans_v;
        for (hi, h) in doc.find_hits.iter().enumerate() {
            if h.line != r { continue; }
            let seg: String = text.chars().skip(h.col).take(h.len).collect();
            let kind = if hi == doc.find_cur { EditorSpanKind::HlCur } else { EditorSpanKind::Hl };
            let w = chars_px_width(&seg) as f32;
            spans_v.push(EditorSpan {
                text: seg.into(),
                start: h.col as i32,
                width: w,
                kind,
            });
        }
        let (sel_x, sel_w) = match sel {
            Some((a, b)) if r >= a.line && r <= b.line => {
                let s_col = if r == a.line { a.col } else { 0 };
                let e_col = if r == b.line { b.col } else { text.chars().count() };
                let x = px_of(&text, s_col);
                let w = px_of(&text, e_col) - x;
                (x as f32, w as f32)
            }
            _ => (0.0, 0.0),
        };
        rows.push(EditorLineRow {
            line: (r + 1) as i32,
            text: text.clone().into(),
            spans: ModelRc::new(VecModel::from(spans_v)),
            is_current: is_cur,
            cursor_x: (if is_cur { px_of(&text, doc.cursor.col) } else { 0.0 }) as f32,
            sel_x: sel_x as f32,
            sel_w: sel_w as f32,
            mark: doc.edit_marks.contains(&r),
            hit: doc.find_hits.iter().any(|h| h.line == r),
        });
    }
    editor.set_editor_lines(ModelRc::new(VecModel::from(rows)));
    editor.set_editor_scroll_frac((frac as f64 / ROW_PX) as i32);
}

fn px_of(text: &str, col: usize) -> f64 {
    let mut w = 0.0f64;
    for (i, c) in text.chars().enumerate() {
        if i >= col {
            break;
        }
        w += if unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) >= 2 { 2.0 } else { 1.0 };
    }
    w * CHAR_PX
}

fn chars_px_width(seg: &str) -> f64 {
    seg.chars()
        .map(|c| if unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) >= 2 { 2.0 } else { 1.0 })
        .sum::<f64>()
        * CHAR_PX
}

fn first_letter(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into())
}

fn span_kind_to_slint(k: SpanKind) -> EditorSpanKind {
    match k {
        SpanKind::Plain => EditorSpanKind::Plain,
        SpanKind::Kw => EditorSpanKind::Kw,
        SpanKind::Ty => EditorSpanKind::Ty,
        SpanKind::Fn => EditorSpanKind::Fn,
        SpanKind::Str => EditorSpanKind::Str,
        SpanKind::Num => EditorSpanKind::Num,
        SpanKind::Com => EditorSpanKind::Com,
        SpanKind::Ann => EditorSpanKind::Ann,
        SpanKind::Ts => EditorSpanKind::Ts,
        SpanKind::Thr => EditorSpanKind::Thr,
        SpanKind::LvlError => EditorSpanKind::LvlError,
        SpanKind::LvlWarn => EditorSpanKind::LvlWarn,
        SpanKind::LvlInfo => EditorSpanKind::LvlInfo,
        SpanKind::LvlDebug => EditorSpanKind::LvlDebug,
        SpanKind::Exc => EditorSpanKind::Exc,
        SpanKind::At => EditorSpanKind::At,
        SpanKind::Cs => EditorSpanKind::Cs,
        SpanKind::Hl => EditorSpanKind::Hl,
        SpanKind::HlCur => EditorSpanKind::HlCur,
    }
}

fn default_download_dir() -> String {
    directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_else(|| std::env::temp_dir().to_string_lossy().to_string())
}
