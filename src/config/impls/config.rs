//! Session / application configuration.
//!
//! Persists a simple JSON file in the app's data directory. The default data
//! dir is `~/.mterm`; a `data-dir.txt` bootstrap file next to the executable
//! (written by Settings) overrides it. See [`data_dir`].
//!
//! ## Password encryption
//!
//! Passwords are **not** stored in plaintext.  On first launch a random
//! 256-bit key is written to `secret.key` in the same config directory
//! (mode `0600` on Unix).  Every non-empty password is then encrypted with
//! **ChaCha20-Poly1305** (a random 96-bit nonce per value) and stored as
//!
//! ```text
//! enc:v1:<base64url(nonce_12_bytes || ciphertext)>
//! ```
//!
//! Legacy plaintext passwords (from older installs) are left untouched in
//! memory and silently re-encrypted the next time the config is saved.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit},
    ChaCha20Poly1305,
};
use rand::rngs::OsRng;
use uuid::Uuid;

use super::structs::*;

// ── Data directory (#storage-location 2026-10-02) ─────────────────────────────
// Directory resolution lives in the dedicated `datastore` module
// (src/datastore/); config.rs only consumes `data_dir()` / `log_dir()` from it.


fn normalize_hex_color(value: &str) -> Option<String> {
    let digits = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", digits.to_ascii_uppercase()))
}

/// A brand-new config (no file yet, or the old one was corrupt). Seeds the
/// new-user default layout (#new-user-defaults): ms wallpaper, welcome page as
/// a left sidebar, resource panel docked right, 15% wallpaper transparency, and
/// marks the migration done so it isn't re-applied.
/// (#theme-split rev4) 壁纸默认改为**无**（§0：新用户装完是"幻想3048+被反写的
/// dark"的坑）+ 跟随系统 + mterm 门面默认主题（#theme-dedupe 2026-10-15：新老用户一律新底色）。
fn fresh_config() -> ConfigFile {
    ConfigFile {
        wallpaper: String::new(),
        right_sidebar_dock: "right".to_string(),
        wallpaper_overlay: DEFAULT_WALLPAPER_OVERLAY,
        wallpaper_scope: default_wallpaper_scope(),
        wallpaper_fit: default_wallpaper_fit(),
        scrim_style: default_scrim_style(),
        // (#mode-values 2026-10-17) 明暗两套数值出厂都是默认值。
        appearance_dark: AppearanceValues::default(),
        appearance_light: AppearanceValues::default(),
        // (#follow-ui-removed 2026-10-16) 不再新装即跟随系统：主题直接落具体一套
        // （serde 默认的空串是给 rev4 迁移判定「从未挑过」用的，新装不该带它）。
        follow_system: false,
        theme: "mterm-dark".to_string(),
        defaults_rev: DEFAULTS_REV,
        ..ConfigFile::default()
    }
}

/// One-time push of the new default layout to *existing* users — but only for
/// each item they're still leaving at the old default, so deliberate choices are
/// never clobbered. Runs once (gated by `defaults_rev`); returns whether anything
/// changed so the caller can persist it. (#new-user-defaults)
fn migrate_defaults(cfg: &mut ConfigFile) -> bool {
    if cfg.defaults_rev >= DEFAULTS_REV {
        return false;
    }
    // rev 1: miku / welcome-as-sidebar / right-docked resources / wallpaper overlay.
    if cfg.defaults_rev < 1 {
        // Old default wallpaper → miku. A custom path, "none" (""), or any other
        // built-in means the user chose it, so leave it.
        if cfg.wallpaper == "builtin:tech" {
            cfg.wallpaper = "builtin:miku".to_string();
        }
        // Overlay still unset -> current default.
        if cfg.wallpaper_overlay <= 0.0 {
            cfg.wallpaper_overlay = DEFAULT_WALLPAPER_OVERLAY;
        }
        // Never moved the resource panel (empty = the old left default) → right.
        if cfg.right_sidebar_dock.trim().is_empty() {
            cfg.right_sidebar_dock = "right".to_string();
        }
    }
    // rev 2: settings show wallpaper transparency, while rev 1 accidentally
    // stored the default as panel alpha 0.38, so it displayed as ~62%.
    if cfg.defaults_rev < 2
        && (cfg.wallpaper_overlay - PREVIOUS_DEFAULT_WALLPAPER_TRANSPARENCY).abs() < 0.005
    {
        cfg.wallpaper_overlay = DEFAULT_WALLPAPER_OVERLAY;
    }
    // rev 3: reduce the default transparency from 38% to 15%. Only advance
    // users still on the previous default; preserve every custom slider value.
    if cfg.defaults_rev < 3
        && (cfg.wallpaper_overlay - PREVIOUS_DEFAULT_WALLPAPER_OVERLAY).abs() < 0.005
    {
        cfg.wallpaper_overlay = DEFAULT_WALLPAPER_OVERLAY;
    }
    // rev 4 (#theme-split): 主题/强调色/壁纸三层解耦（§5）。只动「一直在默认
    // 值上」的项；用户显式选过的（含任意 builtin/自定义壁纸）原样保留。
    if cfg.defaults_rev < 4 {
        // 深浅偏好 → 具体主题 id（mterm 门面，#theme-dedupe 2026-10-15：一律新底色）。
        // 空/"system" = 跟随系统（follow_system = true），theme 留空运行时解析。
        // （follow_system 的 serde 默认是 true——dark/light 分支必须显式关掉。）
        if cfg.theme.is_empty() {
            match cfg.theme_pref.as_str() {
                "dark" => {
                    cfg.theme = "mterm-dark".to_string();
                    cfg.follow_system = false;
                }
                "light" => {
                    cfg.theme = "mterm-light".to_string();
                    cfg.follow_system = false;
                }
                _ => cfg.follow_system = true,
            }
        }
        // 一个滑杆曾同时驱动面板+终端 → 拆成独立值（面板沿用旧值，
        // 终端回满档不透明——现状终端本来就更实，视觉不回退，§5）。
        // (#mode-values 2026-10-17) 现在这些值先落到 legacy_* 里，由 rev6 统一
        // 写进「当前模式」的槽；三区同值（用户此前只调过一个值）。
        if cfg.legacy_titlebar_alpha.is_none()
            && cfg.legacy_tabbar_alpha.is_none()
            && cfg.legacy_sidebar_alpha.is_none()
        {
            let a = if cfg.wallpaper_overlay > 0.0 {
                cfg.wallpaper_overlay
            } else {
                DEFAULT_PANEL_ALPHA
            };
            cfg.legacy_titlebar_alpha = Some(a);
            cfg.legacy_tabbar_alpha = Some(a);
            cfg.legacy_sidebar_alpha = Some(a);
            cfg.legacy_term_alpha = Some(DEFAULT_TERM_ALPHA);
        }
        // 从未挑过壁纸的用户 → 默认无壁纸（serde 默认 builtin:tech = 未动过）。
        if cfg.wallpaper == "builtin:tech" {
            cfg.wallpaper = String::new();
        }
    }
    // rev 5 (#follow-ui-removed 2026-10-16): 「跟随系统」整个概念下线 —— UI 两处
    // 入口已删，这里把**行为**也终结：跟随着的配置落成具体主题（theme 为空 →
    // mterm 门面暗色），并一律把 follow_system 关掉。此后主题只有一个轴，不会再
    // 出现「没动主题、界面却随系统变亮」。
    if cfg.defaults_rev < 5 {
        cfg.follow_system = false;
        if cfg.theme.is_empty() {
            cfg.theme = "mterm-dark".to_string();
        }
    }
    // rev 6 (#mode-values 2026-10-17): 外观数值从「单套」拆成「明暗各一套」。
    // 旧值写进**当前主题所属模式**的槽，另一槽留默认 —— 切过去看到的就是
    // 「该模式第一次使用 = 默认值」。旧字段只读（skip_serializing），迁完即弃。
    if cfg.defaults_rev < 6 {
        let dark = crate::theme::by_id(&cfg.theme).map(|p| p.dark).unwrap_or(true);
        let slot = if dark {
            &mut cfg.appearance_dark
        } else {
            &mut cfg.appearance_light
        };
        if let Some(v) = cfg.legacy_titlebar_alpha {
            slot.titlebar_alpha = v;
        }
        if let Some(v) = cfg.legacy_tabbar_alpha {
            slot.tabbar_alpha = v;
        }
        if let Some(v) = cfg.legacy_sidebar_alpha {
            slot.sidebar_alpha = v;
        }
        if let Some(v) = cfg.legacy_term_alpha {
            slot.term_alpha = v;
        }
        if let Some(v) = cfg.legacy_wallpaper_transparency {
            slot.wallpaper_transparency = v;
        }
        if let Some(v) = cfg.legacy_popup_transparency {
            slot.popup_transparency = v;
        }
        if let Some(v) = cfg.legacy_scrim_alpha {
            slot.scrim_alpha = v;
        }
    }
    // rev 7 (#wallpaper-scope-params 2026-10-18): 壁纸参数从「全局一套」改成
    // **主题 × 壁纸 × 作用域** 三级。老配置把当前的壁纸/作用域搬进新表；
    // 用户调过的数值只沿用到**他正在用的那个作用域**，另一种作用域落首设
    // 默认值（否则切过去还是「一片黑」，正是用户反馈的问题）。
    if cfg.defaults_rev < 7 {
        let theme = if cfg.theme.is_empty() {
            "mterm-dark".to_string()
        } else {
            cfg.theme.clone()
        };
        let scope = match cfg.wallpaper_scope.as_str() {
            "terminal" => "terminal",
            _ => "window",
        };
        cfg.wallpaper_scope_by_theme
            .entry(theme.clone())
            .or_insert_with(|| scope.to_string());
        if !cfg.wallpaper.is_empty() {
            let wallpaper = cfg.wallpaper.clone();
            cfg.wallpaper_by_theme
                .entry(theme.clone())
                .or_insert_with(|| wallpaper.clone());
            let dark = crate::theme::by_id(&theme).map(|p| p.dark).unwrap_or(true);
            let av = if dark {
                cfg.appearance_dark.clone()
            } else {
                cfg.appearance_light.clone()
            };
            // 「调过」= 与出厂（四区不透明、壁纸 0%）有实质差异。
            let tuned = (av.titlebar_alpha - DEFAULT_PANEL_ALPHA).abs() > 0.005
                || (av.tabbar_alpha - DEFAULT_PANEL_ALPHA).abs() > 0.005
                || (av.sidebar_alpha - DEFAULT_PANEL_ALPHA).abs() > 0.005
                || (av.term_alpha - DEFAULT_TERM_ALPHA).abs() > 0.005
                || av.wallpaper_transparency > 0.005;
            for s in ["window", "terminal"] {
                let key = wallpaper_key(&theme, &wallpaper, s);
                let entry = cfg
                    .wallpaper_settings
                    .entry(key)
                    .or_insert_with(|| default_scope_settings(s));
                if tuned && s == scope {
                    entry.titlebar_alpha = av.titlebar_alpha;
                    entry.tabbar_alpha = av.tabbar_alpha;
                    entry.sidebar_alpha = av.sidebar_alpha;
                    entry.term_alpha = av.term_alpha;
                    entry.wallpaper_transparency = av.wallpaper_transparency;
                    entry.fit = cfg.wallpaper_fit.clone();
                }
            }
        }
    }
    cfg.defaults_rev = DEFAULTS_REV;
    true
}

/// 自定义规则颜色归一成 `#RRGGBB`。旧配置里的语义名
/// （red/yellow/green/cyan/magenta/gray）在**保存时**迁成对应十六进制
/// （编译期 `parse_color` 也认旧名，双保险）；认不出来的值回落番茄红。
/// (#hl-settings-hifi 2026-10-20：自定义色从 6 档语义色扩到任意色)
pub(crate) fn normalize_highlight_color(color: &str) -> String {
    let c = color.trim();
    if crate::picker::parse_hex(c).is_some() {
        let v = crate::picker::parse_hex(c).unwrap_or(0);
        return format!("#{:06X}", v);
    }
    let legacy = match c.to_ascii_lowercase().as_str() {
        "yellow" => "#F5E147",
        "green" => "#73B00A",
        "cyan" => "#1FC8B4",
        "magenta" => "#C77DBB",
        "gray" => "#7A7E85",
        _ => "#F75464", // red 与一切无法识别的值
    };
    legacy.to_string()
}

/// Remove duplicate entries in place, keeping the *last* (most recent)
/// occurrence of each and preserving relative order (#113). The list is capped
/// at 200, so the quadratic scan is trivial.
fn dedup_keep_last(items: &mut Vec<String>) {
    let mut i = 0;
    while i < items.len() {
        if items[i + 1..].contains(&items[i]) {
            items.remove(i);
        } else {
            i += 1;
        }
    }
}

/// Display-only session groups that must never be persisted as user folders.
/// `default` maps to an empty group; `system` is owned by built-in local shells.
pub(crate) fn is_reserved_session_group(name: &str) -> bool {
    name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("system")
}

/// (#default-group-drag 2026-09-10) 会话的【显示组】:未分组(group 为空)与
/// 保留名一律归于 "default",其余即自身组名。默认组成为正式组(有组头、可
/// 折叠、可作拖放落点)后,拖拽的每个判定点都必须用同一套显示组语义——
/// reorder_session 的邻组查找、build_session_rows 的渲染分组同源,
/// 否则落点与实际行对不上。
pub(crate) fn display_group_of(session: &Session) -> String {
    if session.group.is_empty() || is_reserved_session_group(session.group.trim()) {
        "default".to_string()
    } else {
        session.group.clone()
    }
}

/// Named display groups: explicit folders ∪ the groups sessions are filed
/// under, with reserved names and ungrouped excluded, de-duplicated and
/// sorted case-insensitively. The single source of truth for group display
/// order — shared by the welcome list, drag-reorder target finding and the
/// group dropdown; if these ever drift, drop targets and rendered rows
/// disagree (#41).
pub(crate) fn named_display_groups(explicit: &[String], sessions: &[Session]) -> Vec<String> {
    // (#group-drag-reorder 2026-09-06) 不再按字母排序:explicit groups 的
    // 存储顺序就是用户手动排列的组顺序(组头拖动换位维护它),session-
    // only 组按首次出现顺序附在后面。dedup 保留(explicit 优先)。
    let named: Vec<String> = explicit
        .iter()
        .filter(|group| !is_reserved_session_group(group.trim()))
        .cloned()
        .chain(
            sessions
                .iter()
                .filter(|session| {
                    !session.group.is_empty() && !is_reserved_session_group(session.group.trim())
                })
                .map(|session| session.group.clone()),
        )
        .collect();
    // (#group-dropdown-dedup 2026-09-08) 全局按首次出现去重(explicit 优先),
    // 不能用 Vec::dedup——它只移除【相邻】重复。#group-drag-reorder 改为
    // 保持存储顺序后,explicit 与 sessions 组序不再一致,拼接结果如
    // [1,2,3,2,1,3] 无相邻重复,dedup 原样放行:分组下拉框出现重复项
    // (用户实测),reorder_session 的跨组相邻组查找同样被污染。
    let mut unique: Vec<String> = Vec::with_capacity(named.len());
    for group in named {
        if !unique.contains(&group) {
            unique.push(group);
        }
    }
    unique
}

/// Repair configurations created before #316/#324, when the Move-to menu exposed
/// the built-in `system` group as a destination for saved server sessions.
fn normalize_reserved_session_groups(cfg: &mut ConfigFile) -> bool {
    let old_group_count = cfg.groups.len();
    cfg.groups
        .retain(|group| !is_reserved_session_group(group.trim()));
    let mut changed = cfg.groups.len() != old_group_count;
    for session in &mut cfg.sessions {
        if is_reserved_session_group(session.group.trim()) {
            session.group.clear();
            changed = true;
        }
    }
    changed
}

#[cfg(any(target_os = "macos", test))]
fn normalize_macos_renderer_mode(mode: &str) -> &'static str {
    match mode {
        "femtovg" => "femtovg",
        "skia" => "skia",
        _ => "software",
    }
}

/// `wallpaper_settings` 的键形如 `<theme>|<wallpaper>|<scope>`；判断中段是否
/// 为 `id`。（三段的取值都不含 `|`：壁纸 id 是库内路径或 `builtin:*`。）
fn wallpaper_key_targets(key: &str, id: &str) -> bool {
    key.split_once('|')
        .and_then(|(_, rest)| rest.strip_prefix(id))
        .map(|rest| rest.starts_with('|'))
        .unwrap_or(false)
}

impl ConfigStore {
    /// The prefix that marks an encrypted password blob in sessions.json.
    const ENC_PREFIX: &'static str = "enc:v1:";

    /// Marks a password encrypted with the **portable export key** (issue #46).
    const EXPORT_PREFIX: &'static str = "enc:exp:v1:";

    /// Fixed 32-byte key for portable exports. Baked into the binary so an
    /// exported file decrypts on any machine. Obfuscation only — see `ExportFile`.
    const EXPORT_KEY: [u8; 32] = *b"mterm.export.portable.key.000001";

    // ── Encryption helpers ────────────────────────────────────────────────

