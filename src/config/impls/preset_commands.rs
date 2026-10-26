//! 内置预置快捷命令（#preset-commands 2026-10-26 / #preset-groups 2026-10-26）。
//!
//! 出厂清单全部是**只读/诊断类**高频命令，主流 Linux 可直接跑；刻意不放
//! 变更类操作（重启/删除），也不绑定具体业务（nginx/mysql）与硬件
//! （nvidia-smi）。参数化命令（ping）用 `send_enter = false`：点击落入
//! 输入框，补全目标后自行回车 —— 顺带演示「填入」这一档发送方式。
//!
//! **按用途分组成 4 个分类**（#preset-groups）：全部塞进一个「预设」组会让
//! 用户的分组栏只剩一个巨型分类、卡片上也只能看到同一个 chip，既难扫读也
//! 挤占了用户自己的分组位。分类名就是普通分组名（可改名/改色/删除），颜色
//! 取自 `ACCENT_PRESETS` 里的固定几档，深浅主题下都够对比。
//!
//! 共三个注入点，全部复用本表，互不重跑：
//!  · 新装用户：`fresh_config()` 直接种子（defaults_rev 出厂即最新，
//!    不会再进 migrate_defaults 的分支）；
//!  · 存量用户：`migrate_defaults`（rev 8 = 空表注入 / rev 9 = 老「预设」组
//!    按分类拆开）；
//!  · 管理器空态「从预置导入」：`ConfigStore::import_presets()`，按命令
//!    文本去重，任何时候可恢复。
//!
//! 名称/备注在**写入配置时**按当前语言定格（QuickCommand 存的是纯文本），
//! 之后切换界面语言不改写已存条目 —— 与用户自建命令同一待遇。

use super::{ConfigFile, QuickCommand};
use crate::i18n::t;
use std::collections::HashMap;

/// 一条出厂命令。
pub(crate) struct PresetItem {
    pub name_zh: &'static str,
    pub name_en: &'static str,
    pub command: &'static str,
    pub note_zh: &'static str,
    pub note_en: &'static str,
    /// true = 点一下填入并回车执行；false = 只填入（参数待补全）。
    pub send_enter: bool,
}

/// 一个出厂分类（= 一个快捷命令分组，自带组色）。
pub(crate) struct PresetGroup {
    pub name_zh: &'static str,
    pub name_en: &'static str,
    /// 组色 `#RRGGBB`。取值全部来自 `theme::palettes::ACCENT_PRESETS`
    /// （琥珀橙 / 水色 / 翡翠绿 / 薰衣草色），不另造色。
    pub color: &'static str,
    pub items: &'static [PresetItem],
}

const fn item(
    name_zh: &'static str,
    name_en: &'static str,
    command: &'static str,
    note_zh: &'static str,
    note_en: &'static str,
    send_enter: bool,
) -> PresetItem {
    PresetItem { name_zh, name_en, command, note_zh, note_en, send_enter }
}

pub(crate) static PRESET_GROUPS: &[PresetGroup] = &[
    PresetGroup {
        name_zh: "磁盘与内存",
        name_en: "Storage & memory",
        color: "#FF9B0B",
        items: &[
            item("磁盘占用", "Disk usage", "df -h", "各挂载点容量与使用率", "Capacity per mount point", true),
            item(
                "目录占用",
                "Dir sizes",
                "du -h --max-depth=1 . | sort -rh | head -20",
                "当前目录下最占空间的 Top 20",
                "Top 20 under current directory",
                true,
            ),
            item("内存使用", "Memory", "free -h", "内存与交换分区", "RAM and swap", true),
        ],
    },
    PresetGroup {
        name_zh: "网络排查",
        name_en: "Network",
        color: "#42A5F5",
        items: &[
            item("网卡地址", "NIC addresses", "ip -br addr", "IP 与状态一行式", "One line per interface", true),
            item("监听端口", "Listening ports", "ss -tlnp", "哪个进程监听哪个端口", "Which process listens where", true),
            item("连通测试", "Ping", "ping -c 4 ", "补全目标地址再回车", "Fill the host, then press Enter", false),
        ],
    },
    PresetGroup {
        name_zh: "服务与日志",
        name_en: "Services & logs",
        color: "#29A383",
        items: &[
            item(
                "运行中服务",
                "Running services",
                "systemctl list-units --type=service --state=running",
                "systemd 服务一览",
                "systemd units overview",
                true,
            ),
            item("异常服务", "Failed services", "systemctl --failed", "启动失败的服务", "Units that failed to start", true),
            item("跟踪系统日志", "Follow journal", "journalctl -f -n 100", "实时滚动系统日志", "Stream the system journal", true),
            item("内核消息", "Kernel messages", "dmesg | tail -50", "最近的内核告警", "Recent kernel messages", true),
        ],
    },
    PresetGroup {
        name_zh: "进程与容器",
        name_en: "Processes & containers",
        color: "#7E57C2",
        items: &[
            item("实时进程", "Top processes", "top", "按 CPU / 内存排序", "Sorted by CPU / memory", true),
            item("容器列表", "Containers", "docker ps -a", "全部容器及状态", "All containers and their status", true),
        ],
    },
];

