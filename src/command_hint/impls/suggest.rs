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
    let known = subcommands_for(first_word).is_some();
    let phase = input.phase(known);

    // ★ 触发阈值：正在补的词 ≥ 2 字符才弹（用户实测反馈：一个字符误命中
    //   太多，噪音大于价值）。但路径/参数前缀不受限 —— `/` `~` `-` 本身
    //   就是明确的意图信号，1 个字符就该弹。
    // ★ 整行搜索阶段（§3 放宽：head 非已知命令）按**整行长度**判 ——
    //   `docker l` 的词 `l` 只有 1 字符，但用户已经敲了 8 个字符，
    //   按词长拦掉会让整行预测完全出不来（实测踩过）。
    let pop = if phase == HintPhase::Command && !input.settled.is_empty() {
        input.line.chars().count() >= 2
    } else {
        should_pop(input.word, input.trailing_space, phase)
    };
    if !pop {
        return HintModel::closed(context);
    }

    // 子命令阶段：候选来自内置表，而不是「命令名」表。
    if phase == HintPhase::Subcommand {
        return subcommand_hint(store, &input, &q, context);
    }

    // ★ 命令阶段整行搜索（§3 定稿）：第一个词不是已知命令时（中文快捷命令
    //   名称「修 bug 并提交」、整行历史预测 `df -h | head -4`），匹配词是
    //   **用户敲的整段**，不是最后一个词 —— 否则多词名称永远搜不到。
    let match_q = if phase == HintPhase::Command {
        input.line.to_lowercase()
    } else {
        q.clone()
    };

    // ── 相关度打分（高保真 §3 排序五级键；2026-10-30 四轮收紧）──────────
    //   匹配度：精确 100 > 行前缀 80 > 词边界 65 > 子串 50。
    //
    //   ★ 模糊子序列档**删除**（用户实测：「无论怎么输入都有匹配」）：
    //     原第 5 档（18 分）判定 = query 字符按序出现在候选里即可，2 字符
    //     输入几乎能命中任何命令（`lh` → `sudo tee …aLias…`），内联建议被
    //     噪音淹没。行业口径（fish/Warp 的内联建议、FinalShell 的自动提示）
    //     都是**前缀 + 子串封顶**；fzf 级别的模糊只属于显式搜索场景。
    //
    //   ★ 中文/多词名称的搜索靠「切分子串」而不是字符模糊：query 与 text
    //     各自按空白切词后，任一 query 词是任一 text 词的子串即命中
    //     （40 分，介于词边界与整串子串之间）。「查看内存」切出「查看/
    //     内存」，text 名称「查看内存占用」同样切词后子串命中——
    //     中文无空格分词，这里按字符 2-gram 兜一层（`查内存` → 「查」
    //     「内存」都能在名称里找到）。fuzzy 档删掉后中文搜索由它承接。
    //
    //   来源平手裁决（第 2 级，`pri`）：快捷命令 0 < 内置 1 < 历史 2 ——
    //   用户自定义永远压过系统给的（Warp workflows / Raycast 口径）。
    //   历史另在匹配分上 -10，与裁决同向：自己配的排前面，敲过的也能浮上来。
    //
    //   末级兜底是**字母序**（`text` 字典序）——同样的输入永远得到同样的
    //   列表顺序，排序不确定的补全器看起来像在「抖」（VS Code/JetBrains）。
    fn word_boundary(text: &str, q: &str) -> bool {
        if q.is_empty() {
            return false;
        }
        let tl = text.to_lowercase();
        let ql = q.to_lowercase();
        tl.split_whitespace()
            .skip(1)
            .any(|w| w.starts_with(&ql))
    }
    /// 切分子串：两侧各按空白切词，任一 query 词是任一 text 词的子串 → 命中。
    /// 中文（无空格）整段是一个「词」，退化为整串子串；若整串不中，再按
    /// 字符 2-gram 切 query 兜一层（`查内存` → 「查」+「内存」）。
    fn token_substring(text: &str, q: &str) -> bool {
        let tl = text.to_lowercase();
        let ql = q.to_lowercase();
        let t_toks: Vec<&str> = tl.split_whitespace().collect();
        let q_toks: Vec<&str> = ql.split_whitespace().collect();
        // ① 词对词：任一 query 词是任一 text 词的子串。
        if q_toks
            .iter()
            .any(|qw| t_toks.iter().any(|tw| tw.contains(qw)))
        {
            return true;
        }
        // ② 中文 2-gram 兜底：query 无空白切不出多词且 text 含任一 2-gram。
        //    只对「含 CJK」的 query 启用 —— 英文短词的 2-gram 噪音太大。
        let has_cjk = ql.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c));
        if !has_cjk || ql.chars().count() < 2 {
            return false;
        }
        let grams: Vec<String> = {
            let cs: Vec<char> = ql.chars().collect();
            (0..cs.len().saturating_sub(1))
                .map(|i| cs[i..i + 2].iter().collect())
                .collect()
        };
        grams.iter().any(|g| tl.contains(g.as_str()))
    }
    fn five_level(text: &str, q: &str) -> i32 {
        if q.is_empty() {
            return 1;
        }
        let tl = text.to_lowercase();
        let ql = q.to_lowercase();
        if tl == ql {
            return 100;
        }
        if tl.starts_with(&ql) {
            return 80;
        }
        if word_boundary(text, q) {
            return 65;
        }
        if tl.contains(&ql) {
            return 50;
        }
        if token_substring(text, q) {
            return 40;
        }
        -1 // 不命中
    }

    // ── 收集全部命中（先不截断，合并去重 + 排序后再取前 N）──────────────
    struct Scored {
        s: i32,
        pri: i32,
        row: HintRowData,
    }

    let mut scored: Vec<Scored> = Vec::new();

    for c in store.quick_commands() {
        // 双字段匹配（§3 快捷命令行）：name 与 command 都参与，名称命中
        // 视同前缀不吃降档；填入的永远是 command。
        let s = five_level(&c.command, &match_q).max(five_level(&c.name, &match_q));
        if s < 0 {
            continue;
        }
        let quick_desc = if c.name.trim().is_empty() {
            t("用户自建的快捷命令", "User-defined quick command").to_string()
        } else {
            c.name.clone()
        };
        scored.push(Scored {
            s,
            pri: 0,
            row: HintRowData::new(
                c.command.clone(),
                HintKind::Quick,
                c.name.clone(),
                t("快捷命令", "Quick command").to_string(),
                c.command.clone(),
                quick_desc,
                0,
                &input,
                phase,
            ),
        });
    }

    let history = store.command_history();
    for cmd in history.iter().rev() {
        let s = five_level(cmd, &match_q) - 10;
        if s < 0 {
            continue;
        }
        scored.push(Scored {
            s,
            pri: 2,
            row: HintRowData::new(
                cmd.clone(),
                HintKind::History,
                String::new(),
                t("历史", "History").to_string(),
                cmd.clone(),
                t(
                    "敲过的命令；Enter 直接执行",
                    "Previously run; Enter runs it again",
                )
                .to_string(),
                2,
                &input,
                phase,
            ),
        });
    }

    // ── 跨来源合并（§3 去重②）：整段填入文本相同 = 同一条命令，只留一行。
    //    tag/说明取来源优先级更高（pri 小）的一边，分数取两边最大。
    //    词与行不互斥（去重③）：内置命令名与历史整行文本不同，不去。
    let mut deduped: Vec<Scored> = Vec::with_capacity(scored.len());
    for item in scored {
        if let Some(old) = deduped.iter_mut().find(|o| o.row.text == item.row.text) {
            if item.pri < old.pri {
                old.row = item.row;
                old.pri = item.pri;
            }
            old.s = old.s.max(item.s);
        } else {
            deduped.push(item);
        }
    }

    // 排序：匹配分降序 → 来源裁决升序 → 字母序兜底（确定性）。
    deduped.sort_by(|a, b| {
        b.s.cmp(&a.s)
            .then_with(|| a.pri.cmp(&b.pri))
            .then_with(|| a.row.text.cmp(&b.row.text))
    });

    let total = deduped.len();
    let (danger_token, danger_why) = danger_of(line);
    if total == 0 {
        if danger_token.is_empty() {
            return HintModel::closed(context);
        }
        // 危险命令 0 候选：框仍要弹 —— 告警条是安全信息（§3②，沙盘同款：
        // `rm -rf xyz` 即使没有路径候选，也要让红色告警出现在眼前）。
        return HintModel {
            open: true,
            phase,
            context,
            total: 0,
            rows: Vec::new(),
            ghost: String::new(),
            danger_token,
            danger_why,
        };
    }
    // ── 内联鬼影（§5 →）：首候选落在「整行」或「当前词」前缀上时，
    //    追加到行尾就能补全的那段。区分大小写（shell 语义，不 lowercase）。
    let ghost = match deduped.first().map(|x| x.row.text.clone()) {
        Some(first) if first.starts_with(line) => first[line.len()..].to_string(),
        Some(first) if !input.word.is_empty() && first.starts_with(input.word) => {
            first[input.word.len()..].to_string()
        }
        _ => String::new(),
    };
    let rows: Vec<HintRowData> = deduped
        .into_iter()
        .take(HINT_MAX_ROWS)
        .map(|x| x.row)
        .collect();
    HintModel {
        open: true,
        phase,
        context,
        total,
        rows,
        ghost,
        danger_token,
        danger_why,
    }
}

