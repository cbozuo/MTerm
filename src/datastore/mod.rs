// Data-store layer: where the app keeps its personal data (config + logs).
//
// Split out of config.rs (#storage-location 2026-10-02) so directory policy
// lives in one module instead of inside the config load/save file.
//
// Resolution order (#storage-location):
//   1. bootstrap file  <exe_dir>/data-dir.txt  — written when the user picks a
//      custom location in Settings; deleting it returns to the default.
//   2. default dir     ~/.mterm
//   3. %TEMP% fallback so the app still launches on a broken machine.
//
// `log_dir()` follows the data dir (`<data_dir>/log`): config and logs move
// together as "personal data" (#storage-location).
#[path = "struct/storage.rs"]
mod storage;
#[path = "impls/paths.rs"]
mod paths;
#[path = "impls/switch.rs"]
mod switch;

pub(crate) use paths::{config_dir, data_dir, default_data_dir, log_dir};
pub(crate) use switch::{storage_info, switch_data_dir};
