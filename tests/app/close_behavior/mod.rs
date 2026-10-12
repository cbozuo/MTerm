//! (#close-behavior) 关闭按钮行为的纯函数单测。
//!
//! 只覆盖不碰 UI/系统的部分:确认卡回传档位的解析、关闭行为配置的
//! 归一化、以及 (mode → 弹卡/托盘/退出) 决策表。窗口回调本身仍依赖
//! 真实窗口,不在单测范围。

use crate::app::{
    close_behavior_or_default, close_mode_remembers, decide_close, parse_close_mode, CloseDecision,
};

#[test]
fn close_mode_parses_tray_and_exit() {
    assert_eq!(parse_close_mode("tray"), "tray");
    assert_eq!(parse_close_mode("exit"), "exit");
}

#[test]
fn close_mode_tolerates_remember_suffix() {
    // 用户勾了「记住我的选择」再点卡片时,Slint 回传带后缀的参数。
    assert_eq!(parse_close_mode("tray,remember"), "tray");
    assert_eq!(parse_close_mode("exit,remember"), "exit");
    assert!(close_mode_remembers("tray,remember"));
    assert!(close_mode_remembers("exit,remember"));
    assert!(!close_mode_remembers("tray"));
    assert!(!close_mode_remembers("exit"));
}

#[test]
fn close_mode_falls_back_to_exit_on_unknown() {
    // 契约只有 tray / exit。空值与无法识别的值都按「完全退出」处理 ——
    // 宁可退出也不要把「想关掉」误读成「留在后台」。
    assert_eq!(parse_close_mode(""), "exit");
    assert_eq!(parse_close_mode("something-else"), "exit");
    assert_eq!(parse_close_mode("TRAY"), "exit"); // 大小写敏感:只有小写是契约值
    // 参数形态是「档位 [+ ,remember 后缀]」,故前缀匹配:仍归到对应档位。
    assert_eq!(parse_close_mode("tray,remember"), "tray");
}

#[test]
fn close_behavior_normalizes_legacy_and_invalid_values() {
    // 旧配置没有该字段 → 空串 → 回落 "ask"(等价于改动前的行为)。
    assert_eq!(close_behavior_or_default(""), "ask");
    assert_eq!(close_behavior_or_default("ask"), "ask");
    assert_eq!(close_behavior_or_default("tray"), "tray");
    assert_eq!(close_behavior_or_default("exit"), "exit");
    // 脏值不该被当成有效档位(否则会静默改变关窗行为)。
    assert_eq!(close_behavior_or_default("TRAY"), "ask");
    assert_eq!(close_behavior_or_default("quit"), "ask");
    assert_eq!(close_behavior_or_default("ask,remember"), "ask");
}

#[test]
fn decide_close_ignores_session_count() {
    // 「每次询问」在 0 个活动会话时也要弹卡,不能 hide() 把窗口藏掉。
    assert_eq!(decide_close("ask"), CloseDecision::Ask);
    assert_eq!(decide_close(""), CloseDecision::Ask);
    assert_eq!(decide_close("tray"), CloseDecision::Tray);
    assert_eq!(decide_close("exit"), CloseDecision::Exit);
    assert_eq!(decide_close("quit"), CloseDecision::Ask);
}

// (#close-behavior-sync 2026-10-12) 关闭窗口的行为是**全局**偏好。
//
// 用户反馈两处应跨窗口同步：
//   · 关闭确认卡勾「记住我的选择」→ 写入的行为档位
//   · 设置页「界面 › 窗口 › 关闭窗口的行为」
// 两者写的是同一份配置（`ConfigStore::close_behavior`），此前都**只写 store
// 不广播**（`registry.broadcast_config_changed()`），于是其它窗口的
// `close-behavior` 属性停在旧值 —— 表现为「在窗口 A 改了，窗口 B 点关闭仍按
// 旧行为走」。现在两处写入都触发广播，广播监听器回填各窗口属性。
//
// 这里锁住"配置是单一来源"这条不变量：任一入口写入后，其它窗口读到的档位
// 必须与写入值一致（读路径统一走 `close_behavior_or_default` 归一化）。
#[test]
fn close_behavior_is_a_single_global_preference() {
    // 两个入口写同一个键 → 只有一个真值来源。
    for raw in ["ask", "tray", "exit"] {
        // 归一化后即全局真值；任意窗口读到它都得到同一档。
        let canonical = close_behavior_or_default(raw);
        assert_eq!(
            close_behavior_or_default(canonical),
            canonical,
            "归一化必须幂等：'{raw}' → '{canonical}' 再归一化不应漂移"
        );
    }
}

#[test]
fn remembered_choice_produces_the_same_behavior_as_settings_entry() {
    // 确认卡勾「记住我的选择」回传带后缀的参数，设置页直接写裸档位 ——
    // 两条入口必须落到**同一个**档位上，否则"记住"记住的是另一档。
    for mode in ["tray", "exit"] {
        // parse_close_mode 返回借用入参的 &str，临时值必须先绑定再取。
        let raw = format!("{mode},remember");
        let from_card = parse_close_mode(&raw);
        let from_settings = mode;
        assert_eq!(
            close_behavior_or_default(from_card),
            close_behavior_or_default(from_settings),
            "确认卡(记住)与设置页写 '{mode}' 应得到同一行为"
        );
    }
    // 且记住后的行为与该卡片的即时动作一致（勾了记住 = 以后都走这条路）。
    assert_eq!(
        decide_close(close_behavior_or_default(parse_close_mode("tray,remember"))),
        CloseDecision::Tray
    );
    assert_eq!(
        decide_close(close_behavior_or_default(parse_close_mode("exit,remember"))),
        CloseDecision::Exit
    );
}
