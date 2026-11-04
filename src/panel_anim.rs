//! 侧栏收展动效领域层（#panel-anim 2026-11-05）。
//!
//! 对应高保真 `docs/design/two-sidebar-hifi-2026-11-04.html`（V9）：左右两栏
//! 收起/展开的过渡形态共 6 种（瞬时/推挤/吞没/手风琴/归巢/果冻），一套配置
//! （形态 / 时长 80–600ms / 缓动 / 减少动效）同时作用于左右两侧。本模块负责：
//!
//! * 词表与出厂值（Slint 侧 `global PanelAnim` 的同源常量）；
//! * 配置值的归一化（手改 sessions.json 的容错回落，参照 right_sidebar_dock
//!   的读取端回落范式）；
//! * 4 组预设（跟手/顺滑/电影感/活泼）与「自定义」判定；
//! * Windows 系统级「减少动效」检测（SPI_GETCLIENTAREAANIMATION，照抄
//!   app.rs 的 `SystemParametersInfoW` extern 先例，零新依赖）。
//!
//! 布局与动画本体在 `ui/panel_anim.slint`（global）与 `ui/app.slint`（容器
//! 动画属性）；本文件不做任何 UI，保证可单测。

/// 收展形态词表，即设置页「形态」六选的存储值（与高保真 §02 卡片一一对应）。
pub const SHAPES: [&str; 6] = ["instant", "push", "swallow", "accordion", "homing", "jelly"];
/// 缓动词表：标准 / 顺滑 / 弹性（cubic-bezier 见 ui/panel_anim.slint 映射）。
pub const EASINGS: [&str; 3] = ["standard", "smooth", "spring"];

pub const SHAPE_DEFAULT: &str = "push";
pub const EASING_DEFAULT: &str = "smooth";
pub const MS_DEFAULT: u32 = 240;
/// 高保真 §01 时长滑块范围：80–600ms，UI 侧步进 10。
pub const MS_MIN: u32 = 80;
pub const MS_MAX: u32 = 600;

/// 预设词表（高保真 §04）：key →（形态，时长，缓动）。
/// 「顺滑」为落盘默认；设置页 chips 一键套用后即为这套三元组。
pub const PRESETS: [(&str, &str, u32, &str); 4] = [
    ("follow", "push", 120, "standard"),
    ("smooth", "push", 240, "smooth"),
    ("cinema", "swallow", 320, "smooth"),
    ("lively", "jelly", 380, "spring"),
];

/// 归一化形态值：大小写/首尾空白宽容，非法回落 [`SHAPE_DEFAULT`]。
pub fn normalize_shape(raw: &str) -> &'static str {
    let v = raw.trim().to_ascii_lowercase();
    SHAPES.iter().copied().find(|s| *s == v).unwrap_or(SHAPE_DEFAULT)
}

/// 归一化缓动值：非法回落 [`EASING_DEFAULT`]。
pub fn normalize_easing(raw: &str) -> &'static str {
    let v = raw.trim().to_ascii_lowercase();
    EASINGS.iter().copied().find(|s| *s == v).unwrap_or(EASING_DEFAULT)
}

/// 时长钳制：0 视为未设置回落出厂值；其余钳到 [MS_MIN, MS_MAX]。
pub fn clamp_ms(v: u32) -> u32 {
    if v == 0 {
        MS_DEFAULT
    } else {
        v.clamp(MS_MIN, MS_MAX)
    }
}

/// 当前三元组命中的预设 key；未命中返回 None（设置页显示「自定义」）。
pub fn preset_key(shape: &str, ms: u32, easing: &str) -> Option<&'static str> {
    PRESETS
        .iter()
        .find(|(_, s, m, e)| *s == shape && *m == ms && *e == easing)
        .map(|(k, ..)| *k)
}

