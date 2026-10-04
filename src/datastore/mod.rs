// Data-store layer: where the app keeps its personal data (config + logs).
//
// Split out of config.rs (#storage-location 2026-10-02) so directory policy
// lives in one module instead of inside the config load/save file.
//
// Resolution order (#storage-location):
//   1. bootstrap file  <exe_dir>/data-dir.txt  — written when the user picks a
//      custom location in Settings; deleting it returns to portable/default.
//   2. default dir     ~/.mterm                — the new default. On the first
//      launch that lands here, data from a legacy location (portable config/
//      beside the exe, or the pre-0.4.15 per-user dir) is COPIED over so
//      upgrading users keep their sessions.
//   3. legacy per-user dir → %TEMP% fallback (unchanged safety net).
//
// `log_dir()` follows the data dir (`<data_dir>/log`): config and logs move
// together as "personal data" (#storage-location).
#[path = "struct/storage.rs"]
mod storage;
#[path = "impls/paths.rs"]
mod paths;
#[path = "impls/switch.rs"]
mod switch;

pub(crate) use paths::{
    bootstrap_file, config_dir, data_dir, default_data_dir, dir_is_writable,
    legacy_data_dir, log_dir, migrate_data_files,
};
pub(crate) use switch::{storage_info, switch_data_dir};
