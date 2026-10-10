use super::*;

pub(super) fn history_model(store: &ConfigStore) -> ModelRc<SharedString> {
    let rows: Vec<SharedString> = store
        .command_history()
        .iter()
        .map(|s| s.clone().into())
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn output_highlight_rule_model(store: &ConfigStore) -> ModelRc<OutputRuleItem> {
    let rows: Vec<OutputRuleItem> = store
        .output_highlight_rules()
        .iter()
        .map(|rule| OutputRuleItem {
            pattern: rule.pattern.clone().into(),
            regex: rule.regex,
            case_sensitive: rule.case_sensitive,
            whole_line: rule.whole_line,
            color: match rule.color.as_str() {
                "yellow" | "green" | "cyan" | "magenta" | "gray" => rule.color.clone(),
                _ => "red".to_string(),
            }
            .into(),
            enabled: rule.enabled,
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn validate_output_highlight_rule(
    pattern: &str,
    is_regex: bool,
    case_sensitive: bool,
) -> std::result::Result<(), String> {
    if pattern.is_empty() {
        return Err(t(
            "请输入关键词或正则表达式",
            "Enter a keyword or regular expression",
        )
        .into());
    }
    if pattern.chars().count() > 512 {
        return Err(t(
            "规则不能超过 512 个字符",
            "Rules cannot exceed 512 characters",
        )
        .into());
    }
    if is_regex {
        regex::RegexBuilder::new(pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|error| {
                format!(
                    "{}: {error}",
                    t("无效的正则表达式", "Invalid regular expression")
                )
            })?;
    }
    Ok(())
}

/// Build the filtered history-view rows for the dropdown, oldest first with the
/// newest command at the bottom (#55, #101). The command-history model shares
/// the same storage order so ↑/↓ recall keeps its shell-like semantics.
pub(super) fn history_view_rows(history: &[String], query: &str) -> Vec<SharedString> {
    let q = query.trim().to_lowercase();
    history
        .iter()
        .filter(|command| q.is_empty() || command.to_lowercase().contains(&q))
        .map(|command| command.clone().into())
        .collect()
}

/// Build the filtered history-view model for the dropdown: case-insensitive
/// substring matches of `query`, ordered from oldest to newest (#101).
pub(super) fn history_view_model(store: &ConfigStore, query: &str) -> ModelRc<SharedString> {
    let rows = history_view_rows(store.command_history(), query);
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

#[cfg(test)]
#[path = "../../tests/app/command_history/mod.rs"]
mod history_view_tests;

/// Find every (case-insensitive) occurrence of `query` across the currently
/// displayed rows and return highlight rectangles in GRID-COLUMN space (wide
/// CJK glyphs count as two columns, so highlights line up over the text #132).
pub(super) fn compute_find_matches(rows: &[String], query: &str) -> Vec<TermMatch> {
    let mut out: Vec<TermMatch> = Vec::new();
    if query.is_empty() {
        return out;
    }
    let q: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    if q.is_empty() {
        return out;
    }
    for (r, line) in rows.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let lower: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
        let prefix = cell_prefix(&chars);
        let mut i = 0usize;
        while i + q.len() <= lower.len() {
            if lower[i..i + q.len()] == q[..] {
                let col = prefix[i] as i32;
                let len = (prefix[i + q.len()] - prefix[i]) as i32;
                out.push(TermMatch {
                    row: r as i32,
                    col,
                    len,
                });
                i += q.len();
            } else {
                i += 1;
            }
        }
    }
    out
}

/// Apply a settled terminal size to the PTY + vt100 grid. Factored out of the
/// resize callback so that callback can debounce — a layout reflow can briefly
/// report a near-zero width, collapsing term-cols to its 10-col floor; applying
/// that to the remote PTY reflows vt100 and garbles running output like a
/// `git clone` progress meter (#163). Debouncing means only the settled size
/// ever reaches the server.
pub(super) fn apply_terminal_resize(
    handles: &Rc<RefCell<HashMap<String, SessionHandle>>>,
    bufs: &TermBuffers,
    last_term_size: &Arc<Mutex<(u32, u32)>>,
    tab_id: &str,
    cols: u32,
    rows: u32,
) {
    *last_term_size.lock().unwrap() = (cols, rows);
    if let Some(handle) = handles.borrow().get(tab_id) {
        handle.resize(cols, rows);
    }
    if let Some(h) = term_buf(bufs, tab_id) {
        let mut buf = h.lock().unwrap();
        let (old_rows, old_cols) = buf.parser.screen().size();
        let (new_rows, new_cols) = (rows as u16, cols as u16);
        if (new_rows, new_cols) != (old_rows, old_cols) {
            if buf.parser.screen().alternate_screen() {
                // Alt-screen (tmux/vim/btop): the remote redraws the whole screen
                // on SIGWINCH, so just resize the grid and let that redraw fill it.
                buf.parser.screen_mut().set_size(new_rows, new_cols);
            } else {
                // Reflow already-printed output to the new width by replaying the
                // byte stream — vt100's set_size only truncates/pads (#169).
                buf.reflow(new_rows, new_cols);
            }
            // The pre/post-resize screens differ; drop the scroll-detection
            // snapshot so the next output isn't mis-read as a scroll.
            buf.prev.clear();
        }
    }
}

/// Recompute spans + cursor + find/selection highlights for one tab from its
/// current vt100 screen (respecting scrollback) and push them to the model.
/// Used by scroll + selection callbacks (Output has its own equivalent inline).
pub(super) fn rebuild_tab_display(win: &AppWindow, bufs: &TermBuffers, tab_id: &str) {
    let data = with_term_buf(bufs, tab_id, |buf| {
        let cols = buf.parser.screen().size().1;
        let b = buf.render(); // also refreshes buf.displayed_text
        let matches = compute_find_matches(&buf.displayed_text, &buf.find_query);
        let sel = buf.selection_rects_visible(cols);
        (b, matches, sel)
    });
    let Some((b, matches, sel)) = data else {
        return;
    };
    let spans = ModelRc::from(Rc::new(VecModel::from(b.spans)));
    let fm = ModelRc::from(Rc::new(VecModel::from(matches)));
    let sm = ModelRc::from(Rc::new(VecModel::from(sel)));
    let (cr, cc, ru, alt) = (b.cursor_row, b.cursor_col, b.rows_used, b.is_alt);
    let (smax, soff) = (b.scroll_max, b.scroll_offset);
    set_terminal_row(win, tab_id, move |row| {
        row.spans = spans.clone();
        row.cursor_row = cr;
        row.cursor_col = cc;
        row.rows_used = ru;
        row.is_alt_screen = alt;
        row.mouse_tracked = b.mouse_tracked;
        row.find_matches = fm.clone();
        row.selection = sm.clone();
        row.scroll_max = smax;
        row.scroll_offset = soff;
    });
    win.window().request_redraw();
}

/// Refresh only the lightweight selection overlay. Dragging used to call
/// `rebuild_tab_display` for every mouse-move event, reparsing and rebuilding
/// all terminal spans even though the underlying screen had not changed.
pub(super) fn refresh_terminal_selection(win: &AppWindow, bufs: &TermBuffers, tab_id: &str) {
    let selection = with_term_buf(bufs, tab_id, |buf| {
        let cols = buf.parser.screen().size().1;
        buf.selection_rects_visible(cols)
    });
    let Some(selection) = selection else {
        return;
    };
    let model = ModelRc::from(Rc::new(VecModel::from(selection)));
    set_terminal_row(win, tab_id, move |row| {
        row.selection = model.clone();
    });
    win.window().request_redraw();
}

/// Resolve the user's saved theme preference to a dark/light bool (mirrors the
/// startup logic): "light"/"dark" win; otherwise ask the OS, defaulting to dark.
/// (#theme-split) 仅在 theme 为空（跟随系统）时作为 graphite 族的深浅依据。
pub(super) fn theme_pref_is_dark(store: &ConfigStore) -> bool {
    match store.theme_pref() {
        "light" => false,
        "dark" => true,
        _ => match dark_light::detect() {
            dark_light::Mode::Light => false,
            dark_light::Mode::Dark => true,
            dark_light::Mode::Default => true, // undetectable → dark
        },
    }
}

/// (#theme-split) 从 config 组装主题状态（强调色三态 + 三透明度 + 主题 id）。
/// 「跟随图片取色」开启且壁纸在场时，强调色来源切到图片主色——这是唯一
/// 让壁纸反向影响界面的开关（默认关）。
pub(super) fn theme_state_of(window: &AppWindow, store: &ConfigStore) -> crate::theme::ThemeState {
    let mut accent_mode = crate::theme::AccentCfg::from_config(
        store.accent_mode(),
        store.accent_preset(),
        store.accent_custom(),
    );
    if store.wallpaper_color_pickup() && window.get_wallpaper_active() {
        let c = window.get_wp_accent();
        accent_mode = crate::theme::AccentCfg::Custom(
            ((c.red() as u32) << 16) | ((c.green() as u32) << 8) | c.blue() as u32,
        );
    }
    crate::theme::ThemeState {
        theme_id: store.theme().to_string(),
        accent_mode,
        panel_alpha: store.panel_alpha(),
        term_alpha: store.term_alpha(),
        wallpaper_visible: store.wallpaper_visible(),
        popup_transparency: store.popup_transparency(),
        dark_pref: theme_pref_is_dark(store),
    }
}

/// (#theme-split) 应用当前主题（唯一入口）：整套槽位写进主窗口 Theme、
/// 每个 terminal buffer 换色表（ANSI/默认前景背景随主题），并全量重渲染。
/// 切主题 / 改强调色 / 改三滑杆 / 改通透度 / 广播同步都走这里。
/// 返回生效的主题 id。
pub(super) fn apply_theme(window: &AppWindow, bufs: &TermBuffers, store: &ConfigStore) -> String {
    let st = theme_state_of(window, store);
    let p = crate::theme::apply(window, &st);
    {
        let handles: Vec<_> = bufs.lock().unwrap().values().cloned().collect();
        for h in handles {
            h.lock().unwrap().palette = p;
            h.lock().unwrap().is_dark = p.dark;
        }
    }
    let tab_ids: Vec<String> = bufs.lock().unwrap().keys().cloned().collect();
    for tid in tab_ids {
        rebuild_tab_display(window, bufs, &tid);
    }
    p.id.to_string()
}

pub(super) fn apply_output_highlight(
    window: &AppWindow,
    bufs: &TermBuffers,
    enabled: bool,
    preset: &str,
) {
    let mode = OutputHighlightPreset::from_settings(enabled, preset);
    {
        let handles: Vec<_> = bufs.lock().unwrap().values().cloned().collect();
        for handle in handles {
            handle.lock().unwrap().output_highlight = mode;
        }
    }
    let tab_ids: Vec<String> = bufs.lock().unwrap().keys().cloned().collect();
    for tab_id in tab_ids {
        rebuild_tab_display(window, bufs, &tab_id);
    }
}

pub(super) fn apply_custom_output_rules(
    window: &AppWindow,
    bufs: &TermBuffers,
    rules: &[OutputHighlightRule],
) {
    let compiled = compile_output_rules(rules);
    {
        let handles: Vec<_> = bufs.lock().unwrap().values().cloned().collect();
        for handle in handles {
            handle.lock().unwrap().custom_highlight_rules = compiled.clone();
        }
    }
    let tab_ids: Vec<String> = bufs.lock().unwrap().keys().cloned().collect();
    for tab_id in tab_ids {
        rebuild_tab_display(window, bufs, &tab_id);
    }
}

/// Apply a wallpaper id to the window (#theme-split：壁纸只负责"背后那张
/// 图"——加载图片、写 wallpaper-img / wp-accent（跟随图片取色的来源色），
/// **不再反写主题深浅、不再顶替强调色**）。An empty or undecodable id turns
/// immersive mode off.
pub(super) fn apply_wallpaper(
    window: &AppWindow,
    store: &ConfigStore,
    bufs: &TermBuffers,
    id: &str,
) {
    match crate::wallpaper::load(id) {
        Some(wp) => {
            let (ar, ag, ab) = wp.palette.accent;
            window.set_wallpaper_img(wp.image);
            window.set_wp_accent(slint::Color::from_rgb_u8(ar, ag, ab));
            window.set_wallpaper_active(true);
            window.set_current_wallpaper(id.into());
            let name = if crate::wallpaper::is_builtin(id) {
                String::new()
            } else {
                std::path::Path::new(id)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };
            window.set_custom_wallpaper_name(name.into());
            // 跟随图片取色开着：图片主色要立刻成为强调色来源（重算整套并刷
            // 终端——强调色进了 ANSI 之外的 UI 各处）。
            if store.wallpaper_color_pickup() {
                apply_theme(window, bufs, store);
            }
        }
        None => {
            window.set_wallpaper_active(false);
            window.set_current_wallpaper("".into());
            window.set_custom_wallpaper_name("".into());
        }
    }
}

/// Resolve which interface drives the top sparkline: the user's selection if it
/// still exists, otherwise the busiest (the list is sorted busiest-first).
/// Returns (name, rx_bps, tx_bps).
pub(super) fn selected_iface(st: &TabStatus) -> (String, u64, u64) {
    if !st.selected_iface.is_empty() {
        if let Some(e) = st.net.iter().find(|e| e.0 == st.selected_iface) {
            return e.clone();
        }
    }
    st.net.first().cloned().unwrap_or_default()
}

/// Recompute the whole sidebar (status dot + CPU/mem/swap + dual network panel)
/// for whichever tab is active.  Welcome tab → local machine; a session tab →
/// that server.  The bottom network graph is always the local machine.
/// Must run on the Slint event loop thread.
/// The copyable IP/host from a `user@host` connection label (#192): the part
/// after the last `@`, trimmed. Falls back to the whole string when there's no
/// `@` (already a bare host/IP).
pub(super) fn conn_ip(host: &str) -> String {
    host.rsplit('@').next().unwrap_or(host).trim().to_string()
}
