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
/// dark"的坑）+ 跟随系统 + graphite 族默认主题（已确认决策：新老用户一律新底色）。
fn fresh_config() -> ConfigFile {
    ConfigFile {
        wallpaper: String::new(),
        welcome_as_sidebar: true,
        sidebar_dock: "right".to_string(),
        wallpaper_overlay: DEFAULT_WALLPAPER_OVERLAY,
        panel_alpha: DEFAULT_PANEL_ALPHA,
        term_alpha: DEFAULT_TERM_ALPHA,
        follow_system: true,
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
        // Never enabled the welcome sidebar → enable it.
        if !cfg.welcome_as_sidebar {
            cfg.welcome_as_sidebar = true;
        }
        // Never moved the resource panel (empty = the old left default) → right.
        if cfg.sidebar_dock.trim().is_empty() {
            cfg.sidebar_dock = "right".to_string();
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
        // 深浅偏好 → 具体主题 id（graphite 族，已确认决策：一律新底色）。
        // 空/"system" = 跟随系统（follow_system = true），theme 留空运行时解析。
        // （follow_system 的 serde 默认是 true——dark/light 分支必须显式关掉。）
        if cfg.theme.is_empty() {
            match cfg.theme_pref.as_str() {
                "dark" => {
                    cfg.theme = "graphite-dark".to_string();
                    cfg.follow_system = false;
                }
                "light" => {
                    cfg.theme = "graphite-light".to_string();
                    cfg.follow_system = false;
                }
                _ => cfg.follow_system = true,
            }
        }
        // 一个滑杆曾同时驱动面板+终端 → 拆成两个独立值（面板沿用旧值，
        // 终端回满档不透明——现状终端本来就更实，视觉不回退，§5）。
        if cfg.panel_alpha == default_panel_alpha_value() {
            cfg.panel_alpha = if cfg.wallpaper_overlay > 0.0 {
                cfg.wallpaper_overlay
            } else {
                DEFAULT_PANEL_ALPHA
            };
            cfg.term_alpha = DEFAULT_TERM_ALPHA;
        }
        // 从未挑过壁纸的用户 → 默认无壁纸（serde 默认 builtin:tech = 未动过）。
        if cfg.wallpaper == "builtin:tech" {
            cfg.wallpaper = String::new();
        }
    }
    cfg.defaults_rev = DEFAULTS_REV;
    true
}

/// 与 serde 默认区分的比较基准（panel_alpha 的 serde default 就是 DEFAULT，
/// 所以「用户没动过」与「默认值」无法靠值区分——rev4 只对 defaults_rev<4 的
/// 配置跑一次，覆盖旧配置里 panel_alpha 尚不存在（=serde 默认 0.85）的情况）。
fn default_panel_alpha_value() -> f32 {
    DEFAULT_PANEL_ALPHA
}

fn normalize_highlight_color(color: &str) -> &'static str {
    match color {
        "yellow" => "yellow",
        "green" => "green",
        "cyan" => "cyan",
        "magenta" => "magenta",
        "gray" => "gray",
        _ => "red",
    }
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
    /// 面板不透明度（0.30–1.00）。
    pub fn panel_alpha(&self) -> f32 {
        self.cache.panel_alpha
    }
    pub fn set_panel_alpha(&mut self, v: f32) {
        self.cache.panel_alpha = v.clamp(0.30, 1.0);
    }
    /// 终端不透明度（0.75–1.00，下限保护 ANSI 可读）。
    pub fn term_alpha(&self) -> f32 {
        self.cache.term_alpha
    }
    pub fn set_term_alpha(&mut self, v: f32) {
        self.cache.term_alpha = v.clamp(0.75, 1.0);
    }
    /// 壁纸图自身可见度（0.00–1.00）。
    pub fn wallpaper_visible(&self) -> f32 {
        self.cache.wallpaper_visible
    }
    pub fn set_wallpaper_visible(&mut self, v: f32) {
        self.cache.wallpaper_visible = v.clamp(0.0, 1.0);
    }
    /// 弹窗通透度三档（0/1/2）。
    pub fn popup_transparency(&self) -> i32 {
        self.cache.popup_transparency.clamp(0, 2)
    }
    pub fn set_popup_transparency(&mut self, v: i32) {
        self.cache.popup_transparency = v.clamp(0, 2);
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

    /// Selected built-in rule set. Unknown values safely fall back to the
    /// conservative log-level preset for forward/backward compatibility.
    pub fn output_highlight_preset(&self) -> &str {
        match self.cache.output_highlight_preset.as_str() {
            "devops" => "devops",
            _ => "log",
        }
    }

    pub fn set_output_highlight_preset(&mut self, preset: String) {
        self.cache.output_highlight_preset = match preset.as_str() {
            "devops" => "devops".to_string(),
            _ => "log".to_string(),
        };
    }

    pub fn output_highlight_rules(&self) -> &[OutputHighlightRule] {
        &self.cache.output_highlight_rules
    }

    pub fn add_output_highlight_rule(&mut self, mut rule: OutputHighlightRule) {
        rule.pattern = rule.pattern.trim().to_string();
        rule.color = normalize_highlight_color(&rule.color).to_string();
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

    /// Immersive wallpaper id ("" = none).
    pub fn wallpaper(&self) -> &str {
        &self.cache.wallpaper
    }

    pub fn set_wallpaper(&mut self, id: impl Into<String>) {
        self.cache.wallpaper = id.into();
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
    pub fn collapse_sidebar_default(&self) -> bool {
        self.cache.collapse_sidebar_default
    }

    pub fn set_collapse_sidebar_default(&mut self, v: bool) {
        self.cache.collapse_sidebar_default = v;
    }

    /// Persisted sidebar width in logical px. Falls back to the default when the
    /// stored value is unset/zero (e.g. a config created via `Default`).
    pub fn sidebar_width(&self) -> f32 {
        let w = self.cache.sidebar_width;
        if w <= 0.0 {
            default_sidebar_width()
        } else {
            w
        }
    }

    pub fn set_sidebar_width(&mut self, v: f32) {
        self.cache.sidebar_width = v;
    }

    /// Resource / SFTP panel docking geometry, persisted across restarts (#dock).
    /// Sizes fall back to their defaults when unset/zero; docks fall back to a
    /// sensible edge when the stored string is empty.
    pub fn sidebar_height(&self) -> f32 {
        let h = self.cache.sidebar_height;
        if h <= 0.0 {
            default_sidebar_height()
        } else {
            h
        }
    }
    pub fn set_sidebar_height(&mut self, v: f32) {
        self.cache.sidebar_height = v;
    }
    pub fn sidebar_dock(&self) -> String {
        let d = self.cache.sidebar_dock.trim();
        if d.is_empty() {
            "left".into()
        } else {
            d.to_string()
        }
    }
    pub fn set_sidebar_dock(&mut self, v: String) {
        self.cache.sidebar_dock = v;
    }
    pub fn sidebar_collapsed(&self) -> Option<bool> {
        self.cache.sidebar_collapsed
    }
    pub fn set_sidebar_collapsed(&mut self, v: bool) {
        self.cache.sidebar_collapsed = Some(v);
    }
    pub fn welcome_as_sidebar(&self) -> bool {
        self.cache.welcome_as_sidebar
    }
    pub fn set_welcome_as_sidebar(&mut self, v: bool) {
        self.cache.welcome_as_sidebar = v;
    }
    pub fn welcome_sidebar_width(&self) -> f32 {
        let w = self.cache.welcome_sidebar_width;
        if w <= 0.0 {
            240.0
        } else {
            w
        }
    }
    pub fn set_welcome_sidebar_width(&mut self, v: f32) {
        self.cache.welcome_sidebar_width = v;
    }
    pub fn welcome_sidebar_dock(&self) -> String {
        let d = self.cache.welcome_sidebar_dock.trim();
        if d.is_empty() {
            "left".into()
        } else {
            d.to_string()
        }
    }
    pub fn set_welcome_sidebar_dock(&mut self, v: String) {
        self.cache.welcome_sidebar_dock = v;
    }
    pub fn welcome_collapsed(&self) -> Option<bool> {
        self.cache.welcome_collapsed
    }
    pub fn set_welcome_collapsed(&mut self, v: bool) {
        self.cache.welcome_collapsed = Some(v);
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
        // (#theme-split rev4) Fresh install: 默认无壁纸 + 跟随系统 + graphite 族
        // + 三滑杆默认值（§0/§5：零操作即正确）。
        let fresh = fresh_config();
        assert_eq!(fresh.wallpaper, "");
        assert!((fresh.wallpaper_overlay - 0.85).abs() < f32::EPSILON);
        assert!((fresh.panel_alpha - 0.85).abs() < f32::EPSILON);
        assert!((fresh.term_alpha - 1.0).abs() < f32::EPSILON);
        assert!(fresh.follow_system);
        assert_eq!(fresh.theme, "");
        // User upgrading from before the feature: JSON without the key keeps the
        // serde default (builtin:tech) so rev4 can tell "never chose" apart.
        let cfg: ConfigFile = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.wallpaper, "builtin:tech");
        assert_eq!(cfg.theme, "");
        assert!((cfg.panel_alpha - 0.85).abs() < f32::EPSILON);
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

    #[test]
    fn rev4_migrates_theme_pref_and_splits_overlay() {
        // (#theme-split rev4 §5) 一直在默认值上的老用户：
        // theme_pref=dark → graphite-dark；overlay → panel(旧值)+term(1.0)；
        // 默认壁纸 → 无。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme_pref":"dark","defaults_rev":3}"#,
        )
        .unwrap();
        // serde 默认 wallpaper = builtin:tech（从未挑过）。
        assert_eq!(cfg.wallpaper, "builtin:tech");
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "graphite-dark");
        assert!(!cfg.follow_system);
        assert!((cfg.panel_alpha - 0.85).abs() < f32::EPSILON);
        assert!((cfg.term_alpha - 1.0).abs() < f32::EPSILON);
        assert_eq!(cfg.wallpaper, "");

        // 亮色偏好 → graphite-light。
        let mut cfg: ConfigFile =
            serde_json::from_str(r#"{"theme_pref":"light","defaults_rev":3}"#).unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "graphite-light");

        // 跟随系统 → theme 留空 + follow_system=true。
        let mut cfg: ConfigFile =
            serde_json::from_str(r#"{"theme_pref":"","defaults_rev":3}"#).unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.theme, "");
        assert!(cfg.follow_system);

        // 用户调过滑杆：panel 沿用旧 overlay 值，不覆盖。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme_pref":"dark","wallpaper_overlay":0.62,"defaults_rev":3}"#,
        )
        .unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert!((cfg.panel_alpha - 0.62).abs() < f32::EPSILON);

        // 用户挑过壁纸：不被清成无。
        let mut cfg: ConfigFile = serde_json::from_str(
            r#"{"theme_pref":"dark","wallpaper":"builtin:miku","defaults_rev":3}"#,
        )
        .unwrap();
        assert!(migrate_defaults(&mut cfg));
        assert_eq!(cfg.wallpaper, "builtin:miku");
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
    fn output_highlight_defaults_and_preset_validation() {
        let mut store = temp_store();
        assert!(store.output_highlight_enabled());
        assert!(store.json_format_output());
        assert_eq!(store.output_highlight_preset(), "log");

        store.set_output_highlight_enabled(false);
        store.set_output_highlight_preset("devops".to_string());
        assert!(!store.output_highlight_enabled());
        store.set_json_format_output(false);
        assert!(!store.json_format_output());
        assert_eq!(store.output_highlight_preset(), "devops");

        store.set_output_highlight_preset("future-preset".to_string());
        assert_eq!(store.output_highlight_preset(), "log");

        store.add_output_highlight_rule(OutputHighlightRule {
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
        assert_eq!(store.output_highlight_rules()[0].color, "red");
        store.set_output_highlight_rule_enabled(0, false);
        assert!(!store.output_highlight_rules()[0].enabled);
        store.remove_output_highlight_rule(0);
        assert!(store.output_highlight_rules().is_empty());

        // An older settings file without either field retains the feature that
        // shipped in the previous version: enabled with the log preset.
        let legacy: ConfigFile = serde_json::from_str("{}").unwrap();
        store.cache = legacy;
        assert!(store.output_highlight_enabled());
        assert_eq!(store.output_highlight_preset(), "log");
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

