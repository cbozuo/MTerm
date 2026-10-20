//! 输出高亮规则：内置预设表 + 编译。
//!
//! (#hl-settings-hifi 2026-10-20) 旧实现是「一个总开关 + log/devops 两档打包 +
//! 6 档语义色的自定义规则」，两档规则各只有一张关键词表、且 devops 与 log 大量
//! 重复。按 iShellPro 的结构重做成**逐条可开关的预设表**（表序即优先级，
//! 先命中先着色），自定义规则扩到任意色（#RRGGBB，取色盘/48 色预设卡决定）。
//!
//! 三条预设正则是实跑修出来的真 bug（Rust regex 无 lookbehind/lookahead）：
//! * 文件路径：`(?<![\w.@+-])` 不可用 → 前置捕获组占位，只高亮路径组；
//! * 表格分隔线：旧写法在 `+-------+------+` 上只吃到前半段 → 锚定整行；
//! * IPv6：压缩形必须含 `::`、完整形至少 4 组 —— 中间那版宽 pattern 会把
//!   时钟时间 `10:20:30` 当 IPv6 染掉（见 ipv6 条目注释）。
//!
//! 彩虹括号（paren）不是正则规则，单独一个开关 + presentation 的特殊通道。

use super::state::RuleColor;
use crate::config::OutputHighlightRule;
use crate::picker::parse_hex;
use crate::terminal::CompiledOutputRule;

/// 一条内置预设规则。`ansi` 是 ANSI 槽位 —— 预设色**跟随主题色表**（换主题
/// 时终端里的高亮色与 UI 色点同源换装）；自定义规则才是绝对 RGB。
pub(crate) struct HighlightPreset {
    pub(crate) id: &'static str,
    pub(crate) group_zh: &'static str,
    pub(crate) group_en: &'static str,
    pub(crate) name_zh: &'static str,
    pub(crate) name_en: &'static str,
    pub(crate) pattern: &'static str,
    pub(crate) ansi: u8,
    /// 只高亮第几个捕获组（0 = 整个匹配）。path 用前置组占位，取 2。
    pub(crate) group: usize,
    /// (#hl-bold-tier 2026-10-21) 是否加粗：日志四级（err/warn/ok/info）=
    /// 加粗 + bright 档（最醒目）；标识/数值类 = 细体 + normal 档 —— 同色相
    /// 靠「粗细 + 明暗」双维度区分，19+ 条规则不再挤 7 个视觉档。
    pub(crate) bold: bool,
}

/// 彩虹括号的预设 id：非正则，`presentation::apply_rainbow_parens` 单独处理。
pub(crate) const RAINBOW_PRESET_ID: &str = "paren";

