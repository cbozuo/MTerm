// (#editor-hifi) 高亮 token 类别。枚举顺序即 Slint 侧 kind 整数序
// (ui/editor_types.slint 的 EditorSpan.kind,两处必须同序修改)。
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
    /// 查找命中(琥珀底;查找时由宿主桥注入)
    Hl = 17,
    /// 当前查找命中(亮琥珀)
    HlCur = 18,
}

/// 字符区间着色段(列以 unicode 字符计)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    /// 字符列起点(0 基,含)
    pub start: usize,
    /// 字符长度
    pub len: usize,
    pub kind: SpanKind,
}

impl SpanKind {
    pub(crate) fn as_index(self) -> usize {
        self as usize
    }
}
