// (#editor-native 2026-11-16) 全文高亮:输出交给 Slint TextInput.syntax-spans 的
// **全文 UTF-8 字节区间**。两条引擎:
//   · tree-sitter(代码文件,整树解析 → 叶子字节区间;语言无 grammar 时回退行级)
//   · 行级规则(日志 / 行式配置:regex 逐规则找最早命中,间隙不产出)
//
// 旧实现按「可见行窗口」产出「行内字符区间」,再由 Slint 按 `列 × 单格宽` 绝对定位。
// 那是终端的坐标模型:等宽主字体没有 CJK 字形,中文回退成比例字体、前进量≠2 格,
// 于是中文之后整行漂移。现在改为:整篇扫描、字节区间、交给原生 shaping 叠色 ——
// 段落切分/度量/光标/选择/IME 全部与纯文本一致,只有字形颜色变化。
use super::highlight::{GSpan, SpanKind};
use super::{highlight_line, Lang};
use tree_sitter::Parser;

/// tree-sitter 整树解析上限:再大就用行级规则(解析成本随体积线性上升)。
const TS_MAX_BYTES: usize = 2 * 1024 * 1024;
/// 行级规则(regex 逐行)的着色上限。整篇扫描是一次性成本,但 8MB 已近 1s 量级,
/// 再大就不给色 —— 大文件浏览场景颜色价值低,而滚动/编辑本身照常。
/// (宿主桥对 256KB 以上的扫描会丢到后台线程,所以这里不是"卡 UI"的阈值。)
const LINE_MAX_BYTES: usize = 8 * 1024 * 1024;

/// 全文高亮。返回的区间有序、互不重叠(渲染侧会丢弃越界/重叠项)。
pub(crate) fn highlight_all(text: &str, lang: Lang) -> Vec<GSpan> {
    if text.is_empty() {
        return Vec::new();
    }
    if lang.uses_tree_sitter() && text.len() <= TS_MAX_BYTES {
        if let Some(spans) = ts_spans(text, lang) {
            return spans;
        }
    }
    if text.len() > LINE_MAX_BYTES {
        return Vec::new();
    }
    line_spans(text, lang)
}

