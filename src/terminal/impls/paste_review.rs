//! (#multiline-paste-hifi 2026-11-10) 多行粘贴确认弹框的分析层。
//!
//! 高保真 `docs/design/multiline-paste-hifi-2026-11-06-v1.html` 与权威 spec
//! `multiline-paste-spec-2026-11-06.md`（§5.1 三档 / §5.2 预览六件套 /
//! §5.3 D1–D5）的 Rust 侧单一出处：档位判定、危险行 A/B 级标记（D4）、
//! 控制字符计数（§5.2⑦）、折叠窗口与预览行模型、页脚组合文案。
//! Slint 的 string 没有逐字符访问，规则匹配/文案组装必须在 Rust 侧做
//! （与 send_check 同一硬原因）。
//!
//! 与 send_check 的关系：复用其引号剥离 / 段切分 / 段首前缀剥离的防误报
//! 口径（`echo "rm -rf x"` 不误报、`sudo rm -rf` 直达 rm -rf），但**规则表
//! 独立** —— 发送窗的词表按发送窗体验收调窄过（如只报 `kill -9 1/-1`），
//! 粘贴弹框按 spec D4 全量模式 + A/B 分级，两表各自演进互不牵连。

use crate::terminal::send_check::{find_sep, strip_prefix, strip_quoted};

/// 危险级别：A = 不可逆破坏（红），B = 提权/管道执行（黄）。spec §5.3 D4。
pub const RISK_A: i32 = 1;
pub const RISK_B: i32 = 2;

/// 单条风险命中：0-based 行号 + 级别 + 类别名（双语）+ 行尾徽标图标 glyph。
#[derive(Debug, Clone, PartialEq)]
pub struct PasteRiskHit {
    pub line: usize,
    pub level: i32,
    pub category_zh: &'static str,
    pub category_en: &'static str,
    pub icon: &'static str,
}

/// 档位。spec §5.1：紧凑 1–3 行且 ≤600 字符 / 标准 4–12 行且 ≤600 字符 /
/// 评审 >12 行 **或** >600 字符（任一超出即评审档）。
/// 行数为「显示行数」口径（`lines()` 语义，末尾换行不计行 —— 高保真 B 帧
/// 「12 行 / 364 字符」带尾换行仍归标准档，G 帧「1 行 / 71 字符」同理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteTier {
    Compact,
    Standard,
    Review,
}

const COMPACT_MAX_LINES: usize = 3;
const TIER_CHAR_LIMIT: usize = 600;
const TIER_LINE_LIMIT: usize = 12;
/// spec §5.2⑥：超过 200 行时走摘要（虚拟化）展示，展开全部后由 ListView
/// 按可视窗口按需渲染。
const VIRTUAL_LINE_LIMIT: usize = 200;
/// force_review 阈值：>100 KB 即使关掉「不再提示」也必须复核（spec §5.3 D3）。
pub const FORCE_REVIEW_BYTES: usize = 100 * 1024;
/// 虚拟化（摘要）模式的样本行数（高保真 F 帧：通告行 + 12 行样本）。
const VIRTUAL_SAMPLE_LINES: usize = 12;

/// 一次粘贴的完整分析结果。后台线程构造，经 `invoke_from_event_loop`
/// 回 UI 线程（字段全部 Send）。
#[derive(Debug, Clone)]
pub struct PasteAnalysis {
    /// 显示行（已剥控制字符、`\t` 展开为 4 空格 —— 仅预览展示用；
    /// 风险检测用原始行）。
    pub lines: Vec<String>,
    pub line_count: usize,
    /// 字符数（chars 口径，含末尾换行 —— 高保真 B 帧注记「12 行正文 + 末尾换行」）。
    pub char_count: usize,
    pub trailing_newline: bool,
    pub a_count: usize,
    pub b_count: usize,
    /// 控制字符计数（bracketed 模式下 `encode_pasted_text` 将剥除它们）。
    pub esc_count: usize,
    pub ctrlc_count: usize,
    pub risks: Vec<PasteRiskHit>,
    /// >100 KB 强制复核（spec §5.3 D3 例外路径）。
    pub force_review: bool,
    /// 行数 >200 或字节 >100 KB：预览走摘要 + 展开全部（spec §5.2⑥）。
    pub virtualized: bool,
    pub tier: PasteTier,
}

/// 折叠窗口（spec §5.1「首尾各留 1 行 … 折叠」的落地定稿）：
/// 行 1 恒显；窗口 = [首风险行, 末风险行+2]（无风险 → [第 2 行, 第 8 行]），
/// 头/尾窗口外的行折叠成提示行。高保真 B 帧（风险 4/5/8 → 显示 1 + 4..10，
/// 折 2+2）与该规则逐行吻合；C/E 帧为评审页手工采样，以规则为准
/// （折叠账三方相等由单测硬断言）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldWindow {
    /// 行 1 与窗口起点之间被折叠的行数（0 = 无头折叠行）。
    pub head_hidden: usize,
    /// 窗口起点（0-based，含）。
    pub start: usize,
    /// 窗口终点（0-based，含）。
    pub end: usize,
    /// 窗口终点之后被折叠的行数（0 = 无尾折叠行）。
    pub tail_hidden: usize,
}

impl FoldWindow {
    /// 被折叠总行数 = 「展开全部（另有 N 行）」按钮的 N。
    pub fn hidden_total(&self) -> usize {
        self.head_hidden + self.tail_hidden
    }
}

