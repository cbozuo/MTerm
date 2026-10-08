#[path = "struct/mod.rs"]
mod structs;
#[path = "impls/config.rs"]
mod config;
#[path = "impls/finalshell.rs"]
mod finalshell;

pub(crate) use config::*;
pub(crate) use structs::*;

// Data-store layer (#storage-location 2026-10-02): directory resolution and
// the Settings-driven location switch live in src/datastore/. Re-exported so
// existing `crate::config::log_dir()` call sites keep working untouched.
pub(crate) use crate::datastore::log_dir;