/// rev 8 之前（以及 2026-10-26 当天装上的版本）预置命令共用的那一个组名。
/// 只给迁移用 —— 现在的出厂清单按 [`PRESET_GROUPS`] 分类，不再有「预设」组。
pub(crate) fn legacy_preset_group_names() -> [&'static str; 2] {
    ["预设", "Presets"]
}

fn group_name(g: &PresetGroup) -> String {
    t(g.name_zh, g.name_en).to_string()
}

/// 出厂预置清单（扁平序 = 分类顺序 × 组内顺序）。
pub(crate) fn preset_commands() -> Vec<QuickCommand> {
    let mut out = Vec::new();
    for g in PRESET_GROUPS {
        let group = group_name(g);
        for it in g.items {
            out.push(QuickCommand {
                name: t(it.name_zh, it.name_en).to_string(),
                command: it.command.to_string(),
                group: group.clone(),
                note: t(it.note_zh, it.note_en).to_string(),
                send_enter: it.send_enter,
            });
        }
    }
    out
}

/// 出厂分组清单（有序）与各自的组色，按当前语言定格。
pub(crate) fn preset_group_seed() -> (Vec<String>, HashMap<String, String>) {
    let mut names = Vec::with_capacity(PRESET_GROUPS.len());
    let mut colors = HashMap::with_capacity(PRESET_GROUPS.len());
    for g in PRESET_GROUPS {
        let name = group_name(g);
        colors.insert(name.clone(), g.color.to_string());
        names.push(name);
    }
    (names, colors)
}