    /// Encrypt `plaintext` with ChaCha20-Poly1305 and return
    /// `"enc:v1:<base64url(nonce_12_bytes || ciphertext)>"`.
    fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<String> {
        let cipher = ChaCha20Poly1305::new(key.into());
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng); // 12 random bytes
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("password encrypt error: {e}"))?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ciphertext);
        Ok(format!(
            "{}{}",
            Self::ENC_PREFIX,
            URL_SAFE_NO_PAD.encode(&blob)
        ))
    }

    /// Try to decrypt a value produced by [`Self::encrypt`].
    /// Returns `None` if the string is not an encrypted blob (e.g. a legacy
    /// plaintext value, an empty string, or a tampered/corrupt blob).
    fn try_decrypt(key: &[u8; 32], s: &str) -> Option<String> {
        let b64 = s.strip_prefix(Self::ENC_PREFIX)?;
        let blob = URL_SAFE_NO_PAD.decode(b64).ok()?;
        if blob.len() < 12 {
            return None;
        }
        let (nonce_bytes, ciphertext) = blob.split_at(12);
        let cipher = ChaCha20Poly1305::new(key.into());
        let nonce = chacha20poly1305::Nonce::from_slice(nonce_bytes);
        let plain = cipher.decrypt(nonce, ciphertext).ok()?;
        String::from_utf8(plain).ok()
    }

    // ── Key file management ───────────────────────────────────────────────

    /// Load the 32-byte key from `<config_dir>/secret.key`, or generate and
    /// persist a fresh one.  On Unix the key file is created with mode `0600`
    /// so other local accounts cannot read it.  On Windows files in `%APPDATA%`
    /// are already restricted to the owning user by default ACLs.
    fn load_or_create_key(config_dir: &Path) -> Result<[u8; 32]> {
        use rand::RngCore as _;
        let key_path = config_dir.join("secret.key");

        if key_path.exists() {
            let bytes = fs::read(&key_path)
                .with_context(|| format!("failed to read {}", key_path.display()))?;
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Ok(key);
            }
            tracing::warn!("secret.key has wrong length — regenerating");
        }

        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        fs::write(&key_path, &key)
            .with_context(|| format!("failed to write {}", key_path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("failed to set permissions on {}", key_path.display()))?;
        }
        tracing::info!("generated new encryption key at {}", key_path.display());
        Ok(key)
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Load (or initialise) the config file. On any parse error we back up the
    /// broken file and start fresh — losing saved sessions is better than
    /// crashing at launch.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        let config_dir = path
            .parent()
            .context("config path has no parent directory")?
            .to_path_buf();

        fs::create_dir_all(&config_dir)
            .with_context(|| format!("failed to create config dir {}", config_dir.display()))?;

        let key = Self::load_or_create_key(&config_dir)?;

        let mut migrated = false;
        let cache = if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            match serde_json::from_str::<ConfigFile>(&raw) {
                Ok(mut cfg) => {
                    // (#sync-dialog-v2-fix 2026-10-12) 频道成员改为**内存态**
                    //（跟随活 tab，tab 关闭即退频道；退出软件自然清空）——
                    // 配置文件里历史遗留的成员表一律忽略。
                    cfg.channel_members = default_channel_members();
                    // Decrypt any encrypted passwords; leave legacy plaintext
                    // values untouched (they will be encrypted on next save).
                    for session in &mut cfg.sessions {
                        if let Some(plain) = Self::try_decrypt(&key, session.password.as_str()) {
                            session.password = Secret::new(plain);
                        }
                        if let Some(plain) =
                            Self::try_decrypt(&key, session.private_key_inline.as_str())
                        {
                            session.private_key_inline = Secret::new(plain);
                        }
                        for trigger in &mut session.triggers {
                            if let Some(plain) = Self::try_decrypt(&key, trigger.response.as_str())
                            {
                                trigger.response = Secret::new(plain);
                            }
                        }
                    }
                    if let Some(plain) = Self::try_decrypt(&key, cfg.webdav_password.as_str()) {
                        cfg.webdav_password = Secret::new(plain);
                    }
                    // Clean up any duplicate history accumulated before #113,
                    // keeping the last (most recent) occurrence of each command.
                    dedup_keep_last(&mut cfg.command_history);
                    // `system` and `default` are display-only group names. Older
                    // builds allowed moving saved servers into `system`, creating
                    // a duplicate empty-menu folder (#316, #324).
                    migrated |= normalize_reserved_session_groups(&mut cfg);
                    // One-time push of the new default layout to existing users
                    // (only for items they never changed). (#new-user-defaults)
                    migrated |= migrate_defaults(&mut cfg);
                    cfg
                }
                Err(err) => {
                    let backup = path.with_extension("json.broken");
                    let _ = fs::rename(&path, &backup);
                    tracing::warn!(
                        "config file was corrupt ({err}); backed up to {}",
                        backup.display()
                    );
                    fresh_config()
                }
            }
        } else {
            fresh_config()
        };

        let store = Self {
            path,
            cache,
            key,
        };
        // Persist the migration so it runs exactly once (and so a later opt-out —
        // e.g. turning the welcome sidebar back off — isn't reverted next launch).
        if migrated {
            if let Err(e) = store.save() {
                tracing::warn!("failed to persist default-layout migration: {e:#}");
            }
        }
        Ok(store)
    }

    fn config_path() -> Result<PathBuf> {
        Ok(crate::datastore::config_dir().join("sessions.json"))
    }

    pub fn sessions(&self) -> &[Session] {
        &self.cache.sessions
    }

    /// Drag-to-reorder a saved session (`dir < 0` = up). The stored Vec order
    /// is the display order within a group (same convention as quick commands).
    /// Same-group hops swap neighbours; when there is no same-group neighbour
    /// the hop crosses the group boundary instead: the session moves into the
    /// nearest visible display group along `dir`, landing at that group's
    /// boundary (first member moving down, last moving up). Returns whether
    /// anything changed.
    pub fn reorder_session(&mut self, id: &str, dir: isize) -> bool {
        // Display group via display_group_of (single source of truth, mirrors
        // build_session_rows in src/app/session_models.rs).
        let Some(idx) = self.cache.sessions.iter().position(|s| s.id == id) else {
            return false;
        };
        let group = display_group_of(&self.cache.sessions[idx]);

        // Same-group neighbour in stored (= display) order → plain swap.
        let same_group_target = {
            let sessions = &self.cache.sessions;
            if dir < 0 {
                (0..idx)
                    .rev()
                    .find(|&i| display_group_of(&sessions[i]) == group)
            } else {
                (idx + 1..sessions.len()).find(|&i| display_group_of(&sessions[i]) == group)
            }
        };
        if let Some(target) = same_group_target {
            self.cache.sessions.swap(idx, target);
            return true;
        }

        // Cross-group hop. Display order: "default" first (only when ungrouped
        // sessions exist), then named groups — explicit folders ∪ sessions'
        // groups — alphabetically. Collapsed groups are skipped: their rows
        // are hidden, so a card must never land inside one.
        let Some(collapsed_groups) = self.cache.collapsed_session_groups.clone() else {
            // Mirrors build_session_rows: no collapse list = everything
            // collapsed, so no visible target group can exist.
            return false;
        };
        let is_collapsed = |name: &str| collapsed_groups.iter().any(|g| g == name);

        let mut display: Vec<String> = Vec::new();
        if self
            .cache
            .sessions
            .iter()
            .any(|s| display_group_of(s) == "default")
        {
            display.push("default".to_string());
        }
        display.extend(named_display_groups(
            &self.cache.groups,
            &self.cache.sessions,
        ));

        let Some(pos) = display.iter().position(|g| g == &group) else {
            return false;
        };
        let target_group = if dir < 0 {
            (0..pos).rev().find(|&i| !is_collapsed(&display[i]))
        } else {
            (pos + 1..display.len()).find(|&i| !is_collapsed(&display[i]))
        }
        .map(|i| display[i].clone());
        let Some(target_group) = target_group else {
            return false;
        };

        let source_group = self.cache.sessions[idx].group.clone();
        let mut moved = self.cache.sessions.remove(idx);
        moved.group = if target_group == "default" {
            String::new()
        } else {
            target_group.clone()
        };
        let insert_at = {
            let members: Vec<usize> = self
                .cache
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| display_group_of(s) == target_group)
                .map(|(i, _)| i)
                .collect();
            if members.is_empty() {
                idx.min(self.cache.sessions.len())
            } else if dir < 0 {
                members[members.len() - 1] + 1
            } else {
                members[0]
            }
        };
        self.cache.sessions.insert(insert_at, moved);

        // A named source group that just lost its last member would vanish
        // from the list (changing the row count mid-drag, which drops the
        // dragging row's pointer grab); keep it as an empty folder. Register
        // it in `groups` directly rather than via add_group: the folder was
        // visibly expanded during the drag, and add_group would collapse it.
        if group != "default"
            && !self.cache.sessions.iter().any(|s| s.group == source_group)
            && !self.cache.groups.iter().any(|g| g == &source_group)
        {
            self.cache.groups.push(source_group);
        }
        true
    }

    /// Move session `id` to sit directly before/after `target_id` in stored
    /// order, inheriting the target's display group (#drag-ghost-pointer
    /// 2026-09-06). Replaces the dn/gi/size slot protocol: the Slint side now
    /// resolves the drop against real row geometry (hovered row + upper/lower
    /// half), so same-group reordering and cross-group moves share one precise,
    /// geometry-driven path. A source group emptied by the move is kept as an
    /// explicit empty folder (same convention as reorder_session).
    pub fn move_session_relative(&mut self, id: &str, target_id: &str, after: bool) -> bool {
        if id == target_id {
            return false;
        }
        let Some(idx) = self.cache.sessions.iter().position(|s| s.id == id) else {
            return false;
        };
        let Some(t_idx) = self.cache.sessions.iter().position(|s| s.id == target_id) else {
            return false;
        };
        let source_group = self.cache.sessions[idx].group.clone();
        let mut moved = self.cache.sessions.remove(idx);
        // Target index shifts left when the dragged row sat before it.
        let t_idx = if idx < t_idx { t_idx - 1 } else { t_idx };
        moved.group = self.cache.sessions[t_idx].group.clone();
        let insert_at = t_idx + usize::from(after);
        self.cache.sessions.insert(insert_at, moved);

        // A named source group that just lost its last member would vanish
        // from the list; keep it as an empty explicit folder (mirrors
        // reorder_session).
        let display_source = if source_group.is_empty()
            || is_reserved_session_group(source_group.trim())
        {
            "default".to_string()
        } else {
            source_group.clone()
        };
        if display_source != "default"
            && !self.cache.sessions.iter().any(|s| s.group == source_group)
            && !self.cache.groups.iter().any(|g| g == &source_group)
        {
            self.cache.groups.push(source_group);
        }
        true
    }

    /// (#group-manager) 批量换组：按 `ids` 给定顺序整体追加到目标组末尾。
    ///
    /// 调用方一次 borrow + 一次 save；`ids` 必须是**显示顺序**（调用方按
    /// gm-members 模型过滤 picked 得到），否则目标组内的相对次序会乱。
    /// 返回是否有会话被移动。
    pub fn move_sessions_to_group(&mut self, ids: &[String], target: &str) -> bool {
        if ids.is_empty() {
            return false;
        }
        let target = target.trim();
        let mut moved = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(pos) = self.cache.sessions.iter().position(|s| &s.id == id) {
                moved.push(self.cache.sessions.remove(pos));
            }
        }
        if moved.is_empty() {
            return false;
        }
        for session in &mut moved {
            session.group = target.to_string();
        }
        // 目标组末尾：最后一个同组成员之后；组不存在时落到全局末尾。
        let insert_at = self
            .cache
            .sessions
            .iter()
            .rposition(|s| s.group == target)
            .map(|i| i + 1)
            .unwrap_or(self.cache.sessions.len());
        self.cache.sessions.splice(insert_at..insert_at, moved);
        true
    }

    /// (#group-manager) 批量组内排序：把 `ids` 作为一个整体插到 `target_id`
    /// 前/后，块内保持 `ids` 的顺序，块外次序不变。
    ///
    /// 这是「拖多行排序」必须一次完成的操作 —— 逐条 `move_session_relative`
    /// 会让块内顺序随手势路径变化，且每次都要重建列表。
    pub fn move_sessions_relative(
        &mut self,
        ids: &[String],
        target_id: &str,
        after: bool,
    ) -> bool {
        if ids.is_empty() || ids.iter().any(|id| id == target_id) {
            return false;
        }
        let Some(target_group) = self
            .cache
            .sessions
            .iter()
            .find(|s| s.id == target_id)
            .map(|s| s.group.clone())
        else {
            return false;
        };
        let mut block = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(pos) = self.cache.sessions.iter().position(|s| &s.id == id) {
                block.push(self.cache.sessions.remove(pos));
            }
        }
        if block.is_empty() {
            return false;
        }
        for session in &mut block {
            session.group = target_group.clone();
        }
        // 摘出块后重找目标下标（前面的元素被移除会前移）。
        let t_idx = self
            .cache
            .sessions
            .iter()
            .position(|s| s.id == target_id)
            .unwrap_or(self.cache.sessions.len());
        let insert_at = t_idx + usize::from(after);
        self.cache.sessions.splice(insert_at..insert_at, block);
        true
    }

    pub fn upsert(&mut self, mut session: Session) {
        if is_reserved_session_group(session.group.trim()) {
            session.group.clear();
        }
        if let Some(existing) = self.cache.sessions.iter_mut().find(|s| s.id == session.id) {
            *existing = session;
        } else {
            self.cache.sessions.push(session);
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.cache.sessions.retain(|s| s.id != id);
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.cache.sessions.iter().find(|s| s.id == id)
    }

    pub fn download_dir(&self) -> &str {
        &self.cache.download_dir
    }

    pub fn set_download_dir(&mut self, dir: String) {
        self.cache.download_dir = dir;
    }

    /// UI language code ("zh" default / "en").
    pub fn language(&self) -> &str {
        if self.cache.language.is_empty() {
            "zh"
        } else {
            &self.cache.language
        }
    }

    pub fn set_language(&mut self, lang: String) {
        self.cache.language = lang;
    }

    /// Theme preference: "system" (default) | "dark" | "light".
    pub fn theme_pref(&self) -> &str {
        if self.cache.theme_pref.is_empty() {
            "system"
        } else {
            &self.cache.theme_pref
        }
    }

    pub fn set_theme_pref(&mut self, pref: String) {
        self.cache.theme_pref = pref;
    }

    // ── (#theme-split 2026-10-07) 主题/强调色/壁纸三层解耦的新字段 ──────

    /// 主题 id。空 = 跟随系统（follow_system）。合法值见 src/theme/palettes.rs。
    pub fn theme(&self) -> &str {
        &self.cache.theme
    }
    pub fn set_theme(&mut self, id: String) {
        self.cache.theme = id;
    }
    pub fn follow_system(&self) -> bool {
        self.cache.follow_system
    }
    pub fn set_follow_system(&mut self, on: bool) {
        self.cache.follow_system = on;
    }
    /// 「常用主题」id 列表（#theme-dedupe：速选面板常用区；上限 8 由回调侧校验）。
    pub fn favorite_themes(&self) -> Vec<String> {
        self.cache.favorite_themes.clone()
    }
    pub fn set_favorite_themes(&mut self, list: Vec<String>) {
        self.cache.favorite_themes = list;
    }
    /// 强调色三态："follow" | "preset" | "custom"。
    pub fn accent_mode(&self) -> &str {
        if self.cache.accent_mode.is_empty() {
            "follow"
        } else {
            &self.cache.accent_mode
        }
    }
    pub fn set_accent_mode(&mut self, mode: String) {
        self.cache.accent_mode = mode;
    }
    pub fn accent_preset(&self) -> &str {
        &self.cache.accent_preset
    }
    pub fn set_accent_preset(&mut self, key: String) {
        self.cache.accent_preset = key;
    }
    pub fn accent_custom(&self) -> &str {
        &self.cache.accent_custom
    }
    pub fn set_accent_custom(&mut self, hex: String) {
        self.cache.accent_custom = hex;
    }
    /// (#mode-values 2026-10-17) 外观数值：**暗 / 亮各一套**。
    /// 读当前模式用无参访问器；写入必须显式给 `dark`（= 用户此刻看到的模式），
    /// 免得"按 A 算、按 B 存"。
    pub fn appearance_for(&self, dark: bool) -> &AppearanceValues {
        if dark {
            &self.cache.appearance_dark
        } else {
            &self.cache.appearance_light
        }
    }
    /// 当前主题是否暗色（与 `theme::resolve` 判据同源：直接看色表的 dark 列）。
    pub fn is_dark_theme(&self) -> bool {
        crate::theme::by_id(self.theme()).map(|p| p.dark).unwrap_or(true)
    }

    /// (#wallpaper-scope-params 2026-10-18) 分层存储用的主题键。
    /// `theme` 为空（只在迁移前的瞬间出现）时按明暗落到 "dark" / "light"，
    /// 保证任何时刻都有稳定的槽位。
    fn theme_key(&self) -> String {
        if self.cache.theme.is_empty() {
            if self.is_dark_theme() {
                "dark".to_string()
            } else {
                "light".to_string()
            }
        } else {
            self.cache.theme.clone()
        }
    }

    /// 当前主题的壁纸 id（空 = 该主题没有壁纸）。
    fn theme_wallpaper(&self) -> String {
        self.cache
            .wallpaper_by_theme
            .get(&self.theme_key())
            .cloned()
            .unwrap_or_default()
    }

    /// 当前主题有没有壁纸（决定弹框/遮罩用哪一档）。
    fn has_wallpaper(&self) -> bool {
        !self.theme_wallpaper().is_empty()
    }

    /// (#theme-appearance 2026-10-19) 当前主题「无壁纸」档的完整快照。
    ///
    /// 没有条目 → 回落旧的暗/亮槽 + 旧全局遮罩样式（老配置零迁移）。
    fn theme_plain(&self) -> AppearanceValues {
        match self.cache.appearance_by_theme.get(&self.theme_key()) {
            Some(t) => t.plain.clone(),
            None => {
                let mut av = self.appearance_for(self.is_dark_theme()).clone();
                av.scrim_style = self.cache.scrim_style.clone();
                av
            }
        }
    }

    /// 取/建当前主题的条目。首次编辑时两档都从当前值起步，
    /// 不会因为「刚分家」而跳回出厂默认。
    fn theme_appearance_mut(&mut self) -> &mut ThemeAppearance {
        let theme = self.theme_key();
        if !self.cache.appearance_by_theme.contains_key(&theme) {
            let plain = self.theme_plain();
            let entry = ThemeAppearance {
                wallpaper: PopupAppearance {
                    popup_transparency: plain.popup_transparency,
                    scrim_alpha: plain.scrim_alpha,
                    scrim_style: plain.scrim_style.clone(),
                },
                plain,
            };
            self.cache.appearance_by_theme.insert(theme.clone(), entry);
        }
        self.cache
            .appearance_by_theme
            .get_mut(&theme)
            .expect("theme appearance inserted above")
    }

    /// 当前 (主题, 壁纸, 作用域) 的键；没有壁纸时返回 None。
    fn current_scope_key(&self) -> Option<String> {
        let wallpaper = self.theme_wallpaper();
        if wallpaper.is_empty() {
            return None;
        }
        Some(wallpaper_key(
            &self.theme_key(),
            &wallpaper,
            self.wallpaper_scope(),
        ))
    }

    /// 当前作用域的参数（没有壁纸 → None，调用方回落到「明暗各一套」的旧值）。
    pub fn scope_settings(&self) -> Option<ScopeSettings> {
        self.current_scope_key()
            .map(|k| {
                self.cache
                    .wallpaper_settings
                    .get(&k)
                    .cloned()
                    .unwrap_or_else(|| default_scope_settings(self.wallpaper_scope()))
            })
    }

    /// (#wallpaper-scope-params) 该 (主题, 壁纸, 作用域) 是否已经落过盘。
    pub fn scope_settings_saved(&self) -> bool {
        self.current_scope_key()
            .map(|k| self.cache.wallpaper_settings.contains_key(&k))
            .unwrap_or(false)
    }

    /// (#zone-default-context 2026-10-19) 当前「主题 × 壁纸 × 作用域」下，
    /// 四根分区滑杆**双击恢复**的目标值（alpha = 1 − 透明度）。
    ///
    /// 有壁纸 → 该组合的首设默认值（用户定稿：「没有壁纸和有壁纸的默认值是
    /// 不一样的」）；没有壁纸 → 1.0（= 0% 透明，主题原色）。
    /// 设置页的「(默认)」标记与双击复位都读这一份，避免两处各写各的。
    pub fn default_zone_alphas(&self) -> (f32, f32, f32, f32) {
        match self.scope_settings() {
            Some(_) => {
                let d = default_scope_settings(self.wallpaper_scope());
                (
                    d.titlebar_alpha,
                    d.tabbar_alpha,
                    d.sidebar_alpha,
                    d.term_alpha,
                )
            }
            None => (1.0, 1.0, 1.0, 1.0),
        }
    }

    /// 改当前作用域的参数（没有壁纸时返回 false，调用方写回明暗槽）。
    fn with_scope_settings(&mut self, f: impl FnOnce(&mut ScopeSettings)) -> bool {
        let Some(key) = self.current_scope_key() else {
            return false;
        };
        let scope = self.wallpaper_scope().to_string();
        let entry = self
            .cache
            .wallpaper_settings
            .entry(key)
            .or_insert_with(|| default_scope_settings(&scope));
        f(entry);
        true
    }

    /// 当前生效的一整套外观数值：有壁纸时用 (主题, 壁纸, 作用域) 那一套，
    /// 没有壁纸时用该主题自己的基础值（`theme_plain`）。
    pub fn effective_appearance(&self) -> AppearanceValues {
        // (#theme-appearance 2026-10-19) 基础值按**主题**取 —— 四根滑杆与
        // 弹框/遮罩都不再跨主题共享（旧实现按明暗两槽，同模式主题互相串）。
        let mut av = self.theme_plain();
        if let Some(p) = self.scope_settings() {
            av.titlebar_alpha = p.titlebar_alpha;
            av.tabbar_alpha = p.tabbar_alpha;
            av.sidebar_alpha = p.sidebar_alpha;
            av.term_alpha = p.term_alpha;
            av.wallpaper_transparency = p.wallpaper_transparency;
        }
        // 弹框/遮罩按「有无壁纸」两档；有壁纸且该主题还没分过家 → 沿用无壁纸档。
        if self.has_wallpaper() {
            if let Some(t) = self.cache.appearance_by_theme.get(&self.theme_key()) {
                av.popup_transparency = t.wallpaper.popup_transparency;
                av.scrim_alpha = t.wallpaper.scrim_alpha;
                av.scrim_style = t.wallpaper.scrim_style.clone();
            }
        }
        av
    }

    pub fn titlebar_alpha(&self) -> f32 {
        self.effective_appearance().titlebar_alpha
    }
    pub fn set_titlebar_alpha(&mut self, v: f32, _dark: bool) {
        let v = v.clamp(0.0, 1.0);
        if !self.with_scope_settings(|p| p.titlebar_alpha = v) {
            self.theme_appearance_mut().plain.titlebar_alpha = v;
        }
    }
    pub fn tabbar_alpha(&self) -> f32 {
        self.effective_appearance().tabbar_alpha
    }
    pub fn set_tabbar_alpha(&mut self, v: f32, _dark: bool) {
        let v = v.clamp(0.0, 1.0);
        if !self.with_scope_settings(|p| p.tabbar_alpha = v) {
            self.theme_appearance_mut().plain.tabbar_alpha = v;
        }
    }
    pub fn sidebar_alpha(&self) -> f32 {
        self.effective_appearance().sidebar_alpha
    }
    pub fn set_sidebar_alpha(&mut self, v: f32, _dark: bool) {
        let v = v.clamp(0.0, 1.0);
        if !self.with_scope_settings(|p| p.sidebar_alpha = v) {
            self.theme_appearance_mut().plain.sidebar_alpha = v;
        }
    }
    pub fn term_alpha(&self) -> f32 {
        self.effective_appearance().term_alpha
    }
    pub fn set_term_alpha(&mut self, v: f32, _dark: bool) {
        let v = v.clamp(0.0, 1.0);
        if !self.with_scope_settings(|p| p.term_alpha = v) {
            self.theme_appearance_mut().plain.term_alpha = v;
        }
    }
    /// (#wallpaper-transparency 2026-10-17) 壁纸透明度（0 = 完全显示）。
    /// (#wallpaper-scope-params) 现在属于「当前壁纸 × 当前作用域」。
    pub fn wallpaper_transparency(&self) -> f32 {
        self.effective_appearance().wallpaper_transparency
    }
    pub fn set_wallpaper_transparency(&mut self, v: f32, _dark: bool) {
        let v = v.clamp(0.0, 1.0);
        if !self.with_scope_settings(|p| p.wallpaper_transparency = v) {
            self.theme_appearance_mut().plain.wallpaper_transparency = v;
        }
    }
    /// 壁纸作用域："window"（整个界面，默认）| "workspace"（主区域，标题栏以下）
    /// | "terminal"（终端列）；每个主题各记一份。
    ///
    /// 无壁纸期间一律报默认的 `"window"`（作用域此时没有意义，设置页也该显示
    /// 默认档），但**不会**改写存下的值 —— 再选壁纸时用户原来的档位回来。
    pub fn wallpaper_scope(&self) -> &str {
        let theme = self.theme_key();
        // (#wallpaper-none-default 2026-10-19) 没有壁纸就不该报出旧作用域 ——
        // 设置页在「无壁纸」下必须显示默认的「整个界面」。
        if self
            .cache
            .wallpaper_by_theme
            .get(&theme)
            .map(|w| w.is_empty())
            .unwrap_or(true)
        {
            return "window";
        }
        self.cache
            .wallpaper_scope_by_theme
            .get(&theme)
            .map(String::as_str)
            .unwrap_or("window")
    }
    pub fn set_wallpaper_scope(&mut self, v: String) {
        // (#wallpaper-scope-workspace 2026-10-19) 白名单必须显式列全：
        // 旧实现只认 terminal，其余一律 window —— 新增的 workspace 会被静默吃掉。
        let v = match v.as_str() {
            "workspace" => "workspace".to_string(),
            "terminal" => "terminal".to_string(),
            _ => "window".to_string(),
        };
        let theme = self.theme_key();
        self.cache
            .wallpaper_scope_by_theme
            .insert(theme, v.clone());
        // 兜底：该主题下所有壁纸都补上这一作用域的首设参数，切过去不会是
        // 「一片黑」（见 default_scope_settings 的注释）。
        self.ensure_scope_defaults(&v);
        // 旧全局字段同步（老版本读它；新版本不再读）。
        self.cache.wallpaper_scope = v;
    }
    /// 壁纸填充方式："auto"（自动，默认）| "cover" | "contain" | "fill"。
    /// 返回 `String`：它现在属于 (主题, 壁纸, 作用域) 那一套，可能是临时兜底值。
    pub fn wallpaper_fit(&self) -> String {
        match self.scope_settings() {
            Some(p) => p.fit,
            None => self.cache.wallpaper_fit.clone(),
        }
    }
    pub fn set_wallpaper_fit(&mut self, v: String) {
        let v = match v.as_str() {
            "auto" => "auto".to_string(),
            "contain" => "contain".to_string(),
            "fill" => "fill".to_string(),
            _ => "cover".to_string(),
        };
        if !self.with_scope_settings(|p| p.fit = v.clone()) {
            self.cache.wallpaper_fit = v;
        }
    }

    /// (#wallpaper-first-defaults) 给当前主题下的每张壁纸补齐某作用域的
    /// 首设参数（只在缺条目时写，调过的一律不动）。
    fn ensure_scope_defaults(&mut self, scope: &str) {
        let theme = self.theme_key();
        let wallpapers: Vec<String> = self
            .cache
            .wallpaper_by_theme
            .get(&theme)
            .into_iter()
            .flat_map(|cur| {
                let mut all = vec![cur.clone()];
                for k in self.cache.wallpaper_settings.keys() {
                    let mut parts = k.split('|');
                    if parts.next() == Some(theme.as_str()) {
                        if let Some(wp) = parts.next() {
                            all.push(wp.to_string());
                        }
                    }
                }
                all
            })
            .filter(|w| !w.is_empty())
            .collect();
        for wp in wallpapers {
            let key = wallpaper_key(&theme, &wp, scope);
            self.cache
                .wallpaper_settings
                .entry(key)
                .or_insert_with(|| default_scope_settings(scope));
        }
    }
    /// 遮罩样式（9 选）。(#theme-appearance 2026-10-19) 按主题 × 有无壁纸各记一份。
    pub fn scrim_style(&self) -> String {
        self.effective_appearance().scrim_style
    }
    pub fn set_scrim_style(&mut self, v: String) {
        let v = match v.as_str() {
            "dark" | "light" | "tint" | "soft" | "frost" | "spot" | "sheen"
            | "random" => v,
            _ => "auto".to_string(),
        };
        // 改哪一档就只写哪一档；另一档保持自己的值。
        if self.has_wallpaper() {
            self.theme_appearance_mut().wallpaper.scrim_style = v;
        } else {
            self.theme_appearance_mut().plain.scrim_style = v;
        }
    }
    /// 遮罩浓度（0.00–1.00）。
    pub fn scrim_alpha(&self) -> f32 {
        self.effective_appearance().scrim_alpha
    }
    pub fn set_scrim_alpha(&mut self, v: f32, _dark: bool) {
        let v = v.clamp(0.0, 1.0);
        if self.has_wallpaper() {
            self.theme_appearance_mut().wallpaper.scrim_alpha = v;
        } else {
            self.theme_appearance_mut().plain.scrim_alpha = v;
        }
    }
    /// 弹窗透明度（0 = 实心）。
    pub fn popup_transparency(&self) -> f32 {
        self.effective_appearance()
            .popup_transparency
            .clamp(0.0, 1.0)
    }
    pub fn set_popup_transparency(&mut self, v: f32, _dark: bool) {
        let v = v.clamp(0.0, 1.0);
        if self.has_wallpaper() {
            self.theme_appearance_mut().wallpaper.popup_transparency = v;
        } else {
            self.theme_appearance_mut().plain.popup_transparency = v;
        }
    }
    /// 「跟随图片取色」（默认关——唯一让壁纸反向影响界面的开关）。
    pub fn wallpaper_color_pickup(&self) -> bool {
        self.cache.wallpaper_color_pickup
    }
    pub fn set_wallpaper_color_pickup(&mut self, on: bool) {
        self.cache.wallpaper_color_pickup = on;
    }
    /// 频道成员表：4 槽各一个会话稳定 id 列表。
    pub fn channel_members(&self) -> &[Vec<String>; 4] {
        &self.cache.channel_members
    }
    /// 加入频道槽（幂等：已在表内则不动）。换频道前必须先 detach_all。
    pub fn set_channel_member(&mut self, slot: usize, session_id: &str) {
        if slot < 4 && !self.cache.channel_members[slot].iter().any(|m| m == session_id) {
            self.cache.channel_members[slot].push(session_id.to_string());
        }
    }
    /// 把会话从全部 4 个频道清一遍（换频道是「移动」不是「复制」，§1：
    /// 同一实体只能属于一处时，写入前从全部容器清一遍）。
    pub fn detach_all_channels(&mut self, session_id: &str) {
        for slot in self.cache.channel_members.iter_mut() {
            slot.retain(|m| m != session_id);
        }
    }
    /// 清空指定频道的成员（关闭频道只清这个频道的成员，别整表清）。
    pub fn clear_channel(&mut self, slot: usize) {
        if slot < 4 {
            self.cache.channel_members[slot].clear();
        }
    }
    /// 已暂停参与频道的会话 id 集合（按会话粒度）。
    pub fn channel_paused(&self) -> &Vec<String> {
        &self.cache.channel_paused
    }
    pub fn channel_is_paused(&self, session_id: &str) -> bool {
        self.cache.channel_paused.iter().any(|s| s == session_id)
    }
    pub fn set_channel_paused(&mut self, session_id: &str, paused: bool) {
        if paused {
            if !self.channel_is_paused(session_id) {
                self.cache.channel_paused.push(session_id.to_string());
            }
        } else {
            self.cache.channel_paused.retain(|s| s != session_id);
        }
    }
    /// 清掉某会话的暂停标记（离开频道/关闭 tab 时——用户已忘了暂停过，
    /// 留着会在重新加入时莫名多一道删除线）。
    pub fn clear_channel_pause(&mut self, session_id: &str) {
        self.set_channel_paused(session_id, false);
    }

    /// Renderer preference for the current platform.
    #[cfg(target_os = "macos")]
    pub fn renderer_mode(&self) -> &str {
        normalize_macos_renderer_mode(&self.cache.renderer_mode)
    }

    /// Missing and invalid Windows values deliberately use software so upgrades
    /// preserve the high-DPI/VM compatibility from #224.
    #[cfg(target_os = "windows")]
    pub fn renderer_mode(&self) -> &str {
        match self.cache.renderer_mode.as_str() {
            "auto" => "auto",
            "gpu" => "gpu",
            _ => "software",
        }
    }

    #[cfg(target_os = "macos")]
    pub fn set_renderer_mode(&mut self, mode: String) {
        self.cache.renderer_mode = normalize_macos_renderer_mode(&mode).into();
    }

    /// Linux previously used Slint's automatic renderer selection and had no
    /// settings entry. Keep that behaviour for existing configurations.
    #[cfg(target_os = "linux")]
    pub fn renderer_mode(&self) -> &str {
        match self.cache.renderer_mode.as_str() {
            "gpu" => "gpu",
            "software" => "software",
            _ => "auto",
        }
    }

    #[cfg(target_os = "windows")]
    pub fn set_renderer_mode(&mut self, mode: String) {
        self.cache.renderer_mode = match mode.as_str() {
            "auto" => "auto".into(),
            "gpu" => "gpu".into(),
            _ => "software".into(),
        };
    }

    #[cfg(target_os = "linux")]
    pub fn set_renderer_mode(&mut self, mode: String) {
        self.cache.renderer_mode = match mode.as_str() {
            "gpu" => "gpu".into(),
            "software" => "software".into(),
            _ => "auto".into(),
        };
    }

    /// UI (sans) font family ("" = first usable system CJK family from the chain).
    pub fn ui_font_family(&self) -> &str {
        &self.cache.ui_font_family
    }

    pub fn set_ui_font_family(&mut self, family: String) {
        self.cache.ui_font_family = family;
    }

    /// Terminal font family ("" = built-in default).
    pub fn font_family(&self) -> &str {
        &self.cache.font_family
    }

    pub fn set_font_family(&mut self, family: String) {
        self.cache.font_family = family;
    }

    /// Terminal font size in px (falls back to 13 when unset).
    pub fn font_size(&self) -> u32 {
        if self.cache.font_size == 0 {
            13
        } else {
            self.cache.font_size
        }
    }

    pub fn set_font_size(&mut self, size: u32) {
        self.cache.font_size = size.clamp(8, 32);
    }

    pub fn terminal_line_spacing(&self) -> f32 {
        let value = self.cache.terminal_line_spacing;
        if value <= 0.0 {
            1.0
        } else {
            value.clamp(0.8, 1.5)
        }
    }

    pub fn set_terminal_line_spacing(&mut self, value: f32) {
        self.cache.terminal_line_spacing = value.clamp(0.8, 1.5);
    }

    pub fn paste_confirm_enabled(&self) -> bool {
        !self.cache.paste_confirm_disabled
    }

    pub fn set_paste_confirm_enabled(&mut self, enabled: bool) {
        self.cache.paste_confirm_disabled = !enabled;
    }

    /// (#close-behavior) 点窗口关闭键时的行为。旧配置里该字段为空 →
    /// 归一化为 "ask"(每次询问),保持既有行为不变。
    pub fn close_behavior(&self) -> &'static str {
        match self.cache.close_behavior.as_str() {
            "tray" => "tray",
            "exit" => "exit",
            _ => "ask",
        }
    }

    /// 写入关闭行为;非法值一律落到 "ask"。
    pub fn set_close_behavior(&mut self, value: &str) {
        self.cache.close_behavior = match value {
            "tray" => "tray",
            "exit" => "exit",
            _ => "ask",
        }
        .to_string();
    }

    pub fn extra_paste_shortcuts_enabled(&self) -> bool {
        !self.cache.extra_paste_shortcuts_disabled
    }

    pub fn set_extra_paste_shortcuts_enabled(&mut self, enabled: bool) {
        self.cache.extra_paste_shortcuts_disabled = !enabled;
    }

    pub fn zen_mode(&self) -> bool {
        self.cache.zen_mode
    }

    pub fn set_zen_mode(&mut self, enabled: bool) {
        self.cache.zen_mode = enabled;
    }

    /// Force regular terminal text to render with a bold face (#262).
    pub fn terminal_bold(&self) -> bool {
        self.cache.terminal_bold
    }

    pub fn set_terminal_bold(&mut self, bold: bool) {
        self.cache.terminal_bold = bold;
    }

    /// Selected terminal insertion cursor shape. Legacy and invalid values use
    /// the existing block cursor so upgrades preserve the current appearance.
    pub fn terminal_cursor_style(&self) -> &str {
        match self.cache.terminal_cursor_style.as_str() {
            "bar" => "bar",
            "underline" => "underline",
            _ => "block",
        }
    }

    pub fn set_terminal_cursor_style(&mut self, style: String) {
        self.cache.terminal_cursor_style = match style.as_str() {
            "bar" => "bar".into(),
            "underline" => "underline".into(),
            _ => "block".into(),
        };
    }

    pub fn terminal_cursor_color(&self) -> &str {
        if normalize_hex_color(&self.cache.terminal_cursor_color).is_some() {
            &self.cache.terminal_cursor_color
        } else {
            ""
        }
    }

    pub fn set_terminal_cursor_color(&mut self, color: &str) -> bool {
        let Some(normalized) = normalize_hex_color(color) else {
            return false;
        };
        self.cache.terminal_cursor_color = normalized;
        true
    }

    /// Whether client-side highlighting of otherwise unstyled output is active.
    pub fn output_highlight_enabled(&self) -> bool {
        !self.cache.output_highlight_disabled
    }

    pub fn set_output_highlight_enabled(&mut self, enabled: bool) {
        self.cache.output_highlight_disabled = !enabled;
    }

    pub fn json_format_output(&self) -> bool {
        !self.cache.json_format_disabled
    }

    pub fn set_json_format_output(&mut self, enabled: bool) {
        self.cache.json_format_disabled = !enabled;
    }

    /// 被单独关掉的预设规则 id（其余默认全开）。
    pub fn output_highlight_disabled_presets(&self) -> &[String] {
        &self.cache.output_highlight_disabled_presets
    }

    /// 逐条开关预设规则（彩虹括号 id = "paren"）。写坏的值原样存，
    /// 编译期按表匹配，未知 id 自然无效。
    pub fn set_output_highlight_preset_enabled(&mut self, id: &str, enabled: bool) {
        let pos = self
            .cache
            .output_highlight_disabled_presets
            .iter()
            .position(|d| d == id);
        if enabled {
            if let Some(i) = pos {
                self.cache.output_highlight_disabled_presets.remove(i);
            }
        } else if pos.is_none() {
            self.cache
                .output_highlight_disabled_presets
                .push(id.to_string());
        }
    }

    pub fn output_highlight_rules(&self) -> &[OutputHighlightRule] {
        &self.cache.output_highlight_rules
    }

    pub fn add_output_highlight_rule(&mut self, mut rule: OutputHighlightRule) {
        rule.pattern = rule.pattern.trim().to_string();
        rule.name = rule.name.trim().to_string();
        rule.color = normalize_highlight_color(&rule.color);
        self.cache.output_highlight_rules.push(rule);
    }

    pub fn remove_output_highlight_rule(&mut self, index: usize) {
        if index < self.cache.output_highlight_rules.len() {
            self.cache.output_highlight_rules.remove(index);
        }
    }

    pub fn set_output_highlight_rule_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(rule) = self.cache.output_highlight_rules.get_mut(index) {
            rule.enabled = enabled;
        }
    }

    /// Global UI scale in percent (#100). Defaults to 100.
    pub fn ui_scale(&self) -> u32 {
        if self.cache.ui_scale == 0 {
            100
        } else {
            self.cache.ui_scale
        }
    }

    pub fn set_ui_scale(&mut self, percent: u32) {
        self.cache.ui_scale = percent.clamp(80, 200);
    }

    /// 当前主题的壁纸 id（空 = 该主题没有壁纸）。
    ///
    /// (#wallpaper-scope-params 2026-10-18) 壁纸选择从「全局一个」改成
    /// **每个主题各记一份**：切主题不再把上一套壁纸/参数带过去。
    pub fn wallpaper(&self) -> &str {
        self.cache
            .wallpaper_by_theme
            .get(&self.theme_key())
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn set_wallpaper(&mut self, id: impl Into<String>) {
        let id = id.into();
        let theme = self.theme_key();
        self.cache
            .wallpaper_by_theme
            .insert(theme.clone(), id.clone());
        // 老字段留一份镜像（老版本 / 迁移只读它）。
        self.cache.wallpaper = id.clone();
        if id.is_empty() {
            // (#wallpaper-none-default 2026-10-19) 切「无壁纸」**只影响显示**：
            // 作用域在无壁纸期间按默认档呈现（见 `wallpaper_scope()`），但这里
            // **不能清掉**用户存下的选择 —— 再切回这张壁纸时，作用域与那套
            // 分区参数都要原样回来（用户调过的值必须保留）。
            return;
        }
        // (#wallpaper-first-defaults) **首次**设这张壁纸：两种作用域都先落一套
        // 「看得见壁纸」的默认值（四区 + 壁纸透明度 + 填充）。已有条目不动 ——
        // 用户调过的值必须原样保留。
        for scope in ["window", "terminal"] {
            let key = wallpaper_key(&theme, &id, scope);
            self.cache
                .wallpaper_settings
                .entry(key)
                .or_insert_with(|| default_scope_settings(scope));
        }
    }

    /// 删除壁纸文件后彻底清掉它在**所有主题**下的数据：
    /// ① `<theme>|<id>|<scope>` 的全部参数条目（分区透明度 / 壁纸透明度 / 填充）；
    /// ② 用它当壁纸的主题的选中项与作用域记录。
    ///
    /// 与 `set_wallpaper("")`（切到「无壁纸」）明确区分：那条路只改显示，用户
    /// 调过的档位和数值必须保留；**只有真正删除文件**才销毁这些数据。
    /// 返回删掉的参数条目数（调用方不关心，测试用来证明确实清过）。
    pub fn forget_wallpaper(&mut self, id: &str) -> usize {
        if id.is_empty() {
            return 0;
        }
        let before = self.cache.wallpaper_settings.len();
        self.cache
            .wallpaper_settings
            .retain(|key, _| !wallpaper_key_targets(key, id));
        let removed = before - self.cache.wallpaper_settings.len();

        let themes: Vec<String> = self
            .cache
            .wallpaper_by_theme
            .iter()
            .filter(|(_, w)| w.as_str() == id)
            .map(|(theme, _)| theme.clone())
            .collect();
        self.cache
            .wallpaper_by_theme
            .retain(|_, w| w.as_str() != id);
        for theme in themes {
            self.cache.wallpaper_scope_by_theme.remove(&theme);
        }
        // 老字段镜像（老版本只读它）：壁纸没了，作用域也回默认。
        if self.cache.wallpaper == id {
            self.cache.wallpaper.clear();
            self.cache.wallpaper_scope = "window".to_string();
        }
        removed
    }

    /// Whether the SFTP panel follows the terminal's cd (default true).
    pub fn sftp_follow_cd(&self) -> bool {
        !self.cache.sftp_no_follow_cd
    }

    pub fn set_sftp_follow_cd(&mut self, follow: bool) {
        self.cache.sftp_no_follow_cd = !follow;
    }

    /// Whether the quick-command bar under the terminal is hidden.
    pub fn cmd_bar_hidden(&self) -> bool {
        self.cache.hide_cmd_bar
    }

    pub fn set_cmd_bar_hidden(&mut self, hidden: bool) {
        self.cache.hide_cmd_bar = hidden;
    }

    /// Saved quick commands (#55).
    pub fn quick_commands(&self) -> &[QuickCommand] {
        &self.cache.quick_commands
    }

    pub fn set_quick_commands(&mut self, cmds: Vec<QuickCommand>) {
        self.cache.quick_commands = cmds;
    }

    pub fn wsl_profiles(&self) -> &[WslProfile] {
        &self.cache.wsl_profiles
    }

    pub fn add_wsl_profile(&mut self, name: String, distribution: String, directory: String) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.cache.wsl_profiles.push(WslProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            distribution: distribution.trim().to_string(),
            directory: match directory.trim() {
                "" => "~".to_string(),
                value => value.to_string(),
            },
        });
    }

    pub fn remove_wsl_profile(&mut self, id: &str) {
        self.cache.wsl_profiles.retain(|profile| profile.id != id);
    }

    pub fn quick_panel_open(&self) -> bool {
        self.cache.quick_panel_open
    }

    pub fn quick_commands_as_sidebar(&self) -> bool {
        self.cache.quick_commands_as_sidebar
    }

    pub fn set_quick_commands_as_sidebar(&mut self, enabled: bool) {
        self.cache.quick_commands_as_sidebar = enabled;
        if !enabled {
            self.cache.quick_panel_open = false;
        }
    }

    pub fn set_quick_panel_open(&mut self, open: bool) {
        self.cache.quick_panel_open = open;
    }

    pub fn quick_panel_collapsed(&self) -> bool {
        self.cache.quick_panel_collapsed
    }

    pub fn set_quick_panel_collapsed(&mut self, collapsed: bool) {
        self.cache.quick_panel_collapsed = collapsed;
    }

    pub fn quick_panel_width(&self) -> f32 {
        let width = self.cache.quick_panel_width;
        if width <= 0.0 {
            default_quick_panel_width()
        } else {
            width
        }
    }

    pub fn set_quick_panel_width(&mut self, width: f32) {
        self.cache.quick_panel_width = width;
    }

    pub fn quick_panel_height(&self) -> f32 {
        let height = self.cache.quick_panel_height;
        if height <= 0.0 {
            default_quick_panel_height()
        } else {
            height
        }
    }

    pub fn set_quick_panel_height(&mut self, height: f32) {
        self.cache.quick_panel_height = height;
    }

    pub fn quick_panel_dock(&self) -> String {
        match self.cache.quick_panel_dock.trim() {
            "left" | "right" | "top" | "bottom" => self.cache.quick_panel_dock.clone(),
            _ => "right".into(),
        }
    }

    pub fn set_quick_panel_dock(&mut self, dock: String) {
        self.cache.quick_panel_dock = dock;
    }

    /// Explicit quick-command groups (#55) — parallels [`groups`](Self::groups).
    pub fn quick_groups(&self) -> &[String] {
        &self.cache.quick_groups
    }

    /// Create an empty quick-command group. Ignores blank, "default", duplicates.
    pub fn add_quick_group(&mut self, name: String) {
        let n = name.trim().to_string();
        if n.is_empty() || n.eq_ignore_ascii_case("default") {
            return;
        }
        if !self.cache.quick_groups.iter().any(|g| g == &n) {
            self.cache.quick_groups.push(n);
        }
    }

    /// Delete a quick-command group; any command still in it falls back to
    /// ungrouped (the UI only offers delete on empty groups, but clear defensively).
    pub fn remove_quick_group(&mut self, name: &str) {
        self.cache.quick_groups.retain(|g| g != name);
        for c in &mut self.cache.quick_commands {
            if c.group == name {
                c.group.clear();
            }
        }
    }

    /// Rename a quick-command group, moving its commands along. No-op for
    /// blank / "default".
    pub fn rename_quick_group(&mut self, old: &str, new: String) {
        let n = new.trim().to_string();
        if n.is_empty() || n.eq_ignore_ascii_case("default") || n == old {
            return;
        }
        for g in &mut self.cache.quick_groups {
            if g == old {
                *g = n.clone();
            }
        }
        for c in &mut self.cache.quick_commands {
            if c.group == old {
                c.group = n.clone();
            }
        }
        self.cache.quick_groups.sort();
        self.cache.quick_groups.dedup();
    }

    /// Update one quick command in place by index (#55).
    pub fn update_quick_command(&mut self, index: usize, cmd: QuickCommand) {
        if let Some(slot) = self.cache.quick_commands.get_mut(index) {
            *slot = cmd;
        }
    }

    /// Recent command-box history, oldest first (#55).
    pub fn command_history(&self) -> &[String] {
        &self.cache.command_history
    }

    /// Append a command to the history: skips blanks, de-duplicates globally so
    /// each command appears once, and re-appends at the end so the most-recently
    /// used command is always last. Capped so it can't grow without bound (#113).
    pub fn push_command_history(&mut self, cmd: String) {
        if cmd.trim().is_empty() {
            return;
        }
        // Drop any earlier occurrence, then push → no duplicates and "last used"
        // moves to the end (bash `HISTCONTROL=erasedups` semantics).
        self.cache.command_history.retain(|c| c != &cmd);
        const CAP: usize = 200;
        self.cache.command_history.push(cmd);
        let len = self.cache.command_history.len();
        if len > CAP {
            self.cache.command_history.drain(0..len - CAP);
        }
    }

    /// Remove a single command-history entry by storage index (#96).
    pub fn remove_command_history(&mut self, index: usize) {
        if index < self.cache.command_history.len() {
            self.cache.command_history.remove(index);
        }
    }

    /// Collapse the resource sidebar on startup (default false) (#78).
    pub fn collapse_right_sidebar_default(&self) -> bool {
        self.cache.collapse_right_sidebar_default
    }

    pub fn set_collapse_right_sidebar_default(&mut self, v: bool) {
        self.cache.collapse_right_sidebar_default = v;
    }

    /// Persisted sidebar width in logical px. Falls back to the default when the
    /// stored value is unset/zero (e.g. a config created via `Default`).
    pub fn right_sidebar_width(&self) -> f32 {
        let w = self.cache.right_sidebar_width;
        if w <= 0.0 {
            default_right_sidebar_width()
        } else {
            w
        }
    }

    pub fn set_right_sidebar_width(&mut self, v: f32) {
        self.cache.right_sidebar_width = v;
    }

    /// Resource / SFTP panel docking geometry, persisted across restarts (#dock).
    /// Sizes fall back to their defaults when unset/zero; docks fall back to a
    /// sensible edge when the stored string is empty.
    pub fn right_sidebar_height(&self) -> f32 {
        let h = self.cache.right_sidebar_height;
        if h <= 0.0 {
            default_right_sidebar_height()
        } else {
            h
        }
    }
    pub fn set_right_sidebar_height(&mut self, v: f32) {
        self.cache.right_sidebar_height = v;
    }
    pub fn right_sidebar_dock(&self) -> String {
        let d = self.cache.right_sidebar_dock.trim();
        if d.is_empty() {
            // (#dock-default 2026-10-19) 字段是 2026-10-18 新增的，老配置读出来
            // 是空串。这里必须回落成**出厂默认**（右侧），否则升级后资源面板会
            // 被停到左边，和左侧会话栏挤在同一条边（fresh_config 里是 "right"）。
            "right".into()
        } else {
            d.to_string()
        }
    }
    pub fn set_right_sidebar_dock(&mut self, v: String) {
        self.cache.right_sidebar_dock = v;
    }
    pub fn right_sidebar_collapsed(&self) -> Option<bool> {
        self.cache.right_sidebar_collapsed
    }
    pub fn set_right_sidebar_collapsed(&mut self, v: bool) {
        self.cache.right_sidebar_collapsed = Some(v);
    }
    pub fn left_sidebar_width(&self) -> f32 {
        let w = self.cache.left_sidebar_width;
        if w <= 0.0 {
            240.0
        } else {
            w
        }
    }
    pub fn set_left_sidebar_width(&mut self, v: f32) {
        self.cache.left_sidebar_width = v;
    }
    pub fn left_sidebar_dock(&self) -> String {
        let d = self.cache.left_sidebar_dock.trim();
        if d.is_empty() {
            "left".into()
        } else {
            d.to_string()
        }
    }
    pub fn set_left_sidebar_dock(&mut self, v: String) {
        self.cache.left_sidebar_dock = v;
    }
    pub fn left_sidebar_collapsed(&self) -> Option<bool> {
        self.cache.left_sidebar_collapsed
    }
    pub fn set_left_sidebar_collapsed(&mut self, v: bool) {
        self.cache.left_sidebar_collapsed = Some(v);
    }
    pub fn mcp_enabled(&self) -> bool {
        self.cache.mcp_enabled
    }
    pub fn set_mcp_enabled(&mut self, enabled: bool) {
        self.cache.mcp_enabled = enabled;
    }
    pub fn mcp_use_saved_credentials(&self) -> bool {
        self.cache.mcp_use_saved_credentials
    }
    pub fn set_mcp_use_saved_credentials(&mut self, enabled: bool) {
        self.cache.mcp_use_saved_credentials = enabled;
    }
    pub fn mcp_allow_commands(&self) -> bool {
        self.cache.mcp_allow_commands
    }
    pub fn set_mcp_allow_commands(&mut self, enabled: bool) {
        self.cache.mcp_allow_commands = enabled;
    }
    pub fn mcp_allow_file_transfers(&self) -> bool {
        self.cache.mcp_allow_file_transfers
    }
    pub fn set_mcp_allow_file_transfers(&mut self, enabled: bool) {
        self.cache.mcp_allow_file_transfers = enabled;
    }
    pub fn wallpaper_overlay(&self) -> f32 {
        let a = self.cache.wallpaper_overlay;
        // Floor lowered 0.40 -> 0.30 so more see-through panels are reachable.
        if a <= 0.0 {
            DEFAULT_WALLPAPER_OVERLAY
        } else {
            a.clamp(0.30, 1.0)
        }
    }
    pub fn set_wallpaper_overlay(&mut self, v: f32) {
        self.cache.wallpaper_overlay = v.clamp(0.30, 1.0);
    }
    pub fn panel_font(&self) -> u32 {
        if self.cache.panel_font == 0 {
            100
        } else {
            self.cache.panel_font
        }
    }
    pub fn set_panel_font(&mut self, percent: u32) {
        self.cache.panel_font = percent.clamp(80, 160);
    }
    pub fn sftp_panel_width(&self) -> f32 {
        let w = self.cache.sftp_panel_width;
        if w <= 0.0 {
            default_sftp_width()
        } else {
            w
        }
    }
    pub fn set_sftp_panel_width(&mut self, v: f32) {
        self.cache.sftp_panel_width = v;
    }
    pub fn sftp_panel_height(&self) -> f32 {
        let h = self.cache.sftp_panel_height;
        if h <= 0.0 {
            default_sftp_height()
        } else {
            h
        }
    }
    pub fn set_sftp_panel_height(&mut self, v: f32) {
        self.cache.sftp_panel_height = v;
    }
    pub fn sftp_tree_width(&self) -> f32 {
        let width = self.cache.sftp_tree_width;
        if width <= 0.0 {
            default_sftp_tree_width()
        } else {
            width.clamp(120.0, 420.0)
        }
    }
    pub fn set_sftp_tree_width(&mut self, width: f32) {
        self.cache.sftp_tree_width = width.clamp(120.0, 420.0);
    }
    pub fn sftp_visible_columns(&self) -> Vec<String> {
        const COLUMNS: &[&str] = &["name", "type", "size", "modified", "permissions", "owner", "group"];
        if self.cache.sftp_visible_columns.is_empty() {
            return COLUMNS.iter().map(|column| (*column).to_string()).collect();
        }
        let mut columns: Vec<String> = self
            .cache
            .sftp_visible_columns
            .iter()
            .filter(|column| COLUMNS.contains(&column.as_str()))
            .cloned()
            .collect();
        if !columns.iter().any(|column| column == "name") {
            columns.insert(0, "name".to_string());
        }
        columns
    }
    pub fn set_sftp_visible_columns(&mut self, columns: Vec<String>) {
        let allowed = ["name", "type", "size", "modified", "permissions", "owner", "group"];
        let mut normalized: Vec<String> = columns
            .into_iter()
            .filter(|column| allowed.contains(&column.as_str()))
            .collect();
        normalized.dedup();
        if !normalized.iter().any(|column| column == "name") {
            normalized.insert(0, "name".to_string());
        }
        self.cache.sftp_visible_columns = normalized;
    }
    pub fn sftp_dock(&self) -> String {
        let d = self.cache.sftp_dock.trim();
        if d.is_empty() {
            "bottom".into()
        } else {
            d.to_string()
        }
    }
    pub fn set_sftp_dock(&mut self, v: String) {
        self.cache.sftp_dock = v;
    }
    /// Last window size in logical px; `(0,0)` means unset (use the default).
    pub fn window_size(&self) -> (f32, f32) {
        (self.cache.window_width, self.cache.window_height)
    }
    pub fn set_window_size(&mut self, w: f32, h: f32) {
        self.cache.window_width = w;
        self.cache.window_height = h;
    }

    /// Collapse the SFTP panel on startup (default false) (#78).
    pub fn collapse_sftp_default(&self) -> bool {
        self.cache.collapse_sftp_default
    }

    pub fn set_collapse_sftp_default(&mut self, v: bool) {
        self.cache.collapse_sftp_default = v;
    }

    /// Mirror SFTP uploads to other sessions while session-sync is on (default
    /// false). Only has effect when the session-sync toggle is on.
    pub fn sync_upload(&self) -> bool {
        self.cache.sync_upload
    }

    pub fn set_sync_upload(&mut self, v: bool) {
        self.cache.sync_upload = v;
    }

    pub fn webdav_enabled(&self) -> bool {
        self.cache.webdav_enabled
    }

    pub fn webdav_url(&self) -> &str {
        &self.cache.webdav_url
    }

    pub fn webdav_username(&self) -> &str {
        &self.cache.webdav_username
    }

    pub fn webdav_password(&self) -> &str {
        self.cache.webdav_password.as_str()
    }

    pub fn webdav_remote_path(&self) -> &str {
        if self.cache.webdav_remote_path.trim().is_empty() {
            "mterm-connections.json"
        } else {
            &self.cache.webdav_remote_path
        }
    }

    pub fn webdav_accept_invalid_certs(&self) -> bool {
        self.cache.webdav_accept_invalid_certs
    }

    pub fn set_webdav_settings(
        &mut self,
        enabled: bool,
        url: String,
        username: String,
        password: String,
        remote_path: String,
        accept_invalid_certs: bool,
    ) {
        self.cache.webdav_enabled = enabled;
        self.cache.webdav_url = url.trim().trim_end_matches('/').to_string();
        self.cache.webdav_username = username.trim().to_string();
        self.cache.webdav_password = Secret::new(password);
        self.cache.webdav_remote_path = if remote_path.trim().is_empty() {
            "mterm-connections.json".to_string()
        } else {
            remote_path.trim().trim_start_matches('/').to_string()
        };
        self.cache.webdav_accept_invalid_certs = accept_invalid_certs;
    }

    /// Whether each download prompts for a save location (default false) (#87).
    pub fn download_always_ask(&self) -> bool {
        self.cache.download_always_ask
    }

    pub fn set_download_always_ask(&mut self, ask: bool) {
        self.cache.download_always_ask = ask;
    }

    // ── Session groups / folders (#41) ────────────────────────────────────

    /// Explicit groups (empty folders included). "default" is implicit.
    pub fn groups(&self) -> &[String] {
        &self.cache.groups
    }

    pub fn collapsed_session_groups(&self) -> Option<&[String]> {
        self.cache.collapsed_session_groups.as_deref()
    }

    /// (#group-color 2026-09-14) 全部分组颜色(组名 -> "#RRGGBB"),供模型层
    /// 组装 SessionInfo 时查表。未设色的组不在表里。
    pub fn group_colors(&self) -> &std::collections::HashMap<String, String> {
        &self.cache.group_colors
    }

    /// (#hide-system-group) 是否隐藏欢迎列表里的"本地终端"保留组
    /// (列表空白处 / 组头右键菜单切换)。默认 false = 显示。
    pub fn system_group_hidden(&self) -> bool {
        self.cache.hide_system_group
    }

    /// 切换并**立即落盘**:与其它偏好(hide_cmd_bar 等)同一条持久化路径,
    /// 重启后保持上次的显隐状态。
    pub fn set_system_group_hidden(&mut self, hidden: bool) {
        if self.cache.hide_system_group == hidden {
            return;
        }
        self.cache.hide_system_group = hidden;
        if let Err(err) = self.save() {
            tracing::warn!("failed to save system group visibility: {err:#}");
        }
    }

    /// Remember a Quick Connect folder's open/closed state. On the first
    /// interaction, materialise the default-collapsed state for every existing
    /// folder so expanding one folder does not accidentally expand the rest.
    pub fn set_session_group_collapsed(&mut self, name: &str, collapsed: bool) {
        if self.cache.collapsed_session_groups.is_none() {
            let mut groups = vec!["system".to_string()];
            if self
                .cache
                .sessions
                .iter()
                .any(|session| session.group.is_empty())
            {
                groups.push("default".to_string());
            }
            groups.extend(self.cache.groups.iter().cloned());
            groups.extend(
                self.cache
                    .sessions
                    .iter()
                    .filter(|session| !session.group.is_empty())
                    .map(|session| session.group.clone()),
            );
            groups.sort();
            groups.dedup();
            self.cache.collapsed_session_groups = Some(groups);
        }

        let groups = self.cache.collapsed_session_groups.as_mut().unwrap();
        groups.retain(|group| group != name);
        if collapsed {
            groups.push(name.to_string());
            groups.sort();
            groups.dedup();
        }
    }

    /// Whether a user group already exists, including groups inferred from
    /// sessions that were created before explicit group records were added.
    pub fn session_group_exists(&self, name: &str) -> bool {
        let target = name.trim();
        if target.is_empty() {
            return false;
        }
        self.cache
            .groups
            .iter()
            .any(|group| group.trim().eq_ignore_ascii_case(target))
            || self.cache.sessions.iter().any(|session| {
                !session.group.trim().is_empty()
                    && session.group.trim().eq_ignore_ascii_case(target)
            })
    }

    /// (#group-order-materialize 2026-09-12) 把**显示组序**物化进 explicit
    /// groups:组排序前必须先补这一步,否则"会话对话框里自由输入过组名"
    /// (GroupCombo 可编辑,#179)的组会漏在显式列表外 —— 它照常显示
    /// (named_display_groups 把 session-only 组按首现顺序附在末尾),却不在
    /// cache.groups 里,下面两个组排序函数按名字查列表直接找不到,表现为
    /// **组头拖动完全没反应**(拖了不换位、也不能作为落点)。
    /// 顺序不变、幂等:只是把隐含组按它当前显示的位置写成显式,顺带丢掉
    /// 保留组名等脏数据(与 normalize_reserved_session_groups 同口径)。
    fn materialize_group_order(&mut self) {
        let display = named_display_groups(&self.cache.groups, &self.cache.sessions);
        if display != self.cache.groups {
            self.cache.groups = display;
        }
    }

    /// (#group-sort-line-r2 2026-09-11) 组排序的提交语义与**插入线同源**:
    /// 线画在哪个组头之前,松手就把 `name` 移到 `target` 之前。旧公式
    /// `round(dy/pitch)` 按格数估位移,组高不等时会与线的落点脱节
    /// (线在 A 前、松手落在 B 前)。
    /// target 不存在 / 与 name 相同 → false(no-op,回原位)。
    /// default/system 不在 explicit groups,天然不参与。
    pub fn move_group_before(&mut self, name: &str, target: &str) -> bool {
        if name.is_empty() || target.is_empty() || name == target {
            return false;
        }
        self.materialize_group_order();
        let Some(from) = self.cache.groups.iter().position(|g| g == name) else {
            return false;
        };
        if self.cache.groups.iter().position(|g| g == target).is_none() {
            return false;
        }
        let group = self.cache.groups.remove(from);
        // remove 之后 target 的下标可能前移,按名字重找插入点。
        let to = self
            .cache
            .groups
            .iter()
            .position(|g| g == target)
            .unwrap_or(self.cache.groups.len());
        self.cache.groups.insert(to, group);
        true
    }

    /// (#group-drag-tail 2026-09-11) 移到 explicit groups 末尾 = 列表最后
    /// 一位。尾部哨兵上报的 "(tail)" 落点由 app.rs 分派到这里——此前
    /// "插入到某组之前"的表达式永远覆盖不到"最后组之后"这个位置,组别
    /// 无法被拖到末位(用户反馈)。已在末位时 no-op(避免无谓写盘)。
    pub fn move_group_to_end(&mut self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        // (#group-order-materialize 2026-09-12) 同 move_group_before:session-only
        // 组要先物化进显式列表,否则"移到末位"对这个组同样静默无效。
        self.materialize_group_order();
        let Some(from) = self.cache.groups.iter().position(|g| g == name) else {
            return false;
        };
        if from + 1 == self.cache.groups.len() {
            return false;
        }
        let group = self.cache.groups.remove(from);
        self.cache.groups.push(group);
        true
    }

    /// Create an empty group. Ignores blank/reserved names and duplicates.
    pub fn add_group(&mut self, name: String) {
        let n = name.trim().to_string();
        if n.is_empty() || is_reserved_session_group(&n) || self.session_group_exists(&n) {
            return;
        }
        self.cache.groups.push(n.clone());
        if let Some(groups) = &mut self.cache.collapsed_session_groups {
            groups.push(n);
            groups.sort();
            groups.dedup();
        }
    }

    /// Delete a group. Any session still in it falls back to ungrouped — the UI
    /// only offers delete on empty groups, but we clear sessions defensively.
    pub fn remove_group(&mut self, name: &str) {
        if is_reserved_session_group(name.trim()) {
            return;
        }
        self.cache.groups.retain(|g| g != name);
        // (#group-color 2026-09-14) 组没了,颜色条目也要跟着清掉:否则以后
        // 建一个同名组时会"继承"上一个组的颜色,看起来像凭空冒出来的。
        self.cache.group_colors.remove(name);
        if let Some(groups) = &mut self.cache.collapsed_session_groups {
            groups.retain(|group| group != name);
        }
        for s in &mut self.cache.sessions {
            if s.group == name {
                s.group.clear();
            }
        }
    }

    /// Rename a group, moving its sessions along. No-op for reserved names.
    pub fn rename_group(&mut self, old: &str, new: String) {
        let n = new.trim().to_string();
        if n.is_empty()
            || is_reserved_session_group(old.trim())
            || is_reserved_session_group(&n)
            || n == old
            || (!n.eq_ignore_ascii_case(old) && self.session_group_exists(&n))
        {
            return;
        }
        for g in &mut self.cache.groups {
            if g == old {
                *g = n.clone();
            }
        }
        // (#group-color 2026-09-14) 颜色是按组名存的 key,改名时必须一起迁移 ——
        // 否则"改个名颜色就没了"(用户设的色静默丢失)。
        if let Some(hex) = self.cache.group_colors.remove(old) {
            self.cache.group_colors.insert(n.clone(), hex);
        }
        for s in &mut self.cache.sessions {
            if s.group == old {
                s.group = n.clone();
            }
        }
        if let Some(groups) = &mut self.cache.collapsed_session_groups {
            for group in groups.iter_mut() {
                if group == old {
                    *group = n.clone();
                }
            }
            groups.sort();
            groups.dedup();
        }
        self.cache.groups.sort();
        self.cache.groups.dedup();
    }

    /// 设定分组颜色;`hex` 传空串 = 清除(回到无色)。
    /// (#group-color-reserved 2026-09-14) **保留组也可着色**:system(本地终端)与
    /// default(默认组)默认仍无色(不参与自动配色),但用户右击设色后照常显示 ——
    /// 用户定稿"系统组、默认组都支持右击修改分组颜色"。因此这里只拒绝空名。
    pub fn set_group_color(&mut self, name: &str, hex: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let hex = hex.trim();
        if hex.is_empty() {
            self.cache.group_colors.remove(name);
        } else {
            self.cache
                .group_colors
                .insert(name.to_string(), hex.to_string());
        }
    }

    pub fn save(&self) -> Result<()> {
        // Build a disk copy where every non-empty password is encrypted.
        let mut disk = self.cache.clone();
        // (#sync-dialog-v2-fix 2026-10-12) 频道成员不入盘（内存态，见 load）。
        disk.channel_members = default_channel_members();
        for session in &mut disk.sessions {
            if !session.password.is_empty()
                && !session.password.as_str().starts_with(Self::ENC_PREFIX)
            {
                let enc = Self::encrypt(&self.key, session.password.as_str())?;
                session.password = Secret::new(enc);
            }
            if !session.private_key_inline.is_empty()
                && !session
                    .private_key_inline
                    .as_str()
                    .starts_with(Self::ENC_PREFIX)
            {
                let enc = Self::encrypt(&self.key, session.private_key_inline.as_str())?;
                session.private_key_inline = Secret::new(enc);
            }
            for trigger in &mut session.triggers {
                if !trigger.response.is_empty()
                    && !trigger.response.as_str().starts_with(Self::ENC_PREFIX)
                {
                    let enc = Self::encrypt(&self.key, trigger.response.as_str())?;
                    trigger.response = Secret::new(enc);
                }
            }
        }
        if !disk.webdav_password.is_empty()
            && !disk.webdav_password.as_str().starts_with(Self::ENC_PREFIX)
        {
            let enc = Self::encrypt(&self.key, disk.webdav_password.as_str())?;
            disk.webdav_password = Secret::new(enc);
        }
        let raw = serde_json::to_string_pretty(&disk)?;
        // Write to a sibling temp file then rename — cheap atomicity.
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, &raw).with_context(|| format!("failed to write {}", tmp.display()))?;
        // Restrict to owner-only before publishing (#34): sessions.json holds
        // (encrypted) credentials, so it shouldn't be world-readable. Set 0600
        // on the temp file so the permission is already in place at rename.
        // Windows %APPDATA% is owner-restricted by default ACLs — no-op there.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("failed to set permissions on {}", tmp.display()))?;
        }
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("failed to finalise {}", self.path.display()))?;
        Ok(())
    }

    // ── Portable export / import (issue #46) ──────────────────────────────

    /// Encrypt a password with the portable export key → `"enc:exp:v1:<b64>"`.
    fn encrypt_export(plaintext: &str) -> Result<String> {
        let cipher = ChaCha20Poly1305::new((&Self::EXPORT_KEY).into());
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("export encrypt error: {e}"))?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ciphertext);
        Ok(format!(
            "{}{}",
            Self::EXPORT_PREFIX,
            URL_SAFE_NO_PAD.encode(&blob)
        ))
    }

    /// Decrypt a value produced by [`Self::encrypt_export`]; `None` if it isn't one.
    fn decrypt_export(s: &str) -> Option<String> {
        let b64 = s.strip_prefix(Self::EXPORT_PREFIX)?;
        let blob = URL_SAFE_NO_PAD.decode(b64).ok()?;
        if blob.len() < 12 {
            return None;
        }
        let (nonce_bytes, ciphertext) = blob.split_at(12);
        let cipher = ChaCha20Poly1305::new((&Self::EXPORT_KEY).into());
        let nonce = chacha20poly1305::Nonce::from_slice(nonce_bytes);
        let plain = cipher.decrypt(nonce, ciphertext).ok()?;
        String::from_utf8(plain).ok()
    }

    /// Export all sessions to a portable JSON file. Passwords are re-encrypted
    /// with the built-in export key; everything else stays plaintext so the
    /// file is human-readable and editable. Returns the number of sessions.
    pub fn export_json(&self) -> Result<(String, usize)> {
        let mut out = ExportFile {
            mterm_export: 1,
            sessions: self.cache.sessions.clone(),
        };
        for s in &mut out.sessions {
            // `cache` holds plaintext passwords; obfuscate with the export key.
            if !s.password.is_empty() {
                let enc = Self::encrypt_export(s.password.as_str())?;
                s.password = Secret::new(enc);
            }
            if !s.private_key_inline.is_empty() {
                let enc = Self::encrypt_export(s.private_key_inline.as_str())?;
                s.private_key_inline = Secret::new(enc);
            }
            for trigger in &mut s.triggers {
                if !trigger.response.is_empty() {
                    let enc = Self::encrypt_export(trigger.response.as_str())?;
                    trigger.response = Secret::new(enc);
                }
            }
            // `last_used` is machine-local noise — don't carry it across.
            s.last_used = None;
        }
        Ok((serde_json::to_string_pretty(&out)?, out.sessions.len()))
    }

    /// Export all sessions to a portable JSON file. Passwords are re-encrypted
    /// with the built-in export key; everything else stays plaintext so the
    /// file is human-readable and editable. Returns the number of sessions.
    pub fn export_to(&self, path: &Path) -> Result<usize> {
        let (raw, count) = self.export_json()?;
        fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(count)
    }

    /// Import sessions from a MTerm portable export or a FinalShell connection
    /// export. New sessions get fresh ids; duplicates (same host+user+port+kind)
    /// are skipped.
    /// Returns `(added, skipped)`. The store is saved if anything was added.
    pub fn import_json(&mut self, raw: &str) -> Result<(usize, usize)> {
        let (sessions, decrypt_mterm_secrets) =
            match serde_json::from_str::<ExportFile>(raw) {
                Ok(file) => (file.sessions, true),
                Err(mterm_error) => (
                    super::finalshell::parse_export(raw).with_context(|| {
                        format!(
                            "not a valid MTerm or FinalShell export file; MTerm parser: {mterm_error}"
                        )
                    })?,
                    false,
                ),
            };

        let mut added = 0usize;
        let mut skipped = 0usize;
        for mut s in sessions {
            // Recover the plaintext password (cache stores plaintext). Accept an
            // export blob, our local enc:v1 blob, or a legacy plaintext value.
            // FinalShell's parser has already decrypted its DES password, so avoid
            // interpreting a coincidental `enc:*` plaintext prefix as ours.
            if decrypt_mterm_secrets {
                if let Some(plain) = Self::decrypt_export(s.password.as_str()) {
                    s.password = Secret::new(plain);
                } else if let Some(plain) = Self::try_decrypt(&self.key, s.password.as_str()) {
                    s.password = Secret::new(plain);
                }
                if let Some(plain) = Self::decrypt_export(s.private_key_inline.as_str()) {
                    s.private_key_inline = Secret::new(plain);
                } else if let Some(plain) =
                    Self::try_decrypt(&self.key, s.private_key_inline.as_str())
                {
                    s.private_key_inline = Secret::new(plain);
                }
                for trigger in &mut s.triggers {
                    if let Some(plain) = Self::decrypt_export(trigger.response.as_str()) {
                        trigger.response = Secret::new(plain);
                    } else if let Some(plain) =
                        Self::try_decrypt(&self.key, trigger.response.as_str())
                    {
                        trigger.response = Secret::new(plain);
                    }
                }
            }
            let dup = self.cache.sessions.iter().any(|x| {
                x.host == s.host && x.user == s.user && x.port == s.port && x.kind == s.kind
            });
            if dup {
                skipped += 1;
                continue;
            }
            s.id = Uuid::new_v4().to_string();
            self.upsert(s);
            added += 1;
        }
        if added > 0 {
            self.save()?;
        }
        Ok((added, skipped))
    }

    /// Import sessions from a MTerm or FinalShell JSON export file.
    pub fn import_from(&mut self, path: &Path) -> Result<(usize, usize)> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        self.import_json(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> ConfigStore {
        let path = std::env::temp_dir().join(format!("ms-test-{}.json", Uuid::new_v4()));
        ConfigStore {
            path,
            cache: ConfigFile::default(),
            key: [7u8; 32],
        }
    }

    #[test]
    fn terminal_cursor_style_defaults_and_validates() {
        let mut store = temp_store();
        assert_eq!(store.terminal_cursor_style(), "block");

        store.set_terminal_cursor_style("bar".into());
        assert_eq!(store.terminal_cursor_style(), "bar");
        store.set_terminal_cursor_style("underline".into());
        assert_eq!(store.terminal_cursor_style(), "underline");
        store.set_terminal_cursor_style("unexpected".into());
        assert_eq!(store.terminal_cursor_style(), "block");

        store.cache = serde_json::from_str("{}").expect("legacy config must deserialize");
        assert_eq!(store.terminal_cursor_style(), "block");
    }

    #[test]
    fn sftp_visible_columns_keep_name_and_ignore_unknown_values() {
        let mut store = temp_store();
        store.set_sftp_visible_columns(vec!["owner".into(), "unknown".into(), "owner".into()]);
        assert_eq!(
            store.sftp_visible_columns(),
            vec!["name".to_string(), "owner".to_string()]
        );
    }

    #[test]
    fn missing_sftp_columns_use_the_current_default() {
        let store = temp_store();
        assert_eq!(store.sftp_visible_columns().len(), 7);
    }

    #[test]
    fn right_sidebar_dock_falls_back_to_the_shipped_default() {
        let mut store = temp_store();
        // A config written before the dock field existed deserializes with "",
        // which must resolve to the shipped default (right), not to "left" —
        // otherwise upgrading stacks the resource panel on the same edge as
        // the session sidebar (#dock-default 2026-10-19).
        store.cache = serde_json::from_str("{}").expect("legacy config must deserialize");
        assert_eq!(store.right_sidebar_dock(), "right");

        // An explicit choice is preserved verbatim.
        store.set_right_sidebar_dock("left".into());
        assert_eq!(store.right_sidebar_dock(), "left");
        store.set_right_sidebar_dock("top".into());
        assert_eq!(store.right_sidebar_dock(), "top");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn renderer_mode_preserves_compatibility_default_and_validates() {
        let mut store = temp_store();
        assert_eq!(store.renderer_mode(), "software");

        store.set_renderer_mode("auto".into());
        assert_eq!(store.renderer_mode(), "auto");
        store.set_renderer_mode("gpu".into());
        assert_eq!(store.renderer_mode(), "gpu");
        store.set_renderer_mode("unexpected".into());
        assert_eq!(store.renderer_mode(), "software");

        store.cache = serde_json::from_str("{}").expect("legacy config must deserialize");
        assert_eq!(store.renderer_mode(), "software");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn renderer_mode_preserves_linux_automatic_default_and_validates() {
        let mut store = temp_store();
        assert_eq!(store.renderer_mode(), "auto");

        store.set_renderer_mode("gpu".into());
        assert_eq!(store.renderer_mode(), "gpu");
        store.set_renderer_mode("software".into());
        assert_eq!(store.renderer_mode(), "software");
        store.set_renderer_mode("unexpected".into());
        assert_eq!(store.renderer_mode(), "auto");

        store.cache = serde_json::from_str("{}").expect("legacy config must deserialize");
        assert_eq!(store.renderer_mode(), "auto");
    }

    #[test]
    fn quick_connect_groups_default_collapsed_and_remember_expansion() {
        let mut store = temp_store();
        store.cache.groups = vec!["production".into(), "staging".into()];
        store.cache.sessions.push(Session {
            group: "production".into(),
            ..sample_session("server")
        });

        assert!(store.collapsed_session_groups().is_none());
        store.set_session_group_collapsed("production", false);

        let collapsed = store.collapsed_session_groups().unwrap();
        assert!(!collapsed.iter().any(|group| group == "production"));
        assert!(collapsed.iter().any(|group| group == "staging"));
        assert!(collapsed.iter().any(|group| group == "system"));

        store.set_session_group_collapsed("production", true);
        assert!(store
            .collapsed_session_groups()
            .unwrap()
            .iter()
            .any(|group| group == "production"));
    }

    /// Display order for these tests: default(d1, d2), alpha(a1), beta(b1, b2)
    /// — "default" first, named groups alphabetically, stored Vec order
    /// matching display order.
    fn reorder_store() -> ConfigStore {
        let mut store = temp_store();
        store.cache.sessions = vec![
            sample_session("d1"),
            sample_session("d2"),
            Session {
                group: "alpha".into(),
                ..sample_session("a1")
            },
            Session {
                group: "beta".into(),
                ..sample_session("b1")
            },
            Session {
                group: "beta".into(),
                ..sample_session("b2")
            },
        ];
        // Empty collapse list = every group expanded.
        store.cache.collapsed_session_groups = Some(Vec::new());
        store
    }

    fn id_of(store: &ConfigStore, name: &str) -> String {
        store
            .sessions()
            .iter()
            .find(|s| s.name == name)
            .expect("session present")
            .id
            .clone()
    }

    fn order_of(store: &ConfigStore) -> Vec<(String, String)> {
        store
            .sessions()
            .iter()
            .map(|s| (s.name.clone(), s.group.clone()))
            .collect()
    }

    #[test]
    fn reorder_session_swaps_same_group_neighbours() {
        let mut store = reorder_store();
        assert!(store.reorder_session(&id_of(&store, "d1"), 1));
        assert_eq!(
            order_of(&store)[..2],
            [("d2".into(), "".into()), ("d1".into(), "".into())]
        );
    }

    #[test]
    fn reorder_session_hops_down_into_next_group_at_its_top() {
        let mut store = reorder_store();
        assert!(store.reorder_session(&id_of(&store, "d2"), 1));
        assert_eq!(
            order_of(&store)[1..4],
            [
                ("d2".into(), "alpha".into()),
                ("a1".into(), "alpha".into()),
                ("b1".into(), "beta".into()),
            ]
        );
    }

    #[test]
    fn reorder_session_hops_up_into_previous_group_at_its_bottom() {
        let mut store = reorder_store();
        assert!(store.reorder_session(&id_of(&store, "a1"), -1));
        assert_eq!(
            order_of(&store)[..3],
            [
                ("d1".into(), "".into()),
                ("d2".into(), "".into()),
                ("a1".into(), "".into()),
            ]
        );
    }

    #[test]
    fn reorder_session_skips_collapsed_groups() {
        let mut store = reorder_store();
        store.set_session_group_collapsed("alpha", true);
        assert!(store.reorder_session(&id_of(&store, "d2"), 1));
        assert_eq!(
            order_of(&store)[1..4],
            [
                ("a1".into(), "alpha".into()),
                ("d2".into(), "beta".into()),
                ("b1".into(), "beta".into()),
            ]
        );
    }

    #[test]
    fn reorder_session_lands_in_empty_explicit_folder() {
        let mut store = reorder_store();
        // (#drag-cross-group-fix 2026-09-06) named_display_groups 已改为存储
        // 序(#group-drag-reorder):explicit 组按存储顺序渲染、session-only
        // 组按首次出现附后。空文件夹要渲染在 beta 之后,必须先把 alpha/beta
        // 按显示顺序显式注册,再追加 gamma;否则 gamma(explicit)渲染在隐式
        // 组之前,b2(末组)向下没有相邻组,换位失败。
        store.cache.groups = vec!["alpha".into(), "beta".into()];
        store.cache.groups.push("gamma".into());
        assert!(store.reorder_session(&id_of(&store, "b2"), 1));
        let last = store.sessions().iter().find(|s| s.name == "b2").unwrap();
        assert_eq!(last.group, "gamma");
    }

    /// (#group-order-materialize 2026-09-12) 组头拖动排序必须能作用在"只在
    /// 会话上出现过"的组(会话对话框的组名可自由输入 #179):这类组能显示、
    /// 但不在 explicit 列表里 —— 排序前先按**显示顺序**物化,否则
    /// move_group_before / move_group_to_end 按名字查列表直接找不到,表现
    /// 为"拖了没反应"。同时保证物化不会打乱显示顺序(幂等)。
    #[test]
    fn group_reorder_materializes_session_only_groups() {
        let mut store = reorder_store();
        // delta 是显式空文件夹;alpha / beta 只出现在会话的 group 字段上。
        store.cache.groups = vec!["delta".into()];
        assert_eq!(
            named_display_groups(store.groups(), store.sessions()),
            ["delta", "alpha", "beta"]
        );

        // 把隐式的 beta 移到同样隐式的 alpha 之前:两者都要先物化。
        assert!(store.move_group_before("beta", "alpha"));
        assert_eq!(store.groups(), ["delta", "beta", "alpha"]);

        // 末位:显式组 delta 移到末尾(此前它会静默 no-op)。
        assert!(store.move_group_to_end("delta"));
        assert_eq!(store.groups(), ["beta", "alpha", "delta"]);
    }

    /// (#group-dropdown-dedup 2026-09-08) 拼接结果无相邻重复时 Vec::dedup
    /// 原样放行:explicit 组序与会话组序不一致时(组头拖动换位后常态),
    /// 分组下拉框出现 [1,2,3,2,1,3] 式重复。去重必须全局、保首次出现
    /// (explicit 优先)。
    #[test]
    fn named_display_groups_dedups_across_explicit_and_session_order() {
        fn session(id: &str, group: &str) -> Session {
            let mut value = Session::new_empty();
            value.id = id.into();
            value.group = group.into();
            value
        }
        let explicit = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        // 会话存储顺序的组首现序列与 explicit 不同:2, 1, 3。
        let sessions = vec![
            session("a", "2"),
            session("b", "2"),
            session("c", "1"),
            session("d", "3"),
        ];
        assert_eq!(
            named_display_groups(&explicit, &sessions),
            ["1", "2", "3"]
        );
        // 保留组与未分组不进下拉。
        let with_reserved = vec![
            session("a", "system"),
            session("b", ""),
            session("c", "2"),
        ];
        assert_eq!(
            named_display_groups(&explicit.clone(), &with_reserved),
            ["1", "2", "3"]
        );
    }

    /// (#drag-ghost-pointer 2026-09-06) 几何落位:同组前/后插、跨组继承目标
    /// 组、拖空源组保留、自定位与未知 id 均为 no-op。
    #[test]
    fn move_session_relative_places_relative_to_target() {
        let mut store = reorder_store();

        // Same group: b2 moves before b1.
        assert!(store.move_session_relative(&id_of(&store, "b2"), &id_of(&store, "b1"), false));
        assert_eq!(
            order_of(&store)[3..5],
            [
                ("b2".to_string(), "beta".to_string()),
                ("b1".to_string(), "beta".to_string())
            ]
        );

        // Cross group: a1 moves after d2 (last ungrouped row), inheriting "".
        assert!(store.move_session_relative(&id_of(&store, "a1"), &id_of(&store, "d2"), true));
        assert_eq!(order_of(&store)[2], ("a1".to_string(), "".to_string()));

        // Emptied implicit source group survives as an explicit folder.
        assert!(store.groups().iter().any(|g| g == "alpha"));

        // Self-target and unknown ids are no-ops.
        assert!(!store.move_session_relative(&id_of(&store, "b1"), &id_of(&store, "b1"), true));
        assert!(!store.move_session_relative("nope", &id_of(&store, "b1"), true));
    }

    /// (#group-manager) 批量换组：按调用方给的显示顺序追加到目标组末尾，
    /// 不受 HashSet 遍历顺序影响。
    #[test]
    fn move_sessions_to_group_appends_in_display_order() {
        let mut store = reorder_store();
        let ids = vec![id_of(&store, "b2"), id_of(&store, "b1")];
        assert!(store.move_sessions_to_group(&ids, "alpha"));

        let alpha: Vec<&str> = store
            .sessions()
            .iter()
            .filter(|s| s.group == "alpha")
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(alpha, ["a1", "b2", "b1"], "块内顺序必须按 ids 原样");
        // 未涉及的组不受影响。
        assert_eq!(
            store
                .sessions()
                .iter()
                .filter(|s| s.group.is_empty())
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["d1", "d2"]
        );
    }

    /// (#group-manager) 批量组内排序：整块插到目标前/后，块内顺序不变。
    #[test]
    fn move_sessions_relative_keeps_block_order() {
        let mut store = reorder_store();
        let ids = vec![id_of(&store, "b2"), id_of(&store, "b1")];
        assert!(store.move_sessions_relative(&ids, &id_of(&store, "a1"), false));

        assert_eq!(
            order_of(&store)[2..5],
            [
                ("b2".to_string(), "alpha".to_string()),
                ("b1".to_string(), "alpha".to_string()),
                ("a1".to_string(), "alpha".to_string()),
            ]
        );

        // 目标本身在被拖集合里 → no-op（否则会自我参照）。
        assert!(!store.move_sessions_relative(&ids, &ids[0], true));
    }

    #[test]
    fn reorder_session_keeps_emptied_implicit_group_as_folder() {
        let mut store = reorder_store();
        assert!(store.reorder_session(&id_of(&store, "a1"), 1));
        assert_eq!(
            store
                .sessions()
                .iter()
                .find(|s| s.name == "a1")
                .unwrap()
                .group,
            "beta"
        );
        assert!(store.groups().iter().any(|g| g == "alpha"));
    }

    #[test]
    fn reorder_session_no_op_at_list_edges() {
        let mut store = reorder_store();
        assert!(!store.reorder_session(&id_of(&store, "d1"), -1));
        assert!(!store.reorder_session(&id_of(&store, "b2"), 1));
        assert_eq!(order_of(&store).len(), 5);
    }

    #[test]
    fn reorder_session_cross_group_needs_a_collapse_list() {
        let mut store = reorder_store();
        store.cache.collapsed_session_groups = None;
        // Same-group hops still work ...
        assert!(store.reorder_session(&id_of(&store, "d1"), 1));
        // ... but crossing a boundary does not: no list means every group
        // renders collapsed (mirrors build_session_rows).
        assert!(!store.reorder_session(&id_of(&store, "b2"), 1));
    }

    #[test]
    fn issue_316_reserved_system_groups_are_repaired_and_rejected() {
        let mut system_session = sample_session("misfiled");
        system_session.group = "system".into();
        system_session.password = Secret::default();
        let mut default_session = sample_session("legacy-default");
        default_session.group = "Default".into();
        let mut cfg = ConfigFile {
            sessions: vec![system_session, default_session],
            groups: vec![
                "system".into(),
                "System".into(),
                "default".into(),
                "prod".into(),
            ],
            collapsed_session_groups: Some(vec!["system".into(), "prod".into()]),
            ..ConfigFile::default()
        };

        assert!(normalize_reserved_session_groups(&mut cfg));
        assert_eq!(cfg.groups, ["prod"]);
        assert!(cfg.sessions.iter().all(|session| session.group.is_empty()));
        assert!(cfg.sessions[0].password.is_empty());
        // The built-in system folder's collapse preference is display state,
        // not a user-created group, so normalization must preserve it.
        assert_eq!(
            cfg.collapsed_session_groups.as_deref(),
            Some(["system".to_string(), "prod".to_string()].as_slice())
        );

        let mut store = temp_store();
        store.add_group("system".into());
        store.add_group("DEFAULT".into());
        store.add_group("prod".into());
        store.rename_group("prod", "System".into());
        assert_eq!(store.groups(), ["prod"]);

        let mut session = sample_session("server");
        session.group = "SYSTEM".into();
        let id = session.id.clone();
        store.upsert(session);
        assert_eq!(store.get(&id).unwrap().group, "");
    }

    #[test]
    fn session_group_names_are_unique_case_insensitively() {
        let mut store = temp_store();
        store.add_group("Production".into());
        store.add_group("production".into());
        assert_eq!(store.groups(), ["Production"]);
        assert!(store.session_group_exists(" PRODUCTION "));

        let mut session = sample_session("staging-server");
        session.group = "Staging".into();
        store.upsert(session);
        assert!(store.session_group_exists("staging"));

        store.rename_group("Production", "STAGING".into());
        assert_eq!(store.groups(), ["Production"]);

        // Changing only the spelling/case of the same group remains valid.
        store.rename_group("Production", "production".into());
        assert_eq!(store.groups(), ["production"]);
    }

    #[test]
    fn macos_renderer_mode_defaults_to_cpu_and_preserves_gpu_choices() {
        assert_eq!(normalize_macos_renderer_mode(""), "software");
        assert_eq!(normalize_macos_renderer_mode("software"), "software");
        assert_eq!(normalize_macos_renderer_mode("femtovg"), "femtovg");
        assert_eq!(normalize_macos_renderer_mode("skia"), "skia");
        assert_eq!(normalize_macos_renderer_mode("unexpected"), "software");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn renderer_mode_uses_macos_backends_and_validates() {
        let mut store = temp_store();
        assert_eq!(store.renderer_mode(), "software");

        store.set_renderer_mode("skia".into());
        assert_eq!(store.renderer_mode(), "skia");
        store.set_renderer_mode("femtovg".into());
        assert_eq!(store.renderer_mode(), "femtovg");
        store.set_renderer_mode("software".into());
        assert_eq!(store.renderer_mode(), "software");
        store.set_renderer_mode("unexpected".into());
        assert_eq!(store.renderer_mode(), "software");

        store.cache = serde_json::from_str("{}").expect("legacy config must deserialize");
        assert_eq!(store.renderer_mode(), "software");
    }

    #[test]
    fn terminal_cursor_color_normalizes_and_rejects_invalid_values() {
        let mut store = temp_store();
        assert_eq!(store.terminal_cursor_color(), "");

        assert!(store.set_terminal_cursor_color("#1a2B3c"));
        assert_eq!(store.terminal_cursor_color(), "#1A2B3C");
        assert!(store.set_terminal_cursor_color("abcdef"));
        assert_eq!(store.terminal_cursor_color(), "#ABCDEF");

        assert!(!store.set_terminal_cursor_color("#12345"));
        assert_eq!(store.terminal_cursor_color(), "#ABCDEF");
        assert!(!store.set_terminal_cursor_color("#GG0000"));
        assert_eq!(store.terminal_cursor_color(), "#ABCDEF");
    }

    fn sample_session(name: &str) -> Session {
        Session {
            name: name.into(),
            host: "192.168.100.2".into(),
            port: 22,
            user: "root".into(),
            ..Session::new_empty()
        }
    }

    #[test]
    fn wallpaper_defaults_follow_the_migration_chain() {
        // (#theme-split rev4 → #follow-ui-removed rev5) Fresh install: 默认无壁纸
        // + mterm 门面暗色 + 不跟随系统 + 三滑杆默认值（§0/§5：零操作即正确）。
        let fresh = fresh_config();
        assert_eq!(fresh.wallpaper, "");
        assert!((fresh.wallpaper_overlay - 0.85).abs() < f32::EPSILON);
        // (#mode-values 2026-10-17) 明暗两套出厂都是默认值。
        for av in [&fresh.appearance_dark, &fresh.appearance_light] {
            assert!((av.titlebar_alpha - 1.0).abs() < f32::EPSILON);
            assert!((av.tabbar_alpha - 1.0).abs() < f32::EPSILON);
            assert!((av.sidebar_alpha - 1.0).abs() < f32::EPSILON);
            assert!((av.term_alpha - 1.0).abs() < f32::EPSILON);
            assert!((av.wallpaper_transparency).abs() < f32::EPSILON);
            assert!((av.popup_transparency).abs() < f32::EPSILON);
            assert!((av.scrim_alpha - 0.32).abs() < f32::EPSILON);
        }
        assert_eq!(fresh.wallpaper_scope, "window");
        // (#wallpaper-auto-fit 2026-10-18) 出厂 = 「自动」（按图片与作用域区域
        // 的比例在填充 / 适应之间选）；老配置里显式存过的 cover/contain/fill 保持。
        assert_eq!(fresh.wallpaper_fit, "auto");
        assert_eq!(fresh.scrim_style, "auto");
        assert!(!fresh.follow_system);
        assert_eq!(fresh.theme, "mterm-dark");
        // User upgrading from before the feature: JSON without the key keeps the
        // serde default (builtin:tech) so rev4 can tell "never chose" apart.
        let cfg: ConfigFile = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.wallpaper, "builtin:tech");
        // 空串 = 「尚未落定」（serde 默认），交给迁移链处理；不再是「跟随系统」。
        assert_eq!(cfg.theme, "");
        assert!((cfg.appearance_dark.titlebar_alpha - 1.0).abs() < f32::EPSILON);
        assert!((cfg.appearance_light.sidebar_alpha - 1.0).abs() < f32::EPSILON);
        // An explicit "无"/none (stored as "") is preserved, not re-defaulted.
        let cfg: ConfigFile = serde_json::from_str(r#"{"wallpaper":""}"#).unwrap();
        assert_eq!(cfg.wallpaper, "");
        // A custom choice is preserved.
        let cfg: ConfigFile = serde_json::from_str(r#"{"wallpaper":"builtin:light"}"#).unwrap();
        assert_eq!(cfg.wallpaper, "builtin:light");

        let mut cfg = ConfigFile {
            wallpaper: "builtin:miku".to_string(),
            defaults_rev: DEFAULTS_REV,
            ..ConfigFile::default()
        };
        assert!(!migrate_defaults(&mut cfg));
        assert_eq!(cfg.wallpaper, "builtin:miku");
    }

    /// (#wallpaper-scope-params 2026-10-18) 主题 × 壁纸 × 作用域 三级隔离：
    /// ① 首设壁纸时两种作用域都落「看得见壁纸」的默认值（否则满屏黑）；
    /// ② 两个作用域各存一套，互不覆盖；
    /// ③ 换主题不带上一套壁纸/参数。
    #[test]
    fn wallpaper_settings_are_per_theme_and_per_scope() {
        let mut cfg = ConfigStore {
            path: PathBuf::new(),
            cache: fresh_config(),
            key: [0u8; 32],
        };
        cfg.set_theme("mterm-dark".into());

        cfg.set_wallpaper("builtin:miku");
        assert_eq!(cfg.wallpaper(), "builtin:miku");
        assert_eq!(cfg.wallpaper_scope(), "window");
        let win = cfg.scope_settings().expect("window scope");
        assert!(
            win.titlebar_alpha < 0.99 && win.sidebar_alpha < 0.99 && win.term_alpha < 0.99,
            "首设默认值必须能透出壁纸，实际 {win:?}"
        );
        assert_eq!(win.fit, "auto");

        // 切到终端作用域：另一套，且默认就带一点终端透明度（不是黑屏）。
        cfg.set_wallpaper_scope("terminal".into());
        let term = cfg.scope_settings().expect("terminal scope");
        assert!(term.term_alpha < 0.99, "终端作用域首次使用也要能看见壁纸");
        assert!((term.titlebar_alpha - 1.0).abs() < 1e-6, "终端作用域三区不吃壁纸");

        // 两个作用域互不影响。
        cfg.set_wallpaper_scope("window".into());
        cfg.set_term_alpha(0.2, true);
        assert!((cfg.term_alpha() - 0.2).abs() < 1e-6);
        cfg.set_wallpaper_scope("terminal".into());
        assert!((cfg.term_alpha() - term.term_alpha).abs() < 1e-6);

        // 换主题：不带上一套（未配置过 = 无壁纸 + 明暗基础值）。
        cfg.set_theme("mterm-light".into());
        assert_eq!(cfg.wallpaper(), "");
        assert!((cfg.term_alpha() - 1.0).abs() < 1e-6);

        // 切回去：原选择原参数都在。
        cfg.set_theme("mterm-dark".into());
        assert_eq!(cfg.wallpaper(), "builtin:miku");
        assert_eq!(cfg.wallpaper_scope(), "terminal");
        assert!((cfg.term_alpha() - term.term_alpha).abs() < 1e-6);

        // (#zone-default-context 2026-10-19) 双击复位的默认值随上下文变：
        // 有壁纸 = 该组合首设默认值；无壁纸 = 全不透明（0% 透明）。
        let (tb, tab, sb, term_d) = cfg.default_zone_alphas();
        assert_eq!((tb, tab, sb), (1.0, 1.0, 1.0), "终端作用域三区默认不透明");
        assert!((term_d - default_scope_settings("terminal").term_alpha).abs() < 1e-6);
        cfg.set_wallpaper_scope("window".into());
        let win_default = default_scope_settings("window");
        assert_eq!(
            cfg.default_zone_alphas(),
            (
                win_default.titlebar_alpha,
                win_default.tabbar_alpha,
                win_default.sidebar_alpha,
                win_default.term_alpha
            )
        );
        cfg.set_wallpaper("");
        assert_eq!(cfg.default_zone_alphas(), (1.0, 1.0, 1.0, 1.0), "无壁纸默认 0% 透明");
    }

    /// (#wallpaper-scope-workspace 2026-10-19) 新增的「主区域」档：
    /// ① 默认档仍是「整个界面」；② 主区域有自己的首设默认值（标题栏实心）；
    /// ③ 非法值归一化；④ 切走再切回，数值还在。
    #[test]
    fn workspace_scope_has_own_defaults() {
        let mut cfg = ConfigStore {
            path: PathBuf::new(),
            cache: fresh_config(),
            key: [0u8; 32],
        };
        cfg.set_theme("mterm-dark".into());
        cfg.set_wallpaper("builtin:miku");

        // ① 没配过 = 整个界面。
        assert_eq!(cfg.wallpaper_scope(), "window");

        // ② 主区域首设：标题栏实心，其余三区与「整个界面」同值。
        cfg.set_wallpaper_scope("workspace".into());
        assert_eq!(cfg.wallpaper_scope(), "workspace");
        let ws = cfg.scope_settings().expect("workspace scope");
        assert!((ws.titlebar_alpha - 1.0).abs() < 1e-6, "主区域标题栏必须实心");
        assert!((ws.tabbar_alpha - 0.70).abs() < 1e-6);
        assert!((ws.sidebar_alpha - 0.72).abs() < 1e-6);
        assert!((ws.term_alpha - 0.60).abs() < 1e-6);
        assert!(
            cfg.cache
                .wallpaper_settings
                .keys()
                .any(|k| k.ends_with("|workspace")),
            "切到主区域应落下 <theme>|<wp>|workspace 条目"
        );

        // 双击复位的目标跟着档走（标题栏回到实心）。
        let (tb, _, _, _) = cfg.default_zone_alphas();
        assert!((tb - 1.0).abs() < 1e-6);

        // ③ 非法值归一化为默认档。
        cfg.set_wallpaper_scope("nonsense".into());
        assert_eq!(cfg.wallpaper_scope(), "window");
        assert!((cfg.titlebar_alpha() - 0.78).abs() < 1e-6);

        // ④ 切回主区域：之前那套还在（等于默认，因为没改过）。
        cfg.set_wallpaper_scope("workspace".into());
        assert!((cfg.term_alpha() - 0.60).abs() < 1e-6);
        // 与「整个界面」是两套独立条目。
        let keys: Vec<String> = cfg
            .cache
            .wallpaper_settings
            .keys()
            .filter(|k| k.starts_with("mterm-dark|builtin:miku|"))
            .cloned()
            .collect();
        assert!(keys.iter().any(|k| k.ends_with("|window")));
        assert!(keys.iter().any(|k| k.ends_with("|workspace")));
    }

    /// (#wallpaper-none-default 2026-10-19) 切到「无壁纸」时作用域**只改显示**：
    /// ① 无壁纸期间一律呈现默认的「整个界面」（老配置也一样）；
    /// ② 用户存下的档位与参数不清空 —— 再切回这张壁纸时原样恢复。
    #[test]
    fn clearing_the_wallpaper_shows_the_default_scope_but_keeps_the_choice() {
        let mut cfg = ConfigStore {
            path: PathBuf::new(),
            cache: fresh_config(),
            key: [0u8; 32],
        };
        cfg.set_theme("mterm-dark".into());
        cfg.set_wallpaper("builtin:miku");
        cfg.set_wallpaper_scope("terminal".into());
        cfg.set_term_alpha(0.42, true);
        assert_eq!(cfg.wallpaper_scope(), "terminal");

        cfg.set_wallpaper("");
        assert_eq!(cfg.wallpaper_scope(), "window", "无壁纸 → 显示默认档");
        assert_eq!(
            cfg.cache
                .wallpaper_scope_by_theme
                .get("mterm-dark")
                .map(String::as_str),
            Some("terminal"),
            "只是不显示，存下的选择不能丢"
        );

        // 切回这张壁纸：作用域与调过的参数原样回来。
        cfg.set_wallpaper("builtin:miku");
        assert_eq!(cfg.wallpaper_scope(), "terminal");
        assert!((cfg.term_alpha() - 0.42).abs() < 1e-6);

        // 老配置（旧版本切「无」时留下的 terminal）：无壁纸期间同样按默认档
        // 显示，重新选壁纸后用户原来的档位照样生效。
        let legacy: ConfigFile = serde_json::from_str(
            r#"{"theme":"mterm-dark","wallpaper":"",
                "wallpaper_scope":"terminal",
                "wallpaper_by_theme":{"mterm-dark":""},
                "wallpaper_scope_by_theme":{"mterm-dark":"terminal"}}"#,
        )
        .expect("legacy config must deserialize");
        let mut store = ConfigStore {
            path: PathBuf::new(),
            cache: legacy,
            key: [0u8; 32],
        };
        assert_eq!(store.wallpaper_scope(), "window");
        store.set_wallpaper("builtin:miku");
        assert_eq!(store.wallpaper_scope(), "terminal");
    }

    /// (#wallpaper-delete-forget 2026-10-19) 只有**删除壁纸**才销毁数据：
    /// 该壁纸在所有主题下的参数条目 + 作用域记录都要清掉，其它壁纸不受影响。
    #[test]
    fn deleting_a_wallpaper_forgets_its_scopes_and_settings() {
        let mut cfg = ConfigStore {
            path: PathBuf::new(),
            cache: fresh_config(),
            key: [0u8; 32],
        };
        cfg.set_theme("mterm-dark".into());
        cfg.set_wallpaper("builtin:miku");
        cfg.set_wallpaper_scope("terminal".into());
        cfg.set_term_alpha(0.31, true);

        // 另一主题下的另一张壁纸：必须原样保留。
        cfg.set_theme("mterm-light".into());
        cfg.set_wallpaper("builtin:tech");
        cfg.set_wallpaper_scope("workspace".into());

        let removed = cfg.forget_wallpaper("builtin:miku");
        assert!(removed > 0, "应删掉至少一条参数条目");
        assert!(
            !cfg.cache
                .wallpaper_settings
                .keys()
                .any(|k| k.contains("builtin:miku")),
            "被删壁纸的参数条目必须清空"
        );
        assert!(
            cfg.cache
                .wallpaper_settings
                .keys()
                .any(|k| k.contains("builtin:tech")),
            "其它壁纸的参数不能误删"
        );
        assert_eq!(
            cfg.cache
                .wallpaper_by_theme
                .get("mterm-dark")
                .map(String::as_str),
            None,
            "用它的主题回到「无壁纸」"
        );
        assert!(cfg
            .cache
            .wallpaper_scope_by_theme
            .get("mterm-dark")
            .is_none());
        assert_eq!(
            cfg.cache
                .wallpaper_by_theme
                .get("mterm-light")
                .map(String::as_str),
            Some("builtin:tech")
        );
        assert_eq!(
            cfg.cache
                .wallpaper_scope_by_theme
                .get("mterm-light")
                .map(String::as_str),
            Some("workspace")
        );
    }

    /// (#theme-appearance 2026-10-19) 四根滑杆不再跨主题共享：无壁纸时每个主题
    /// 各存一套（旧实现按明暗两槽，同模式的主题会互相串）。
    #[test]
    fn zone_values_are_per_theme_when_no_wallpaper() {
        let mut cfg = ConfigStore {
            path: PathBuf::new(),
            cache: fresh_config(),
            key: [0u8; 32],
        };
        cfg.set_theme("mterm-dark".into());
        cfg.set_term_alpha(0.42, true);
        assert!((cfg.term_alpha() - 0.42).abs() < 1e-6);

        // 同是暗色的另一主题：自己的默认值（不跟随）。
        cfg.set_theme("nord-dark".into());
        assert!((cfg.term_alpha() - 1.0).abs() < 1e-6);
        // 亮色主题同样独立。
        cfg.set_theme("mterm-light".into());
        assert!((cfg.term_alpha() - 1.0).abs() < 1e-6);

        cfg.set_theme("mterm-dark".into());
        assert!((cfg.term_alpha() - 0.42).abs() < 1e-6);
    }

    /// (#theme-appearance 2026-10-19) 弹框/遮罩按「主题 × 有无壁纸」两套：
    /// 改哪档存哪档，切壁纸或切主题都不会串。
    #[test]
    fn popup_and_scrim_have_per_theme_wallpaper_variants() {
        let mut cfg = ConfigStore {
            path: PathBuf::new(),
            cache: fresh_config(),
            key: [0u8; 32],
        };
        cfg.set_theme("mterm-dark".into());
        // 无壁纸档。
        cfg.set_popup_transparency(0.10, true);
        cfg.set_scrim_alpha(0.22, true);
        cfg.set_scrim_style("dark".into());

        // 有壁纸档：自己的出厂值（0% / 32% / auto），不是无壁纸档的拷贝。
        cfg.set_wallpaper("builtin:miku");
        assert!((cfg.popup_transparency() - 0.0).abs() < 1e-6);
        assert!((cfg.scrim_alpha() - DEFAULT_SCRIM_ALPHA).abs() < 1e-6);
        assert_eq!(cfg.scrim_style(), "auto");
        cfg.set_popup_transparency(0.55, true);
        cfg.set_scrim_alpha(0.60, true);
        cfg.set_scrim_style("frost".into());

        // 切回无壁纸：原来那档还在。
        cfg.set_wallpaper("");
        assert!((cfg.popup_transparency() - 0.10).abs() < 1e-6);
        assert!((cfg.scrim_alpha() - 0.22).abs() < 1e-6);
        assert_eq!(cfg.scrim_style(), "dark");

        // 再开：有壁纸档回来。
        cfg.set_wallpaper("builtin:miku");
        assert!((cfg.popup_transparency() - 0.55).abs() < 1e-6);
        assert!((cfg.scrim_alpha() - 0.60).abs() < 1e-6);
        assert_eq!(cfg.scrim_style(), "frost");

        // 另一主题不受影响。
        cfg.set_theme("mterm-light".into());
        assert!((cfg.popup_transparency() - 0.0).abs() < 1e-6);
        assert_eq!(cfg.scrim_style(), "auto");
    }

    #[test]
    fn rev4_migrates_theme_pref_and_splits_overlay() {
        // (#theme-split rev4 §5) 一直在默认值上的老用户：
        // theme_pref=dark → mterm-dark；overlay → panel(旧值)+term(1.0)；
        // 默认壁纸 → 无。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme_pref":"dark","defaults_rev":3}"#,
        )
        .unwrap();
        // serde 默认 wallpaper = builtin:tech（从未挑过）。
        assert_eq!(cfg.wallpaper, "builtin:tech");
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "mterm-dark");
        assert!(!cfg.follow_system);
        let av = &cfg.appearance_dark; // theme=mterm-dark → 旧值落暗槽
        assert!((av.titlebar_alpha - 1.0).abs() < f32::EPSILON);
        assert!((av.tabbar_alpha - 1.0).abs() < f32::EPSILON);
        assert!((av.sidebar_alpha - 1.0).abs() < f32::EPSILON);
        assert!((av.term_alpha - 1.0).abs() < f32::EPSILON);
        // 另一套仍是默认值（"第一次使用 = 默认"）
        assert!((cfg.appearance_light.sidebar_alpha - 1.0).abs() < f32::EPSILON);
        assert_eq!(cfg.wallpaper, "");

        // 亮色偏好 → mterm-light。
        let mut cfg: ConfigFile =
            serde_json::from_str(r#"{"theme_pref":"light","defaults_rev":3}"#).unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "mterm-light");

        // 跟随系统 → rev4 先置 follow=true，rev5 再**终结该概念**：落 mterm-dark
        // 且 follow 关掉（#follow-ui-removed）。
        let mut cfg: ConfigFile =
            serde_json::from_str(r#"{"theme_pref":"","defaults_rev":3}"#).unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "mterm-dark");
        assert!(!cfg.follow_system);

        // 用户调过滑杆：三区沿用旧 overlay 值，不覆盖。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme_pref":"dark","wallpaper_overlay":0.62,"defaults_rev":3}"#,
        )
        .unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert!((cfg.appearance_dark.titlebar_alpha - 0.62).abs() < f32::EPSILON);
        assert!((cfg.appearance_dark.tabbar_alpha - 0.62).abs() < f32::EPSILON);
        assert!((cfg.appearance_dark.sidebar_alpha - 0.62).abs() < f32::EPSILON);
        // 亮槽不受影响
        assert!((cfg.appearance_light.titlebar_alpha - 1.0).abs() < f32::EPSILON);

        // 用户挑过壁纸：不被清成无。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme_pref":"dark","wallpaper":"builtin:miku","defaults_rev":3}"#,
        )
        .unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.wallpaper, "builtin:miku");
    }

    #[test]
    fn rev5_retires_follow_system() {
        // (#follow-ui-removed 2026-10-16) 停在跟随态的老配置（follow=true +
        // theme 空）：一次性落成具体主题并关掉跟随 —— 「跟随」从行为上消失。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme":"","follow_system":true,"defaults_rev":4}"#,
        )
        .unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "mterm-dark");
        assert!(!cfg.follow_system);

        // 已经挑过主题的用户：主题原样保留，只把 follow 关掉。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme":"mterm-light","follow_system":true,"defaults_rev":4}"#,
        )
        .unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "mterm-light");
        assert!(!cfg.follow_system);

        // 已是最新 rev：迁移只跑一次，不再改动任何东西。
        let mut cfg = ConfigFile {
            theme: "mterm-light".to_string(),
            follow_system: true,
            defaults_rev: DEFAULTS_REV,
            ..ConfigFile::default()
        };
        assert!(!migrate_defaults(&mut cfg));
        assert!(cfg.follow_system);
    }

    #[test]
    fn wallpaper_transparency_default_migrates_without_overwriting_custom_value() {
        let mut old_default = ConfigFile {
            wallpaper_overlay: PREVIOUS_DEFAULT_WALLPAPER_OVERLAY,
            defaults_rev: 2,
            ..ConfigFile::default()
        };
        assert!(migrate_defaults(&mut old_default));
        assert!((old_default.wallpaper_overlay - 0.85).abs() < f32::EPSILON);

        let mut custom = ConfigFile {
            wallpaper_overlay: 0.70,
            defaults_rev: 2,
            ..ConfigFile::default()
        };
        assert!(migrate_defaults(&mut custom));
        assert!((custom.wallpaper_overlay - 0.70).abs() < f32::EPSILON);
    }

    #[test]
    fn output_highlight_defaults_and_preset_switches() {
        let mut store = temp_store();
        assert!(store.output_highlight_enabled());
        assert!(store.json_format_output());
        // 出厂预设全开（逐条开关，没有 log/devops 两档了）
        assert!(store.output_highlight_disabled_presets().is_empty());

        store.set_output_highlight_enabled(false);
        assert!(!store.output_highlight_enabled());
        store.set_json_format_output(false);
        assert!(!store.json_format_output());
        store.set_output_highlight_enabled(true);

        // 单条预设开关：关 → 进禁用表；再开 → 出来；未知 id 只存不炸
        store.set_output_highlight_preset_enabled("num", false);
        store.set_output_highlight_preset_enabled("paren", false);
        assert_eq!(
            store.output_highlight_disabled_presets(),
            &["num".to_string(), "paren".to_string()]
        );
        store.set_output_highlight_preset_enabled("num", true);
        assert_eq!(
            store.output_highlight_disabled_presets(),
            &["paren".to_string()]
        );
        // 重复关同一条不产生重复项
        store.set_output_highlight_preset_enabled("paren", false);
        assert_eq!(
            store.output_highlight_disabled_presets(),
            &["paren".to_string()]
        );

        store.add_output_highlight_rule(OutputHighlightRule {
            name: "  构建耗时  ".to_string(),
            pattern: "  connection refused  ".to_string(),
            regex: false,
            case_sensitive: false,
            whole_line: true,
            color: "unknown".to_string(),
            enabled: true,
        });
        assert_eq!(store.output_highlight_rules().len(), 1);
        assert_eq!(
            store.output_highlight_rules()[0].pattern,
            "connection refused"
        );
        assert_eq!(store.output_highlight_rules()[0].name, "构建耗时");
        // 任意色：#RRGGBB 原样归一；认不出的值回落红
        assert_eq!(store.output_highlight_rules()[0].color, "#F75464");
        store.set_output_highlight_rule_enabled(0, false);
        assert!(!store.output_highlight_rules()[0].enabled);
        store.remove_output_highlight_rule(0);
        assert!(store.output_highlight_rules().is_empty());

        // 十六进制与旧语义名都归一成 #RRGGBB
        store.add_output_highlight_rule(OutputHighlightRule {
            name: String::new(),
            pattern: "abc".to_string(),
            regex: false,
            case_sensitive: false,
            whole_line: false,
            color: "cyan".to_string(),
            enabled: true,
        });
        assert_eq!(store.output_highlight_rules()[0].color, "#1FC8B4");

        // An older settings file without any of the fields retains the feature
        // that shipped in the previous version: enabled with every preset on.
        // The legacy `output_highlight_preset` key is simply ignored by serde.
        let legacy: ConfigFile = serde_json::from_str(
            r#"{"output_highlight_preset":"devops","output_highlight_rules":[]}"#,
        )
        .unwrap();
        store.cache = legacy;
        assert!(store.output_highlight_enabled());
        assert!(store.output_highlight_disabled_presets().is_empty());
    }

    #[test]
    fn saved_password_encrypts_and_decrypts_without_changes() {
        let mut store = temp_store();
        let password = "p@ss word!^&*中文";
        store.cache.sessions.push(Session {
            name: "windows-password".into(),
            host: "192.168.100.2".into(),
            port: 22,
            user: "root".into(),
            password: Secret::new(password),
            ..Session::new_empty()
        });

        store.save().unwrap();
        let raw = std::fs::read_to_string(&store.path).unwrap();
        assert!(!raw.contains(password));
        let disk: ConfigFile = serde_json::from_str(&raw).unwrap();
        let encrypted = disk.sessions[0].password.as_str();
        assert!(encrypted.starts_with(ConfigStore::ENC_PREFIX));
        assert_eq!(
            ConfigStore::try_decrypt(&store.key, encrypted).as_deref(),
            Some(password)
        );

        let _ = std::fs::remove_file(&store.path);
    }

    #[test]
    fn export_import_roundtrip_preserves_password() {
        let mut a = temp_store();
        a.cache.sessions.push(Session {
            name: "pve".into(),
            host: "192.168.100.2".into(),
            port: 22,
            user: "root".into(),
            password: Secret::new("s3cr3t"),
            ..Session::new_empty()
        });

        let export_path = std::env::temp_dir().join(format!("ms-exp-{}.json", Uuid::new_v4()));
        assert_eq!(a.export_to(&export_path).unwrap(), 1);

        // The file keeps host/user plaintext but the password is obfuscated.
        let raw = std::fs::read_to_string(&export_path).unwrap();
        assert!(raw.contains("192.168.100.2"));
        assert!(raw.contains(ConfigStore::EXPORT_PREFIX));
        assert!(!raw.contains("s3cr3t"));

        // Importing into a fresh store recovers the plaintext password.
        let mut b = temp_store();
        assert_eq!(b.import_from(&export_path).unwrap(), (1, 0));
        assert_eq!(b.cache.sessions.len(), 1);
        assert_eq!(b.cache.sessions[0].password.as_str(), "s3cr3t");
        assert_eq!(b.cache.sessions[0].host, "192.168.100.2");

        // Re-importing the same file skips the duplicate.
        assert_eq!(b.import_from(&export_path).unwrap(), (0, 1));

        let _ = std::fs::remove_file(&export_path);
        let _ = std::fs::remove_file(&a.path);
        let _ = std::fs::remove_file(&b.path);
    }

    #[test]
    fn imports_finalshell_export_and_reencrypts_password_at_rest() {
        let mut store = temp_store();
        let raw = r#"{
            "conection_type": 100,
            "name": "FinalShell host",
            "host": "192.0.2.20",
            "port": 22,
            "user_name": "operator",
            "password": "AwcLDRETFx1OXQgZJNatCplesw+x/P04",
            "authentication_type": 1,
            "terminal_encoding": "UTF-8"
        }"#;

        assert_eq!(store.import_json(raw).unwrap(), (1, 0));
        let session = &store.cache.sessions[0];
        assert_eq!(session.host, "192.0.2.20");
        assert_eq!(session.user, "operator");
        assert_eq!(session.password.as_str(), "meatshell-test");

        let saved = std::fs::read_to_string(&store.path).unwrap();
        assert!(!saved.contains("meatshell-test"));
        let disk: ConfigFile = serde_json::from_str(&saved).unwrap();
        assert!(disk.sessions[0]
            .password
            .as_str()
            .starts_with(ConfigStore::ENC_PREFIX));

        let _ = std::fs::remove_file(&store.path);
    }

    #[test]
    fn issue_300_interface_defaults_and_ranges_are_safe() {
        let mut store = temp_store();

        // Legacy configs keep the safe confirmation and familiar paste aliases.
        store.cache = serde_json::from_str("{}").unwrap();
        assert!(store.paste_confirm_enabled());
        assert!(store.extra_paste_shortcuts_enabled());
        assert!(!store.zen_mode());
        assert_eq!(store.terminal_line_spacing(), 1.0);

        store.set_terminal_line_spacing(0.1);
        assert_eq!(store.terminal_line_spacing(), 0.8);
        store.set_terminal_line_spacing(9.0);
        assert_eq!(store.terminal_line_spacing(), 1.5);

        store.set_paste_confirm_enabled(false);
        store.set_extra_paste_shortcuts_enabled(false);
        store.set_zen_mode(true);
        assert!(!store.paste_confirm_enabled());
        assert!(!store.extra_paste_shortcuts_enabled());
        assert!(store.zen_mode());
    }

    #[test]
    fn reorder_session_swaps_same_group_siblings_only() {
        let mut store = temp_store();
        let mk = |id: &str, group: &str| Session {
            id: id.into(),
            name: id.into(),
            group: group.into(),
            ..Session::new_empty()
        };
        store.cache.sessions = vec![mk("a", ""), mk("x", "ops"), mk("b", ""), mk("c", "")];

        // Moving "c" up swaps it with the nearest ungrouped sibling ("b"),
        // leaving the grouped session in between untouched.
        assert!(store.reorder_session("c", -1));
        assert_eq!(
            store
                .sessions()
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "x", "c", "b"]
        );
        // "a" is already the first ungrouped session; "x" is alone in its
        // group; unknown ids are no-ops.
        assert!(!store.reorder_session("a", -1));
        assert!(!store.reorder_session("x", 1));
        assert!(!store.reorder_session("nope", 1));
    }
}

