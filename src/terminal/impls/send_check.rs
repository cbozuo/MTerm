//! (#send-cmd-p1) 发送命令窗口的文字检测:危险词命中 + 续行风险。
//!
//! 这是设计稿 send-command-hifi-2026-10-27 §04④/§10① 的「单一出处」:
//! 以前危险词在 send_cmd_window.slint:106-120 和 terminal_view.slint 各抄一份,
//! 现在收进 Rust,发送窗经回调拿结果。命令栏那份 Slint 词表后续同一来源迁移。
//!
//! ⚠️ 放 Rust 而不是 Slint 的硬原因:续行风险要逐字符扫引号配对、逐行数
//! 块开合 —— Slint 的 string 没有逐字符访问,做不了(§16 定稿注记)。

/// 危险词表:按「段首」匹配(段 = 按 `;`、换行、`&&`、`||`、`|` 切开)。
/// 匹配前先剥掉引号内容(`echo "rm -rf x"` 不算危险)和
/// sudo/env/time/nohup/exec/\ 等段首前缀。
const DANGER_RULES: &[(&str, &str)] = &[
    // 正则不好上(不引 regex 依赖),逐条写成判别函数:
    // ("展示名", 前缀词匹配器)
];

/// 判别某「段」(已剥引号与前缀)是否命中危险词,返回展示名。
fn seg_danger(seg: &str) -> Option<&'static str> {
    let s = seg.trim_start();
    let lower = s.to_lowercase();
    let starts = |p: &str| lower.starts_with(p);
    let word = |w: &str| {
        lower.starts_with(w)
            && lower[w.len()..]
                .chars()
                .next()
                .map_or(true, |c| !c.is_ascii_alphanumeric() && c != '_')
    };
    // rm 的 -r(f) 组合:rm -rf / rm -fr / rm -r -f / rm -f -r / rm -Rf …
    let rm_args = s.split_whitespace().skip(1).take(3);
    let mut rm_r = false;
    let mut rm_f = false;
    let mut saw_arg = false;
    for a in rm_args {
        saw_arg = true;
        let la = a.to_lowercase();
        if la.starts_with('-') && !la.starts_with("--") {
            if la.contains('r') { rm_r = true; }
            if la.contains('f') { rm_f = true; }
        } else {
            break; // 到目标路径了,参数段结束
        }
    }
    if saw_arg && rm_r && rm_f {
        return Some("rm -rf");
    }
    if starts("mkfs") { return Some("mkfs"); }
    if starts("dd if=") || starts("dd  if=") { return Some("dd if="); }
    if word("shutdown") || word("reboot") || word("poweroff") || word("halt") {
        return Some("shutdown");
    }
    if starts("init 0") { return Some("init 0"); }
    // chmod -R …777(数字串里含 7 连三)
    if starts("chmod ") {
        let toks: Vec<&str> = s.split_whitespace().collect();
        if toks.len() >= 3 && (toks[1].eq_ignore_ascii_case("-r") || toks[1].eq_ignore_ascii_case("-r")) {
            if let Some(mode) = toks.get(2) {
                if mode.len() >= 3 && mode.ends_with("777") {
                    return Some("chmod -R 777");
                }
            }
        }
    }
    // fork bomb :(){ :|:& };:
    if s.starts_with(':') && s.contains("(){") && s.contains("|:&") { return Some("fork bomb"); }
    // 写裸设备:dd of=/dev/sdX 也算(hide 的 > /dev/sdX 由重定向段匹配)
    if lower.contains("> /dev/sd") || lower.contains(">/dev/sd") { return Some("> /dev/sdX"); }
    if starts("iptables -f") { return Some("iptables -F"); }
    if starts("kill -9 1") || starts("kill -9 -1") { return Some("kill -9 1"); }
    None
}

