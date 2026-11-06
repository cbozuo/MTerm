//! (#left-panel-v3-r2 2026-11-06 V3「定位当前终端」) 引擎半边：展开祖先分组。
//!
//! 滚动坐标的累计在桥层（对拍平后的行逐行套 `row_metrics`）；这里只负责
//! ConfigStore 侧的状态变更（展开 + 落盘语义由调用方 save）。

use super::row::{row_metrics, RowKind};
use crate::config::ConfigStore;

/// 展开目标会话所属分组链（自本组向上逐级）。返回**实际展开**的组数
/// （0 = 全部原本就展开 / 会话不存在）。只改内存态，落盘由桥层统一 save。
pub(crate) fn expand_ancestors(store: &mut ConfigStore, session_id: &str) -> usize {
    let mut gid = store
        .sessions()
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| s.group_id.clone())
        .unwrap_or_default();
    let mut expanded = 0usize;
    while !gid.is_empty() {
        let Some(record) = store.group_records().iter().find(|r| r.id == gid).cloned() else {
            break;
        };
        if record.collapsed {
            store.set_group_collapsed(&record.id, false);
            expanded += 1;
        }
        gid = record.parent_id;
    }
    expanded
}

/// (#tree-geom) 定位行的**中心 y**（列表内容坐标）：对拍平行序逐行累计
/// `row_metrics`（padding-top + 行高），命中目标会话行时取其半高。行高/间距
/// 与 Slint 侧行 wrapper 同一张表，两侧改任一侧必须同步。
/// (当前桥层遍历 Slint 模型 + row_metrics 自行累计;本函数供 TreeRow 路径与
/// 单测使用 —— #left-panel-v3-r3 可把桥层收敛到它。)
#[allow(dead_code)]
pub(crate) fn locate_center_y(rows: &[super::TreeRow], session_id: &str) -> Option<f32> {
    let mut y = 0.0f32;
    for (i, r) in rows.iter().enumerate() {
        let (pad, h) = row_metrics(r.row_kind_str(), r.depth);
        // 组头行的 GROUP_GAP 仅在 i > 0 时计入(首行贴顶),与 Slint 一致。
        let pad = if r.row_kind == RowKind::Group && i == 0 { 0.0 } else { pad };
        if r.row_kind == RowKind::Session && r.id == session_id {
            return Some(y + pad + h / 2.0);
        }
        y += pad + h;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_tree::build_tree_rows;
    use crate::session_tree::GroupSort;

    #[test]
    fn locate_center_y_accumulates_metrics() {
        let mut store = temp_locate_store();
        let rows = build_tree_rows(&store, &[], "", false, GroupSort::Storage);
        // 序: prod 头(0+30) → cn 头(6+27) → api 行(2+34)。
        let api = rows.iter().find(|r| r.id == "api-1").unwrap();
        assert_eq!(api.row_kind_str(), "session");
        let y = locate_center_y(&rows, "api-1").unwrap();
        assert!((y - (30.0 + 6.0 + 27.0 + 2.0 + 34.0 / 2.0)).abs() < 1e-4, "y={y}");
        assert_eq!(locate_center_y(&rows, "missing"), None);
    }

    #[test]
    fn expand_ancestors_opens_the_whole_chain_and_reports_changes() {
        let mut store = temp_locate_store();
        let cn_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "cn")
            .map(|r| r.id.clone())
            .unwrap();
        let prod_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "prod")
            .map(|r| r.id.clone())
            .unwrap();
        store.set_group_collapsed(&prod_id, true);
        store.set_group_collapsed(&cn_id, true);

        assert_eq!(expand_ancestors(&mut store, "api-1"), 2, "两级都展开");
        assert_eq!(expand_ancestors(&mut store, "api-1"), 0, "已展开再调 = 0");
        assert!(!store
            .group_records()
            .iter()
            .find(|r| r.id == prod_id)
            .unwrap()
            .collapsed);
        assert_eq!(expand_ancestors(&mut store, "ghost"), 0, "会话不存在 = 0");
    }

    fn temp_locate_store() -> ConfigStore {
        let dir = crate::test_tmp_dir("ms-locate-test");
        let mut store = ConfigStore {
            path: dir.join("config.json"),
            cache: crate::config::ConfigFile::default(),
            key: [7u8; 32],
        };
        store.add_group("prod".into(), "");
        let prod_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "prod")
            .map(|r| r.id.clone())
            .unwrap();
        store.add_group("cn".into(), &prod_id);
        let cn_id = store
            .group_records()
            .iter()
            .find(|r| r.name == "cn")
            .map(|r| r.id.clone())
            .unwrap();
        let mut s = crate::config::Session::new_empty();
        s.id = "api-1".into();
        s.name = "api".into();
        s.group_id = cn_id;
        store.upsert(s);
        store
    }
}
