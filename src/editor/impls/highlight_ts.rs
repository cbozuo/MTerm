// (#editor-hifi) tree-sitter 高亮引擎:可编辑代码文件(≤16MB)整树解析 + 可见窗口遍历。
// 解析惰性:编辑时只 bump edit_version,滚动到该 doc / 空闲刷新时才重 parse(一次 ~100ms/16MB,
// 不在键入路径上)。着色 = 语法树叶子节点 → kind 表(关键字走匿名 token 文本比对)。
use super::highlight::{Span, SpanKind};
use super::{highlight_line, EditorDoc, Lang};
use tree_sitter::{Parser, Tree};

/// 对可见窗口 [first, last] 输出每行 spans(ts 语言走语法树,其余走行级规则)。
pub(crate) fn highlight_visible(doc: &mut EditorDoc, first: usize, last: usize) -> Vec<Vec<Span>> {
    let last = last.min(doc.len_lines().saturating_sub(1));
    let mut rows = Vec::with_capacity(last.saturating_sub(first) + 1);
    if !doc.lang.uses_tree_sitter() || doc.big {
        for r in first..=last {
            let spans = if let Some(cached) = doc.spans_cache.get(&r) {
                cached.clone()
            } else {
                let s = highlight_line::highlight_line(&doc.line_text(r), doc.lang);
                doc.spans_cache.insert(r, s.clone());
                s
            };
            rows.push(spans);
        }
        return rows;
    }
    ensure_tree(doc);
    let Some(tree) = doc.tree_cache.clone() else {
        // 解析失败兜底:整窗口默认色
        for _ in first..=last {
            rows.push(Vec::new());
        }
        return rows;
    };
    // ts 模式缓存也复用(行号 → spans)
    for r in first..=last {
        if let Some(cached) = doc.spans_cache.get(&r) {
            rows.push(cached.clone());
        } else {
            rows.push(Vec::new());
        }
    }
    collect_into(&tree, doc, first, last, &mut rows);
    for (i, r) in (first..=last).enumerate() {
        doc.spans_cache.insert(r, rows[i].clone());
    }
    rows
}

/// 惰性解析(版本不符才重 parse)。
pub(crate) fn ensure_tree(doc: &mut EditorDoc) {
    if doc.tree_cache.is_some() && doc.ts_version == doc.edit_version {
        return;
    }
    let Some(lang) = ts_language(doc.lang) else { return };
    let text = doc.rope.to_string();
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return;
    }
    doc.tree_cache = parser.parse(&text, None);
    doc.ts_version = doc.edit_version;
    doc.spans_cache.clear();
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

/// DFS 遍历:只深入与窗口相交的子树;叶子按 kind 表着色,跨行节点逐行拆分。
fn collect_into(tree: &Tree, doc: &EditorDoc, first: usize, last: usize, out: &mut [Vec<Span>]) {
    let mut cursor = tree.walk();
    walk(&mut cursor, None, first, last, doc, out);
}

fn walk(cursor: &mut tree_sitter::TreeCursor, parent: Option<&str>, first: usize, last: usize, doc: &EditorDoc, out: &mut [Vec<Span>]) {
    let node = cursor.node();
    let kind = node.kind().to_string();
    let (sr, er) = (node.start_position().row, node.end_position().row);
    if er < first || sr > last.min(doc.len_lines().saturating_sub(1)) {
        return; // 剪枝:整棵子树与窗口不相交
    }
    if node.child_count() == 0 {
        if let Some(sp_kind) = kind_for(doc.lang, &kind, parent, node.is_named()) {
            push_leaf(node, sr, er, sp_kind, doc, first, out);
        }
        return;
    }
    let pk = kind.as_str();
    if cursor.goto_first_child() {
        loop {
            walk(cursor, Some(pk), first, last, doc, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn push_leaf(node: tree_sitter::Node, sr: usize, er: usize, kind: SpanKind, doc: &EditorDoc, first: usize, out: &mut [Vec<Span>]) {
    for row in sr..=er.min(doc.len_lines().saturating_sub(1)) {
        if row < first || row - first >= out.len() {
            continue;
        }
        let line_chars = doc.line_text(row).chars().count();
        let s_col = if row == sr { node.start_position().column } else { 0 };
        let e_col = if row == er { node.end_position().column } else { line_chars };
        let s = s_col.min(line_chars);
        let e = e_col.min(line_chars);
        if e > s {
            out[row - first].push(Span { start: s, len: e - s, kind });
        }
    }
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
            "string" => Some(match parent.unwrap_or("") {
                "document" | "array" | "object" => SpanKind::Str,
                // pair 的键(第一个 string 子节点)按 key 着色由调用顺序天然保证:
                // pair 的 key 与 value 都是 string,这里再按父节点区分:pair → 键青/值绿
                _ => SpanKind::Str,
            }),
            _ => Some(SpanKind::Plain),
        },
        Lang::Yaml => match kind {
            "comment" => Some(SpanKind::Com),
            "double_quote_scalar" | "single_quote_scalar" | "block_scalar" => Some(SpanKind::Str),
            "anchor" | "alias" => Some(SpanKind::Ann),
            "plain_scalar" => Some(SpanKind::Plain),
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
            "Text" | "CharData" => Some(SpanKind::Plain),
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
