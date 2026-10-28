//! (#cmd-hint 2026-10-27) 命令提示框的数据侧（高保真稿：
//! `docs/design/terminal-command-hint-hifi-2026-10-27.html`）。
//!
//! 高保真把提示框定为**四段式**：上下文条 / 候选列表 / 详情区 / 键盘脚注。
//! 原来的 `suggest_models()` 只产出「展示文本 + 类型」两列，够画一个扁平列表，
//! 画不了详情区（语法签名、一句话说明、来源）和阶段胶囊。这里补上那几列，
//! 并且**保持原函数不动** —— 完整历史面板还在读它。
//!
//! 三条与高保真对齐的硬规则：
//!   1. 阶段由**当前输入**判定（命令 / 子命令 / 参数 / 路径），不是猜的；
//!   2. 候选上限 = 可见行数（5），选中项不可能越出渲染出来的行；
//!   3. 空 query 不返回任何候选（高保真 §4「空串处短路」），避免一聚焦就刷屏。
//!
//! P1（终端内联）用的是同一份生成器：只要把「终端里正在敲的那一行」喂进来，
//! 阶段的判定与候选就完全一致，两处宿主不会长出两套语义。

use super::*;

/// 候选列表上限（用户实测反馈：5 条太少，放宽到 8 条）。
/// `ui/command_hint.slint` 的列表行数与可见行数按同一个数排，
/// 两边必须一致，否则选中项会落在看不见的行上。
pub(super) const HINT_MAX_ROWS: usize = 8;

/// 内置子命令表（P0 用）。只收**最常用**的几个命令 —— 提示框的价值在
/// 「常见的补得准」，不在「收全 man page」；收全了反而让候选排序变差。
///
/// 返回 `None` 表示这个命令没有子命令概念（`df`、`free`、自建脚本…），
/// 调用方据此把第二段判成「参数」而不是「子命令」。
pub(super) fn subcommands_for(command: &str) -> Option<&'static [(&'static str, &'static str)]> {
    let subs: &'static [(&'static str, &'static str)] = match command {
        "git" => &[
            ("status", "查看工作区状态"),
            ("commit", "提交暂存区改动"),
            ("push", "推送本地分支"),
            ("pull", "拉取并合并"),
            ("log", "查看提交历史"),
            ("diff", "对比差异"),
            ("checkout", "切换分支或还原文件"),
            ("branch", "管理分支"),
            ("rebase", "变基"),
            ("reset", "回退提交"),
            ("stash", "暂存未提交改动"),
            ("fetch", "只取不合并"),
            ("merge", "合并分支"),
            ("remote", "管理远端"),
            ("clone", "克隆仓库"),
        ],
        "systemctl" => &[
            ("status", "查看服务状态"),
            ("start", "启动服务"),
            ("stop", "停止服务"),
            ("restart", "重启服务"),
            ("enable", "设为开机自启"),
            ("disable", "取消自启"),
            ("reload", "重新加载配置"),
            ("list-units", "列出单元"),
        ],
        "docker" => &[
            ("ps", "列出容器"),
            ("images", "列出镜像"),
            ("logs", "查看容器日志"),
            ("exec", "进入容器执行"),
            ("run", "新建并启动容器"),
            ("rm", "删除容器"),
            ("rmi", "删除镜像"),
            ("build", "构建镜像"),
            ("compose", "编排多容器"),
        ],
        "kubectl" => &[
            ("get", "列出资源"),
            ("describe", "查看详情"),
            ("logs", "查看 Pod 日志"),
            ("exec", "进入容器"),
            ("apply", "应用清单"),
            ("delete", "删除资源"),
        ],
        "killall" => &[("nginx", "按名字结束进程")],
        _ => return None,
    };
    Some(subs)
}

/// 提示框所处的补全阶段 —— 对应高保真上下文条里的那枚胶囊。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum HintPhase {
    Command,
    Subcommand,
    Option,
    Path,
}

