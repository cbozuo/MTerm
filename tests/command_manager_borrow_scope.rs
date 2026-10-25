//! (#cm-borrow-scope 2026-10-26) 静态守卫：命令管理回调里 `RefMut` 不得跨到
//! `refresh()` / `commit()`。
//!
//! 起因（用户实测）：点弹窗里任意一个「全选」，程序**当场消失**、无提示、无进程。
//! 根因是 `on_cm_set_all_picked` 写成
//!
//! ```ignore
//! let mut c = cm.borrow_mut();       // RefMut 有 Drop，NLL 让它活到作用域末尾
//! ...
//! refresh(&w, &store_rc.borrow(), &cm.borrow());   // ← 同一 RefCell 再次借用
//! ```
//!
//! `RefCell::borrow()` 在已有可变借用时直接 panic，而 release profile 是
//! `panic = "abort"` —— GUI 进程没有控制台，panic 信息无处可看，表现就是
//! 「点一下软件自己关了」。
//!
//! 正确写法只有两种：把借用圈进 `{ ... }` 块，或显式 `drop(x)`。这条测试把
//! 两种之外的写法钉死。
//!
//! 只扫 `src/app/command_manager.rs`：`src/app.rs` 里同类字面形态都是
//! 「显式 drop」或「if/else 分支互斥」，静态扫描分不清分支，会误报。

use std::path::Path;

/// 一行里若声明了 `let mut <var> = <recv>.borrow_mut();`，返回 (var, recv)。
fn parse_mut(code: &str) -> Option<(String, String)> {
    let idx = code.find(".borrow_mut()")?;
    let head = &code[..idx];
    let (lhs, recv) = head.split_once('=')?;
    let var = lhs.trim().strip_prefix("let mut ")?.trim().to_string();
    let recv = recv.trim().trim_end_matches('.').trim().to_string();
    if var.is_empty() || recv.is_empty() {
        return None;
    }
    Some((var, recv))
}

#[test]
fn command_manager_refmut_never_outlives_scope_into_refresh() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/command_manager.rs");
    let src = std::fs::read_to_string(&path).expect("读不到 src/app/command_manager.rs");

    let mut depth: i32 = 0;
    // (变量名, 被借的 RefCell 名, 声明时的块深度, 是否已 drop)
    let mut open: Vec<(String, String, i32, bool)> = Vec::new();
    let mut bad: Vec<String> = Vec::new();

    for (i, line) in src.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");

        if let Some((var, recv)) = parse_mut(code) {
            open.push((var, recv, depth, false));
        }

        for (var, recv, d, dropped) in open.iter_mut() {
            if *dropped {
                continue;
            }
            if code.contains(&format!("drop({var})")) {
                *dropped = true;
                continue;
            }
            if depth >= *d {
                let reborrow = code.contains(&format!("{recv}.borrow()"));
                let flush = code.contains("refresh(") || code.contains("commit(");
                if reborrow || flush {
                    bad.push(format!(
                        "src/app/command_manager.rs:{} `{var}` 的 RefMut 还没释放就调用了 \
                         {} —— 会 panic `already mutably borrowed`（release 下 = 进程静默退出）",
                        i + 1,
                        if reborrow { format!("{recv}.borrow()") } else { "refresh()/commit()".into() }
                    ));
                }
            }
        }

        depth += code.matches('{').count() as i32 - code.matches('}').count() as i32;
        // 离开声明所在的块 → 借用已随作用域结束
        open.retain(|(_, _, d, _)| depth >= *d);
    }

    assert!(
        bad.is_empty(),
        "命令管理回调里存在「RefMut 跨 refresh」的写法（点一下就崩）：\n{}",
        bad.join("\n")
    );
}
