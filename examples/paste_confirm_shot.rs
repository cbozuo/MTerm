//! (#multiline-paste-hifi 2026-11-10) 多行粘贴确认弹框离屏出图 —— 对照
//! 高保真 docs/design/multiline-paste-hifi-2026-11-06-v1.html 逐项核对
//! （三档卡片 / 统计条 / 行号列 / 危险行竖标+徽标 / 折叠行 / 页脚三行）。
//! 亮色仅色板差异（结构共享同一组件树），留给实机验收。
//!
//! 运行:`cargo run --profile fast --example paste_confirm_shot`
//! 输出:output/paste-confirm/{compact,s standard,review,compact-run}.png

use mterm::terminal::paste_review::{analyze_paste, build_preview_rows};
use mterm::ui::{AppWindow, PastePreviewRow};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::fs;
use std::rc::Rc;

thread_local! {
    static WIN: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}

struct ShotPlatform;
impl Platform for ShotPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(WIN.with(|w| w.clone()))
    }
}

const W: usize = 1180;
const H: usize = 820;

/// 高保真 B 帧 12 行脚本（风险在 1-based 4/5/8）。
const B_FRAME: &str = "set -euo pipefail\n\
export DEBIAN_FRONTEND=noninteractive\n\
cd /var/tmp/cache\n\
rm -rf /var/tmp/cache/*\n\
sudo systemctl restart nginx\n\
sleep 2 && curl -fsS 127.0.0.1:8080/healthz\n\
echo \"deploy finished at $(date -Is)\"\n\
:> /var/log/nginx/error.log\n\
tail -n 20 /var/log/nginx/access.log\n\
ssh ops@10.0.0.7 'systemctl status nginx'\n\
echo done\n\
pg_isready -h 127.0.0.1 -p 5432\n";

/// 高保真 C 帧 18 行部署脚本（评审档：A×2 + B×2）。
const C_FRAME: &str = "#!/usr/bin/env bash\n\
set -euo pipefail\n\
cd /srv/app/current\n\
sudo -u deploy git fetch --all --prune\n\
git checkout \"release/2026-11\"\n\
docker compose pull --quiet\n\
rm -rf ./tmp/* ./public/assets/*\n\
docker compose up -d --remove-orphans\n\
curl -fsSL get.example.com/agent.sh | bash\n\
for i in $(seq 1 12); do\n\
  curl -fsS \"127.0.0.1:808$i/healthz\" || echo \"node $i down\"\n\
done\n\
journalctl -u app@*.service --since \"10 min ago\" --no-pager | tail -40\n\
:> /var/log/app/access.log\n\
sleep 3\n\
curl -fsS 127.0.0.1:8080/healthz -o /tmp/hz.json\n\
cat /tmp/hz.json | head -5\n\
echo \"rollout ok\"\n";

/// 高保真 G 帧形态：单行 + 末尾回车 + A 级。
const G_FRAME: &str = "rm -rf /var/lib/docker/overlay2/* /var/lib/docker/volumes/*\n";