impl HintPhase {
    /// 胶囊文案。走 i18n（签名是 `t(zh, en)`，中文在前）。
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Command => crate::i18n::t("命令", "Command"),
            Self::Subcommand => crate::i18n::t("子命令", "Subcommand"),
            Self::Option => crate::i18n::t("参数", "Option"),
            Self::Path => crate::i18n::t("路径", "Path"),
        }
    }
}

/// 候选来源 —— 决定行首图标与来源标签。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum HintKind {
    /// 用户自建的快捷命令（bolt 图标）。
    Quick = 1,
    /// 敲过的历史（时钟图标）。
    History = 0,
}

/// 把命令行拆成「当前正在补的那个词」及其前面已经落定的部分。
///
/// ★ 尾随空格是**分段信号**，不是噪音：高保真 §5 的 Enter 两段式正是靠它
/// 判定「这个词已经落定」。所以这里必须保留信息，不能先 trim。
pub(super) struct HintInput<'a> {
    /// 原始整行（含尾随空格）。
    pub(super) line: &'a str,
    /// 已落定的前几个词（不含正在补的那个）。
    pub(super) settled: Vec<&'a str>,
    /// 正在补的词；空串 = 光标停在空格之后，还没开始打。
    pub(super) word: &'a str,
    /// 整行是否以空白结尾。
    pub(super) trailing_space: bool,
}

impl<'a> HintInput<'a> {
    pub(super) fn parse(line: &'a str) -> Self {
        let trailing_space = line.ends_with(|c: char| c.is_whitespace());
        let mut toks: Vec<&str> = line.split_whitespace().collect();
        // 行尾有空格 → 最后一个词已落定，`word` 为空；否则最后一个是正在补的词。
        let word = if trailing_space {
            ""
        } else {
            toks.pop().unwrap_or("")
        };
        Self {
            line,
            settled: toks,
            word,
            trailing_space,
        }
    }

    /// 判定阶段（高保真 §3 的四种胶囊）。
    ///
    /// `has_subcommands` 由调用方给：只有「命令本身带子命令」时第二段才叫
    /// 子命令，否则一律算参数。这样自建脚本（没有 spec）不会被标成子命令。
    pub(super) fn phase(&self, has_subcommands: bool) -> HintPhase {
        // 正在补的词里带路径分隔符 → 路径阶段（`ls /var/`、`cat ~/.bash`）。
        if self.word.contains('/') || self.word.starts_with('~') {
            return HintPhase::Path;
        }
        // 第一个词都还没打 → 命令阶段。
        if self.settled.is_empty() {
            // 行尾空格且一个字都没打，仍是命令阶段（用户刚开始）。
            return HintPhase::Command;
        }
        // 第二个词，且命令带子命令 → 子命令阶段。
        if self.settled.len() == 1 && has_subcommands {
            return HintPhase::Subcommand;
        }
        // 正在补 `-` / `--` 开头的词 → 参数阶段。
        if self.word.starts_with('-') {
            return HintPhase::Option;
        }
        // 其余（第三个词之后，或命令没有子命令）→ 参数阶段。
        HintPhase::Option
    }
}

/// 一行候选的全部展示字段（对应 `export struct HintRow`）。
pub(super) struct HintRowData {
    pub(super) text: String,
    pub(super) kind: HintKind,
    /// 一句话副标题（快捷命令的 name / 历史的空）。
    pub(super) subtitle: String,
    /// 来源标签文案。
    pub(super) source: String,
    /// 详情区语法签名；空 = 没有 spec，详情区只显示「直接回车执行」。
    pub(super) signature: String,
    /// 详情区说明。
    pub(super) description: String,
    /// 选中这一条后，命令行**整行**应该变成什么。
    ///
    /// ★ 在 Rust 侧算，不在 Slint 侧拼：Slint 1.18 的 string 没有
    ///   `trim-end` / `rfind` / `substr`，替换「正在补的那个词」在 UI 里
    ///   只能靠 `replace-all` 硬凑，命令里出现重复片段就会错删。
    ///   而拆词的逻辑这里已经有了（`HintInput`），顺带产出 `apply` 只留一个来源。
    pub(super) apply: String,
}

