use super::*;

pub(super) fn push_ring(buf: &mut Vec<f32>, val: f32) {
    if buf.len() != NET_HISTORY_LEN {
        *buf = vec![0.0; NET_HISTORY_LEN];
    }
    buf.remove(0);
    buf.push(val);
}

/// Auto-scale a raw bytes/sec history to 0..1 against its own window peak so the
/// sparkline always uses the full height (like FinalShell's relative graph).
pub(super) fn normalized_model(buf: &[f32]) -> ModelRc<f32> {
    let max = buf.iter().cloned().fold(1.0_f32, f32::max);
    let scaled: Vec<f32> = buf.iter().map(|v| (v / max).clamp(0.0, 1.0)).collect();
    ModelRc::from(Rc::new(VecModel::from(scaled)))
}

/// Build the filesystem-usage model (path, "avail/total", used fraction).
pub(super) fn disk_rows(disks: &[(String, u64, u64)]) -> Vec<DiskInfo> {
    disks
        .iter()
        .map(|(mount, avail, total)| {
            let used = total.saturating_sub(*avail);
            let percent = if *total > 0 {
                used as f32 / *total as f32
            } else {
                0.0
            };
            DiskInfo {
                path: mount.clone().into(),
                detail: format!("{}/{}", format_size(*avail), format_size(*total)).into(),
                percent,
            }
        })
        .collect()
}

pub(super) fn disk_model(disks: &[(String, u64, u64)]) -> ModelRc<DiskInfo> {
    ModelRc::from(Rc::new(VecModel::from(disk_rows(disks))))
}

/// Build the process-monitor model for the popup (#23). `cpu`/`mem` are
/// pre-formatted to one decimal; `cpu_frac` (0..1) drives the row's load bar.
pub(super) fn set_process_action_error(weak: &slint::Weak<ProcWindow>, message: &str) {
    if let Some(window) = weak.upgrade() {
        window.set_action_busy(false);
        window.set_action_error(true);
        window.set_action_status(message.into());
    }
}

/// A root login can signal any process directly. Non-root logins may signal
/// only their own processes; root and other users' processes require `su`.
pub(super) fn process_needs_root(current_user: &str, process_user: &str) -> bool {
    current_user != "root" && process_user != current_user
}

pub(super) fn proc_rows(procs: &[ProcInfo], current_user: &str, tab_id: &str) -> Vec<ProcRow> {
    procs
        .iter()
        .map(|p| ProcRow {
            tab_id: tab_id.into(),
            pid: p.pid.to_string().into(),
            user: p.user.clone().into(),
            cpu: format!("{:.1}", p.cpu).into(),
            mem: format!("{:.1}", p.mem).into(),
            command: p.command.clone().into(),
            cpu_frac: (p.cpu / 100.0).clamp(0.0, 1.0),
            own_process: !process_needs_root(current_user, &p.user),
        })
        .collect()
}

#[cfg(test)]
#[path = "../../tests/app/process_monitor/mod.rs"]
mod process_row_tests;

pub(super) fn metric_rows(
    cpu: f32,
    mem: f32,
    swap: f32,
    mem_detail: impl Into<SharedString>,
    swap_detail: impl Into<SharedString>,
) -> Vec<SysMetricRow> {
    vec![
        SysMetricRow {
            label: "CPU".into(),
            percent: cpu,
            detail: "".into(),
            kind: 0,
        },
        SysMetricRow {
            label: t("内存", "Memory").into(),
            percent: mem,
            detail: mem_detail.into(),
            kind: 1,
        },
        SysMetricRow {
            label: t("交换", "Swap").into(),
            percent: swap,
            detail: swap_detail.into(),
            kind: 2,
        },
    ]
}

pub(super) fn net_rows(net: &[(String, u64, u64)]) -> Vec<SysNetRow> {
    net.iter()
        .map(|(name, rx, tx)| SysNetRow {
            name: name.clone().into(),
            up: format_bytes_per_sec(*tx).into(),
            down: format_bytes_per_sec(*rx).into(),
        })
        .collect()
}

pub(super) fn pairs_to_overview_rows(pairs: &[(String, String)]) -> Vec<SysInfoRow> {
    pairs
        .chunks(2)
        .map(|chunk| {
            let first = &chunk[0];
            let second = chunk.get(1);
            SysInfoRow {
                c1: first.0.clone().into(),
                c2: first.1.clone().into(),
                c3: second.map(|p| p.0.clone()).unwrap_or_default().into(),
                c4: second.map(|p| p.1.clone()).unwrap_or_default().into(),
                c5: "".into(),
            }
        })
        .collect()
}

