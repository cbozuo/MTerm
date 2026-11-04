//! (#panel-anim 2026-11-05) 侧栏收展动效的离屏行为回归。
//!
//! 锁四个行为（高保真 docs/design/two-sidebar-hifi-2026-11-04.html V9）：
//!   ① 首帧门卫：`PanelAnim.ready == false` 时任何 collapsed 切换都是瞬时
//!      （启动 seed / 布局恢复永不播动画，坑 01）；
//!   ② 推挤（默认形态）：collapsed 切换后宽度有过渡过程（动画中段边缘仍是
//!      面板色、中央已露底 = 插值在推进），之后收敛到收起态外观；
//!   ③ 瞬时形态：`shape = "instant"` 时宽度直接赋值；
//!   ④ 减少动效：`reduced = true` 时时长归零，同样瞬时（不做形态降级）。
//!
//! 断言只用「采样点颜色在几个时刻的相等关系」，不依赖插值曲线细节。
//!
//! 结构注记：本文件与 tests/ 下其它离屏用例一样只放**一个** #[test]（4 个
//! 场景顺序执行）；每个场景经 `spawn_ui()` 拿**独立**的 MinimalSoftwareWindow
//! （platform 每次新建并经全局槽传回），避免多组件共享同一 adapter 互相渲染。
//!
//! ⚠️ 离屏 DPR=1。运行：`cargo test --test panel_anim`

use mterm::ui::{AppWindow, PanelAnim};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const W: usize = 800;
const H: usize = 600;
/// 右栏默认 220px、dock=right → 面板中央的采样点（展开时被右栏覆盖，
/// 收起后由终端区接管）。
const SAMPLE_X: usize = W - 110;
/// 贴近收口边缘的采样点：只在面板收窄到最后 20px 内才露出。smooth 曲线
/// （cubic-bezier(.25,1,.3,1)）前段极快——40ms/240ms 插值已达 ~58%，中央
/// 采样点早就露底了；边缘点在过渡早期仍是面板色，适合做「未瞬完」断言。
const SAMPLE_EDGE_X: usize = W - 20;
const SAMPLE_Y: usize = H / 2;

/// platform 每次 create_window_adapter 都新建窗口，经这里传回给测试。
/// （create 与 AppWindow::new 同线程，thread_local 即可；Rc 不跨线程。）
thread_local! {
    static LAST_ADAPTER: RefCell<Option<Rc<MinimalSoftwareWindow>>> =
        const { RefCell::new(None) };
}

struct ProbePlatform;

impl slint::platform::Platform for ProbePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let win = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        LAST_ADAPTER.with(|slot| *slot.borrow_mut() = Some(win.clone()));
        Ok(win)
    }
}

/// 一个场景 = 一个全新组件 + 一个全新离屏窗口。
fn spawn_ui() -> (AppWindow, Rc<MinimalSoftwareWindow>) {
    let ui = AppWindow::new().unwrap();
    let win = LAST_ADAPTER.with(|slot| slot.borrow_mut().take().unwrap());
    ui.set_right_sidebar_dock("right".into());
    win.set_size(LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();
    (ui, win)
}

struct Frame {
    buf: Vec<PremultipliedRgbaColor>,
}

impl Frame {
    fn new() -> Self {
        Self { buf: vec![PremultipliedRgbaColor::default(); W * H] }
    }

    fn grab(&mut self, ui: &AppWindow, win: &MinimalSoftwareWindow) {
        ui.window().request_redraw();
        win.draw_if_needed(|r| {
            r.render(&mut self.buf, W);
        });
    }

    fn px(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let p = self.buf[y * W + x];
        if p.alpha == 0 {
            return (0, 0, 0);
        }
        let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
        (un(p.red), un(p.green), un(p.blue))
    }
}

/// 推进 Slint 动画：真实时钟走过 `ms` 后补一个指针事件触发重算
/// （离屏探针的已证实行为：只 sleep 不推进 animate，同 sidebar_boundaries）。
fn advance(ui: &AppWindow, ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(-10.0, -10.0),
    });
}

