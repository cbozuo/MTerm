use serde::{Deserialize, Serialize};

/// Snapshot of the storage-location settings shown in the Settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    /// Where data is being read from right now (resolved, absolute).
    pub current: String,
    /// The built-in default (`~/.mterm`) — what "restore default" fills in.
    pub default: String,
}
