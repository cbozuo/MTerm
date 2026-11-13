//! (#double-shift-v1 2026-11-11) 全局搜索单元测试：trigger / query / rank / classify。
//!
//! 领域层纯逻辑（无 Slint、无 window），照 `tests/app/command_hint/` 的
//! 「crate 内测试」口径直接 `use super::…` 取 pub(crate) 项。
//! 执行语义（五类 Enter/Ctrl+Enter）依赖 window 回调，归宿主桥的离屏
//! 测试（examples/quick_search_shot.rs）与实机验证覆盖，这里不测。

use super::trigger::{DoubleShiftTracker, KeyEvent};
use super::{
    build_index, group_and_cap, recent_items, search, tokenize, Category, MatchRow, Payload,
    SearchIndex, SearchItem, SessionView,
};
use crate::config::ConfigStore;
use std::collections::HashMap;
use std::time::{Duration, Instant};


/// (#qs-trigger-global 2026-11-13) classify 已从生产代码退役（触发判定收敛到
/// winit 钩子直接分类），本测试文件把归一逻辑原样内联，继续守护状态机语义。
fn classify(key: &str, ctrl: bool, alt: bool, shift: bool) -> KeyEvent<'_> {
    let shift_code = key == "\u{0010}" || key == "\u{0015}";
    if key.is_empty() || shift_code {
        if ctrl || alt {
            return KeyEvent::Other;
        }
        if shift {
            return KeyEvent::Shift;
        }
        if shift_code {
            return KeyEvent::ImeMarker(key);
        }
        return KeyEvent::Other;
    }
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        let cp = c as u32;
        let standalone = matches!(cp, 0x08 | 0x09 | 0x0A | 0x0D | 0x1B);
        if (0x01..=0x1f).contains(&cp) && !standalone {
            return KeyEvent::ImeMarker(key);
        }
    }
    KeyEvent::Other
}

// ── 工具 ─────────────────────────────────────────────────────────────

fn item(id: &str, cat: Category, title: &str, kw: &str, open: bool, rank: i32) -> SearchItem {
    SearchItem::new(
        id.to_string(),
        cat,
        title.to_string(),
        String::new(),
        String::new(),
        kw.to_string(),
        open,
        rank,
        Payload::None,
    )
}

/// 空 SearchIndex 的快捷构造（测试里逐条 push）。
fn empty_index() -> SearchIndex {
    SearchIndex::default()
}

/// 内存 store：只读 getter（sessions/quick_commands/history_entries）走空默认。
/// `ConfigStore` 字段 pub(crate)，crate 内测试可整体构造（command_hint 同款）。
fn mem_store() -> ConfigStore {
    ConfigStore {
        path: std::path::PathBuf::from("unused.json"),
        cache: crate::config::ConfigFile::default(),
        key: [7u8; 32],
    }
}

/// 测试会话：`Session` 无 Default（Secret 字段），走 serde 反序列化 ——
/// 只给五个无 serde(default) 的必填字段，其余全部落默认值。
fn mk_session(id: &str, name: &str, host: &str, port: u16, user: &str) -> crate::config::Session {
    serde_json::from_str(&format!(
        r#"{{"id":"{id}","name":"{name}","host":"{host}","port":{port},"user":"{user}","auth":"password"}}"#
    ))
    .expect("test session json")
}

// ── trigger：双击 Shift 状态机 ───────────────────────────────────────

#[test]
fn double_shift_inside_the_window_opens() {
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    assert!(!tr.on_key(KeyEvent::Shift, t0, false));
    let t1 = t0 + Duration::from_millis(180);
    assert!(tr.on_key(KeyEvent::Shift, t1, false));
    // 命中后回到待机。
    assert!(tr.idle());
}

#[test]
fn double_shift_edges_80_and_360_are_inclusive() {
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    assert!(tr.on_key(KeyEvent::Shift, t0 + Duration::from_millis(80), false));

    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    // 360 命中后本次重置，下一次按下是全新起点。
    assert!(tr.on_key(KeyEvent::Shift, t0 + Duration::from_millis(360), false));
    assert!(tr.idle());
}

#[test]
fn double_shift_too_fast_becomes_a_new_start() {
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    // 79ms：太快 → 不开，但本次按下成为新 t0。
    assert!(!tr.on_key(KeyEvent::Shift, t0 + Duration::from_millis(79), false));
    assert!(!tr.idle());
    assert!(tr.on_key(KeyEvent::Shift, t0 + Duration::from_millis(79 + 150), false));
}

#[test]
fn double_shift_too_slow_misses_and_rearms() {
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    // 361ms：超窗 → 不开，重臂。
    assert!(!tr.on_key(KeyEvent::Shift, t0 + Duration::from_millis(361), false));
    assert!(!tr.idle());
}