pub(super) fn pairs_to_one_row(pairs: &[(String, String)]) -> Vec<SysInfoRow> {
    let value = |idx: usize| {
        pairs
            .get(idx)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "-".to_string())
    };
    vec![SysInfoRow {
        c1: value(0).into(),
        c2: value(1).into(),
        c3: value(2).into(),
        c4: value(3).into(),
        c5: value(4).into(),
    }]
}

pub(super) fn pairs_to_rows(pairs: &[(String, String)], width: usize) -> Vec<SysInfoRow> {
    pairs
        .chunks(width)
        .filter(|chunk| {
            chunk
                .iter()
                .any(|(_, v)| !v.trim().is_empty() && v.trim() != "-")
        })
        .map(|chunk| {
            let value = |idx: usize| {
                chunk
                    .get(idx)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| "-".to_string())
            };
            SysInfoRow {
                c1: value(0).into(),
                c2: value(1).into(),
                c3: value(2).into(),
                c4: value(3).into(),
                c5: value(4).into(),
            }
        })
        .collect()
}

pub(super) fn cpu_usage_detail_rows(pairs: &[(String, String)]) -> Vec<SysInfoRow> {
    let value = |idx: usize| {
        pairs
            .get(idx)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "0.0%".to_string())
    };
    let extra = pairs
        .iter()
        .skip(4)
        .map(|(k, v)| format!("{k} {v}"))
        .collect::<Vec<_>>()
        .join(" / ");
    vec![SysInfoRow {
        c1: value(0).into(),
        c2: value(2).into(),
        c3: value(1).into(),
        c4: value(3).into(),
        c5: extra.into(),
    }]
}

pub(super) fn tuple5_rows(rows: &[(String, String, String, String, String)]) -> Vec<SysInfoRow> {
    rows.iter()
        .map(|r| SysInfoRow {
            c1: r.0.clone().into(),
            c2: r.1.clone().into(),
            c3: r.2.clone().into(),
            c4: r.3.clone().into(),
            c5: r.4.clone().into(),
        })
        .collect()
}

/// (#theme-split) 把主窗口 Theme global 的全部主题槽位拷到子窗口的
/// Theme global。Theme 是每窗口一份的 Slint global，子窗口不吃主窗口的
/// 属性变更——apply_theme 只写主窗口，独立窗口靠这里的整表拷贝跟上。
macro_rules! copy_theme_slots {
    ($src:expr, $dst:expr) => {{
        let s = $src.global::<crate::ui::Theme<'_>>();
        let d = $dst.global::<crate::ui::Theme<'_>>();
        d.set_theme_id(s.get_theme_id());
        d.set_theme_name(s.get_theme_name());
        d.set_dark(s.get_dark());
        d.set_slot_root(s.get_slot_root());
        d.set_slot_panel(s.get_slot_panel());
        d.set_slot_palt(s.get_slot_palt());
        d.set_slot_elev(s.get_slot_elev());
        d.set_slot_hov(s.get_slot_hov());
        d.set_slot_act(s.get_slot_act());
        d.set_slot_line(s.get_slot_line());
        d.set_slot_lstr(s.get_slot_lstr());
        d.set_slot_t1(s.get_slot_t1());
        d.set_slot_t2(s.get_slot_t2());
        d.set_slot_t3(s.get_slot_t3());
        d.set_slot_tbg(s.get_slot_tbg());
        d.set_slot_tfg(s.get_slot_tfg());
        d.set_slot_ac(s.get_slot_ac());
        d.set_slot_ok(s.get_slot_ok());
        d.set_slot_wr(s.get_slot_wr());
        d.set_slot_dg(s.get_slot_dg());
        d.set_group_colors(s.get_group_colors());
        d.set_channel_colors(s.get_channel_colors());
        d.set_titlebar_transparency(s.get_titlebar_transparency());
        d.set_tabbar_transparency(s.get_tabbar_transparency());
        d.set_sidebar_transparency(s.get_sidebar_transparency());
        d.set_term_transparency(s.get_term_transparency());
        d.set_wallpaper_transparency(s.get_wallpaper_transparency());
        d.set_wallpaper_scope(s.get_wallpaper_scope());
        d.set_wallpaper_fit(s.get_wallpaper_fit());
        d.set_scrim_style(s.get_scrim_style());
        d.set_scrim_strength(s.get_scrim_strength());
        d.set_popup_transparency(s.get_popup_transparency());
        d.set_wallpaper(s.get_wallpaper());
        d.set_wallpaper_active(s.get_wallpaper_active());
        d.set_wp_accent(s.get_wp_accent());
    }};
}