/// 算出「把 `cand` 填进去之后整行是什么」。
///
/// ★ 与 Slint 侧的 Enter 两段式判据严格配对：判据是
///   `line == row.apply` → 执行；否则先填入。所以这里必须产出**稳定的**整行，
///   不能每次重算都漂移（否则 Enter 永远等不到「相等」的那一次）。
impl HintRowData {
    /// 统一构造函数：顺手把 `apply`（选中后整行长什么样）算好。
    /// 四处候选来源（快捷命令 / 历史 / 内置子命令 / 子命令阶段的历史）都走这里，
    /// 保证「填入」与「Enter 判据」永远看同一个值。
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        text: String,
        kind: HintKind,
        subtitle: String,
        source: String,
        signature: String,
        description: String,
        input: &HintInput,
        phase: HintPhase,
    ) -> Self {
        let apply = compute_apply(&text, kind, input, phase);
        Self {
            text,
            kind,
            subtitle,
            source,
            signature,
            description,
            apply,
        }
    }
}

fn compute_apply(cand: &str, kind: HintKind, input: &HintInput, phase: HintPhase) -> String {
    // ★ 命令阶段：候选是「命令名」或「整条命令」，整行替换。
    if kind == HintKind::History || phase == HintPhase::Command {
        return cand.to_string();
    }
    // ★ 子命令阶段的候选**本身已经是整条**（`git commit`，见 subcommand_hint），
    //   所以必须整行替换 —— 走到下面任何一个分支都会拼成 `git git commit`
    //   （两条都踩过：先拼前缀、再拼尾随空格）。
    if phase == HintPhase::Subcommand {
        return format!("{cand} ");
    }
    // 路径：接上去继续补下一级，不加空格。
    if phase == HintPhase::Path {
        if input.trailing_space {
            return format!("{}{}", input.line, cand);
        }
        return rebuild(input, cand, "");
    }
    // 参数：替换正在补的那个词，并补一个尾随空格（进入下一阶段）。
    // ★ 尾随空格不是装饰：它同时是 Slint 侧「这个词已落定」的信号，
    //   也是下一步判阶段的输入。不补就会卡在同一阶段，Enter 永远只填入不执行。
    if input.trailing_space {
        return format!("{}{} ", input.line, cand);
    }
    rebuild(input, cand, " ")
}

/// 用 `cand` 替换正在补的那个词；`tail` 附在整行末尾。
fn rebuild(input: &HintInput, cand: &str, tail: &str) -> String {
    if input.settled.is_empty() {
        return format!("{cand}{tail}");
    }
    format!("{} {cand}{tail}", input.settled.join(" "))
}

/// 整块提示框的数据（对应 `ui/command_hint.slint` 的一组输入属性）。
pub(super) struct HintModel {
    pub(super) open: bool,
    pub(super) phase: HintPhase,
    /// 上下文条左边那段：`user@host : cwd`。
    pub(super) context: String,
    /// 命中总数（可能大于 `rows.len()`，用于「N 条」计数；行数仍按 5 封顶）。
    pub(super) total: usize,
    pub(super) rows: Vec<HintRowData>,
}

impl HintModel {
    fn closed(context: String) -> Self {
        Self {
            open: false,
            phase: HintPhase::Command,
            context,
            total: 0,
            rows: Vec::new(),
        }
    }

    /// 明确「不弹」：密码输入 / 全屏程序 / 空提示符（P1 用）。
    ///
    /// 与 `closed()` 的区别只在语义：`hidden` 表示「判定为不该弹」，
    /// 调用方据此清掉终端内联浮层。两者产出相同，但分开命名让调用点自解释。
    pub(super) fn hidden(context: String) -> Self {
        Self::closed(context)
    }
}