/// 行号口径的词边界判断：段首独立词（`sudo`/`su` 不会咬到 `sudoers`）。
fn word_at(lower: &str, w: &str) -> bool {
    lower.starts_with(w)
        && lower[w.len()..]
            .chars()
            .next()
            .map_or(true, |c| !c.is_ascii_alphanumeric() && c != '_')
}

/// 段级危险判定（段 = 已剥引号）。返回 (级别, 类别名 zh, 类别名 en, 图标)。
/// 顺序：先判 A 级（剥前缀后的段，`sudo rm -rf` 直达 rm -rf），再判 B 级；
/// B 级里 `sudo`/`su` 判在剥前缀**前**（剥掉就看不见 sudo 了）。
fn paste_seg_risk(seg: &str) -> Option<(i32, &'static str, &'static str, &'static str)> {
    let s = seg.trim_start();
    if s.is_empty() {
        return None;
    }
    let bare = strip_prefix(s);
    if let Some(hit) = paste_seg_risk_a(bare) {
        return Some(hit);
    }
    let lower = s.to_lowercase();
    if word_at(&lower, "sudo") || word_at(&lower, "su") {
        return Some((RISK_B, "权限提升", "privilege escalation", "shield-alert"));
    }
    paste_seg_risk_b(bare)
}

/// A 级（不可逆破坏）。spec §5.3 D4：rm -rf / mkfs / dd of=/dev/ / >/dev/sd* /
/// fork 炸弹 / :>file·truncate -s 0 / shutdown 族 / DROP DATABASE|TABLE·dropdb。
fn paste_seg_risk_a(s: &str) -> Option<(i32, &'static str, &'static str, &'static str)> {
    let s = s.trim_start();
    let lower = s.to_lowercase();
    let starts = |p: &str| lower.starts_with(p);
    // rm 的 -r(f) 组合：rm -rf / rm -fr / rm -r -f / rm -Rf …（send_check 同款）
    let mut rm_r = false;
    let mut rm_f = false;
    let mut saw_arg = false;
    for a in s.split_whitespace().skip(1).take(3) {
        saw_arg = true;
        let la = a.to_lowercase();
        if la.starts_with('-') && !la.starts_with("--") {
            if la.contains('r') {
                rm_r = true;
            }
            if la.contains('f') {
                rm_f = true;
            }
        } else {
            break;
        }
    }
    if saw_arg && rm_r && rm_f {
        return Some((RISK_A, "递归强制删除", "recursive delete", "trash"));
    }
    if starts("mkfs") {
        return Some((RISK_A, "格式化文件系统", "format fs", "disk"));
    }
    if starts("dd ") && lower.contains("of=/dev/") {
        return Some((RISK_A, "直写块设备", "raw write", "disk"));
    }
    if lower.contains("> /dev/sd") || lower.contains(">/dev/sd") {
        return Some((RISK_A, "覆盖磁盘设备", "overwrite device", "disk"));
    }
    if s.starts_with(':') && s.contains("(){") && s.contains("|:&") {
        return Some((RISK_A, "fork 炸弹", "fork bomb", "error"));
    }
    // 清空/截断：:> file（>> 是追加，不算）与 truncate -s 0。
    if (starts(":>") || (starts(">") && !starts(">>")))
        && lower[if starts(":>") { 2 } else { 1 }..]
            .trim_start()
            .chars()
            .next()
            .map_or(false, |c| c != '&' && c != '|' && c != ';')
    {
        return Some((RISK_A, "清空/截断文件", "truncate file", "file-x"));
    }
    if starts("truncate -s 0") || starts("truncate -s 00") {
        return Some((RISK_A, "清空/截断文件", "truncate file", "file-x"));
    }
    if word_at(&lower, "shutdown")
        || word_at(&lower, "reboot")
        || word_at(&lower, "poweroff")
        || word_at(&lower, "halt")
        || starts("init 0")
        || starts("init 6")
    {
        return Some((RISK_A, "关机或重启", "shutdown", "clock"));
    }
    if lower.contains("drop database")
        || lower.contains("drop table")
        || word_at(&lower, "dropdb")
    {
        return Some((RISK_A, "数据库破坏", "database drop", "error"));
    }
    None
}

/// B 级（提权 / 管道执行，sudo/su 之外的段级部分）。
fn paste_seg_risk_b(s: &str) -> Option<(i32, &'static str, &'static str, &'static str)> {
    let s = s.trim_start();
    let lower = s.to_lowercase();
    let starts = |p: &str| lower.starts_with(p);
    let toks: Vec<&str> = s.split_whitespace().collect();
    // chmod 777（任意位置含 777 数字串）/ chown -R
    if word_at(&lower, "chmod") {
        if toks.iter().skip(1).any(|t| t.len() >= 3 && t.ends_with("777")) {
            return Some((RISK_B, "权限全开", "permission 777", "tune"));
        }
    }
    if word_at(&lower, "chown")
        && toks.iter().any(|t| t.eq_ignore_ascii_case("-r") || t.starts_with("-R"))
    {
        return Some((RISK_B, "权限全开", "permission 777", "tune"));
    }
    if word_at(&lower, "useradd")
        || word_at(&lower, "usermod")
        || word_at(&lower, "passwd")
        || word_at(&lower, "visudo")
    {
        return Some((RISK_B, "账户与凭据变更", "account change", "input"));
    }
    if word_at(&lower, "iptables")
        || word_at(&lower, "nft")
        || word_at(&lower, "ufw")
        || starts("firewall-cmd")
    {
        return Some((RISK_B, "防火墙变更", "firewall change", "shield-alert"));
    }
    if starts("kill -9") || word_at(&lower, "pkill") || word_at(&lower, "killall") {
        return Some((RISK_B, "强制终止进程", "kill process", "remove"));
    }
    if starts("history -c") || starts("crontab -r") {
        return Some((RISK_B, "痕迹清理", "trace cleanup", "log-file"));
    }
    if starts("git ") && lower.contains("reset") && lower.contains("--hard") {
        return Some((RISK_B, "丢弃本地改动", "discard changes", "restore"));
    }
    if starts("git ") && lower.contains("clean") && lower.contains("clean -f") {
        return Some((RISK_B, "丢弃本地改动", "discard changes", "restore"));
    }
    if (starts("apt ") || starts("apt-get ") || starts("yum ") || starts("dnf "))
        && (lower.contains(" remove") || lower.contains(" purge"))
    {
        return Some((RISK_B, "卸载软件包", "package removal", "delete"));
    }
    None
}

