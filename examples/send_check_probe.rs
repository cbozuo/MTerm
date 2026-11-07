//! (#send-cmd-v2 2026-11-07) send_check 新函数(danger_lines / insert_indent)探针。
//!
//! 为什么不是 #[cfg(test)]:这台机器 test 目标被 config.rs 的 23 处存量债
//! 卡死编译(11-06 在案);example = bin 类链接,绕开 dlltool 病 —— 照
//! probe_nerd_glyphs 先例,断言跑真函数,输出 PASS/FAIL。
//! 运行:`cargo run --example send_check_probe`

use mterm::terminal::send_check::{danger_lines, insert_indent};

fn main() {
    let mut fails = 0usize;

    // ── danger_lines:行级危险集合 ──
    let cases: Vec<(&str, Vec<usize>)> = vec![
        ("rm -rf /data", vec![0]),
        ("df -h\nrm -rf /data\necho hi", vec![1]),
        ("mkfs /dev/sda; reboot", vec![0]),           // 一行多段命中只算一行
        ("echo \"rm -rf x\"\nrm -fr /data", vec![1]), // 引号内不误报
        ("", vec![]),
        ("ls -la\ndf -h", vec![]),
        ("echo a\nshutdown -h now\necho b", vec![1]),
        ("time rm -rf /data", vec![0]),               // 前缀剥掉后命中
    ];
    for (text, want) in &cases {
        let got = danger_lines(text);
        if &got != want {
            fails += 1;
            println!("FAIL danger_lines({text:?}) = {got:?}, want {want:?}");
        }
    }

    // 与 danger_token 同口径:token 有 → 行集合非空;token 无 → 空
    for text in ["rm -rf /data", "echo ok && chmod -R 777 /", "grep shutdown app.log"] {
        let tok = mterm::terminal::send_check::danger_token(text);
        let has = !danger_lines(text).is_empty();
        if (tok != "") != has {
            fails += 1;
            println!("FAIL token/lines mismatch {text:?}: token={tok:?} lines_empty={} ", !has);
        }
    }

    // ── insert_indent:光标处精确插 4 空格 ──
    let (t, c) = insert_indent("ls\ncd tmp", 1, 3);
    if t != "ls\ncd     tmp" || c != "ls\ncd ".len() + 4 {
        fails += 1;
        println!("FAIL indent mid: {t:?} caret={c}");
    }
    let (t, c) = insert_indent("abc", 0, 0);
    if t != "    abc" || c != 4 {
        fails += 1;
        println!("FAIL indent head: {t:?} caret={c}");
    }
    let (t, _) = insert_indent("ab", 0, 99);
    if t != "ab    " {
        fails += 1;
        println!("FAIL indent col-clamp: {t:?}");
    }
    let (t, _) = insert_indent("ab\ncd", 99, 0);
    if t != "ab\n    cd" {
        fails += 1;
        println!("FAIL indent line-clamp: {t:?}");
    }
    let (t, c) = insert_indent("", 0, 0);
    if t != "    " || c != 4 {
        fails += 1;
        println!("FAIL indent empty: {t:?} caret={c}");
    }
    // 中文行:按字符列(不是字节)
    let (t, _) = insert_indent("数据abc", 0, 2);
    if t != "数据    abc" {
        fails += 1;
        println!("FAIL indent cjk: {t:?}");
    }

    if fails == 0 {
        println!("ALL PASS (danger_lines 8 cases + token 口径 3 + insert_indent 6)");
    } else {
        println!("{fails} FAILURES");
        std::process::exit(1);
    }
}
