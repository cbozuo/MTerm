//! (#double-shift-v1 2026-11-11) 设置项与全局动作的**静态清单**。
//!
//! 设置项映射 settings 弹框的 `ifd-page` 页名（`ui/app.slint:1003` 起的全集，
//! 见 `ui/interface_panel.slint:1193` 导航）；动作映射 `src/app/search_ui.rs`
//! 的执行分派键。中英文与拼音别名手工维护（动态数据——会话/命令——不做拼音，
//! 只吃子序列匹配；这里清单是静态的，别名一次写死）。
//!
//! 文案走 `i18n::t(zh, en)`（原子语言变量），同 `src/command_hint/struct/model.rs` 口径。

use crate::i18n::t;
use super::model::{Category, Payload, SearchItem};

/// 静态清单条目构造的公共字段（减参数版）。
struct Def {
    key: &'static str,
    title_zh: &'static str,
    title_en: &'static str,
    sub_zh: &'static str,
    sub_en: &'static str,
    desc_zh: &'static str,
    desc_en: &'static str,
    /// 检索别名（英文词 + 全拼 + 首字母），空格分隔；构造时并入 keywords。
    aliases: &'static str,
}

impl Def {
    fn item(&self, id_prefix: &str, cat: Category, payload: Payload) -> SearchItem {
        let title = t(self.title_zh, self.title_en).to_string();
        let sub = t(self.sub_zh, self.sub_en).to_string();
        let desc = t(self.desc_zh, self.desc_en).to_string();
        let cat_word = match cat {
            Category::Setting => "setting settings preference 设置 shezhi sz",
            _ => "action actions 操作 dongzuo dz",
        };
        let keywords = format!("{} {} {} {}", self.title_zh, self.title_en, self.aliases, cat_word);
        SearchItem::new(
            format!("{id_prefix}:{}", self.key),
            cat,
            title,
            sub,
            desc,
            keywords,
            false,
            0,
            payload,
        )
    }
}

