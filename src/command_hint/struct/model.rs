//! 命令提示框的**数据类型**：阶段 / 来源 / 输入切分 / 候选行 / 整块模型。
//!
//! 纯数据 + 只依赖自身字段的小逻辑（切词、阶段判定、apply 计算）。
//! 不依赖 Slint、不依赖 app 层 —— 分层说明见 `src/command_hint/mod.rs`。

use crate::i18n::t;

/// 提示框所处的补全阶段 —— 对应高保真上下文条里的那枚胶囊。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HintPhase {
    Command,
    Subcommand,
    Option,
    Path,
}

impl HintPhase {
    /// 胶囊文案。走 i18n（签名是 `t(zh, en)`，中文在前）。
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Command => t("命令", "Command"),
            Self::Subcommand => t("子命令", "Subcommand"),
            Self::Option => t("参数", "Option"),
            Self::Path => t("路径", "Path"),
        }
    }
}

/// 候选来源 —— 决定行首图标与来源标签。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HintKind {
    /// 用户自建的快捷命令（bolt 图标）。
    Quick = 1,
    /// 敲过的历史（时钟图标）。
    History = 0,
}

/// 把命令行拆成「当前正在补的那个词」及其前面已经落定的部分。
///
/// ★ 尾随空格是**分段信号**，不是噪音：高保真 §5 的 Enter 两段式正是靠它
/// 判定「这个词已经落定」。所以这里必须保留信息，不能先 trim。
pub(crate) struct HintInput<'a> {
    /// 原始整行（含尾随空格）。
    pub(crate) line: &'a str,
    /// 已落定的前几个词（不含正在补的那个）。
    pub(crate) settled: Vec<&'a str>,
    /// 正在补的词；空串 = 光标停在空格之后，还没开始打。
    pub(crate) word: &'a str,
    /// 整行是否以空白结尾。
    pub(crate) trailing_space: bool,
}

