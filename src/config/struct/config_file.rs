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
/// rev 5 = #follow-ui-removed（跟随系统下线：一律 follow=false、theme 落成具体一套）。
pub const DEFAULTS_REV: u32 = 6;

pub(crate) const PREVIOUS_DEFAULT_WALLPAPER_TRANSPARENCY: f32 = 0.38;
pub(crate) const PREVIOUS_DEFAULT_WALLPAPER_OVERLAY: f32 =
    1.0 - PREVIOUS_DEFAULT_WALLPAPER_TRANSPARENCY;
pub(crate) const DEFAULT_WALLPAPER_TRANSPARENCY: f32 = 0.15;
pub(crate) const DEFAULT_WALLPAPER_OVERLAY: f32 = 1.0 - DEFAULT_WALLPAPER_TRANSPARENCY;

// (#theme-split 2026-10-07) 主题/强调色/壁纸解耦后的新字段默认值。
// rev4 迁移（只对「一直在默认值上」的用户生效，§5）：
//   theme_pref = dark  → theme = mterm-dark
//   theme_pref = light → theme = mterm-light
//   theme_pref = system/"" → theme = ""（follow_system = true）
//   wallpaper_overlay → panel_alpha(旧值) + term_alpha(1.0)
//   wallpaper 仍在 serde 默认（builtin:tech，从未挑过）→ ""（默认无壁纸）
// (#zone-alpha-default0 2026-10-18) 出厂四根透明度全 0% = alpha 1.0：面板即主题
// 原色，不混桌面/壁纸。想要雾化由用户自己拉滑杆（旧版默认 0.85 = 15% 雾化，
// 那时窗口不透明，观感上等于原色；窗口改成真透明后 15% 会真的透出桌面）。
pub(crate) const DEFAULT_PANEL_ALPHA: f32 = 1.0;
pub(crate) const DEFAULT_TERM_ALPHA: f32 = 1.0;
/// 遮罩浓度出厂值 = 原「标准档」基线（深色 0.32）。
pub(crate) const DEFAULT_SCRIM_ALPHA: f32 = 0.32;

pub(crate) fn default_theme() -> String {
    // (#follow-ui-removed 2026-10-16) 「跟随系统」已下线，但这里**必须仍返回空串**：
    // rev4 迁移靠 `theme.is_empty()` 判定「用户从未挑过主题」（本函数即该字段的
    // serde 默认），返回具体 id 会让老配置被误判成「已挑过」而整段跳过迁移。
    // 空串的语义已变：不再是「跟随系统」，而是「尚未落定」—— rev5 迁移把它落成
    // mterm-dark，`fresh_config()` 也直接给具体 id。运行时 `palette_or_default`
    // 对空/非法 id 的兜底此后只作异常修复用。
    String::new()
}
pub(crate) fn default_panel_alpha() -> f32 {
    DEFAULT_PANEL_ALPHA
}
pub(crate) fn default_term_alpha() -> f32 {
    DEFAULT_TERM_ALPHA
}
pub(crate) fn default_scrim_alpha() -> f32 {
    DEFAULT_SCRIM_ALPHA
}
pub(crate) fn default_wallpaper_scope() -> String {
    "window".to_string()
}
pub(crate) fn default_wallpaper_fit() -> String {
    // (#wallpaper-auto-fit 2026-10-18) 默认「自动」：按图片与作用域区域的比例
    // 在填充 / 适应之间选。老配置里显式存过 cover/contain/fill 的保持原值。
    "auto".to_string()
}
pub(crate) fn default_scrim_style() -> String {
    "auto".to_string()
}
pub(crate) fn default_follow_system() -> bool {
    // (#follow-ui-removed 2026-10-16) 跟随系统下线：不再有「新装即跟随」。
    false
}
/// 常用主题出厂值：门面两套（#theme-dedupe 用户决策：默认只显示 MTerm 暗/亮）。
pub(crate) fn default_favorite_themes() -> Vec<String> {
    vec!["mterm-dark".to_string(), "mterm-light".to_string()]
}
pub(crate) fn default_channel_members() -> [Vec<String>; 4] {
    [Vec::new(), Vec::new(), Vec::new(), Vec::new()]
}

pub(crate) fn default_right_sidebar_width() -> f32 {
    220.0
}
pub(crate) fn default_right_sidebar_height() -> f32 {
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

/// (#mode-values 2026-10-17) 一套「外观数值」—— 暗 / 亮各存一份。
///
/// 用户定稿：「暗主题调好了，切亮主题不该用暗主题的数值；明暗不共用，某个
/// 模式第一次使用就是默认值」。字段语义与之前的平铺版本一致（alpha = 1 − 透明度）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceValues {
    /// 四分区不透明度（0–1）。
    #[serde(default = "default_panel_alpha")]
    pub titlebar_alpha: f32,
    #[serde(default = "default_panel_alpha")]
    pub tabbar_alpha: f32,
    #[serde(default = "default_panel_alpha")]
    pub sidebar_alpha: f32,
    #[serde(default = "default_term_alpha")]
    pub term_alpha: f32,
    /// 壁纸透明度（0 = 完全显示）。
    #[serde(default)]
    pub wallpaper_transparency: f32,
    /// 弹窗透明度（0 = 实心）。
    #[serde(default)]
    pub popup_transparency: f32,
    /// 遮罩浓度（0–1，默认 0.32）。
    #[serde(default = "default_scrim_alpha")]
    pub scrim_alpha: f32,
}