/// 内置预设表。**顺序即优先级**：具体形态（IP/URL/日期/UUID…）在前，宽泛的
/// 「数字」在最后，避免数字规则先把日期/IP 里的片段染掉。自定义规则整体排
/// 在预设之前（用户意图优先）。
///
/// (#hl-preset-rev1 2026-10-21) 规则集修订：
/// * ipv6 提到网络组最前 —— 2 字符 8 组形（`ab:cd:…:90`）会被排在前面的
///   mac 先吃掉 6 组、v4-mapped（`::ffff:1.2.3.4`）会被 ipv4 先吃尾段，
///   先命中先着色模型下整段地址必须由本条优先认领；
/// * uuid 换槽 9→15：红是错误语义的保留色，UUID 染红与 err 撞色误导；
/// * 词表扩展：err 补 panic/traceback/segfault 等 9 个运维高频致命词，
///   warn 补 deprecated/timeout/retry，ok 补 finished/skipped；
/// * 新增 duration / percent / hex / semver / diff 五条；
/// * 存量修正：iso 时区偏移、syslog 毫秒、unix 词首 \b、size 的 bytes
///   全词、path 的 Windows 盘符与 `~/` 家目录、url 的 `www.` 裸域名。
///
/// 词级预设（日志级别 / 月份 / 大小数单位 / URL 协议）带 `(?i)`：终端日志里
/// `ERROR` 与 `error`、`HTTP://` 与 `http://` 一样常见，旧引擎就认大写级别，
/// 这里不能退步。形态类预设（IP/UUID/路径…）自带大小写字符类，无需旗标。
pub(crate) const HIGHLIGHT_PRESETS: &[HighlightPreset] = &[
    // ── 网络与地址 ───────────────────────────────────────────────
    HighlightPreset {
        id: "ipv6",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "IPv6",
        name_en: "IPv6",
        // 三段式：
        // ① v4-mapped 形 `::ffff:1.2.3.4`：必须整段一条吃掉，否则尾段被
        //    ipv4 抢走（一截两色）。放在所有网络规则之前。
        // ② 压缩形（含 `::`）：前缀 1~7 组 `hex:` 再跟一个冒号，或开头就是
        //    `::`；尾巴任意组。`fe80::1` / `::1` / `2001:db8::8a2e:370:7334`
        //    整段命中（旧写法 `(?::[0-9A-Fa-f]{0,4}){2,}` 只染 `fe80::`，尾巴丢）。
        // ③ 完整形（无 `::`）：**恰好 8 组**。(#hl-preset-rev1) 原先放宽到
        //    「≥4 组」是为了覆盖弱形态，但 ipv6 提到 mac 之前后，4~7 组的
        //    宽松档会把 6 组 2 字符的 MAC（`aa:bb:cc:dd:ee:ff`）抢走 ——
        //    无 `::` 的合法 IPv6 本就必须恰好 8 组，收紧后 MAC 完整归位。
        // ⚠️ 宽 pattern 的真回归教训：十六进制组能吞掉时间戳 `10:20:30`
        //    （纯数字也是合法十六进制），整行日志时间都会被染成 IPv6 色。
        //    三段式都要求「有 `::` 或满 8 组」—— 时钟时间最多 3 组，天然排除。
        pattern: r#"(?:::[fF]{4}:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)(?:\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)){3}|(?:::|(?:[0-9A-Fa-f]{1,4}:)+:)(?:[0-9A-Fa-f]{1,4}:)*[0-9A-Fa-f]{1,4}|(?:[0-9A-Fa-f]{1,4}:){7}[0-9A-Fa-f]{1,4})"#,
        ansi: 6,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "url",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "URL/URI",
        name_en: "URL/URI",
        // (#hl-preset-rev1) 加 `www.` 裸域名分支：`访问 www.example.com` 这类
        // 无协议前缀的引用原先完全不染。带路径/查询时一并吃掉。
        pattern: r#"(?i)(?:(?:https?|ftp|file):\/\/[-A-Za-z0-9+&@#/%?=~_|!:,.;]*[-A-Za-z0-9+&@#/%=~_|]|\bwww\.[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+(?:/[^\s"')\]]*)?)"#,
        ansi: 4,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "ipv4",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "IPv4",
        name_en: "IPv4",
        pattern: r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b",
        ansi: 2,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "mac",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "MAC 地址",
        name_en: "MAC address",
        pattern: r"(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}",
        ansi: 12,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "email",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "Email",
        name_en: "Email",
        pattern: r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
        ansi: 13,
        group: 0,
        bold: false,
    },
    // ── 时间与标识 ───────────────────────────────────────────────
    HighlightPreset {
        id: "iso",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "ISO 日期",
        name_en: "ISO date",
        // (#hl-iso-datetime 2026-10-20) 日期后面紧跟的时间一并吃掉：
        // `2026-10-20 15:46:40.583` 原先被 iso（橙）+ time（青）切成两段
        // （用户截图「日期被分成两个颜色」）。现在整条时间戳一个颜色；
        // 不带日期前缀的纯时间仍归 time 规则。`Z` 收尾覆盖 `2026-10-20T15:46:40Z`
        // —— 旧写法在这条上连日期都匹配不到（T/Z 与数字同为 \w，\b 不成立）。
        // (#hl-preset-rev1) 尾部再加 `[+-]\d{2}:\d{2}` 时区偏移：
        // `…T15:46:40+08:00` 原先只染到秒，`+08:00` 被 time 规则二次染色
        // （同一时间戳两色）。有 Z 就没有偏移，二者在一个可选组内互斥。
        pattern: r"\b\d{4}[-/](?:0[1-9]|1[0-2])[-/](?:0[1-9]|[12]\d|3[01])(?:[ T]\d{1,2}:\d{2}(?::\d{2})?(?:[.,]\d{1,9})?(?:Z|z|[+-]\d{2}:\d{2})?)?\b",
        ansi: 3,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "unix",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "Unix 日期",
        name_en: "Unix date",
        // (#hl-preset-rev1) 词首补 \b：(?i) 下 `Common Jan 5` 会命中其中的
        // `mon`。词尾 `\d{1,2}\b` 一并补上（`May 5x` 不染）。
        pattern: r"(?i)\b(?:Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s+(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}\b",
        ansi: 3,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "syslog",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "Syslog 日期",
        name_en: "Syslog date",
        // (#hl-preset-rev1) 词首补 \b + 秒后毫秒：rsyslog RFC5424 高精度
        // 时间 `Oct 21 14:47:03.123` 原先整条不染（尾部毫秒吃不上）。
        pattern: r"(?i)\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(?:[0-2]?\d|3[01])\s+(?:[01]?\d|2[0-3]):[0-5]\d:[0-5]\d(?:\.\d+)?",
        ansi: 3,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "time",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "时间 HH:MM:SS",
        name_en: "Time HH:MM:SS",
        pattern: r"\b(?:[01]?\d|2[0-3]):[0-5]\d(?::[0-5]\d)?(?:\.\d+)?\b",
        ansi: 6,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "uuid",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "UUID",
        name_en: "UUID",
        // (#hl-preset-rev1) 槽位 9→15：红是 err 的语义保留色，UUID 是中性
        // 标识，染红与错误撞色有误导；白/灰档（15）不与任何规则冲突。
        pattern: r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        ansi: 15,
        group: 0,
        bold: false,
    },
    // ── 日志级别 ─────────────────────────────────────────────────
    HighlightPreset {
        id: "err",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Error / Failed",
        name_en: "Error / Failed",
        // (#hl-preset-rev1) 补运维高频致命词：panic（Go/Rust 崩溃主词）、
        // traceback（Python）、segfault/core dumped（段错误）、refused/denied
        // （连接/权限拒绝）、unauthorized/forbidden/not found（HTTP 4xx）、
        // severe/emerg（syslog / java.util.logging 级别名）。
        pattern: r"(?i)\b(?:error|failed|failure|exception|fatal|critical|panic|traceback|segfault|segmentation fault|core dumped|refused|denied|unauthorized|forbidden|not found|severe|emerg)\b",
        ansi: 1,
        group: 0,
        bold: true,
    },
    HighlightPreset {
        id: "warn",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Warning",
        name_en: "Warning",
        // (#hl-preset-rev1) 补 deprecat\* 词族（构建输出高频，含 Python 的
        // `DeprecationWarning` 整类名）、timeout/timed out、retry 系。timeout
        // 归黄不归红：可重试类异常染红会造成红字泛滥，这是与 err 的分工线。
        pattern: r"(?i)\b(?:warning|warn|caution|deprecat\w*|timeout|timed out|retry(?:ing|ed)?)\b",
        ansi: 3,
        group: 0,
        bold: true,
    },
    HighlightPreset {
        id: "info",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Info",
        name_en: "Info",
        pattern: r"(?i)\binfo(?:rmation)?\b",
        ansi: 4,
        group: 0,
        bold: true,
    },
    HighlightPreset {
        id: "debug",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Debug / Trace",
        name_en: "Debug / Trace",
        pattern: r"(?i)\b(?:debug|trace|verbose)\b",
        ansi: 8,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "ok",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Success / OK",
        name_en: "Success / OK",
        // (#hl-preset-rev1) 补 finished/skipped。done/ok 本就偏宽（cat 纯
        // 文本会误染），属高亮只染 plain 输出的固有代价，保持不收。
        pattern: r"(?i)\b(?:success|succeeded|ok|passed|done|complete|completed|finished|skipped)\b",
        ansi: 2,
        group: 0,
        bold: true,
    },
    // ── 数值与路径 ───────────────────────────────────────────────
    HighlightPreset {
        id: "duration",
        group_zh: "数值与路径",
        group_en: "Values & paths",
        name_zh: "耗时",
        name_en: "Duration",
        // (#hl-preset-rev1) 新增：`123ms` / `1.5s` / `2m30s` / `7µs` 是构建与
        // 请求日志的核心信息，原先完全不染。排在 size 之前：`5m` 这种单字母
        // 歧义形态优先给时长（日志里 5m 几乎总是分钟，5M/5Mi 才是容量）。
        // `\d+s\b` 对 `1st` 天然不成立（s 后跟 t 无边界），同理 2nd/3rd 安全。
        pattern: r"(?i)\b\d+(?:\.\d+)?(?:ns|µs|us|ms|[hms])\b|\b\d+(?:m\d+)+s\b",
        ansi: 6,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "percent",
        group_zh: "数值与路径",
        group_en: "Values & paths",
        name_zh: "百分比",
        name_en: "Percentage",
        // (#hl-preset-rev1) 新增：df 使用率列、进度输出。% 非 \w，尾部无需 \b。
        pattern: r"\b\d{1,3}(?:\.\d+)?%",
        ansi: 6,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "size",
        group_zh: "数值与路径",
        group_en: "Values & paths",
        name_zh: "内存 / 磁盘大小",
        name_en: "Memory / disk size",
        // (#hl-size-units 2026-10-20) 旧写法 `(?:i?B)?\b` 在 `5.9Gi` 上 **0 命中**：
        // `Gi`/`Mi`/`Ki` 里 `G` 与 `i` 都是单词字符，`\b` 在两者之间不成立，而
        // `i?B` 又要求 B —— 于是 `free -h` 这种 Linux 标准输出整段吃不到，用户
        // 反馈「内存大小规则开关没差别」。改成显式分支 `(?:iB|B|i)?` 收尾于 `\b`：
        // `5.9Gi` / `27Mi` / `0B` 全命中，`512KB` / `100MiB` / `1.5G` 不变。
        // (#hl-preset-rev1) 再补 `bytes` 全词分支（`123 bytes` 原先不染）。
        // （Rust regex 无 lookahead，所以不能用 `(?![A-Za-z])` 收尾。）
        pattern: r"(?i)\b\d+(?:\.\d+)?\s*(?:[KMGTPE](?:iB|B|i)?|B|bytes)\b",
        ansi: 4,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "hex",
        group_zh: "数值与路径",
        group_en: "Values & paths",
        name_zh: "十六进制值",
        name_en: "Hex literal",
        // (#hl-preset-rev1) 新增：`0xDEADBEEF` / `0x7ffd8c3b2a1c` —— 崩溃
        // 日志的指针地址、位掩码定位。要求 0x 前缀，裸十六进制串不染
        // （与普通词无法区分）。
        pattern: r"\b0[xX][0-9A-Fa-f]+\b",
        ansi: 15,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "path",
        group_zh: "数值与路径",
        group_en: "Values & paths",
        name_zh: "文件路径",
        name_en: "File path",
        // Rust regex 无 lookbehind：iShellPro 的 `(?<![\w.@+-])` 改写为前置
        // 捕获组占位，只高亮组 2（路径本身），前置的空白/引号/括号不染色。
        // 旧写法漏绝对路径（`/var/log/...` 0 命中）：`/?` 可挫前缀必须有。
        // (#hl-preset-rev1) 主体改双分支：Windows 盘符路径（`C:\Users\me`
        // 原先完全不染）+ POSIX 形（首段加 `~` 支持 `~/repo/src`）。
        pattern: r#"(^|[\s"'(=])([A-Za-z]:\\[\w.@+-]+(?:\\[\w.@+-]+)+|~?/?[\w.@+-]+(?:/[\w.@+-]+)+)"#,
        ansi: 5,
        group: 2,
        bold: false,
    },
    HighlightPreset {
        id: "semver",
        group_zh: "数值与路径",
        group_en: "Values & paths",
        name_zh: "版本号",
        name_en: "Version number",
        // (#hl-preset-rev1) 新增：`v1.2.3` / `1.2.3-rc.1` / `2.0.0+build.5`。
        // 表序在 ipv4 之后：IP 先认领四段形态，仅当 ipv4 关闭时
        // `1.2.3.4` 才可能被本条染前三段。
        pattern: r"\bv?\d+\.\d+\.\d+(?:[-+][\w.]+)*\b",
        ansi: 5,
        group: 0,
        bold: false,
    },
    // ── 结构 ─────────────────────────────────────────────────────
    HighlightPreset {
        id: "rule",
        group_zh: "结构",
        group_en: "Structure",
        name_zh: "表格分隔线",
        name_en: "Table separator",
        // 旧写法 `^[+\-=|]{3,}$` 在 `+-------+------+` 上只吃到前半段（find_iter
        // 从行中继续扫），且漏 `-----` / `=====`。整行规则 + whole_line 着色。
        pattern: r"^[+\-=|]{3,}$",
        ansi: 8,
        group: 0,
        bold: false,
    },
    HighlightPreset {
        id: "diff",
        group_zh: "结构",
        group_en: "Structure",
        name_zh: "diff 结构行",
        name_en: "Diff structure",
        // (#hl-preset-rev1) 新增：无色管道的 git diff 输出。只染 hunk 头
        // （到第二个 `@@` 为止，后面的函数上下文不染）与 `+++ `/`--- ` 文件
        // 头整行。**刻意不染增删行**（`^[+-]`）：ps 输出、表格行大量以
        // +/- 开头，误伤面不可控。
        pattern: r"^(?:@@ -\d+(?:,\d+)? \+\d+(?:,\d+)? @@|\+\+\+ .*|--- .*)",
        ansi: 4,
        group: 0,
        bold: false,
    },
    // 数字垫底：全表最宽泛的一条，必须排在最后（否则会先把日期/IP 里的
    // 数字片段染掉，具体形态就吃不到了）。
    HighlightPreset {
        id: "num",
        group_zh: "数值与路径",
        group_en: "Values & paths",
        name_zh: "数字",
        name_en: "Number",
        pattern: r"\b\d+(?:\.\d+)?\b",
        ansi: 6,
        group: 0,
        bold: false,
    },
];