/// Mirror the main window's theme/scale/UI-font onto the detached process
/// window. Theme is a per-window Slint global, so a detached window keeps its
/// compile-time (dark) defaults until we copy these across (#23).
/// 「发送命令」独立窗口的主题镜像（与 sync_proc_theme 同款）。
pub(super) fn sync_send_cmd_theme(main: &AppWindow, w: &SendCmdWindow) {
    copy_theme_slots!(main, w);
    w.set_dark_mode(main.get_dark_mode());
    w.set_ui_scale(main.get_ui_scale());
    w.set_ui_font_family(main.get_ui_font_family());
    w.set_wallpaper_img(main.get_wallpaper_img());
    w.set_wallpaper_active(main.get_wallpaper_active());
    w.set_wp_accent(main.get_wp_accent());
    w.set_wp_tint(main.get_wp_tint());
}

/// 把「发送命令」窗口摆到主窗内容区正中（v2 定稿 940×600,最小 800×540;
/// 1366×768 笔记本上左右各留 213px 仍完整显示）。
pub(super) fn place_send_cmd_window(main: &AppWindow, w: &SendCmdWindow) {
    const W: f64 = 940.0;
    const H: f64 = 600.0;
    let Some(((ox, oy), (iw, ih))) = main
        .window()
        .with_winit_window(|ww| {
            let scale = ww.scale_factor().max(0.01);
            let outer_pos = ww
                .outer_position()
                .ok()
                .or_else(|| ww.inner_position().ok())
                .unwrap_or_default();
            let inner = ww.inner_size();
            Some((
                (outer_pos.x as f64 / scale, outer_pos.y as f64 / scale),
                (inner.width as f64 / scale, inner.height as f64 / scale),
            ))
        })
        .flatten()
    else {
        return;
    };
    if iw <= 0.0 || ih <= 0.0 {
        return;
    }
    let x = ox + (iw - W).max(0.0) / 2.0;
    let y = oy + (ih - H).max(0.0) / 2.0;
    w.window().set_size(slint::LogicalSize::new(W as f32, H as f32));
    w.window().set_position(slint::LogicalPosition::new(x as f32, y as f32));
}

pub(super) fn sync_proc_theme(main: &AppWindow, proc: &ProcWindow) {
    copy_theme_slots!(main, proc);
    proc.set_dark_mode(main.get_dark_mode());
    proc.set_ui_scale(main.get_ui_scale());
    proc.set_ui_font_family(main.get_ui_font_family());
    proc.set_wallpaper_img(main.get_wallpaper_img());
    proc.set_wallpaper_active(main.get_wallpaper_active());
    proc.set_wp_accent(main.get_wp_accent());
    proc.set_wp_tint(main.get_wp_tint());
}

pub(super) fn sync_system_info_theme(main: &AppWindow, sys: &SystemInfoWindow) {
    copy_theme_slots!(main, sys);
    sys.set_dark_mode(main.get_dark_mode());
    sys.set_ui_scale(main.get_ui_scale());
    sys.set_ui_font_family(main.get_ui_font_family());
    sys.set_wallpaper_img(main.get_wallpaper_img());
    sys.set_wallpaper_active(main.get_wallpaper_active());
    sys.set_wp_accent(main.get_wp_accent());
    sys.set_wp_tint(main.get_wp_tint());
}

pub(super) fn sync_editor_theme(main: &AppWindow, editor: &EditorWindow) {
    copy_theme_slots!(main, editor);
    editor.set_dark_mode(main.get_dark_mode());
    editor.set_ui_scale(main.get_ui_scale());
    editor.set_ui_font_family(main.get_ui_font_family());
    // (#editor-hifi) 编辑器代码区等宽字体链 + 语法色 16 槽随主题切换
    {
        let s = main.global::<crate::ui::Theme<'_>>();
        let d = editor.global::<crate::ui::Theme<'_>>();
        d.set_font_mono(s.get_font_mono());
        d.set_ansi16(s.get_ansi16());
    }
    editor.set_wallpaper_img(main.get_wallpaper_img());
    editor.set_wallpaper_active(main.get_wallpaper_active());
    editor.set_wp_accent(main.get_wp_accent());
    editor.set_wp_tint(main.get_wp_tint());
}

