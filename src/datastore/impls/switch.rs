//! Runtime storage switch (#storage-location 2026-10-02).
//!
//! `switch_data_dir` validates a target, optionally carries data over, and
//! writes the bootstrap file. Because `DATA_DIR` is resolved once per process,
//! the switch takes effect after an app restart (the Settings UI shows a hint).

use std::fs;
use std::path::Path;

use super::paths::{
    bootstrap_file, data_dir, default_data_dir, dir_is_writable, migrate_data_files,
};
use super::storage::StorageInfo;
/// Snapshot for the Settings card.
pub fn storage_info() -> StorageInfo {
    StorageInfo {
        current: data_dir().to_string_lossy().into_owned(),
        default: default_data_dir()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// Point the app at `new_dir` on its next launch.
///
/// * `migrate = true`  → copy current data (and logs) into `new_dir` first;
///   existing destination files are never overwritten.
/// * `migrate = false` → just make sure the dir exists ("use as-is").
///
/// Writes the bootstrap file beside the executable, which wins over every
/// other resolution rule on the next launch.
pub fn switch_data_dir(new_dir: &Path, migrate: bool) -> Result<(), String> {
    let new_dir = PathBuf::from(new_dir.to_string_lossy().trim());
    if new_dir.as_os_str().is_empty() {
        return Err("目录不能为空".into());
    }
    let cur = data_dir();
    if new_dir == cur {
        return Err("新目录与当前目录相同".into());
    }
    fs::create_dir_all(&new_dir)
        .map_err(|e| format!("无法创建目录 {}: {e}", new_dir.display()))?;
    if !dir_is_writable(&new_dir) {
        return Err(format!("目录不可写: {}", new_dir.display()));
    }

    if migrate {
        // Only the two recognized subtrees move — config/ and log/ — so the
        // target dir never receives anything that isn't this app's data.
        let cfg_src = cur.join("config");
        if cfg_src.exists() {
            migrate_data_files(&cfg_src, &new_dir.join("config"));
        } else {
            // The current dir may still be an OLD FLAT layout (fallback dir or
            // a hand-prepared dir): copy the flat config files into the
            // target's config/ so nothing is lost (#storage-location).
            for name in ["sessions.json", "secret.key", "known_hosts"] {
                let flat = cur.join(name);
                if flat.exists() {
                    migrate_data_files(&flat, &new_dir.join("config"));
                }
            }
        }
        let log_src = cur.join("log");
        if log_src.exists() {
            migrate_data_files(&log_src, &new_dir.join("log"));
        }
    }

    let file = bootstrap_file().ok_or_else(|| "无法定位引导文件（exe 目录不可用）".to_string())?;
    fs::write(&file, new_dir.to_string_lossy().as_bytes())
        .map_err(|e| format!("写入引导文件失败: {e}"))?;
    tracing::info!(
        "storage: switched data dir to {} (migrate={}) — takes effect after restart",
        new_dir.display(),
        migrate
    );
    Ok(())
}

use std::path::PathBuf;