/// 彩虹括号是否启用（总开关开 + 未被单独关掉）。
pub(crate) fn rainbow_parens_enabled(enabled: bool, disabled_presets: &[String]) -> bool {
    enabled && !disabled_presets.iter().any(|id| id == RAINBOW_PRESET_ID)
}

/// 预设槽位的**实际渲染槽**：终端把「bold + 基色(0-7)」渲染成 bright 档
/// （8-15），与 `presentation::idx_to_rgb` 的提升规则一致。设置弹框的色点
/// 按它取色，保证「色点 = 终端实色」（(#hl-preset-rev1) 前 ansi_dot 直接
/// 显示槽 i 原色，normal≠bright 的主题上色点与终端渲染色有肉眼偏差）。
pub(crate) fn rendered_slot(ansi: u8, bold: bool) -> u8 {
    if bold && ansi < 8 {
        ansi + 8
    } else {
        ansi
    }
}

/// 某条预设是否启用。
pub(crate) fn preset_enabled(enabled: bool, disabled_presets: &[String], id: &str) -> bool {
    enabled && !disabled_presets.iter().any(|d| d == id)
}

/// 把配置里的自定义规则 + 启用的预设编成按优先级排好的规则表：
/// **自定义在前，预设按表序**（表序即优先级，先命中先着色）。
/// 总开关关着 = 空表（= 关闭高亮）。
pub(crate) fn compile_highlight_rules(
    enabled: bool,
    disabled_presets: &[String],
    custom: &[OutputHighlightRule],
) -> Vec<CompiledOutputRule> {
    if !enabled {
        return Vec::new();
    }
    let mut out: Vec<CompiledOutputRule> = Vec::with_capacity(custom.len() + HIGHLIGHT_PRESETS.len());
    for rule in custom.iter().filter(|r| r.enabled && !r.pattern.trim().is_empty()) {
        let pattern = if rule.regex {
            rule.pattern.clone()
        } else {
            regex::escape(&rule.pattern)
        };
        let matcher = match regex::RegexBuilder::new(&pattern)
            .case_insensitive(!rule.case_sensitive)
            .build()
        {
            Ok(m) => m,
            Err(error) => {
                tracing::warn!(pattern = %rule.pattern, %error, "hl: custom rule failed to compile");
                continue;
            }
        };
        out.push(CompiledOutputRule {
            matcher,
            whole_line: rule.whole_line,
            color: parse_color(&rule.color),
            group: 0,
            // 自定义规则维持醒目（加粗 + 用户选色）；分层只对内置预设
            bold: true,
        });
    }
    for preset in HIGHLIGHT_PRESETS {
        if !preset_enabled(enabled, disabled_presets, preset.id) {
            continue;
        }
        // 预设正则都是字面量，理论上不可能编不过；真挂了说明表被改坏，
        // 跳过该条而不是让整个高亮失效。
        let matcher = match regex::RegexBuilder::new(preset.pattern).build() {
            Ok(m) => m,
            Err(error) => {
                tracing::error!(preset = preset.id, %error, "hl: preset regex broken");
                continue;
            }
        };
        out.push(CompiledOutputRule {
            matcher,
            // 表格分隔线是整行语义：任一 run 命中即染整行。
            whole_line: preset.id == "rule",
            color: RuleColor::Idx(preset.ansi),
            group: preset.group,
            bold: preset.bold,
        });
    }
    out
}

