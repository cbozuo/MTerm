use serde::{Deserialize, Serialize};

/// A saved quick command (#55): a named snippet the user clicks to send to the
/// active terminal.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuickCommand {
    pub name: String,
    pub command: String,
    /// Optional group/folder name. Empty = the implicit "default" group (#55).
    #[serde(default)]
    pub group: String,
    /// Whether clicking the chip sends + executes (appends Return). `false` only
    /// drops the command into the input box to tweak first. Defaults to `true` so
    /// existing quick commands keep running on click. (B站 suggestion)
    #[serde(default = "default_true")]
    pub send_enter: bool,
}

/// One user-defined client-side terminal highlighting rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputHighlightRule {
    /// 用户给规则起的名字（可空；空 = 列表里回落显示正则本身）。
    #[serde(default)]
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_line: bool,
    /// `#RRGGBB`（48 色预设卡 / 调色盘决定）。旧配置里的语义名
    /// red/yellow/green/cyan/magenta/gray 在读入编译时按原 ANSI 槽位解析。
    #[serde(default)]
    pub color: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}
