use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{OutputHighlightRule, QuickCommand, Secret, Session};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WslProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub distribution: String,
    #[serde(default = "default_wsl_home")]
    pub directory: String,
}

fn default_wsl_home() -> String {
    "~".to_string()
}

// Testing-stage MCP defaults. Switch these serde defaults to false before the
// feature is promoted from preview to a stable release.
fn default_mcp_preview_enabled() -> bool {
    true
}

/// Ships with the "幻想 3048" sci-fi wallpaper on by default (a dark theme). New
/// installs and users upgrading from before the wallpaper feature get it; once
/// the user picks anything (including "无"/none, stored as ""), their choice is
/// saved and sticks.
fn default_wallpaper() -> String {
    // Serde default for the `wallpaper` field: kept at the old "幻想 3048" so an
    // *existing* config that predates the field stays on tech — `migrate_defaults`
    // then advances default-following users through the migration chain. Brand-new
    // installs get the current default straight from `fresh_config`.
    "builtin:tech".to_string()
}

/// Bump when `migrate_defaults` gains a new one-time default-layout change.
pub const DEFAULTS_REV: u32 = 4;

pub(crate) const PREVIOUS_DEFAULT_WALLPAPER_TRANSPARENCY: f32 = 0.38;
pub(crate) const PREVIOUS_DEFAULT_WALLPAPER_OVERLAY: f32 =
    1.0 - PREVIOUS_DEFAULT_WALLPAPER_TRANSPARENCY;
pub(crate) const DEFAULT_WALLPAPER_TRANSPARENCY: f32 = 0.15;
pub(crate) const DEFAULT_WALLPAPER_OVERLAY: f32 = 1.0 - DEFAULT_WALLPAPER_TRANSPARENCY;

// (#theme-split 2026-10-07) 主题/强调色/壁纸解耦后的新字段默认值。
// rev4 迁移（只对「一直在默认值上」的用户生效，§5）：
//   theme_pref = dark  → theme = graphite-dark
//   theme_pref = light → theme = graphite-light
//   theme_pref = system/"" → theme = ""（follow_system = true）
//   wallpaper_overlay → panel_alpha(旧值) + term_alpha(1.0)
//   wallpaper 仍在 serde 默认（builtin:tech，从未挑过）→ ""（默认无壁纸）
pub(crate) const DEFAULT_PANEL_ALPHA: f32 = 0.85;
pub(crate) const DEFAULT_TERM_ALPHA: f32 = 1.0;

pub(crate) fn default_theme() -> String {
    // 空 = 跟随系统（follow_system = true 时按探测的深浅取 graphite 族）。
    // 设计稿 §5：默认壁纸改为无 + 跟随系统，「零操作即正确」。
    String::new()
}
pub(crate) fn default_panel_alpha() -> f32 {
    DEFAULT_PANEL_ALPHA
}
pub(crate) fn default_term_alpha() -> f32 {
    DEFAULT_TERM_ALPHA
}
pub(crate) fn default_wallpaper_visible() -> f32 {
    1.0
}
pub(crate) fn default_follow_system() -> bool {
    true
}
pub(crate) fn default_channel_members() -> [Vec<String>; 4] {
    [Vec::new(), Vec::new(), Vec::new(), Vec::new()]
}

pub(crate) fn default_sidebar_width() -> f32 {
    220.0
}
pub(crate) fn default_sidebar_height() -> f32 {
    240.0
}
pub(crate) fn default_sftp_width() -> f32 {
    380.0
}
pub(crate) fn default_sftp_height() -> f32 {
    220.0
}
pub(crate) fn default_sftp_tree_width() -> f32 {
    160.0
}