// ── tree-sitter 路径 ──────────────────────────────────────────
/// 整树解析 → DFS 叶子字节区间。None = 该语言无 grammar 或解析失败(调用方回退行级)。
fn ts_spans(text: &str, lang: Lang) -> Option<Vec<GSpan>> {
    let language = ts_language(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(text, None)?;
    let mut out = Vec::new();
    let mut cursor = tree.walk();
    walk(&mut cursor, None, lang, &mut out);
    Some(out)
}

pub(crate) fn ts_language(lang: Lang) -> Option<tree_sitter::Language> {
    use tree_sitter::Language;
    Some(match lang {
        Lang::Java => Language::from(tree_sitter_java::LANGUAGE),
        Lang::Sql => Language::from(tree_sitter_sequel::LANGUAGE),
        Lang::Json => Language::from(tree_sitter_json::LANGUAGE),
        Lang::Yaml => Language::from(tree_sitter_yaml::LANGUAGE),
        Lang::Toml => Language::from(tree_sitter_toml_ng::LANGUAGE),
        Lang::Bash => Language::from(tree_sitter_bash::LANGUAGE),
        Lang::Xml => Language::from(tree_sitter_xml::LANGUAGE_XML),
        Lang::Python => Language::from(tree_sitter_python::LANGUAGE),
        _ => return None,
    })
}

/// DFS:叶子按 kind 表着色(仅非 Plain 才产出,缩短下发字符串)。
fn walk(cursor: &mut tree_sitter::TreeCursor, parent: Option<&str>, lang: Lang, out: &mut Vec<GSpan>) {
    let node = cursor.node();
    let kind = node.kind().to_string();
    if node.child_count() == 0 {
        if let Some(sp_kind) = kind_for(lang, &kind, parent, node.is_named()) {
            if sp_kind != SpanKind::Plain {
                let (s, e) = (node.start_byte(), node.end_byte());
                if e > s {
                    out.push(GSpan { start: s, end: e, kind: sp_kind });
                }
            }
        }
        return;
    }
    let pk = kind.as_str();
    if cursor.goto_first_child() {
        loop {
            walk(cursor, Some(pk), lang, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

// ── 行级规则路径 ──────────────────────────────────────────────
/// 逐行跑行级规则,把「行内字符列」换算成全文字节偏移。
fn line_spans(text: &str, lang: Lang) -> Vec<GSpan> {
    let mut out = Vec::new();
    let mut line_start = 0usize;
    for raw in text.split_inclusive('\n') {
        let body = raw.strip_suffix('\n').unwrap_or(raw);
        let body = body.strip_suffix('\r').unwrap_or(body);
        if !body.is_empty() {
            let offsets = char_byte_offsets(body);
            for sp in highlight_line::highlight_line(body, lang) {
                if sp.kind == SpanKind::Plain {
                    continue;
                }
                let s = offsets[sp.start.min(offsets.len() - 1)];
                let e = offsets[(sp.start + sp.len).min(offsets.len() - 1)];
                if e > s {
                    out.push(GSpan {
                        start: line_start + s,
                        end: line_start + e,
                        kind: sp.kind,
                    });
                }
            }
        }
        line_start += raw.len();
    }
    out
}

/// 每字符起始字节 + 末尾哨兵(长度 = 字符数 + 1)。
fn char_byte_offsets(s: &str) -> Vec<usize> {
    let mut v: Vec<usize> = s.char_indices().map(|(i, _)| i).collect();
    v.push(s.len());
    v
}

/// 着色表:通用规则(注释/字符串/数字)+ 每语言 identifier/关键字特化。
fn kind_for(lang: Lang, kind: &str, parent: Option<&str>, named: bool) -> Option<SpanKind> {
    // 注释/字符串/数字:各 grammar 的 kind 名高度规整,先走通用
    if kind.contains("comment") {
        return Some(SpanKind::Com);
    }
    match lang {
        Lang::Java => {
            if !named {
                return Some(keyword_or_plain(kind));
            }
            match kind {
                "type_identifier" => Some(SpanKind::Ty),
                "string_literal" | "character_literal" | "text_block" => Some(SpanKind::Str),
                "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal"
                | "binary_integer_literal" | "decimal_floating_point_literal" => Some(SpanKind::Num),
                "line_comment" | "block_comment" => Some(SpanKind::Com),
                "identifier" => Some(match parent.unwrap_or("") {
                    "method_invocation" | "method_declaration" | "constructor_declarator" | "object_creation_expression" => SpanKind::Fn,
                    k if k.contains("annotation") => SpanKind::Ann,
                    _ => SpanKind::Plain,
                }),
                "annotation" | "marker_annotation" => Some(SpanKind::Ann),
                _ => Some(SpanKind::Plain),
            }
        }
        Lang::Sql => {
            if kind == "keyword" {
                return Some(SpanKind::Kw);
            }
            match kind {
                "string" => Some(SpanKind::Str),
                "number_literal" => Some(SpanKind::Num),
                "identifier" => Some(match parent.unwrap_or("") {
                    "function_call" => SpanKind::Fn,
                    _ => SpanKind::Plain,
                }),
                _ => Some(SpanKind::Plain),
            }
        }
        Lang::Json => match kind {
            "true" | "false" | "null" => Some(SpanKind::Kw),
            "number" => Some(SpanKind::Num),
            "string" => Some(SpanKind::Str),
            _ => Some(SpanKind::Plain),
        },
        Lang::Yaml => match kind {
            "comment" => Some(SpanKind::Com),
            "double_quote_scalar" | "single_quote_scalar" | "block_scalar" => Some(SpanKind::Str),
            "anchor" | "alias" => Some(SpanKind::Ann),
            _ => Some(SpanKind::Plain),
        },
        Lang::Toml => match kind {
            "comment" => Some(SpanKind::Com),
            "string" => Some(SpanKind::Str),
            "integer" | "float" | "date_time" | "local_date" | "local_time" | "date" | "time" => Some(SpanKind::Num),
            "boolean" => Some(SpanKind::Kw),
            "bare_key" | "quoted_key" | "dotted_key" => Some(SpanKind::Ty),
            _ => Some(SpanKind::Plain),
        },
        Lang::Bash => {
            if !named {
                return Some(keyword_or_plain(kind));
            }
            match kind {
                "comment" => Some(SpanKind::Com),
                "string" | "raw_string" | "concatenation" => Some(SpanKind::Str),
                "number" => Some(SpanKind::Num),
                "variable_name" | "simple_expansion" => Some(SpanKind::Ann),
                "word" => Some(match parent.unwrap_or("") {
                    "command" => SpanKind::Fn,
                    _ => SpanKind::Plain,
                }),
                "file_descriptor" => Some(SpanKind::Num),
                _ => Some(SpanKind::Plain),
            }
        }
        Lang::Xml => match kind {
            "Name" => Some(SpanKind::Fn),
            "AttributeValue" | "AttValue" => Some(SpanKind::Str),
            "Comment" | "CDATA" => Some(SpanKind::Com),
            "Pi" | "Decl" | "SystemLiteral" => Some(SpanKind::Ann),
            _ => Some(SpanKind::Plain),
        },
        Lang::Python => {
            if !named {
                return Some(keyword_or_plain(kind));
            }
            match kind {
                "string" | "concatenated_string" => Some(SpanKind::Str),
                "integer" | "float" => Some(SpanKind::Num),
                "identifier" => Some(match parent.unwrap_or("") {
                    "function_definition" | "call" => SpanKind::Fn,
                    "class_definition" => SpanKind::Ty,
                    _ => SpanKind::Plain,
                }),
                "decorator" => Some(SpanKind::Ann),
                "comment" => Some(SpanKind::Com),
                _ => Some(SpanKind::Plain),
            }
        }
        _ => Some(SpanKind::Plain),
    }
}

fn keyword_or_plain(token_kind: &str) -> SpanKind {
    const JAVA_KW: &[&str] = &[
        "public", "private", "protected", "static", "final", "void", "class", "interface", "enum",
        "return", "new", "if", "else", "for", "while", "do", "try", "catch", "finally", "throw",
        "throws", "implements", "extends", "import", "package", "this", "super", "abstract",
        "null", "true", "false", "instanceof", "assert", "synchronized", "volatile", "transient",
        "native", "default",
    ];
    const BASH_KW: &[&str] = &[
        "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac",
        "function", "in", "select", "coproc", "time",
    ];
    const PY_KW: &[&str] = &[
        "def", "return", "if", "elif", "else", "for", "while", "import", "from", "as", "with",
        "try", "except", "finally", "raise", "class", "lambda", "pass", "break", "continue",
        "global", "nonlocal", "del", "assert", "yield", "in", "is", "not", "and", "or", "None",
        "True", "False",
    ];
    let k = token_kind;
    let hit = match k {
        "true" | "false" | "null" => true,
        _ => JAVA_KW.contains(&k) || BASH_KW.contains(&k) || PY_KW.contains(&k),
    };
    if hit { SpanKind::Kw } else { SpanKind::Plain }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 所有区间必须:在界内、落在字符边界、有序、互不重叠 —— 否则渲染侧会丢色,
    /// 或(越界/半字符)直接丢弃。
    fn assert_wellformed(text: &str, spans: &[GSpan]) {
        let mut prev_end = 0usize;
        for sp in spans {
            assert!(sp.start < sp.end, "空区间 {:?}", sp);
            assert!(sp.end <= text.len(), "越界 {:?} (len {})", sp, text.len());
            assert!(text.is_char_boundary(sp.start), "起点非字符边界 {:?}", sp);
            assert!(text.is_char_boundary(sp.end), "终点非字符边界 {:?}", sp);
            assert!(sp.start >= prev_end, "乱序/重叠 {:?} (prev_end {})", sp, prev_end);
            prev_end = sp.end;
        }
    }

    #[test]
    fn shell_spans_stay_on_char_boundaries_with_cjk_comments() {
        // CJK 注释 + 中文串:字节区间必须仍落在字符边界(旧网格模型在此漂移)
        let text = "#!/bin/sh\n# 中文注释:检查依赖\nif [ \"$1\" = \"就绪\" ]; then\n  echo 42\nfi\n";
        let spans = highlight_all(text, Lang::Bash);
        assert!(!spans.is_empty(), "shell 应有高亮");
        assert_wellformed(text, &spans);
    }

    #[test]
    fn cjk_only_text_yields_wellformed_spans() {
        let text = "# 全是中文的注释行\n# 第二行也是中文\nvalue = 1\n";
        let spans = highlight_all(text, Lang::Toml);
        assert_wellformed(text, &spans);
    }

    #[test]
    fn line_rule_language_reports_keyword_color() {
        let text = "# comment\nvalue = 123\n";
        let spans = highlight_all(text, Lang::Toml);
        // 至少注释一条
        assert!(spans.iter().any(|s| s.kind == SpanKind::Com), "应有着色的注释");
    }

    #[test]
    fn empty_and_huge_text_are_refused() {
        assert!(highlight_all("", Lang::Bash).is_empty());
    }

    #[test]
    fn ts_language_path_is_wellformed_for_json() {
        let text = "{\n  \"键\": \"中文值\",\n  \"n\": 12\n}\n";
        let spans = highlight_all(text, Lang::Json);
        assert_wellformed(text, &spans);
    }

    #[test]
    fn java_with_cjk_comments_is_wellformed_and_colored() {
        // 旧的手搓网格在中文注释处必然漂移;字节区间模型必须逐字对齐
        let text = concat!(
            "package com.mterm.transfer.sftp;\n",
            "\n",
            "/** SFTP 传输服务:上传 / 下载 / 断点续传。 */\n",
            "public class SftpTransferService implements TransferService {\n",
            "    // 同步上传(带断点续传),返回远端字节数\n",
            "    public long syncUpload(String remotePath) throws IOException {\n",
            "        log.info(\"上传完成:{} ({} 字节)\", remotePath, bytes);\n",
            "    }\n",
            "}\n",
        );
        let spans = highlight_all(text, Lang::Java);
        assert_wellformed(text, &spans);
        // 中文注释必须整条落在注释类别里(不因 CJK 宽度而截断)
        let com = spans.iter().find(|s| s.kind == SpanKind::Com).expect("应有注释");
        let seg = &text[com.start..com.end];
        assert!(seg.contains("传输服务"), "注释未覆盖中文: {seg:?}");
        assert!(seg.starts_with("/**"), "注释起点应在 /** : {seg:?}");
    }
}