/// 设置项清单（标题 / 路径 / 别名 / `ifd-page`）。
///
/// 路径两级 = 页名 › 项名，与 `ui/interface_panel.slint` 导航一致；项级定位
/// v1 不做（SettingRow 无 id），Enter 打开到对应页。
pub(crate) fn setting_items() -> Vec<SearchItem> {
    const DEFS: &[Def] = &[
        Def {
            key: "theme", title_zh: "主题", title_en: "Theme",
            sub_zh: "外观 › 主题", sub_en: "Appearance › Theme",
            desc_zh: "切换 MTerm 的整体配色方案，38 套预设即时生效。",
            desc_en: "Switch the overall color scheme of MTerm.",
            aliases: "theme color scheme dark light 暗色 浅色 zhuti zt",
        },
        Def {
            key: "accent", title_zh: "强调色", title_en: "Accent color",
            sub_zh: "外观 › 强调色", sub_en: "Appearance › Accent",
            desc_zh: "强调色跟随主题，或从预设色板与取色盘中自定。",
            desc_en: "Accent color: follow the theme or pick a custom one.",
            aliases: "accent color 强调 颜色 qiangdiao qd color picker",
        },
        Def {
            key: "wallpaper", title_zh: "壁纸", title_en: "Wallpaper",
            sub_zh: "外观 › 壁纸", sub_en: "Appearance › Wallpaper",
            desc_zh: "主界面壁纸与透明度、填充方式。",
            desc_en: "Main-window wallpaper, transparency and fill mode.",
            aliases: "wallpaper background 壁纸 背景 bizhi bz",
        },
        Def {
            key: "ui-font", title_zh: "界面字体", title_en: "Interface font",
            sub_zh: "显示与字体 › 界面字体", sub_en: "Display & Fonts › Interface font",
            desc_zh: "界面 UI 使用的字体；等宽链兜底 Consolas。",
            desc_en: "Font used by the UI chrome.",
            aliases: "ui font family typeface 界面 字体 jiemian ziti jmzt",
        },
        Def {
            key: "ui-scale", title_zh: "界面缩放", title_en: "UI scale",
            sub_zh: "显示与字体 › 界面缩放", sub_en: "Display & Fonts › UI scale",
            desc_zh: "整体界面缩放比例。",
            desc_en: "Global interface scale.",
            aliases: "ui scale zoom 界面 缩放 放大 jiemian suofang jmsf",
        },
        Def {
            key: "term-font", title_zh: "终端字体", title_en: "Terminal font",
            sub_zh: "显示与字体 › 终端字体", sub_en: "Display & Fonts › Terminal font",
            desc_zh: "终端正文使用的等宽字体与字号。",
            desc_en: "Monospace font used by the terminal body.",
            aliases: "terminal font mono monospace 终端 字体 zhongduan ziti zdzt",
        },
        Def {
            key: "cursor", title_zh: "光标类型", title_en: "Cursor shape",
            sub_zh: "显示与字体 › 光标类型", sub_en: "Display & Fonts › Cursor shape",
            desc_zh: "终端输入光标的形状：块状 / 竖线 / 下划线。",
            desc_en: "Terminal cursor shape: block, bar or underline.",
            aliases: "cursor caret shape 光标 类型 guangbiao gb",
        },
        Def {
            key: "paste-confirm", title_zh: "确认多行粘贴", title_en: "Confirm multiline paste",
            sub_zh: "输入 › 确认多行粘贴", sub_en: "Input › Confirm multiline paste",
            desc_zh: "粘贴包含换行的内容时先弹出确认，避免误执行多条命令。",
            desc_en: "Ask before pasting content with newlines.",
            aliases: "paste clipboard multiline 粘贴 多行 确认 zhantie duohang ztqr",
        },
        Def {
            key: "renderer", title_zh: "渲染器", title_en: "Renderer",
            sub_zh: "渲染 › 渲染器", sub_en: "Rendering › Renderer",
            desc_zh: "软件渲染 / GPU 渲染的切换。",
            desc_en: "Switch between software and GPU rendering.",
            aliases: "renderer rendering gpu software 渲染 器 xuanranqi xrq",
        },
        Def {
            key: "highlight", title_zh: "高亮规则", title_en: "Highlight rules",
            sub_zh: "输出高亮 › 高亮规则", sub_en: "Output › Highlight rules",
            desc_zh: "终端输出的关键词高亮规则与颜色。",
            desc_en: "Keyword highlight rules and colors for terminal output.",
            aliases: "highlight rules 输出 高亮 规则 shuchu gaoliang scgl",
        },
        Def {
            key: "sftp-cd", title_zh: "SFTP 跟随 cd", title_en: "SFTP follows cd",
            sub_zh: "SFTP › SFTP 跟随 cd 更新", sub_en: "SFTP › SFTP follows cd",
            desc_zh: "终端 cd 后 SFTP 面板自动跟随刷新目录。",
            desc_en: "SFTP panel follows terminal cd automatically.",
            aliases: "sftp cd follow 目录 跟随 genjun gjsftp",
        },
        Def {
            key: "zen", title_zh: "专注模式", title_en: "Zen mode",
            sub_zh: "侧栏 › 专注模式", sub_en: "Sidebars › Zen mode",
            desc_zh: "隐藏侧栏与标签栏，只留终端（Ctrl+Alt+Z 切换）。",
            desc_en: "Hide sidebars and the tab strip, terminal only.",
            aliases: "zen mode focus 专注 禅 模式 chanzhong zhuanzhu zzms",
        },
        Def {
            key: "download", title_zh: "下载目录", title_en: "Download directory",
            sub_zh: "下载 › 下载目录", sub_en: "Download › Download directory",
            desc_zh: "SFTP 下载文件的默认落盘位置。",
            desc_en: "Default location for SFTP downloads.",
            aliases: "download directory folder path 下载 目录 xiazai mulu xzml",
        },
        Def {
            key: "data-dir", title_zh: "数据目录", title_en: "Data directory",
            sub_zh: "数据目录 › 位置", sub_en: "Data directory › Location",
            desc_zh: "配置与日志的存放位置，切换后重启生效。",
            desc_en: "Where config and logs live; change takes effect after restart.",
            aliases: "data directory storage 数据 目录 位置 shuju mulu sjml",
        },
    ];
    DEFS.iter()
        .map(|d| {
            d.item(
                "set",
                Category::Setting,
                Payload::Setting { page: page_of(d.key), key: d.key },
            )
        })
        .collect()
}