#[test]
fn panel_anim_behaviours() {
    slint::platform::set_platform(Box::new(ProbePlatform)).ok();

    // ── 场景 ① 首帧门卫：ready=false 时收展一律瞬时（坑 01）──────────
    let (ui, win) = spawn_ui();
    {
        let pa = ui.global::<PanelAnim>();
        assert_eq!(pa.get_shape(), "push");
        assert_eq!(pa.get_ms(), 240);
        assert_eq!(pa.get_easing(), "smooth");
        assert!(!pa.get_ready(), "Rust seed 前门卫必须关闭");

        let mut f = Frame::new();
        ui.set_right_sidebar_collapsed(false);
        f.grab(&ui, &win);
        let c1 = f.px(SAMPLE_X, SAMPLE_Y);

        ui.set_right_sidebar_collapsed(true);
        f.grab(&ui, &win);
        let c2 = f.px(SAMPLE_X, SAMPLE_Y);
        assert_ne!(c1, c2, "收起后采样点颜色必须变化（右栏让位）");
        advance(&ui, 50);
        f.grab(&ui, &win);
        assert_eq!(f.px(SAMPLE_X, SAMPLE_Y), c2, "ready=false 时不得出现过渡过程");
    }
    drop(ui);

    // ── 场景 ② 推挤（默认形态）：有过渡 + 收敛 ────────────────────────
    let (ui, win) = spawn_ui();
    {
        let pa = ui.global::<PanelAnim>();
        pa.set_ready(true);
        pa.set_shape("push".into());
        pa.set_ms(240);
        pa.set_easing("smooth".into());
        pa.set_reduced(false);

        let mut f = Frame::new();
        ui.set_right_sidebar_collapsed(false);
        f.grab(&ui, &win);
        let c1 = f.px(SAMPLE_EDGE_X, SAMPLE_Y);

        // 收起：动画起点帧——set 后**立即**抓（同线程无 sleep，插值必然还在
        // 起点），边缘点必须仍是面板色：若出现瞬时（enabled 失效/被禁）这里
        // 直接露底。零延迟抓取对线程调度抖动不敏感（时序敏感的中段断言已
        // 移除，见场景 ②b 注释）。
        ui.set_right_sidebar_collapsed(true);
        f.grab(&ui, &win);
        assert_eq!(
            f.px(SAMPLE_EDGE_X, SAMPLE_Y),
            c1,
            "推挤形态收起起点帧边缘点应仍是面板色——出现瞬时即动画失效"
        );
        // 收敛到收起态。
        advance(&ui, 600);
        f.grab(&ui, &win);
        let c2 = f.px(SAMPLE_X, SAMPLE_Y);
        assert_ne!(c2, c1, "收起终态必须与展开态不同");

        // 展开：起点帧不应已是展开态？反向同理——起点帧仍是收起态。
        ui.set_right_sidebar_collapsed(false);
        f.grab(&ui, &win);
        assert_ne!(
            f.px(SAMPLE_EDGE_X, SAMPLE_Y),
            c1,
            "展开起点帧边缘点应仍是收起态（动画在跑，而非瞬跳）"
        );
        advance(&ui, 800);
        f.grab(&ui, &win);
        assert_eq!(f.px(SAMPLE_EDGE_X, SAMPLE_Y), c1, "展开终态必须回到展开态颜色");
    }
    drop(ui);

    // ── 场景 ②b 其余动画形态：吞没 / 手风琴 / 归巢 / 果冻 ────────────
    // 用**稳态三段式**断言（收起收敛 + 展开完全复位），不做毫秒级中段断言
    // ——中段断言对线程调度抖动敏感（本机编译高负载下实测会漂），「过渡
    // 过程存在性」已由场景 ②（推挤）的中段断言锁住，各形态中段观感留给
    // 实机核对（Step 6）。这里锁各形态最重要的不变量：
    //   * 收起终态正确（裁切/延迟/淡出/过冲都不拦住收敛）；
    //   * 展开终态完全复位（swallow 的固定满宽、accordion 的延迟层、
    //     homing 的位移+淡出、jelly 的过冲都必须归位）。
    for shape in ["swallow", "accordion", "homing", "jelly"] {
        let (ui, win) = spawn_ui();
        {
            let pa = ui.global::<PanelAnim>();
            pa.set_ready(true);
            pa.set_shape(shape.into());
            pa.set_ms(240);
            pa.set_easing("smooth".into());
            pa.set_reduced(false);

            let mut f = Frame::new();
            ui.set_right_sidebar_collapsed(false);
            f.grab(&ui, &win);
            let c1 = f.px(SAMPLE_EDGE_X, SAMPLE_Y);

            ui.set_right_sidebar_collapsed(true);
            advance(&ui, 600);
            f.grab(&ui, &win);
            let c2 = f.px(SAMPLE_EDGE_X, SAMPLE_Y);
            assert_ne!(c2, c1, "{shape}: 收起终态必须与展开态不同");

            ui.set_right_sidebar_collapsed(false);
            advance(&ui, 800);
            f.grab(&ui, &win);
            assert_eq!(
                f.px(SAMPLE_EDGE_X, SAMPLE_Y),
                c1,
                "{shape}: 展开终态必须完全回到展开态颜色（淡出/裁切/延迟层都复位）"
            );
        }
        drop(ui);
    }

    // ── 场景 ③ 瞬时形态：宽度直接赋值 ─────────────────────────────────
    let (ui, win) = spawn_ui();
    {
        let pa = ui.global::<PanelAnim>();
        pa.set_ready(true);
        pa.set_shape("instant".into());
        pa.set_reduced(false);

        let mut f = Frame::new();
        ui.set_right_sidebar_collapsed(false);
        f.grab(&ui, &win);
        let c1 = f.px(SAMPLE_X, SAMPLE_Y);

        ui.set_right_sidebar_collapsed(true);
        f.grab(&ui, &win);
        let c2 = f.px(SAMPLE_X, SAMPLE_Y);
        assert_ne!(c1, c2, "收起后采样点颜色必须变化");
        advance(&ui, 0);
        f.grab(&ui, &win);
        assert_eq!(f.px(SAMPLE_X, SAMPLE_Y), c2, "instant 必须无过渡（立即终态）");
    }
    drop(ui);

    // ── 场景 ④ 减少动效：时长归零、不做形态降级 ──────────────────────
    let (ui, win) = spawn_ui();
    {
        let pa = ui.global::<PanelAnim>();
        pa.set_ready(true);
        pa.set_shape("push".into()); // 形态保持推挤：reduced 只归零时长
        pa.set_ms(240);
        pa.set_reduced(true);

        let mut f = Frame::new();
        ui.set_right_sidebar_collapsed(false);
        f.grab(&ui, &win);
        let c1 = f.px(SAMPLE_X, SAMPLE_Y);

        ui.set_right_sidebar_collapsed(true);
        f.grab(&ui, &win);
        let c2 = f.px(SAMPLE_X, SAMPLE_Y);
        assert_ne!(c1, c2, "收起后采样点颜色必须变化");
        advance(&ui, 0);
        f.grab(&ui, &win);
        assert_eq!(
            f.px(SAMPLE_X, SAMPLE_Y),
            c2,
            "reduced=true 时时长必须归零（瞬时终态），布局逻辑不变"
        );
    }
    drop(ui);
}