impl<'a> HintInput<'a> {
    pub(crate) fn parse(line: &'a str) -> Self {
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

    /// 判定阶段。
    ///
    /// `known` 由调用方给：第一个词在「内置子命令表」里 = 已知命令。
    ///   · 只有已知命令的第二段才叫子命令，否则一律算参数——
    ///     自建脚本（没有 spec）不会被标成子命令。
    ///   · ★（高保真 §3 定稿 2026-10-30）第一个词**不是**已知命令、当前词
    ///     又没有参数特征（`-` 开头）时 → 退回**命令阶段整行搜索**：
    ///     按中文名称搜快捷命令（「修 bug 并提交」含空格，靠词级匹配永远
    ///     搜不到）、整行历史预测（`df -h` → 历史 `df -h | head -4`）都靠它。
    ///     这是主流启动器的口径：输入不认识的东西时，搜的是自定义项与历史。
    pub(crate) fn phase(&self, known: bool) -> HintPhase {
        // 正在补的词里带路径分隔符 → 路径阶段（`ls /var/`、`cat ~/.bash`）。
        if self.word.contains('/') || self.word.starts_with('~') {
            return HintPhase::Path;
        }
        // 第一个词都还没打 → 命令阶段。
        if self.settled.is_empty() {
            // 行尾空格且一个字都没打，仍是命令阶段（用户刚开始）。
            return HintPhase::Command;
        }
        // 未知命令 + 非参数词 → 整行搜索（见上）。
        if !known && !self.word.starts_with('-') {
            return HintPhase::Command;
        }
        // 第二个词，且命令带子命令 → 子命令阶段。
        if self.settled.len() == 1 && known {
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
pub(crate) struct HintRowData {
    pub(crate) text: String,
    pub(crate) kind: HintKind,
    /// 一句话副标题（快捷命令的 name / 历史的空）。
    pub(crate) subtitle: String,
    /// 来源标签文案。
    pub(crate) source: String,
    /// 详情区语法签名；空 = 没有 spec，详情区只显示「直接回车执行」。
    pub(crate) signature: String,
    /// 详情区说明。
    pub(crate) description: String,
    /// 排序的**来源平手裁决键**（高保真 §3 排序五级键第 2 级）：
    /// 0 = 快捷命令（用户自定义置顶）＜ 1 = 内置知识 ＜ 2 = 历史。
    /// 匹配分相同的时候用它决胜负，最后才是字母序兜底。
    pub(crate) pri: i32,
    /// 选中这一条后，命令行**整行**应该变成什么。
    ///
    /// ★ 在 Rust 侧算，不在 Slint 侧拼：Slint 1.18 的 string 没有
    ///   `trim-end` / `rfind` / `substr`，替换「正在补的那个词」在 UI 里
    ///   只能靠 `replace-all` 硬凑，命令里出现重复片段就会错删。
    ///   而拆词的逻辑这里已经有了（`HintInput`），顺带产出 `apply` 只留一个来源。
    pub(crate) apply: String,
}

/// 算出「把 `cand` 填进去之后整行是什么」。
///
/// ★ 与 Slint 侧的 Enter 判据严格配对：判据是
///   `line == row.apply` → 执行；否则先填入。所以这里必须产出**稳定的**整行，
///   不能每次重算都漂移（否则 Enter 永远等不到「相等」的那一次）。
impl HintRowData {
    /// 统一构造函数：顺手把 `apply`（选中后整行长什么样）算好。
    /// 四处候选来源（快捷命令 / 历史 / 内置子命令 / 子命令阶段的历史）都走这里，
    /// 保证「填入」与「Enter 判据」永远看同一个值。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        text: String,
        kind: HintKind,
        subtitle: String,
        source: String,
        signature: String,
        description: String,
        pri: i32,
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
            pri,
            apply,
        }
    }
}

pub(crate) fn compute_apply(cand: &str, kind: HintKind, input: &HintInput, phase: HintPhase) -> String {
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
pub(crate) struct HintModel {
    pub(crate) open: bool,
    pub(crate) phase: HintPhase,
    /// 上下文条左边那段：`user@host : cwd`。
    pub(crate) context: String,
    /// 命中总数（可能大于 `rows.len()`，用于「N 条」计数；行数按 `HINT_MAX_ROWS` 封顶）。
    pub(crate) total: usize,
    pub(crate) rows: Vec<HintRowData>,
    /// ★ 内联鬼影（高保真 §5 →）：首候选落在「当前整行」或「当前词」的
    ///   前缀上时，追加到行尾就能补全的那段文本。区分大小写（shell 语义）。
    ///   空 = 无鬼影。`→` 键把它发给 PTY —— Tab 归 shell 之后，
    ///   这是「补命令名」的主捷径。
    pub(crate) ghost: String,
    /// 危险命令命中的模式原文（与 `terminal_view.slint` 的 `danger-token()`
    /// 同一张表，Rust 侧镜像——终端内联的行文本在 Rust 侧，Slint 函数够不着）。
    /// 非空 → 提示框列表上方出现红色告警条。
    pub(crate) danger_token: String,
    pub(crate) danger_why: String,
}

impl HintModel {
    pub(crate) fn closed(context: String) -> Self {
        Self {
            open: false,
            phase: HintPhase::Command,
            context,
            total: 0,
            rows: Vec::new(),
            ghost: String::new(),
            danger_token: String::new(),
            danger_why: String::new(),
        }
    }

    /// 明确「不弹」：密码输入 / 全屏程序 / 空提示符（P1 用）。
    ///
    /// 与 `closed()` 的区别只在语义：`hidden` 表示「判定为不该弹」，
    /// 调用方据此清掉终端内联浮层。两者产出相同，但分开命名让调用点自解释。
    pub(crate) fn hidden(context: String) -> Self {
        Self::closed(context)
    }
}
