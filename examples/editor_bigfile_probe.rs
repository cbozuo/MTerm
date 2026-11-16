//! (#editor-native) 探查:编辑器正文体积 → Slint 排版成本。
//!
//! 用途:定「内置编辑器能开多大」的门限。原生架构下正文整篇交给 TextInput,
//! `preferred-height` 要求全文行盒(no-wrap 下行数 × 行高),所以要量一下
//! 排版时间随体积的曲线,别把门限拍脑袋定在 16MB 或 100MB。
//!
//! 运行:`cargo run --profile fast --example editor_bigfile_probe`
//! 输出:每个体积一行 `size=..MB lines=.. layout=..ms`

use mterm::ui::{EditorTabInfo, EditorWindow, Theme};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;
use std::time::Instant;

thread_local! {
    static WIN: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}

struct ProbePlatform;
impl Platform for ProbePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(WIN.with(|w| w.clone()))
    }
}

const W: usize = 1200;
const H: usize = 760;

/// 造一份近似真实代码的文本:每行 ~64 字节,带缩进与中文注释。
fn make_text(bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 128);
    let mut i = 0usize;
    while s.len() < bytes {
        s.push_str(&format!(
            "    fn handler_{i}(pool: &SshSessionPool) -> Result<(), IoError> {{ // 第 {i} 行\n"
        ));
        i += 1;
    }
    s
}

fn main() {
    slint::platform::set_platform(Box::new(ProbePlatform)).unwrap();
    let ui = EditorWindow::new().unwrap();
    ui.set_custom_titlebar(true);
    ui.set_dark_mode(true);
    {
        let th = ui.global::<Theme>();
        th.set_ui_scale(1.0);
        th.set_font_mono("Consolas".into());
        th.set_ui_font_family("Segoe UI".into());
    }
    ui.set_editor_tabs(ModelRc::new(VecModel::from(vec![EditorTabInfo {
        id: "probe".into(),
        name: "probe.sh".into(),
        dirty: false,
        active: true,
        lang: "Shell".into(),
        letter: "P".into(),
        icon_color: slint::Color::default(),
    }])));
    ui.set_editor_readonly(true);
    ui.show().unwrap();

    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    WIN.with(|w| w.set_size(slint::PhysicalSize::new(W as u32, H as u32)));

    println!("\n  size(MB)   lines     cold(ms)   per-key(ms)");
    for mb in [0.016f64, 0.032, 0.064, 0.128, 0.25, 0.5, 1.0, 2.0] {
        let bytes = (mb * 1024.0 * 1024.0) as usize;
        let text = make_text(bytes);
        let lines = text.lines().count();
        // 换一次正文 → 标记脏 → 下一次 draw 必然重排
        ui.set_editor_content(text.as_str().into());
        ui.set_editor_syntax_source(text.as_str().into());
        ui.set_editor_syntax_spans("".into());

        // 冷排(首次:包含窗口尺寸变化触发的整树布局)
        let t0 = Instant::now();
        WIN.with(|w| {
            w.draw_if_needed(|r| { r.render(&mut buf, W); });
        });
        let cold = t0.elapsed().as_secs_f64() * 1000.0;

        // 击键代价:改一个字 → 整篇重排(这是可编辑上限的决定因素)
        let mut edited = text.clone();
        edited.push('x');
        ui.set_editor_content(edited.as_str().into());
        let t1 = Instant::now();
        WIN.with(|w| {
            w.draw_if_needed(|r| { r.render(&mut buf, W); });
        });
        let key = t1.elapsed().as_secs_f64() * 1000.0;

        println!("  {mb:8.3}  {lines:8}   cold {cold:9.1}   per-key {key:8.1}");
    }
    println!("\n注:per-key = 改一个字符后重排 + 绘制一帧的耗时(含代码区 + 行号槽两个 TextInput)。\n");
}
