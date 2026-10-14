/// 启动模式：二进制按 argv 分发到 GUI / CLI / MCP / 版本打印四条路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartMode {
    /// `MTerm mcp serve` —— MCP stdio 服务器（供外部 Agent 调用）。
    Mcp,
    /// `MTerm cli <command>` —— 面向脚本的命令行。
    Cli,
    /// 默认 —— GUI 主窗口。
    App,
    /// `--version` / `-V` —— 打印版本后立即退出，零副作用。
    Version,
}

impl StartMode {
    /// 依据 argv 判定启动模式。
    ///
    /// 只看头部子命令与全局 `--version` 旗标；其余参数解析由各分支
    /// 自己负责（GUI 走 `app::launch::parse`，CLI 走 `cli::run`）。
    pub(crate) fn detect(args: &[String]) -> Self {
        match args.get(1).map(String::as_str) {
            Some("mcp") if args.get(2).is_some_and(|arg| arg == "serve") => Self::Mcp,
            Some("cli") => Self::Cli,
            _ if args.iter().any(|arg| arg == "--version" || arg == "-V") => Self::Version,
            _ => Self::App,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_start_mode() {
        let mcp = vec![
            "MTerm".to_string(),
            "mcp".to_string(),
            "serve".to_string(),
        ];
        assert!(matches!(StartMode::detect(&mcp), StartMode::Mcp));

        let cli = vec![
            "MTerm".to_string(),
            "cli".to_string(),
            "sessions".to_string(),
        ];
        assert!(matches!(StartMode::detect(&cli), StartMode::Cli));

        let version = vec!["MTerm".to_string(), "--version".to_string()];
        assert!(matches!(StartMode::detect(&version), StartMode::Version));

        let app = vec!["MTerm".to_string()];
        assert!(matches!(StartMode::detect(&app), StartMode::App));
    }
}