impl Default for AppearanceValues {
    fn default() -> Self {
        Self {
            titlebar_alpha: DEFAULT_PANEL_ALPHA,
            tabbar_alpha: DEFAULT_PANEL_ALPHA,
            sidebar_alpha: DEFAULT_PANEL_ALPHA,
            term_alpha: DEFAULT_TERM_ALPHA,
            wallpaper_transparency: 0.0,
            popup_transparency: 0.0,
            scrim_alpha: DEFAULT_SCRIM_ALPHA,
        }
    }
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
    pub collapse_right_sidebar_default: bool,
    /// Last resource-sidebar collapsed state. None means fall back to
    /// `collapse_right_sidebar_default` for older configs.
    #[serde(default)]
    pub right_sidebar_collapsed: Option<bool>,
    /// User-adjustable width of the left resource sidebar, in logical pixels.
    /// Persisted across restarts so the drag-resized width sticks.
    #[serde(default = "default_right_sidebar_width")]
    pub right_sidebar_width: f32,
    /// Resource-panel docking: size when docked top/bottom, and which edge it is
    /// docked to (left|right|top|bottom). Persisted so the layout sticks (#dock).
    #[serde(default = "default_right_sidebar_height")]
    pub right_sidebar_height: f32,
    #[serde(default)]
    pub right_sidebar_dock: String,
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
    /// Width (logical px) of the welcome/session sidebar when docked (v0.5).
    #[serde(default)]
    pub left_sidebar_width: f32,
    /// Welcome/session sidebar dock edge (left|right|top|bottom).
    #[serde(default)]
    pub left_sidebar_dock: String,
    /// Welcome sidebar collapsed to the edge icon strip (IDEA-style) (v0.5).
    /// None means the user has not explicitly collapsed/expanded it yet.
    #[serde(default)]
    pub left_sidebar_collapsed: Option<bool>,
    /// 面板不透明度（0.30–1.00 的旧全局值）。四分区改造后不再直接驱动界面：
    /// 仅作 rev4 迁移的来源值（迁到三区各自的 alpha）。
    #[serde(default)]
    pub wallpaper_overlay: f32,
    // ── (#theme-split 2026-10-07) 主题/强调色/壁纸三层解耦 ──────────────
    /// 主题 id（src/theme/palettes.rs 的 28 套之一）。空 = 跟随系统
    /// （follow_system = true 时按探测深浅取 mterm 门面）。
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Follow the OS dark/light preference instead of a fixed theme id.
    #[serde(default = "default_follow_system")]
    pub follow_system: bool,
    /// 「常用主题」列表（#theme-dedupe：速选面板常用区）。出厂 = 门面两套
    /// ["mterm-dark", "mterm-light"]；用户可自选增删，上限 8 套（回调侧校验），
    /// 只存主题 id，非法 id 在读取端过滤。
    #[serde(default = "default_favorite_themes")]
    pub favorite_themes: Vec<String>,
    /// 强调色三态："follow"（默认，主题自带）| "preset" | "custom"。
    #[serde(default)]
    pub accent_mode: String,
    /// accent_mode = "preset" 时的预设 key（ACCENT_PRESETS）。
    #[serde(default)]
    pub accent_preset: String,
    /// accent_mode = "custom" 时的 "#RRGGBB"。
    #[serde(default)]
    pub accent_custom: String,
    /// (#mode-values 2026-10-17) 外观数值：**暗 / 亮各一套**，切换主题即切换那套；
    /// 某模式第一次使用 = 默认值（新配置两套都是默认）。
    #[serde(default)]
    pub appearance_dark: AppearanceValues,
    #[serde(default)]
    pub appearance_light: AppearanceValues,
    // ── rev6 迁移用的旧单套数值（只读、不再序列化）────────────────────
    #[serde(default, rename = "titlebar_alpha", skip_serializing)]
    pub legacy_titlebar_alpha: Option<f32>,
    #[serde(default, rename = "tabbar_alpha", skip_serializing)]
    pub legacy_tabbar_alpha: Option<f32>,
    #[serde(default, rename = "sidebar_alpha", skip_serializing)]
    pub legacy_sidebar_alpha: Option<f32>,
    #[serde(default, rename = "term_alpha", skip_serializing)]
    pub legacy_term_alpha: Option<f32>,
    #[serde(default, rename = "wallpaper_transparency", skip_serializing)]
    pub legacy_wallpaper_transparency: Option<f32>,
    #[serde(default, rename = "popup_transparency", skip_serializing)]
    pub legacy_popup_transparency: Option<f32>,
    #[serde(default, rename = "scrim_alpha", skip_serializing)]
    pub legacy_scrim_alpha: Option<f32>,
    /// 壁纸作用域："window"（整个界面，默认）| "terminal"（仅终端区）。
    #[serde(default = "default_wallpaper_scope")]
    pub wallpaper_scope: String,
    /// 壁纸填充方式："cover"（默认，铺满裁切）| "contain" | "fill"。
    #[serde(default = "default_wallpaper_fit")]
    pub wallpaper_fit: String,
    /// (#scrim-style 2026-10-17) 遮罩样式：auto / dark / light / tint / soft /
    /// frost / spot / sheen / random。
    #[serde(default = "default_scrim_style")]
    pub scrim_style: String,


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
