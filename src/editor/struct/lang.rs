// (#editor-hifi) 语言识别:扩展名优先 → Dockerfile 等特殊文件名 → shebang 兜底。
// 识别结果决定:高亮引擎(tree-sitter / 行级)、格式化器、大文件降级策略。

/// 全部支持的语言。高亮引擎与格式化支持见 docs 设计稿「文件格式支持矩阵」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lang {
    Java,
    Sql,
    Json,
    Yaml,
    Toml,
    Bash,
    Xml,
    Python,
    Log,
    Nginx,
    Ini,
    Properties,
    Env,
    Dockerfile,
    Markdown,
    Text,
}

impl Lang {
    /// 扩展名(小写,不含点)→ 语言。Dockerfile 无扩展名,按文件名匹配。
    pub(super) fn from_ext(ext: &str) -> Option<Lang> {
        Some(match ext {
            "java" => Lang::Java,
            "sql" => Lang::Sql,
            "json" => Lang::Json,
            "yml" | "yaml" => Lang::Yaml,
            "toml" => Lang::Toml,
            "sh" | "bash" | "zsh" => Lang::Bash,
            "xml" | "svg" | "plist" => Lang::Xml,
            "py" | "pyw" => Lang::Python,
            "log" | "out" | "err" => Lang::Log,
            "conf" | "cnf" => Lang::Nginx,
            "ini" | "service" | "cfg" | "reg" => Lang::Ini,
            "properties" => Lang::Properties,
            "md" | "markdown" => Lang::Markdown,
            _ => return None,
        })
    }

    /// 无扩展名时的文件名匹配(Dockerfile / .env / .gitignore 等)。
    pub(super) fn from_name(name_lower: &str) -> Option<Lang> {
        if name_lower == "dockerfile" || name_lower.starts_with("dockerfile.") {
            return Some(Lang::Dockerfile);
        }
        if name_lower == ".env" || name_lower.starts_with(".env.") {
            return Some(Lang::Env);
        }
        if name_lower == "crontab" {
            return Some(Lang::Ini);
        }
        None
    }

    /// shebang 兜底(无扩展名脚本):#!/bin/bash、#!/usr/bin/env python3 …
    fn from_shebang(first_line: &str) -> Option<Lang> {
        let l = first_line.trim();
        let rest = l.strip_prefix("#!")?;
        if rest.contains("bash") || rest.contains("/sh") || rest.ends_with("/sh") {
            return Some(Lang::Bash);
        }
        if rest.contains("python") {
            return Some(Lang::Python);
        }
        None
    }

    /// 识别入口:文件名 + 首行(可选,用于 shebang)。
    pub fn detect(file_name: &str, first_line: Option<&str>) -> Lang {
        let name_lower = file_name.to_lowercase();
        if let Some(l) = Self::from_name(&name_lower) {
            return l;
        }
        let ext = name_lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        if let Some(l) = Self::from_ext(ext) {
            return l;
        }
        if let Some(fl) = first_line {
            if let Some(l) = Self::from_shebang(fl) {
                return l;
            }
        }
        Lang::Text
    }

    /// 高亮引擎:tree-sitter(可编辑代码文件)还是行级规则。
    pub fn uses_tree_sitter(self) -> bool {
        matches!(
            self,
            Lang::Java
                | Lang::Sql
                | Lang::Json
                | Lang::Yaml
                | Lang::Toml
                | Lang::Bash
                | Lang::Xml
                | Lang::Python
        )
    }

    /// 状态栏显示名(与高保真稿状态栏一致)。
    pub fn display(self) -> &'static str {
        match self {
            Lang::Java => "Java",
            Lang::Sql => "SQL",
            Lang::Json => "JSON",
            Lang::Yaml => "YAML",
            Lang::Toml => "TOML",
            Lang::Bash => "Shell",
            Lang::Xml => "XML",
            Lang::Python => "Python",
            Lang::Log => "日志",
            Lang::Nginx => "nginx.conf",
            Lang::Ini => "INI",
            Lang::Properties => "Properties",
            Lang::Env => "Env",
            Lang::Dockerfile => "Dockerfile",
            Lang::Markdown => "Markdown",
            Lang::Text => "纯文本",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_by_ext() {
        assert_eq!(Lang::detect("a.JAVA", None), Lang::Java);
        assert_eq!(Lang::detect("orders.sql", None), Lang::Sql);
        assert_eq!(Lang::detect("x.yml", None), Lang::Yaml);
        assert_eq!(Lang::detect("a.SERVICE", None), Lang::Ini);
        assert_eq!(Lang::detect("app.log", None), Lang::Log);
    }

    #[test]
    fn detect_by_name_and_shebang() {
        assert_eq!(Lang::detect("Dockerfile", None), Lang::Dockerfile);
        assert_eq!(Lang::detect(".env", None), Lang::Env);
        assert_eq!(Lang::detect("run", Some("#!/usr/bin/env python3")), Lang::Python);
        assert_eq!(Lang::detect("go", Some("#!/bin/bash")), Lang::Bash);
        assert_eq!(Lang::detect("noext", None), Lang::Text);
    }

    #[test]
    fn engine_split() {
        assert!(Lang::Java.uses_tree_sitter());
        assert!(!Lang::Log.uses_tree_sitter());
        assert!(!Lang::Nginx.uses_tree_sitter());
    }
}