/// 剥掉引号内容(单引号对保留占位 `''`,双引号对保留 `""`)。
/// 只剥「同一行内配对」的引号 —— 跨行引号由续行风险单独检测。
/// (#multiline-paste-hifi 2026-11-10) 放宽 pub(crate):粘贴弹框的分析层
/// (paste_review)复用同一套防误报口径,词表仍各自独立。
pub(crate) fn strip_quoted(line: &str) -> String {
    let bytes: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '\'' || c == '"' {
            let close = c;
            let mut j = i + 1;
            let mut closed = false;
            while j < bytes.len() {
                if bytes[j] == '\\' { j += 2; continue; }
                if bytes[j] == close { closed = true; break; }
                j += 1;
            }
            if closed {
                out.push(close);
                out.push(close);
                i = j + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// 剥掉段首前缀(sudo/env/time/nohup/exec/\)。
pub(crate) fn strip_prefix(mut s: &str) -> &str {
    loop {
        let t = s.trim_start();
        let changed = t.len() != s.len();
        s = t;
        let cut = ["sudo", "env", "time", "nohup", "exec", "\\"]
            .iter()
            .find_map(|p| {
                if s.starts_with(p) {
                    let rest = &s[p.len()..];
                    // "sudox" 不算 sudo 前缀:后一个字符必须是空白
                    if rest.starts_with(char::is_whitespace) { Some(rest) } else { None }
                } else {
                    None
                }
            });
        match cut {
            Some(rest) => s = rest,
            None => {
                if !changed { return s; }
            }
        }
    }
}

/// 危险词命中,返回展示名(如 "rm -rf");未命中返回空串。
/// (#send-cmd-p1) 检测口径 = 设计稿 §04④:先剥引号内容再按段首匹配,
/// `grep shutdown app.log` 不误报,`rm -rf /data` 会命中。
pub fn danger_token(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let stripped = strip_quoted(text);
    // 按 ;、换行、&&、||、| 切段(&&/|| 先于单字符,保序)
    let mut segs: Vec<&str> = Vec::new();
    let mut rest = stripped.as_str();
    while !rest.is_empty() {
        match find_sep(rest) {
            Some((pos, len)) => {
                segs.push(&rest[..pos]);
                rest = &rest[pos + len..];
            }
            None => {
                segs.push(rest);
                break;
            }
        }
    }
    for seg in segs {
        if let Some(name) = seg_danger(strip_prefix(seg)) {
            return name.to_string();
        }
    }
    String::new()
}

/// 危险行集合:0-based 行号,按行剥引号后切段判 seg_danger。
/// (#send-cmd-v2 2026-11-07) 行号槽三角 + 危险行背景条的数据源;与
/// danger_token 同一套词表/前缀规则(单一出处),只是粒度到行 —— 行号槽
/// 需要「哪一行危险」,整体 token 只说「第一个危险词是什么」。
pub fn danger_lines(text: &str) -> Vec<usize> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        // 逐行剥「行内配对」的引号(echo "rm -rf x" 不误报),再按分隔符切段
        let stripped = strip_quoted(line);
        let mut hit = false;
        let mut rest = stripped.as_str();
        while !rest.is_empty() {
            match find_sep(rest) {
                Some((pos, len)) => {
                    if seg_danger(strip_prefix(&rest[..pos])).is_some() {
                        hit = true;
                        break;
                    }
                    rest = &rest[pos + len..];
                }
                None => {
                    if seg_danger(strip_prefix(rest)).is_some() {
                        hit = true;
                    }
                    break;
                }
            }
        }
        if hit {
            out.push(i);
        }
    }
    out
}

/// 找下一个命令分隔符,返回 (字节位置, 长度)。&& / || 优先于单字符。
pub(crate) fn find_sep(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b';' | b'\n' | b'|' | b'&' => {
                if i + 1 < b.len() && b[i + 1] == b[i] && (b[i] == b'&' || b[i] == b'|') {
                    return Some((i, 2));
                }
                return Some((i, 1));
            }
            _ => i += 1,
        }
    }
    None
}