/// 自定义规则颜色：`#RRGGBB` 绝对色；旧配置里的语义名
/// （red/yellow/green/cyan/magenta/gray）按原 ANSI 槽位解析，读入即用。
pub(crate) fn parse_color(color: &str) -> RuleColor {
    if let Some(v) = parse_hex(color) {
        return RuleColor::Rgb(
            ((v >> 16) & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            (v & 0xFF) as u8,
        );
    }
    match color.trim().to_ascii_lowercase().as_str() {
        "yellow" => RuleColor::Idx(11),
        "green" => RuleColor::Idx(10),
        "cyan" => RuleColor::Idx(14),
        "magenta" => RuleColor::Idx(13),
        "gray" => RuleColor::Idx(8),
        // red 与一切无法识别的值
        _ => RuleColor::Idx(9),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(pattern: &str, regex: bool, color: &str) -> OutputHighlightRule {
        OutputHighlightRule {
            name: String::new(),
            pattern: pattern.to_string(),
            regex,
            case_sensitive: false,
            whole_line: false,
            color: color.to_string(),
            enabled: true,
        }
    }

    /// 表本身就是产品契约：id 唯一、正则全部可编译、捕获组序号不越界。
    #[test]
    fn preset_table_is_well_formed() {
        let mut ids: Vec<&str> = Vec::new();
        for p in HIGHLIGHT_PRESETS {
            assert!(!ids.contains(&p.id), "preset id duplicated: {}", p.id);
            ids.push(p.id);
            let re = regex::Regex::new(p.pattern)
                .unwrap_or_else(|e| panic!("preset {} regex broken: {e}", p.id));
            assert!(
                p.group < re.captures_len(),
                "preset {} group {} out of range (captures_len={})",
                p.id,
                p.group,
                re.captures_len()
            );
        }
        assert_eq!(HIGHLIGHT_PRESETS.len(), 24);
        // 表序 = 优先级：num 必须是最后一条正则预设（宽泛数字垫底）。
        assert_eq!(HIGHLIGHT_PRESETS.last().unwrap().id, "num");
        // ipv6 必须排在 mac / ipv4 之前（(#hl-preset-rev1) 表序修复）。
        let pos = |id: &str| HIGHLIGHT_PRESETS.iter().position(|p| p.id == id).unwrap();
        assert!(pos("ipv6") < pos("mac"), "ipv6 必须先于 mac 认领");
        assert!(pos("ipv6") < pos("ipv4"), "ipv6 必须先于 ipv4 认领");
        // uuid 是中性标识：红（1/9）是 err 的语义保留色，不得占用。
        assert_eq!(HIGHLIGHT_PRESETS.iter().find(|p| p.id == "uuid").unwrap().ansi, 15);
        // duration 必须先于 size（`5m` 单字母歧义优先给时长）。
        assert!(pos("duration") < pos("size"));
    }

    /// 色点取实际渲染槽：bold + 基色 → bright 档。
    #[test]
    fn rendered_slot_promotes_base_colours() {
        assert_eq!(rendered_slot(1, true), 9);
        assert_eq!(rendered_slot(4, true), 12);
        assert_eq!(rendered_slot(15, true), 15);
        assert_eq!(rendered_slot(8, true), 8);
        assert_eq!(rendered_slot(3, false), 3);
    }

    /// 总开关关掉 → 空表；开着 → 自定义在前、预设按表序。
    #[test]
    fn compile_orders_custom_before_presets() {
        assert!(compile_highlight_rules(false, &[], &[]).is_empty());
        let rules = compile_highlight_rules(true, &[], &[custom("zzz", false, "#112233")]);
        assert_eq!(rules.len(), 25); // 1 自定义 + 24 预设
        assert!(matches!(rules[0].color, RuleColor::Rgb(0x11, 0x22, 0x33)));
        // 关掉两条预设后表变短（24 - 2 = 22）
        let rules = compile_highlight_rules(true, &["num".into(), "rule".into()], &[]);
        assert_eq!(rules.len(), 22);
    }

    /// size 预设必须吃掉 Linux `free -h` / `df -h` 的单位写法（`Gi` / `Mi` /
    /// `Ki` / `B`）。旧写法在 `5.9Gi` 上 0 命中 —— 用户反馈「内存大小规则开
    /// 关没差别」，因为屏幕上的数字本来就没被这条规则染过（是 `num` 垫底染的
    /// 数字部分）。同时不能误吃日期 / 端口 / 时间戳。
    #[test]
    fn size_preset_matches_linux_units() {
        let preset = HIGHLIGHT_PRESETS
            .iter()
            .find(|p| p.id == "size")
            .expect("size preset");
        let re = regex::Regex::new(preset.pattern).expect("size regex");

        for (line, want) in [
            ("Mem:           93Gi        5.9Gi        86Gi", "93Gi"),
            ("Swap:         4.9Gi          0B         4.9Gi", "4.9Gi"),
            ("             27Mi       2.2Gi", "27Mi"),
            ("sum 512KB 100MiB 1.5G 4.9GB 2KiB", "512KB"),
            ("blocks 1B", "1B"),
        ] {
            assert_eq!(re.find(line).map(|m| m.as_str()), Some(want), "line={line}");
        }

        for s in ["2026-10-20", "14:47:03", "8080", "B2B", "100GiBps"] {
            assert!(re.find(s).is_none(), "{s} must not match the size preset");
        }
    }

    /// path 预设只高亮捕获组 2：前置空白/引号不染色，绝对路径也命中。
    /// （旧写法 `/var/log/...` 0 命中是真 bug。）
    /// (#hl-preset-rev1) 新覆盖：Windows 盘符路径与 `~/` 家目录。
    #[test]
    fn path_preset_uses_capture_group_and_matches_absolute() {
        let rules = compile_highlight_rules(true, &[], &[]);
        let path_rule = rules
            .iter()
            .find(|r| r.group == 2)
            .expect("path preset should carry group 2");
        let caps = path_rule
            .matcher
            .captures("see /var/log/nginx/error.log now")
            .expect("absolute path should match");
        assert_eq!(caps.get(2).unwrap().as_str(), "/var/log/nginx/error.log");
        let caps = path_rule.matcher.captures("log=\"/tmp/a b/x\"").unwrap();
        assert_eq!(caps.get(2).unwrap().as_str(), "/tmp/a");
        // 前面紧跟路径字符（如 `abc/def` 里的 abc）不该被算作独立命中边界
        let caps = path_rule.matcher.captures("key=path/to/file").unwrap();
        assert_eq!(caps.get(2).unwrap().as_str(), "path/to/file");
        // Windows 盘符路径（原先完全不染）
        let caps = path_rule
            .matcher
            .captures(r"error at C:\Users\me\app.log end")
            .expect("windows drive path should match");
        assert_eq!(caps.get(2).unwrap().as_str(), r"C:\Users\me\app.log");
        // `~/` 家目录
        let caps = path_rule
            .matcher
            .captures("edit ~/repo/src/main.rs now")
            .expect("~ home path should match");
        assert_eq!(caps.get(2).unwrap().as_str(), "~/repo/src/main.rs");
    }

    /// IPv6：`fe80::` 与完整地址都要整段命中（旧写法尾巴丢）。
    /// 同时守住回归：时钟时间 `10:20:30` 绝不是 IPv6（纯数字也是合法十六进制，
    /// 宽 pattern 会把日志里的时间整段染错）。
    /// (#hl-preset-rev1) 表序修复后新覆盖：2 字符 8 组形不被 mac 腰斩、
    /// v4-mapped 不被 ipv4 拆成两截 —— 整段必须被单条规则完整认领。
    #[test]
    fn ipv6_preset_matches_full_address() {
        let rules = compile_highlight_rules(true, &[], &[]);
        let ipv6 = rules
            .iter()
            .find(|r| r.group == 0 && r.matcher.is_match("fe80::1"))
            .expect("ipv6 preset");
        let m = ipv6.matcher.find("addr fe80::1%eth0 and 2001:db8::8a2e:370:7334").unwrap();
        assert_eq!(m.as_str(), "fe80::1");
        let m = ipv6.matcher.find("2001:db8::8a2e:370:7334").unwrap();
        assert_eq!(m.as_str(), "2001:db8::8a2e:370:7334");
        assert!(ipv6.matcher.is_match("::1"), "loopback should match");
        assert!(
            ipv6.matcher.is_match("1:2:3:4:5:6:7:8"),
            "full eight-group form should match"
        );
        // 2 字符 8 组：ipv6 在表序上先于 mac，整段一条吃掉
        let m = ipv6.matcher.find("addr ab:cd:ef:12:34:56:78:90 rest").unwrap();
        assert_eq!(m.as_str(), "ab:cd:ef:12:34:56:78:90");
        // v4-mapped：整段命中，尾段不留给 ipv4
        let m = ipv6.matcher.find("nat ::ffff:192.168.1.1 map").unwrap();
        assert_eq!(m.as_str(), "::ffff:192.168.1.1");
        // 表序级验证：跑完整规则表的认领（自定义在前、预设按表序），
        // `ab:…:90` 整段最终只被一条规则认领 —— mac 不能再腰斩前 6 组。
        let text = "ab:cd:ef:12:34:56:78:90";
        let mut claimed: Vec<(usize, usize)> = Vec::new();
        for rule in rules.iter().filter(|r| !r.whole_line) {
            let spans = if rule.group > 0 {
                rule.matcher
                    .captures_iter(text)
                    .filter_map(|c| c.get(rule.group).map(|m| (m.start(), m.end())))
                    .collect::<Vec<_>>()
            } else {
                rule.matcher
                    .find_iter(text)
                    .filter(|m| !m.is_empty())
                    .map(|m| (m.start(), m.end()))
                    .collect::<Vec<_>>()
            };
            for (s, e) in spans {
                if claimed.iter().any(|(cs, ce)| s < *ce && e > *cs) {
                    continue; // 已被更高优先级规则认领
                }
                claimed.push((s, e));
            }
        }
        assert!(
            claimed.contains(&(0, text.len())),
            "整段地址应被单条规则完整认领，实际分段 {:?}",
            claimed
        );
        for clock in ["10:20:30", "Jul 14 10:20:30", "09:15:00.250", "1:2:3"] {
            assert!(
                !ipv6.matcher.is_match(clock),
                "clock time {clock:?} must not look like IPv6"
            );
        }
    }

    /// (#hl-preset-rev1) 新增与修正的规则行为契约。
    #[test]
    fn preset_rev1_rules_match_their_targets() {
        let by_id = |id: &str| {
            let p = HIGHLIGHT_PRESETS.iter().find(|p| p.id == id).unwrap();
            regex::Regex::new(p.pattern).unwrap()
        };
        // duration
        let dur = by_id("duration");
        for (line, want) in [
            ("took 123ms", "123ms"),
            ("elapsed 1.5s", "1.5s"),
            ("wait 2m30s", "2m30s"),
            ("spin 7µs", "7µs"),
            ("up 3h", "3h"),
            ("t 500us", "500us"),
        ] {
            assert_eq!(dur.find(line).map(|m| m.as_str()), Some(want), "line={line}");
        }
        for s in ["1st", "2nd", "3rd"] {
            assert!(dur.find(s).is_none(), "{s} 不是时长");
        }
        // percent
        let pct = by_id("percent");
        assert_eq!(pct.find("Use% 72%").map(|m| m.as_str()), Some("72%"));
        assert_eq!(pct.find("99.9% done").map(|m| m.as_str()), Some("99.9%"));
        assert!(pct.find("Use%").is_none());
        // hex
        let hex = by_id("hex");
        assert_eq!(hex.find("at 0xDEADBEEF").map(|m| m.as_str()), Some("0xDEADBEEF"));
        assert_eq!(hex.find("ptr 0x7ffd8c3b2a1c").map(|m| m.as_str()), Some("0x7ffd8c3b2a1c"));
        assert!(hex.find("hash deadbeef").is_none(), "裸十六进制串不染");
        // semver
        let sv = by_id("semver");
        assert_eq!(sv.find("upgrade v1.2.3").map(|m| m.as_str()), Some("v1.2.3"));
        assert_eq!(sv.find("rel 1.2.3-rc.1").map(|m| m.as_str()), Some("1.2.3-rc.1"));
        assert!(sv.find("ver 1.2").is_none(), "两段不是 semver");
        // diff 结构
        let diff = by_id("diff");
        assert_eq!(diff.find("@@ -1,3 +1,4 @@").map(|m| m.as_str()), Some("@@ -1,3 +1,4 @@"));
        // hunk 头后的函数上下文不染
        assert_eq!(diff.find("@@ -12,3 +12,7 @@ pub fn foo()").map(|m| m.as_str()), Some("@@ -12,3 +12,7 @@"));
        assert_eq!(diff.find("+++ b/src/main.rs").map(|m| m.as_str()), Some("+++ b/src/main.rs"));
        assert_eq!(diff.find("--- a/src/main.rs").map(|m| m.as_str()), Some("--- a/src/main.rs"));
        assert!(diff.find("++x").is_none());
        assert!(diff.find("--x").is_none(), "--- 要求后跟空格，防误伤");
        // url 的 www 裸域名分支
        let url = by_id("url");
        assert_eq!(url.find("visit www.example.com now").map(|m| m.as_str()), Some("www.example.com"));
        assert_eq!(
            url.find("see www.example.com/a/b?q=1 end").map(|m| m.as_str()),
            Some("www.example.com/a/b?q=1")
        );
        assert_eq!(url.find("open https://a.b/c").map(|m| m.as_str()), Some("https://a.b/c"));
        // iso 时区偏移：整段时间戳一个 pattern 吃完
        let iso = by_id("iso");
        assert_eq!(
            iso.find("at 2026-10-20T15:46:40+08:00 end").map(|m| m.as_str()),
            Some("2026-10-20T15:46:40+08:00")
        );
        assert_eq!(
            iso.find("d 2026-10-20 15:46:40.583 z").map(|m| m.as_str()),
            Some("2026-10-20 15:46:40.583")
        );
        // syslog 高精度毫秒
        let syslog = by_id("syslog");
        assert_eq!(
            syslog.find("Oct 21 14:47:03.123 host").map(|m| m.as_str()),
            Some("Oct 21 14:47:03.123")
        );
        // size 的 bytes 全词
        let size = by_id("size");
        assert_eq!(size.find("read 123 bytes").map(|m| m.as_str()), Some("123 bytes"));
        // unix 词首边界
        let unix = by_id("unix");
        assert!(unix.is_match("Mon Jan 5"));
        assert!(!unix.is_match("Common Jan 5"), "词中 mon 不该命中");
        // 词表扩展
        let err = by_id("err");
        for w in ["panic", "Traceback", "segmentation fault", "core dumped", "refused", "not found"] {
            assert!(err.is_match(w), "{w} 应命中 err");
        }
        let warn = by_id("warn");
        for w in ["deprecated", "DeprecationWarning", "timed out", "retrying"] {
            assert!(warn.is_match(w), "{w} 应命中 warn");
        }
        let ok = by_id("ok");
        for w in ["finished", "skipped"] {
            assert!(ok.is_match(w), "{w} 应命中 ok");
        }
    }

    /// 表格分隔线整行命中（`+-------+------+` 旧写法只吃前半段）。
    /// 表格线是唯一的 whole_line 预设，按这个特征找（表尾是 num，别用 last）。
    #[test]
    fn table_separator_matches_whole_line() {
        let rules = compile_highlight_rules(true, &[], &[]);
        let rule = rules
            .iter()
            .find(|r| r.whole_line)
            .expect("table separator preset is the whole_line one");
        for line in ["+-------+------+", "-----", "=====", "|---+---|"] {
            let m = rule.matcher.find(line);
            assert_eq!(m.map(|m| m.as_str()), Some(line), "line {line:?}");
        }
        assert!(!rule.matcher.is_match("+---+ foo"));
    }

    /// 自定义色：#RRGGBB → 绝对 RGB；旧语义名 → 原 ANSI 槽位。
    #[test]
    fn parse_color_accepts_hex_and_legacy_names() {
        assert!(matches!(parse_color("#1FC8B4"), RuleColor::Rgb(0x1F, 0xC8, 0xB4)));
        assert!(matches!(parse_color("1fc8b4"), RuleColor::Rgb(0x1F, 0xC8, 0xB4)));
        assert!(matches!(parse_color("red"), RuleColor::Idx(9)));
        assert!(matches!(parse_color("yellow"), RuleColor::Idx(11)));
        assert!(matches!(parse_color("green"), RuleColor::Idx(10)));
        assert!(matches!(parse_color("cyan"), RuleColor::Idx(14)));
        assert!(matches!(parse_color("magenta"), RuleColor::Idx(13)));
        assert!(matches!(parse_color("gray"), RuleColor::Idx(8)));
        assert!(matches!(parse_color("nonsense"), RuleColor::Idx(9)));
    }

    /// 彩虹括号开关：总开关 + 单独关闭都要能关掉。
    #[test]
    fn rainbow_toggle_follows_master_and_preset_switch() {
        assert!(rainbow_parens_enabled(true, &[]));
        assert!(!rainbow_parens_enabled(false, &[]));
        assert!(!rainbow_parens_enabled(true, &[RAINBOW_PRESET_ID.to_string()]));
        assert!(rainbow_parens_enabled(true, &["num".to_string()]));
    }
}
