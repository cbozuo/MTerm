// (#editor-native 2026-11-16) 宿主桥:EditorWindow(独立窗)↔ src/editor 领域层。
//
// 文本权威在 Slint 侧 TextInput —— shaping、光标、选择、剪贴板、IME、撤销全部原生:
//   · 打开 / 切 tab   → 把该 tab 的正文下发到 editor-content
//   · 用户编辑        → 只标脏(Slint 侧自发 120ms 防抖高亮请求)
//   · 保存/查找/格式化/替换 → 从 editor-content 读当前正文
//   · 切走 / 关闭     → 先把正文读回 tab 暂存(stash),避免丢改动
//
// 高亮:Rust 产出「全文 UTF-8 字节区间」,读 Theme.ansi16 编码成
// `start:end:AARRGGBB;` 交给 TextInput.syntax-spans,由 Slint 在 shaping 层叠色。
// 行号:gutter 是一块只读 TextInput + line-number-width,由 Slint 自己画。
use crate::editor::find as find_ext;
use crate::editor::format as fmt_ext;
use crate::editor::{self, EditorTabs, GSpan, SpanKind};
use crate::sftp::DownloadConflict;
use crate::sftp::SftpHandles;
use crate::ui::{EditorFindResult, EditorTabInfo, EditorWindow};
use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, Model as _, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

pub(crate) type SharedTabs = Rc<RefCell<EditorTabs>>;

/// 行号位数决定 gutter 宽度(与 hifi:56 / ≥10 万行 74)
const WIDE_GUTTER_LINES: usize = 100_000;
const GUTTER_W: i32 = 56;
const GUTTER_W_WIDE: i32 = 74;
/// 滚动条命中标记上限(再多没有可视意义,也拖慢模型下发)
const FIND_MARK_CAP: usize = 400;
/// 只读大文件的提示阈值
const BIG_READONLY_BYTES: usize = 1024 * 1024;
/// 着色内联上限:再大就丢后台线程(见 request_highlight)
const HIGHLIGHT_INLINE_BYTES: usize = 256 * 1024;

thread_local! {
    static LAST_DL_DIR: RefCell<String> = RefCell::new(String::new());
    static TABS: RefCell<Option<SharedTabs>> = const { RefCell::new(None) };
}

pub(crate) fn tabs() -> SharedTabs {
    TABS.with(|c| c.borrow().as_ref().expect("editor tabs 未初始化(editor_ui::wire)").clone())
}

fn last_dl_dir() -> String {
    LAST_DL_DIR.with(|c| {
        if c.borrow().is_empty() {
            *c.borrow_mut() = default_download_dir();
        }
        c.borrow().clone()
    })
}
fn set_last_dl_dir(v: &str) {
    LAST_DL_DIR.with(|c| *c.borrow_mut() = v.to_string());
}
fn default_download_dir() -> String {
    directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_else(|| std::env::temp_dir().to_string_lossy().to_string())
}

// ═══════════════════════════════════════════════════════════
// 高亮:字节区间 → `start:end:AARRGGBB;`
// ═══════════════════════════════════════════════════════════
/// 语法类别 → 主题 ANSI16 槽位(色值对照见设计稿「语法色映射表」)。
/// Plain / 查找命中不需要着色(查找是原生选区),返回 None 即不产出。
fn kind_ansi_slot(k: SpanKind) -> Option<usize> {
    use SpanKind::*;
    Some(match k {
        Plain | Hl | HlCur => return None,
        Kw => 5,
        Ty => 11,
        Fn => 12,
        Str => 10,
        Num => 3,
        Com => 8,
        Ann => 14,
        Ts => 14,
        Thr => 12,
        LvlError => 9,
        LvlWarn => 11,
        LvlInfo => 10,
        LvlDebug => 8,
        Exc => 11,
        At => 8,
        Cs => 9,
    })
}

/// 把全文着色段编码成渲染侧约定的线格式。越界/非字符边界/乱序重叠一律跳过
/// (渲染侧也会校验,这里提前挡掉省一次着色)。
fn encode_spans(editor: &EditorWindow, text: &str, spans: &[GSpan]) -> String {
    if spans.is_empty() {
        return String::new();
    }
    let ansi: Vec<slint::Color> =
        editor.global::<crate::ui::Theme<'_>>().get_ansi16().iter().collect();
    let mut out = String::with_capacity(spans.len() * 22);
    let mut prev_end = 0usize;
    for sp in spans {
        let Some(slot) = kind_ansi_slot(sp.kind) else { continue };
        let Some(color) = ansi.get(slot) else { continue };
        let (s, e) = (sp.start, sp.end);
        if s < prev_end
            || e <= s
            || e > text.len()
            || !text.is_char_boundary(s)
            || !text.is_char_boundary(e)
        {
            continue;
        }
        prev_end = e;
        let _ = write!(out, "{}:{}:{:08x};", s, e, color.as_argb_encoded());
    }
    out
}

