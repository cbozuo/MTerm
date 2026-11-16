// (#editor-native 2026-11-16) 内置文本编辑器领域层。高保真稿:
// docs/design/sftp-editor-hifi-2026-11-14.html(落地映射表)。
//
// 架构基线(对齐 yituorou/meatshell 的原生编辑器,见 vendor/EDITOR_PATCH.md):
// 文本由 Slint 的 TextInput 原生渲染/编辑(shaping、光标、选择、IME、撤销全原生),
// 本层只负责「语言判定 + 词法着色(全文字节区间)+ 查找(字节区间)+ 格式化 + 编码」。
// 旧版 rope + 可见行窗口 + 手搓等宽网格已退役:等宽主字体无 CJK 字形、中文回退
// 比例字体前进量≠2 格,网格定位必然漂移。
#[path = "struct/lang.rs"]
pub(crate) mod lang;
#[path = "struct/highlight.rs"]
pub(crate) mod highlight;
#[path = "struct/tabs.rs"]
pub(crate) mod tabs;
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

pub(crate) use highlight::*;
pub(crate) use lang::*;
pub(crate) use tabs::*;