#[test]
fn other_keys_reset_the_tracker() {
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    tr.on_key(KeyEvent::Other, t0 + Duration::from_millis(100), false);
    assert!(tr.idle(), "any other key clears the armed state");
    // 打字之后的第一击只重新武装（不打开）。
    assert!(!tr.on_key(KeyEvent::Shift, t0 + Duration::from_millis(200), false));
    assert!(!tr.idle(), "re-armed for the second press");
}

#[test]
fn ime_markers_are_transparent_to_the_state_machine() {
    // 百度注入的 C0 标记与真实 Shift 同码位，靠 modifiers.shift 区分：
    // shift=false（IME 钩子不改键盘 state）→ 标记，透明处理。
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    // 标记先/后到都忽略。
    tr.on_key(classify("\u{0015}", false, false, false), t0 + Duration::from_millis(30), false);
    assert!(!tr.idle(), "IME marker must not clear the first press");
    assert!(tr.on_key(KeyEvent::Shift, t0 + Duration::from_millis(200), false));
}

#[test]
fn trailing_backspace_after_right_shift_resets() {
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    // 右 Shift 场景 ~900ms 后尾随退格：清零（防御性，此时本就超窗）。
    tr.on_key(KeyEvent::Other, t0 + Duration::from_millis(900), false);
    assert!(tr.idle());
}

#[test]
fn open_palette_ignores_further_double_shift() {
    let mut tr = DoubleShiftTracker::new();
    let now = Instant::now();
    // 面板已开：双击不重复打开，也不进入待武装态。
    assert!(!tr.on_key(KeyEvent::Shift, now, true));
    assert!(tr.idle());
}

#[test]
fn classify_maps_the_three_real_shapes() {
    // 裸 Shift 的真实形态：Slint 键码表把 Shift/ShiftR 定在
    // U+0010 / U+0015（i-slint-common-1.18.1/key_codes.rs:40,46），
    // winit 后端把它们当普通 text 送上来（非空，不会被丢弃）。
    assert!(matches!(classify("\u{0010}", false, false, true), KeyEvent::Shift));
    assert!(matches!(classify("\u{0015}", false, false, true), KeyEvent::Shift));
    // 空串形态（app.rs 历史注释记的形态）也接。
    assert!(matches!(classify("", false, false, true), KeyEvent::Shift));
    // 修饰组合不是裸 Shift。
    assert!(matches!(classify("", true, false, true), KeyEvent::Other));
    assert!(matches!(classify("\u{0010}", true, false, true), KeyEvent::Other));
    // ★ 百度 IME 注入的标记与真实 Shift 同码位：靠 modifiers.shift 区分。
    //   IME 钩子不改键盘 state → shift=false → 判为标记（透明处理）。
    assert!(matches!(classify("\u{0015}", false, false, false), KeyEvent::ImeMarker(_)));
    assert!(matches!(classify("\u{0010}", false, false, false), KeyEvent::ImeMarker(_)));
    // 独立功能键（退格/Tab）不是标记。
    assert!(matches!(classify("\u{0008}", false, false, false), KeyEvent::Other));
    assert!(matches!(classify("a", false, false, false), KeyEvent::Other));
}

#[test]
fn real_shift_double_click_opens_even_with_ime_markers_around() {
    // 真实场景：百度拼音开着，用户按两下左 Shift。
    // Slint 侧裸 Shift 的码位（U+0010/U+0015）与 IME 注入的标记**同码位**，
    // 无法区分 —— 标记必须「透明 + 重新武装」，而不是清零（清零会让装了
    // 百度拼音的用户永远命中不了双击）。
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    // 第一击：真实左 Shift（U+0010）。
    assert!(!tr.on_key(classify("\u{0010}", false, false, true), t0, false));
    // 紧随其后的 IME 标记（同一消息批次，间隔几 ms）：重新武装到标记时刻。
    assert!(!tr.on_key(classify("\u{0015}", false, false, true), t0 + Duration::from_millis(3), false));
    // 第二击：真实左 Shift，距标记约 180ms → 命中。
    assert!(tr.on_key(classify("\u{0010}", false, false, true), t0 + Duration::from_millis(183), false));
}

#[test]
fn ime_marker_arriving_late_still_allows_the_second_press() {
    // 标记晚到（IME 注入排在真实 keydown 之后）：标记把起点推后几 ms，
    // 第二击仍在 80–360ms 窗口内 → 命中。
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    assert!(!tr.on_key(classify("\u{0010}", false, false, true), t0, false));
    assert!(!tr.on_key(classify("\u{0015}", false, false, true), t0 + Duration::from_millis(40), false));
    assert!(tr.on_key(classify("\u{0010}", false, false, true), t0 + Duration::from_millis(220), false));
}