/// 着色结果 → 窗口属性。source 与 spans 必须同帧下发 —— 渲染侧只在
/// `syntax-source == 显示文本` 时才着色(防串色,也天然挡掉过期结果)。
///
/// 顺带下发两个「随正文变化」的行几何量(此处 text 已在手,免得别处再拷一份全文):
/// 行号槽宽度(行数位数)与总行数(滚动条命中标记的分数分母)。
fn apply_highlight(editor: &EditorWindow, text: &str, spans: Vec<GSpan>) {
    // 过期结果直接丢弃:后台线程算完时用户可能已继续打字或切了 tab。
    // 渲染侧本来也不会给不符的 source 着色,但 total-lines / 行号槽宽度
    // 会被这份旧正文写脏(滚动条标记分母错位),所以在这里就拦住。
    if editor.get_editor_content().as_str() != text {
        return;
    }
    let lines = text.lines().count().max(1);
    editor.set_editor_gutter_w(if lines >= WIDE_GUTTER_LINES { GUTTER_W_WIDE } else { GUTTER_W });
    editor.set_editor_total_lines(lines as i32);
    editor.set_editor_syntax_source(text.into());
    editor.set_editor_syntax_spans(encode_spans(editor, text, &spans).into());
}

/// 同步着色(打开 / 切 tab 的兜底路径;一致走 request_highlight 分流)。
fn refresh_highlight_with(editor: &EditorWindow, text: &str) {
    request_highlight(editor, text.into());
}

/// 打字防抖后的着色请求。正文超过 `HIGHLIGHT_INLINE_BYTES` 就丢后台线程 ——
/// 整篇扫描(≥2MB 时 0.2–1s)会阻塞 UI 线程,而这里每次停顿后都会被调用。
/// 结果回 UI 线程下发:编码要读 Theme.ansi16(只能在 UI 线程),且视作废由
/// 渲染侧的 `source == 文本` 判定兜住(用户已继续打字 → 旧结果自然不着色)。
fn request_highlight(editor: &EditorWindow, text: SharedString) {
    let lang = match tabs().borrow().active_doc().map(|d| d.lang) {
        Some(l) => l,
        None => return,
    };
    if text.len() <= HIGHLIGHT_INLINE_BYTES {
        let spans = editor::highlight_ts::highlight_all(text.as_str(), lang);
        apply_highlight(editor, text.as_str(), spans);
        return;
    }
    // SharedString 是 Arc 句柄(Send):直接搬进线程,不拷贝正文
    let weak = editor.as_weak();
    std::thread::spawn(move || {
        let spans = editor::highlight_ts::highlight_all(text.as_str(), lang);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(editor) = weak.upgrade() {
                apply_highlight(&editor, text.as_str(), spans);
            }
        });
    });
}

/// 重算查找命中并下发:总数、当前序号、滚动条标记行。
fn refresh_find(editor: &EditorWindow, keep_cur: bool) {
    let text = editor.get_editor_content().to_string();
    let query = editor.get_editor_find_query().to_string();
    let case = tabs().borrow().case_sensitive;
    let hits = find_ext::find_all(&text, &query, case);
    editor.set_editor_find_count(hits.len() as i32);
    if !keep_cur {
        editor.set_editor_find_cur(if hits.is_empty() { 0 } else { 1 });
    } else {
        let cur = editor.get_editor_find_cur();
        editor.set_editor_find_cur(cur.min(hits.len() as i32));
    }
    let marks: Vec<i32> = hits
        .iter()
        .take(FIND_MARK_CAP)
        .map(|h| h.line as i32)
        .collect();
    editor.set_editor_find_hit_lines(ModelRc::new(VecModel::from(marks)));
}

// ═══════════════════════════════════════════════════════════
// 下发:tab 栏 / 状态 / 全文
// ═══════════════════════════════════════════════════════════
fn first_letter(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into())
}

fn push_tabs(editor: &EditorWindow) {
    let t = tabs();
    let t = t.borrow();
    let items: Vec<EditorTabInfo> = t
        .docs
        .iter()
        .enumerate()
        .map(|(i, d)| EditorTabInfo {
            id: d.id.clone().into(),
            name: d.name.clone().into(),
            dirty: d.dirty,
            active: i == t.active,
            lang: d.lang.display().into(),
            letter: first_letter(&d.name).into(),
            icon_color: slint::Color::default(),
        })
        .collect();
    editor.set_editor_tabs(ModelRc::new(VecModel::from(items)));
}

