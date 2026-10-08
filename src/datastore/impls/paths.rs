//! Directory resolution for the data store (#storage-location 2026-10-02).
//!
//! The default is `~/.mterm`. A bootstrap file (`<exe_dir>/data-dir.txt`)
//! written by Settings wins over everything; deleting it re-enables the
//! default flow.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use directories::UserDirs;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The single directory holding all user data (sessions.json, secret.key,
/// known_hosts, log/). Resolved once and cached; switching at runtime requires
/// an app restart (Settings shows a hint).
pub fn data_dir() -> PathBuf {
    DATA_DIR.get_or_init(resolve_data_dir).clone()
}

/// Where configuration files live: `<data_dir>/config` (#storage-location).
/// The data root shows only two folders — config/ and log/ — so everything in
/// it is recognizably this app's.
pub fn config_dir() -> PathBuf {
    data_dir().join("config")
}

/// Logs live inside the data dir as a `log/` child (#storage-location):
/// config and logs move together as one "personal data" location.
pub fn log_dir() -> PathBuf {
    let dir = data_dir().join("log");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Built-in default: `~/.mterm` next to the user's home.
pub fn default_data_dir() -> Option<PathBuf> {
    let home = UserDirs::new()?.home_dir().to_path_buf();
    Some(home.join(".mterm"))
}

/// Bootstrap marker beside the executable; contains the custom data dir path.
pub fn bootstrap_file() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("data-dir.txt"))
}

/// True only if we can actually create and write a file in `dir` — Program
/// Files and other system locations can reject writes even when the dir
/// appears to exist, so a real write probe is the reliable test.
pub fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".write_probe_{}", std::process::id()));
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Read the custom dir from the bootstrap file, if it names a usable dir.
fn bootstrapped_data_dir() -> Option<PathBuf> {
    let file = bootstrap_file()?;
    let raw = fs::read_to_string(file).ok()?;
    let dir = PathBuf::from(raw.trim());
    if dir.is_absolute() && dir.join("config").exists() || (dir.is_absolute() && dir_is_writable(&dir)) {
        Some(dir)
    } else {
        None
    }
}

fn resolve_data_dir() -> PathBuf {
    // 1. Explicit choice from Settings (bootstrap file) wins.
    if let Some(dir) = bootstrapped_data_dir() {
        return dir;
    }

    // 2. Built-in default: ~/.mterm.
    if let Some(default_dir) = default_data_dir() {
        if fs::create_dir_all(&default_dir).is_ok() && dir_is_writable(&default_dir) {
            return default_dir;
        }
    }

    // 3. A temp dir as the last resort so the app still launches.
    let dir = std::env::temp_dir().join("mterm");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Copy every file (recursively) from `src` into `dst`, creating parents.
/// Existing destination files are never overwritten; runtime droppings
/// (single-instance sockets, temp/broken snapshots, write probes) are skipped.
pub fn migrate_data_files(src: &Path, dst: &Path) {
    let Ok(entries) = fs::read_dir(src) else {
        return;
    };
    let _ = fs::create_dir_all(dst);
    for entry in entries.flatten() {
        let from = entry.path();
        let Some(name) = from.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if matches!(name, "ipc.port" | "ipc.sock")
            || name.ends_with(".tmp")
            || name.ends_with(".broken")
            || name.starts_with(".write_probe_")
        {
            continue;
        }
        let to = dst.join(name);
        if from.is_dir() {
            migrate_data_files(&from, &to);
        } else if from.exists() && !to.exists() {
            if let Err(e) = fs::copy(&from, &to) {
                tracing::warn!(
                    "storage: failed to copy {} → {}: {e}",
                    from.display(),
                    to.display()
                );
            }
        }
    }
}