#[test]
fn reset_paths_clear_the_armed_state() {
    // 设计稿 §03 reset-paths：鼠标按下 / 窗口失焦 → 清零。
    let mut tr = DoubleShiftTracker::new();
    let t0 = Instant::now();
    tr.on_key(KeyEvent::Shift, t0, false);
    assert!(!tr.idle());
    tr.reset();
    assert!(tr.idle(), "explicit reset must clear the armed state");
    // 复位后再按两下 → 第一击重新武装，第二击命中（复位不破坏后续双击）。
    let t1 = t0 + Duration::from_millis(500);
    assert!(!tr.on_key(KeyEvent::Shift, t1, false));
    assert!(tr.on_key(KeyEvent::Shift, t1 + Duration::from_millis(150), false));
}

// ── query：分词 / AND / 分层评分 ─────────────────────────────────────

#[test]
fn tokenize_splits_whitespace_and_lowercases() {
    assert_eq!(tokenize("Docker PS"), vec!["docker", "ps"]);
    assert!(tokenize("   ").is_empty());
}

#[test]
fn every_token_must_match() {
    let idx = build_index(&mem_store(), &SessionView {
        open_ids: &Default::default(),
        recent_rank: &Default::default(),
    });
    // 索引里必有设置「主题」：docker + 主题 两词 AND 无一命中。
    let hit = search(&idx, "docker 主题", None).unwrap();
    assert!(hit.is_empty(), "AND semantics must reject partial matches");
}

#[test]
fn exact_beats_prefix_beats_substring_beats_subsequence() {
    let mut idx = empty_index();
    idx.items.push(item("a", Category::Session, "docker", "docker", false, 0));
    idx.items.push(item("b", Category::Session, "dockerx", "dockerx", false, 0));
    idx.items.push(item("c", Category::Session, "my-docker-thing", "docker", false, 0));
    // 子序列命中只来自 title（kw 不含查询词，避免走精确层干扰分层对照）。
    idx.items.push(item("d", Category::Session, "doc-keeper", "synthetic", false, 0));
    let hits = search(&idx, "docker", None).unwrap();
    let (_, flat) = group_and_cap(hits, None);
    let order: Vec<&str> = flat.iter().map(|r| r.item.id.as_str()).collect();
    // 精确 > 前缀 > 词界子串 > 子序列。
    assert_eq!(order, vec!["a", "b", "c", "d"]);
}

#[test]
fn open_sessions_get_a_boost_and_rank_higher() {
    let mut idx = empty_index();
    // 同名同分：已打开者胜。
    idx.items.push(item("closed", Category::Session, "web-prod", "web-prod", false, 0));
    idx.items.push(item("opened", Category::Session, "web-prod", "web-prod", true, 0));
    let hits = search(&idx, "web", None).unwrap();
    let (_, flat) = group_and_cap(hits, Some(Category::Session));
    assert_eq!(flat[0].item.id, "opened");
    assert_eq!(flat[0].score - flat[1].score, 40);
}

#[test]
fn recent_rank_adds_a_bounded_boost() {
    let mut idx = empty_index();
    idx.items.push(item("old", Category::Session, "db-primary", "db-primary", false, 0));
    idx.items.push(item("new", Category::Session, "db-primary", "db-primary", false, 1));
    let hits = search(&idx, "db", None).unwrap();
    let (_, flat) = group_and_cap(hits, Some(Category::Session));
    assert_eq!(flat[0].item.id, "new");
    assert_eq!(flat[0].score - flat[1].score, 11); // max(0, 12-1) - 0
}

// ── rank：分组 / 截断 / 平手 / 最近使用 ─────────────────────────────

fn nine_sessions(prefix: &str) -> Vec<SearchItem> {
    (0..9).map(|i| item(&format!("{prefix}{i}"), Category::Session, &format!("s{i}"), &format!("s{i}"), false, 0)).collect()
}

#[test]
fn all_view_caps_each_group_at_8_and_orders_categories() {
    let mut idx = empty_index();
    idx.items.extend(nine_sessions("s"));
    idx.items.push(item("z1", Category::Command, "cmd-a", "cmd-a", false, 0));
    idx.items.push(item("z2", Category::Setting, "theme", "theme", false, 0));
    idx.items.push(item("z3", Category::Action, "about", "about", false, 0));
    // 直接走 rank 层（不经查询，全量喂入，覆盖「组序 + 每组上限」）。
    let all: Vec<MatchRow> = idx
        .items
        .iter()
        .map(|it| MatchRow { item: it, score: 0 })
        .collect();
    let (groups, flat) = group_and_cap(all, None);
    assert_eq!(groups.len(), 4, "session/command/setting/action all present");
    assert_eq!(groups[0].cat, Category::Session);
    assert_eq!(groups[0].items.len(), 8, "per-group cap");
    assert_eq!(groups[1].cat, Category::Command);
    assert_eq!(flat.len(), 8 + 1 + 1 + 1);
}