pub(crate) fn default_sftp_visible_columns() -> Vec<String> {
    ["name", "type", "size", "modified", "permissions", "owner", "group"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub(crate) fn default_quick_panel_width() -> f32 {
    260.0
}

pub(crate) fn default_quick_panel_height() -> f32 {
    220.0
}

/// On-disk layout. Keep additive to ease forward-compat.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    pub sessions: Vec<Session>,
    /// User-managed WSL launch entries. An empty list keeps the implicit default
    /// WSL entry for backwards compatibility.
    #[serde(default)]
    pub wsl_profiles: Vec<WslProfile>,
    /// Preset SFTP download directory. Empty = ask each time.
    #[serde(default)]
    pub download_dir: String,
    /// UI language code: "zh" (default) or "en".
    #[serde(default)]
    pub language: String,
    /// Theme preference: "system" (default) | "dark" | "light".
    #[serde(default)]
    pub theme_pref: String,
    /// Platform renderer preference. Windows uses software/auto/gpu; macOS uses
    /// software/femtovg/skia. Missing or foreign-platform values use the platform default.
    #[serde(default)]
    pub renderer_mode: String,
    /// UI (sans) font family, chosen in Interface › Font › Interface font. Empty
    /// = the first usable system CJK family from the chain in app.rs. A saved
    /// name that is no longer installed — or that no longer covers the probe
    /// glyphs — falls back to the chain at startup and is rewritten to empty.
    #[serde(default)]
    pub ui_font_family: String,
    /// Terminal font family. Empty = the first usable system monospace
    /// (resolved from the mono chain in app.rs). Stale names that are no
    /// longer installed migrate back to empty at startup.
    #[serde(default)]
    pub font_family: String,
    /// Terminal font size in px. 0 = the built-in default.
    #[serde(default)]
    pub font_size: u32,
    /// Terminal line-height multiplier. 0 means the default 1.0.
    #[serde(default)]
    pub terminal_line_spacing: f32,
    /// Force regular terminal text to render with a bold face (#262).
    #[serde(default)]
    pub terminal_bold: bool,
    /// Terminal insertion cursor shape: block (default), bar, or underline (#275).
    #[serde(default)]
    pub terminal_cursor_style: String,
    /// Custom terminal cursor colour as #RRGGBB. Empty follows the theme (#275).
    #[serde(default)]
    pub terminal_cursor_color: String,
    /// Stored inverted so missing/legacy config keeps the automatic plain-text
    /// output highlighter enabled by default.
    #[serde(default)]
    pub output_highlight_disabled: bool,
    /// Built-in output highlight preset: "log" (default) or "devops".
    #[serde(default)]
    pub output_highlight_preset: String,
    /// User-defined rules applied before the selected built-in preset.
    #[serde(default)]
    pub output_highlight_rules: Vec<OutputHighlightRule>,
    /// Stored inverted so complete JSON lines are formatted and syntax-coloured
    /// by default while still allowing users to preserve byte-for-byte display.
    #[serde(default)]
    pub json_format_disabled: bool,
    /// Global UI scale in percent (#100). 0 = default (100%).
    #[serde(default)]
    pub ui_scale: u32,
    /// Immersive wallpaper id: "" = none, "builtin:light" / "builtin:dark" /
    /// "builtin:tech", or a filesystem path to a custom image. Drives the
    /// wallpaper + tinted theme. Defaults to the "幻想 3048" built-in.
    #[serde(default = "default_wallpaper")]
    pub wallpaper: String,
    /// Explicit session groups/folders (#41), including empty ones so a folder
    /// can exist before any session is moved into it. "default" is implicit and
    /// not stored here.
    #[serde(default)]
    pub groups: Vec<String>,
    /// Quick Connect folders that were collapsed when the UI was last used.
    /// `None` is a legacy/new config and starts with every folder collapsed;
    /// `Some([])` means the user explicitly expanded every folder.
    #[serde(default)]
    pub collapsed_session_groups: Option<Vec<String>>,
    /// (#group-color 2026-09-14) 用户为分组指定的颜色:组名 -> "#RRGGBB"。
    /// **没有条目 = 无色**(渲染时回落主题的次级前景色,不是置灰)。
    /// 迁移时不种子任何值:旧的 hash 派生色按设计丢弃,所有组默认中性,
    /// 直到用户手动选一个。
    #[serde(default)]
    pub group_colors: HashMap<String, String>,
    /// Stored inverted ("don't follow") so both serde and the Default derive
    /// yield `false` = the feature defaults to ON: the SFTP panel follows the
    /// terminal's cd (OSC 7) unless the user opts out in Interface settings.
    #[serde(default)]
    pub sftp_no_follow_cd: bool,
    /// Always prompt for the save location on each download instead of using the
    /// preset download dir. Defaults to false (#87).
    #[serde(default)]
    pub download_always_ask: bool,
    /// Hide the quick-command bar under the terminal. Defaults to false.
    #[serde(default)]
    pub hide_cmd_bar: bool,
    /// (#hide-system-group) Hide the "Local Terminals" reserved group in the
    /// welcome list. Toggled from the blank-area / group-header context menus.
    /// Defaults to false (visible).
    #[serde(default)]
    pub hide_system_group: bool,
    /// Stored inverted so multiline paste confirmation remains enabled for
    /// existing configurations unless the user explicitly disables it (#300).
    #[serde(default)]
    pub paste_confirm_disabled: bool,
    /// (#close-behavior) 点窗口关闭键时的行为:ask 每次询问 /
    /// tray 最小化到系统托盘 / exit 直接完全退出。
    /// 空串(旧配置)按 ask 处理,见 ConfigStore::close_behavior。
    #[serde(default)]
    pub close_behavior: String,
    /// Stored inverted so Ctrl+Alt+V, Shift+Insert, and middle-click paste stay
    /// enabled for existing users (#300).
    #[serde(default)]
    pub extra_paste_shortcuts_disabled: bool,
    /// Hide auxiliary panels and edge strips so the terminal fills the window.
    #[serde(default)]
    pub zen_mode: bool,
    /// Saved quick commands (#55).
    #[serde(default)]
    pub quick_commands: Vec<QuickCommand>,
    /// Explicit quick-command group names — mirrors `groups` for sessions so that
    /// empty quick-command groups survive and can be renamed/deleted (#55).
    #[serde(default)]
    pub quick_groups: Vec<String>,
    /// Opt-in docked quick-command sidebar (#215). The command-bar popup remains
    /// available until the user actually drags it into the main dock layer.
    #[serde(default)]
    pub quick_commands_as_sidebar: bool,
    #[serde(default)]
    pub quick_panel_open: bool,
    #[serde(default)]
    pub quick_panel_collapsed: bool,
    #[serde(default = "default_quick_panel_width")]
    pub quick_panel_width: f32,
    #[serde(default = "default_quick_panel_height")]
    pub quick_panel_height: f32,
    #[serde(default)]
    pub quick_panel_dock: String,
    /// Recent commands sent from the command box, oldest first, capped (#55).
    #[serde(default)]
    pub command_history: Vec<String>,
    /// Collapse the left resource sidebar on startup (#78).
    #[serde(default)]
    pub collapse_sidebar_default: bool,
    /// Last resource-sidebar collapsed state. None means fall back to
    /// `collapse_sidebar_default` for older configs.
    #[serde(default)]
    pub sidebar_collapsed: Option<bool>,
    /// User-adjustable width of the left resource sidebar, in logical pixels.
    /// Persisted across restarts so the drag-resized width sticks.
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    /// Resource-panel docking: size when docked top/bottom, and which edge it is
    /// docked to (left|right|top|bottom). Persisted so the layout sticks (#dock).
    #[serde(default = "default_sidebar_height")]
    pub sidebar_height: f32,
    #[serde(default)]
    pub sidebar_dock: String,
    /// SFTP-panel docking: extents (px) and docked edge, persisted (#dock).
    #[serde(default = "default_sftp_width")]
    pub sftp_panel_width: f32,
    #[serde(default = "default_sftp_height")]
    pub sftp_panel_height: f32,
    #[serde(default = "default_sftp_tree_width")]
    pub sftp_tree_width: f32,
    #[serde(default)]
    pub sftp_dock: String,
    /// Columns shown in the SFTP file list. Unknown values are ignored by the
    /// accessor so newer builds can safely read older or hand-edited configs.
    #[serde(default = "default_sftp_visible_columns")]
    pub sftp_visible_columns: Vec<String>,
    /// Last window size in logical px (0 = unset → use the built-in default).
    /// Lets users keep their preferred window size across restarts.
    #[serde(default)]
    pub window_width: f32,
    #[serde(default)]
    pub window_height: f32,
    /// Collapse the bottom SFTP panel on startup (#78).
    #[serde(default)]
    pub collapse_sftp_default: bool,
    /// When session-sync is on, also mirror SFTP uploads to the other online
    /// sessions (same path, falling back to each panel's current dir).
    #[serde(default)]
    pub sync_upload: bool,
    /// WebDAV sync settings (#185). Password is encrypted at rest like session
    /// passwords; remote_path is the JSON export object path under the endpoint.
    #[serde(default)]
    pub webdav_enabled: bool,
    #[serde(default)]
    pub webdav_url: String,
    #[serde(default)]
    pub webdav_username: String,
    #[serde(default)]
    pub webdav_password: Secret,
    #[serde(default)]
    pub webdav_remote_path: String,
    #[serde(default)]
    pub webdav_accept_invalid_certs: bool,
    /// Render the welcome page (session list) as a docked left sidebar instead of
    /// a "New tab" tab (v0.5). Persisted so the layout choice sticks.
    #[serde(default)]
    pub welcome_as_sidebar: bool,
    /// Width (logical px) of the welcome/session sidebar when docked (v0.5).
    #[serde(default)]
    pub welcome_sidebar_width: f32,
    /// Welcome/session sidebar dock edge (left|right|top|bottom).
    #[serde(default)]
    pub welcome_sidebar_dock: String,
    /// Welcome sidebar collapsed to the edge icon strip (IDEA-style) (v0.5).
    /// None means the user has not explicitly collapsed/expanded it yet.
    #[serde(default)]
    pub welcome_collapsed: Option<bool>,
    /// Frosted-panel opacity over a wallpaper (0.30–1.00); user-adjustable via the
    /// Interface › Wallpaper opacity slider. 0 = use the current default.
    #[serde(default)]
    pub wallpaper_overlay: f32,
    // ── (#theme-split 2026-10-07) 主题/强调色/壁纸三层解耦 ──────────────
    /// 主题 id（src/theme/palettes.rs 的 38 套之一）。空 = 跟随系统
    /// （follow_system = true 时按探测深浅取 graphite 族）。
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Follow the OS dark/light preference instead of a fixed theme id.
    #[serde(default = "default_follow_system")]
    pub follow_system: bool,
    /// 强调色三态："follow"（默认，主题自带）| "preset" | "custom"。
    #[serde(default)]
    pub accent_mode: String,
    /// accent_mode = "preset" 时的预设 key（ACCENT_PRESETS）。
    #[serde(default)]
    pub accent_preset: String,
    /// accent_mode = "custom" 时的 "#RRGGBB"。
    #[serde(default)]
    pub accent_custom: String,
    /// 面板不透明度（30–100 滑杆 → 0.30–1.00）。由旧 wallpaper_overlay 迁移。
    #[serde(default = "default_panel_alpha")]
    pub panel_alpha: f32,
    /// 终端不透明度（75–100 滑杆 → 0.75–1.00，下限保护 ANSI 可读）。
    #[serde(default = "default_term_alpha")]
    pub term_alpha: f32,
    /// 壁纸图自身可见度（0–100 滑杆 → 0.00–1.00）。
    #[serde(default = "default_wallpaper_visible")]
    pub wallpaper_visible: f32,
    /// 弹窗通透度三档：0 标准（实心）| 1 通透 | 2 全透。
    #[serde(default)]
    pub popup_transparency: i32,
    /// 「跟随图片取色」——唯一让壁纸反向影响界面的开关（默认关，§6-待定①）。
    #[serde(default)]
    pub wallpaper_color_pickup: bool,
    /// 频道成员表：4 个固定槽（A 海蓝/B 松石/C 琥珀/D 玫红）各存一个**会话
    /// 稳定 id**（Session.uuid / builtin "system:*"）；空 = 该槽无成员。
    /// 跨重启保留（离线成员留在表里只跳过投递），tab 序号绝不入表。
    #[serde(default)]
    pub channel_members: [Vec<String>; 4],
    /// 已暂停参与频道的会话 id 集合（按会话粒度：既不发送也不接收，
    /// 成员关系保留；换频道时跟着会话走）。
    #[serde(default)]
    pub channel_paused: Vec<String>,
    /// Settings-panel font scale, percent (80–160). 0 = 100% default (v0.5).
    #[serde(default)]
    pub panel_font: u32,
    /// Enable the local stdio MCP server (`MTerm mcp serve`).
    #[serde(default = "default_mcp_preview_enabled")]
    pub mcp_enabled: bool,
    /// Allow MCP tools to use credentials already stored by MTerm. Secrets
    /// remain internal and are never included in protocol responses.
    #[serde(default = "default_mcp_preview_enabled")]
    pub mcp_use_saved_credentials: bool,
    /// Allow MCP clients to execute arbitrary commands on saved SSH sessions.
    #[serde(default = "default_mcp_preview_enabled")]
    pub mcp_allow_commands: bool,
    /// Allow MCP clients to upload local files and download remote files.
    #[serde(default = "default_mcp_preview_enabled")]
    pub mcp_allow_file_transfers: bool,
    /// One-time default-layout migration marker (#new-user-defaults). 0 = config
    /// predates the migration. `migrate_defaults` bumps it to `DEFAULTS_REV` after
    /// pushing the new look (default wallpaper / welcome-as-sidebar / right-docked
    /// resource panel / wallpaper overlay) to users still sitting on old defaults.
    #[serde(default)]
    pub defaults_rev: u32,
}

/// Portable export file (issue #46): sessions with everything in plaintext
/// **except** the password, which is encrypted with a fixed key baked into the
/// binary so the file opens on *any* machine running MTerm.
///
/// Security note: a built-in key in open-source code is **obfuscation, not real
/// security** — anyone with the source can derive it. It only stops a casual
/// over-the-shoulder read of the file, same level as FinalShell's export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExportFile {
    /// Format marker / version so the schema can evolve later.
    pub(crate) mterm_export: u32,
    pub(crate) sessions: Vec<Session>,
}