/// 设置项 → settings 弹框页名（`ifd-page` 全集见 app.slint:1003）。
pub(crate) fn page_of(key: &str) -> &'static str {
    match key {
        "theme" | "accent" | "wallpaper" => "theme",
        "ui-font" | "ui-scale" | "term-font" | "cursor" => "font",
        "paste-confirm" => "input",
        "renderer" => "renderer",
        "highlight" => "output",
        "sftp-cd" => "sftp",
        "zen" => "sidebar",
        "download" => "download",
        "data-dir" => "storage",
        _ => "theme",
    }
}

/// 全局动作清单（`act` = `src/app/search_ui.rs` 执行分派键）。
pub(crate) fn action_items() -> Vec<SearchItem> {
    const DEFS: &[(&str, Def)] = &[
        ("new-window", Def {
            key: "new-window", title_zh: "新建窗口", title_en: "New window",
            sub_zh: "全局动作 · Ctrl+Shift+N", sub_en: "Action · Ctrl+Shift+N",
            desc_zh: "打开一个新的 MTerm 主窗口。",
            desc_en: "Open another MTerm window.",
            aliases: "new window create window 新建 窗口 xinjian chuangkou xjck",
        }),
        ("new-session", Def {
            key: "new-session", title_zh: "新建会话", title_en: "New session",
            sub_zh: "全局动作 · Ctrl+N", sub_en: "Action · Ctrl+N",
            desc_zh: "打开新建会话向导。",
            desc_en: "Open the new-session dialog.",
            aliases: "new session create connection 新建 会话 连接 xinjian huihua xjhh",
        }),
        ("toggle-sidebar", Def {
            key: "toggle-sidebar", title_zh: "切换左侧栏", title_en: "Toggle left sidebar",
            sub_zh: "全局动作 · Ctrl+B", sub_en: "Action · Ctrl+B",
            desc_zh: "收起或展开左侧会话栏，为终端让出空间。",
            desc_en: "Collapse or expand the session sidebar.",
            aliases: "toggle sidebar left panel 切换 左侧 栏 qiehuan zuoce qhzcl",
        }),
        ("toggle-right", Def {
            key: "toggle-right", title_zh: "切换右侧栏", title_en: "Toggle right sidebar",
            sub_zh: "全局动作 · Ctrl+Alt+B", sub_en: "Action · Ctrl+Alt+B",
            desc_zh: "收起或展开右侧资源监控栏。",
            desc_en: "Collapse or expand the resource monitor.",
            aliases: "toggle right sidebar resource monitor 切换 右侧 资源 qiehuan youce qhycl",
        }),
        ("zen", Def {
            key: "zen", title_zh: "专注模式", title_en: "Zen mode",
            sub_zh: "全局动作 · Ctrl+Alt+Z", sub_en: "Action · Ctrl+Alt+Z",
            desc_zh: "隐藏侧栏、标签栏与资源栏，只留终端。",
            desc_en: "Hide sidebars, tab strip and monitor; terminal only.",
            aliases: "zen mode focus 专注 禅 chanzhong zhuanzhu zz",
        }),
        ("toggle-theme", Def {
            key: "toggle-theme", title_zh: "切换主题", title_en: "Toggle theme",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "在深色与浅色主题之间切换。",
            desc_en: "Switch between dark and light theme.",
            aliases: "toggle theme dark light 切换 主题 qiehuan zhuti qhzt",
        }),
        ("font-inc", Def {
            key: "font-inc", title_zh: "字体放大", title_en: "Increase font size",
            sub_zh: "全局动作 · Ctrl+=", sub_en: "Action · Ctrl+=",
            desc_zh: "将当前会话终端字体放大一级。",
            desc_en: "Zoom the active terminal font in.",
            aliases: "font increase zoom in bigger 字体 放大 ziti fangda ztfd",
        }),
        ("font-dec", Def {
            key: "font-dec", title_zh: "字体缩小", title_en: "Decrease font size",
            sub_zh: "全局动作 · Ctrl+-", sub_en: "Action · Ctrl+-",
            desc_zh: "将当前会话终端字体缩小一级。",
            desc_en: "Zoom the active terminal font out.",
            aliases: "font decrease zoom out smaller 字体 缩小 ziti suoxiao ztsx",
        }),
        ("font-reset", Def {
            key: "font-reset", title_zh: "重置字体", title_en: "Reset font size",
            sub_zh: "全局动作 · Ctrl+0", sub_en: "Action · Ctrl+0",
            desc_zh: "将终端字体恢复到默认大小。",
            desc_en: "Reset the terminal font to default.",
            aliases: "font reset default zoom reset 重置 字体 chongzhi ziti czzt",
        }),
        ("open-settings", Def {
            key: "open-settings", title_zh: "打开设置", title_en: "Open settings",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "打开 MTerm 设置窗口。",
            desc_en: "Open the MTerm settings dialog.",
            aliases: "open settings preferences 打开 设置 dakai shezhi dksz",
        }),
        ("open-command-manager", Def {
            key: "open-command-manager", title_zh: "打开命令管理", title_en: "Open command manager",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "管理快捷命令与命令分组。",
            desc_en: "Manage quick commands and groups.",
            aliases: "command manager quick commands 命令 管理 mingling guanli mlgl",
        }),
        ("open-send-window", Def {
            key: "open-send-window", title_zh: "打开发送命令窗口", title_en: "Open send-command window",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "向一个或多个会话批量发送命令。",
            desc_en: "Broadcast commands to one or more sessions.",
            aliases: "send command broadcast multiple 发送 命令 窗口 fasong mingling fsmlck",
        }),
        ("open-history", Def {
            key: "open-history", title_zh: "打开命令历史", title_en: "Open command history",
            sub_zh: "全局动作 · Ctrl+R", sub_en: "Action · Ctrl+R",
            desc_zh: "在面板中浏览并重新执行历史命令。",
            desc_en: "Browse and re-run past commands.",
            aliases: "command history log 历史 命令 lishi mingling lsml",
        }),
        ("terminal-find", Def {
            key: "terminal-find", title_zh: "终端查找", title_en: "Terminal find",
            sub_zh: "全局动作 · Ctrl+F", sub_en: "Action · Ctrl+F",
            desc_zh: "在当前终端缓冲区中查找文本。",
            desc_en: "Search the current terminal buffer.",
            aliases: "terminal find search text 终端 查找 搜索 zhongduan chazhao zdcz",
        }),
        ("ping", Def {
            key: "ping", title_zh: "Ping 检测", title_en: "Ping check",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "对主机发起连通性检测。",
            desc_en: "Run a connectivity check against a host.",
            aliases: "ping test connectivity 检测 连通 测试 ceshi pltcs",
        }),
        ("system-info", Def {
            key: "system-info", title_zh: "系统信息", title_en: "System information",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "查看主机的系统、内核与资源信息。",
            desc_en: "System, kernel and resource information.",
            aliases: "system info information 系统 信息 xitong xinxi xtxx",
        }),
        ("processes", Def {
            key: "processes", title_zh: "进程管理", title_en: "Process manager",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "打开远程进程列表与终止操作。",
            desc_en: "Remote process list and kill actions.",
            aliases: "process manager task list 进程 管理 jincheng guanli jcgl",
        }),
        ("about", Def {
            key: "about", title_zh: "关于 MTerm", title_en: "About MTerm",
            sub_zh: "全局动作 · 应用内执行", sub_en: "Action · in-app",
            desc_zh: "查看版本、构建信息与开源许可。",
            desc_en: "Version, build info and licenses.",
            aliases: "about version build license 关于 版本 guanyu banben gybb",
        }),
    ];
    DEFS.iter()
        .map(|(act, d)| {
            let shortcut = d
                .sub_zh
                .strip_prefix("全局动作 · ")
                .filter(|s| !s.is_empty() && *s != "应用内执行")
                .unwrap_or("");
            d.item(
                "act",
                Category::Action,
                Payload::Action { act, shortcut },
            )
        })
        .collect()
}