#[test]
fn single_category_view_is_flat_and_capped() {
    let mut idx = empty_index();
    idx.items.extend(nine_sessions("s"));
    let mut rows: Vec<super::MatchRow> = idx
        .items
        .iter()
        .map(|it| super::MatchRow { item: it, score: 0 })
        .collect();
    rows.reverse(); // 打乱输入顺序，验证 rank 层排序
    let (groups, flat) = group_and_cap(rows, Some(Category::Session));
    assert_eq!(groups.len(), 1);
    assert_eq!(flat.len(), 9, "single category capped at 40, 9 < 40");
}

#[test]
fn tie_break_is_title_alphabetical() {
    let mut idx = empty_index();
    idx.items.push(item("b", Category::Action, "b-item", "x", false, 0));
    idx.items.push(item("a", Category::Action, "a-item", "x", false, 0));
    let mut rows: Vec<super::MatchRow> = idx
        .items
        .iter()
        .map(|it| super::MatchRow { item: it, score: 0 })
        .collect();
    let (_, flat) = group_and_cap(rows, Some(Category::Action));
    assert_eq!(flat[0].item.id, "a");
    assert_eq!(flat[1].item.id, "b");
}

#[test]
fn recent_items_follow_ids_then_pad_with_sessions() {
    let mut idx = empty_index();
    idx.items.push(item("act:about", Category::Action, "关于", "关于", false, 0));
    idx.items.push(item("session:s1", Category::Session, "s1", "s1", false, 0));
    let recent = recent_items(&idx, &["act:about".to_string()]);
    assert_eq!(recent[0].item.id, "act:about");
    assert_eq!(recent.len(), 2, "padded with a session");
    assert_eq!(recent[1].item.cat, Category::Session);
}

// ── index：五类数据落进一张索引 ─────────────────────────────────────

#[test]
fn index_counts_reflect_all_sources() {
    let mut store = mem_store();
    store.cache.quick_commands.push(crate::config::QuickCommand {
        name: "重启 Docker".into(),
        command: "sudo systemctl restart docker".into(),
        group: "Docker".into(),
        note: "守护进程会短暂中断".into(),
        send_enter: true,
    });
    store.cache.history_entries.push(crate::config::HistoryEntry {
        cmd: "git status".into(),
        last_used: 0,
        uses: 1,
        pinned: false,
    });
    let open_ids: std::collections::HashSet<String> = ["sess-1".to_string()].into();
    let mut rank_map = HashMap::new();
    rank_map.insert("sess-1".to_string(), 1i32);

    // 塞一条已保存会话。
    store.cache.sessions.push(mk_session("sess-1", "web-prod", "10.24.8.11", 22, "root"));

    let idx = build_index(&store, &SessionView { open_ids: &open_ids, recent_rank: &rank_map });

    assert!(idx.counts.session >= 1 + 1, "saved + builtin local");
    assert_eq!(idx.counts.command, 2, "quick + history");
    assert!(idx.counts.setting >= 14, "static catalog");
    assert!(idx.counts.action >= 18, "static catalog");
    assert_eq!(idx.counts.all, idx.items.len());

    // 已打开会话：open 标记 + recent_rank 生效。
    let hit = search(&idx, "web-prod", None).unwrap();
    assert!(!hit.is_empty());
    let top = hit.iter().find(|r| r.item.title == "web-prod").unwrap();
    assert!(top.item.open, "open_ids must mark the session");

    // 中文标题可检索（快捷命令名）。
    let hit = search(&idx, "重启", None).unwrap();
    assert!(hit.iter().any(|r| r.item.title == "重启 Docker"));

    // 命令预览载荷。
    let hit = search(&idx, "restart docker", None).unwrap();
    let cmd = hit.iter().find(|r| r.item.title == "重启 Docker").unwrap();
    assert!(matches!(&cmd.item.payload, Payload::Command { cmd, .. } if cmd.contains("restart docker")));

    // 设置别名（英文）可检索；动作拼音可检索。
    assert!(search(&idx, "wallpaper", None).unwrap().iter().any(|r| matches!(r.item.payload, Payload::Setting { .. })));
    assert!(search(&idx, "guanyu", None).unwrap().iter().any(|r| r.item.id == "act:about"));

    // SSH 复制载荷（非 22 端口带 -p）。
    store.cache.sessions.push(mk_session("sess-2", "gateway", "203.0.113.10", 2222, "ops"));
    let idx = build_index(&store, &SessionView {
        open_ids: &Default::default(),
        recent_rank: &Default::default(),
    });
    let hit = search(&idx, "gateway", None).unwrap().pop().unwrap();
    assert!(matches!(&hit.item.payload, Payload::Session { ssh_cmd, .. } if ssh_cmd.contains("-p 2222")));
}
