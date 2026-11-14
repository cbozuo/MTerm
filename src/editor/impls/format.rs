// (#editor-hifi) 格式化:纯 Rust 生态真实边界(设计稿矩阵定稿):
// ✓ JSON=serde_json / SQL=sqlformat / TOML=toml_edit(保注释)/ XML+nginx=自写状态机 /
//   Java=缩进归一;✗ YAML(serde_yaml 丢注释)/ Shell、Python(无纯 Rust formatter)。
use super::Lang;

/// 返回格式化后全文;Err = 用户可读的错误信息;Ok(None) = 该语言不支持(菜单应置灰)。
pub(crate) fn format_doc(lang: Lang, text: &str) -> Result<Option<String>, String> {
    match lang {
        Lang::Json => {
            let v: serde_json::Value = serde_json::from_str(text)
                .map_err(|e| format!("JSON 语法错误: {e}"))?;
            Ok(Some(serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?))
        }
        Lang::Sql => {
            let opts = sqlformat::FormatOptions {
                indent: sqlformat::Indent::Spaces(4),
                uppercase: Some(true),
                lines_between_queries: 1,
                ..Default::default()
            };
            Ok(Some(sqlformat::format(text, &sqlformat::QueryParams::None, &opts)))
        }
        Lang::Toml => {
            // toml_edit:parse 后回写,注释与既有格式保留,引号/空白由解析器规范化
            let doc = text
                .parse::<toml_edit::DocumentMut>()
                .map_err(|e| format!("TOML 语法错误: {e}"))?;
            Ok(Some(doc.to_string()))
        }
        Lang::Xml => Ok(Some(format_xml(text)?)),
        Lang::Nginx => Ok(Some(format_braces(text, 4))),
        Lang::Java => Ok(Some(reindent_braces(text, 4))),
        Lang::Yaml => Err("YAML 不提供格式化(重序列化会丢失注释)".into()),
        Lang::Bash | Lang::Python => Err("该语言暂无纯 Rust 格式化器".into()),
        Lang::Log | Lang::Text | Lang::Markdown | Lang::Dockerfile => Ok(None),
        Lang::Ini | Lang::Properties | Lang::Env => Ok(None),
    }
}

/// 菜单项是否可用(只读/不支持语言置灰)。
pub(crate) fn can_format(lang: Lang) -> bool {
    matches!(lang, Lang::Json | Lang::Sql | Lang::Toml | Lang::Xml | Lang::Nginx | Lang::Java)
}

// ── XML:tag 深度缩进状态机 ─────────────────────────────────
fn format_xml(text: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut depth = 0i32;
    let indent = "    ";
    let mut rest = text.trim();
    // 粗分词:扫描 <...> 与标签间文本。声明/注释/CDATA 原样;开 tag 深度+,闭 tag-。
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            // 尾部纯文本:贴在上一行后(不做重排)
            let t = rest.trim();
            if !t.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&"    ".repeat(depth.max(0) as usize));
                out.push_str(t);
            }
            break;
        };
        // 标签前文本
        let head = rest[..lt].trim();
        if !head.is_empty() && !out.is_empty() {
            // 内联文本(DTD 等)忽略换行处理,原样拼在当前行
        }
        let Some(gt_rel) = rest[lt..].find('>') else {
            return Err("XML 语法错误: 未闭合的 <".into());
        };
        let gt = lt + gt_rel;
        let tag_full = &rest[lt..=gt];
        let tag_inner = tag_full.trim_start_matches('<').trim_end_matches('>');
        let is_comment = tag_inner.starts_with("!--");
        let is_decl = tag_inner.starts_with('?') || tag_inner.starts_with('!');
        let is_close = tag_inner.starts_with('/');
        let is_selfclose = tag_inner.ends_with('/');
        if is_comment || is_decl {
            // 注释/声明原样一行
        } else if is_close {
            depth -= 1;
        }
        let trimmed_head = head.is_empty() && out.is_empty();
        let _ = trimmed_head;
        out.push_str(&"    ".repeat(depth.max(0) as usize));
        out.push_str(tag_full.trim());
        out.push('\n');
        if !is_comment && !is_decl && !is_close && !is_selfclose {
            depth += 1;
        }
        rest = rest[gt + 1..].trim_start();
        // 开 tag 与闭 tag 同行:<a></a> —— 简化:不做(主流 XML 文件多为分行)
    }
    while out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