/// 把活动 tab 的正文读回暂存(切 tab / 关闭 / 保存前都要做,否则丢改动)。
fn stash_active(editor: &EditorWindow) {
    let text = editor.get_editor_content().to_string();
    let tabs = tabs();
    let mut t = tabs.borrow_mut();
    if let Some(d) = t.active_doc_mut() {
        d.text = text;
    }
}

/// 把活动 tab 的正文下发给 Slint(切 tab / 打开后)。
fn load_active(editor: &EditorWindow) {
    // SharedString 只造一次:正文下发与着色共用同一个句柄(大文本不重复拷贝)
    let text: SharedString = {
        let tabs = tabs();
        let t = tabs.borrow();
        match t.active_doc() {
            Some(d) => d.text.as_str().into(),
            None => SharedString::default(),
        }
    };
    editor.set_editor_content(text.clone());
    request_highlight(editor, text);
}

/// 元数据(不碰正文、不读正文 —— 击键路径上会调它,任何全文拷贝都是白费)。
/// 行号槽宽度与总行数不在这里:它们随正文变化,由高亮那趟(text 在手)下发。
fn push_meta(editor: &EditorWindow) {
    let t = tabs();
    let t = t.borrow();
    let Some(d) = t.active_doc() else {
        return;
    };
    editor.set_editor_name(d.name.clone().into());
    editor.set_editor_path(d.path.clone().into());
    editor.set_editor_encoding(d.encoding.clone().into());
    editor.set_editor_lang_display(d.lang.display().into());
    editor.set_editor_readonly(d.readonly);
    editor.set_can_format(!d.readonly && fmt_ext::can_format(d.lang));
    editor.set_editor_badge(if d.dirty { "修改".into() } else { "".into() });
    editor.set_editor_badge_kind(if d.dirty { "dirty".into() } else { "none".into() });
    // 大文件只读提示条(只读且超阈值;`big` 档位在 TabDoc 里已置 readonly)
    let sized_big = d.readonly && d.text.len() > BIG_READONLY_BYTES;
    editor.set_editor_banner_visible(sized_big);
    if sized_big {
        editor.set_editor_banner_text(
            format!(
                "{} · 只读模式({:.1} MB)",
                if d.big { "文件过大" } else { "大文件" },
                d.text.len() as f64 / 1048576.0
            )
            .into(),
        );
    }
    drop(t);
    push_tabs(editor);
}

/// 重编码语法色 + 刷新元数据。**不动正文** —— 字体/主题/缩放变化走这里;
/// 若在此下发 tab 暂存正文,会把 Slint 里的实时正文覆盖掉(丢未保存改动)。
pub(crate) fn refresh(editor: &EditorWindow) {
    // SharedString 是引用计数句柄,这里不产生全文拷贝;大文本的分流在
    // request_highlight 里(≤256KB 内联,再大丢后台线程)。
    request_highlight(editor, editor.get_editor_content());
    push_meta(editor);
}

/// 切 tab / 打开 / 关闭后的整篇重载:下发该 tab 的正文 + 高亮 + 元数据 + 查找态。
fn reload_active(editor: &EditorWindow) {
    load_active(editor);
    push_meta(editor);
    refresh_find(editor, false);
}

fn set_status(editor: &EditorWindow, msg: &str) {
    editor.set_editor_status_msg(msg.into());
}

/// 行号(1 基)文本 → 起始字节偏移。超范围钳到末行。
fn line_start_byte(text: &str, line1: usize) -> usize {
    if line1 <= 1 {
        return 0;
    }
    let mut n = 1usize;
    for (i, c) in text.char_indices() {
        if c == '\n' {
            n += 1;
            if n == line1 {
                return i + 1;
            }
        }
    }
    text.len()
}

// ═══════════════════════════════════════════════════════════
// 打开 / 关闭
// ═══════════════════════════════════════════════════════════
/// 编辑器窗口关闭(hide)前的宿主收尾:把实时正文 stash 回 tab(否则丢改动)、
/// 收起所有浮层。由 app.rs 的 on_close_editor 调用(见 wire 里的说明)。
pub(crate) fn on_editor_closed(editor: &EditorWindow) {
    stash_active(editor);
    editor.set_editor_find_open(false);
    editor.set_ctx_open(false);
    editor.set_tabmenu_open(false);
    editor.set_goto_open(false);
    editor.set_dl_open(false);
}