/// 行级「管道执行远端脚本」：`curl -fsSL url | sh` 这类跨段组合 ——
/// 段切分会把 curl 与 sh 拆成两段各自无害，必须在**行级**判
/// （剥引号后的行含 curl/wget 段，且某段管道后的首个词是 shell）。
fn line_pipe_to_shell(line_stripped: &str) -> bool {
    let mut has_fetcher = false;
    for seg in line_stripped.split('|') {
        let seg = seg.trim_start();
        let lower = seg.to_lowercase();
        if word_at(&lower, "curl") || word_at(&lower, "wget") {
            has_fetcher = true;
        }
        // 管道后的接收端是 shell 才算（`| grep sh` 不算）
        if has_fetcher
            && (word_at(&lower, "sh")
                || word_at(&lower, "bash")
                || word_at(&lower, "zsh")
                || word_at(&lower, "dash")
                || word_at(&lower, "fish")
                || word_at(&lower, "ksh"))
        {
            return true;
        }
    }
    false
}

/// 单行风险判定：行级管道 / fork 炸弹优先（两者都会被 `|` 段切分切碎，
/// 必须在切段前整行判），然后逐段（;、\n、&&、||、| 切段，剥引号与前缀后
/// 匹配规则表）。一行只出一条徽标（取首个命中）。
fn line_risk(raw_line: &str) -> Option<PasteRiskHit> {
    let stripped = strip_quoted(raw_line);
    let hit = |level: i32, zh: &'static str, en: &'static str, icon: &'static str| {
        Some(PasteRiskHit { line: 0, level, category_zh: zh, category_en: en, icon })
    };
    if line_pipe_to_shell(&stripped) {
        return hit(RISK_B, "管道执行远端脚本", "pipe to shell", "download");
    }
    // fork 炸弹 `:(){ :|:& };:` —— 段切分会把 `|:&` 切碎，行级整行判
    if stripped.starts_with(':') && stripped.contains("(){") && stripped.contains("|:&") {
        return hit(RISK_A, "fork 炸弹", "fork bomb", "error");
    }
    // 数据库破坏的 SQL 字面量通常写在引号里（psql -c 'DROP DATABASE …'），
    // 引号剥离会把它剥掉 —— 在未剥引号的原始行上补判（词组误报率极低）。
    let raw_lower = raw_line.to_lowercase();
    if raw_lower.contains("drop database") || raw_lower.contains("drop table") {
        return hit(RISK_A, "数据库破坏", "database drop", "error");
    }
    let mut rest = stripped.as_str();
    while !rest.is_empty() {
        match find_sep(rest) {
            Some((pos, len)) => {
                if let Some((level, zh, en, icon)) = paste_seg_risk(&rest[..pos]) {
                    return Some(PasteRiskHit { line: 0, level, category_zh: zh, category_en: en, icon });
                }
                rest = &rest[pos + len..];
            }
            None => {
                return paste_seg_risk(rest).map(
                    |(level, zh, en, icon)| PasteRiskHit { line: 0, level, category_zh: zh, category_en: en, icon },
                );
            }
        }
    }
    None
}

/// 预览展示行的净化：剥掉全部控制字节（`encode_pasted_text` 在 bracketed
/// 模式会剥 \x1b/\x03，其余控制字节同样不该进预览），`\t` 展开 4 空格。
fn display_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '\t' => out.push_str("    "),
            '\u{7f}' => {}
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// spec §5.1 三档判定。`line_count` 为显示行数（`lines()` 语义）。
pub fn paste_tier(line_count: usize, char_count: usize) -> PasteTier {
    if line_count > TIER_LINE_LIMIT || char_count > TIER_CHAR_LIMIT {
        PasteTier::Review
    } else if line_count > COMPACT_MAX_LINES {
        PasteTier::Standard
    } else {
        PasteTier::Compact
    }
}

/// 折叠窗口计算（无折叠时返回 None —— 窗口覆盖全部行）。
pub fn fold_window(line_count: usize, risks: &[PasteRiskHit]) -> Option<FoldWindow> {
    if line_count == 0 {
        return None;
    }
    let last = line_count - 1;
    let (start, end) = if let Some(first) = risks.iter().map(|r| r.line).min() {
        let last_risk = risks.iter().map(|r| r.line).max().unwrap_or(first);
        (first, (last_risk + 2).min(last))
    } else {
        (1usize.min(last), 7usize.min(last))
    };
    if start > end {
        return None;
    }
    let head_hidden = start.saturating_sub(1);
    let tail_hidden = last.saturating_sub(end);
    if head_hidden == 0 && tail_hidden == 0 {
        return None;
    }
    Some(FoldWindow { head_hidden, start, end, tail_hidden })
}