/// 续行风险:这段文本按「逐行回车」写进 PTY 会不会卡在续行提示符。
/// 返回风险说明(如 "引号未闭合"),无风险返回空串。
/// 口径 = 设计稿 §10①:**完整的 for…do…done 不算风险**(闭合的,逐行敲本来就能用),
/// 只有「没写完」才需要兜;引号按整段逐字符配对(跨行引号合法),
/// 块结构逐块开合计数(两个块并存时,一个闭合 + 一个没写完也要报)。
pub fn continuation_risk(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }

    // ① 引号:整段状态机,\" 转义跳过
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => { chars.next(); }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ => {}
        }
    }
    if in_single { return "单引号未闭合".into(); }
    if in_double { return "引号未闭合".into(); }

    // ② here-doc:结束标记必须顶格、逐行核对,整段粘贴一次成型更稳(保守报)
    for line in text.lines() {
        if has_heredoc_marker(line) {
            return "here-doc".into();
        }
    }

    // ③ 块结构:开合计数(剥掉单行内配对的引号内容再数,
    //    `echo "done"` 的 done 不会变成闭合标记)。未闭合数下限钳 0,
    //    参数里的字面 done 不会把多出来的「开」抵消掉。
    let mut n_loop = 0usize; // for / while / until ↔ done
    let mut n_if = 0usize;   // if ↔ fi
    let mut n_case = 0usize; // case ↔ esac
    for raw in text.lines() {
        let stripped = strip_quoted(raw);
        let t = stripped.trim();
        if t.is_empty() { continue; }
        if t.ends_with('\\') { return "续行反斜杠".into(); }
        n_loop = (n_loop + count_word(t, &["for", "while", "until"]) ).saturating_sub(count_word(t, &["done"]));
        n_if = (n_if + count_word(t, &["if"])).saturating_sub(count_word(t, &["fi"]));
        n_case = (n_case + count_word(t, &["case"])).saturating_sub(count_word(t, &["esac"]));
    }
    if n_loop > 0 || n_if > 0 || n_case > 0 {
        return "块结构未闭合".into();
    }
    String::new()
}

fn has_heredoc_marker(line: &str) -> bool {
    // <<EOF / <<- EOF / <<"EOF"
    let b = line.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if b[i] == b'<' && b.get(i + 1) == Some(&b'<') {
            let mut j = i + 2;
            if b.get(j) == Some(&b'-') { j += 1; }
            let rest = &line[j.min(line.len())..];
            let trimmed = rest.trim_start();
            let first = trimmed.chars().next();
            if matches!(first, Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '"' || c == '\'') {
                return true;
            }
        }
    }
    false
}

/// 数独立词出现次数(前后边界:非字母数字下划线)。
fn count_word(text: &str, words: &[&str]) -> usize {
    let b = text.as_bytes();
    let mut n = 0;
    for w in words {
        let wb = w.as_bytes();
        let mut i = 0;
        while i + wb.len() <= b.len() {
            if &b[i..i + wb.len()] == wb
                && (i == 0 || !is_word_byte(b[i - 1]))
                && (i + wb.len() == b.len() || !is_word_byte(b[i + wb.len()]))
            {
                n += 1;
                i += wb.len();
            } else {
                i += 1;
            }
        }
    }
    n
}

fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danger_basic() {
        assert_eq!(danger_token("rm -rf /data"), "rm -rf");
        assert_eq!(danger_token("rm -fr /data"), "rm -rf");
        assert_eq!(danger_token("rm -r -f /data"), "rm -rf");
        assert_eq!(danger_token("sudo rm -rf /data"), "rm -rf");
        assert_eq!(danger_token("mkfs.ext4 /dev/sda1"), "mkfs");
        assert_eq!(danger_token("dd if=/dev/zero of=/dev/sda"), "dd if=");
        assert_eq!(danger_token("shutdown -h now"), "shutdown");
        assert_eq!(danger_token("reboot"), "shutdown");
        assert_eq!(danger_token("init 0"), "init 0");
        assert_eq!(danger_token("chmod -R 777 /data"), "chmod -R 777");
        assert_eq!(danger_token("iptables -F"), "iptables -F");
        assert_eq!(danger_token("kill -9 1"), "kill -9 1");
    }

    #[test]
    fn danger_no_false_positive() {
        assert_eq!(danger_token("grep shutdown app.log"), "");
        assert_eq!(danger_token("echo \"rm -rf x\""), "");
        assert_eq!(danger_token("df -h && free -m"), "");
        assert_eq!(danger_token("ls -la"), "");
        assert_eq!(danger_token("echo shutdown"), "");
        assert_eq!(danger_token("systemctl restart nginx"), "");
        // rm 无 -rf 组合不算
        assert_eq!(danger_token("rm file.txt"), "");
        assert_eq!(danger_token("rm -r dir"), "");
        assert_eq!(danger_token("rm -f file"), "");
        // 单词边界
        assert_eq!(danger_token("ifconfig"), "");
        assert_eq!(danger_token("shutdown_now"), "");
    }

    #[test]
    fn danger_segments() {
        assert_eq!(danger_token("echo ok; rm -rf /data"), "rm -rf");
        assert_eq!(danger_token("echo ok && rm -rf /data"), "rm -rf");
        assert_eq!(danger_token("df -h | shutdown"), "shutdown");
        assert_eq!(danger_token("cd /tmp\nrm -rf /data"), "rm -rf");
        assert_eq!(danger_token("time rm -rf /data"), "rm -rf");
    }

    #[test]
    fn risk_closed_block_is_safe() {
        // §10① 的承诺:完整的 for…do…done 不算风险
        assert_eq!(continuation_risk("for h in a b; do\n  ssh $h x\ndone"), "");
        assert_eq!(continuation_risk("if true; then echo hi; fi"), "");
        assert_eq!(continuation_risk("case $x in\n a) echo 1;;\nesac"), "");
    }

    #[test]
    fn risk_unclosed() {
        assert_eq!(continuation_risk("if true; then\n  echo hi"), "块结构未闭合");
        // 单行块也查(单行 if…then 敲下去一样停在续行提示符)
        assert_eq!(continuation_risk("if true; then"), "块结构未闭合");
        // 闭合块 + 未闭合块并存(全文关键词法会漏报,逐块计数不漏)
        assert_eq!(continuation_risk("for i in 1 2; do echo $i; done\nif [ -f x ]; then"), "块结构未闭合");
        // 字面 done 把开吃成负数 → 钳 0 后仍报
        assert_eq!(continuation_risk("echo done\nfor x; do"), "块结构未闭合");
    }

    #[test]
    fn risk_quotes() {
        assert_eq!(continuation_risk("echo \"deploy to\nweb-01\""), "");
        assert_eq!(continuation_risk("echo \"a\\\"b\" | grep x"), "");
        assert_eq!(continuation_risk("echo 'don'"), "");
        assert_eq!(continuation_risk("echo 'unterminated"), "单引号未闭合");
        assert_eq!(continuation_risk("echo \"unterminated"), "引号未闭合");
        // 引号里的块词不计数
        assert_eq!(continuation_risk("echo \"for if done fi esac\""), "");
    }

    #[test]
    fn risk_misc() {
        assert_eq!(continuation_risk("cat <<EOF\nhi\nEOF"), "here-doc");
        assert_eq!(continuation_risk("apt install \\\n nginx"), "续行反斜杠");
        assert_eq!(continuation_risk("df -h && free -m"), "");
        assert_eq!(continuation_risk(""), "");
        assert_eq!(continuation_risk("rm -rf /data"), "");
    }

    // (#send-cmd-v2 2026-11-07) 行号槽三角 + 危险行背景条的数据源
    #[test]
    fn danger_lines_by_row() {
        assert_eq!(danger_lines("rm -rf /data"), vec![0]);
        assert_eq!(danger_lines("df -h\nrm -rf /data\necho hi"), vec![1]);
        // 一行多段命中只算一行
        assert_eq!(danger_lines("mkfs /dev/sda; reboot"), vec![0]);
        // 引号里的危险词不误报;行内剥引号
        assert_eq!(danger_lines("echo \"rm -rf x\"\nrm -fr /data"), vec![1]);
        // 空文本与纯安全文本
        assert!(danger_lines("").is_empty());
        assert!(danger_lines("ls -la\ndf -h").is_empty());
        // 分行:第 3 行命中(0-based 2)
        assert_eq!(danger_lines("echo a\nshutdown -h now\necho b"), vec![1]);
        // 段内前缀:time/sudo 剥掉后命中
        assert_eq!(danger_lines("time rm -rf /data"), vec![0]);
    }

    #[test]
    fn insert_indent_exact() {
        // 行中插入:光标在第 2 行(col=3)
        let (t, c) = insert_indent("ls\ncd tmp", 1, 3);
        assert_eq!(t, "ls\ncd     tmp");
        assert_eq!(c, "ls\ncd ".len() + 4);
        // 行首插入
        let (t, c) = insert_indent("abc", 0, 0);
        assert_eq!(t, "    abc");
        assert_eq!(c, 4);
        // 末尾插入
        let (t, _) = insert_indent("ls -l", 0, 5);
        assert_eq!(t, "ls -l    ");
        // 列越界钳到行尾
        let (t, _) = insert_indent("ab", 0, 99);
        assert_eq!(t, "ab    ");
        // 行号越界钳到最后一行
        let (t, _) = insert_indent("ab\ncd", 99, 0);
        assert_eq!(t, "ab\n    cd");
        // 空文本
        let (t, c) = insert_indent("", 0, 0);
        assert_eq!(t, "    ");
        assert_eq!(c, 4);
    }
}

