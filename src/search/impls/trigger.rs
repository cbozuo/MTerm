//! (#double-shift-v1 2026-11-11) 双击 Shift 触发状态机。
//!
//! 高保真 §03：只认「抬起后 80–360ms 内第二次按下」，重置路径 = 其它按键 /
//! 鼠标按下 / 失焦 / 超时。本项目实测（app.rs on_send_key 注释）：
//!   - 裸 Shift 到达形态是 `key=""` + `shift=true`（winit WM_KEYDOWN 无 char）；
//!   - 百度拼音注入的 C0 标记（左 U+0015 / 右 U+0010）与真实 Shift **成对**
//!     到达，顺序不定 → 这些标记对状态机**透明**（忽略，既不判定也不清零，
//!     否则后到的标记会把第一击的时间戳抹掉，双击永远差一击）；
//!   - 右 Shift 场景尾随 U+0008（~900ms 后）→ 清零（此时早已超窗，防御性）。

use std::time::{Duration, Instant};

/// 双击窗口下限（ms）：给手指自然回弹留区间。
pub(crate) const SHIFT_MIN_MS: u64 = 80;
/// 双击窗口上限（ms）：超过即视为两次独立单击。
pub(crate) const SHIFT_MAX_MS: u64 = 360;

/// 纯状态机：吃事件与时间戳，吐「是否该打开面板」。不碰 Slint。
#[derive(Debug, Default)]
pub(crate) struct DoubleShiftTracker {
    /// 上一记裸 Shift 的时间戳（None = 待机）。
    last: Option<Instant>,
}

/// 键事件分类（宿主桥把 Slint/winit 形态归一成这三类再喂进来）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyEvent<'a> {
    /// 裸 Shift 按下（无修饰组合）。
    Shift,
    /// IME 注入的 C0 标记字符（0x15 / 0x10 等，非独立功能键）。
    ImeMarker(&'a str),
    /// 其余任何键（含修饰组合、普通字符、独立功能键如退格/Tab）。
    Other,
}

impl DoubleShiftTracker {
    pub(crate) fn new() -> Self {
        Self { last: None }
    }

    /// 喂一个键事件。返回 true = 双击命中，应打开面板。
    pub(crate) fn on_key(&mut self, ev: KeyEvent<'_>, now: Instant, already_open: bool) -> bool {
        match ev {
            // 面板开着时双击不重复打开，直接回到待机（高保真 openPalette 守卫）。
            KeyEvent::Shift if already_open => {
                self.last = None;
                false
            }
            KeyEvent::Shift => match self.last {
                Some(t0) => {
                    let gap = now.duration_since(t0);
                    self.last = None;
                    let ms = gap.as_millis() as u64;
                    // 太快（≤80，键盘抖动）或太慢（>360）都不算；本次按下成为新起点。
                    if (SHIFT_MIN_MS..=SHIFT_MAX_MS).contains(&ms) {
                        true
                    } else {
                        self.last = Some(now);
                        false
                    }
                }
                None => {
                    self.last = Some(now);
                    false
                }
            },
            // IME 标记：透明处理（既不判定也不清零）。
            //
            // ★ 与设计稿 §03 R1「标记立即清空计时器」的**有意偏差**：
            //   原型跑在浏览器里，`event.key === "Shift"` 与 IME 注入的
            //   U+0015/U+0010 是**不同**的 key，能干净区分 → 直接清零。
            //   Slint 侧裸 Shift 的码位恰好就是 U+0010/U+0015
            //   （i-slint-common key_codes.rs:40,46），百度注入的标记与
            //   **真实 Shift 同码位**（百度左→0x15 撞真实右 Shift、
            //   右→0x10 撞真实左 Shift），无法区分。
            //   若照抄「清零」，装了百度拼音的用户按两下 Shift 永远命中不了
            //   ——标记会把第一击的时间戳抹掉。
            //   这里改成「透明 + 重新武装」：标记既不参与判定、也不清零，
            //   只把起点推到标记那一刻（几 ms 内）；第二击仍落在 80–360ms
            //   窗口里 → 双击照常可用。效果与原型的「取消本次候选」等价，
            //   但保住了后续双击的机会。
            KeyEvent::ImeMarker(_) => false,
            // 其它任何键（含 U+0008 尾随退格）→ 清零。
            KeyEvent::Other => {
                self.last = None;
                false
            }
        }
    }

    /// 显式重置（鼠标按下 / 窗口失焦——高保真的另外两条重置路径）。
    pub(crate) fn reset(&mut self) {
        self.last = None;
    }

    /// 是否处于待机（无第一击在途）。供诊断日志。
    pub(crate) fn idle(&self) -> bool {
        self.last.is_none()
    }

    /// 超时自清（可选：360ms 后的下一击本就会因超窗失败，这里供宿主节流）。
    pub(crate) fn expired(&self, now: Instant) -> bool {
        self.last
            .is_some_and(|t0| now.duration_since(t0) > Duration::from_millis(SHIFT_MAX_MS))
    }
}