/// 是否该为这个词弹提示框（触发阈值）。
///
/// 用户实测反馈：**一个字符就弹，误命中太多**。所以默认要求 ≥2 字符；
/// 三类例外（前缀本身就是明确的意图信号，1 个字符就该弹）：
///   · 路径：`/` `~` 开头（`ls /` 就该列目录）；
///   · 参数：`-` 开头（`journalctl -` 就该列参数）；
///   · 子命令阶段已落定一个词（`git ` 空格后要列子命令，无字符可数）。
pub(super) fn should_pop(word: &str, trailing_space: bool, phase: HintPhase) -> bool {
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
pub(super) fn hint_model(store: &ConfigStore, line: &str, context: String) -> HintModel {
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
            crate::i18n::t("用户自建的快捷命令", "User-defined quick command").to_string()
        } else {
            c.name.clone()
        };
        scored.push((
            s,
            HintRowData::new(
                c.command.clone(),
                HintKind::Quick,
                c.name.clone(),
                crate::i18n::t("快捷命令", "Quick command").to_string(),
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
                crate::i18n::t("历史", "History").to_string(),
                cmd.clone(),
                crate::i18n::t(
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
        b.0.cmp(&a.0).then_with(|| a.1.text.chars().count().cmp(&b.1.text.chars().count()))
    });

    let total = scored.len();
    if total == 0 {
        return HintModel::closed(context);
    }
    let rows: Vec<HintRowData> =
        scored.into_iter().take(HINT_MAX_ROWS).map(|(_, r)| r).collect();
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
                crate::i18n::t("内置", "Built-in").to_string(),
                format!("{command} {name} [<opts>]"),
                (*desc).to_string(),
                &input,
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
                crate::i18n::t("历史", "History").to_string(),
                cmd.clone(),
                crate::i18n::t(
                    "敲过的命令；回车直接执行",
                    "Previously run; Enter runs it again",
                )
                .to_string(),
                &input,
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

/// 把 `HintModel` 写成**终端内联**那一组属性（P1）。
///
/// 与 `apply_hint_model`（命令栏）互不干扰：命令栏读 `hint-*`，
/// 终端内联读 `term-hint-*`。两处可以同时存在（终端里敲字 + 命令栏里也有
/// 半行草稿），但浮层只有当前聚焦的那个会真正显示。
pub(super) fn apply_terminal_hint_model(win: &AppWindow, model: &HintModel) {
    let rows = hint_rows_model(model);
    win.set_term_hint_rows(rows);
    win.set_term_hint_phase(model.phase.label().into());
    win.set_term_hint_context(model.context.clone().into());
    win.set_term_hint_total(model.total as i32);
    win.set_term_hint_open(model.open);
}

/// 把 `HintModel` 写成命令栏那一组属性（P0）。
///
/// 单点写入：hint-* 六列必须**一起**更新，否则会出现「列表换了、阶段胶囊还是
/// 上一次的」这种半新半旧的一帧。所有调用点都走这里。
pub(super) fn apply_hint_model(win: &AppWindow, model: &HintModel) {
    win.set_hint_rows(hint_rows_model(model));
    win.set_hint_phase(model.phase.label().into());
    win.set_hint_context(model.context.clone().into());
    win.set_hint_total(model.total as i32);
}

/// `HintModel.rows` → Slint 模型。两个 apply 共用，避免字段名抄两遍。
fn hint_rows_model(model: &HintModel) -> ModelRc<HintRow> {
    let rows: Vec<HintRow> = model
        .rows
        .iter()
        .map(|r| HintRow {
            text: r.text.clone().into(),
            kind: r.kind as i32,
            subtitle: r.subtitle.clone().into(),
            source: r.source.clone().into(),
            signature: r.signature.clone().into(),
            description: r.description.clone().into(),
            apply: r.apply.clone().into(),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

#[cfg(test)]
#[path = "../../tests/app/command_hint/mod.rs"]
mod command_hint_tests;