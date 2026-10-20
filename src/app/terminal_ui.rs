use super::*;

pub(super) fn history_model(store: &ConfigStore) -> ModelRc<SharedString> {
    let rows: Vec<SharedString> = store
        .command_history()
        .iter()
        .map(|s| s.clone().into())
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

/// 预设/自定义规则模型的公共取色入口：ANSI 槽位 → 当前主题实色。
/// (#hl-settings-hifi) 弹框里的色点必须与终端里真正染上去的颜色同源，否则
/// 「列表里是青的、终端里是黄的」。用与 presentation 同一张主题色表。
fn ansi_dot(idx: u8, dark: bool, pal: &crate::theme::palettes::Palette) -> slint::Color {
    let v = pal.ansi16[idx as usize % 16];
    let _ = dark; // ANSI16 本身即分暗/亮两套表（palette 已按明暗选好）
    slint::Color::from_rgb_u8(((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8)
}

/// 自定义规则色点：`#RRGGBB` 绝对色（取色盘/48 色卡给什么就是什么）。
fn rule_dot(color: &str) -> slint::Color {
    match crate::picker::parse_hex(color) {
        Some(v) => slint::Color::from_rgb_u8(
            ((v >> 16) & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            (v & 0xFF) as u8,
        ),
        // 旧配置里的语义名（red/yellow…）按原 ANSI 槽位落点
        None => ansi_dot(
            match crate::terminal::parse_color(color) {
                crate::terminal::RuleColor::Idx(i) => i,
                crate::terminal::RuleColor::Rgb(r, g, b) => {
                    return slint::Color::from_rgb_u8(r, g, b)
                }
            },
            true,
            crate::theme::palette_or_default("", true),
        ),
    }
}

/// 高亮设置弹框的「预设规则」页模型：表序即列表序，`enabled` 逐条可关；
/// 彩虹括号（paren）不是正则，单独一行 + 彩虹条 + 「非正则」角标。
pub(super) fn highlight_preset_model(store: &ConfigStore) -> ModelRc<HighlightPresetItem> {
    let dark = theme_pref_is_dark(store);
    let pal = crate::theme::palette_or_default(&store.theme(), dark);
    let enabled = store.output_highlight_enabled();
    let rows: Vec<HighlightPresetItem> = crate::terminal::HIGHLIGHT_PRESETS
        .iter()
        .map(|p| HighlightPresetItem {
            id: p.id.into(),
            name: t(p.name_zh, p.name_en).into(),
            group: t(p.group_zh, p.group_en).into(),
            pattern: p.pattern.into(),
            dot: ansi_dot(p.ansi, dark, pal),
            rainbow: false,
            special: "".into(),
            enabled: crate::terminal::preset_enabled(
                enabled,
                store.output_highlight_disabled_presets(),
                p.id,
            ),
        })
        .chain(std::iter::once(HighlightPresetItem {
            id: crate::terminal::RAINBOW_PRESET_ID.into(),
            name: t("彩虹括号", "Rainbow brackets").into(),
            group: t("特殊着色", "Special").into(),
            pattern: t(
                "对嵌套括号 () [] {} 按层级循环着色",
                "Colour nested () [] {} by depth",
            )
            .into(),
            // 彩虹行的色点用第一色，行内另画七色彩条
            dot: slint::Color::from_rgb_u8(0xF7, 0x54, 0x64),
            rainbow: true,
            special: t("非正则", "Not a regex").into(),
            enabled: crate::terminal::preset_enabled(
                enabled,
                store.output_highlight_disabled_presets(),
                crate::terminal::RAINBOW_PRESET_ID,
            ),
        }))
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

/// 「自定义」页模型：色点是绝对色（#RRGGBB 或旧语义名解析后的 RGB）。
pub(super) fn highlight_custom_model(store: &ConfigStore) -> ModelRc<HighlightCustomItem> {
    let rows: Vec<HighlightCustomItem> = store
        .output_highlight_rules()
        .iter()
        .map(|r| HighlightCustomItem {
            name: if r.name.trim().is_empty() {
                r.pattern.clone().into()
            } else {
                r.name.clone().into()
            },
            pattern: r.pattern.clone().into(),
            hex: crate::config::normalize_highlight_color(&r.color).into(),
            dot: rule_dot(&r.color),
            enabled: r.enabled && store.output_highlight_enabled(),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

/// 徽标「N 已启用」：启用的预设数（含彩虹括号）+ 启用的自定义规则数。
pub(super) fn highlight_enabled_count(store: &ConfigStore) -> i32 {
    if !store.output_highlight_enabled() {
        return 0;
    }
    let disabled = store.output_highlight_disabled_presets();
    let presets = crate::terminal::HIGHLIGHT_PRESETS
        .iter()
        .filter(|p| !disabled.iter().any(|d| d == p.id))
        .count()
        + usize::from(!disabled.iter().any(|d| d == crate::terminal::RAINBOW_PRESET_ID));
    let custom = store
        .output_highlight_rules()
        .iter()
        .filter(|r| r.enabled)
        .count();
    (presets + custom) as i32
}

/// 新规则校验：空 / 超长 / 正则编不过都拦在这里（弹框内联提示）。
pub(super) fn validate_highlight_rule(pattern: &str) -> std::result::Result<(), String> {
    if pattern.trim().is_empty() {
        return Err(t(
            "请输入正则表达式",
            "Enter a regular expression",
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
    regex::Regex::new(pattern).map_err(|error| {
        format!(
            "{}: {error}",
            t("无效的正则表达式", "Invalid regular expression")
        )
    })?;
    Ok(())
}

/// 把当前配置编译成规则表并写进**每一个** terminal buffer，随后全量重渲染
/// （滚动回看里在改动之前抓取的行也会重新着色）。总开关 / 预设开关 /
/// 自定义规则增删改都走这一个入口 —— 与 apply_theme 同构。
pub(super) fn apply_highlight_rules(window: &AppWindow, bufs: &TermBuffers, store: &ConfigStore) {
    // ConfigStore 的取值方法自带内部锁，直接调即可（无 borrow()）
    let (rules, rainbow) = {
        (
            compile_highlight_rules(
                store.output_highlight_enabled(),
                store.output_highlight_disabled_presets(),
                store.output_highlight_rules(),
            ),
            rainbow_parens_enabled(
                store.output_highlight_enabled(),
                store.output_highlight_disabled_presets(),
            ),
        )
    };
    {
        let handles: Vec<_> = bufs.lock().unwrap().values().cloned().collect();
        for handle in handles {
            let mut buf = handle.lock().unwrap();
            buf.highlight_rules = rules.clone();
            buf.rainbow_parens = rainbow;
        }
    }
    let tab_ids: Vec<String> = bufs.lock().unwrap().keys().cloned().collect();
    for tab_id in tab_ids {
        rebuild_tab_display(window, bufs, &tab_id);
    }
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
/// (#theme-split) 仅在 theme 为空（跟随系统）时作为 mterm 门面的深浅依据。
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
        // (#wallpaper-scope-params 2026-10-18) 一次性解析出「当前生效」的一整套
        // 数值：有壁纸 → (主题, 壁纸, 作用域) 那一套；无壁纸 → 明暗槽。
        appearance: store.effective_appearance(),
        // (#zone-default-context 2026-10-19) 双击复位的目标：有壁纸用该组合
        // 首设默认值，无壁纸回 0% 透明（两者确实不同，用户定稿）。
        zone_defaults: {
            let (titlebar, tabbar, sidebar, term) = store.default_zone_alphas();
            crate::theme::ZoneDefaults { titlebar, tabbar, sidebar, term }
        },
        wallpaper_scope: store.wallpaper_scope().to_string(),
        wallpaper_fit: store.wallpaper_fit().to_string(),
        scrim_style: store.scrim_style().to_string(),
        dark_pref: theme_pref_is_dark(store),
    }
}

/// (#theme-split) 应用当前主题（唯一入口）：整套槽位写进主窗口 Theme、
/// 每个 terminal buffer 换色表（ANSI/默认前景背景随主题），并全量重渲染。
/// 切主题 / 改强调色 / 改三滑杆 / 改通透度 / 广播同步都走这里。
/// 返回生效的主题 id。
pub(super) fn apply_theme(window: &AppWindow, bufs: &TermBuffers, store: &ConfigStore) -> String {
    let st = theme_state_of(window, store);
    apply_theme_resolved(window, bufs, &st).id.to_string()
}

/// (#theme-dedupe §5) 用「已解析的 ThemeState」完整应用：UI 槽 + 强调色守卫
/// 回写 + 终端 buffer 色表 + 重渲染。**悬停预览与点击落盘共用这一条路径** ——
/// 高保真 §5：预览必须同步终端色表，否则预览与落盘结果不一致。
pub(super) fn apply_theme_resolved(
    window: &AppWindow,
    bufs: &TermBuffers,
    st: &crate::theme::ThemeState,
) -> &'static crate::theme::palettes::Palette {
    let p = crate::theme::apply(window, st);
    // (#accent-guard 2026-10-13) 设置页 §3.8 的强调色守卫提示行：每次重算
    // （切主题/改强调色/悬停预览/图片取色）都回写原色与生效色，面板上的
    // chip 与「对比度守卫」文案据此显示真实数值，不再是一句静态说明。
    let g = crate::theme::accent_guard(p, st.accent_mode);
    window.set_accent_guard(crate::ui::AccentGuard {
        eff: format!("#{:06X}", g.eff).into(),
        panel: format!("#{:06X}", g.panel).into(),
        raw: format!("#{:06X}", g.raw).into(),
        ratio: format!("{:.2}", g.ratio).into(),
        adjusted: g.adjusted,
    });
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
    p
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
            // (#wallpaper-auto-fit 2026-10-18) 「自动」填充要按图片真实比例判定：
            // 解码后的尺寸随图片一起回写（缩略/自定义都走同一条）。
            let size = wp.image.size();
            let aspect = if size.height > 0 {
                size.width as f32 / size.height as f32
            } else {
                1.6
            };
            window
                .global::<crate::ui::Theme>()
                .set_wallpaper_aspect(aspect);
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