/// 折叠账硬断言的输入：被折叠总行数（0 = 无折叠）。
pub fn fold_hidden_count(line_count: usize, risks: &[PasteRiskHit]) -> usize {
    fold_window(line_count, risks).map_or(0, |w| w.hidden_total())
}

/// 预览行数据（Slint `PastePreviewRow` 的 Rust 镜像）。
#[derive(Debug, Clone, PartialEq)]
pub struct PastePreviewRowData {
    /// 0 = 普通行，1 = 头折叠行，2 = 尾折叠行，3 = 虚拟化通告行。
    pub kind: i32,
    /// 行号（折叠行 = "…"，通告行 = "—"，紧凑档 = ""）。
    pub gutter: String,
    pub text: String,
    /// 0 = 无，1 = A 级，2 = B 级。
    pub level: i32,
    /// 行尾徽标类别名（空 = 无徽标）。
    pub badge: String,
    pub badge_icon: String,
}

/// 预览行模型 + 「展开全部」按钮的折叠数（0 = 不出按钮）。
#[derive(Debug, Clone)]
pub struct PastePreviewModel {
    pub rows: Vec<PastePreviewRowData>,
    pub fold_count: usize,
}

/// 构建预览行模型。`expanded` = 用户已点「展开全部」。
pub fn build_preview_rows(a: &PasteAnalysis, expanded: bool) -> PastePreviewModel {
    let line_row = |idx: usize, text: &str| PastePreviewRowData {
        kind: 0,
        gutter: if a.tier == PasteTier::Compact {
            String::new()
        } else {
            (idx + 1).to_string()
        },
        text: text.to_string(),
        level: 0,
        badge: String::new(),
        badge_icon: String::new(),
    };
    let risk_row = |idx: usize, text: &str, hit: &PasteRiskHit| PastePreviewRowData {
        level: hit.level,
        badge: crate::i18n::t(hit.category_zh, hit.category_en).to_string(),
        badge_icon: hit.icon.to_string(),
        ..line_row(idx, text)
    };
    let mut rows: Vec<PastePreviewRowData> = Vec::new();

    // 虚拟化（摘要）模式：通告行 + 前 12 行样本 + 尾折叠行（高保真 F 帧）。
    if a.virtualized && !expanded {
        rows.push(PastePreviewRowData {
            kind: 3,
            gutter: "—".into(),
            text: crate::i18n::t(
                "虚拟化通告：本段内容较大，此处只渲染部分样本行；点「展开全部」后其余行按可视窗口按需渲染。",
                "Virtualization notice: only a sample of this large paste is rendered; the rest streams in on demand after “Expand all”.",
            )
            .to_string(),
            level: 0,
            badge: String::new(),
            badge_icon: String::new(),
        });
        let sample = a.line_count.min(VIRTUAL_SAMPLE_LINES);
        for idx in 0..sample {
            let hit = a.risks.iter().find(|r| r.line == idx);
            let text = &a.lines[idx];
            rows.push(match hit {
                Some(h) => risk_row(idx, text, h),
                None => line_row(idx, text),
            });
        }
        let hidden = a.line_count - sample;
        if hidden > 0 {
            rows.push(fold_row(2, hidden));
        }
        return PastePreviewModel { rows, fold_count: hidden };
    }

    if a.tier == PasteTier::Compact {
        for (idx, text) in a.lines.iter().enumerate() {
            let hit = a.risks.iter().find(|r| r.line == idx);
            rows.push(match hit {
                Some(h) => risk_row(idx, text, h),
                None => line_row(idx, text),
            });
        }
        return PastePreviewModel { rows, fold_count: 0 };
    }

    let Some(win) = (if expanded { None } else { fold_window(a.line_count, &a.risks) }) else {
        for (idx, text) in a.lines.iter().enumerate() {
            let hit = a.risks.iter().find(|r| r.line == idx);
            rows.push(match hit {
                Some(h) => risk_row(idx, text, h),
                None => line_row(idx, text),
            });
        }
        return PastePreviewModel { rows, fold_count: 0 };
    };
    // 行 1 恒显（头部入口；start == 0 时窗口已含它）
    if win.start > 0 {
        let hit = a.risks.iter().find(|r| r.line == 0);
        let text = &a.lines[0];
        rows.push(match hit {
            Some(h) => risk_row(0, text, h),
            None => line_row(0, text),
        });
    }
    if win.head_hidden > 0 {
        rows.push(fold_row(1, win.head_hidden));
    }
    for idx in win.start..=win.end {
        let hit = a.risks.iter().find(|r| r.line == idx);
        let text = &a.lines[idx];
        rows.push(match hit {
            Some(h) => risk_row(idx, text, h),
            None => line_row(idx, text),
        });
    }
    if win.tail_hidden > 0 {
        rows.push(fold_row(2, win.tail_hidden));
    }
    PastePreviewModel { rows, fold_count: win.hidden_total() }
}

