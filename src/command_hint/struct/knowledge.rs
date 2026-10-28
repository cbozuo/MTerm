//! 内置命令知识表（只收最常用的几条，不做 man page）。
//!
//! 独立成文件的原因：这是**数据**，会随产品需要增删，和「怎么算候选」的
//! 逻辑没有关系。改表不该碰排序，改排序也不该碰表。

/// 内置子命令表（P0 用）。只收**最常用**的几个命令 —— 提示框的价值在
/// 「常见的补得准」，不在「收全 man page」；收全了反而让候选排序变差。
///
/// 返回 `None` 表示这个命令没有子命令概念（`df`、`free`、自建脚本…），
/// 调用方据此把第二段判成「参数」而不是「子命令」。
pub(crate) fn subcommands_for(command: &str) -> Option<&'static [(&'static str, &'static str)]> {
    let subs: &'static [(&'static str, &'static str)] = match command {
        "git" => &[
            ("status", "查看工作区状态"),
            ("commit", "提交暂存区改动"),
            ("push", "推送本地分支"),
            ("pull", "拉取并合并"),
            ("log", "查看提交历史"),
            ("diff", "对比差异"),
            ("checkout", "切换分支或还原文件"),
            ("branch", "管理分支"),
            ("rebase", "变基"),
            ("reset", "回退提交"),
            ("stash", "暂存未提交改动"),
            ("fetch", "只取不合并"),
            ("merge", "合并分支"),
            ("remote", "管理远端"),
            ("clone", "克隆仓库"),
        ],
        "systemctl" => &[
            ("status", "查看服务状态"),
            ("start", "启动服务"),
            ("stop", "停止服务"),
            ("restart", "重启服务"),
            ("enable", "设为开机自启"),
            ("disable", "取消自启"),
            ("reload", "重新加载配置"),
            ("list-units", "列出单元"),
        ],
        "docker" => &[
            ("ps", "列出容器"),
            ("images", "列出镜像"),
            ("logs", "查看容器日志"),
            ("exec", "进入容器执行"),
            ("run", "新建并启动容器"),
            ("rm", "删除容器"),
            ("rmi", "删除镜像"),
            ("build", "构建镜像"),
            ("compose", "编排多容器"),
        ],
        "kubectl" => &[
            ("get", "列出资源"),
            ("describe", "查看详情"),
            ("logs", "查看 Pod 日志"),
            ("exec", "进入容器"),
            ("apply", "应用清单"),
            ("delete", "删除资源"),
        ],
        "killall" => &[("nginx", "按名字结束进程")],
        _ => return None,
    };
    Some(subs)
}
