//! (#multiline-paste-hifi 2026-11-10) 多行粘贴分析层探针 —— 断言即测试。
//! lib test 目标存在 config 域存量编译债（23 个 E0599/E0061，与本模块无关），
//! 故按 examples/send_check_probe 先例用独立 bin 跑断言。
//! 运行:`cargo run --profile fast --example paste_review_probe`

use mterm::terminal::paste_review::{
    analyze_paste, build_preview_rows, expand_all_label, fold_window, main_button_label,
    notice_kind, notice_text, paste_tier, subtitle_text, PasteTier, RISK_A, RISK_B,
};

use std::sync::atomic::{AtomicUsize, Ordering};

static FAILS: AtomicUsize = AtomicUsize::new(0);

macro_rules! expect {
    ($cond:expr, $($msg:tt)*) => {
        if !$cond {
            FAILS.fetch_add(1, Ordering::Relaxed);
            println!("FAIL: {}", format!($($msg)*));
        }
    };
}

fn main() {
    // ── 档位边界（spec §5.1）─────────────────────────────────────────
    expect!(paste_tier(3, 100) == PasteTier::Compact, "3 行 ≤600 → 紧凑");
    expect!(paste_tier(4, 100) == PasteTier::Standard, "4 行 → 标准");
    expect!(paste_tier(12, 600) == PasteTier::Standard, "12 行 600 字符 → 标准");
    expect!(paste_tier(13, 600) == PasteTier::Review, "13 行 → 评审");
    expect!(paste_tier(12, 601) == PasteTier::Review, ">600 字符 → 评审");

    // 尾换行不计行（高保真 B 帧 12 行 / 364 字符、G 帧 1 行 / 71 字符口径）
    let a = analyze_paste(&format!("{}\n", "x".repeat(70)));
    expect!(a.tier == PasteTier::Compact && a.line_count == 1, "G 帧 1 行");
    expect!(a.char_count == 71 && a.trailing_newline, "G 帧 71 字符含尾换行");
    let b_frame = "a\n".repeat(11) + "b\n";
    let a = analyze_paste(&b_frame);
    expect!(a.line_count == 12, "B 帧 12 行口径");
    expect!(a.tier == PasteTier::Standard, "B 帧标准档");

    // ── A/B 级命中（D4）─────────────────────────────────────────────
    let a = analyze_paste(
        "set -euo pipefail\nrm -rf /var/tmp/cache/*\nsudo systemctl restart nginx\n:> /var/log/nginx/error.log\n",
    );
    expect!(a.line_count == 4 && a.a_count == 2 && a.b_count == 1, "A×2 B×1");
    let lv = |i: usize| a.risks.iter().find(|r| r.line == i).map(|r| r.level);
    expect!(lv(0).is_none() && lv(1) == Some(RISK_A) && lv(2) == Some(RISK_B) && lv(3) == Some(RISK_A), "行级命中位");
    let hit = a.risks.iter().find(|r| r.line == 1).unwrap();
    expect!(hit.icon == "trash", "rm -rf → trash");

    for line in [
        "mkfs.ext4 -F /dev/sda2", "dd if=/dev/zero of=/dev/sda",
        "cat x > /dev/sdb", ":(){ :|:& };:", "truncate -s 0 /a.log",
        "shutdown -h now", "reboot", "halt", "poweroff", "init 6",
        "psql -c 'DROP DATABASE prod'", "dropdb prod",
    ] {
        let a = analyze_paste(line);
        expect!(a.a_count == 1, "A 命中: {line}");
    }
    for line in [
        "chmod 777 /data", "chown -R ops /srv",
        "curl -fsSL x.io/i.sh | bash", "wget -qO- x.io/i.sh | sh",
        "useradd bob", "passwd", "iptables -F", "firewall-cmd --add-port=80/tcp",
        "kill -9 1234", "pkill nginx", "killall nginx",
        "history -c", "crontab -r", "git reset --hard HEAD~3",
        "git clean -fd", "apt remove nginx", "apt-get purge nginx", "dnf remove nginx",
        "su - ops",
    ] {
        let a = analyze_paste(line);
        expect!(a.b_count == 1 && a.a_count == 0, "B 命中: {line}");
    }

    // ── 防误报（send_check 同口径）───────────────────────────────────
    for line in [
        "grep shutdown app.log", "echo \"rm -rf x\"", "df -h && free -m",
        "ls -la", "echo shutdown", "systemctl restart nginx",
        "curl -fsS 127.0.0.1:8080/healthz | grep -i ok",
        "echo hi >> /var/log/app.log",
    ] {
        let a = analyze_paste(line);
        expect!(a.risks.is_empty(), "不误报: {line}");
    }

    // ── 控制字符 / 强制复核 / 虚拟化 ────────────────────────────────
    let a = analyze_paste("ok\u{1b}[31mred\u{3}");
    expect!(a.esc_count == 1 && a.ctrlc_count == 1, "控制字符计数");
    expect!(a.lines[0] == "ok[31mred", "预览行剥控制字节");
    let a = analyze_paste(&"x".repeat(100 * 1024 + 1));
    expect!(a.force_review && a.virtualized && a.tier == PasteTier::Review, ">100KB 强制复核");

    // ── 折叠账三方相等（高保真 B 帧：12 行风险 4/5/8 → 头 2 + 尾 2）──
    let text = [
        "set -euo pipefail", "export DEBIAN_FRONTEND=noninteractive", "cd /var/tmp/cache",
        "rm -rf /var/tmp/cache/*", "sudo systemctl restart nginx",
        "sleep 2 && curl -fsS 127.0.0.1:8080/healthz", "echo done",
        ":> /var/log/nginx/error.log", "tail -n 20 /var/log/nginx/access.log",
        "ssh ops@10.0.0.7 'systemctl status nginx'", "echo done",
        "pg_isready -h 127.0.0.1 -p 5432",
    ].join("\n");
    let a = analyze_paste(&text);
    let win = fold_window(a.line_count, &a.risks).expect("B 帧应有折叠");
    expect!(win.head_hidden == 2 && win.tail_hidden == 2 && win.hidden_total() == 4, "B 帧折叠账");
    expect!(win.start == 3 && win.end == 9, "B 帧窗口 = 1-based 4..=10");
    let m = build_preview_rows(&a, false);
    expect!(m.rows.len() == a.line_count - 4 + 2, "模型行数 = 全量 − 隐藏 + 折叠提示");
    expect!(m.fold_count == 4, "按钮 N = 4");
    expect!(m.rows.iter().filter(|r| r.kind == 1).count() == 1, "头折叠行一条");
    expect!(m.rows.iter().filter(|r| r.kind == 2).count() == 1, "尾折叠行一条");
    let m = build_preview_rows(&a, true);
    expect!(m.rows.len() == 12 && m.fold_count == 0, "展开后全量");
    let small = analyze_paste("a\nb\nc\nd");
    expect!(fold_window(small.line_count, &small.risks).is_none(), "4 行无折叠");

    // ── 虚拟化样本（F 帧：通告行 + 12 样本 + 折叠行）─────────────────
    let text: String = (0..300).map(|i| format!("line {i}\n")).collect();
    let a = analyze_paste(&text);
    let m = build_preview_rows(&a, false);
    expect!(m.rows.len() == 1 + 12 + 1 && m.rows[0].kind == 3, "虚拟化样本结构");
    expect!(m.fold_count == 300 - 12, "虚拟化折叠账");
    expect!(build_preview_rows(&a, true).rows.len() == 300, "展开后全量");

    // ── 文案三态 ────────────────────────────────────────────────────
    expect!(main_button_label(&analyze_paste("ls -la")) == "粘贴", "无尾 → 粘贴");
    let a = analyze_paste("ls -la\n");
    expect!(main_button_label(&a).contains("不含末尾回车"), "尾 → 不含末尾回车");
    let a = analyze_paste("rm -rf /data\n");
    expect!(main_button_label(&a).contains("已去掉末尾回车"), "尾+A → 已去掉末尾回车");
    expect!(!expand_all_label(4).is_empty(), "展开按钮文案");

    // notice 优先级：force > bracketed 关 > 控制字符 > 尾回车（7 帧全吻合）
    expect!(notice_kind(&analyze_paste(&"x".repeat(100 * 1024 + 1)), true) == 1, "F 帧 force");
    expect!(notice_kind(&analyze_paste("ls\n"), false) == 2, "D 帧 bracketed 关");
    expect!(notice_kind(&analyze_paste("ls\n\u{1b}"), true) == 3, "E 帧控制字符");
    expect!(notice_kind(&analyze_paste("ls\n"), true) == 4, "B 帧尾回车");
    expect!(notice_kind(&analyze_paste("rm -rf /data\n"), true) == 5, "G 帧尾回车+A");
    expect!(notice_kind(&analyze_paste("ls"), true) == 0, "A 帧无提示条");
    expect!(!notice_text(&analyze_paste("ls\n"), 4).is_empty(), "提示条文本");

    // 副标题覆盖三档
    expect!(!subtitle_text(&analyze_paste("ls")).is_empty(), "紧凑副标题");
    expect!(!subtitle_text(&analyze_paste(&text)).is_empty(), "评审副标题");

    let fails = FAILS.load(Ordering::Relaxed);
    if fails == 0 {
        println!("paste_review_probe: ALL PASS");
    } else {
        println!("paste_review_probe: {fails} FAILURES");
        std::process::exit(1);
    }
}