/// 按 key 取预设三元组；未知 key 返回 None。
pub fn preset(key: &str) -> Option<(&'static str, u32, &'static str)> {
    PRESETS
        .iter()
        .find(|(k, ..)| *k == key)
        .map(|(_, s, m, e)| (*s, *m, *e))
}

/// 「减少动效」最终生效值：仅当用户开启「跟随系统」且系统要求减少动效时为
/// true。用户关闭跟随（false）→ 始终播放。拆成纯函数便于单测。
pub fn effective_reduced(follow_system: bool, system_reduced: bool) -> bool {
    follow_system && system_reduced
}

/// 读 Windows「关闭客户区动画」辅助功能（设置 → 辅助功能 → 视觉效果 → 动画
/// 效果）。返回 true = 系统要求减少动效。查询失败按 false 处理（宁多勿少）。
#[cfg(windows)]
pub fn system_reduced_motion() -> bool {
    // SPI_GETCLIENTAREAANIMATION：pvParam 收 BOOL，0 = 客户区动画已关闭。
    const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;

    // app.rs 的 center_window_px 里已有同名 extern 声明（pvparam 收 *mut Rect）。
    // 两处按符号链接到同一个 user32 导出，指针宽度一致、ABI 兼容，这里收 BOOL
    // 指针是正确用法；lint 只是保守提示，零侵入压制而非改既有签名。
    #[allow(clashing_extern_declarations)]
    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(action: u32, uiparam: u32, pvparam: *mut i32, winini: u32) -> i32;
    }

    let mut enabled: i32 = 1;
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut enabled, 0) };
    ok != 0 && enabled == 0
}

#[cfg(not(windows))]
pub fn system_reduced_motion() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_wordlist_normalizes_and_falls_back() {
        assert_eq!(normalize_shape("push"), "push");
        assert_eq!(normalize_shape("  JELLY "), "jelly");
        assert_eq!(normalize_shape("flip"), "push"); // 翻页已裁撤 → 回落默认
        assert_eq!(normalize_shape(""), "push");
        assert_eq!(SHAPES.len(), 6);
    }

    #[test]
    fn easing_wordlist_normalizes_and_falls_back() {
        assert_eq!(normalize_easing("standard"), "standard");
        assert_eq!(normalize_easing(" Spring"), "spring");
        assert_eq!(normalize_easing("bounce"), "smooth");
        assert_eq!(EASINGS.len(), 3);
    }

    #[test]
    fn ms_clamps_to_slider_range() {
        assert_eq!(clamp_ms(0), MS_DEFAULT);
        assert_eq!(clamp_ms(79), MS_MIN);
        assert_eq!(clamp_ms(100), 100);
        assert_eq!(clamp_ms(240), 240);
        assert_eq!(clamp_ms(601), MS_MAX);
    }

    #[test]
    fn presets_match_spec() {
        // 高保真 §04：跟手/顺滑/电影感/活泼 四组三元组。
        assert_eq!(preset("follow"), Some(("push", 120, "standard")));
        assert_eq!(preset("smooth"), Some(("push", 240, "smooth")));
        assert_eq!(preset("cinema"), Some(("swallow", 320, "smooth")));
        assert_eq!(preset("lively"), Some(("jelly", 380, "spring")));
        assert_eq!(preset("custom"), None);
    }

    #[test]
    fn preset_key_round_trip_and_custom() {
        assert_eq!(preset_key("push", 240, "smooth"), Some("smooth"));
        assert_eq!(preset_key("swallow", 320, "smooth"), Some("cinema"));
        assert_eq!(preset_key("push", 241, "smooth"), None); // 自定义
        assert_eq!(preset_key("accordion", 140, "spring"), None);
    }

    #[test]
    fn reduced_motion_logic() {
        // 关掉「跟随系统」→ 与系统状态无关，始终播放。
        assert!(!effective_reduced(false, true));
        assert!(!effective_reduced(false, false));
        // 跟随系统时透传系统检测值。
        assert!(effective_reduced(true, true));
        assert!(!effective_reduced(true, false));
    }
}