/// `defaults_rev` 9：把老版本种下的「预设」单组按分类拆开。
///
/// 只动**仍是出厂命令原文**的条目（用户改过的命令/自建条目一条不碰），
/// 分类组按出厂顺序插到旧组原来的位置上，旧组清空后删除、还有用户自己的
/// 条目则原样保留。返回是否改动了配置。
pub(crate) fn regroup_legacy_presets(cfg: &mut ConfigFile) -> bool {
    let legacy = legacy_preset_group_names();
    let is_legacy = |group: &str| legacy.iter().any(|l| *l == group);
    let has_legacy_group = cfg.quick_groups.iter().any(|g| is_legacy(g))
        || cfg.quick_commands.iter().any(|c| is_legacy(&c.group));
    if !has_legacy_group {
        return false;
    }

    // 命令原文 → 分类下标。
    let mut by_command: HashMap<&'static str, usize> = HashMap::new();
    for (gi, g) in PRESET_GROUPS.iter().enumerate() {
        for it in g.items {
            by_command.insert(it.command.trim(), gi);
        }
    }

    let mut moved = false;
    for c in cfg.quick_commands.iter_mut() {
        if !is_legacy(&c.group) {
            continue;
        }
        if let Some(&gi) = by_command.get(c.command.trim()) {
            let name = group_name(&PRESET_GROUPS[gi]);
            if c.group != name {
                c.group = name;
                moved = true;
            }
        }
    }
    if !moved {
        return false;
    }

    // 分类组插到旧组原位置（用户排过的其他分组相对顺序不变）。旧组若还有
    // 用户自己的条目就保留（排在分类组之后）—— 不然那几个条目会指向一个
    // 不存在的组，左栏直接消失。
    let legacy_still_used = cfg.quick_commands.iter().any(|c| is_legacy(&c.group));
    let mut groups: Vec<String> = Vec::with_capacity(cfg.quick_groups.len() + PRESET_GROUPS.len());
    let mut placed = false;
    for g in std::mem::take(&mut cfg.quick_groups) {
        if is_legacy(&g) {
            if !placed {
                for pg in PRESET_GROUPS {
                    let name = group_name(pg);
                    if !groups.contains(&name) {
                        groups.push(name);
                    }
                }
                placed = true;
            }
            if legacy_still_used && !groups.contains(&g) {
                groups.push(g);
            }
        } else if !groups.contains(&g) {
            groups.push(g);
        }
    }
    cfg.quick_groups = groups;
    for (name, hex) in preset_group_seed().1 {
        cfg.group_colors.entry(name).or_insert(hex);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_seed_matches_flat_command_list() {
        let (names, colors) = preset_group_seed();
        assert_eq!(names.len(), PRESET_GROUPS.len());
        let cmds = preset_commands();
        assert_eq!(cmds.len(), PRESET_GROUPS.iter().map(|g| g.items.len()).sum::<usize>());
        // 每条命令的组名都在清单里，且组色齐备。
        for c in &cmds {
            assert!(names.contains(&c.group), "未登记的组名：{}", c.group);
            assert!(colors.get(&c.group).is_some_and(|h| h.starts_with('#') && h.len() == 7));
        }
        // 分组语义：一组不能只有 0 条，也不能全部挤在同一个组里。
        assert!(names.len() > 1);
    }

    #[test]
    fn regroup_moves_only_pristine_presets() {
        let mut cfg = ConfigFile::default();
        cfg.quick_groups = vec!["预设".into(), "我的组".into()];
        cfg.quick_commands = preset_commands();
        // rev 8 装上的样子：出厂条目全在同一个「预设」组里。
        for c in &mut cfg.quick_commands {
            c.group = "预设".into();
        }
        // 用户自己加的一条，混在「预设」组里。
        cfg.quick_commands.push(QuickCommand {
            name: "自定义".into(),
            command: "uptime".into(),
            group: "预设".into(),
            note: String::new(),
            send_enter: true,
        });
        // 再改掉一条预置命令的原文 → 该条不该被搬走。
        let edited = cfg
            .quick_commands
            .iter_mut()
            .find(|c| c.command == "free -h")
            .expect("预置里应有 free -h");
        edited.command = "free -m".into();

        assert!(regroup_legacy_presets(&mut cfg));

        let groups: Vec<&str> = cfg.quick_commands.iter().map(|c| c.group.as_str()).collect();
        assert!(groups.iter().any(|g| *g == "磁盘与内存"));
        assert!(groups.iter().any(|g| *g == "网络排查"));
        assert!(groups.iter().any(|g| *g == "服务与日志"));
        assert!(groups.iter().any(|g| *g == "进程与容器"));
        // 用户条目与改过命令的条目仍留在旧组 → 旧组不能删。
        assert_eq!(cfg.quick_commands.last().unwrap().group, "预设");
        let free = cfg.quick_commands.iter().find(|c| c.command == "free -m").unwrap();
        assert_eq!(free.group, "预设");
        assert!(cfg.quick_groups.iter().any(|g| g == "预设"));
        assert!(cfg.quick_groups.iter().any(|g| g == "我的组"));
        // 分类组插在旧组原位置之前（旧组在第 0 位 → 分类组排到最前）。
        assert_eq!(cfg.quick_groups[0], "磁盘与内存");
        assert_eq!(cfg.quick_groups[PRESET_GROUPS.len()], "预设");
        // 组色落盘。
        assert_eq!(cfg.group_colors.get("磁盘与内存").map(String::as_str), Some("#FF9B0B"));
    }

    #[test]
    fn regroup_drops_the_legacy_group_when_empty() {
        let mut cfg = ConfigFile::default();
        cfg.quick_groups = vec!["预设".into()];
        cfg.quick_commands = preset_commands();
        for c in &mut cfg.quick_commands {
            c.group = "预设".into();
        }
        assert!(regroup_legacy_presets(&mut cfg));
        assert!(!cfg.quick_groups.iter().any(|g| g == "预设"));
        assert_eq!(cfg.quick_groups.len(), PRESET_GROUPS.len());
        // 幂等：再跑一次没有旧组可拆。
        assert!(!regroup_legacy_presets(&mut cfg));
    }
}
