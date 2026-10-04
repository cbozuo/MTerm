//! Directory resolution for the data store (#storage-location 2026-10-02).
//!
//! Migrated verbatim-in-spirit from config.rs (portable-first #141) and
//! re-targeted: the default is now `~/.mterm` instead of a portable `config/`
//! beside the executable. A bootstrap file (`<exe_dir>/data-dir.txt`) written
//! by Settings wins over everything; deleting it re-enables the default flow.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use directories::{ProjectDirs, UserDirs};

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Marker in %TEMP% recording that the one-time carry-over already ran (moved
/// out of the data dir so only config/ and log/ show up there).
pub(crate) fn migrated_marker() -> PathBuf {
    std::env::temp_dir().join("meatshell.migrated")
}

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

/// Pre-0.4.15 location: the per-user OS config dir
/// (`%APPDATA%/meatshell`, `~/.config/meatshell`, …). Still consulted for
/// one-time migration and by the legacy backup sync.
pub(crate) fn legacy_data_dir() -> Option<PathBuf> {
    ProjectDirs::from("dev", "meatshell", "meatshell").map(|d| d.config_dir().to_path_buf())
}

/// Portable location: a `config/` folder beside the executable. No longer the
/// default, but its presence marks an upgrading install whose data should be
/// carried into `~/.mterm`.
pub(crate) fn portable_data_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("config"))
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

/// One-time carry-over of data from a legacy location into `~/.mterm`.
/// Old layout had config files flat in the data dir; the new layout puts them
/// in `config/`, so this both copies from legacy sources AND tidies any flat
/// files left by an earlier build of the migration. Runtime droppings
/// (`ipc.port`, the old in-dir marker) are deleted, not copied. Leaves a
/// marker in %TEMP% so a user who deletes files afterwards doesn't get them
/// resurrected.
fn carry_over_legacy_into(default_dir: &Path) {
    let marker = migrated_marker();
    if marker.exists() {
        return;
    }
    let config_dst = default_dir.join("config");
    // Oldest source wins (portable config beside the exe, else legacy dir).
    let sources: [Option<PathBuf>; 2] = [portable_data_dir(), legacy_data_dir()];
    for src in sources.into_iter().flatten() {
        if src.exists() && src != default_dir && src != config_dst {
            migrate_data_files(&src, &config_dst);
        }
    }
    // Tidy the data root: an earlier migration build left the config files
    // flat next to log/ — move them into config/ and drop runtime droppings.
    // The flat file is only removed once the copy is confirmed in place, so a
    // failed rename/copy can never destroy the only copy.
    let _ = fs::create_dir_all(&config_dst);
    for name in ["sessions.json", "secret.key", "known_hosts"] {
        let flat = default_dir.join(name);
        let dst = config_dst.join(name);
        if flat.exists() {
            if dst.exists() || fs::rename(&flat, &dst).is_ok() || fs::copy(&flat, &dst).is_ok() {
                let _ = fs::remove_file(&flat);
            } else {
                tracing::warn!(
                    "storage: could not tidy {} into {} — leaving it in place",
                    flat.display(),
                    dst.display()
                );
            }
        }
    }
    // Runtime droppings from earlier builds: the old in-dir migration marker
    // and the single-instance port/socket files (now in %TEMP%).
    let _ = fs::remove_file(default_dir.join(".migrated-from-portable"));
    let _ = fs::remove_file(default_dir.join("ipc.port"));
    let _ = fs::remove_file(default_dir.join("ipc.sock"));
    // Logs from the old locations too (best effort).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let old_log = parent.join("log");
            if old_log.exists() {
                migrate_data_files(&old_log, &default_dir.join("log"));
            }
        }
    }
    if let Some(legacy) = legacy_data_dir() {
        let old_log = user_log_dir_from_legacy(&legacy);
        if old_log.exists() {
            migrate_data_files(&old_log, &default_dir.join("log"));
        }
    }
    let _ = fs::write(marker, "");
    tracing::info!(
        "storage: legacy data carried over into {}",
        default_dir.display()
    );
}

fn user_log_dir_from_legacy(legacy: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        if let Some(base) = legacy.parent() {
            return base.join("log").join("log");
        }
    }
    legacy.join("log")
}

fn resolve_data_dir() -> PathBuf {
    // 1. Explicit choice from Settings (bootstrap file) wins.
    if let Some(dir) = bootstrapped_data_dir() {
        return dir;
    }

    // 2. Built-in default: ~/.mterm (carrying over any legacy data once).
    if let Some(default_dir) = default_data_dir() {
        if fs::create_dir_all(&default_dir).is_ok() && dir_is_writable(&default_dir) {
            carry_over_legacy_into(&default_dir);
            return default_dir;
        }
    }

    // 3. Legacy per-user dir (pre-0.4.15 location) as the safety net...
    if let Some(dir) = legacy_data_dir() {
        if fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    // ...and a temp dir as the last resort so the app still launches.
    let dir = std::env::temp_dir().join("meatshell");
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
        if matches!(name, "ipc.port" | "ipc.sock" | ".migrated-from-portable")
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