pub(crate) fn open_remote_file(
    editor: &EditorWindow,
    text: &str,
    name: &str,
    path: &str,
    session_tab_id: &str,
    encoding: &str,
    force_readonly: bool,
) {
    // 先把当前 tab 的实时正文存回(带着未保存改动切走会丢)
    stash_active(editor);
    {
        let tabs = tabs();
        let mut t = tabs.borrow_mut();
        match t.index_of_path(path, session_tab_id) {
            Some(i) => t.activate(i),
            None => {
                let doc = crate::editor::TabDoc::new(
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
    }
    reload_active(editor);
    let _ = editor.show();
    super::window::arm_window_icon(editor.window());
    editor.window().with_winit_window(|ww| ww.focus_window());
}

// ═══════════════════════════════════════════════════════════
// 回调注册(在 wire_sftp_callbacks 末尾调用,sftp_handles 可用)
// ═══════════════════════════════════════════════════════════
pub(crate) fn wire(editor: &EditorWindow, sftp_handles: SftpHandles) {
    TABS.with(|c| {
        if c.borrow().is_none() {
            *c.borrow_mut() = Some(SharedTabs::new(RefCell::new(EditorTabs::new())));
        }
    });

    // ── PickerMath 接线(编辑器窗口是独立实例)──
    // ui/color_picker.slint 的 PickerMath 是**按窗口实例化**的全局,纯回调在 Rust 侧
    // 注册;主窗在 open_window 里接了线,子窗口各自的实例默认没有实现 —— 不接的话
    // `ink-on()` 返回 Color::default()(全透明),标题栏「保存」按钮与查找浮层
    // 「替换全部」的字会**看不见**(按钮只剩一块底色)。
    {
        let pm = editor.global::<crate::ui::PickerMath<'_>>();
        pm.on_ink_on(move |hex: SharedString| {
            let ink = crate::picker::ink_on(crate::picker::parse_hex(&hex).unwrap_or(0));
            slint::Color::from_rgb_u8(
                ((ink >> 16) & 0xFF) as u8,
                ((ink >> 8) & 0xFF) as u8,
                (ink & 0xFF) as u8,
            )
        });
    }

    // ── 用户编辑:标脏(正文本身已在 TextInput 里,不必回写)──
    // **击键路径不许有 O(n) 工作**:旧写法在这里调 push_meta → 里面把整篇正文
    // `to_string()` 出来数行(1MB ≈ 0.5–1ms,16MB ≈ 10ms,等于把流畅度还回去)。
    // 现在只在「脏标记首次翻转」时推一次 tab 模型(dirty 点),其余什么也不做;
    // 行数/行号槽宽度由 120ms 防抖后的高亮那趟顺手下发。
    {
        let weak = editor.as_weak();
        editor.on_content_changed(move || {
            let Some(editor) = weak.upgrade() else { return };
            let first_edit = {
                let tabs = tabs();
                let mut t = tabs.borrow_mut();
                match t.active_doc_mut() {
                    Some(d) if !d.dirty => {
                        d.dirty = true;
                        true
                    }
                    _ => false,
                }
            };
            if first_edit {
                push_tabs(&editor);
            }
        });
    }

    // ── 防抖高亮:正文 → spans(Slint 侧 120ms Timer 请求)──
    {
        let weak = editor.as_weak();
        editor.on_editor_highlight(move |text: SharedString| {
            let Some(editor) = weak.upgrade() else { return };
            request_highlight(&editor, text);
        });
    }

    // ── 光标/选区变化 → 状态栏 Ln/Col + 选区统计 ──
    {
        let weak = editor.as_weak();
        editor.on_cursor_moved(move |caret: i32, anchor: i32| {
            let Some(editor) = weak.upgrade() else { return };
            let text = editor.get_editor_content().to_string();
            let caret = caret.max(0) as usize;
            let anchor = anchor.max(0) as usize;
            let (line, col) = find_ext::line_col(&text, caret);
            editor.set_editor_cursor_line(line as i32 + 1);
            editor.set_editor_cursor_col(col as i32 + 1);
            if caret == anchor {
                editor.set_editor_sel_lines(0);
                editor.set_editor_sel_chars(0);
            } else {
                let (lo, hi) = if caret < anchor { (caret, anchor) } else { (anchor, caret) };
                let (l0, _) = find_ext::line_col(&text, lo);
                let (l1, _) = find_ext::line_col(&text, hi);
                let chars = text[lo.min(text.len())..hi.min(text.len())].chars().count();
                editor.set_editor_sel_lines(l1.saturating_sub(l0) as i32);
                editor.set_editor_sel_chars(chars as i32);
            }
        });
    }

    // ── 查找:计数 / 跳转 ──
    {
        let weak = editor.as_weak();
        editor.on_editor_count_matches(move |text: SharedString, query: SharedString| {
            let _ = weak.upgrade();
            let case = tabs().borrow().case_sensitive;
            find_ext::find_all(text.as_str(), query.as_str(), case).len() as i32
        });
    }
    {
        // 查询词/大小写变化:重算总数与标记(不移动光标)
        let weak = editor.as_weak();
        editor.on_find_edited(move |case: bool| {
            let Some(editor) = weak.upgrade() else { return };
            tabs().borrow_mut().case_sensitive = case;
            refresh_find(&editor, false);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_editor_find_step(move |text: SharedString, query: SharedString, from: i32, forward: bool| {
            let _ = weak.upgrade();
            let case = tabs().borrow().case_sensitive;
            let hits = find_ext::find_all(text.as_str(), query.as_str(), case);
            if hits.is_empty() {
                return EditorFindResult { start: 0, end: 0, count: 0, index: 0 };
            }
            let from = from.max(0) as usize;
            let idx = if forward {
                hits.iter().position(|h| h.start > from).unwrap_or(0)
            } else {
                hits.iter().rposition(|h| h.start < from).unwrap_or(hits.len() - 1)
            };
            let h = &hits[idx];
            EditorFindResult {
                start: h.start as i32,
                end: h.end as i32,
                count: hits.len() as i32,
                index: idx as i32 + 1,
            }
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_find_close(move || {
            let Some(editor) = weak.upgrade() else { return };
            editor.set_editor_find_open(false);
            editor.set_editor_find_query("".into());
            editor.set_editor_find_count(0);
            editor.set_editor_find_cur(0);
            editor.set_editor_find_hit_lines(ModelRc::new(VecModel::from(Vec::<i32>::new())));
        });
    }

    // ── 替换:单个 / 全部 ──
    {
        let weak = editor.as_weak();
        editor.on_replace_one(move |sel_start: i32, sel_end: i32| {
            let Some(editor) = weak.upgrade() else { return };
            if editor.get_editor_readonly() {
                return;
            }
            let query = editor.get_editor_find_query().to_string();
            let rep = editor.get_editor_replace_text().to_string();
            if query.is_empty() {
                return;
            }
            let mut text = editor.get_editor_content().to_string();
            let case = tabs().borrow().case_sensitive;
            let (lo, hi) = (
                sel_start.max(0) as usize,
                sel_end.max(0) as usize,
            );
            let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
            // 只有「当前选区正好是查询词」才替换(否则先跳到命中,交给下一次点击)
            let hit = hi > lo
                && hi <= text.len()
                && text.is_char_boundary(lo)
                && text.is_char_boundary(hi)
                && matches_hit(&text[lo..hi], &query, case);
            if hit {
                text.replace_range(lo..hi, &rep);
                editor.set_editor_content(text.as_str().into());
                mark_dirty(&editor);
                // 选区落到替换文本之后
                let after = lo + rep.len();
                editor.set_sel_request_start(after as i32);
                editor.set_sel_request_end(after as i32);
                let tok = editor.get_sel_request_token() + 1;
                editor.set_sel_request_token(tok);
                refresh_highlight_with(&editor, &text);
            } else {
                let hits = find_ext::find_all(&text, &query, case);
                if hits.is_empty() {
                    set_status(&editor, "无命中");
                } else {
                    set_status(&editor, "已跳到命中 · 再次点击「替换」执行替换");
                }
            }
            refresh_find(&editor, false);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_replace_all(move || {
            let Some(editor) = weak.upgrade() else { return };
            if editor.get_editor_readonly() {
                return;
            }
            let query = editor.get_editor_find_query().to_string();
            let rep = editor.get_editor_replace_text().to_string();
            if query.is_empty() {
                return;
            }
            let text = editor.get_editor_content().to_string();
            let case = tabs().borrow().case_sensitive;
            let (new_text, n) = replace_all_with(&text, &query, &rep, case);
            if n == 0 {
                set_status(&editor, "无命中，未替换");
                return;
            }
            editor.set_editor_content(new_text.as_str().into());
            mark_dirty(&editor);
            refresh_highlight_with(&editor, &new_text);
            refresh_find(&editor, false);
            set_status(&editor, &format!("已替换 {n} 处"));
        });
    }

    // ── 格式化 ──
    {
        let weak = editor.as_weak();
        editor.on_format_doc(move || {
            let Some(editor) = weak.upgrade() else { return };
            if editor.get_editor_readonly() {
                set_status(&editor, "只读文件不可格式化");
                return;
            }
            let lang = tabs().borrow().active_doc().map(|d| d.lang);
            let Some(lang) = lang else { return };
            if !fmt_ext::can_format(lang) {
                set_status(&editor, "该语言不支持格式化");
                return;
            }
            let text = editor.get_editor_content().to_string();
            match fmt_ext::format_doc(lang, &text) {
                Ok(Some(new_text)) => {
                    editor.set_editor_content(new_text.as_str().into());
                    mark_dirty(&editor);
                    refresh_highlight_with(&editor, &new_text);
                    refresh_find(&editor, false);
                    // 光标回文首(整体重排后旧偏移无意义)
                    editor.set_sel_request_start(0);
                    editor.set_sel_request_end(0);
                    let tok = editor.get_sel_request_token() + 1;
                    editor.set_sel_request_token(tok);
                    set_status(&editor, "已格式化（Ctrl+Z 可撤销）");
                }
                Ok(None) => set_status(&editor, "该语言不支持格式化"),
                Err(msg) => set_status(&editor, &msg),
            }
        });
    }

    // ── 转到行 ──
    {
        let weak = editor.as_weak();
        editor.on_goto_go(move |line_text: SharedString| {
            let Some(editor) = weak.upgrade() else { return };
            let Ok(n) = line_text.trim().parse::<usize>() else {
                set_status(&editor, "请输入行号");
                return;
            };
            let text = editor.get_editor_content().to_string();
            let total = text.lines().count().max(1);
            let line = n.clamp(1, total);
            let byte = line_start_byte(&text, line);
            editor.set_goto_open(false);
            editor.set_sel_request_start(byte as i32);
            editor.set_sel_request_end(byte as i32);
            let tok = editor.get_sel_request_token() + 1;
            editor.set_sel_request_token(tok);
            set_status(&editor, &format!("已转到第 {line} 行"));
        });
    }

    // ── 保存(当前 tab 回传远端)──
    {
        let handles = sftp_handles.clone();
        let weak = editor.as_weak();
        editor.on_save_file(move || {
            let Some(editor) = weak.upgrade() else { return };
            if editor.get_editor_readonly() {
                set_status(&editor, "只读文件不可保存");
                return;
            }
            let content = editor.get_editor_content().to_string();
            let (path, session, dirty) = {
                let tabs = tabs();
                let t = tabs.borrow();
                match t.active_doc() {
                    Some(d) => (d.path.clone(), d.session_tab_id.clone(), d.dirty),
                    None => return,
                }
            };
            if !dirty {
                set_status(&editor, "没有需要保存的改动");
                return;
            }
            let ok = if let Ok(handles) = handles.lock() {
                if let Some(h) = handles.get(&session) {
                    h.write_text(path, content.clone());
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if ok {
                {
                    let tabs = tabs();
                    let mut t = tabs.borrow_mut();
                    if let Some(d) = t.active_doc_mut() {
                        d.text = content;
                        d.dirty = false;
                    }
                }
                push_meta(&editor);
                set_status(&editor, "已保存");
            } else {
                set_status(&editor, "保存失败：连接不可用");
            }
        });
    }

    // ── 关闭窗口(仅隐藏;tabs 保留,重开同文件恢复)──
    // 这里不注册 on_close_editor:AppWindow 侧也注册了同一个回调(要同步主窗的
    // editor-open 标志),而 Slint 的「后注册覆盖先注册」会吃掉本处逻辑 —— 改由
    // app.rs 的处理器调用 on_editor_closed(),其中包含 stash(否则丢改动)。

    // ── tab 操作 ──
    {
        let weak = editor.as_weak();
        editor.on_tab_activate(move |idx| {
            let Some(editor) = weak.upgrade() else { return };
            stash_active(&editor);           // 切走前先存回
            tabs().borrow_mut().activate(idx.max(0) as usize);
            reload_active(&editor);
        });
    }
    {
        let weak = editor.as_weak();
        editor.on_tab_close(move |idx| {
            let Some(editor) = weak.upgrade() else { return };
            stash_active(&editor);
            let tabs = tabs();
            request_close_tab(&editor, &tabs, idx.max(0) as usize, "close");
        });
    }
    {
        let weak = editor.as_weak();
        let handles = sftp_handles.clone();
        editor.on_tab_menu(move |cmd: SharedString, idx: i32| {
            let Some(editor) = weak.upgrade() else { return };
            editor.set_tabmenu_open(false);
            stash_active(&editor);
            let idx = idx.max(0) as usize;
            let tabs = tabs();
            match cmd.as_str() {
                "close" => request_close_tab(&editor, &tabs, idx, "close"),
                "close-others" => request_close_tab(&editor, &tabs, idx, "close-others"),
                "close-right" => request_close_tab(&editor, &tabs, idx, "close-right"),
                "close-all" => request_close_tab(&editor, &tabs, 0, "close-all"),
                "download" => {
                    if let Some(d) = tabs.borrow().docs.get(idx) {
                        editor.set_editor_download_name(d.name.clone().into());
                        editor.set_editor_download_info(
                            format!("{} · {}", d.name, d.path).into(),
                        );
                        editor.set_editor_download_dir(last_dl_dir().into());
                        editor.set_dl_open(true);
                    }
                }
                "show-panel" => {
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
            if !matches!(cmd.as_str(), "close" | "close-others" | "close-right" | "close-all") {
                refresh(&editor);
            }
            if tabs.borrow().is_empty() {
                editor.set_editor_find_open(false);
                editor.set_ctx_open(false);
                editor.set_tabmenu_open(false);
                editor.set_dl_open(false);
                let _ = editor.hide();
            }
        });
    }

    // ── 编辑区右键菜单:宿主侧命令(编辑类由 Slint 直接调 TextInput)──
    {
        let weak = editor.as_weak();
        editor.on_ctx_action(move |cmd: SharedString| {
            let Some(editor) = weak.upgrade() else { return };
            match cmd.as_str() {
                "format" => editor.invoke_format_doc(),
                "save" => editor.invoke_save_file(),
                "close" => editor.invoke_close_editor(),
                "sysopen" => {
                    // 「用系统默认应用打开」需要先把远端文件落到本地,走下载弹窗更诚实
                    let name = editor.get_editor_name().to_string();
                    let path = editor.get_editor_path().to_string();
                    editor.set_editor_download_name(name.clone().into());
                    editor.set_editor_download_info(format!("{name} · {path}").into());
                    editor.set_editor_download_dir(last_dl_dir().into());
                    editor.set_dl_open(true);
                }
                _ => {}
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
                let Some(dir) = rfd::FileDialog::new().set_title("选择下载目录").pick_folder()
                else {
                    return;
                };
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(editor) = weak2.upgrade() {
                        let dv = dir.to_string_lossy().to_string();
                        set_last_dl_dir(&dv);
                        editor.set_editor_download_dir(dv.into());
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
            let dir = editor.get_editor_download_dir().to_string();
            if dir.is_empty() {
                return;
            }
            set_last_dl_dir(&dir);
            let info = {
                let tabs = tabs();
                let t = tabs.borrow();
                t.active_doc().map(|d| (d.session_tab_id.clone(), d.path.clone()))
            };
            if let Some((session, path)) = info {
                if let Ok(handles) = handles.lock() {
                    if let Some(h) = handles.get(&session) {
                        h.download(path, dir, DownloadConflict::Replace);
                        set_status(&editor, "已加入下载队列（后台传输）");
                    }
                }
            }
        });
    }

    // 丢弃未保存确认(数据安全)
    wire_close_confirm(editor, sftp_handles.clone());
}

// ═══════════════════════════════════════════════════════════
// 小工具
// ═══════════════════════════════════════════════════════════
fn mark_dirty(editor: &EditorWindow) {
    {
        let tabs = tabs();
        let mut t = tabs.borrow_mut();
        if let Some(d) = t.active_doc_mut() {
            d.dirty = true;
        }
    }
    push_meta(editor);
}

/// 忽略大小写比较(按字符,避免 Unicode 小写化改变字节长度)
fn matches_hit(hay: &str, query: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        return hay == query;
    }
    hay.chars().flat_map(|c| c.to_lowercase()).eq(query.chars().flat_map(|c| c.to_lowercase()))
}

/// 全部替换:返回(新正文, 替换处数)。直接按命中区间重建,避免 replace 的转义陷阱。
fn replace_all_with(text: &str, query: &str, rep: &str, case_sensitive: bool) -> (String, usize) {
    let hits = find_ext::find_all(text, query, case_sensitive);
    if hits.is_empty() {
        return (text.to_string(), 0);
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for h in &hits {
        out.push_str(&text[last..h.start]);
        out.push_str(rep);
        last = h.end;
    }
    out.push_str(&text[last..]);
    (out, hits.len())
}

// ═══════════════════════════════════════════════════════════
// tab 关闭确认链(dirty 销毁路径;窗口 hide 不销毁 tabs 无需确认)
// ═══════════════════════════════════════════════════════════
fn request_close_tab(editor: &EditorWindow, tabs: &SharedTabs, idx: usize, cmd: &str) {
    let t = tabs.borrow();
    let dirty_hit = match cmd {
        "close" => t.docs.get(idx).map(|d| d.dirty).unwrap_or(false),
        "close-others" => t.docs.iter().enumerate().any(|(i, d)| i != idx && d.dirty),
        "close-right" => t.docs.iter().enumerate().any(|(i, d)| i > idx && d.dirty),
        _ => t.docs.iter().any(|d| d.dirty),
    };
    drop(t);
    if dirty_hit {
        editor.set_close_confirm_open(true);
        editor.set_close_confirm_idx(idx as i32);
        editor.set_close_confirm_cmd(cmd.into());
        editor.set_close_confirm_text(
            match cmd {
                "close" => "该文件",
                "close-others" => "其他",
                "close-right" => "右侧",
                _ => "",
            }
            .into(),
        );
        return;
    }
    do_close(editor, tabs, idx, cmd);
}

fn do_close(editor: &EditorWindow, tabs: &SharedTabs, idx: usize, cmd: &str) {
    {
        let mut t = tabs.borrow_mut();
        match cmd {
            "close" => {
                let _ = t.close(idx);
            }
            "close-others" => {
                let _ = t.close_others(idx);
            }
            "close-right" => {
                let _ = t.close_right(idx);
            }
            _ => {
                let _ = t.close_all();
            }
        }
    }
    // 关闭会改变活动 tab(甚至清空),必须整篇重载而不是只重编码语法色
    if tabs.borrow().docs.is_empty() {
        editor.set_editor_content("".into());
        editor.set_editor_syntax_source("".into());
        editor.set_editor_syntax_spans("".into());
        editor.set_editor_tabs(ModelRc::new(VecModel::from(Vec::<EditorTabInfo>::new())));
    } else {
        reload_active(editor);
    }
}

fn wire_close_confirm(editor: &EditorWindow, sftp_handles: SftpHandles) {
    // 丢弃并关闭
    {
        let weak = editor.as_weak();
        editor.on_close_confirm_discard(move || {
            let Some(editor) = weak.upgrade() else { return };
            let idx = editor.get_close_confirm_idx().max(0) as usize;
            let cmd = editor.get_close_confirm_cmd().to_string();
            editor.set_close_confirm_open(false);
            let tabs = tabs();
            do_close(&editor, &tabs, idx, &cmd);
            set_status(&editor, "已丢弃改动并关闭");
            if tabs.borrow().is_empty() {
                let _ = editor.hide();
            }
        });
    }
    // 保存后关闭
    {
        let handles = sftp_handles;
        let weak = editor.as_weak();
        editor.on_close_confirm_save(move || {
            let Some(editor) = weak.upgrade() else { return };
            let idx = editor.get_close_confirm_idx().max(0) as usize;
            let cmd = editor.get_close_confirm_cmd().to_string();
            editor.set_close_confirm_open(false);
            stash_active(&editor); // 活动 tab 的实时正文先落回暂存
            // 收集所有待保存的 (session, path, text);先取完再释放借用
            let jobs: Vec<(String, String, String)> = {
                let tabs = tabs();
                let t = tabs.borrow();
                t.docs
                    .iter()
                    .filter(|d| d.dirty)
                    .map(|d| (d.session_tab_id.clone(), d.path.clone(), d.text.clone()))
                    .collect()
            };
            if let Ok(handles) = handles.lock() {
                for (session, path, body) in &jobs {
                    if let Some(h) = handles.get(session) {
                        h.write_text(path.clone(), body.clone());
                    }
                }
            }
            {
                let tabs = tabs();
                let mut t = tabs.borrow_mut();
                for d in t.docs.iter_mut() {
                    d.dirty = false;
                }
            }
            let tabs = tabs();
            do_close(&editor, &tabs, idx, &cmd);
            set_status(&editor, "已保存并关闭");
            if tabs.borrow().is_empty() {
                let _ = editor.hide();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_all_uses_byte_ranges() {
        let (out, n) = replace_all_with("超时 = 30\n超时 = 60\n", "超时", "timeout", true);
        assert_eq!(n, 2);
        assert_eq!(out, "timeout = 30\ntimeout = 60\n");
    }

    #[test]
    fn replace_all_case_insensitive() {
        let (out, n) = replace_all_with("Ab ab AB", "ab", "x", false);
        assert_eq!(n, 3);
        assert_eq!(out, "x x x");
    }

    #[test]
    fn line_start_byte_counts_lines() {
        let t = "a\nbb\nccc\n";
        assert_eq!(line_start_byte(t, 1), 0);
        assert_eq!(line_start_byte(t, 2), 2);
        assert_eq!(line_start_byte(t, 3), 5);
        // 超范围钳到末尾
        assert_eq!(line_start_byte(t, 99), t.len());
    }

    #[test]
    fn matches_hit_ignores_case_only_when_asked() {
        assert!(matches_hit("Sftp", "sftp", false));
        assert!(!matches_hit("Sftp", "sftp", true));
        assert!(matches_hit("超时", "超时", true));
    }
}
