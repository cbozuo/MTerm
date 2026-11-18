// (#editor-native 2026-11-16) 高亮 token 类别。渲染侧不再按类别查色 —— 宿主桥
// (src/app/editor_ui.rs) 把类别映射到主题 ANSI16 槽位并编码成 `start:end:AARRGGBB;`,
// 交给 Slint 在 shaping 层叠色;这里只表达「这是什么」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpanKind {
    /// 默认前景
    Plain = 0,
    /// 关键字 → ANSI 品红
    Kw = 1,
    /// 类型/类名 → ANSI 亮黄
    Ty = 2,
    /// 函数/指令 → ANSI 亮蓝
    Fn = 3,
    /// 字符串/路径 → ANSI 亮绿
    Str = 4,
    /// 数字 → ANSI 黄
    Num = 5,
    /// 注释 → ANSI 亮黑 + 斜体
    Com = 6,
    /// 注解/变量 $x/时间戳 → ANSI 亮青
    Ann = 7,
    /// 日志时间戳(与 Ann 同槽,单列便于日志单独调)
    Ts = 8,
    /// 日志线程 [xxx]
    Thr = 9,
    /// 日志级别 ERROR/FATAL
    LvlError = 10,
    /// 日志级别 WARN
    LvlWarn = 11,
    /// 日志级别 INFO
    LvlInfo = 12,
    /// 日志级别 DEBUG/TRACE
    LvlDebug = 13,
    /// 异常类名(XxxException/Error)
    Exc = 14,
    /// 堆栈 at … 行
    At = 15,
    /// Caused by: 行
    Cs = 16,
}

/// 字符区间着色段(列以 unicode 字符计)。行级规则引擎的行内产物。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    /// 字符列起点(0 基,含)
    pub start: usize,
    /// 字符长度
    pub len: usize,
    pub kind: SpanKind,
}

/// (#editor-native) 全文着色段:**UTF-8 字节**区间。
///
/// 这是交给 Slint 侧 `TextInput.syntax-spans` 的形态:字节偏移正是原生 shaping
/// 使用的坐标系,因此注释里的 CJK、emoji 都不会让后续区间错位(旧的手搓网格
/// 按「字符 × 单格宽」定位,中文回退字体前进量≠2 格,整行漂移)。
/// 区间必须**有序且互不重叠**,否则渲染侧会丢弃越界/重叠项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GSpan {
    /// 起始字节(含)
    pub start: usize,
    /// 结束字节(不含)
    pub end: usize,
    pub kind: SpanKind,
}