/// 危险命令表 —— **严格镜像** `ui/terminal_view.slint` 的 `danger-token()`
/// （`has-token` = 子串包含判定）。Rust 侧多一份是因为终端内联的行文本
/// 在这里算，Slint 的函数够不着；**改表必须两边同步**（§7「单一真源」
/// 的理想解是把 Slint 两处也收编到这里，牵动高亮渲染，暂未做）。
fn danger_of(line: &str) -> (String, String) {
    let s = line.to_lowercase();
    let has = |tok: &str| s.contains(tok);
    let (tok, why): (&str, &str) = if has("rm -rf") || has("rm -fr") || has("rm -r -f") {
        (
            "rm -rf",
            t(
                "递归强制删除，删掉的内容不进回收站。",
                "Recursive forced delete; nothing goes to a trash bin.",
            ),
        )
    } else if has("mkfs") {
        (
            "mkfs",
            t(
                "格式化会直接覆盖目标分区上的全部数据。",
                "Formatting overwrites everything on the target volume.",
            ),
        )
    } else if has("dd if=") {
        (
            "dd if=",
            t(
                "按块写入，写错 of= 会抹掉整块盘。",
                "Block-level write; a wrong of= wipes the whole disk.",
            ),
        )
    } else if has("shutdown") {
        (
            "shutdown",
            t(
                "会使远端主机停机，当前所有会话中断。",
                "Shuts the remote host down; all sessions drop.",
            ),
        )
    } else if has("reboot") {
        (
            "reboot",
            t(
                "会使远端主机重启，当前所有会话中断。",
                "Reboots the remote host; all sessions drop.",
            ),
        )
    } else if has("chmod -r 777") {
        (
            "chmod -R 777",
            t(
                "把权限放开到所有人可写。",
                "Opens the path to world-writable.",
            ),
        )
    } else if has("init 0") {
        (
            "init 0",
            t(
                "切到关机运行级。",
                "Switches to the halt runlevel.",
            ),
        )
    } else {
        ("", "")
    };
    (tok.to_string(), why.to_string())
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
                1,
                input,
                HintPhase::Subcommand,
            ));
        }
    }
    // 子命令表里没有的，退回到历史里以 `command ` 开头且命中子命令的条目。
    let history = store.command_history();
    for cmd in history.iter().rev() {
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
                    "敲过的命令；Enter 直接执行",
                    "Previously run; Enter runs it again",
                )
                .to_string(),
                2,
                input,
                HintPhase::Subcommand,
            ));
        }
    }
    let total = rows.len();
    if rows.is_empty() {
        return HintModel::closed(context);
    }
    // 内联鬼影（与 hint_model 同口径）：子命令阶段 `git st` → `git status` 的尾巴。
    let ghost = match rows.first().map(|r| r.text.clone()) {
        Some(first) if first.starts_with(input.line) => first[input.line.len()..].to_string(),
        Some(first)
            if !input.word.is_empty() && first.starts_with(input.word) =>
        {
            first[input.word.len()..].to_string()
        }
        _ => String::new(),
    };
    let (danger_token, danger_why) = danger_of(input.line);
    HintModel {
        open: true,
        phase: HintPhase::Subcommand,
        context,
        total,
        rows,
        ghost,
        danger_token,
        danger_why,
    }
}