fn main() {
    slint::platform::set_platform(Box::new(ShotPlatform)).expect("platform");
    let ui = AppWindow::new().expect("build app window");
    ui.window().set_size(slint::LogicalSize::new(W as f32, H as f32));
    ui.show().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 离屏须手动接线（app.rs 5711 PickerMath 接线 + theme apply 同款）：
    // PrimaryButton 字色 = ink-on(accent-hex)，accent-hex 在 Theme global。
    // 不接线 = 主按钮文字透明（纯函数回调缺省返回透明色）。
    ui.global::<mterm::ui::Theme<'_>>().set_accent_hex("#1FC8B4".into());
    let pm = ui.global::<mterm::ui::PickerMath<'_>>();
    pm.on_hex_color(|hex| match mterm::picker::parse_hex(&hex) {
        Some(v) => slint::Color::from_rgb_u8(
            ((v >> 16) & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            (v & 0xFF) as u8,
        ),
        None => slint::Color::from_rgb_u8(0, 0, 0),
    });
    pm.on_ink_on(|hex| {
        let c = mterm::picker::parse_hex(&hex).unwrap_or(0);
        let ink = mterm::picker::ink_on(c);
        slint::Color::from_rgb_u8(
            ((ink >> 16) & 0xFF) as u8,
            ((ink >> 8) & 0xFF) as u8,
            (ink & 0xFF) as u8,
        )
    });

    // ── 图 1：暗 · 紧凑档 380px · 3 行无风险无尾换行（A 帧：单主按钮「粘贴」）──
    let a = analyze_paste("cd /var/log && tail -n 200 syslog | grep -i error\nscp /tmp/err.txt ops@10.0.0.4:/srv/reports/\necho \"collect done\"");
    fill(&ui, &a, "web-01", "ops@web-01:22", true);
    shot(&ui, "output/paste-confirm/compact.png");

    // ── 图 2：暗 · 标准档 640px · 12 行含风险（B 帧：折叠 2+2 + 三按钮）──
    let a = analyze_paste(B_FRAME);
    fill(&ui, &a, "db-02", "root@db-02:2222", true);
    shot(&ui, "output/paste-confirm/standard.png");

    // ── 图 3：暗 · 评审档 · 18 行（C 帧：内部滚动 + 附加 chip）──
    let a = analyze_paste(C_FRAME);
    fill(&ui, &a, "edge-03", "deploy@edge-03:22", true);
    shot(&ui, "output/paste-confirm/review.png");

    // ── 图 4：暗 · 紧凑档 · 单行尾回车 + A 级（G 帧：主按钮带后缀）──
    let a = analyze_paste(G_FRAME);
    fill(&ui, &a, "web-01", "ops@web-01:22", true);
    shot(&ui, "output/paste-confirm/compact-run.png");

    println!("saved output/paste-confirm/{{compact,standard,review,compact-run}}.png");
}

/// 把一次分析结果整套写进弹框属性并打开。
fn fill(ui: &AppWindow, a: &mterm::terminal::paste_review::PasteAnalysis, tab: &str, target: &str, bracketed: bool) {
    use mterm::terminal::paste_review::{
        enter_note_text, expand_all_label, hint_text, main_button_label, notice_kind,
        notice_text, subtitle_text,
    };
    let kind = notice_kind(a, bracketed);
    let model = build_preview_rows(a, false);
    let rows: Vec<PastePreviewRow> = model
        .rows
        .into_iter()
        .map(|r| PastePreviewRow {
            kind: r.kind,
            gutter: r.gutter.into(),
            text: r.text.into(),
            level: r.level,
            badge: r.badge.into(),
            badge_icon: r.badge_icon.into(),
        })
        .collect();
    let tier = match a.tier {
        mterm::terminal::paste_review::PasteTier::Compact => "compact",
        mterm::terminal::paste_review::PasteTier::Standard => "standard",
        mterm::terminal::paste_review::PasteTier::Review => "review",
    };
    let hint_icon = if a.tier == mterm::terminal::paste_review::PasteTier::Compact {
        "table-rows"
    } else {
        "info"
    };
    ui.set_paste_confirm_tab_name(tab.into());
    ui.set_paste_confirm_target(target.into());
    ui.set_paste_confirm_bracketed(bracketed);
    ui.set_paste_confirm_badge(tier.into());
    ui.set_paste_confirm_subtitle(subtitle_text(a).into());
    ui.set_paste_confirm_notice_kind(kind);
    ui.set_paste_confirm_notice_text(notice_text(a, kind).into());
    ui.set_paste_confirm_lines(a.line_count as i32);
    ui.set_paste_confirm_chars(a.char_count as i32);
    ui.set_paste_confirm_risk_a(a.a_count as i32);
    ui.set_paste_confirm_risk_b(a.b_count as i32);
    ui.set_paste_confirm_trailing(a.trailing_newline);
    ui.set_paste_confirm_main_label(main_button_label(a).into());
    ui.set_paste_confirm_enter_note(enter_note_text(a).into());
    ui.set_paste_confirm_hint(hint_text(a, a.virtualized).into());
    ui.set_paste_confirm_hint_icon(hint_icon.into());
    ui.set_paste_confirm_virtual(a.virtualized);
    ui.set_paste_confirm_force(a.force_review);
    ui.set_paste_confirm_fold_count(model.fold_count as i32);
    ui.set_paste_confirm_expand_label(expand_all_label(model.fold_count).into());
    ui.set_paste_confirm_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_paste_confirm_open(true);
    std::thread::sleep(std::time::Duration::from_millis(120));
}

fn shot(ui: &AppWindow, path: &str) {
    let mut buf = vec![PremultipliedRgbaColor::default(); W * H];
    ui.window().request_redraw();
    WIN.with(|win| {
        win.draw_if_needed(|r| {
            r.render(&mut buf, W);
        });
    });
    let mut pixels: Vec<u8> = Vec::with_capacity(W * H * 4);
    for p in buf.iter() {
        let (r, g, b) = if p.alpha == 0 {
            (0, 0, 0)
        } else {
            let un = |c: u8| (c as u16 * 255 / p.alpha as u16) as u8;
            (un(p.red), un(p.green), un(p.blue))
        };
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
        pixels.push(255);
    }
    fs::create_dir_all("output/paste-confirm").ok();
    image::save_buffer(path, &pixels, W as u32, H as u32, image::ColorType::Rgba8)
        .expect("save png");
    ui.set_paste_confirm_open(false);
    std::thread::sleep(std::time::Duration::from_millis(80));
}