fn fold_row(kind: i32, n: usize) -> PastePreviewRowData {
    let text = if kind == 1 {
        crate::i18n::t("… 以上另有 %1 行（已折叠）", "… %1 more line(s) above (folded)")
            .replace("%1", &n.to_string())
    } else {
        crate::i18n::t("… 以下另有 %1 行（已折叠）", "… %1 more line(s) below (folded)")
            .replace("%1", &n.to_string())
    };
    PastePreviewRowData {
        kind,
        gutter: "…".into(),
        text,
        level: 0,
        badge: String::new(),
        badge_icon: String::new(),
    }
}

/// 「展开全部（另有 N 行）」按钮文案（N 由调用方填，0 = 按钮不出现）。
pub fn expand_all_label(n: usize) -> String {
    crate::i18n::t("展开全部（另有 %1 行）", "Expand all (%1 more)")
        .replace("%1", &n.to_string())
}

/// 分析一段剪贴板文本。行数为 `lines()` 语义；风险检测用原始行
/// （控制字符不参与规则匹配 —— 剥引号口径同样剥控制字符会破坏占位对齐）。
pub fn analyze_paste(text: &str) -> PasteAnalysis {
    let display: Vec<String> = display_lines(text);
    let line_count = display.len();
    let char_count = text.chars().count();
    let trailing_newline = text.ends_with(['\r', '\n']);
    let mut risks = Vec::new();
    for (idx, raw) in raw_lines(text).iter().enumerate() {
        if let Some(mut hit) = line_risk(raw) {
            hit.line = idx;
            risks.push(hit);
        }
    }
    let a_count = risks.iter().filter(|r| r.level == RISK_A).count();
    let b_count = risks.iter().filter(|r| r.level == RISK_B).count();
    let esc_count = text.matches('\u{1b}').count();
    let ctrlc_count = text.matches('\u{3}').count();
    let force_review = text.len() > FORCE_REVIEW_BYTES;
    let virtualized = line_count > VIRTUAL_LINE_LIMIT || force_review;
    let tier = paste_tier(line_count, char_count);
    PasteAnalysis {
        lines: display,
        line_count,
        char_count,
        trailing_newline,
        a_count,
        b_count,
        esc_count,
        ctrlc_count,
        risks,
        force_review,
        virtualized,
        tier,
    }
}

/// 原始行（保留控制字符，供风险检测；`lines()` 语义对 \r\n/\r/\n 一视同仁）。
fn raw_lines(text: &str) -> Vec<String> {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<String> = unified.split('\n').map(str::to_string).collect();
    if text.ends_with('\n') || text.ends_with('\r') {
        out.pop();
    }
    out
}

/// 显示行 = 原始行过 display_line 净化。
fn display_lines(text: &str) -> Vec<String> {
    raw_lines(text).iter().map(|l| display_line(l)).collect()
}

// ── 页脚/头部组合文案（spec §5.5 + 高保真 7 帧对照）────────────────────────

/// 副标题（dlg-head .st）。
pub fn subtitle_text(a: &PasteAnalysis) -> String {
    let lines = a.line_count.to_string();
    let chars = a.char_count.to_string();
    match a.tier {
        PasteTier::Compact => {
            if !a.trailing_newline {
                return crate::i18n::t(
                    "%1 行文本将粘贴到当前光标位置。",
                    "%1 line(s) will be pasted at the cursor position.",
                )
                .replace("%1", &lines);
            }
            if a.a_count > 0 {
                let cat = a
                    .risks
                    .iter()
                    .find(|r| r.level == RISK_A)
                    .map(|r| crate::i18n::t(r.category_zh, r.category_en).to_string())
                    .unwrap_or_default();
                return crate::i18n::t(
                    "%1 行 / %2 字符，末尾有回车；命中 A 级「%3」。",
                    "%1 line(s) / %2 chars, trailing return; grade-A “%3” hit.",
                )
                .replace("%1", &lines)
                .replace("%2", &chars)
                .replace("%3", &cat);
            }
            crate::i18n::t(
                "%1 行 / %2 字符，末尾有回车。",
                "%1 line(s) / %2 chars, trailing return.",
            )
            .replace("%1", &lines)
            .replace("%2", &chars)
        }
        PasteTier::Standard => {
            let base = if a.risks.is_empty() {
                crate::i18n::t(
                    "%1 行文本，无风险命令；",
                    "%1 line(s), no flagged command; ",
                )
                .replace("%1", &lines)
                + &crate::i18n::t(
                    "远端未启用 bracketed paste 时多行会按逐行送达。",
                    "without bracketed paste, lines arrive one by one.",
                )
            } else if a.trailing_newline {
                crate::i18n::t(
                    "本段含 %1 处危险命令（A 级 %2 · B 级 %3），且末尾回车会立即执行。",
                    "Contains %1 flagged command(s) (A×%2, B×%3); the trailing return executes immediately.",
                )
                .replace("%1", &(a.a_count + a.b_count).to_string())
                .replace("%2", &a.a_count.to_string())
                .replace("%3", &a.b_count.to_string())
            } else {
                crate::i18n::t(
                    "本段含 %1 处危险命令（A 级 %2 · B 级 %3）。",
                    "Contains %1 flagged command(s) (A×%2, B×%3).",
                )
                .replace("%1", &(a.a_count + a.b_count).to_string())
                .replace("%2", &a.a_count.to_string())
                .replace("%3", &a.b_count.to_string())
            };
            base
        }
        PasteTier::Review => {
            if a.force_review {
                crate::i18n::t(
                    "%1 行 / %2 字符（约 %3 KB）——超过 100 KB，已强制进入评审档。",
                    "%1 line(s) / %2 chars (~%3 KB) — over 100 KB, forced into review.",
                )
                .replace("%1", &lines)
                .replace("%2", &chars)
                .replace("%3", &((text_kb(a)).to_string()))
            } else if a.esc_count + a.ctrlc_count > 0 {
                crate::i18n::t(
                    "%1 行 / %2 字符；bracketed 已启用，%3 个控制字符将被剥离。",
                    "%1 line(s) / %2 chars; bracketed is on, %3 control byte(s) will be stripped.",
                )
                .replace("%1", &lines)
                .replace("%2", &chars)
                .replace("%3", &(a.esc_count + a.ctrlc_count).to_string())
            } else if a.risks.is_empty() {
                crate::i18n::t(
                    "%1 行 / %2 字符。已进入评审档：预览区可滚动。",
                    "%1 line(s) / %2 chars. Review tier: the preview scrolls.",
                )
                .replace("%1", &lines)
                .replace("%2", &chars)
            } else {
                crate::i18n::t(
                    "%1 行 / %2 字符，含 %3 处危险命令（A 级 %4 · B 级 %5）。已进入评审档：预览区可滚动。",
                    "%1 line(s) / %2 chars, %3 flagged (A×%4, B×%5). Review tier: the preview scrolls.",
                )
                .replace("%1", &lines)
                .replace("%2", &chars)
                .replace("%3", &(a.a_count + a.b_count).to_string())
                .replace("%4", &a.a_count.to_string())
                .replace("%5", &a.b_count.to_string())
            }
        }
    }
}

