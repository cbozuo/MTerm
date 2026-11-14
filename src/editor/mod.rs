// (#editor-hifi 2026-11-15) 内置文本编辑器领域层。高保真稿:
// docs/design/sftp-editor-hifi-2026-11-14.html(落地映射表)。
// 数据结构 = ropey rope + 可见窗口模型(全文不过 Slint 边界);
// 高亮 = tree-sitter(可编辑代码文件,窗口增量)+ 行级规则(日志/行式配置,零依赖);
// 其余:多 tab 容器 / 查找替换 / 格式化(纯 Rust 生态边界见设计稿矩阵)/ 编码检测。
#[path = "struct/lang.rs"]
pub(crate) mod lang;
#[path = "struct/highlight.rs"]
pub(crate) mod highlight;
#[path = "struct/doc.rs"]
pub(crate) mod doc;
#[path = "struct/tabs.rs"]
pub(crate) mod tabs;
#[path = "impls/doc_impls.rs"]
pub(crate) mod doc_impls;
#[path = "impls/highlight_line.rs"]
pub(crate) mod highlight_line;
#[path = "impls/highlight_ts.rs"]
pub(crate) mod highlight_ts;
#[path = "impls/find.rs"]
pub(crate) mod find;
#[path = "impls/format.rs"]
pub(crate) mod format;
#[path = "impls/encode.rs"]
pub(crate) mod encode;

pub(crate) use doc::*;
pub(crate) use highlight::*;
pub(crate) use lang::*;
pub(crate) use tabs::*;
