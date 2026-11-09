//! (#left-panel-v3 2026-11-06) 会话分组树:组从「组名字符串」升级为一等实体。
//!
//! V3 左侧面板高保真把分组做成可嵌套的树(环境 › 区域 › 主机),组需要稳定
//! 的 id 才能在重命名/拖拽/着色时保持身份(组名做键时 rename 要连带迁移
//! 颜色/折叠/成员,是设计稿 P0① 点名的坑)。本模块定义 `GroupRecord` 与
//! 保留组常量;持久化在 ConfigFile.group_records,旧版单层组名快照
//! (`groups` / `collapsed_session_groups` / `group_colors`)由
//! `migrate_group_records` 一次性升级,save 时反向写回快照供降级读取。

use serde::{Deserialize, Serialize};

/// 保留组:未分组会话的兜底组(显示名「默认组」)。id 固定,不入用户组列表。
pub const GROUP_ID_DEFAULT: &str = "grp-default";

/// 保留组:内建本地终端(builtin sessions 恒归此组)。id 固定。
pub const GROUP_ID_SYSTEM: &str = "grp-system";

/// 一个会话分组节点。`parent_id` 为空 = 根级;同级顺序 = Vec 中的相对顺序。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupRecord {
    pub id: String,
    pub name: String,
    /// "" = 根级分组;否则指向父组 id(父组不存在时按根级渲染)。
    #[serde(default)]
    pub parent_id: String,
    /// 用户自定义组色 "#RRGGBB";None = 无自定义色(继承父组或主题默认)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 折叠状态(折叠的是本组的子层,组头行本身仍显示)。
    #[serde(default)]
    pub collapsed: bool,
    /// 保留组(default/system):不可删除、不可重命名、不可移入会话。
    #[serde(default)]
    pub reserved: bool,
}

impl GroupRecord {
    pub fn new(id: String, name: String, parent_id: String) -> Self {
        Self {
            id,
            name,
            parent_id,
            color: None,
            collapsed: false,
            reserved: false,
        }
    }

    /// 保留组的固定 record(default/system 迁移时补建)。
    pub fn reserved(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            parent_id: String::new(),
            color: None,
            collapsed: false,
            reserved: true,
        }
    }
}

/// 生成新的用户组 id:`grp-` + 8 位 hex(uuid 去连字符取前 8)。
pub fn new_group_id() -> String {
    format!("grp-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

/// 旧版单层组名(含会话推断出的隐式组)→ `Vec<GroupRecord>` 树。
/// `collapsed` / `colors` 来自旧字段;default/system 两条保留组固定补建。
/// 幂等:已是新格式(legacy 列表为空)时返回 None。
pub(crate) fn migrate_group_records(
    legacy_groups: &[String],
    legacy_collapsed: Option<&[String]>,
    legacy_colors: &std::collections::HashMap<String, String>,
    sessions_group_names: &[String],
) -> Option<Vec<GroupRecord>> {
    // 收集所有出现过的组名(显式列表 ∪ 会话引用),保持首现顺序。
    let mut names: Vec<String> = Vec::new();
    let mut push = |name: &str| {
        let name = name.trim();
        if name.is_empty() || is_reserved_session_group(name) {
            return;
        }
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    };
    legacy_groups.iter().for_each(|g| push(g));
    sessions_group_names.iter().for_each(|g| push(g));
    if names.is_empty() && legacy_colors.is_empty() {
        return None;
    }
    // 会话推断的组若与显式列表重名,只保留一条(上面 push 已去重)。
    let mut records: Vec<GroupRecord> = vec![
        GroupRecord::reserved(GROUP_ID_DEFAULT, "default"),
        GroupRecord::reserved(GROUP_ID_SYSTEM, "system"),
    ];
    let collapsed = |name: &str| {
        legacy_collapsed
            .map(|list| list.iter().any(|c| c == name))
            .unwrap_or(false)
    };
    for name in names {
        let id = new_group_id();
        records.push(GroupRecord {
            color: legacy_colors.get(&name).cloned(),
            collapsed: collapsed(&name),
            ..GroupRecord::new(id, name, String::new())
        });
    }
    Some(records)
}

/// 组名是否为保留名(default / system,大小写不敏感)。
/// (#group-name-guard 2026-11-09 用户反馈"组名写 default/默认/默认组不要再
/// 新建默认组") 中文显示名变体一并入列 —— 它们与保留组显示名撞车(树里
/// 出现两个「默认组」),任何建组入口都必须拦下:
///   default 侧: default / 默认 / 默认组 / 默认分组 / 默认值
///   system 侧: system / 本地终端 / 系统 / 系统组
/// (#left-panel-v3) 新代码应优先用 id 常量判断;本函数仍被旧字符串路径
/// (legacy 快照、显示名解析)使用。
pub fn is_reserved_session_group(name: &str) -> bool {
    reserved_session_group_id(name).is_some()
}

/// 保留名变体 → 保留组 id。None = 非保留名(普通组名)。
/// 大小写不敏感(ASCII);中文名精确匹配(trim 由调用方或本函数处理)。
pub fn reserved_session_group_id(name: &str) -> Option<&'static str> {
    let name = name.trim();
    if name.eq_ignore_ascii_case("default")
        || matches!(name, "默认" | "默认组" | "默认分组" | "默认值")
    {
        return Some(GROUP_ID_DEFAULT);
    }
    if name.eq_ignore_ascii_case("system")
        || matches!(name, "本地终端" | "系统" | "系统组")
    {
        return Some(GROUP_ID_SYSTEM);
    }
    None
}

#[cfg(test)]
mod group_record_tests {
    use super::*;

    #[test]
    fn migrate_uses_legacy_collapsed_and_colors() {
        let mut colors = std::collections::HashMap::new();
        colors.insert("生产环境".to_string(), "#F2A65A".to_string());
        let records = migrate_group_records(
            &["生产环境".to_string(), "预发布".to_string()],
            Some(&["预发布".to_string()]),
            &colors,
            &["开发测试".to_string()],
        )
        .expect("legacy groups must migrate");

        assert_eq!(records.len(), 5); // default + system + 3 用户组
        assert!(records[0].reserved && records[0].id == GROUP_ID_DEFAULT);
        assert!(records[1].reserved && records[1].id == GROUP_ID_SYSTEM);
        let prod = records.iter().find(|r| r.name == "生产环境").unwrap();
        assert_eq!(prod.color.as_deref(), Some("#F2A65A"));
        assert!(!prod.collapsed);
        let stage = records.iter().find(|r| r.name == "预发布").unwrap();
        assert!(stage.collapsed);
        // 会话推断组("开发测试")也入树。
        assert!(records.iter().any(|r| r.name == "开发测试"));
    }

    #[test]
    fn migrate_is_none_for_fresh_config() {
        assert!(migrate_group_records(&[], None, &std::collections::HashMap::new(), &[]).is_none());
    }

    #[test]
    fn reserved_names_are_filtered_from_user_groups() {
        let records = migrate_group_records(
            &["default".to_string(), "自定义".to_string()],
            None,
            &std::collections::HashMap::new(),
            &[],
        )
        .unwrap();
        // "default" 是保留名,不生成用户组 record。
        assert_eq!(records.len(), 3);
        assert!(records.iter().filter(|r| !r.reserved).all(|r| r.name == "自定义"));
    }

    #[test]
    fn new_group_id_shape() {
        let id = new_group_id();
        assert!(id.starts_with("grp-"));
        assert_eq!(id.len(), "grp-".len() + 8);
    }
}
