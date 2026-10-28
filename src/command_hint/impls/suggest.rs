//! 候选生成的**纯逻辑**：触发阈值 / 相关度打分 / 排序 / 阶段分流。
//!
//! 这一层只吃 `&ConfigStore` 和一行文本，不碰 Slint、不碰 AppWindow ——
//! 所以能被单元测试直接驱动（`tests/app/command_hint/mod.rs`）。

use super::{subcommands_for, HintInput, HintKind, HintModel, HintPhase, HintRowData, HINT_MAX_ROWS};
use crate::config::ConfigStore;
use crate::i18n::t;

/// 是否该为这个词弹提示框（触发阈值）。
///
/// 用户实测反馈：**一个字符就弹，误命中太多**。所以默认要求 ≥2 字符；
/// 三类例外（前缀本身就是明确的意图信号，1 个字符就该弹）：
///   · 路径：`/` `~` 开头（`ls /` 就该列目录）；
///   · 参数：`-` 开头（`journalctl -` 就该列参数）；
///   · 子命令阶段已落定一个词（`git ` 空格后要列子命令，无字符可数）。
pub(crate) fn should_pop(word: &str, trailing_space: bool, phase: HintPhase) -> bool {
    if phase == HintPhase::Subcommand {
        return true;
    }
    if word.starts_with('/') || word.starts_with('~') || word.starts_with('-') {
        return true;
    }
    if trailing_space && phase != HintPhase::Command {
        return true;
    }
    word.chars().count() >= 2
}

/// 生成提示框数据。
///
/// `context` 是上下文条左侧文案（`user@host : cwd`）。cwd 由 OSC 7 送来，
/// 拿不到时调用方传空串，UI 只显示用户名那半截。
///
/// 两条口径（用户实测反馈后调整）：
///   · **触发阈值**：正在补的词 ≥ 2 字符才弹（1 个字符误命中太多，噪音大于
///     价值）；但以 `/` `~` `-` 开头的词不受限 —— 路径/参数的前缀本身就是
///     明确的意图信号，1 个字符就该弹。
///   · **排序**：按相关度（前缀命中 > 子串命中，短命令优先），而不是按
///     存储顺序截断 —— 否则候选永远只是「最早录进去的那几条」，
///     用户实测反馈：「待选择的命令就只有这几条吗？」
pub(crate) fn hint_model(store: &ConfigStore, line: &str, context: String) -> HintModel {
    let input = HintInput::parse(line);
    // 整行都是空白（或空串）→ 不弹。
    if input.settled.is_empty() && input.word.is_empty() {
        return HintModel::closed(context);
    }
    // 只在补**第一个词**或后续词时都要有实际输入；否则（例如只有空格）不弹。
    if input.word.is_empty() && input.settled.is_empty() {
        return HintModel::closed(context);
    }

    let q = input.word.to_lowercase();
    let first_word = input.settled.first().copied().unwrap_or(input.word);
    let has_subcommands = subcommands_for(first_word).is_some();
    let phase = input.phase(has_subcommands);

    // ★ 触发阈值：正在补的词 ≥ 2 字符才弹（用户实测反馈：一个字符误命中
    //   太多，噪音大于价值）。但路径/参数前缀不受限 —— `/` `~` `-` 本身
    //   就是明确的意图信号，1 个字符就该弹。
    if !should_pop(input.word, input.trailing_space, phase) {
        return HintModel::closed(context);
    }

    // 子命令阶段：候选来自内置表，而不是「命令名」表。
    if phase == HintPhase::Subcommand {
        return subcommand_hint(store, &input, &q, context);
    }

    // ── 相关度打分（越高越靠前）───────────────────────────────────────
    //   100 命令名前缀命中（打 `git` → 「git 相关」的快捷命令）
    //    90 命令前缀命中
    //    70 名字子串命中
    //    50 命令子串命中
    //   历史：在同样规则上 -10（用户更想要「自己配的」排前面，
    //         但最近敲过的也要能浮上来）
    fn score_of(text: &str, name: &str, q: &str) -> i32 {
        if q.is_empty() {
            return 1;
        }
        let tl = text.to_lowercase();
        let nl = name.to_lowercase();
        if nl.starts_with(q) {
            return 100;
        }
        if tl.starts_with(q) {
            return 90;
        }
        if nl.contains(q) {
            return 70;
        }
        if tl.contains(q) {
            return 50;
        }
        -1 // 不命中
    }

    // ── 收集全部命中（先不截断，排序后再取前 N）──────────────────────
    let mut scored: Vec<(i32, HintRowData)> = Vec::new();

    for c in store.quick_commands() {
        let s = score_of(&c.command, &c.name, &q);
        if s < 0 {
            continue;
        }
        // 短命令优先：同样命中时 `top` 比 `systemctl list-units …` 更相关。
        let s = s + (64 - c.command.chars().count().min(63)) as i32 / 8;
        // ★ 详情区要有内容：快捷命令的「说明」就是用户自己起的 name
        //   （命令管理里那张卡的标题），`signature` 用命令原文 ——
        //   否则详情区只能显示「暂无用法说明」，等于白占 94px。
        let quick_desc = if c.name.trim().is_empty() {
            t("用户自建的快捷命令", "User-defined quick command").to_string()
        } else {
            c.name.clone()
        };
        scored.push((
            s,
            HintRowData::new(
                c.command.clone(),
                HintKind::Quick,
                c.name.clone(),
                t("快捷命令", "Quick command").to_string(),
                c.command.clone(),
                quick_desc,
                &input,
                phase,
            ),
        ));
    }

    for cmd in store.command_history().iter().rev() {
        let s = score_of(cmd, "", &q);
        if s < 0 {
            continue;
        }
        let s = s - 10 + (64 - cmd.chars().count().min(63)) as i32 / 8;
        // 去重：同一条命令既在快捷命令里又在历史里时，只留快捷命令那条。
        if scored.iter().any(|(_, r)| r.text == *cmd) {
            continue;
        }
        scored.push((
            s,
            HintRowData::new(
                cmd.clone(),
                HintKind::History,
                String::new(),
                t("历史", "History").to_string(),
                cmd.clone(),
                t(
                    "敲过的命令；回车直接执行",
                    "Previously run; Enter runs it again",
                )
                .to_string(),
                &input,
                phase,
            ),
        ));
    }

    // 排序：分数降序；同分时短文本优先（更接近用户想敲的那条）。
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.text.chars().count().cmp(&b.1.text.chars().count()))
    });

    let total = scored.len();
    if total == 0 {
        return HintModel::closed(context);
    }
    let rows: Vec<HintRowData> = scored
        .into_iter()
        .take(HINT_MAX_ROWS)
        .map(|(_, r)| r)
        .collect();
    HintModel {
        open: true,
        phase,
        context,
        total,
        rows,
    }
}

