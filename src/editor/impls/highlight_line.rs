// (#editor-hifi) 行级高亮:日志 / nginx / ini / properties / env / Dockerfile / Markdown。
// 与设计稿 JS tokenizer 同一套规则(regex 逐规则找最早命中,间隙=默认色);
// span 列以 unicode 字符计(渲染侧按字符切片着色)。
use super::highlight::{Span, SpanKind};
use super::Lang;
use regex::Regex;

struct Rule {
    re: Regex,
    kind: SpanKind,
}

fn rules_for(lang: Lang) -> &'static Vec<Rule> {
    // 一次性编译;行级规则都是词法级,无跨行状态(唯一跨行的 md 围栏用单行近似)
    static LOG: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    static NGINX: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    static INI: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    static DOCKER: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    static MD: std::sync::OnceLock<Vec<Rule>> = std::sync::OnceLock::new();
    match lang {
        Lang::Log => LOG.get_or_init(|| {
            vec![
                r(ri(r"^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}[.\d]*"), SpanKind::Ts),
                r(ri(r"\[[\w .,-]+\]"), SpanKind::Thr),
                r(ri(r"\b(?:FATAL|ERROR)\b"), SpanKind::LvlError),
                r(ri(r"\bWARN(?:ING)?\b"), SpanKind::LvlWarn),
                r(ri(r"\bINFO\b"), SpanKind::LvlInfo),
                r(ri(r"\b(?:DEBUG|TRACE)\b"), SpanKind::LvlDebug),
                r(ri(r"^Caused by:.*$"), SpanKind::Cs),
                r(ri(r"^[\t ]+at .*|^[\t ]*\.\.\..*$"), SpanKind::At),
                r(ri(r"[\w.$]+(?:Exception|Error)\b"), SpanKind::Exc),
            ]
        }),
        Lang::Nginx => NGINX.get_or_init(|| {
            vec![
                r(ri(r"#.*$"), SpanKind::Com),
                r(ri(r#""[^"]*"|'[^']*'"#), SpanKind::Str),
                r(ri(r"\$\w+"), SpanKind::Ann),
                r(ri(r"\b(?:user|worker_processes|pid|events|http|include|default_type|log_format|access_log|sendfile|tcp_nopush|tcp_nodelay|keepalive_timeout|client_max_body_size|gzip|gzip_types|server|listen|server_name|ssl_certificate|ssl_certificate_key|location|proxy_pass|proxy_set_header|proxy_redirect|proxy_read_timeout|proxy_connect_timeout|proxy_http_version|root|index|try_files|rewrite|return|set|if|worker_connections|error_log|upstream|charset|autoindex|ssl_protocols|ssl_ciphers|add_header|fastcgi_pass|expires)\b"), SpanKind::Fn),
                r(ri(r"\b\d+(?:\.\d+)?[kKmMgG]?\b"), SpanKind::Num),
                r(ri(r"[{}]"), SpanKind::Ty),
            ]
        }),
        Lang::Ini | Lang::Properties | Lang::Env => INI.get_or_init(|| {
            vec![
                r(ri(r"^[;#].*$"), SpanKind::Com),
                r(ri(r"^\[[^\]]*\]"), SpanKind::Ty),
                // KEY= 的 key 侧(properties/env 大写键醒目)
                r(ri(r"^\s*[A-Za-z_][\w .-]*="), SpanKind::Fn),
            ]
        }),
        Lang::Dockerfile => DOCKER.get_or_init(|| {
            vec![
                r(ri(r"#.*$"), SpanKind::Com),
                r(ri(r#"^\s*(?:FROM|RUN|CMD|LABEL|EXPOSE|ENV|ADD|COPY|ENTRYPOINT|VOLUME|USER|WORKDIR|ARG|ONBUILD|STOPSIGNAL|HEALTHCHECK|SHELL|MAINTAINER)\b"#), SpanKind::Kw),
                r(ri(r#""[^"]*""#), SpanKind::Str),
            ]
        }),
        Lang::Markdown => MD.get_or_init(|| {
            vec![
                r(ri(r"^#{1,6} .*$"), SpanKind::Ty),
                r(ri(r"^```.*|^~~~.*"), SpanKind::Com),
                r(ri(r"\*\*[^*]+\*\*|__[^_]+__"), SpanKind::Kw),
                r(ri(r"`[^`]+`"), SpanKind::Str),
                r(ri(r"^\s*[-*+] |^\s*\d+\. "), SpanKind::Ann),
            ]
        }),
        _ => LOG.get_or_init(|| vec![]),
    }
}

fn ri(pattern: &str) -> Regex {
    Regex::new(pattern).expect("行级高亮正则编译失败")
}
fn r(re: Regex, kind: SpanKind) -> Rule {
    Rule { re, kind }
}

/// 对一行做行级高亮(与设计稿 JS 版 tokenizer 同语义:逐位置找最早命中的规则)。
pub(crate) fn highlight_line(line: &str, lang: Lang) -> Vec<Span> {
    let rules = rules_for(lang);
    if rules.is_empty() || line.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = line.chars().collect();
    let _ = chars; // 字符总数在 push_span_bytes 内按 char_indices 统计

    let mut out: Vec<Span> = Vec::new();
    let mut i = 0usize; // 字节游标
    while i < line.len() {
        // 跳过 UTF-8 中间字节(非起始字节直接前进)
        if !line.is_char_boundary(i) {
            i += 1;
            continue;
        }
        // 最早命中优先;同起点时按规则声明顺序(词法优先级)
        let best = rules
            .iter()
            .filter_map(|rule| rule.re.find_at(line, i).map(|m| (m.start(), m.end(), rule.kind)))
            .min_by_key(|(s, _, _)| *s);
        match best {
            Some((s, e, kind)) => {
                if s > i {
                    push_span_bytes(&mut out, line, i, s, SpanKind::Plain);
                }
                push_span_bytes(&mut out, line, s, e, kind);
                i = e;
            }
            None => {
                // 该位置起无任何命中 → 剩余全部默认色
                push_span_bytes(&mut out, line, i, line.len(), SpanKind::Plain);
                break;
            }
        }
    }
    out
}

/// 字节区间 → 字符区间入列(长度为 0 丢弃)。
fn push_span_bytes(out: &mut Vec<Span>, line: &str, s: usize, e: usize, kind: SpanKind) {
    let cs = line[..s].chars().count();
    let ce = line[..e].chars().count();
    if ce > cs {
        out.push(Span { start: cs, len: ce - cs, kind });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_levels() {
        let line = "2026-11-14 21:02:44.102 [http-nio-8080-exec-7] ERROR c.m.t.S - Failed to upload";
        let spans = highlight_line(line, Lang::Log);
        let kinds: Vec<_> = spans.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SpanKind::Ts));
        assert!(kinds.contains(&SpanKind::Thr));
        assert!(kinds.contains(&SpanKind::LvlError));
    }

    #[test]
    fn nginx_directive() {
        let spans = highlight_line("    listen 443 ssl;", Lang::Nginx);
        assert!(spans.iter().any(|s| s.kind == SpanKind::Fn));
        assert!(spans.iter().any(|s| s.kind == SpanKind::Num));
    }

    #[test]
    fn ini_section_and_comment() {
        let a = highlight_line("[server]", Lang::Ini);
        assert!(a.iter().any(|s| s.kind == SpanKind::Ty));
        let b = highlight_line("# 注释", Lang::Ini);
        assert!(b.iter().any(|s| s.kind == SpanKind::Com));
    }

    #[test]
    fn spans_cover_in_char_space() {
        let line = "2026-11-14 21:02:44 [t] ERROR at x 中文中文";
        for s in highlight_line(line, Lang::Log) {
            assert!(s.start + s.len <= line.chars().count());
        }
    }
}
