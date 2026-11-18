// 液态玻璃实验（#glass-lab 2026-11-18）：backdrop-blur 在 femtovg 渲染器上的
// 视觉/性能验证。真窗口（winit+femtovg），8 秒后自动退出；期间外部用
// PrintWindow(PW_RENDERFULLCONTENT) 抓帧。
//
// 跑法：cargo run --release --example glass_shot
// （release 必须——debug 的 GL 驱动慢得没法看性能）

use slint::ComponentHandle;
use slint::Timer;
use slint::TimerMode;

slint::slint! {
    export component GlassLab inherits Window {
        title: "Glass Lab (femtovg)";
        width: 900px;
        height: 620px;
        background: rgba(16, 20, 24, 255);

        property <int> tick: 0;
        callback tick-up();
        pure callback line-text(int) -> string;
        pure callback line-color(int) -> color;

        tick-up() => { self.tick += 1; }
        line-text(row) => {
            if (Math.mod(row + self.tick, 7) == 0) {
                return "INFO  Application - tick " + self.tick + " row " + row;
            } else if (Math.mod(row + self.tick, 5) == 0) {
                return "WARN  Pool - connection usage 8%";
            } else if (Math.mod(row + self.tick, 3) == 0) {
                return "ERROR Sync - upload failed: self cert passed";
            }
            return "2026-11-18 14:00:00." + row + " [main] sample log line " + row;
        }
        line-color(row) => {
            if (Math.mod(row + self.tick, 7) == 0) { return rgba(126, 231, 135, 255); }
            if (Math.mod(row + self.tick, 5) == 0) { return rgba(210, 153, 34, 255); }
            if (Math.mod(row + self.tick, 3) == 0) { return rgba(255, 123, 114, 255); }
            return rgba(139, 148, 158, 255);
        }

        VerticalLayout {
            padding: 8px;
            for row in 26 : Text {
                text: root.line-text(row);
                color: root.line-color(row);
                font-family: "Consolas";
                font-size: 13px;
            }
        }

        Rectangle {
            x: 80px; y: 120px;
            width: 420px; height: 240px;
            border-radius: 16px;
            backdrop-blur: 8px;
            background: rgba(255, 255, 255, 20);
            border-width: 1px;
            border-color: rgba(255, 255, 255, 46);
            VerticalLayout {
                padding: 16px;
                spacing: 6px;
                Text { text: "实验组：backdrop-blur 8px"; color: white; font-size: 14px; }
                Text { text: "压在滚动终端画布上的液态玻璃"; color: rgba(255, 255, 255, 204); font-size: 12px; }
                Text { text: "每帧文本都在变化 → 每次重绘都要重新模糊"; color: rgba(255, 255, 255, 153); font-size: 12px; }
            }
        }

        Rectangle {
            x: 540px; y: 120px;
            width: 300px; height: 240px;
            border-radius: 16px;
            background: rgba(255, 255, 255, 20);
            border-width: 1px;
            border-color: rgba(255, 255, 255, 46);
            VerticalLayout {
                padding: 16px;
                Text { text: "对照组：无 blur"; color: white; font-size: 14px; }
                Text { text: "同底色同圆角，无 backdrop-blur"; color: rgba(255, 255, 255, 204); font-size: 12px; }
            }
        }

        Rectangle {
            x: 80px; y: 400px;
            width: 420px; height: 140px;
            border-radius: 16px;
            backdrop-blur: 20px;
            background: rgba(255, 255, 255, 10);
            border-width: 1px;
            border-color: rgba(255, 255, 255, 46);
            VerticalLayout {
                padding: 16px;
                Text { text: "强 blur：20px"; color: white; font-size: 14px; }
                Text { text: "femtovg canvas.blur_path(sigma=20*dpi)"; color: rgba(255, 255, 255, 204); font-size: 12px; }
            }
        }

        Text {
            x: 540px; y: 400px;
            text: "tick 计数见 stdout（每 0.5s 全画布重绘一次）";
            color: rgba(255, 255, 255, 102);
            font-size: 12px;
        }
    }
}

fn main() {
    std::env::set_var("SLINT_BACKEND", "winit-femtovg");
    let lab = GlassLab::new().expect("build glass lab");

    let timer = Timer::default();
    let weak = lab.as_weak();
    timer.start(TimerMode::Repeated, std::time::Duration::from_millis(500), move || {
        if let Some(lab) = weak.upgrade() {
            lab.invoke_tick_up();
        }
    });

    let quit = Timer::default();
    quit.start(TimerMode::SingleShot, std::time::Duration::from_secs(8), move || {
        println!("8s elapsed, quitting");
        slint::quit_event_loop().ok();
    });

    lab.run().expect("run");
    drop(timer);
}