/// 子命令阶段的候选：整条「命令 + 子命令」，这样 Tab 填进去就能直接补参数。
fn subcommand_hint(
    store: &ConfigStore,
    input: &HintInput,
    q: &str,
    context: String,
) -> HintModel {
    let command = input.settled.first().copied().unwrap_or("");
    let subs = subcommands_for(command).unwrap_or(&[]);
    let mut rows: Vec<HintRowData> = Vec::new();
    for (name, desc) in subs.iter() {
        if rows.len() >= HINT_MAX_ROWS {
            break;
        }
        // 行尾空格（word 为空）时把所有子命令列出来；否则按前缀过滤。
        if q.is_empty() || name.to_lowercase().starts_with(q) {
            rows.push(HintRowData::new(
                format!("{command} {name}"),
                HintKind::Quick,
                (*desc).to_string(),
                t("内置", "Built-in").to_string(),
                format!("{command} {name} [<opts>]"),
                (*desc).to_string(),
                input,
                HintPhase::Subcommand,
            ));
        }
    }
    // 子命令表里没有的，退回到历史里以 `command ` 开头且命中子命令的条目。
    for cmd in store.command_history().iter().rev() {
        if rows.len() >= HINT_MAX_ROWS {
            break;
        }
        if !cmd.starts_with(&format!("{command} ")) {
            continue;
        }
        if q.is_empty() || cmd.to_lowercase().contains(q) {
            if rows.iter().any(|r| r.text == *cmd) {
                continue;
            }
            rows.push(HintRowData::new(
                cmd.clone(),
                HintKind::History,
                String::new(),
                t("历史", "History").to_string(),
                cmd.clone(),
                t(
                    "敲过的命令；回车直接执行",
                    "Previously run; Enter runs it again",
                )
                .to_string(),
                input,
                HintPhase::Subcommand,
            ));
        }
    }
    let total = rows.len();
    if rows.is_empty() {
        return HintModel::closed(context);
    }
    HintModel {
        open: true,
        phase: HintPhase::Subcommand,
        context,
        total,
        rows,
    }
}