/// (#send-cmd-p1 §04⑤) 多行 + 段首含 sudo：第二行起会被当密码吞掉。
/// 通知信息区补一句提示(不拦发送)。
pub fn sudo_hint(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 { return false; }
    for line in &lines {
        let t = line.trim_start();
        if t.starts_with("sudo") {
            let rest = &t[4..];
            if rest.starts_with(char::is_whitespace) { return true; }
        }
    }
    false
}

/// (#send-cmd-p1 §04②) 把命令插到「光标所在行的行尾之后」,新命令独占一行,
/// 返回 (新文本, 光标字节偏移)。行列由 Slint 侧按光标像素反推(等宽字体),
/// 中文行的列会有半格误差 —— 末尾追加(最常用路径)不受影响。
/// 规则与沙盘 insertCmd 同一口径:绝不把用户正在写的行劈成两半。
const NL: char = 0x0A as char;
const NL_CH: char = NL;
pub fn insert_at_cursor(text: &str, line: usize, col: usize, cmd: &str) -> (String, usize) {
    if cmd.is_empty() { return (text.to_string(), text.len()); }
    let lines: Vec<&str> = if text.is_empty() { vec![""] } else { text.split(NL).collect() };
    let li = line.min(lines.len().saturating_sub(1));
    let row = lines[li];
    let col = col.min(row.chars().count());
    // 行尾字符偏移 = 前面所有行的字符数 + 换行数 + 本行列数(Slint 侧列按
    // 字符格算,这里统一字符口径;char_indices 换回字节给字符串切片)
    let mut off = 0usize;
    for l in lines.iter().take(li) {
        off += l.chars().count() + 1;
    }
    // 插入点 = 光标所在行的**行尾**(不是光标列):绝不让新命令劈开正在写的行;
    off += lines[li].chars().count();
    let byte_at = |chars: usize| -> usize {
        text.char_indices().nth(chars).map(|(b, _)| b).unwrap_or(text.len())
    };
    let before = &text[..byte_at(off)];
    let after = &text[byte_at(off)..];
    let mut ins = String::new();
    if !before.is_empty() { ins.push(NL_CH); }
    ins.push_str(cmd);
    if !after.is_empty() && !after.starts_with(NL_CH) { ins.push(NL_CH); }
    let new_text = format!("{before}{ins}{after}");
    let caret = before.len() + ins.len();
    (new_text, caret)
}

/// (#send-cmd-v2 2026-11-07) Tab 缩进:在光标(行,列)处精确插入 4 个空格,
/// 不换行、不劈行 —— 与 insert_at_cursor(行尾插入+换行包裹)是两条语义,
/// 不能混用。返回 (新文本, 光标字节偏移)。
pub fn insert_indent(text: &str, line: usize, col: usize) -> (String, usize) {
    const INDENT: &str = "    ";
    let lines: Vec<&str> = if text.is_empty() { vec![""] } else { text.split(NL).collect() };
    let li = line.min(lines.len().saturating_sub(1));
    let row = lines[li];
    let col = col.min(row.chars().count());
    // 行列 → 字符偏移(与 insert_at_cursor 同一套换算口径)
    let mut off = 0usize;
    for l in lines.iter().take(li) {
        off += l.chars().count() + 1;
    }
    off += col;
    let byte_at = |chars: usize| -> usize {
        text.char_indices().nth(chars).map(|(b, _)| b).unwrap_or(text.len())
    };
    let at = byte_at(off);
    let new_text = format!("{}{}{}", &text[..at], INDENT, &text[at..]);
    (new_text, at + INDENT.len())
}