fn text_kb(a: &PasteAnalysis) -> usize {
    (a.char_count.max(1) / 1024).max(1)
}

/// 提示条类别：0 无 / 1 dg·force_review / 2 warn·bracketed 关 /
/// 3 warn·控制字符 / 4 dg·尾回车（无 A）/ 5 dg·尾回车 + A 级。
/// 优先级按高保真 7 帧反推定稿：force > bracketed 关 > 控制字符 > 尾回车。
pub fn notice_kind(a: &PasteAnalysis, bracketed: bool) -> i32 {
    if a.force_review {
        return 1;
    }
    if !bracketed {
        return 2;
    }
    if a.esc_count + a.ctrlc_count > 0 {
        return 3;
    }
    if a.trailing_newline {
        return if a.a_count > 0 { 5 } else { 4 };
    }
    0
}

/// 提示条文本（kind 来自 notice_kind）。
pub fn notice_text(a: &PasteAnalysis, kind: i32) -> String {
    match kind {
        1 => crate::i18n::t(
            "内容约 %1 KB，跨过强制复核阈值（100 KB）：即使关闭「不再提示多行粘贴」也必须复核。此段含 %2 处 A 级不可逆破坏。",
            "About %1 KB — past the force-review threshold (100 KB): this prompt appears even when “don't ask again” is on. Contains %2 grade-A irreversible hit(s).",
        )
        .replace("%1", &text_kb(a).to_string())
        .replace("%2", &a.a_count.to_string()),
        2 => crate::i18n::t(
            "远端未开启 bracketed paste：整段多行将逐行执行。注意：bracketed 开启本身也不等于安全（远端可自行开启该模式），因此两种状态都要确认。",
            "The remote has not enabled bracketed paste: the block runs line by line. Bracketed mode is not a safety guarantee either — the remote can turn it on itself — so both states confirm.",
        )
        .to_string(),
        3 => crate::i18n::t(
            "控制字符：原文含 %1 个 ESC (0x1b) 与 %2 个 Ctrl-C (0x03)。bracketed 开启时这些字节将先被剥离再进入终端。",
            "Control bytes: %1× ESC (0x1b) and %2× Ctrl-C (0x03). With bracketed on, they are stripped before reaching the terminal.",
        )
        .replace("%1", &a.esc_count.to_string())
        .replace("%2", &a.ctrlc_count.to_string()),
        4 => crate::i18n::t(
            "末尾含回车：主按钮会剥离这个回车，最后一行留在命令行不执行；「粘贴并执行」等价原样粘贴的行为。",
            "Trailing return: the primary strips it, leaving the last line unexecuted at the prompt; “Paste and run” pastes the block as-is.",
        )
        .to_string(),
        5 => crate::i18n::t(
            "末尾回车会立刻执行；默认动作已固定为剥离末尾回车，最后一行留在命令行等你按回车；要立即执行请显式点「粘贴并执行」。",
            "A trailing return would run instantly. The default is pinned to stripping it, leaving the line at the prompt; use “Paste and run” to execute deliberately.",
        )
        .to_string(),
        _ => String::new(),
    }
}

/// 主按钮文案（D1 三态 → #paste-editable 2026-11-13 收敛单态）：文案去
/// 说明后缀，详情已在页脚 enter-note 一行 —— 原样并入按钮会把按钮组撑到
/// 450px+，Enter 说明被压成窄条（出图实证）。
pub fn main_button_label(_a: &PasteAnalysis) -> String {
    crate::i18n::t("粘贴", "Paste").to_string()
}
pub fn enter_note_text(a: &PasteAnalysis) -> String {
    if !a.trailing_newline {
        crate::i18n::t(
            "Enter = 主按钮「粘贴」（安全动作，不回显换行、不执行）",
            "Enter = primary “Paste” (safe, never runs)",
        )
        .to_string()
    } else if a.a_count > 0 {
        crate::i18n::t(
            "Enter = 主按钮「粘贴（已去掉末尾回车）」（安全动作，绝不执行；「粘贴并执行」须显式点击或 Alt+P）",
            "Enter = primary “Paste (return removed)” (safe, never runs; “Paste and run” needs an explicit click or Alt+P)",
        )
        .to_string()
    } else {
        crate::i18n::t(
            "Enter = 主按钮「粘贴（不含末尾回车）」（安全动作，绝不执行）",
            "Enter = primary “Paste without trailing newline” (safe, never runs)",
        )
        .to_string()
    }
}