pub(super) fn place_system_info_window(main: &AppWindow, sys: &SystemInfoWindow) {
    // Centre on the **content area** of the main AppWindow (not the OS window
    // bounds). `size()` returns the OS-level outer size (title bar + frame
    // included), so centring on it lands the popup at the OS window's centre
    // — which the user sees as "a bit too low" because the title bar pushes
    // the visual centre upward. `winit::Window::inner_size()` gives the
    // content area (frame subtracted), so centring on that puts the popup
    // exactly where the user expects: the centre of the shell content
    // (#win-centre).
    let Some(((ox, oy), (iw, ih), tw, th)) = main
        .window()
        .with_winit_window(|ww| {
            let scale = ww.scale_factor().max(0.01);
            // `outer_position()` is a Result — unsupported on some platforms
            // (Wayland). Fall back to the (usually supported) inner_position
            // and finally to (0,0) so the popup still lands near the main
            // window instead of failing the whole placement.
            let outer_pos = ww
                .outer_position()
                .ok()
                .or_else(|| ww.inner_position().ok())
                .unwrap_or_default();
            let inner_size = ww.inner_size();
            Some((
                (outer_pos.x as f64 / scale, outer_pos.y as f64 / scale),
                (inner_size.width as f64 / scale, inner_size.height as f64 / scale),
                1160.0_f64, 760.0_f64, // SystemInfoWindow preferred size
            ))
        })
        .flatten()
    else {
        return;
    };
    if iw <= 0.0 || ih <= 0.0 {
        return;
    }

    // Half the content area, clamped to SystemInfoWindow's min/preferred
    // sizes (`min-width: 760px; min-height: 520px; preferred-width: 1160px;
    // preferred-height: 760px` in system_info_window.slint).
    let target_w = tw.clamp(760.0, (iw - 24.0).max(760.0));
    let target_h = th.clamp(520.0, (ih - 24.0).max(520.0));
    let x = ox + (iw - target_w).max(0.0) / 2.0;
    let y = oy + (ih - target_h).max(0.0) / 2.0;

    // Use the Slint window API instead of the winit handle: hidden windows are
    // no longer materialized eagerly (they map on Wayland and pollute the
    // taskbar — see the vendor patch in i-slint-backend-winit), so the first
    // open may run before the native window exists. In that state the Slint
    // API stores the size/position on the adapter and applies it at creation;
    // with a live window it behaves like the winit calls did.
    sys.window()
        .set_size(slint::LogicalSize::new(target_w as f32, target_h as f32));
    sys.window()
        .set_position(slint::LogicalPosition::new(x as f32, y as f32));
}

/// Centre the process monitor on the **content area** of the main AppWindow
/// (not the OS outer bounds) — see the long comment on `place_system_info_window`
/// for why inner_size matters. On first open the winit handle may not exist
/// yet (`outer_size()` returns `(0, 0)`), which would push the window to the
/// right half — the "process popup appears bottom-right" complaint. We size
/// the winit window to the panel's preferred size on first open, then centre
/// it on the main window's content area in logical coordinates.
pub(super) fn place_process_window(main: &AppWindow, process: &ProcWindow) {
    let monitor = main
        .window()
        .with_winit_window(|ww| ww.current_monitor().or_else(|| ww.primary_monitor()))
        .flatten();
    let Some(monitor) = monitor else { return };
    let origin = monitor.position();
    let monitor_size = monitor.size();

    // The winit window may not exist yet on the first open (deferred creation,
    // see place_system_info_window); fall back to a zero size, which anchors
    // the window's top-left at the monitor center until the next open.
    // Wayland ignores client-side positioning entirely, so this only affects
    // X11/Windows first-open placement.
    let window_size = process
        .window()
        .with_winit_window(|ww| ww.outer_size())
        .unwrap_or_default();
    let x = origin.x + monitor_size.width.saturating_sub(window_size.width) as i32 / 2;
    let y = origin.y + monitor_size.height.saturating_sub(window_size.height) as i32 / 2;
    process
        .window()
        .set_position(slint::PhysicalPosition::new(x, y));
}