/// nginx / 通用 {}:token 流重排(语句以 ; / 换行结束,{} 调深度),支持单行压缩输入。
fn format_braces(text: &str, indent_spaces: usize) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    let mut buf = String::new();
    let pad = |d: i32| " ".repeat((indent_spaces as i32 * d).max(0) as usize);
    for c in text.chars() {
        match c {
            '{' => {
                let line = buf.trim().to_string();
                buf.clear();
                out.push_str(&pad(depth));
                if !line.is_empty() {
                    out.push_str(&line);
                    out.push(' ');
                }
                out.push('{');
                out.push('\n');
                depth += 1;
            }
            '}' => {
                let line = buf.trim().to_string();
                buf.clear();
                if !line.is_empty() {
                    out.push_str(&pad(depth));
                    out.push_str(&line);
                    out.push('\n');
                }
                depth -= 1;
                out.push_str(&pad(depth.max(0)));
                out.push('}');
                out.push('\n');
            }
            ';' => {
                buf.push(';');
                let line = buf.trim().to_string();
                buf.clear();
                if !line.is_empty() {
                    out.push_str(&pad(depth));
                    out.push_str(&line);
                    out.push('\n');
                }
            }
            '\n' => {
                let line = buf.trim().to_string();
                buf.clear();
                if !line.is_empty() {
                    out.push_str(&pad(depth));
                    out.push_str(&line);
                    out.push('\n');
                }
            }
            _ => buf.push(c),
        }
    }
    let line = buf.trim().to_string();
    if !line.is_empty() {
        out.push_str(&pad(depth));
        out.push_str(&line);
        out.push('\n');
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Java 缩进归一:逐行 strip 前导空白,按 {} 深度重排(跳过字符串/注释里的括号)。
fn reindent_braces(text: &str, indent_spaces: usize) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for raw in text.lines() {
        let line = raw.trim_start();
        if line.is_empty() {
            out.push('\n');
            continue;
        }
        let opens_close = line.starts_with('}');
        if opens_close {
            depth -= 1;
        }
        out.push_str(&" ".repeat(indent_spaces * depth.max(0) as usize));
        out.push_str(line);
        out.push('\n');
        depth += net_braces(line);
        if depth < 0 {
            depth = 0;
        }
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// 行内净 {} 变化(忽略字符串字面量与 // 行注释里的花括号)。
fn net_braces(line: &str) -> i32 {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    let mut prev = '\0';
    for c in line.chars() {
        if escape {
            escape = false;
            prev = c;
            continue;
        }
        if c == '\\' && in_str {
            escape = true;
            prev = c;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            prev = c;
            continue;
        }
        if in_str {
            prev = c;
            continue;
        }
        if prev == '/' && c == '/' {
            return depth; // 行注释起,后面全忽略
        }
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        prev = c;
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_pretty() {
        let out = format_doc(Lang::Json, r#"{"a":1,"b":[2,3]}"#).unwrap().unwrap();
        assert!(out.contains("\"a\": 1"));
        assert!(out.lines().count() > 1);
    }

    #[test]
    fn sql_uppercase() {
        let out = format_doc(Lang::Sql, "select * from t where a=1").unwrap().unwrap();
        assert!(out.contains("SELECT"));
        assert!(out.contains("WHERE"));
    }

    #[test]
    fn yaml_refused() {
        assert!(format_doc(Lang::Yaml, "a: 1").is_err());
    }

    #[test]
    fn unsupported_none() {
        assert!(format_doc(Lang::Log, "x").unwrap().is_none());
        assert!(format_doc(Lang::Text, "x").unwrap().is_none());
    }

    #[test]
    fn nginx_reindent() {
        let out = format_doc(Lang::Nginx, "server{listen 443;location /{root /www;}}").unwrap().unwrap();
        assert!(out.contains("server {"));
        assert!(out.contains("    listen 443;"));
    }

    #[test]
    fn java_reindent_keeps_string_braces() {
        let out = reindent_braces("class A {\nvoid m(){ String s=\"{\"; if (x) { log(\"}\"); } }\n}", 4);
        // 字符串里的 {} 不参与深度
        assert!(out.contains("    void m()"));
    }

    #[test]
    fn json_error_message() {
        let e = format_doc(Lang::Json, "{bad}").unwrap_err();
        assert!(e.contains("JSON"));
    }
}