/// 预览底部 hint 条。
pub fn hint_text(a: &PasteAnalysis, virtualized: bool) -> String {
    if virtualized {
        return crate::i18n::t(
            "展开全部后列表按可视窗口渲染，避免一次布局全部行。",
            "After expanding, rows render through a virtual window.",
        )
        .to_string();
    }
    match a.tier {
        PasteTier::Compact => crate::i18n::t(
            "紧凑档：1–3 行不画行号列，行数见上方统计条。",
            "Compact tier shows no gutter; the real count is in the stats bar.",
        )
        .to_string(),
        PasteTier::Standard => {
            if a.risks.is_empty() {
                crate::i18n::t(
                    "内容将逐行粘贴到当前光标位置。",
                    "The content is pasted at the cursor position.",
                )
                .to_string()
            } else {
                crate::i18n::t(
                    "危险行 = 左侧色条 + 行尾类别徽标；仅提示，不会阻止粘贴。",
                    "Flagged rows carry a colour bar + a category badge; advisory only, pasting is never blocked.",
                )
                .to_string()
            }
        }
        PasteTier::Review => crate::i18n::t(
            "评审档：预览区独立滚动。",
            "Review tier: the preview scrolls independently.",
        )
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_boundaries() {
        // 紧凑 1–3 行且 ≤600 字符
        assert_eq!(analyze_paste("a\nb\nc").tier, PasteTier::Compact);
        assert_eq!(analyze_paste("a\nb\nc\nd").tier, PasteTier::Standard);
        // >12 行或 >600 字符任一超出 → 评审
        assert_eq!(analyze_paste("x\n".repeat(13).as_str()).tier, PasteTier::Review);
        assert_eq!(analyze_paste("x".repeat(601).as_str()).tier, PasteTier::Review);
        // 12 行 + 尾换行仍是标准档（高保真 B 帧 12 行 / 364 字符口径）
        let b = "a\n".repeat(11) + "b\n";
        assert_eq!(analyze_paste(&b).tier, PasteTier::Standard);
        assert_eq!(analyze_paste(&b).line_count, 12);
        // 单行 + 尾换行（高保真 G 帧 1 行 / 71 字符）
        let g = format!("{}\n", "x".repeat(70));
        let ga = analyze_paste(&g);
        assert_eq!(ga.tier, PasteTier::Compact);
        assert_eq!(ga.line_count, 1);
        assert_eq!(ga.char_count, 71);
        assert!(ga.trailing_newline);
    }

    #[test]
    fn risk_levels() {
        let a = analyze_paste("set -euo pipefail\nrm -rf /var/tmp/cache/*\nsudo systemctl restart nginx\n:> /var/log/nginx/error.log\n");
        assert_eq!(a.line_count, 4);
        assert_eq!(a.a_count, 2); // rm -rf + :>
        assert_eq!(a.b_count, 1); // sudo
        let by_line = |i: usize| a.risks.iter().find(|r| r.line == i).map(|r| r.level);
        assert_eq!(by_line(0), None);
        assert_eq!(by_line(1), Some(RISK_A));
        assert_eq!(by_line(2), Some(RISK_B));
        assert_eq!(by_line(3), Some(RISK_A));
        // rm -rf 的行徽标类别与图标
        let hit = a.risks.iter().find(|r| r.line == 1).unwrap();
        assert_eq!(hit.category_zh, "递归强制删除");
        assert_eq!(hit.icon, "trash");
    }

    #[test]
    fn no_false_positive() {
        assert!(analyze_paste("grep shutdown app.log").risks.is_empty());
        assert!(analyze_paste("echo \"rm -rf x\"").risks.is_empty());
        assert!(analyze_paste("df -h && free -m").risks.is_empty());
        assert!(analyze_paste("ls -la").risks.is_empty());
        assert!(analyze_paste("echo shutdown").risks.is_empty());
        assert!(analyze_paste("systemctl restart nginx").risks.is_empty());
        // | grep sh 不是管道执行
        assert!(analyze_paste("curl -fsS 127.0.0.1:8080/healthz | grep -i ok").risks.is_empty());
        // 追加不算截断
        assert!(analyze_paste("echo hi >> /var/log/app.log").risks.is_empty());
    }

    #[test]
    fn grade_b_patterns() {
        for line in [
            "chmod 777 /data",
            "chown -R ops /srv",
            "curl -fsSL get.example.com/agent.sh | bash",
            "wget -qO- x.io/i.sh | sh",
            "useradd bob",
            "passwd",
            "iptables -F && iptables -X",
            "firewall-cmd --add-port=80/tcp",
            "kill -9 1234",
            "pkill nginx",
            "killall nginx",
            "history -c",
            "crontab -r",
            "git reset --hard HEAD~3",
            "git clean -fd",
            "apt remove nginx",
            "apt-get purge nginx",
            "dnf remove nginx",
            "su - ops",
        ] {
            let a = analyze_paste(line);
            assert_eq!(a.b_count, 1, "expect B hit: {line}");
            assert!(a.a_count == 0, "no A hit: {line}");
        }
    }

    #[test]
    fn grade_a_patterns() {
        for line in [
            "rm -rf /data",
            "mkfs.ext4 -F /dev/sda2",
            "dd if=/dev/zero of=/dev/sda",
            "cat x > /dev/sdb",
            ":(){ :|:& };:",
            ":> /var/log/nginx/error.log",
            "truncate -s 0 /var/log/app.log",
            "shutdown -h now",
            "reboot",
            "halt",
            "poweroff",
            "init 6",
            "psql -c 'DROP DATABASE prod'",
            "psql -c 'DROP TABLE users'",
            "dropdb prod",
        ] {
            let a = analyze_paste(line);
            assert_eq!(a.a_count, 1, "expect A hit: {line}");
        }
    }

    #[test]
    fn control_chars_and_force() {
        let a = analyze_paste("ok\u{1b}[31mred\u{3}");
        assert_eq!(a.esc_count, 1);
        assert_eq!(a.ctrlc_count, 1);
        // 显示行剥掉控制字节
        assert_eq!(a.lines[0], "ok[31mred");
        // >100 KB 强制复核 + 虚拟化
        let big = "x".repeat(100 * 1024 + 1);
        let a = analyze_paste(&big);
        assert!(a.force_review);
        assert!(a.virtualized);
        assert_eq!(a.tier, PasteTier::Review);
    }

    #[test]
    fn fold_account_balances() {
        // 高保真 B 帧：12 行、风险在 1-based 4/5/8 → 头折 2 + 尾折 2 = 4
        let lines: Vec<&str> = vec![
            "set -euo pipefail",
            "export DEBIAN_FRONTEND=noninteractive",
            "cd /var/tmp/cache",
            "rm -rf /var/tmp/cache/*",
            "sudo systemctl restart nginx",
            "sleep 2 && curl -fsS 127.0.0.1:8080/healthz",
            "echo done",
            ":> /var/log/nginx/error.log",
            "tail -n 20 /var/log/nginx/access.log",
            "ssh ops@10.0.0.7 'systemctl status nginx'",
            "echo done",
            "pg_isready -h 127.0.0.1 -p 5432",
        ];
        let text = lines.join("\n");
        let a = analyze_paste(&text);
        let win = fold_window(a.line_count, &a.risks).unwrap();
        assert_eq!(win.head_hidden, 2);
        assert_eq!(win.tail_hidden, 2);
        assert_eq!(win.hidden_total(), 4);
        // 窗口 = 1-based 4..=10
        assert_eq!(win.start, 3);
        assert_eq!(win.end, 9);
        // 模型行数 = 1 + 窗口 + 1 = 9（12 − 折 4 + 折 2 提示行）
        let m = build_preview_rows(&a, false);
        assert_eq!(m.rows.len(), a.line_count - 4 + 2);
        assert_eq!(m.fold_count, 4);
        // 头折叠行 / 尾折叠行各一条
        assert_eq!(m.rows.iter().filter(|r| r.kind == 1).count(), 1);
        assert_eq!(m.rows.iter().filter(|r| r.kind == 2).count(), 1);
        // 展开后全量且无折叠
        let m = build_preview_rows(&a, true);
        assert_eq!(m.rows.len(), 12);
        assert_eq!(m.fold_count, 0);
        // 无风险短内容不折叠
        let small = analyze_paste("a\nb\nc\nd");
        assert!(fold_window(small.line_count, &small.risks).is_none());
    }

    #[test]
    fn virtualized_sample() {
        let text: String = (0..300).map(|i| format!("line {i}\n")).collect();
        let a = analyze_paste(&text);
        assert!(a.virtualized);
        let m = build_preview_rows(&a, false);
        // 通告行 + 12 样本 + 尾折叠行
        assert_eq!(m.rows.len(), 1 + 12 + 1);
        assert_eq!(m.rows[0].kind, 3);
        assert_eq!(m.fold_count, 300 - 12);
        let m = build_preview_rows(&a, true);
        assert_eq!(m.rows.len(), 300);
        assert_eq!(m.fold_count, 0);
    }

    #[test]
    fn button_and_notice_texts() {
        // (#paste-editable 2026-11-13) 主钮文案收敛单态「粘贴」（说明已在
        // 页脚 enter-note），三态断言随之退役；notice 优先级不变。
        let a = analyze_paste("ls -la");
        assert_eq!(main_button_label(&a), crate::i18n::t("粘贴", "Paste"));
        let a = analyze_paste("ls -la\n");
        assert_eq!(main_button_label(&a), crate::i18n::t("粘贴", "Paste"));
        // notice 优先级：force > bracketed 关 > 控制字符 > 尾回车
        let a = analyze_paste(&"x".repeat(100 * 1024 + 1));
        assert_eq!(notice_kind(&a, true), 1);
        let a = analyze_paste("ls\n");
        assert_eq!(notice_kind(&a, false), 2);
        let a = analyze_paste("ls\n\u{1b}");
        assert_eq!(notice_kind(&a, true), 3);
        let a = analyze_paste("ls\n");
        assert_eq!(notice_kind(&a, true), 4);
        let a = analyze_paste("rm -rf /data\n");
        assert_eq!(notice_kind(&a, true), 5);
        let a = analyze_paste("ls");
        assert_eq!(notice_kind(&a, true), 0);
    }
}
