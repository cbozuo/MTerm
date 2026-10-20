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
}

/// 彩虹括号的预设 id：非正则，`presentation::apply_rainbow_parens` 单独处理。
pub(crate) const RAINBOW_PRESET_ID: &str = "paren";

/// 内置预设表。**顺序即优先级**：具体形态（URL/IP/日期/UUID…）在前，宽泛的
/// 「数字」在最后，避免数字规则先把日期/IP 里的片段染掉。自定义规则整体排
/// 在预设之前（用户意图优先）。
///
/// 词级预设（日志级别 / 月份 / 大小数单位 / URL 协议）带 `(?i)`：终端日志里
/// `ERROR` 与 `error`、`HTTP://` 与 `http://` 一样常见，旧引擎就认大写级别，
/// 这里不能退步。形态类预设（IP/UUID/路径…）自带大小写字符类，无需旗标。
pub(crate) const HIGHLIGHT_PRESETS: &[HighlightPreset] = &[
    // ── 网络与地址 ───────────────────────────────────────────────
    HighlightPreset {
        id: "url",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "URL/URI",
        name_en: "URL/URI",
        pattern: r#"(?i)(https?|ftp|file):\/\/[-A-Za-z0-9+&@#/%?=~_|!:,.;]*[-A-Za-z0-9+&@#/%=~_|]"#,
        ansi: 4,
        group: 0,
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
    },
    HighlightPreset {
        id: "ipv6",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "IPv6",
        name_en: "IPv6",
        // 两段式：
        // ① 压缩形（含 `::`）：前缀 1~7 组 `hex:` 再跟一个冒号，或开头就是
        //    `::`；尾巴任意组。`fe80::1` / `::1` / `2001:db8::8a2e:370:7334`
        //    整段命中（旧写法 `(?::[0-9A-Fa-f]{0,4}){2,}` 只染 `fe80::`，尾巴丢）。
        // ② 完整八组形（无 `::`）：至少 4 组，覆盖 `1:2:3:4:5:6:7:8`。
        // ⚠️ 中间那版 `{0,4}(?::…){2,7}` 是**真回归**：十六进制组能吞掉时间戳
        //    `10:20:30`（纯数字也是合法十六进制），而 ipv6 排在 syslog/time 前面，
        //    整行日志时间都会被染成 IPv6 色。所以两段式都要求「有 `::` 或 ≥4 组」
        //    —— 时钟时间最多 3 组，天然排除。
        pattern: r#"(?:::|(?:[0-9A-Fa-f]{1,4}:)+:)(?:[0-9A-Fa-f]{1,4}:)*[0-9A-Fa-f]{1,4}|(?:[0-9A-Fa-f]{1,4}:){3,7}[0-9A-Fa-f]{1,4}"#,
        ansi: 6,
        group: 0,
    },
    HighlightPreset {
        id: "email",
        group_zh: "网络与地址",
        group_en: "Network",
        name_zh: "Email",
        name_en: "Email",
        pattern: r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
        ansi: 5,
        group: 0,
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
        pattern: r"\b\d{4}[-/](?:0[1-9]|1[0-2])[-/](?:0[1-9]|[12]\d|3[01])(?:[ T]\d{1,2}:\d{2}(?::\d{2})?(?:[.,]\d{1,9})?(?:Z|z)?)?\b",
        ansi: 3,
        group: 0,
    },
    HighlightPreset {
        id: "unix",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "Unix 日期",
        name_en: "Unix date",
        pattern: r"(?i)(?:Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s+(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}",
        ansi: 3,
        group: 0,
    },
    HighlightPreset {
        id: "syslog",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "Syslog 日期",
        name_en: "Syslog date",
        pattern: r"(?i)(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(?:[0-2]?\d|3[01])\s+(?:[01]?\d|2[0-3]):[0-5]\d:[0-5]\d",
        ansi: 3,
        group: 0,
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
    },
    HighlightPreset {
        id: "uuid",
        group_zh: "时间与标识",
        group_en: "Time & ID",
        name_zh: "UUID",
        name_en: "UUID",
        pattern: r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        ansi: 9,
        group: 0,
    },
    // ── 日志级别 ─────────────────────────────────────────────────
    HighlightPreset {
        id: "err",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Error / Failed",
        name_en: "Error / Failed",
        pattern: r"(?i)\b(?:error|failed|failure|exception|fatal|critical)\b",
        ansi: 1,
        group: 0,
    },
    HighlightPreset {
        id: "warn",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Warning",
        name_en: "Warning",
        pattern: r"(?i)\b(?:warning|warn|caution)\b",
        ansi: 3,
        group: 0,
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
    },
    HighlightPreset {
        id: "ok",
        group_zh: "日志级别",
        group_en: "Log levels",
        name_zh: "Success / OK",
        name_en: "Success / OK",
        pattern: r"(?i)\b(?:success|succeeded|ok|passed|done|complete|completed)\b",
        ansi: 2,
        group: 0,
    },
    // ── 数值与路径 ───────────────────────────────────────────────
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
        // （Rust regex 无 lookahead，所以不能用 `(?![A-Za-z])` 收尾。）
        pattern: r"(?i)\b\d+(?:\.\d+)?\s*(?:[KMGTPE](?:iB|B|i)?|B)\b",
        ansi: 12,
        group: 0,
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
        pattern: r#"(^|[\s"'(=])(/?[\w.@+-]+(?:/[\w.@+-]+)+)"#,
        ansi: 5,
        group: 2,
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
    },
];

/// 彩虹括号是否启用（总开关开 + 未被单独关掉）。
pub(crate) fn rainbow_parens_enabled(enabled: bool, disabled_presets: &[String]) -> bool {
    enabled && !disabled_presets.iter().any(|id| id == RAINBOW_PRESET_ID)
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
        assert_eq!(HIGHLIGHT_PRESETS.len(), 19);
        // 表序 = 优先级：num 必须是最后一条正则预设（宽泛数字垫底）。
        assert_eq!(HIGHLIGHT_PRESETS.last().unwrap().id, "num");
    }

    /// 总开关关掉 → 空表；开着 → 自定义在前、预设按表序。
    #[test]
    fn compile_orders_custom_before_presets() {
        assert!(compile_highlight_rules(false, &[], &[]).is_empty());
        let rules = compile_highlight_rules(true, &[], &[custom("zzz", false, "#112233")]);
        assert_eq!(rules.len(), 20); // 1 自定义 + 19 预设
        assert!(matches!(rules[0].color, RuleColor::Rgb(0x11, 0x22, 0x33)));
        // 关掉两条预售后表变短
        let rules = compile_highlight_rules(true, &["num".into(), "rule".into()], &[]);
        assert_eq!(rules.len(), 17);
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
    }

    /// IPv6：`fe80::` 与完整地址都要整段命中（旧写法尾巴丢）。
    /// 同时守住回归：时钟时间 `10:20:30` 绝不是 IPv6（纯数字也是合法十六进制，
    /// 宽 pattern 会把日志里的时间整段染错）。
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
        for clock in ["10:20:30", "Jul 14 10:20:30", "09:15:00.250", "1:2:3"] {
            assert!(
                !ipv6.matcher.is_match(clock),
                "clock time {clock:?} must not look like IPv6"
            );
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
