//! Storage-location callbacks (#storage-location 2026-10-02).
//!
//! Wires the Settings › storage card (ui/storage_settings.slint) to the
//! `datastore` layer: read-only info, open/copy, folder picking, and the
//! direct/migrate switch. The switch only writes the bootstrap file — it takes
//! effect after an app restart (the card shows that hint).

use std::cell::RefCell;
use std::rc::Rc;


use super::*;
use crate::config::ConfigStore;
use crate::datastore::{data_dir, default_data_dir, storage_info, switch_data_dir};

pub(super) fn wire_storage_callbacks(window: &AppWindow, store: Rc<RefCell<ConfigStore>>) {
    // Initial values for the card.
    //
    // (#storage-ui 2026-10-04) 两个计数喂给「包含」chips：主机 = 已配置的会话数
    // （一条 Session 就是一台主机），快捷指令 = 快捷指令条数。为 0 时对应 chip
    // 不显示，所以这里必须用真实的 store 数据，不能写死。
    {
        let info = storage_info();
        window.set_storage_current(info.current.clone().into());
        window.set_storage_default(info.default.into());
        window.set_storage_target(info.current.into());
        let s = store.borrow();
        window.set_storage_host_count(s.sessions().len() as i32);
        window.set_storage_quick_count(s.quick_commands().len() as i32);
    }

    // Open the current data dir in the platform file manager.
    {
        let weak = window.as_weak();
        window.on_open_storage_dir(move || {
            let Some(w) = weak.upgrade() else { return };
            let dir = data_dir().to_string_lossy().into_owned();
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
            }
            let _ = w;
        });
    }

    // Copy the current path; give feedback via the card's status strip.
    {
        let weak = window.as_weak();
        window.on_copy_storage_path(move || {
            let Some(w) = weak.upgrade() else { return };
            let text = data_dir().to_string_lossy().into_owned();
            // arboard on the UI thread can pump the Win32 message loop; do it on
            // a thread like the other clipboard writers in app.rs.
            std::thread::spawn(move || {
                clipboard_set_text(text);
            });
            w.set_storage_status(t("路径已复制", "Path copied").into());
            w.set_storage_status_error(false);
        });
    }

    // Native folder picker → fills the target field (cancel = keep current).
    // (#target-focus 2026-10-05) rfd 是**模态阻塞**调用：同步打开会让主窗在
    // 「聚焦样式绘制之前」就失焦停绘，用户看到的弹框期间输入框永远没有聚焦
    // 样式（关闭后主窗重新激活才补绘）。所以延迟 ~60ms 再开：先给 Slint 一帧
    // 把聚焦样式画出来，视觉连续（点图标 → 聚焦亮起 → 弹框出现）。
    {
        let weak = window.as_weak();
        window.on_pick_storage_dir(move || {
            // rfd 弹出前先强制重绘一帧:聚焦样式(target-focus 置真后)必须在
            // 弹框出现前画出来 —— winit 失焦时会抑制自发重绘,不补这一下,
            // 弹框期间输入框就停在无聚焦的旧画面(用户截图)。
            if let Some(w) = weak.upgrade() {
                w.window().request_redraw();
            }
            let weak = weak.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(60));
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_title(t("选择数据目录", "Choose data directory"))
                        .pick_folder()
                    {
                        if let Some(w) = weak.upgrade() {
                            let dir = folder.to_string_lossy().into_owned();
                            w.set_storage_target(dir.into());
                        }
                    }
                });
            });
        });
    }

    // "Restore default" just refills the field with ~/.mterm; the user still
    // presses a switch button to commit it.
    {
        let weak = window.as_weak();
        window.on_reset_storage_dir(move || {
            if let Some(w) = weak.upgrade() {
                if let Some(d) = default_data_dir() {
                    let dir = d.to_string_lossy().into_owned();
                    w.set_storage_target(dir.into());
                }
            }
        });
    }

    // Commit: write data (+ optionally logs) into the target and flip the
    // bootstrap file. Reports success/failure through the status strip.
    {
        let weak = window.as_weak();
        window.on_switch_storage(move |migrate: bool| {
            let Some(w) = weak.upgrade() else { return false };
            let target = w.get_storage_target().to_string();
            match switch_data_dir(std::path::Path::new(&target), migrate) {
                Ok(()) => {
                    w.set_storage_status(
                        t(
                            "已切换，重启应用后完全生效",
                            "Switched — restart the app to fully apply",
                        )
                        .into(),
                    );
                    w.set_storage_status_error(false);
                    // 计数不动：切换要重启后才生效，当前 store 仍是原目录的数据，
                    // chips 描述的正是「当前存储位置」里有什么。
                    true
                }
                Err(e) => {
                    w.set_storage_status(e.into());
                    w.set_storage_status_error(true);
                    false
                }
            }
        });
    }
}
