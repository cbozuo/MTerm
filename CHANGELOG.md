# Changelog / 更新日志

All notable changes are documented here. 本文件记录所有重要变更。  
中英对照（中文在前，English after）.


## [Unreleased]

- **光标类型在发送命令窗口真正生效（#send-cmd-caret-live）。**
  - **下划线档有形状了**：之前下划线退化成 2px 竖线（和「竖线」档无法区分，用户反馈「设置了光标类型没跟着变」）。TextInput 的内建光标是一根全行高矩形，画不出行底短横线 —— 下划线档把内建光标宽设 0，在 `cursor-position-changed` 缓存的光标像素位置自绘一条 2px 横条（宽度 = 一个字符宽，行高用行号列量出的行距，第几行都对齐），闪烁与内建光标同拍（530ms）、失焦即隐藏。
  - **改设置即时跟随**：光标类型原先只在发送窗打开时镜像一次 —— 窗口开着时去设置里改，得关掉重开才生效。现在 `on_set_term_cursor_style` 落盘后同步到已开的发送窗（进程级单例句柄现成）。
- **Cursor shape now actually applies in the send-command window (#send-cmd-caret-live).**
  - **The underline cursor has a shape**: it used to degrade into a 2px bar, indistinguishable from the "bar" option (user feedback: "it doesn't follow the setting"). TextInput's built-in cursor is a full-line-height rectangle and cannot draw a bottom rule — the underline option now hides the built-in cursor (width 0) and self-draws a 2px rule at the caret's pixel position from `cursor-position-changed` (one character wide, aligned on every line via the gutter-measured line pitch), blinking at the same 530ms cadence and hiding on focus loss.
  - **Live follow**: the style was only mirrored when the window opened, so changing the setting while the window was open required a reopen. `on_set_term_cursor_style` now also pushes the value to the already-open send window (the process-wide singleton handle is right there).

- **「发送命令」窗口第三轮反馈三连修：滚动条可拖、管理入口移除、左栏随输入过滤（#send-cmd-round3）。**
  - **滚动条可拖动**：左栏自绘细条（#send-cmd-scrollbar）原先是纯指示，现在按住上下拖即滚动 —— 命中矩形加宽到 14px（视觉仍 4px 贴右缘、悬停变亮），指针位移 ×（内容高/视口高）写回 `content-y`；热区只在内容溢出时存在、只盖右缘一窄条，不挡列表行点击。
  - **「管理快捷命令…」入口移除**：这个窗口专注「挑一条 → 发」，命令管理仍从主窗侧栏/设置进；`manage` 回调与 Rust 侧注册一并清理。
  - **左栏随输入过滤**：右侧编辑器打字时左栏实时过滤 —— 与命令栏打字补全同一套规则（大小写不敏感子串、快捷命令 name 或 command 命中、历史匹配命令文本），UI 层复刻（`replace-all` 判 contains），省一条每键回 Rust 的链路。过滤态组标题不画（扁平候选，同命令栏匹配框）；点行追加成多行后，已拼进去的命令靠「文本包含该命令」保持可见（Slint string 没有 split，query 只能是整段文本）；清空恢复分组视图。
- **"Send command" window, third feedback round: draggable scrollbar, no manage row, live filtering (#send-cmd-round3).**
  - **Scrollbar is draggable**: the self-drawn thin bar used to be indicator-only — press and drag it now. The hit rect is 14px wide (the visible thumb stays 4px at the edge, brightens on hover); pointer delta × (content/viewport height) is written back to `content-y`. The hit rect only exists while content overflows and covers just the right edge, so list rows keep their clicks.
  - **"Manage quick commands…" entry removed**: the window focuses on "pick one and send"; command management stays in the main window's sidebar/settings. The `manage` callback and its Rust handler are cleaned up.
  - **Left column filters as you type**: typing in the editor filters the left column live, matching the command bar's suggestion rules (case-insensitive substring; quick commands hit on name or command, history on text) — replicated in the UI layer with the `replace-all` contains trick, no per-keystroke round-trip. Group headers are hidden while filtering (flat candidate list, like the command bar); commands already appended into a multi-line draft stay visible via "the draft contains the command" (Slint strings have no split, so the query is the whole text); clearing restores the grouped view.

- **「发送命令」窗口第二轮反馈五连修：边线、竖线、占位、空白拖动与全局单例（#send-cmd-round2）。**
  - **边线粗一倍的真凶**：发送窗的 `Theme.device-pixel-ratio` 在创建时从**还没映射**的主窗拿 `scale_factor()`（恒 1.0），`hairline()` 一直是 1 逻辑 px —— 200% 屏上窗口四边描边、标题线、中缝线全粗一倍。现在打开窗口 show 之后再按自己的真实缩放纠正（主窗同款口径）。
  - **下拉框边线/横线同档**：范围菜单外框 1 逻辑 px → 1 设备 px，且外框整体 snap 到设备栅格（绝对坐标对齐再换回父级坐标，做法同标题栏 hairline）；菜单内两条分隔横线同步改 `hairline()`。主窗命令栏的范围菜单共用同一组件，一起对齐。
  - **中缝竖线不再随菜单消失**：竖线 z 压到内容之下（遮罩之后、布局之前），菜单（z 更高）打开时正好盖住线穿过的那段 —— 上一版是「菜单开着整条隐藏」，用户反馈看着像线丢了。
  - **删占位提示**：编辑区空态那句「输入或粘贴命令；Ctrl+Enter 发送」按用户要求移除（快捷键提示在页脚仍在）。
  - **空白也能拖窗**：加一层夹在「点外部关菜单」遮罩与内容之间的全窗口拖动层 —— 编辑区下方留白、危险告警槽、页脚缝隙这些没有控件的地方按下都能拖；标题栏/输入框/列表行/按钮照常先命中，菜单开着时按空白仍只关菜单不拖窗。
  - **进程级只开一个**：主窗可以有多个（tab 拖出 / 级联），发送窗句柄改为全局登记 —— 已有活实例就复用，再次打开只刷数据并弹到前台（不重置用户拖过的位置/尺寸、不覆盖没发完的草稿）；发送/管理目标跟随最近打开发送窗的那个主窗。
- **"Send command" window, second feedback round: edges, divider, placeholder, blank-drag and single instance (#send-cmd-round2).**
  - **Root cause of the doubled edges**: the window's `Theme.device-pixel-ratio` was taken from the not-yet-mapped main window at creation time (`scale_factor()` is always 1.0 there), so `hairline()` stayed 1 logical px — on a 200% display every border/divider in the window was twice as thick. It is now corrected from the window's own real scale after show.
  - **Scope menu lines aligned**: the menu border went from 1 logical px to 1 device px with the whole frame snapped to the device grid (absolute-position based, like the title-bar hairline); the two inner divider lines also use `hairline()`. The command-bar scope menu shares the component and follows along.
  - **The middle divider no longer vanishes with the menu**: its z was moved below the content (after the outside-click shield, before the layout) — the menu, being higher, simply covers the stretch it spans. The old build hid the whole line while the menu was open.
  - **Placeholder removed**: the "Type or paste a command; Ctrl+Enter sends it" hint is gone per user request (the footer shortcut hint remains).
  - **Blank areas drag the window**: a full-window drag layer sits between the outside-click shield and the content — pressing blank areas (editor whitespace, the danger slot, footer gaps) now drags like the title bar; real controls keep priority, and pressing blank while the menu is open still just closes the menu.
  - **One instance per process**: main windows can multiply (tab detach / cascade); the send window handle is now registered globally — reopening refreshes the data and brings the existing window to the front (keeping user-moved geometry and unfinished text), and send/manage target whichever main window opened it last.

- **「发送命令」窗口七处打磨：边框、置顶、光标、滚动条与线条（#send-cmd-polish）。**
  - **边框与四角**：复用主窗的 `WindowFrame`（1 设备像素直边贴真实边界 + 四角 1 设备像素弧，弧外缘压在 DWM 裁剪弧上），打开时调 `apply_window_chrome` 把系统边框设成 NONE 并把圆角结果回写 —— 和主窗一样「有边、有角」。Win10 / 属性被拒时只画直角直边。
  - **范围菜单不再「穿透」**：中缝竖线画在布局之后（z 最高），菜单从页脚向上弹、横跨 x=224，线就压在菜单中间（用户截图）。菜单打开时整条线不画。
  - **滚动条**：std-widgets 的原生滚动条（16px 轨道 + 上下箭头、在深色面板里立着一根白柱）关掉，左栏换成 4px 自绘细条（内容超出才出现，比例跟 `content-y` 走）。白色根因是 `Palette` 也是**每窗口** global，独立窗口不设就停在 light 默认值 —— 现在和主窗一样随 `dark-mode` 切 `ColorScheme`。
  - **点命令光标落末尾**：外部写 `text` 会把光标重置到 0，表现为停在第一行第一个字前面。补 `set-selection-offsets` 到文本末尾（偏移传大数，Slint 内部走 `ceil_char_boundary` 自动钳到末尾，不必算 UTF-8 字节长度）。
  - **标题栏「钉住」**：关闭键左边一枚 24px 图钉，绑 `Window.always-on-top`（winit 后端的 `WindowLevel::AlwaysOnTop`），钉住时染 accent + 浅底。
  - **编辑器光标跟随设置**：按设置 › 终端 ›「光标类型」给 `text-cursor-width` 取不同值 —— 块状 = 一个字符宽（等宽字符宽从行号列量出来）、竖线 = 2px。位置仍由 TextInput 自己算，永远准；下划线档没有对应画法，退化为细竖线。
  - **分隔线改 `border-subtle`**：标题栏下沿、页脚上沿、中缝、管理入口上沿四处由 `line-2`（更深更硬）换成设置里标题栏下沿那一档，一块窗口里不再两种线。
- **"Send command" window polish: frame, pin, caret, scrollbar and dividers (#send-cmd-polish).**
  - **Frame and corners**: reuses the main window's `WindowFrame` (1-device-pixel straight edges on the real boundary + four 1-device-pixel corner arcs whose outer edge sits on the DWM clip arc). On open it calls `apply_window_chrome` to drop the system border and writes the rounded-corner result back, so the window has an edge and corners like the main one. Win10 / rejected attribute → straight edges only.
  - **Scope menu no longer shows the divider through it**: the middle divider is drawn after the layout (topmost z), while the menu pops up from the footer across x=224 — the line landed in the middle of the menu (user screenshot). The whole line is now hidden while the menu is open.
  - **Scrollbars**: the stock std-widgets bar (16px track with arrows, a white column on a dark panel) is gone; the left column gets a 4px self-drawn thumb (only when content overflows, proportional to `content-y`). The white colour was because `Palette` is a **per-window** global too: a detached window keeps the light defaults. It now follows `dark-mode` like the main window.
  - **Picking a command puts the caret at the end**: writing `text` from the outside resets the cursor to offset 0, so the caret sat before the first character. `set-selection-offsets` now moves it to the end (a huge offset is fine — Slint clamps through `ceil_char_boundary`, so there is no UTF-8 byte counting to do in Slint).
  - **Title-bar "pin"**: a 24px pin left of the close button bound to `Window.always-on-top` (winit's `WindowLevel::AlwaysOnTop`); pinned tints it with the accent colour.
  - **Caret follows the setting**: `text-cursor-width` now depends on Settings › Terminal › "Cursor shape" — block gets one character width (measured from the gutter's monospace digit), bar gets 2px. The position stays with TextInput, so it is always right; the underline option has no equivalent and degrades to a thin bar.
  - **Dividers moved to `border-subtle`**: title-bar edge, footer edge, middle divider and the manage-row edge no longer use the harder `line-2`; they match the settings dialog's title-bar divider.

- **「发送命令」搬成独立系统窗口，可拖拽改尺寸（#send-cmd-window）。**
  - 面板从 AppWindow 的浮层抽成 `ui/send_cmd_window.slint` 的顶层窗口（做法同进程信息窗）：命令栏 ↗ / Ctrl+Shift+E 打开，能拖到第二块屏、和主窗并排，不挡终端；发送完不关窗，范围键（当前标签页 / 全部标签页 / 频道 A-D）随时切换连发。
  - 主题 / 语言 / 模型（快捷命令、历史、范围、台数）每次打开从主窗镜像；`Theme` 是每窗口 global，不镜像会一直是深色默认值。
  - 支持**拖拽窗口 + 右下角抓手改尺寸**（无边框窗口没有系统 resize 边框，与进程窗同款 `drag_resize_window`），布局自适应：左栏固定 224、编辑区随窗拉伸、行号行距运行时实测。
  - 旧浮层及其状态机从 `terminal_view.slint` 移除（`cmd-expand-*` 系列死代码清理）。
  - 测试迁到 `tests/cmd_expand_polish.rs`：直接建 `SendCmdWindow` 量帧，9 条（折叠态列全 / 点选落字 / 行号对齐 / 四区同色 / 中缝贯通 / 占位对齐 / 发送回调不关窗 / 点空白关菜单 / 改尺寸自适应）。
- **"Send command" is now a real OS window, resizable and adaptive (#send-cmd-window).**
  - The panel moved out of AppWindow into a top-level `SendCmdWindow` (`ui/send_cmd_window.slint`, same pattern as the process monitor). Command bar ↗ / Ctrl+Shift+E open it; it can be dragged to a second monitor and side-by-side with the main window, no longer covering the terminal. Sending keeps it open and the scope picker (this tab / all tabs / channels A-D) lets you fire repeatedly.
  - Theme / language / models (quick commands, history, scopes, tab count) are mirrored from the main window on every open (Theme is a per-window global, so without the mirror it would stay dark).
  - **Draggable and resizable**: a bottom-right grip drives `drag_resize_window` (frameless windows have no OS resize border, same as the process window); the layout adapts — the left column stays 224 wide, the editor stretches, and the line-number pitch is measured at runtime.
  - The old overlay and its `cmd-expand-*` state machine were removed from `terminal_view.slint`.
  - Tests moved to `tests/cmd_expand_polish.rs` (drive `SendCmdWindow` directly, 9 cases).

- **弹出编辑面板：范围菜单不再穿透、点左栏能落字、关闭钮统一（#cmd-expand-fixes）。**
  - **范围下拉「穿透」**：面板里的范围菜单还在用 `popup-bg`（半透明），而它本身就压在一块同样半透明的面板上，两层一叠就看得见底下的左栏文字。`ScopeButton` 新增 `on-popup` 档，面板内这一枚传 true → 菜单与悬停气泡改用不透明的 `Theme.field`。
  - **点左栏命令编辑器不刷新**：Slint 的 TextInput 一旦被用户敲过，`text` 就与外部绑定断开，再改 `cmd-expand-text` 编辑器不会跟着变（截图：行号涨到 5 行、编辑区一片空白）。改成用一个 0ms Timer 直接写 `exp-input.text`，并同时回写 `cmd-expand-text`。
  - **「管理快捷命令…」上面那行「时有时没」**：列表高于视口时最后一行被切断。列表底部加 14px 渐隐（必须声明在 ScrollView 之后才盖得住内容）；「管理快捷命令…」的悬停底也从整栏通宽改成与列表行一致的内缩 6px。
  - **右上角关闭按钮统一**：换成与命令管理 / 分组管理 / 设置同一个 `DialogClose`（28px、悬停红底白 ×）。
- **Edit-command panel: the scope menu no longer shows through, picking a command fills the editor, and the close button is unified (#cmd-expand-fixes).**
  - **Scope dropdown was see-through**: the menu still used `popup-bg` (translucent) while sitting on an equally translucent panel, so the left column's text bled through. `ScopeButton` gained an `on-popup` mode; the panel's instance sets it, and its menu / hover card switch to the opaque `Theme.field`.
  - **Clicking a command in the left column did not update the editor**: once a Slint `TextInput` has been edited by the user its `text` is no longer bound to the outside, so changing `cmd-expand-text` no longer refreshed it (the screenshot showed line numbers 1–5 with an empty editor). A 0 ms timer now writes `exp-input.text` directly and syncs `cmd-expand-text` back.
  - **The row above "Manage quick commands…" flickered**: when the list is taller than the viewport, the last row gets sliced. The list now has a 14px fade at the bottom (declared after the ScrollView so it covers the content), and the "Manage quick commands…" hover band is inset 6px like every list row instead of spanning the full column.
  - **Close button unified**: it is now the same `DialogClose` used by the command manager / group manager / settings (28px, red hover, white ×).

- **弹出编辑面板改成常驻浮窗：发送不关窗，可连发（#cmd-expand-window）。**
  - **去掉模态遮罩**：面板外不再压暗、点面板外也不再收起 —— 发完直接能看到终端输出；收起只有标题栏 × 和 Esc 两条路。
  - **点「发送」不关窗**：面板留在原地，命令文本保留（「发给频道 A，再发给全部标签页」是常见用法），发送键短暂变成「已发送」；范围键仍是那一份状态，所以「当前标签页 / 全部标签页 / 频道 A-D」随时切换再发。
  - **标题栏可拖**：绝对坐标差 + 窗格内钳制，与其它弹框同一套。
  - **「管理快捷命令…」行瘦身**：34 → 30 高、圆角 5、内衬收紧（整栏宽的悬停底在窄栏里显得很胖）。
  - **chrome 线按绝对坐标贴设备栅格**：1phx 落在半个像素上会被抗锯齿摊成两行淡影，表现就是「同一个面板，滚动前后中缝线一会儿在、一会儿没了」（用户截图）。标题栏下沿 / 页脚上沿 / 中缝三处都改成 `snap-to-device(绝对坐标) - 父级偏移`。
- **The edit-command panel is now a persistent floating window; sending no longer closes it (#cmd-expand-window).**
  - **Modal scrim removed**: the area outside the panel is no longer dimmed and clicking it no longer dismisses the panel — you can watch the terminal output while it stays open. Closing is via the title-bar × or Esc only.
  - **"Send" keeps the window open**: the panel stays put and the command text is kept ("send to channel A, then to all tabs" is a common flow); the send button briefly shows "Sent". The scope picker keeps its own state, so switching between this tab / all tabs / channels A-D and firing again is one click.
  - **The title bar drags the panel** (absolute-coordinate delta, clamped inside the pane), same pattern as the other dialogs.
  - **The "Manage quick commands…" row is slimmer**: 34 → 30 tall, 5px radius, tighter padding.
  - **Chrome rules are now snapped to the device grid in absolute coordinates**: a 1phx rule landing on a half pixel gets anti-aliased into two faint rows, which is why the same panel showed the middle divider before scrolling and lost it after. The title-bar edge, footer edge and middle divider all use `snap-to-device(absolute) - parent offset` now.

- **弹出编辑面板（命令栏 ↗）：左栏不再空白，管理入口吸底（#cmdbar-expand-left）。**
  - **左栏按分组列出全部快捷命令**：此前直接复用了命令栏那个下拉面板的 `!q.collapsed` 判断 —— 只要用户在面板里折叠过分组，这块就整段空白（用户截图：左栏只剩「最近」两个字，看着像坏了）。现在忽略折叠态，组名画成小标题，顺序仍与面板一致（模型把组名挂在每组第一行）。
  - **内容贴顶 + 管理入口吸底**：`content-height: max(视口高, 内容高)` 会把视口高度下发给内层布局，余量被摊到子项之间 —— 短内容时「最近」贴在栏顶、唯一那条历史孤零零掉到栏底（用户截图左下角那行）。改成让 ScrollView 按子项首选高自己算，并把「管理快捷命令…」钉在栏底（列表再长也不会滚走）。
  - **编辑器占位提示**：空编辑器时显示「输入或粘贴命令；Ctrl+Enter 发送」，不再是一整片纯黑；小节标题（快捷命令 / 最近）提亮到次级色，与分组名拉开层级。
  - **整块面板一个底色**（#cmdbar-expand-flat）：原先左栏是 `popup-bg-inset`、编辑区是 `bg-root`（终端底色，整块里最黑的一块），加上标题行/页脚行一共三种颜色；现在整块 `popup-bg`、只靠 1px 分隔线分区 —— 与命令管理同一口径（用户口径：「每个弹框的标题行、页脚行和面板颜色一致，参考命令管理」）。
  - **行号与正文逐行对齐**：行号列原来写死 18px 行高，正文用 `Theme.fs-sm` 的等宽字体（行距由字体决定，≈15px）—— 每多一行多错一点。现在行距在运行时量出来（同字体同字号下「两行首选高 − 一行首选高」），换字体/DPI 也自动跟随。
  - **页脚范围键画成控件、占位提示与正文同款**（#scope-chip）：弹出编辑面板页脚只有「当前标签页 ⌄」一个东西，原来常态无描边无底色，看着像一行普通文字 —— `ScopeButton` 增加 `framed` 档（1phx 描边 + 圆角，悬停/展开加深），面板页脚启用，命令栏保持原样。编辑器占位提示改成与正文**同字体、同字号、同起点**（原先用比例字体且低 2px，正是「行号和文字不对齐」的观感来源）。
  - **线条统一 1phx，中缝竖线贯通**（#cmdbar-expand-hairline）：标题栏下沿、页脚上沿、中缝、危险告警槽上下沿一律 1 物理像素（逻辑 1px 在 125% 下是 1.25 物理，取整后忽粗忽细）；中缝竖线原来挂在左栏里，到危险槽上面就结束，看着「断了一截」—— 现在画在面板层、从标题下沿直通页脚上沿。命令管理 / 分组管理的可见分隔线也一并改成 1phx。
- **Edit-command panel (command bar ↗): the left column is no longer blank, and the manage entry is pinned (#cmdbar-expand-left).**
  - **The left column lists every quick command, grouped**: it used to reuse the command bar's dropdown `!q.collapsed` check, so as soon as the user had collapsed a group in the panel this whole column went blank (the screenshot showed nothing but the word "Recent", which looked broken). Collapse is now ignored, group names are drawn as small headings, and the order still matches the panel (the model carries the group name on each group's first row).
  - **Content sticks to the top, the manage entry sticks to the bottom**: `content-height: max(viewport, content)` handed the viewport height down to the inner layout, so the leftover space got spread between children — with little content, "Recent" sat at the top and the single history entry was stranded at the bottom of the column (that lone line in the screenshot). The ScrollView now derives the content height from the children, and "Manage quick commands…" is pinned to the bottom of the column so it never scrolls away.
  - **Editor placeholder**: an empty editor now shows "Type or paste a command; Ctrl+Enter sends it" instead of a solid black void; the section headings (Quick commands / Recent) moved up to the secondary colour so they read as headings rather than peers of the group names.
  - **One background for the whole panel** (#cmdbar-expand-flat): the left column used `popup-bg-inset`, the editor pane used `bg-root` (the terminal background, the blackest patch in the panel), so title row / body / footer ended up three different tones. Everything is `popup-bg` now, separated only by 1px dividers — the same rule the command manager follows ("every dialog's title row, footer row and panel should share one colour").
  - **Line numbers line up with the code, row by row**: the gutter had a hard-coded 18px row height while the editor uses the `Theme.fs-sm` monospace font (line pitch ~15px, decided by the font), so every extra line drifted a bit further. The pitch is now measured at runtime (`two-line preferred height − one-line preferred height` with the same font/size) and follows font/DPI changes.
  - **The footer scope picker reads as a control, and the placeholder matches the code** (#scope-chip): the panel footer holds nothing but "This tab ⌄", which used to have no border or fill at rest and looked like a line of plain text. `ScopeButton` gained a `framed` mode (1phx outline, rounded, darker on hover/open) that the footer enables; the command bar keeps its previous look. The editor placeholder now uses the same font, size and origin as the code (it was a proportional font sitting 2px lower — exactly what made the line number look off-centre).
  - **Hairline rules and a continuous middle divider** (#cmdbar-expand-hairline): the title-bar bottom edge, the footer top edge, the middle divider and the danger-strip edges are now 1 device pixel (a logical 1px is 1.25 device px at 125%, which rounds into lines that look thicker or thinner at random). The middle divider used to live inside the left column and stopped right above the 28px danger strip, so it looked cut in half; it is now drawn at panel level, running from the title edge down to the footer edge. The visible dividers in the command manager and group manager were switched to 1phx as well.

- **命令管理：卡片网格等宽 + 预置命令按用途分四组 + 「从预置导入」常驻（#cm-card-eq / #preset-groups）。**
  - **两张卡不再各算各的宽**：行内两张卡的 preferred 宽度跟着命令/备注文字走，Slint 又按 pref 起点均摊余量 → 同一列在不同行宽度不同（实测四种列宽，最窄那行的分组 chip 被压成「…」）。现在列宽只由视口算一次（`(msv.width − 28 − card-gap) / 2`），两张卡都取它，奇数行尾随吸空块。
  - **预置命令分四组**：磁盘与内存 / 网络排查 / 服务与日志 / 进程与容器，各自带一个出厂组色（取自 `ACCENT_PRESETS`）。新装即按分类种下，存量配置走 `defaults_rev` rev 9 按**命令原文**回填分类 —— 用户改过的命令、自己加的条目一条不碰，旧「预设」组清空即删、还有自建条目则保留。
  - **「从预置导入」不再只藏在空态**：左栏底部常驻入口，任何时候都能补回出厂清单（按命令文本去重），导完跳到「全部命令」并报条数。
- **Command manager: equal-width card grid, presets split into four purpose groups, and an always-available "Import presets" entry (#cm-card-eq / #preset-groups).**
  - **The two cards no longer size themselves independently**: each card's preferred width followed its command/note text, and Slint distributed the leftover space from that starting point — so a column had a different width on every row (four distinct widths measured; the narrowest row squeezed its group chip down to "…"). The column width is now computed once from the viewport (`(msv.width − 28 − card-gap) / 2`) and both cards take it; an odd last row keeps the trailing spacer.
  - **Presets split into four groups**: Storage & memory / Network / Services & logs / Processes & containers, each seeded with a factory group colour (taken from `ACCENT_PRESETS`). Fresh installs are seeded that way; existing configs go through `defaults_rev` rev 9, which re-files them **by exact command text** — commands the user edited and entries they added are never touched, and the old "Presets" group is dropped once empty (kept if it still holds their own commands).
  - **"Import presets" is no longer hidden in the empty state**: a permanent entry at the bottom of the left column restores the factory list at any time (deduplicating by command text), then jumps to "All commands" and reports the count.

- **分组管理：改色弹层换成与设置页「强调色」同一套取色界面（#swatch-popup）。**
  - 原来是锚在色点旁的 195px 小浮层、8×6 无名色格、点一下立即落盘；现在是整块取色卡：48 色带**名称 + 色号**、最近行、「选定 ▏调色盘」合并按钮，页脚 无颜色 / 取消 / 应用。点色块只改「选定」，取消 / 遮罩还原，应用才落盘 —— 误点不再改色。
  - 共用组件 `ui/swatch_popup.slint`（`PickerMath` 出字色与对比度守卫）；组还没设颜色时「选定」井与「应用」键显式画成「未设定 / 不可应用」，不再因空 hex 变成透明消失。
  - **弹层改挂窗口根**（#swatch-popup-lift）：分组弹框是 760×560 且 `clip: true`，色块悬浮信息卡（RGB/HSV/HSL）贴着「最近」行上方，弹框靠上时会被裁掉；拖动也被钳在对话框内。现在整块弹层挂在 `app.slint` 根上 —— 整个窗口都能拖、信息卡不被裁，打开「调色盘」时先收起（取色盘层级更低），Esc 先收弹层再关弹框。
- **Group manager: the colour popup now uses the same swatch UI as the theme's accent picker (#swatch-popup).**
  - It used to be a 195px popover anchored to the colour dot with an 8×6 grid of unnamed swatches that applied on click. It is now a full palette card: 48 swatches with **names + hex**, a Recent row, a merged "Selected ▏Palette" button, and a footer of No colour / Cancel / Apply. Clicking a swatch only changes the selection; Cancel or the scrim reverts; only Apply writes — a misclick no longer recolours the group.
  - Shared component `ui/swatch_popup.slint` (ink colour and contrast guard via `PickerMath`). When a group has no colour yet, the "Selected" well and the Apply button render explicitly as "Not set / not applicable" instead of turning transparent and vanishing on an empty hex.
  - **The overlay now lives at the window root** (#swatch-popup-lift): the group dialog is 760×560 with `clip: true`, so the swatch hover card (RGB/HSV/HSL) — which floats above the Recent row — was cut off whenever the card sat near the dialog's top edge, and dragging was clamped inside the dialog. The whole palette now hangs off `app.slint`'s root: it can be dragged anywhere in the window and the hover card is never clipped. It hides while the Palette picker is open (the picker sits lower in the z-order), and Esc closes the palette before the dialog.

- **命令管理：出厂预置 12 条诊断命令，空态一键导入（#preset-commands）。**
  - **预置清单**：磁盘/目录/内存占用、实时进程、监听端口、网卡地址、运行中/异常服务、容器列表、跟踪日志、内核消息 —— 全部只读、主流 Linux 即开即用；「连通测试」只落入输入框不回车，补全目标地址再执行（顺带演示「填入」档）。收进「预设」分组，与普通分组无异，可改名可删除。
  - **三个注入点互不重跑**：新装 `fresh_config()` 直接种子；存量用户走 `defaults_rev` rev 8 迁移，**仅快捷命令表为空时**注入 —— 已有命令的用户不受打扰，删掉的预置也不会被"复活"；管理器空态新增「从预置导入」次级按钮，按**命令文本**去重（自己录过的不重复塞），清空后随时可补。
  - **顺手的界面修正（同一张截图里的反馈）**：头部「新建命令」与空态「新建第一条」去掉加号、统一主按钮规范（30 高 / 12.5 字 / 墨绿字、宽度随文案）；空态文案改为「把常用的命令存进分组，点击即可发送到当前终端」；编辑卡「备注」行提示用撑条钉到行尾 —— 原来裸排两个 Text 会被布局拉伸，提示悬在行中间（分组行有撑条所以一直正常）。
- **Command manager: 12 factory preset diagnostic commands, one-click import from the empty state (#preset-commands).**
  - **The preset list**: disk / directory / memory usage, top processes, listening ports, NIC addresses, running / failed services, containers, journal follow, kernel messages — all read-only, work out of the box on mainstream Linux. "Ping" only fills the input without pressing Enter: complete the host and run it yourself (which also demonstrates the "fill-in" send mode). They land in a "Presets" group that behaves like any user group — rename it, delete it.
  - **Three injection points, never doubled**: fresh installs are seeded straight from `fresh_config()`; existing users go through the `defaults_rev` rev-8 migration and get the presets **only when their quick-command list is empty** — anyone already using quick commands is left untouched, and deleted presets are not resurrected; the manager's empty state gains a secondary "Import presets" button that dedupes by **command text** (commands you recorded yourself are skipped) and can restore the list at any time.
  - **UI fixes from the same screenshots**: the header "New command" and empty-state "First command" buttons drop the plus sign and share one primary-button spec (30 tall / 12.5 type / dark-ink on accent, width follows the label); the empty-state copy now reads "Save frequent commands into groups, then click to send them to the active terminal"; the editor card's Note-row hint is pinned to the line end with a stretch spacer — two bare Texts used to get stretched apart by the layout, floating the hint mid-row (the Group row always had the spacer, which is why it looked right).

- **命令栏：补全候选纳入自定义指令集，历史浮层的两个模式各归其位（#cmdbar-suggest）。**
  - **补全态不再只认历史**：打字弹出的候选框现在把**快捷命令**也找出来（`name` 与 `command` 都参与匹配，展示永远是 `command` —— 否则中文名点一下会把名字当命令发出去），排历史之前；每条左侧标类型：青色闪电 = 自定义指令集，灰色回形箭头 = 敲过的命令。
  - **时钟图标不再误亮**：补全框与完整面板共用 `history-open`，历史钮原来绑的就是它，打字时也跟着亮。新增 `hist-panel-open`（= `history-open && !history-autocomplete`），只有点时钟 / `Ctrl+R` 才点亮。
  - **完整面板以图标中心为锚**：原来钉在命令栏右端。现在 `x = cmd-box.x + cmd-box.width - 204`（图标中心偏移 54 + 面板半宽 150），面板中心正对时钟钮；补全框仍跟着光标走。
  - **补全态点时钟不再关掉面板**：它同时清 `history-autocomplete` 和 `history-open`，`!history-open` 一算就成了关闭。改走 `open_history_panel()` 一步升级成完整面板，并**显式** `search-history("")` —— `history-open` 值没变不触发 `changed`，不显式清过滤的话列表还是按命令栏那几个字筛过的。
- **Command bar: the completion popup now includes quick commands, and the two history-popup modes each behave like themselves (#cmdbar-suggest).**
  - **Completion no longer means "history only"**: what pops up while typing now also searches **quick commands** — matched on both `name` and `command`, always displayed as `command` (otherwise clicking a Chinese name would send the name itself as the command) and listed before history. Each row carries a type glyph: teal bolt = quick command, grey arrow = a command you typed before.
  - **The clock icon no longer lights up while typing**: the popup and the full panel share `history-open`, which is what the history button was bound to. Added `hist-panel-open` (= `history-open && !history-autocomplete`) so only a click / `Ctrl+R` lights it.
  - **The full panel is anchored to the button's centre**: it used to sit pinned to the command bar's right edge. Now `x = cmd-box.x + cmd-box.width - 204` (54 icon-centre offset + 150 half width) puts the panel's centre on the clock button; the completion popup still follows the caret.
  - **Clicking the clock from the completion popup no longer closes it**: that handler cleared `history-autocomplete` and then toggled `history-open`, so `!history-open` turned into "close". It now goes through `open_history_panel()`, which upgrades in one step and **explicitly** calls `search-history("")` — `history-open` does not change so `changed` never fires, and without the explicit clear the list stays filtered by whatever is in the command input.

- **命令栏清空键移到内嵌钮组最前，样式回到「输入清空统一语言」（#cmdbar-clear-first）。**
  - 顺序由 `⚡ 🕘 │ ↗ ×` 改成 **`× ⚡ 🕘 │ ↗`**：清空是「丢弃当前输入」，和后面三个「往输入框里塞东西」不是一类，藏在 ↗ 之后还要跨过去才点得到（用户反馈）。
  - 槽位恒定（`parent.width - 119px`），空框时不画 ×——兄弟按钮都是绝对定位，不会跟着挪。
  - 样式与侧栏搜索 / 主题搜索 / 新建会话 / 分组管理四处清空键同款：常态只有一枚 11px 灰叉，悬停才铺出 **16px accent 实心圆**、叉换成字段底色镂空；命中区仍是 26×26，不必精准戳 16px。
- **The command bar's clear button moved to the front of the inlay group and rejoined the shared clear-button language (#cmdbar-clear-first).**
  - Order went from `⚡ 🕘 │ ↗ ×` to **`× ⚡ 🕘 │ ↗`**: clearing is "discard what I typed", not one of the three "put something into the input" actions, and hiding it behind ↗ meant reaching across the group (user feedback).
  - The slot is fixed (`parent.width - 119px`) and the × simply is not drawn when the box is empty — its siblings are absolutely positioned, so nothing shifts.
  - Styling now matches the four other clear buttons (sidebar search, theme search, new-session dialog, group manager): a plain 11px grey × at rest, a **16px filled accent circle** with the × knocked out to the field colour on hover; the hit area stays 26×26 so you do not have to hit a 16px target.

- **同步输入限定为 SSH，并修掉「没有会话」时的空态文案（#sync-ssh-only）。**
  - **候选与成员只列 SSH**：`fill_sync_rows` 按会话 `kind` 过滤（RDP/VNC 是整幅画面、SFTP 是本机文件面板、本机 shell 没有远端通道），离线持久成员同样过滤。广播侧（`send_key` 的频道投递）也加了同口径判断，配置里残留的非 SSH 成员不会被 `send_raw` 静默丢字节。
  - **tab 频道徽标只给 SSH**：`refresh_tab_channel_row` 对非 SSH tab 不再挂 A/B/C/D（否则 tab 上会出现一个永远不会生效的徽标）。
  - **tab 右键菜单**：`TabInfo` 新增 `sync_ok`（= 该 tab 的会话是 SSH），菜单的「加入频道」整组（标题 + 4 个色块 + 暂停/离开/显示状态条 + 前导分隔线）全部按它显隐；菜单高度改成逐行精确计算（SSH 239 / 有字母 +60 / 有状态条项 +29；非 SSH 187）。
  - **空态文案**：左栏原来说「所有会话均已加入当前频道」——一个会话都没开时这是错的（用户截图）。现在按作用域内 SSH 会话总数分两种：没有 SSH 会话 → 「当前没有打开的 SSH 会话 / 同步输入只支持 SSH 连接」；有但都加进去了 → 原文案。右栏同理（「还没有可加入的 SSH 会话」）。
- **Input sync is SSH-only now, and the empty-state copy no longer lies when nothing is open (#sync-ssh-only).**
  - **Candidates and members list SSH sessions only**: `fill_sync_rows` filters by session `kind` (RDP/VNC are a whole picture, SFTP is a local file panel, a local shell has no remote channel), and offline persisted members are filtered the same way. The broadcast path (`send_key`'s channel fan-out) got the same guard, so stale non-SSH members can no longer swallow `send_raw` bytes silently.
  - **Tab channel badges only for SSH**: `refresh_tab_channel_row` no longer stamps A/B/C/D on non-SSH tabs (it used to show a badge that could never take effect).
  - **Tab context menu**: `TabInfo` gained `sync_ok` (the tab's session is SSH); the whole "Join channel" group (caption + 4 swatches + pause/leave/show-bar + its leading separator) is gated on it, and the menu height is now computed row by row (SSH 239 / +60 with a letter / +29 with the bar item; 187 without the group).
  - **Empty-state copy**: the left pane claimed "All sessions are already in this channel" even with nothing open (the user's screenshot). It now branches on the number of SSH sessions in scope: none → 「当前没有打开的 SSH 会话 / 同步输入只支持 SSH 连接」; some but all joined → the old line. The right pane got the matching variant (「还没有可加入的 SSH 会话」).

- **删除条：展开期间不再能勾选，确认键改「删除分组」（#gm-delete-bar）。**
  - 点垃圾桶本来就会调 `clear-picks`（实测确实调了），但**删除条展开后仍可继续勾选**——勾一条，批量条又冒出来和删除条并排，看起来像「选中状态没清掉」（用户截图）。现在删除条展开期间整行点击、复选框、`Space` 三处都不再改勾选，批量条也加了一道 `confirm-del == ""` 的守卫。
  - 确认键文案 `删除` → `删除分组`（改成内容宽，中英「Delete group」都放得下）。
- **Delete strip: picking is disabled while it is open, and the confirm button says "删除分组" (#gm-delete-bar).**
  - Clicking the trash already calls `clear-picks` (verified), but **picking stayed possible while the strip was open** — pick one row and the batch bar returns next to the delete strip, which reads as "the selection was never cleared" (the user's screenshot). Row clicks, the checkbox and `Space` are now all gated on `confirm-del == ""`, and the batch bar carries the same guard.
  - The confirm button reads `删除分组` (content width, so `Delete group` fits too).

- **两个下拉（移动到 / 删除去向）与删除条文案重做（#gm-menus）。**
  - **位置**：原来硬编码贴面板右缘（`parent.width - 178px` / `y: 104px`），跟触发按钮看不出关系。现在打开时记录按钮的**中心 x**（`move-menu-x` / `del-menu-x`），菜单以该点为轴**左右居中**、出界再夹回面板内 —— 「移动到」的下拉正对按钮上方，删除条的「去向」下拉正对「去向」按钮下方，和原生 select 一致。
  - **高度**：按实际行数算（`menu_rows()` = 可见组 + 默认组被隐藏时补的一行），不再固定 240/30 一排导致底部留一条空白；行高与行内容对齐 28px，计数用等宽字体右对齐。
  - **加标题与回执**：「移动到」下拉顶部补 `移动到` 小标题、底部补 `已选 N 项`（分隔线隔开）；「去向」下拉顶部补 `会话移到`。当前去向那一行用 accent 文字标出来。
  - **删除条文案**：`删除「X」？ n 个会话移到 →` 末尾那个箭头指向空气，读起来像还有下文；改成 `删除「X」？ n 个会话将移到 [去向]`，让下拉成为句子的一部分（空组仍是「该分组为空。」）。
- **Both dropdowns (Move to / delete target) and the delete-strip wording redone (#gm-menus).**
  - **Position**: they used to be pinned to the pane's right edge (`parent.width - 178px`, `y: 104px`) with no visible relation to the button that opened them. They now record the trigger button's **centre x** (`move-menu-x` / `del-menu-x`) and centre themselves on it, clamped inside the pane — the Move-to menu sits above its button, the delete-target menu below its button, like a native select.
  - **Height**: computed from the real row count (`menu_rows()` = visible groups plus the synthetic Default row when it is hidden) instead of a fixed 240/30 multiplication that left a blank strip; rows are 28 px and the counts are mono, right-aligned.
  - **Titles and footer**: the Move-to menu gained a `Move to` caption and a `已选 N 项` footer behind a hairline; the target menu gained `Move sessions to`. The current target row is tinted with accent text.
  - **Delete-strip copy**: `Delete "X"? n sessions move to →` ended in an arrow pointing at nothing; it now reads `删除「X」？ n 个会话将移到 [去向]` so the dropdown completes the sentence (an empty group still says 「该分组为空。」).

- **快捷键气泡：不再自动弹出，文案逐条核对补齐（#gm-shortcuts）。** 用户要求「打开弹框时不要自己弹出来」，并核对里面的文案。
  - 去掉首次打开的自动展开（`keys-seen` 一并删除）：现在只有点底栏 `?` 才弹。
  - 逐条实测了气泡里列的按键（`Space` 勾选、`↑↓` 切组、`F2 + Enter` 改名），三条都真的能触发，原文案没有说错的地方；**缺的是实现里有、气泡没写的入口**，补齐为四组：`范围勾选 Shift+点`、`勾选/取消勾选 点行`、`改名 双击组名`、`换组 / 组内排序 拖动`、`改分组颜色 点色点`。标题从「键盘快捷键」改成「快捷键」，因为最后一组是鼠标操作。
- **Shortcut popover: no longer auto-opens, and every line is now verified (#gm-shortcuts).**
  - The first-open auto-expand is gone (the `keys-seen` flag with it): it only appears when you press `?` in the footer.
  - Each keyboard line was exercised (`Space` toggles a pick, `↑↓` switches groups, `F2 + Enter` renames) — all three really fire, so nothing was wrong; what was **missing** were the entry points the implementation already had: `Shift+click` range pick, click-a-row to pick, double-click a name to rename, drag to move/reorder, click the colour dot to recolour. The title dropped "keyboard" since the last group is mouse.

- **「未分组」改名「默认组」，空着就不占行；空组空态文案改成真的做得到的动作（#gm-default-group）。**
  - 显示名统一为「默认组 / Default」（`GM_UNGROUPED`），删除去向、拖拽回执、批量条文案一起改。
  - **没成员时不显示这一行**：Rust 侧只在 `count > 0`（或一个自定义组都没有，避免整栏空掉）时下发该行；左栏像素映射改成两套 —— `has-cap()` 现在看「有没有自定义组」，新增 `has-default-row()`，`gi-at` / `gi-top` 与「我的分组」小节标题都按有没有默认组行来计算（默认组隐藏时标题顶到列表最上面）。
  - 隐藏行的**副作用一起补掉**：它仍然是合法去向，所以「移动到」和删除条的「去向」下拉在默认组行缺席时补一条「默认组」选项（与侧栏右键菜单「第一位永远是默认组」的既有规则对齐）；当前选中的正是默认组而它被搬空时，选中位置自动落到第一个存在的组。
  - **空组文案修正**：原来写「把会话拖到这里 / 或用底部批量条的「移动到」」，两句都做不到（右栏只列当前组，任何拖拽都落不到这块画布；空组也没有成员可勾选，批量条根本不会出现）。去掉那个假投放虚线框，改成「这个分组还没有会话 / 把会话拖到左侧的分组名上，或在其他分组里用「移动到」移进来」。
- **"Ungrouped" renamed to "Default", the row hides when empty, and the empty-group copy now describes something actually possible (#gm-default-group).**
  - Display name is now 「默认组 / Default」 everywhere (`GM_UNGROUPED`), including delete targets, drag toasts and the batch bar.
  - **The row is not shown when it has no members**: Rust only emits it when `count > 0` (or when there are no custom groups at all, so the column never goes blank). The left-pane pixel mapping got two variants — `has-cap()` now means "there are custom groups", a new `has-default-row()` drives `gi-at` / `gi-top` and the "My groups" caption position (the caption moves to the very top when the default row is gone).
  - **The side effects of hiding are covered**: the hidden row is still a valid destination, so both the "Move to" and the delete-target dropdowns append a "Default" entry when the row is absent (matching the sidebar menu's existing "default is always first" rule); if the selected group *was* the default and it just became empty, the selection falls back to the first group that still exists.
  - **Empty-group copy fixed**: it claimed "drop sessions here" and "use Move to in the batch bar" — neither is possible (the right pane only lists the current group, so nothing can be dropped on it; an empty group has nothing to pick, so the batch bar never appears). The fake drop-zone frame is gone; the text now reads 「这个分组还没有会话 / 把会话拖到左侧的分组名上，或在其他分组里用「移动到」移进来」.

- **全局去掉手型光标，统一用系统箭头（#native-cursor）。** 用户截图三处（右下角侧栏开合键、侧栏「＋ 新建会话」、分组行）都出现手型光标。桌面应用的惯例是箭头（VSCode / Explorer 同理），手型是 Web 语言。做法：`ui/` 下 22 个文件里 **262 处 `mouse-cursor: pointer` 全部改为 `default`**（含 `? pointer : default` 这类三元，折叠成常量），4 处 `grab` / `grabbing`（面板拖拽把手、拖拽中状态）也一并改掉。
  - 保留：`text`（终端/输入框的 I 形）、`ew-resize` / `ns-resize` / `nesw-resize` / `nwse-resize`（分隔条、窗口边）、`move`（拖动中的四向移动）—— 这些是原生语义，不是「手势图标」。
- **No more hand cursors anywhere — the native arrow is used throughout (#native-cursor).** Three screenshots (right-sidebar toggle, "New session" button, a group row) all showed the pointing hand. Desktop apps use the arrow (VSCode / Explorer do); the hand is a web idiom. All **262 occurrences of `mouse-cursor: pointer` across 22 files under `ui/` are now `default`** (ternaries like `? pointer : default` collapse to a constant), plus the four `grab` / `grabbing` sites (dock drag handles and drag-in-progress states).
  - Kept: `text` (I-beam over terminals/inputs), `ew/ns/nesw/nwse-resize` (splitters and window edges) and `move` (during a drag) — those are native semantics, not "hand icons".

- **分组改名重名：报错语义与交互重做（#gm-dup-guard）。** 用户截图两点：左栏行内改名把错误写进底栏却顶着一个**绿勾**（看着像成功），右栏头部那条错误被挤成「已存…」三个字。
  - 底栏反馈区分语义：新增 `toast-error`，错误用 ⚠ + danger 文字，且**不被 2.6s 定时器吞掉**（名字改回合法或结束编辑时自动收起）；成功/结果类消息仍走原样式。
  - 右栏错误改成头部下方**独立 22px 一行**（danger 10% 底 + 底部 danger 细线 + 完整文案），不再塞进 52px 头部被 elide。
  - **提交失败不再被静默丢弃**：`commit_edit()` 改成返回 bool，`完成`／窗口 ×／点遮罩／↑↓ 切组／点另一个组／点「新建分组」都先问它 —— 重名时按「完成」不再关窗（旧版关掉后回来发现根本没改）。
  - 顺带修掉改名入口的一个死路：**改名输入框是随编辑态新建的，Slint 不会自动给焦点**，F2 / 双击改名后直接打字没有任何反应（要再点一下输入框才行）。三个输入框（左栏改名 / 右栏改名 / 新建行）都补了 `init => { self.focus(); }`。
  - 回归：`tests/group_manager_smoke.rs` 新增 `duplicate_rename_keeps_dialog_open`（先验证「完成」坐标真能关窗，再验证重名时关不掉）。
- **Group rename duplicates: error semantics and interaction redone (#gm-dup-guard).** Two things from the screenshots: a left-pane rename failure wrote into the footer while wearing a **green check** (reads as success), and the right-pane error was squeezed into "已存…" inside the 52 px header.
  - The footer now distinguishes semantics: new `toast-error` renders ⚠ + danger text and is **not swallowed by the 2.6 s timer** (it clears when the name becomes valid or editing ends).
  - The right-pane error moved into its own **22 px row** under the header (danger 10 % fill, danger hairline, full text) instead of being elided inside the header.
  - **A failed commit is no longer silently dropped**: `commit_edit()` now returns bool and every close/switch entry point asks it first (`完成`, window ×, scrim click, ↑↓ group switch, clicking another group, opening the new-group row). Pressing "Done" on a duplicate keeps the dialog open (it used to close and throw the rename away).
  - Also fixed a dead end in the rename flow: the rename input is created together with the edit state and **Slint does not focus it automatically**, so typing right after F2 / double-click did nothing until you clicked into it. All three inputs now carry `init => { self.focus(); }`.
  - Regression: new `duplicate_rename_keeps_dialog_open` in `tests/group_manager_smoke.rs` (first proves the Done coordinate really closes the dialog, then that a duplicate keeps it open).

- **分组管理：拖动浮层（ghost）终于跟手（#gm-drag-ghost）。** 用户反馈「拖成员到别的组 / 组内排序时 ghost 不跟手」。
  - 根因：`drag-abs-*` 存的是**窗口**坐标，浮层却挂在 `gm`（居中的弹框）里，减的却是 `root.absolute-position`（弹框根 = 0,0）——于是整块 ghost 比指针偏出 `(gm.x, gm.y)`，窗口越大偏得越多；右缘还写死 `gm.width - 250`（浮层实际 240 宽）再压一次。
  - 修法：改成减 `gm.absolute-position`，左右/上下都用 `self.width/height` 夹在弹框内（4px 边距），指针落在角上也不会把 ghost 拖出可视区。
  - 顺带：**组行的 ghost 从来没跟过手**——组拖拽只在 `TouchArea.moved` 里算落点，从没写过 `drag-abs-*`（成员拖拽是在 `pointer-event` 里写的），所以拖组时浮层一直停在初始 -1000 或被裁掉。现在 `moved` 里同步指针绝对坐标。
- **Group manager: the drag ghost actually follows the pointer now (#gm-drag-ghost).**
  - Cause: `drag-abs-*` holds **window** coordinates while the ghost lives inside `gm` (the centred dialog), and the offset subtracted `root.absolute-position` (0,0) instead of the dialog's own origin — every ghost sat `(gm.x, gm.y)` away from the cursor, worse as the window grew. The right edge also clamped with a hard-coded `gm.width - 250` for a 240 px box.
  - Fix: subtract `gm.absolute-position` and clamp with the element's own `self.width/height` inside the dialog (4 px margin), so the ghost never leaves the visible area.
  - Also: **the group-row ghost never followed at all** — group drags only computed the insert index in `TouchArea.moved` and never wrote `drag-abs-*` (member drags do it in `pointer-event`), so the ghost stayed at its -1000 initial position. `moved` now syncs the absolute pointer position too.

- **标题栏新增「管理分组」入口（#group-manager-topbar）。** 用户要求把分组管理的快捷入口放到标题栏。在「传输」和「更多」之间加一个 `TopbarBtn`（图标 `group-list`，文案「管理分组 / Groups」），点击与侧栏右键菜单、`Ctrl+Shift+G` 走同一个 `gm-open-request`，弹框打开期间按钮保持选中（active = `gm-open`）；点击时顺带收起「更多 / 传输」弹层，避免关掉分组弹框后菜单还挂在下面。1100 / 900 两种窗宽下顶栏仍不挤。
- **Group manager entry added to the title bar (#group-manager-topbar).** A `TopbarBtn` (icon `group-list`, label "管理分组 / Groups") now sits between "Transfers" and "More" and goes through the same `gm-open-request` as the sidebar context menu and `Ctrl+Shift+G`; it stays active while the dialog is open (`active: gm-open`) and closes the More/Transfers popovers first so no menu is left hanging underneath. The bar still fits at 1100 px and 900 px.

- **分组管理弹框：实机反馈四处修复（#gm-hover-fix #gm-focus #gm-foot）。** 用户装包实测提出四点，逐条对到根因：
  - **行尾 ↑↓ 一移过去就消失。** 淡显的 `opacity` 只判了整行 TouchArea 的 `has-hover`；指针压到按钮上时整行已经失去 hover，按钮当场隐掉（悬浮卡又正好盖在那一列，加重了错觉）。现在 `opacity`、行底色、组名颜色都把按钮自己的 hover 一起算进去；组行的 ↑↓ 原包在 `if !reserved` 里、id 在外层不可见，改成常声明 + `visible`（Slint 的不可见元素不占位）。悬浮卡翻转阈值也从「容器右缘」提前 48px，卡片不再压按钮。
  - **搜索态行尾的灰色胶囊（所属组 chip）删除。** 右栏本来就只列当前组，chip 是冗余信息，还跟 user@host 抢宽度（组名短的行会被拉成一条长条）。整块删掉，地址列恢复直读。
  - **输入框聚焦样式统一。** 原来搜索框只有 accent 描边、改名/新建框是「常亮 accent」（分不清有没有焦点）。四个输入框（搜索 / 左栏行内改名 / 新建行 / 右栏改名）统一成 app 既有语言（同 session_dialog 的 LabeledInput）：聚焦 = accent 描边 + `popup-field-2` 提底 + 外扩 3px accent-soft 光环；错误态仍走 danger，不再与聚焦抢边框。
  - **页脚。** 1px 分隔线没写 `y`，默认落在竖直中心、横穿「改动已自动保存」（第一批就带进来了）；补 `y: 0px`。顺带：说明文字 11 → 11.5px、间距 6 → 8，完成按钮改内容宽（中英都合适）并在暗色主题改用稿子的 ink 字（teal 上白字只有 1.8:1）。
- **Group manager: four fixes from the shipped build (#gm-hover-fix #gm-focus #gm-foot).**
  - **Row-end ↑↓ vanished as soon as you moved onto them.** Their fade was bound to the row TouchArea's `has-hover`, which the row loses the moment the pointer lands on the buttons (the tooltip covering that column made it worse). The fade, the row tint and the group-name colour now include the buttons' own hover; the group row's ↑↓ used to sit inside `if !reserved`, so their ids were out of scope for the row — they are now always declared and hidden with `visible` (invisible elements take no space in Slint). The member tooltip also flips 48 px earlier so it never covers the button column.
  - **Dropped the grey pill at the end of rows while searching.** It repeated the group you are already looking at and fought `user@host` for width (rows with a short group name stretched it into a long bar).
  - **Unified input focus styling.** The search box only changed its border while the rename/new-group fields were "always accent" (no way to tell focus). All four inputs now use the app's existing language (same as session_dialog's LabeledInput): accent border + `popup-field-2` fill + a 3 px accent-soft halo; error state still owns the border in danger.
  - **Footer.** The 1 px separator had no `y`, so it defaulted to the vertical centre and ran straight through "Changes save immediately" (present since the first pass). Added `y: 0px`, bumped the note to 11.5 px with 8 px gaps, made the Done button content-width, and switched its label to the design's ink on dark accent (white on teal is only 1.8:1).

- **分组管理弹框对齐 10-22 高保真（#group-manager-hifi）。** 用户按新稿（`docs/design/group-manager-hifi-2026-10-22.html`）逐项核对，改动集中在「组级动作收敛到一处 + 两侧都单行 + 搜索/拖拽/批量给出可见反馈」。
  - 尺寸与两栏：760×522 → **760×560**；左栏 248 → **320**（可拖 220–480，双击回 320），中间 5px 命中区压一条 1px 线（hover/拖动转 accent）；左栏不再用 inset 深色板 —— 稿子里两栏同底色，只靠那条线分隔。
  - 头尾：头部维持 52px 单行，搜索框吃满余量（上限 280）；底栏的四条常驻快捷键提示收进 **`?` 气泡**（首次打开自动展开一次，Esc 逐级收起），任何落盘动作的结果写进底栏那行并 2.6s 回落，**取消居中 toast**。
  - 左栏：组行 36px = 9px 色点 + 3px 同色光晕 + 组名 + 计数（搜索态显示 `命中/总数`）；「我的分组」小节 11.5px/600 + accent 计数徽标（原 10px 灰底灰字）；未分组下的分隔线删除；新建行带色点（建组前就能选色）与清空 ×，重名提示贴在输入行下方。
  - 右栏头部 52px：`[色点][组名][n 个会话]` 左对齐成组（组名不再 `horizontal-stretch`，否则计数被推到大片空白右侧），右端只剩「全选 / 1px 竖线 / 垃圾桶」；勾满时文案变「取消全选」。
  - 删除条 68px 卡片 → **52px 单行**：`⚠ 删除「X」？ n 个会话 → [去向▾] [取消] [删除]`；空组只问「该分组为空。」不摆没用的去向控件；去向改成下拉菜单（Slint 没有原生 select）。
  - 勾选：**整行可点**（按下/松开同一行才算，>5px 起拖，互不误触）+ `Shift` 点复选框范围勾选 + `Space` 勾当前行；行尾 ↑↓ 点到「已勾选的一条且勾了多条」= **整块移动**（新回调 `step-members`，Rust 按组内真实顺序算落点，Slint 函数里没法写 break/累加）。
  - 搜索：组行**不再隐藏**（计数 `命中/总数`、0 命中压暗 45%），组名命中 = 整组命中；当前组 0 命中而别处有时自动跳过去；空态分「所有分组里都没有匹配」（+清除搜索）与「本组没有匹配 · 其他分组还有 N 个」（+跳到「X」）；命中行尾浮出所属组色点 chip。
  - 拖拽：目标组行转 accent 淡底 + 1px accent 描边 + 行尾 `+N` 徽标；成员悬浮卡改为**贴鼠标右下角**（+16/+14，出界翻边），不再是固定 `x=250` 压住名字列。
  - 色板：锚在被点的那个色点下方；打环改用**被编辑组**的颜色（原来读「当前组」的色，给非当前组改色时会打错环）。
  - Esc 逐级：改名 → `?` 气泡 → 去向下拉 → 删除条 → 搜索词 → 关窗（原来只有「改名 → 关窗」两级）。
  - 顺带修掉一个可见 bug：左栏行内改名与右栏标题改名共用 `editing`，改当前组时会**同时出现两个输入框**；新增 `edit-where`（left / right）区分入口。
  - 回归：`tests/group_manager_smoke.rs` 跟着改成「首次 Esc 只收快捷键气泡、第二次才关窗」；`GroupRow` 增 `hit` 字段（左栏 `命中/总数`）。
- **Group manager dialog brought in line with the 10-22 hifi (#group-manager-hifi).** The user diffed the built dialog against `docs/design/group-manager-hifi-2026-10-22.html`; the work is "collapse group-level actions into one place, keep both panes single-line, give search/drag/batch visible feedback".
  - Geometry: 760×522 → **760×560**; left pane 248 → **320** (drag 220–480, double-click resets), with a 5 px hit strip carrying a 1 px line (accent on hover/drag). The left pane no longer paints an inset panel background — the design keeps both panes on the same surface and lets that line do the separating.
  - Chrome: the 52 px single-line header stays, with the search box taking the slack (capped at 280 px); the four permanent shortcut hints in the footer collapse into a **`?` bubble** (auto-opens once on first use, Esc steps through it), and every persisted action now reports into the footer line for 2.6 s instead of raising a centred toast.
  - Left pane: 36 px group rows = 9 px dot + 3 px same-colour halo + name + count (`hits/total` while searching); the “My groups” caption is 11.5 px/600 with an accent pill (was 10 px grey-on-grey); the divider under the ungrouped row is gone; the creation row carries its own colour dot (pick the colour before the group exists) and a clear button, with the duplicate-name error right under the input.
  - Right header, 52 px: `[dot][name][n sessions]` grouped left (the name no longer stretches, which used to shove the count against the far edge), leaving only “Select all / 1 px rule / trash” on the right; the label flips to “Clear” once everything is picked.
  - Delete strip: 68 px card → **52 px single line** `⚠ Delete “X”? n sessions → [target ▾] [Cancel] [Delete]`; an empty group only asks “This group is empty.” without a pointless target control. The target is a dropdown menu since Slint has no native select.
  - Picking: **the whole row toggles** (press and release on the same row; drag starts past 5 px, so the two never collide), `Shift`+checkbox selects a range, `Space` toggles the current row. The row-end ↑↓, when pressed on an already-picked row with several picked, moves **the whole block** (new `step-members` callback; Rust computes the landing spot from the real in-group order because Slint functions cannot break or accumulate).
  - Search: group rows are **no longer hidden** (`hits/total` plus 45 % dimming at zero hits), and a group-name hit keeps the whole group; if the current group has no hits but another does, the dialog jumps there; the empty state now distinguishes “No match in any group” (+clear) from “No match in this group · N elsewhere” (+go to “X”); matched rows grow a group chip at the end.
  - Drag: the target group row turns into an accent tint plus a 1 px accent outline and a `+N` badge; the member tooltip now **follows the mouse** (+16/+14 bottom-right, flipping at the edges) instead of sitting at a fixed `x=250` over the name column.
  - Colour board: anchored under the clicked dot, and the ring now marks the **edited** group's colour (it used to read the current group's colour, so recolouring another group rang the wrong swatch).
  - Esc steps back through: rename → `?` bubble → target dropdown → delete strip → search query → close (previously only rename → close).
  - Also fixes a visible bug: left-row rename and right-title rename shared `editing`, so renaming the current group showed **two inputs at once**; a new `edit-where` (left/right) keeps them apart.
  - Regression: `tests/group_manager_smoke.rs` now expects Esc to collapse the shortcut bubble first and close on the second press; `GroupRow` gained a `hit` field.

- **关闭 RDP 标签页时先发一条 Graceful Disconnect（#remote-graceful-close）。** 用户猜测「关 tab 没退干净导致服务器说另一个会话已登录」。原来的收尾是把协议 future 直接 drop：socket 关了，但服务器要等传输层发现才把会话标成 Disconnected，这段时间内重连会让新旧两个会话同时在册。现在关 tab 时先发 `RdpInputEvent::Close`，走 ironrdp 的 MCS graceful shutdown，最多等 400ms（服务器不答就照旧 drop，不拖住关 tab）。注意：如果那台机器上本来就有一个**活动的控制台会话**（自动登录），「另一个会话被阻止」的提示仍会出现，那是 Windows 单会话策略，客户端无法代答。
- **Closing an RDP tab now sends a graceful disconnect first (#remote-graceful-close).** The user suspected an unclean teardown behind the server's “another session is logged on” screen. Teardown used to drop the protocol future outright: the socket closed, but the server only marks the session Disconnected once the transport notices, and a reconnect inside that window leaves two sessions on record. Closing a tab now sends `RdpInputEvent::Close`, which runs ironrdp's MCS graceful shutdown, and waits at most 400 ms (falling back to the old drop so closing never stalls). Note: if the host already has an **active console session** (auto-logon), the “another session is blocked” prompt still appears. That is Windows' single-session policy and the client cannot answer it on the user's behalf.

- **内置远程桌面不再把整块画面显示成手型光标（#remote-cursor）。** 用户反馈进入 RDP 画面后鼠标一直是「抓手」。根因是 `remote_desktop_view.slint` 给覆盖整幅画面的 TouchArea 写了 `mouse-cursor: pointer`，这是移植时按「可点区域」的占位写法。远端光标形状目前没接 ironrdp 的 pointer update（`RdpOutputEvent::Pointer*` 在驱动里被显式忽略），本地光标就是唯一可见的光标，于是整屏都是手。改为 `default`（系统箭头），不再谎报「这里可以点」。真正的远端光标（箭头/文本/手型跟随远端、隐藏本地箭头）仍列为后续项。
- **The built-in remote desktop no longer shows a hand cursor over the whole picture (#remote-cursor).** Reported after entering an RDP session: the pointer stayed a hand everywhere. The whole-view `TouchArea` in `remote_desktop_view.slint` carried `mouse-cursor: pointer`, a placeholder from the port ("this area is clickable"). Remote cursor shapes are not wired up yet (`RdpOutputEvent::Pointer*` is deliberately ignored in the driver), so the local cursor is the only visible one and it was a hand across the entire desktop. It now uses `default` (the system arrow) instead of claiming the surface is clickable. True remote-cursor support (following the remote arrow/text/hand, hiding the local one) remains a follow-up.

- **分组右键菜单的「管理分组」：文案回中文、图标换成 group-list（#group-manager-icon）。** 截图里同一菜单其它项都是中文，只有这一行是 `Manage groups`。
  - 文案：菜单项本来就写了 `@tr("Manage groups")`，是 `lang/zh/LC_MESSAGES/mterm.po` 漏了条目，回落到英文 msgid。补 `msgid "Manage groups" / msgstr "管理分组"`。
  - 图标：原先是借 `settings` 齿轮，与菜单里其它齿轮语义混淆，也读不出「分组」。新增 `assets/icons/group-list.svg`（复用 `$FOLDER` 骨架 + 两条组清单线），`make-icons.sh` 补 mk 行、`ui/icons.slint` 登记、`gen-preview.sh` 补 row 并重新生成预览页。
- **The group context menu's “Manage groups” row: Chinese label restored, new `group-list` icon (#group-manager-icon).** Every other row in that menu was Chinese; only this one fell back to English.
  - Text: the row already carried `@tr("Manage groups")`; `lang/zh/LC_MESSAGES/mterm.po` simply lacked the entry, so the msgid leaked through. Added `msgid "Manage groups" / msgstr "管理分组"`.
  - Icon: it borrowed the `settings` gear, which both collided with the other gears in the same menu and said nothing about groups. Added `assets/icons/group-list.svg` (the shared `$FOLDER` skeleton plus two group-list rules), with the matching `mk` line, an `ui/icons.slint` registration, and a preview row in `gen-preview.sh` before regenerating the gallery.

- **终端与右侧栏之间的边界线错位 1 设备像素（#sb-edge-snap）。** 用户截图：终端右侧与右栏左侧之间有一条白缝，浅色主题下尤其明显。
  - 根因：右栏宽度来自拖拽（浮点），125% 缩放下 `窗口宽 - 宽度` 落在半个设备像素上；边界线又对 `parent.x + 2px` 做了一次 `snap-to-device` 取整，把线推到右栏内侧 1 设备像素，外侧就漏出一条面板底色。终端底色在那半像素上还多留了一列 AA 边。
  - 修法：把**边界位置**吸到设备网格（`sb-w` / `sb-h` 按停靠边取整：右/下停靠取整 `内侧尺寸 - 面板宽`，左/上停靠取整面板宽），边界线直接取命中区中心（2px）压在边界上，不再二次 round；左/上停靠仍减一个 hairline，落在侧栏内侧那一列。
  - 回归：`tests/sidebar_edge_line.rs` 在 125% 下渲染 220 / 420 / 520 / 519.2 / 400.4 五种宽度，断言边界处恰好 1 列线、其左为终端底色、其右为面板底色（旧实现第一条就挂）。
- **The terminal / right-sidebar boundary line was off by one device pixel (#sb-edge-snap).** User screenshot: a bright seam between the terminal's right edge and the resource sidebar, worst in light themes.
  - Cause: the sidebar width comes from dragging (a float), so at 125% DPI `window - width` lands on half a device pixel; the splitter's visible line also re-ran `snap-to-device` on `parent.x + 2px`, pushing the line one device pixel inside the sidebar and leaving a column of panel background outside it. The terminal background additionally anti-aliased across that half pixel.
  - Fix: snap the **boundary position** to the device grid (`sb-w` / `sb-h` snap `inner - width` for right/bottom docks and `width` for left/top docks), then place the line at the hit-area centre (2px) so it sits exactly on the boundary with no second rounding; left/top docks still subtract one hairline so the line occupies the sidebar's inner column.
  - Regression: `tests/sidebar_edge_line.rs` renders five widths (220 / 420 / 520 / 519.2 / 400.4) at 125% and asserts exactly one line column, terminal background to its left and panel background to its right (the old code fails on the first case).

- **终端格子的字面落点与图标比例（#ink-center #nerd-icon-scale）。** 用户反馈两件事：`ls` 的图标比字大一号、上下糊到邻行；拖选文字时高亮条「偏下」，说不清是字高了还是条低了。
  - 图标：放大倍数 1.75 是按「文件夹图标 0.53em」定的，但 `ls --color` 里最常见的是文档图标（`nf-fa-file_text` 等，墨高 0.77em），放大后 1.35em —— 比 80% 行距下 1.11em 的格子还高，上下各越界 1~3 设备 px，整屏看着又大又错位。
  - 高亮：`parley` 把行盒里剩余的行距全留在基线下方，字形墨迹整体偏上。离屏实测（13px / 125% / 行距 80%，格高 18 设备 px）：大写行墨迹上留白 2、下留白 4 —— 整行偏高 1 设备 px，拖选时下沿就空出一截。
  - 修法：`nerd-scale` 1.75 → 1.15（最高的文档图标 0.92em、文件夹 0.60em、数字 0.73em），并新增 `ink-bias = 0.062em` 把文字与图标一起下移，上下留白补成 3/3。
  - 回归：`TerminalView` 离屏渲染同一场景逐行量墨迹（`#ink-center` 注释里记了改前/改后数值）。
- **Terminal cell ink placement and Nerd icon scale (#ink-center #nerd-icon-scale).** Two reports: `ls` icons were a size larger than the text and bled into the rows above/below; dragging a text selection made the highlight look "low", with no telling whether the text sat high or the band sat low.
  - Icons: the 1.75 multiplier was calibrated on the folder glyph (0.53em), but what `ls --color` mostly emits are document glyphs (`nf-fa-file_text` and friends, 0.77em ink). At 1.75 they reach 1.35em, taller than the 1.11em cell at 80% line spacing, overflowing 1–3 device px on both sides.
  - Highlight: `parley` leaves the line box's leftover leading below the baseline, so the glyph ink sits high. Offscreen measurement (13px / 125% DPI / 80% spacing, 18 device-px cell): a caps row showed 2 px of margin above the ink and 4 px below, i.e. the whole row sat 1 device px high.
  - Fix: `nerd-scale` 1.75 → 1.15 (tallest document icon 0.92em, folder 0.60em, digits 0.73em) plus a new `ink-bias = 0.062em` that shifts text and icons down together, squaring the margins to 3/3.
  - Regression: the same scene is rendered offscreen through `TerminalView` and measured row by row (before/after numbers are recorded next to the `#ink-center` comment).

- **所有弹框的关闭键统一成同一款（#dialog-close-unify）。** 用户以「新建会话」弹框为参照物：常态 = 透明底 + 灰 ×，悬停 = 红底 + 纯白 × + 90ms 过渡。
  - 盘点出的漂移：高亮规则 / 同步输入 / 分组管理 / 快捷键 / 关于 五个弹框的 × 悬停仍是 `bg-hover` 灰底（同步输入的图标还恒 `text-primary`，压到红底上会「显暗」）；系统信息窗缺 90ms 过渡、编辑器窗悬停图标不变白。
  - 修法：`widgets.slint` 抽出唯一的 `DialogClose` 组件 —— 只开放 `btn-size` / `corner` / `glyph-size` 三个尺寸参数，两态颜色写死在组件内部，杜绝再次复制粘贴漂移；`theme.slint` 新增 `close-btn-hot`（`#e81123`，亮暗同值，白字对比 4.64:1）。窗口三键与标签页关闭键同步改读该 token，`ui/` 下已无 `#e81123` 字面量。
  - 回归：`tests/dialog_scrim_drag.rs` 的锚点从「三处 tint 字符串」升级为「组件两态口径 + 调用点必须复用组件 + 禁止内联红底 + 三处窗口标题栏必须用 token 且悬停转白」。
- **Every dialog's close button now shares one implementation (#dialog-close-unify).** The New Session dialog was the reference: transparent background with a muted ×, switching to a red fill with a pure-white × and a 90 ms transition on hover.
  - Drift found: Highlight rules / Sync input / Group manager / Keyboard shortcuts / About still hovered to a plain `bg-hover` grey (Sync input's glyph also stayed `text-primary`, which reads dim on the red fill); the System information window lacked the 90 ms transition and the Editor window never turned its glyph white.
  - Fix: a single `DialogClose` component in `widgets.slint` exposes only three size properties (`btn-size`, `corner`, `glyph-size`) while both state colours stay hard-coded inside it, so the copy-paste drift cannot recur; `theme.slint` gains `close-btn-hot` (`#e81123` in both themes, 4.64:1 against white). The window controls and tab close buttons now read the same token — `ui/` no longer contains a `#e81123` literal.
  - Regression: the anchors in `tests/dialog_scrim_drag.rs` were upgraded from three `tint` strings to "component state contract + every call site must reuse the component + no inline red + the three window title bars must use the token and whiten on hover".

- **侧栏透明度覆盖到面板内的实心块（#sidebar-alpha-follow）。** 用户反馈调侧栏透明度后，命令栏按钮与输入框、两条 5px 拖拽条、文件面板的页签/上传钮/路径框、两侧滚动条仍是实色斑。
  - 根因：`zone-sidebar*` 系列只替换了**面板底**，面板内仍散落 `bg-root` / `bg-elevated` / `accent` / `border-subtle` 四类实色；std `ScrollView` 的滚动条颜色写死在 Fluent 样式里，无法改成半透明。
  - 修法：新增 `zone-sidebar-accent` / `-accent-hover` / `-line` / `-scroll` / `-scroll-active` 五个侧栏档 token；`widgets.slint` 新增 `PanelScroll`（Flickable + 自绘 4px overlay 滚动条，平时隐形、靠近右缘淡入），替换文件面板的目录树 / 文件列表 / 隧道列表三处 `ScrollView`；两条 5px 拖拽条静止态改为「面板同色 + 1px 中线」，不再整条刷实色；聚焦边框与悬停高亮（分隔条、树分界条）同样走侧栏档 —— 否则静止态透、一碰就变实。
  - 强调色块的**透光率翻倍**（α′ = 1 − (1 − α) × 2，α = 1 时结果仍为 1）：中性面板 33% 透光一眼能看出桌面纹理，饱和青块上同样 33% 却看着仍是实心。
- **Sidebar transparency now reaches the solid blocks inside the panels (#sidebar-alpha-follow).** Reported after raising the sidebar transparency: the command-bar buttons and input, both 5 px splitters, the file panel's tabs / upload button / path field and both scrollbars stayed opaque.
  - Cause: the `zone-sidebar*` tokens only covered the panel background; the panels still used four kinds of solid values (`bg-root`, `bg-elevated`, `accent`, `border-subtle`), and std `ScrollView` paints its scrollbar from hard-coded Fluent colours.
  - Fix: five new sidebar-tier tokens (`zone-sidebar-accent`, `-accent-hover`, `-line`, `-scroll`, `-scroll-active`); a new `PanelScroll` component (Flickable + self-drawn 4 px overlay bar, hidden until the pointer nears the right edge) replaces the three `ScrollView`s in the file panel; both 5 px splitters draw a same-colour band plus a 1 px centre line instead of a solid 5 px line; focus borders and drag/hover highlights use the same tier, so nothing turns solid on interaction.
  - Emphasis fills get their **transparency doubled** (α′ = 1 − (1 − α) × 2, unchanged at α = 1): 33% transparency is obvious on a neutral panel where desktop texture shows through, yet invisible on a saturated accent fill.

- **高亮设置弹框按高保真收尾（#hl-hifi-r2）。** 对照 `docs/design/highlight-settings-hifi-2026-10-20.html` 逐项核对。
  - 视觉：色点加同色 15% 光环（等效 hi-fi 的 `box-shadow`）；开关圆点改为四边等距（原坐标按 CSS padding-box 计算，在 Slint 的边框基准上偏了 1 px，看着贴顶偏右），过渡 160 ms + 悬停反馈；完成按钮宽度随内容自适应并补按下态；自定义页修掉标题与第一个输入框之间近 300 px 的空白（ScrollView 内容布局缺 `alignment: start`，余高被可生长的标题 Text 吃掉），补「我的规则」分组标题、首行不再出现双线。
  - 交互：支持点弹框外关闭；弹框打开时点主窗标题栏仍可拖动窗口（遮罩命中区顶部让位 32 px）。
  - 规则：19 条预设的 ANSI 槽对齐高保真（此前普遍用亮色变体，如 ERROR 用 9 号粉红而非 1 号标准红），新增 `every_preset_matches_its_sample` 逐条体检测试；`iso` 吃掉含时间的完整时间戳（原来被切成一橙一青两段），顺带修好 `2026-10-20T15:46:40Z` 完全不上色；`size` 认 `5.9Gi` / `27Mi` / `0B`（旧写法 `\b` 落在 `G` 与 `i` 之间，`free -h` 全不命中）；彩虹括号不再因 bold 被跳过。
- **Highlight-settings dialog finished against the hi-fi spec (#hl-hifi-r2).**
  - Visuals: colour dots gain a 15% same-colour halo (the `box-shadow` equivalent); the switch knob is now equidistant on all four sides (the old values were CSS padding-box numbers, off by 1 px against Slint's border origin height), with a 160 ms transition and hover feedback; the Done button sizes to its content and gains a pressed state; the Custom tab loses the ~300 px gap between its heading and first field (the ScrollView content layout lacked `alignment: start`, so the growable heading Text ate the slack), plus a "My rules" caption and no doubled separator above the first row.
  - Interaction: clicking outside closes the dialog; the main window can still be dragged by its title bar while a dialog is open (the scrim's hit area yields the top 32 px).
  - Rules: preset ANSI slots aligned with the hi-fi spec (most previously used bright variants — ERROR was slot 9 pink instead of slot 1 red), with a new per-rule `every_preset_matches_its_sample` regression; `iso` now takes a whole timestamp including the time (it used to split into orange + teal), which also fixes `2026-10-20T15:46:40Z` never matching at all; `size` recognises `5.9Gi` / `27Mi` / `0B` (the old `\b` fell between `G` and `i`, so `free -h` output never matched); rainbow brackets are no longer skipped on bold runs.

- **启动窗口：默认居中，不再落到屏幕右下（#win-center）。** 用户反馈「打开软件默认位置在右下角」。
  - 根因：居中挂在 `show()` 之后 30ms 的定时器上，此时原生窗口还没映射 —— `size()` 只有最小尺寸 `720×420`、`scale_factor()` 还是 `1.0`，按这对数算出的左上角是固定一点；窗口随后长到真实尺寸而左上角不动，右/下就溢出屏幕。与分辨率、缩放无关，任何机器都歪，只是歪多少不同。
  - 修法：居中改到**首个 `Resized` 事件**，直接用事件自带的物理尺寸（`center_window_px`）；尺寸恢复未落定时会再进一次，以最后一次为准。`center_window` 改为优先读原生窗口 `inner_size()`，全链路统一按物理像素；30ms 定时器只保留「tab 拖出贴指针」与「第 2+ 窗口级联 +40」两条显式定位，并用 `window_position_pinned` 避免被居中覆盖。
  - 实测（2560×1440 @125%）：连续冷启动均落物理 (283,119)，即工作区内居中；第二个窗口仍是 +40 级联。
- **Launch window is centred instead of landing in the bottom-right corner (#win-center).** Reported as "the app opens in the bottom-right corner".
  - Cause: centring ran on a 30 ms timer after `show()`, before the native window was mapped — `size()` reported the `720×420` minimum and `scale_factor()` still `1.0`. The top-left corner was pinned from those numbers and the window then grew to its real size, running off the right/bottom edge. Independent of resolution and DPI: every machine was off, only by different amounts.
  - Fix: centre on the **first `Resized` event** using that event's physical size (`center_window_px`); when a saved size is still settling the handler runs again and the last pass wins. `center_window` now prefers the native `inner_size()` and works in physical pixels throughout; the 30 ms timer keeps only the two explicit placements (tab detach under the cursor, +40 cascade for 2nd+ windows) and `window_position_pinned` stops centring from overwriting them.
  - Measured at 2560×1440 @125%: every cold start lands at physical (283,119), centred in the work area; a second window still cascades by +40.

- **分组管理弹框按高保真逐条对齐（#group-manager）。** 对照 `docs/design/group-manager-hifi-2026-10-19.html` 做三轮核对。
  - 左栏：补「我的分组 N」小节标题，行高不再统一 → 插入线与「指针 y → 组行号」改用 `gi-at` / `gi-top` 一对换算；组行补 12×22 六点拖柄（仅 hover 显形 0.55，未分组无）；选中态改 `accent 12%/8%` + 左 2px 竖条。
  - 右栏：成员行名称 12.5px、host 等宽 10.5px；复选框视觉 16px / 命中区 22px；切组后列表滚回顶部。
  - 搜索：加宽到 206px，补占位「搜索分组或会话」与 `Ctrl F` 徽标，并真正绑定 Ctrl+F 聚焦；无结果态带「清空搜索」。
  - 改名/新建：**逐键实时**预校验（系统保留名 / 忽略大小写撞名），空名失焦静默放弃；新增 `gm-validate-name` 回调。
  - 拖拽：插入线两端加 6px 圆点；浮层 30px、跟手 +14；多选时名称后**追加**「N 项」药丸而非替换；批量搬运按模型顺序整块落盘（`move_sessions_to_group` / `move_sessions_relative`）。
  - 其他：删除改为单次 `borrow_mut` + 单次 `save`（成员迁移 + 移除 + 清理），删完给 toast；空组态补图标并垂直居中；批量条 `accent 15%/30%`、删除条 `danger 9%/42%`（不再写死色值）；新增 `move-out` / `move-in` 两枚图标与离屏回归 `tests/group_manager_smoke.rs`。
- **Group-manager dialog aligned with the hi-fi spec, rule by rule (#group-manager).** Three review passes against `docs/design/group-manager-hifi-2026-10-19.html`.
  - Left column: added the "My groups N" caption; row heights are no longer uniform, so insert lines and pointer→row mapping go through `gi-at` / `gi-top`; 12×22 six-dot drag grip (hover only, 0.55 alpha, never on "Ungrouped"); selection is `accent 12%/8%` plus a 2 px left bar.
  - Right column: member name 12.5 px, host in 10.5 px mono; checkbox is 16 px with a 22 px hit area; switching groups scrolls the list back to the top.
  - Search: 206 px wide with the "search groups or sessions" placeholder and a `Ctrl F` badge, and Ctrl+F really focuses it; empty state offers "Clear search".
  - Rename/create: **live, per-keystroke** validation (reserved names / case-insensitive duplicates); an empty name is silently dropped on blur; new `gm-validate-name` callback.
  - Drag: 6 px dots on both ends of the insert line; 30 px ghost following at +14; multi-select **appends** an "N items" pill instead of replacing the name; batch moves persist as one block in model order (`move_sessions_to_group` / `move_sessions_relative`).
  - Misc: deleting a group is one `borrow_mut` + one `save` (migrate + remove + cleanup) and reports a toast; the empty state gets an icon and is vertically centred; batch bar `accent 15%/30%`, delete bar `danger 9%/42%` (no hard-coded colours); added `move-out` / `move-in` icons and the offscreen regression `tests/group_manager_smoke.rs`.
- **输出高亮设置重做成 iShellPro 式弹框：19 条预设逐条可开关 + 自定义规则任意取色（#hl-settings-hifi）。** 旧的「一个总开关 + log/devops 两档打包 + 6 档语义色」换成一个弹框两页签（预设规则 / 自定义），入口两处（设置 › 终端 › 输出高亮里的「打开高亮设置…」+ 标题栏「高亮」快捷钮），打开的是**同一个**根层弹框。
  - 引擎：预设表**表序即优先级**（URL/IP/日期/UUID/日志级别/大小/路径/数字/表格线），claim 式命中 —— 每条规则只染程序没上过色的片段，先命中先着色；自定义规则整体排在预设之前（用户意图优先）。预设色走**主题 ANSI 槽位**（换主题时终端高亮与 UI 色点同源换装），自定义色才是绝对 RGB。
  - 修掉三条真 bug 正则（Rust regex 无 lookbehind）：路径改前置捕获组占位只染路径本身（旧写法 0 命中绝对路径）、表格分隔线锚定整行（旧写法只吃前半段）、IPv6 压缩形必须含 `::` / 完整形 ≥4 组 —— 中间那版宽 pattern 会把时钟时间 `10:20:30` 当 IPv6 整段染错。词级预设（日志级别/月份/大小单位/URL 协议）带 `(?i)`：`ERROR` 与 `error` 一样常见，旧引擎就认大写级别。
  - 取色复用强调色那一套：47 预设色板卡（与强调色网格同源）+「最近」行 +「调色盘 ›」→ 通用 `ColorPickerDialog`；`PickerMath.fit-hex` 对比度守卫照旧。
  - 彩虹括号独立开关（非正则）：按嵌套层级 7 色循环，只染括号本身，深度栈跨 run 延续。
  - 顺带按 10-17「弹窗投影全删」定稿清掉遗留投影（三个悬浮气泡 + 两组悬停卡 + 分组管理弹框内的下拉/浮层），`tests/dialog_scrim_drag.rs` 的守卫测试转绿；遮罩色类断言改在**预乘空间**比对（半透明像素反预乘会把 8bit 截断误差放大 ~3 倍，实测 57% 档偏差 8）。
- **Output highlighting reworked into an iShellPro-style dialog: 19 per-rule presets + fully customisable colours (#hl-settings-hifi).** The old "one master switch + log/devops bundles + six semantic colours" becomes one dialog with two tabs (Presets / Custom), reachable from Settings › Terminal › Output highlighting and from a new title-bar shortcut — both open the same root-level dialog.
  - Engine: the preset table doubles as the priority order (URL/IP/date/UUID/log level/size/path/number/table rule) with claim-style matching — each rule only paints runs the program left uncoloured, first hit wins, and custom rules run ahead of all presets. Preset colours ride the theme's ANSI slots (so terminal highlighting re-skins with the theme, same source as the UI swatches); custom colours are absolute RGB.
  - Three real regex bugs fixed (no lookbehind in Rust's regex): the path rule uses a leading capture-group placeholder and paints only the path itself (the old pattern never matched absolute paths), the table separator anchors the whole line (the old one only caught the first half), and IPv6 now requires `::` or ≥4 groups — the wide intermediate pattern painted clock times like `10:20:30` as IPv6. Word-level presets (log levels/months/size units/URL schemes) are case-insensitive: `ERROR` is as common as `error` and the old engine matched it.
  - Colour picking reuses the accent-colour machinery: a 47-swatch preset grid (same source as the accent grid), a recents row, and "Palette ›" into the shared `ColorPickerDialog`; the `PickerMath.fit-hex` contrast guard is unchanged.
  - Rainbow parentheses get their own switch (not a regex): seven colours cycle by nesting depth, only the brackets are painted, and the depth stack continues across runs.
  - Also applied the 2026-10-17 "no popup shadows" decision to the leftovers (three hover tips, both hover cards, the group manager's dropdowns/overlays), turning `tests/dialog_scrim_drag.rs` green; its scrim-colour assertions now compare in premultiplied space (un-premultiplying a half-transparent pixel amplifies 8-bit truncation ~3× — measured 8 off at the 57% stop).

- **暗色主题的日期列不再隐形：ANSI 槽 8 统一提到 ≥4.5:1（#ansi-dark-grey）。** 用户反馈「黄色的颜色在日期上，任何主题都不清晰」。取证：终端里 `ls -l` / eza 的**日期列吃槽 8 (brightBlack)**（同一处取证见 #ansi-exempt-slots：「整片列表的日期列是槽 8」）。亮色 14 套当时已压到 ≥4.7，但**暗色 14 套按「brightBlack 只弱不判」的终端惯例原样保留** —— 实测 1.00~3.66:1：**solarized·暗 的 `#002B36` 与 tbg 同色（1.00，日期完全不可见）**、flexoki 1.28、nord 1.69、graphite 2.06、gruvbox 2.09、darcula 2.21、catppuccin 2.46、onedark 2.67、everforest 2.79、ayu 2.78、dracula 3.03、rosepine 3.42、kanagawa 3.33、mterm 3.66。
  - 修法：只动暗色档的槽 8；保色相、**提亮**（暗底方向与亮色那轮相反，绝不压深）、饱和封顶 0.18（solarized 的原值 H=192°/S=100%，不封顶会提成刺眼青绿），目标 4.6:1（验收线 4.5 留 0.1 余量）。提亮后仍须**弱于槽 7**（槽 8 是辅助信息弱化档，不能抬成正文）。
  - 真源 `scripts/_ansi-group.json` → `node scripts/gen_theme_rust.js` 重生成 `palettes.rs`；修正脚本 `scripts/_audit/fix_dark_grey_slot.py`，`_meta.ansi_check` 同步改成「暗色槽 8 同判 ≥4.5」。
  - 回归 `tests/ansi_light_yellow.rs`：暗色用例从「槽 7/8/15 逐值不变」改为「**槽 8 ≥4.5 且弱于槽 7**，槽 7/15 逐值不变」。
  - 代价：同一槽位被 `vim` 注释、`ls` 的暗淡信息共用，它们也会一起变亮一档 —— 客户端无法区分用途。
- **Dark themes' date column is visible again: ANSI slot 8 raised to ≥4.5:1 everywhere (#ansi-dark-grey).** Reported as "the yellow on the date is unreadable in every theme". Forensics: the date column of `ls -l` / eza uses **slot 8 (brightBlack)** — the same finding recorded in #ansi-exempt-slots. The 14 light palettes were fixed then (≥4.7), but the 14 dark ones were left verbatim under the "brightBlack is dim by convention" exemption. Measured against their own tbg they sat at **1.00–3.66:1**: **solarized-dark's `#002B36` equals tbg (1.00 — the date is literally invisible)**, flexoki 1.28, nord 1.69, graphite 2.06, gruvbox 2.09, darcula 2.21, catppuccin 2.46, onedark 2.67, everforest 2.79, ayu 2.78, dracula 3.03, rosepine 3.42, kanagawa 3.33, mterm 3.66.
  - Fix: only slot 8 of the dark palettes; hue held, lightness **raised** (the opposite direction from the light pass — never darkened), saturation capped at 0.18 (solarized's source colour is H=192°/S=100% and would otherwise lift into a loud cyan), target 4.6:1 for a 0.1 margin over the 4.5 line. Slot 8 must still stay **weaker than slot 7** so the "secondary information" tier doesn't collapse into body text.
  - Source of truth `scripts/_ansi-group.json` → regenerated `palettes.rs` via `node scripts/gen_theme_rust.js`; the fixer is `scripts/_audit/fix_dark_grey_slot.py`, and `_meta.ansi_check` now states that dark slot 8 is judged at ≥4.5 too.
  - Regression in `tests/ansi_light_yellow.rs`: the dark case changes from "slots 7/8/15 verbatim" to "**slot 8 ≥4.5 and weaker than slot 7**, slots 7/15 verbatim".
  - Trade-off: the slot is shared with `vim` comments and `ls`'s dimmed fields, so those get one step brighter as well — the client cannot tell the uses apart.

- **命令栏输入框：聚焦态与「新建会话」对话框同款，聚焦即隐藏提示语（#cmdbar-focus）。** 用户附截图：「输入框的聚焦样式需要修改下，改成和新建会话同款」「聚焦之后，输入框里面的提示信息需要自动删除掉」。
  - 聚焦态 = `1px accent 边 + 3px accent-soft 柔光环`，与 `MiniInput` / `LabeledInput`（新建会话）、主题搜索框同一套聚焦语言（光环 x/y −3、宽高 +6、圆角 +2 逐值一致）。输入框自身 `clip: true`（多行输入滚动必须裁），所以外面包了一层不裁的容器专门画光环。
  - 占位提示改为 `text == "" && !has-focus`：聚焦后不再压在光标 / IME 预编辑上（与对话框的 "Hide on focus" 一致）。边框色带 120ms 过渡，和主题搜索框同节奏。
  - 历史命令按钮的图标从 `expand-less`（一个「^」，与左边「快捷」按钮撞脸）换成 `command-history`（`assets/icons/command-history.svg`：钟表 + 回转箭头）；尺寸收到 **12px**（同框的 `^` 是 13px、圆形字形更重，等尺寸会压过左右文字），常态**不再铺灰底** —— 图标按钮没有文字，一块 28×26 的实心灰块比两侧带字的胶囊更抢眼；悬停/展开时才给 `bg-hover` 当可点反馈。
  - 新增渲染级回归 `tests/cmdbar_focus.rs`：离屏量「两侧 1px accent 边框 / 3px 光晕色值 = mix(栏底, accent, accent-soft) / 提示语像素数 200+ → 0 / 历史按钮常态像素 = 栏底色」。
- **Command-bar input: focus state matches the New Session dialog, and the hint hides on focus (#cmdbar-focus).** Per the user's screenshots: "the input's focus style should match New Session" and "the hint inside the input should disappear once focused".
  - Focus = `1px accent border + 3px accent-soft halo`, the same focus language as `MiniInput` / `LabeledInput` (New Session) and the theme search box (halo x/y −3, size +6, radius +2, identical values). The input itself must keep `clip: true` for multi-line scrolling, so the halo lives on a non-clipping wrapper around it.
  - The placeholder now requires `text == "" && !has-focus`, so it no longer sits under the caret / IME preedit (the dialog's "Hide on focus" behaviour); the border colour animates over 120 ms like the search box.
  - The history button's icon switched from `expand-less` (a "^" that read the same as the Quick button) to `command-history` (`assets/icons/command-history.svg`: clock + rewind arrow), now drawn at **12px** (the neighbouring "^" is 13px, and a round glyph carries more weight at the same size); its resting **grey background is gone** — an icon-only control in a 28×26 solid block was louder than the labelled pills beside it — with `bg-hover` kept as the hover/open feedback.
  - New render-level regression `tests/cmdbar_focus.rs`: offscreen checks for both 1px accent borders, the halo colour equal to `mix(bar bg, accent, accent-soft)`, the hint's pixel count dropping from 200+ to 0, and the history button's resting pixel matching the bar background.

- **壁纸参数分层：主题 × 壁纸 × 作用域，各自一套（#wallpaper-scope-params / #wallpaper-first-defaults）。** 用户定稿：「一个主题有多个壁纸，一个壁纸有作用域两种方式，每个方式都对应的参数，这些关系不要混乱了；切换主题，上一个主题的壁纸参数不要带入进来，都是初始化，除非设置过」。
  - 存储：`wallpaper_by_theme`（每主题选中的壁纸）、`wallpaper_scope_by_theme`（每主题的作用域）、`wallpaper_settings`（键 = `主题|壁纸|作用域`，值 = 四区不透明度 + 壁纸透明度 + 填充方式）。切主题读那个主题自己的壁纸；没配过 = 无壁纸，不会把上一套带过来。
  - **首次给某个组合设值就落一套「看得见壁纸」的默认值**（整个界面：标题栏 22% / 标签栏 30% / 侧栏 28% / 终端 40% 透；终端：终端区 28% 透、其余三区原色 —— 实机截图校准过，再透一档侧栏正文就糊了）——出厂四区全 0% 会把壁纸完全盖住，用户实测「设了壁纸满屏纯黑」。
  - 迁移 rev7：老配置把当前壁纸/作用域搬进新表；**用户调过的数值只沿用到他正在用的那个作用域**，另一种作用域落首设默认值（切过去不再黑）。
  - `ThemeState` 改为携带「已解析的一套数值」（`Config::effective_appearance()`），`resolve()` 不再自己按明暗选槽。
- **Wallpaper settings are now stored per theme × wallpaper × scope (#wallpaper-scope-params / #wallpaper-first-defaults).** Spec: "a theme has several wallpapers, a wallpaper has two scopes, each scope has its own values; switching theme must not carry the previous theme's values over — start fresh unless set".
  - Storage: `wallpaper_by_theme`, `wallpaper_scope_by_theme`, and `wallpaper_settings` keyed by `theme|wallpaper|scope` (four zone alphas + wallpaper transparency + fit). Switching theme loads that theme's own wallpaper; a theme never configured has none.
  - **The first value ever set for a combination seeds defaults that actually show the wallpaper** (whole window: 22/30/28/40% transparency; terminal: 28% on the terminal area, other zones solid — calibrated on real screenshots) — the factory all-opaque zones hid the image completely, reported as "everything is black after I pick a wallpaper".
  - Migration rev7 moves the current wallpaper/scope into the new tables; tuned values carry over **only into the scope in use**, the other scope gets the first-use defaults.
  - `ThemeState` now carries the already-resolved value set (`Config::effective_appearance()`); `resolve()` no longer picks a light/dark slot itself.
- **壁纸作用域新增「主区域」三档（#wallpaper-scope-workspace）。** 用户定稿：「整个界面」仍是默认档（没有壁纸时也是它），新增中间档「主区域」= 标题栏**以下**——左右侧栏、终端、下方面板都有图，只有标题栏保持实心。三档各存一套参数（键 `<主题>|<壁纸>|<作用域>`）；主区域的首设默认值标题栏给 **1.00 实心**（这一档图不进标题栏，半透明透出来是桌面而不是壁纸），其余三区与「整个界面」逐值相同（0.70 / 0.72 / 0.60），两档来回切观感连续。运行时索引统一为 0/1/2（终端由 1 → 2），`自 动`填充按三档各自矩形的比例判定。
- **New "Main area" wallpaper scope (#wallpaper-scope-workspace).** "Whole window" stays the default (also when there is no wallpaper); the new middle option covers everything below the title bar — sidebars, terminal and bottom panels — and keeps the title bar solid. Each scope keeps its own value set (key `<theme>|<wallpaper>|<scope>`); the Main-area defaults use **1.00 for the title bar** (the image does not reach there, so translucency would reveal the desktop) and the same 0.70 / 0.72 / 0.60 for the other three zones as Whole window, so switching between the two is visually continuous. Runtime indices are now 0/1/2 (Terminal moved 1 → 2) and auto-fit uses each scope's own rect ratio.
- **删掉空态提示「从左侧选择一个会话开始」（#empty-hint-removed）。** 会话列表常驻侧栏，这句话没有信息量；空 pane 只保留终端底色那一层（壁纸要靠它按「终端区透明度」雾化，不能一起删）。
- **Removed the empty-pane hint "Pick a session on the left to begin" (#empty-hint-removed).** The session list is always in the sidebar, so the hint carried no information. The term-bg layer stays — the wallpaper needs it to frost by the terminal-zone transparency.
- **滑杆圆点改回高保真样式（#slider-thumb-hifi）。** 用户对比稿子指出圆点不一致：稿子（wallpaper-hifi CSS）是 `14px 实心 + 2px 面板色间隙 + 1px 强调色外环`（`box-shadow: 0 0 0 1px`），实现里少了最外那圈强调环，看着只是一个带暗边的实心点。改成三层同心圆（20px = 1px 环 + 2px 间隙 + 14px 实心），全局 `MTermSlider` 一次改完。
- **Slider thumb matches the hi-fi spec again (#slider-thumb-hifi).** The mock-up's thumb is `14px fill + 2px panel gap + 1px accent ring` (`box-shadow: 0 0 0 1px`); the implementation dropped the outer ring, reading as a plain dot with a dark edge. Now three concentric circles (20px = 1px ring + 2px gap + 14px fill) in `MTermSlider`, so every slider picks it up.
- **分区滑杆的「默认」随上下文变（#zone-default-context）。** 设置页四根分区滑杆的「（默认）」标记与双击复位原先写死 0%（无壁纸档），有壁纸时双击会把「能看见壁纸」的默认值（22% / 30% / 28% / 40%）改回 0% —— 用户反馈「双击都变为了无壁纸时候的默认值」。现在默认值由当前 (主题 × 壁纸 × 作用域) 决定：有壁纸 = 该组合首设默认值，无壁纸 = 0%；Rust 把它作为 `Theme.*-default-transparency` 写进主题，滑杆的标记与复位都读同一份。
- **Zone sliders' "default" now depends on context (#zone-default-context).** The "(default)" marker and double-click reset were hard-wired to 0% (the no-wallpaper value), so with a wallpaper active a double-click threw away the visible-wallpaper defaults (22 / 30 / 28 / 40%). The default now comes from the current theme × wallpaper × scope (first-use defaults when a wallpaper is active, 0% otherwise) and is published as `Theme.*-default-transparency`.
- **壁纸「自动」填充改成铺满（#wallpaper-auto-fit-fill）。** 用户实测「终端作用域上下有空缺」——「自动」的裁切容差只有 25%，宽屏窗口 + 16:9 图片裁切 28% 就被判成「适应」而留白。容差提到 55%：自动一律铺满，只有竖图放进超宽区（硬铺会裁掉主体）才退回「适应」。
- **Auto fit now fills (#wallpaper-auto-fit-fill).** The reported "empty bands above/below the wallpaper in terminal scope" came from the 25% crop tolerance: a 16:9 image in a wide pane needs 28% crop, so auto switched to Fit and letterboxed. The tolerance is now 55% — auto fills, and only an extreme mismatch (portrait image in an ultra-wide pane) falls back to Fit.
- **终端作用域：没有打开 tab 时中间区也有壁纸（#wallpaper-scope-tabless）。** 作用域=终端时壁纸原先画在 `TerminalView` 的 `scroll-clip` 里，没有会话就没有 TerminalView → 中间只剩纯黑。改画在 `AppWindow` 的内容容器（= 整个中间区，空态 / 欢迎页 / 终端 / 命令栏都在其上按各自透明度雾化），与是否打开了 tab 无关。
- **Wallpaper in terminal scope now survives "no tab open" (#wallpaper-scope-tabless).** It used to be drawn inside `TerminalView`'s scroll canvas, which does not exist without a session, leaving the centre solid black. The layer now lives on the content container (the whole central area — empty hint, welcome page, terminal and command bar all frost over it), independent of open tabs.
- **悬浮提示气泡 / 拖拽信息卡跟随「弹窗透明度」（#popup-tip）。** 顶层工具栏 tip、底部品牌按钮气泡、频道条截断提示、会话/标签拖拽卡、SFTP 工具栏气泡、磁盘提示、路径操作气泡、组合框提示的底色统一从 `bg-elevated` / `panel-solid` 换成 `popup-bg-elevated`（= 弹窗档位）；设置页「弹窗透明度」说明补上「悬浮气泡 / 浮动小卡同源」。
- **Hover bubbles and floating info cards now follow "Dialog transparency" (#popup-tip).** Top-bar tips, the brand-button bubble, channel-bar tips, session/tab drag cards, SFTP tooltips, disk tips, path-action bubbles and combo tips all use `popup-bg-elevated` instead of `bg-elevated` / `panel-solid`; the setting's description now says so.
- **托盘：图标建不出来时不再「点了没反应」（#tray-unavailable-fallback / #tray-resurrect-order / #tray-add-retry）。**
  - 某些环境（安全软件/权限策略把进程降到中完整性）`Shell_NotifyIconW(NIM_ADD)` 返回 `ERROR_ACCESS_DENIED`，图标建不出来。旧行为是「最小化到托盘」又弹回关闭确认卡，看起来像按钮失效；现在退化为**普通最小化**（窗口仍在任务栏、会话不断）。
  - 修掉重建顺序：先摘旧句柄再 `NIM_ADD`，避免旧句柄的 `Drop`（`NIM_DELETE`）在新图标建好之后把它摘掉。
  - `NIM_ADD` 失败后在 0.9s / 1.8s 各重试一次（登录初期外壳未就绪的瞬时失败能自愈）。
- **Tray: a failed icon no longer means "the button does nothing" (#tray-unavailable-fallback / #tray-resurrect-order / #tray-add-retry).** In environments where a security policy drops the process to medium integrity, `Shell_NotifyIconW(NIM_ADD)` fails with `ERROR_ACCESS_DENIED`. "Minimize to tray" now falls back to a normal minimize (window stays in the taskbar, sessions keep running) instead of bouncing back to the close dialog. Also fixed the re-add ordering (drop the old handle before ADD, so its `Drop` cannot delete the fresh icon) and added two short retries after a failed ADD.

- **「作用域」改成真正的绘制区域：作用域之外一律没有壁纸，与透不透明无关（#wallpaper-scope-clip）。** 用户定稿：「作用域的界限是壁纸在哪个地方生效，其余地方都没有壁纸的内容，不管是否透明」。
  - 旧实现是**遮挡式**：壁纸恒铺满全窗，作用域=终端时用 `zones-masked` 把标题栏/标签栏/侧栏强制成不透明去盖。后果：① 那三根滑杆在该模式下失效；② 分区本身不够透时（出厂默认四根全 0% 正是如此）壁纸被彻底盖住 → 「设置了壁纸却看不到」。
  - 新实现：**整窗** = 根层 `Image` 铺满全窗（同旧行为）；**终端** = 根层不画，改由 `TerminalView` 在终端文本区内画一层（`central` 减去底部命令栏），只被 `term-bg` 按「终端区透明度」雾化。`zones-masked` 删除 → 四根滑杆在两种作用域下都真实生效。
  - 「自动」填充的判定基准随之与绘制矩形一致（整窗用窗口比例、终端用终端区比例），设置页提示行不再口不对实。
  - 设置页「作用域」文案按新语义重写。
  - 验证：离屏探针推入一个真实 pane/terminal 后采样 —— 作用域=终端时终端文本区 = 品红与 `term-bg` 的混合色，左栏/标题栏**完全无品红**；作用域=整窗时左栏同样有品红。`cargo test --lib` 483 通过。
  - 已知取舍：作用域=终端且**没有打开任何会话**（欢迎页）时，界面上任何地方都不会出现壁纸——欢迎页不属于终端区。需要「欢迎页也显示」的话再议。
- **Wallpaper scope is now a real drawing region: outside the scope there is no wallpaper at all, transparent or not (#wallpaper-scope-clip).** Spec from the user: "the scope boundary is where the wallpaper takes effect; everywhere else has no wallpaper content, regardless of transparency".
  - The old implementation *masked* instead of scoping: the image always filled the window and, in terminal scope, `zones-masked` forced the titlebar / tab bar / sidebars opaque to cover it. That killed those three sliders in that mode, and with the new all-opaque factory defaults it hid the wallpaper completely — "I set a wallpaper and can't see it".
  - Now: **Whole window** = the root-level `Image` fills the window (as before); **Terminal** = the root layer is skipped and `TerminalView` draws its own layer inside the terminal text area (`central` minus the command bar), frosted only by `term-bg` per the terminal transparency. `zones-masked` is gone, so all four sliders work in both scopes.
  - The "auto" fit decision now uses the same rectangle that is actually drawn, so the settings hint matches reality.
  - Verified with an offscreen probe (a real pane/terminal pushed into the model): in terminal scope the terminal area shows the wallpaper blended with `term-bg` while the sidebar and title bar contain **no wallpaper pixels at all**; in whole-window scope the sidebar does show it. `cargo test --lib` 483 passing.
  - Known trade-off: in terminal scope with **no session open** (welcome page) no wallpaper appears anywhere — the welcome page is not the terminal area.
- **侧栏内部的实心表面也跟「侧栏透明度」走了（#sidebar-surface-alpha）。** 用户圈出侧栏里的搜索框、新建会话按钮、组头行与计数徽标、右栏 CPU/内存/交换进度条轨道和两张迷你图，反馈「这些地方设置透明度不生效」——它们用的都是与分区无关的实心常量（`bg-root` / `bg-elevated` / `bg-panel-alt` / `bg-active` / `accent`）。
  - 全部换成侧栏分区令牌：`bg-root → zone-sidebar-root`、`bg-panel-alt → zone-sidebar-alt`、`bg-elevated → zone-sidebar-elev`、`bg-active → zone-sidebar-active`（新增）。
  - `Sparkline` 新增 `ground` 属性（默认仍是 `bg-root`，侧栏传 `zone-sidebar-root`）；`PrimaryButton` 新增 `surface-alpha`（默认 1.0，侧栏「新建会话」传 `Theme.sidebar-alpha`）——两处默认值不变，弹框里的同类控件行为不动。
  - 保持不变（有意）：悬停高亮 `bg-hover`（本来就是 8% 淡染）、浮层 tooltip、拖动 ghost 卡（设计上就要求实底）。
  - 验证：离屏探针在侧栏三档采样 alpha —— 透明 0% / 50% / 100% 时搜索框与新建会话按钮为 **255 / 127 / 0**（改前恒 255）；`cargo check` 通过。
- **Surfaces inside the sidebar now follow "Sidebars" transparency too (#sidebar-surface-alpha).** The search field, the New-session button, group-header rows and their count badges, the right panel's CPU/Memory/Swap bar tracks and both sparklines were all painted with zone-independent solid constants (`bg-root` / `bg-elevated` / `bg-panel-alt` / `bg-active` / `accent`), so the slider never reached them.
  - All switched to zone tokens: `bg-root → zone-sidebar-root`, `bg-panel-alt → zone-sidebar-alt`, `bg-elevated → zone-sidebar-elev`, `bg-active → zone-sidebar-active` (new).
  - `Sparkline` gained a `ground` property (still `bg-root` by default; the sidebar passes `zone-sidebar-root`); `PrimaryButton` gained `surface-alpha` (1.0 by default; the sidebar's New-session button passes `Theme.sidebar-alpha`). Defaults keep dialogs unchanged.
  - Deliberately unchanged: hover tint `bg-hover` (already an 8% wash), floating tooltips, and the drag ghost card (solid by design).
  - Verified: an offscreen probe sampled alphas inside the sidebar at 0% / 50% / 100% — the search field and the New-session button now read **255 / 127 / 0** (they were a constant 255 before); `cargo check` passes.
- **左右两栏视觉统一：同档透明度下不再是「左边实、右边透」（#panel-layer-parity / #right-panel-no-anim）。** 用户反馈「切换时右侧面板显示方式和左侧不一样，右侧像从上往下长出来，能看出过程」。
  - `zone-sidebar` 被**重复叠层**：左栏 = 容器 + `LeftSidebar` 根 + 紧凑卡片 = 3 层，右栏 = `RightSidebar` 根 + 内层卡片 = 2 层。半透明时实际不透明度是 1-(1-a)ⁿ —— 滑杆写 50%，左边实际 87.5%、右边 75%，两边既不一致、也不等于滑杆值。现在两侧都只留**根那一层**（容器与内层卡片改透明），同一档数值两边逐像素一致，且「透明 50%」真的就是 50%。
  - 右侧面板上还有唯一一处尺寸动画 `animate width, height {150ms}`（左栏从来没有）。任何宽高变化（窗口缩放、折叠展开、重排）都会播一段生长过程，看上去就是「右侧从上往下出现」。已删除：拖动本来就是 0ms 跟手，删掉不影响拖拽，折叠/展开与左栏一样瞬时。
- **Both sidebars now render identically at the same transparency (#panel-layer-parity / #right-panel-no-anim).** Reported as "the right panel shows up differently from the left — it looks like it grows from the top and you can watch it happen".
  - `zone-sidebar` was painted in **multiple stacked layers**: left = container + `LeftSidebar` root + compact card (3), right = `RightSidebar` root + inner card (2). With alpha the effective opacity is 1-(1-a)ⁿ, so a 50% setting rendered 87.5% on the left and 75% on the right — neither matching the slider. Both panels now keep exactly **one** background layer (container and inner cards are transparent), so the same value renders identically on both sides and "50%" really means 50%.
  - The right panel was also the only one with a size animation (`animate width, height {150ms}`; the left never had one). It replayed on any width/height change (window resize, collapse/expand, re-layout), which read as "grows from the top". Removed: dragging was already 0ms so the drag still tracks the cursor, and collapse/expand is now instant like the left panel.
- **下方面板的透明度真正透到桌面（#bottom-dock-alpha）。** 上一版把命令栏 / 文件传输面板换成了 `zone-sidebar-*` 令牌，但用户实测「侧栏拉满它们还是不动」——因为它们长在终端区**内部**：`TerminalView` 根底铺着不透明的 `term-bg`，`app.slint` 的内容容器又铺了一层 `term-bg`，面板再透也只能透出这两层几乎同色的终端底。修法：终端底色**下沉到画布**（`scroll-clip` 自己画 `term-bg`），`TerminalView` 根底与内容容器改透明，空态占位自己铺 `term-bg`。层级变为「命令栏/SFTP 面板 → … → 窗口底（无壁纸时即桌面）」，与左右侧栏一致；终端文本区、壁纸雾化、作用域语义均不变。
- **Bottom panels now really see through to the desktop (#bottom-dock-alpha).** The previous build switched the command bar / file-transfer panel to `zone-sidebar-*`, but they still looked opaque: they live **inside** the terminal area, whose `TerminalView` root and the `app.slint` content container both painted an opaque `term-bg`, so translucency only revealed another near-identical terminal colour. Fix: the terminal background moved **down into the canvas** (`scroll-clip` paints `term-bg`), the `TerminalView` root and the content container became transparent, and the empty-state placeholder paints `term-bg` itself. The panels now compose against the window base (the desktop when no wallpaper) exactly like the sidebars; the text area, wallpaper frosting and scope semantics are unchanged.
- **下方面板跟随「侧栏」透明度 + 出厂四根默认全 0%（#sidebar-bottom-panel / #zone-alpha-default0）。** 用户反馈「设置了侧栏的透明度，下方面板这些地方也应该跟着变」。
  - 根因：命令栏、文件传输面板、停靠的快捷命令面板用的是 `bg-panel` / `bg-panel-alt` / `bg-root` 这类**不透明常量**，根本不读分区 token，所以侧栏滑杆对它们完全无效。
  - 修法：新增 `zone-sidebar-alt` / `zone-sidebar-elev` / `zone-sidebar-root` 三个分区 token（分别是 panel-alt / elevated / root 三档的 frost 版本，**alpha = 1 时与原色逐像素相同**），把命令栏、SFTP 面板（主体、工具栏、目录树标题、文件列头、折叠态、以及命令栏上沿那条 1px splitter 缓冲带）、停靠的快捷命令面板全部切过去。输入框 / 按钮 / 悬停态仍是控件色，不随面板一起变透（控件要能读）。
  - **出厂默认改为四根全 0%**（`DEFAULT_PANEL_ALPHA` 0.85 → 1.0）：默认就是每个主题原本的主题色，不混桌面/壁纸；要雾化自己拉滑杆。双击分区名复位同样回到 0%。已有配置里存过的值保持不变。
  - 验证：token 探针 —— alpha 1.0 时三档颜色与原色逐像素相同（a=255）、15% → a=217、100% → a=0；`cargo test --lib` 483 通过（迁移链断言同步改为 1.0）。
- **Bottom panels now follow the "Sidebars" transparency; factory default for all four zones is 0% (#sidebar-bottom-panel / #zone-alpha-default0).** Reported as "when I change the sidebars transparency, these places should change too".
  - Cause: the command bar, the file-transfer panel and the docked quick-command panel used opaque constants (`bg-panel` / `bg-panel-alt` / `bg-root`) instead of zone tokens, so the sidebars slider never reached them.
  - Fix: added `zone-sidebar-alt` / `zone-sidebar-elev` / `zone-sidebar-root` (frosted versions of the panel-alt / elevated / root tiers — **identical to the original colours at alpha = 1**) and switched the command bar, the SFTP panel (body, toolbar, tree header, column header, collapsed state and the 1px splitter buffer above the command bar) and the docked quick-command panel to them. Inputs, buttons and hover states keep control colours so they stay readable.
  - **Factory default is now 0% for all four zones** (`DEFAULT_PANEL_ALPHA` 0.85 → 1.0): out of the box you get each theme's own colours with no desktop/wallpaper haze; raise the sliders to frost. Double-click reset returns to 0%. Values already stored in existing configs are untouched.
  - Verified: a token probe shows the three tiers pixel-identical to the original colours at alpha 1.0 (a=255), a=217 at 15% and a=0 at 100%; `cargo test --lib` 483 passing (migration-chain assertions updated to 1.0).
- **分区透明度现在是「真透明」：无壁纸时四根滑杆直接透出桌面；双击分区名恢复默认（#zone-alpha-desktop / #zone-label-dblclick）。** 用户反馈「没有壁纸时这四个设置透明没生效，应该和弹框设置透明的时候一样」，以及「双击分区名自动调整为默认值」。两处根因：
  - **滑杆被硬编码短路**：`theme.slint` 里 `titlebar-alpha / tabbar-alpha / sidebar-alpha` 在「作用域 = 终端」时被强制成 `1.0`（当年为了用不透明面板把壁纸挡在终端区外）—— 这三根滑杆在那套配置下等于失效，而无壁纸时它们又只透出一个与面板色几乎相同的背板，肉眼完全看不出变化。
  - **窗口本身不透明**：主窗是普通不透明窗口，任何半透明都只叠在窗口底色上，永远透不到桌面。
  - 现在：① 主窗以 **per-pixel alpha** 创建（`with_transparent(true)`，winit 的透明是创建期属性），Slint 窗口底改为 `transparent`，四分区按各自 alpha 叠在桌面上 —— 0% = 完全实心，100% = 直接看到桌面，与「弹窗透明度」同一套语义；有壁纸时壁纸层铺满全窗，透出的就是壁纸。② 强制不透明的规则收窄为**只在「有壁纸 + 作用域 = 终端」**（那时壁纸铺满全窗，三区必须挡住它才有作用域语义），其余情况（尤其无壁纸）一律按滑杆真实生效。③ 删掉只为“让透明看得出来”而压暗的 `zone-backdrop` 背板（窗口真透明后它既多余又挡桌面）。
  - **双击复位**：四根分区滑杆的**分区名 / 读数**整行可双击恢复默认（15 / 15 / 15 / 0），与滑杆本体双击同一条路径（先 `changed(默认值)` 再 `reset()`，否则 reset 读到的是旧值）；「壁纸透明度 / 弹窗透明度 / 遮罩浓度」三行的标题读数同样支持。
  - 验证：① 生效 alpha 四组合探针 —— 无壁纸（两种作用域）= 0.00/0.20/0.00/0.29（全按滑杆）、有壁纸+终端 = 1.00/1.00/1.00/0.29（三区挡壁纸）、有壁纸+整窗 = 按滑杆；② 真机（200%、独立数据目录、用户同一份配置）截图：无壁纸 + 作用域=终端 + 100/80/100/71% 时标题栏/标签栏/侧栏/终端区**全部**透出桌面（修复前只有终端区透出）；③ 离屏探针派发双击：依次点四根滑杆的标题区，落盘回调精确为 `titlebar=0.85`、`tabbar=0.85`、`sidebar=0.85`、`term=1.00`。
- **Zone transparency is now real desktop transparency; double-clicking a zone name restores its default (#zone-alpha-desktop / #zone-label-dblclick).** Reported as "with no wallpaper the four transparency sliders do nothing — they should work like Dialog transparency", plus "double-clicking a zone name should reset it". Two causes:
  - **The sliders were short-circuited**: in `theme.slint`, `titlebar-alpha / tabbar-alpha / sidebar-alpha` were forced to `1.0` whenever the scope was "terminal" (an old trick to keep the wallpaper out of the sidebars). With that scope those three sliders were dead, and with no wallpaper they only revealed a backdrop that is almost the same colour as the panels — invisible in practice.
  - **The window itself was opaque**, so translucency could never reach the desktop.
  - Now: (1) the main window is created with **per-pixel alpha** (`with_transparent(true)`; transparency is creation-time in winit) and the Slint window background is `transparent`, so each zone composites over the desktop by its own alpha — 0% solid, 100% fully see-through, exactly the semantics of "Dialog transparency"; with a wallpaper the image fills the window and the zones reveal the image instead. (2) The forced-opaque rule is narrowed to **wallpaper active + scope = terminal** only (there the wallpaper fills the window and the three zones must mask it); every other case, especially no wallpaper, honours the sliders. (3) The `zone-backdrop` layer (added only to make the old effect visible) is gone — with a genuinely transparent window it would just block the desktop.
  - **Double-click reset**: the whole name/readout row of each of the four zone sliders is double-clickable (15 / 15 / 15 / 0), sharing the slider's own path (`changed(default)` then `reset()`, otherwise reset reads the stale value); the "Wallpaper / Dialog transparency" and "Scrim strength" titles work the same way.
  - Verified: (1) an effective-alpha probe over four combinations — no wallpaper (both scopes) = 0.00/0.20/0.00/0.29 (sliders honoured), wallpaper + terminal = 1.00/1.00/1.00/0.29 (zones mask the wallpaper), wallpaper + window = honoured; (2) on-device at 200% with a separate data dir and the user's own config: no wallpaper, scope = terminal, 100/80/100/71% — all four zones now reveal the desktop (before, only the terminal did); (3) an offscreen probe dispatched double-clicks on all four headers and recorded `titlebar=0.85`, `tabbar=0.85`, `sidebar=0.85`, `term=1.00`.

- **外观页按高保真收尾：壁纸库多张化 + 填充方式「自动」+ 常用主题 6 位（#wallpaper-library / #wallpaper-auto-fit / #theme-gallery）。** 逐条对齐 `docs/design/wallpaper-hifi-2026-10-17.html` 定稿：
  - **壁纸库 = 数据目录 `wallpapers/` 文件夹**（用户约定：该文件夹即列表来源）。一条画廊统一为「无 → 内置 5 套 → 自定义图（可多张）→ ＋上传图片」；自定义卡悬停右上角出现 ×（悬停变红，点在 × 上才是删除，点其它位置是选中 —— 只有一张 TouchArea，用坐标判定，避免两个重叠命中区互相抢 hover 导致 × 闪烁）；选中自定义壁纸后下方出现工具条「在文件夹中显示 / 删除这张壁纸」。上传的图片**复制**进库（原件不动，重名自动 ` (2)`），删除有二次确认且**只允许删库内文件**（老配置里指向库外的路径在启动时复制入库并改存库内路径，原件不动）。切到外观页会重扫文件夹，在外面放进来的图也会出现。
  - **填充方式新增「自动」（默认）**：按图片真实比例与**作用域区域**（整个界面 = 窗口；终端 = 内容区）的比例判定 —— 比例差 ≤ 25% 用「填充」，差得远用「适应」（不裁切）；设置页在分段下方用 accent 色写出结论（「自动：这张图在「整个界面」的比例按「适应」显示」）。手动三档直通；老配置里显式存过的值保持不变。
  - **显示分节顺序**改为 作用域 → 填充方式 → 壁纸透明度 → 分区透明度（原「方式」分节删除）；未选壁纸时前 3 项整行灰化且不可点（`SegGroup.enabled`）。
  - **常用主题快捷位 4 → 6**（`FAVORITE_LIMIT`、外观页说明、主题浮层「常用已满」提示三处同步）。
  - **分区透明度文案与壁纸解耦**：明确「与壁纸无关，无壁纸时同样可调（透出窗口底色）」，删掉上一版「没有壁纸变化很小」的提示行。
  - 顺带修掉页面文案里的 Markdown `**`（Slint 不解析富文本，之前会原样画出星号）。
  - 验证：`cargo check` 零新增警告；`cargo test --lib` 483 通过（新增 `wallpaper::library` 两条单测 + `wallpaper_fit_keeps_auto` 回归）；`dialog_scrim_drag` 4/4 通过；离屏探针渲染外观页并**真派发点击** —— 点卡片中心发出 `set-wallpaper("<路径>")`、点右上角 × 发出 `remove-wallpaper("<路径>")`（互不串），无壁纸帧确认三项灰化 + 「未选壁纸：此项暂不可用」。
- **Appearance page finished against the hi-fi: wallpaper library, automatic fill, six theme slots (#wallpaper-library / #wallpaper-auto-fit / #theme-gallery).** Following `docs/design/wallpaper-hifi-2026-10-17.html`:
  - **The wallpaper library is the `wallpapers/` folder of the data directory.** One gallery row: None → 5 built-ins → every custom image → Upload. Custom cards reveal a × on hover (red when hovered; clicking the × deletes, clicking anywhere else selects — a single TouchArea decides by coordinates, so two overlapping hit areas cannot steal each other's hover and make the × flicker). A toolbar under the gallery ("Show in folder" / "Delete this wallpaper") appears while a custom wallpaper is selected. Picked images are **copied** into the folder (originals untouched, ` (2)` on name clashes); deletion asks first and only touches files inside the folder (legacy configs pointing outside are copied in at startup and re-pointed, original kept). Entering the page rescans the folder, so images dropped in from Explorer show up.
  - **Fill mode gained "Auto" (now the default)**: compares the real image ratio with the **scope-area** ratio (whole window, or the content area for terminal scope) — within 25% it uses Fill, otherwise Fit (no cropping); the settings page prints the decision in accent colour. The three manual modes still apply verbatim; explicitly stored values from older configs are kept.
  - **Display order** is now Scope → Fill mode → Wallpaper transparency → Zone transparency (the separate "Fit" section is gone); with no wallpaper the first three rows grey out and stop accepting clicks (`SegGroup.enabled`).
  - **Theme quick slots 4 → 6** (`FAVORITE_LIMIT`, the page copy and the "favourites are full" hint all updated).
  - **Zone-transparency copy is decoupled from the wallpaper** ("works with no wallpaper too — reveals the window base"), and the previous "barely visible without a wallpaper" hint is removed.
  - Also stripped Markdown `**` from the page copy (Slint has no rich text, so the asterisks were being drawn literally).
  - Verified: `cargo check` with no new warnings; `cargo test --lib` 483 passing (two new `wallpaper::library` unit tests plus the `wallpaper_fit_keeps_auto` regression); `dialog_scrim_drag` 4/4; an offscreen probe rendered the page and **dispatched real clicks** — clicking a card centre fires `set-wallpaper("<path>")` while clicking the top-right × fires `remove-wallpaper("<path>")` (never both), and the no-wallpaper frame confirms the greyed rows and the "No wallpaper: not available" note.

- **主界面区域边界线统一为 1 设备像素：修掉"不同区域重叠时线条加粗"（#line-dedup）。** 用户截图圈出 6 处：左右侧栏边界线、标题栏底线在左右侧栏上方都变成了 2 像素。真机 + 代码核对出三处叠线：① 左栏容器里残留一条 `1px`（**逻辑**像素）边线——200% 下就是 **2 设备像素**；② compact 侧栏卡片、右栏根节点各有一圈 hairline 边框，与标题栏底线 / splitter 可见线 / 窗口描边**相邻叠画**（相邻两行/列各一条 = 2 像素）；③ splitter 可见线原来"在 4px 命中区里居中"，125%/150%/200% 这类缩放下会落在半设备像素上，被抗锯齿摊成两个半透明像素（又粗又虚）。现在每条区域边界**全屏只保留一条线**：由 splitter 可见线承担——位置经 `Theme.snap-to-device` 对齐侧栏真实边缘（dock 左/上用边界内侧一设备像素，dock 右/下用边界本身），厚 1 设备像素；左栏容器那条逻辑像素边线、compact 卡片边框、右栏根边框全部删除（它们的边由标题栏底线 / splitter 线 / 窗口描边各自负责）。200% 真机（软件渲染 + femtovg 两套）实测：左/右边界、标题栏底线在左右侧栏上方、以及用户圈出的各个高度，全部恰好 1 物理像素 `border-subtle`，无第二条线。
- **左右两条可拖宽的边界线 hover 高亮（#line-dedup）。** hover 或拖动时边界线整条转强调色（120ms 过渡），仍是 1 设备像素、不变粗；左右两条行为一致。真机验证：hover 时该像素 = accent `(31,200,180)`（mterm-dark），左右邻像素仍是面板/终端底色（没有第二条、没有加粗）；新增离屏像素回归 `tests/sidebar_boundaries.rs`（边界 1 像素 + 标题栏底线在侧栏上方 1 像素 + hover 变 accent + 移开复原）。
- **Zone boundaries are a single device pixel again; no more doubled lines where regions meet (#line-dedup).** The user circled six spots where the left/right sidebar boundary and the title-bar underline above each sidebar looked twice as thick. Three overlapping line sources were found: (1) the left sidebar container still drew a `1px` **logical** line — 2 device px at 200%; (2) the compact sidebar card and the right sidebar root each had their own hairline border, drawn *next to* the title-bar underline / splitter line / window frame (two adjacent rows/cols = 2 px); (3) the splitter's visible line was centred inside its 4 px hit area, so at 125/150/200% it landed on a half device pixel and was antialiased into two half-transparent columns. Now each zone boundary has exactly **one** line: the splitter's visible line, snapped to the device grid and aligned with the sidebar's real edge (one device px inside for left/top docks, the boundary itself for right/bottom docks), 1 device px thick. The left container's logical-pixel line, the compact card border and the right sidebar root border were removed; their edges are covered by the title-bar underline, the splitter line and the window frame respectively. On-device at 200% (software and femtovg renderers) every circled spot is exactly one physical pixel of `border-subtle`, with no second line.
- **The two draggable sidebar boundaries highlight on hover (#line-dedup).** Hovering or dragging turns the whole boundary line accent-coloured (120 ms transition) while staying exactly one device pixel thick; both sides behave identically. Verified on-device: the hovered pixel becomes accent `(31,200,180)` (mterm-dark) and its neighbours stay panel/terminal background (no second column, no thickening). New offscreen pixel regression: `tests/sidebar_boundaries.rs` (one-pixel boundaries, one-pixel title-bar underline above the sidebars, accent on hover, restore after leaving).

- **主窗口外框改成「线与窗口边界重合」：修掉四角断线 + 直边 1 像素缝（#win-edge-frame-r7）。** 用户反馈「主界面周围的细线与边界没有真正重叠贴合」。真机实测（200% DPI，把 DWM 系统边框染成主题色后拟合窗口外缘圆）发现两处根因：① **Win11 的 DWM 圆角是 8 DIP**（200% 下 16 物理像素），旧代码 `radius-win = 8px / device-pixel-ratio` 把它当成「固定 8 物理像素」——200% 下自绘角弧半径只有应有的一半，整段落在窗口裁剪区外被 DWM 裁掉，**四角根本没有线**；直边也被圆角切断。② Slint `Path` 渲染前会把路径 fit 进 (元素尺寸 − stroke-width) 再平移 stroke/2（`i-slint-core items/path.rs::fitted_path_events`），旧注释按「1 单位 = 1 物理像素」写坐标，弧又整体缩进约一个 stroke。现在：`radius-win` 改为 **8px 逻辑值**（不再除 DPR）；四条直边贴真实边界（第 0 行/列、倒数第 1 行/列，厚 1 设备像素）；四角描边弧用**反解 viewbox** `2R(R−h)/(2R−h)` 抵消 fit 缩进 —— 弧外缘正好压在 8 DIP 裁剪弧上、厚度 1 设备像素，端点精确落在直边中心线上；Rust 把 DWM 圆角调用结果回写成 `win-rounded`，Win10 / 属性失败时只画直角直边（窗口本身直角，天然贴边）。外框抽成 `ui/window_frame.slint`，新增离屏像素回归 `tests/window_frame_geometry.rs`（旧实现必红）。200% 实测：四边最外一圈像素 = `border-strong`（旧实现最外一圈是内容、线内缩 1 设备像素），四角弧连续且紧贴裁剪边。
- **The main window frame now coincides with the real window boundary (#win-edge-frame-r7).** Reported as "the thin lines around the main window do not really overlap the boundary". On-device measurement at 200% DPI (with the DWM system border tinted, fitting a circle to the window's outer edge) found two causes: (1) the Win11 DWM corner radius is **8 DIP** (16 physical px at 200%), but `radius-win = 8px / device-pixel-ratio` assumed a fixed 8 *physical* px — so at 200% the self-drawn corner arc had only half the required radius and lay entirely outside the window's clip region, i.e. **the four corners had no line at all**, with the straight edges cut by the rounded clip; (2) Slint's `Path` first fits the path into (element size − stroke-width) and then translates by stroke/2 (`i-slint-core items/path.rs::fitted_path_events`), while the old code wrote coordinates assuming 1 unit = 1 physical px. Now `radius-win` is a plain 8px logical value; the four straight edges sit on the true boundary (first/last pixel row and column, 1 device px thick); the corner arc uses an **inverse-fit viewbox** `2R(R−h)/(2R−h)` so its outer edge lands exactly on the 8 DIP clip arc with 1 device px thickness and its endpoints meet the straight edges' centre lines. Rust feeds the DWM corner-call HRESULT back as `win-rounded`; Windows 10 / failed calls fall back to a square frame (the window itself is square there, so the frame is naturally flush). The frame moved into `ui/window_frame.slint` with an offscreen pixel regression test (`tests/window_frame_geometry.rs`, which the old implementation fails). Measured at 200%: the outermost pixel row/column is `border-strong` (previously the outermost pixel was content, with the line inset by 1 device px), and the corner arcs are continuous and hug the clip edge.

- **无壁纸时四区透明度「看不出效果」→ 主窗背板压暗一档，效果与弹窗同级可见；四根分区滑杆改为等宽。** 上一版把滑杆接通了（`zone-frost`），但面板色 `#181A1D` 与窗口底色 `#161619` 只差 2 级，100% 透明肉眼仍不可辨。新增 `zone-backdrop`（主窗背板 = 窗口底色再压暗：深色 32%、亮色 7%），四区透明时透出的就是它 —— 实测 100% 档标题栏 (14,15,17) vs 面板 (24,26,29)，差 9 级；默认 15% 只差 1 级（观感不变）。独立窗口（编辑器/进程/系统信息）仍用 `window-base`，不受影响。
  - 同轮修「拖动时滑杆长度会变」：四根分区滑杆原来用 `horizontal-stretch`，Slint 按「首选宽 + 均分余量」分配，而每列首选宽随数值文字变化（`0%（默认）` ↔ `100%`）→ 四根互相伸缩（实测 181/191/191/213 px）。改为固定份额 `(行宽 − 42px) × 0.25`，实测四根轨道恒为 174px、间距 192px，拖动过程中不变。
- **"No visible effect" for zone transparency without a wallpaper → the main window now has a darker backdrop; the four zone sliders are equal-width.** The previous fix connected the sliders, but the panel colour (#181A1D) and the window base (#161619) differ by only 2 levels, so 100% still looked identical. A new `zone-backdrop` token darkens the main window's base (32% in dark themes, 7% in light); the zones reveal it. Measured: at 100% the title bar is (14,15,17) vs the panel's (24,26,29) — 9 levels; at the default 15% the difference is 1 level (look unchanged). Standalone windows keep `window-base`.
  - Also fixed the slider length changing while dragging: the four zone sliders used `horizontal-stretch`, which Slint distributes as "preferred width + equal share"; since each column's preferred width depends on its value text (`0%（默认）` ↔ `100%`), the four tracks resized each other (measured 181/191/191/213 px). They now get a fixed share `(row width − 42px) × 0.25`; measured tracks are a constant 174 px with 192 px pitch.

- **修「分区透明度」四根滑杆在无壁纸时完全无效。** 用户截图：四区拉到 100% 毫无变化。根因：分区底走 `frost()`，而它在**没有壁纸**时直接返回不透明实色（历史写法），等于把滑杆短路；页面文案却写着「没有壁纸时透出窗口底色、效果较弱」，实现与文案不一致。新增 `zone-frost()`（始终按 alpha 半透明），四分区 + 终端区改用它；菜单/卡片/独立窗口仍走 `frost`，默认观感不变（默认 15% 下与实色只差 1 级）。实测：无壁纸时标题栏 15% = (23,25,27) → 100% = (22,22,25)（窗口底色）。同时补一条上下文提示：未设壁纸时四根滑杆下方显示「只会透出窗口底色、变化很小，选壁纸后效果立即可见」，避免把「无背景可透」误判成功能坏了。
- **Fixed the four zone-transparency sliders doing nothing without a wallpaper.** The zone surfaces went through `frost()`, which returned an opaque solid whenever no wallpaper was active — short-circuiting the sliders even though the copy promised "reveals the window base, effect is weak". A new `zone-frost()` always applies the alpha; the four zones and the terminal use it, while menus/cards/standalone windows keep `frost` (default look unchanged — at 15% the difference is one level). Measured without a wallpaper: title bar at 15% = (23,25,27), at 100% = (22,22,25) (the window base). A contextual hint now appears under the sliders when no wallpaper is set: "only reveals the window base, the change is barely visible — pick a wallpaper to see it".

- **弹窗投影全部删除；「100% 透明仍是黑的」真凶就是它。** 排查：卡片本身 alpha=0，但 `drop-shadow-color: #000000a0`（62.7% 黑）画在它**背后**，透过透明卡片看到的是一块影子 —— 浅色壁纸 249 → 卡片区 93（249 × 0.373），卡片边缘还有 24px 模糊渐变（投影的签名）。该现象在 DPR=1 的离屏探针里不出现（渲染器不在元素内部画投影），只有真机 200% 缩放才暴露，所以前几轮像素测试全绿。用户定稿「全删」：61 处弹窗表面的阴影统一改为 `transparent`，弹窗只靠 1px 描边 + 遮罩分层；新增审计测试 `popup_surfaces_have_no_drop_shadow` 锁死。
- **外观数值改为「明暗各一套」。** 用户反馈「暗主题调好了，切亮主题为什么用暗主题的数值」。config 从 7 个平铺字段改为 `appearance_dark` / `appearance_light` 两个 `AppearanceValues`（四分区 alpha + 壁纸透明度 + 弹窗透明度 + 遮罩浓度）；`resolve()` 先解析主题再按 `p.dark` 选槽，切主题 = 换一套；写入侧 `set_*(v, dark)`，`dark` 取窗口当前的 `dark_mode`。rev 6 迁移：旧值写进**当前主题所属模式**的槽，另一槽留默认 —— 即「某模式第一次使用 = 默认值」。壁纸作用域/填充/遮罩样式保持全局（与明暗无关）；页头补了一句说明。
- **右侧栏：收起不再留 36px 空白竖条；外缘改直角贴边。** 收起时 `sb-w/sb-h` 仍给 36px（当年那个图标条早已被标题栏切换按钮取代），窗口右边留一条 root 底色的空白带（截图实测 35 逻辑 px）—— 与左栏 `left-sidebar-size`「收起 = 0」对齐；右侧面板外缘的 `border-radius: radius-md` 让左上/左下露出一段 1px 弧线（用户圈出），改 0 后描边直角收口，与终端贴合。
- **侧栏标题行的 `<` / `>` 收起箭头删除。** 左栏两处 `CollapseButton`、右栏 `StatusBlock` 内 18px 按钮改为受 `show-collapse` 控制；Windows/Linux 由标题栏按钮（左上 LOGO / 右上切换）承担，macOS 原生标题栏没有该入口 → 传 `!custom-titlebar` 保留箭头，避免 mac 无法收起。同轮把标题栏新增的侧栏切换图标从 `text-secondary` 提到 `text-primary`（截图实测笔画 (122,126,133) vs 三键 (188,190,196)，差 66 级）。
- **Dialogs lose their drop shadows entirely — and that shadow was the real cause of "100% transparency still looks black".** The card itself was alpha 0, but `drop-shadow-color: #000000a0` (62.7% black) was painted *behind* it: a light wallpaper of 249 showed as 93 inside the card (249 × 0.373), with a 24 px blur ramp at the edge (the shadow's signature). It never reproduced in the DPR=1 offscreen probe (the renderer does not paint a shadow inside the element), only on a real 200% display, which is why the earlier pixel tests were green. Per the decision to remove them, all 61 popup surfaces now use `transparent`, leaving the 1 px border and the scrim for separation; a new audit test (`popup_surfaces_have_no_drop_shadow`) enforces it.
- **Appearance values are stored per light/dark mode.** The 7 flat config fields became two `AppearanceValues` slots (`appearance_dark` / `appearance_light`); `resolve()` picks the slot from the resolved palette's `dark` flag, so switching theme switches the whole set; writes go through `set_*(v, dark)` with `dark` taken from the window. Migration rev 6 copies the old single set into the slot of the *current* mode and leaves the other at defaults — i.e. a mode you never touched starts at its defaults. Wallpaper scope/fit and scrim style stay global (they are not mode-dependent); a note was added to the page header.
- **Right sidebar: no 36 px gap when collapsed, square outer corners.** `sb-w/sb-h` still reserved 36 px while collapsed (the icon strip they were for is long gone), leaving a root-coloured band on the right edge; now zero, matching the left sidebar. The panel's `border-radius: radius-md` produced a 1 px arc at the top-left/bottom-left corners; now square so it meets the terminal.
- **The `<` / `>` collapse chevrons on both panels are gone.** Two `CollapseButton`s in the left sidebar and the 18 px button inside the right `StatusBlock` are now gated by `show-collapse`; Windows/Linux use the title-bar toggles, while macOS (no self-drawn title bar) keeps the chevrons via `!custom-titlebar`. The new title-bar sidebar toggle was also brightened from `text-secondary` to `text-primary` (measured stroke (122,126,133) vs the window buttons' (188,190,196)).

- **弹窗「通透度」= 透明度（语义定稿）：100% 完全透出，且与「遮罩浓度」彻底解耦。** 用户澄清：「通透度和遮罩浓度不是一回事，通透度就是透明度；没有壁纸时 100% 就该看到背后的内容」。上一版为了「看起来像玻璃」加的三处隐藏处理全部撤销：
  - ① `popup-alpha` 顶部 0.35 下限 → 改回 **`1 − 透明度` 一条直线**（100% = alpha 0，弹窗表面完全透出）；
  - ② 玻璃高光 `popup-sheen` / `glass()`（面板往白里提 10%）**删除** —— 弹窗表面颜色不再随透明度漂移，`popup-*` 令牌回到「基础色 × alpha」；
  - ③ 遮罩随弹窗透明度自动 +0.10 的加成**删除** —— `遮罩浓度` 滑杆写 32% 就是 32%。
  - 因此弹窗在高透明档下"看着黑"是**真实结果**：背后内容 × 遮罩浓度；要更亮就把遮罩浓度调低（两个参数各管各的）。设置卡 / 会话 / 同步输入 / 确认四处卡片描边回到常规 token（不再随透明度换"玻璃边"）。
  - 文案：设置项「弹窗通透度」→「**弹窗透明度**」（与「壁纸透明度 / 分区透明度」同词），说明改为「0% = 实心，100% = 完全透明（直接看到背后内容）；背后压多暗由遮罩浓度单独决定」，预览卡描述同步。
  - 回归：`dialog_scrim_drag` ②e 从"好看度断言"改为**语义断言** —— `popup-alpha` 必须严格 = 1 − t（57% / 100% 两档），卡片像素必须 = 背景与面板色按该 alpha 的混合，100% 档必须逐像素等于「遮罩后的背景」（面板色零贡献）。上一版的下限/高光写法会在这里直接报红；② 段的遮罩期望也去掉自动加成。
- **"Dialog transparency" now means exactly that: 100% is fully see-through, and it is fully decoupled from "Scrim strength".** The clarification: translucency and scrim strength are two different things; translucency *is* transparency, and at 100% with no wallpaper you should see straight through to what is behind. All three hidden adjustments added in the previous round are reverted:
  - (1) the 0.35 floor at the top of `popup-alpha` is gone — it is a straight **`1 − transparency`** again (100% = alpha 0);
  - (2) the glass sheen (`popup-sheen` / `glass()`, +10% white) is removed, so a popup's colour never drifts with its transparency; `popup-*` tokens are back to "base colour × alpha";
  - (3) the automatic +0.10 scrim boost is removed — a 32% scrim now is 32%.
  - A dialog "looking black" at high transparency is therefore the honest result of *content behind × scrim strength*; lower the scrim to brighten it. The four dialog cards (settings / session / sync-input / confirm) keep their regular border tokens again (no more transparency-driven "glass rim").
  - Copy: the row is renamed "Dialog transparency" (matching "Wallpaper transparency" / "Zone transparency") with the description "0% = solid, 100% = fully transparent (you see the content behind); how dark that content is, is up to Scrim strength"; the preview caption follows.
  - Regression: section ②e of `dialog_scrim_drag` now asserts the **semantics** — `popup-alpha` must equal 1 − t at 57% and 100%, the card pixel must be the backdrop/panel blend at that alpha, and at 100% it must match the scrimmed backdrop pixel-for-pixel (zero panel contribution). The previous floor/sheen implementation fails this test by construction; the scrim expectation in section ② also drops the auto boost.

- **Appearance page rebuilt: theme gallery (≤4 favourites + More) · per-zone transparency · wallpaper scope/transparency · 9 scrim styles with a strength slider · one unified slider style.** Following the approved hi-fi (`docs/design/wallpaper-hifi-2026-10-17.html`), the page is reordered along the "skin flow": **theme gallery** (≤4 quick slots + "More", which opens the full list; starring stays single-source, hover previews after 220 ms and a click commits; the cap drops 8 → **4**, and the gallery is a projection `theme-favorites` of config `favorite_themes`) → accent → wallpaper library → display → fit → dialogs & scrim.
  - **Per-zone transparency**: the single `panel_alpha` splits into title bar / tab bar / sidebars (left + right + bottom panel) / terminal (defaults 15 / 15 / 15 / 0%), backed by `Theme.zone-titlebar` / `zone-tabbar` / `zone-sidebar`; the tab bar no longer borrows the terminal alpha (`tab-bar-bg` / `tab-active-bg` read the tab-bar value); the old 0.30 / 0.75 floors are gone (full 0–100 range).
  - **Wallpaper scope** (whole window / terminal): with "terminal" the image still fills the window at window coordinates (so switching never makes it jump) and the other three zones are forced opaque, leaving only the terminal area to show it; dialogs are never part of the wallpaper.
  - **Wallpaper transparency** (renamed from "visibility", now 1 − visibility, default 0%) drives image alpha; a new **fill mode** (cover / contain / stretch) maps to `image-fit`.
  - **Nine scrim styles + a strength slider**: auto / dark / light / tinted glass / soft light / frost / spotlight / sheen / random (re-rolled per dialog open). Every scrim now goes through one `Scrim` component (16 overlay sites changed from `background: Theme.modal-scrim` to `background: transparent; Scrim { }`); strength defaults to 32% (the old standard baseline) and rises automatically as dialogs get more translucent (capped at +0.18). The style dropdown is a root-level popover (the settings page lives in a ScrollView) using the project's dropdown row language.
  - **One slider style**: a new `MTermSlider` (accent-filled track + accent thumb + double-click reset) replaces the std-widgets sliders in settings; "Dialog translucency" also becomes a continuous transparency slider (0 solid ≈ 18% ≈ 45% full glass, `popup-alpha = 1 − transparency`), and the scrim follows it.
  - Regressions: `dialog_scrim_drag` 3/3 (the audit now checks "transparent overlay + Scrim component", continuous transparency values, and a drag scan region that avoids the sidebars); 479 lib tests plus `accent_seg_state` / `theme_row_align` / `ansi_light_yellow` are green; offscreen evidence frames verified both themes, the four zones and the scope (checkerboard variance: 69.3 in the title bar vs 0.0 in the sidebar with scope = terminal), and the slider (drag → 0.81, double-click → 0.32).
- **弹框内全部内容表面跟随「弹窗通透度」；会话弹框的下拉选中/悬浮语言对齐主题下拉。** 承接上一条（关闭弹框两块内衬）：用户第二轮反馈「其它弹框的没生效，比如会话的输入框」—— 弹框内的输入框/下拉/列表底此前是**不透明常量**（`field` / `field-2` / 各种 `bg-*`），通透档下就是一块块实心贴片。新增五个 popup 版令牌（`popup-field` / `popup-field-2` / `popup-bg-elevated` / `popup-bg-active` + color 版 `popup-panel` 供渐隐条渐变端点；均基于基础 token 定义，色值单源），把**弹框系文件**（会话 / 同步输入 / 强调色 / 确认 / 设置两个页面 / `seg` / `widgets` 的 dialog 分支 / `app.slint` 的 16 处内联弹框元素）内的全部内容表面（80+ 处）统一乘上 `popup-alpha`；标准档 alpha=1.0 逐像素零回归。瞬时提示层（tooltip / 悬浮信息卡）与主界面、独立窗口继续用基础 token。
  - 用户同步提出「会话弹框的下拉选中状态对齐主题下拉」：协议项（`ProtoItem`）、分组项、`SelectField` 三处选项行统一为主题下拉行的语言 —— 悬浮 = `bg-hover`（accent 浅染，原 `field-2` 灰系）、选中 = 透明底 + **accent 文字** + **accent ✓**（原 accent-soft 底染色 + 主色文字）；协议项图标也随选中变 accent，分组项补上常驻 ✓ 槽位（成员数左移让位）。
  - 回归：`tests/dialog_scrim_drag.rs` 新增 ②d 段（棋盘壁纸判据扫会话弹框「名称」输入框：档 0 先断言扫描区 = field 实色作护栏，档 2 断言透出棋盘）+ 新审计测试 `dialog_content_surfaces_use_popup_tokens`（弹框系文件禁止出现不透明基础 token，白名单逐条注明理由）。
  - 同轮反馈四小修：① 设置页「**界面通透度**」更名「**弹窗通透度**」（原词易与壁纸区的「面板不透明度」= 主界面透明度混淆，本项实际只管弹窗/浮层；英文同步 "Dialog translucency"）；② 去掉该行与预览卡之间的行间分隔线（`SettingRow { show-divider: false }` —— 行下方紧跟的是预览卡而非下一行，这条线是错位的行间线）；③ 设置左菜单选中态不再用底色块（`NavItem`：原 `popup-bg-active` 灰底 → 透明底 + 主题色文字/图标，与下拉选中语言一致）；④ 设置 / 传输管理两个弹框右上角 × 的悬浮图标改纯白（原 tint 恒 `text-primary`，在 `#e81123` 红底上「显暗」—— 注释写着 "white glyph" 而实现未跟上；已与 ProcWindow 的正确写法对齐，并新增锚点防回归）。
  - 追加两处（同日第二轮截图）：⑤ 设置页 `FontCombo`（UI/终端字体、渲染器、规则集、颜色**全部设置下拉共用**）的选项行同样对齐主题下拉行（悬浮 `bg-hover`、选中 = 透明底 + accent 文字 + accent ✓）；经全库排查，dropdown 共 4 处（协议 / 分组 / `SelectField` / `FontCombo`）+ 主题浮层，现已全部统一（右键菜单与调色板浮层经核对不属于下拉范畴）。⑥ 终端「**光标类型**」由三个独立按钮并入 **`SegGroup` 滑动分段**（与「弹窗通透度」同一套滑动语言；块状/竖线/下划线示意图标保留，原 `CursorStyleButton` 移除）。
  - 技术注：`bg-hover` 本身是 8% accent 浅染（自身即透），无需 popup 化。
- **All dialog content surfaces now follow the popup-transparency setting; the session dialog's dropdown selected/hover styling is aligned with the theme picker.** Following the previous entry (the two close-dialog cards): the second report — "the other dialogs didn't take effect, for example the session dialog's input fields" — pointed at the fact that dialog input fields, dropdowns and list surfaces were **opaque constants** (`field` / `field-2` / assorted `bg-*`), i.e. solid tiles inside a translucent dialog. Five new popup tokens (`popup-field`, `popup-field-2`, `popup-bg-elevated`, `popup-bg-active`, plus a colour-form `popup-panel` for gradient endpoints), all defined off the base tokens, were applied to **every content surface (80+ occurrences) inside dialog-family files** (session / sync-input / accent-picker / confirm / the two settings pages / `seg` / `widgets`' dialog branches / the 16 inline dialog elements in `app.slint`), multiplying the popup alpha; at the standard level alpha = 1.0 so the look is pixel-for-pixel unchanged. Transient tooltips and the main window / standalone windows keep the base tokens.
  - The same report also asked for the session dialog's dropdown selected state to match the theme picker: the protocol item (`ProtoItem`), the group item and `SelectField` rows now follow the theme-row language — hover = `bg-hover` (accent tint, was the grey `field-2`), selected = transparent fill + **accent text** + **accent ✓** (was accent-soft fill + primary text); protocol icons tint accent when selected, and the group row gains a resident ✓ slot (member count shifts left).
  - Regressions: new section ②d in `tests/dialog_scrim_drag.rs` (checkerboard-wallpaper probe over the session dialog's "Name" field, with a level-0 guard asserting the scan area is solid `field`) plus a new audit test `dialog_content_surfaces_use_popup_tokens` (dialog-family files may not contain opaque base tokens; whitelist entries each carry a reason).
  - Four small fixes from the same screenshot: (1) the settings row **"Translucency" was renamed "Dialog translucency"** (the old label read as *the* UI translucency, which actually lives in the wallpaper section as "Panel opacity"; this row only drives popups/floating layers); (2) the stray row divider between that row and its preview card was turned off (`SettingRow { show-divider: false }` — the line belongs between two rows, not between a row and the card); (3) the settings nav's selected item no longer paints a fill (`NavItem`: the `popup-bg-active` slab became transparent background + accent text/icon, matching the dropdown selected language); (4) the × on the Settings and Transfers dialogs now turns pure white on hover (its tint was pinned to `text-primary`, which reads as *dimmer* on the `#e81123` red fill — the comments said "white glyph" but the code never did; now aligned with ProcWindow's correct button, with a regression anchor added).
  - Two more from the second screenshot round: (5) the settings `FontCombo` (shared by **every** settings dropdown: UI/terminal font, renderer, rule set, colour) now follows the theme-row language too (hover `bg-hover`; selected = transparent + accent text + accent ✓); a full sweep found four dropdowns in total (protocol / group / `SelectField` / `FontCombo`) plus the theme picker, all now unified (context menus and the colour palette popover were checked and are not dropdowns). (6) The terminal **"Cursor shape"** choice moved from three standalone buttons into a **`SegGroup` sliding segmented control** (the same sliding language as "Dialog translucency"; the block/bar/underline diagram icons are kept, and the old `CursorStyleButton` is removed).
  - Note: `bg-hover` is itself an 8% accent tint (already translucent) and needs no popup variant.
- **弹框内衬随「弹窗通透度」档位变透明（关闭方式两块不再被焊死）。** 用户截图反馈「关于通透性，中间两块怎么没有，只有鼠标悬浮的时候才会有？」。根因：关闭方式弹框的两块卡（最小化到托盘 / 完全退出）常态底用的是 `Theme.bg-root`，而 `frost()` 只在**壁纸激活**时半透明，无壁纸时返回**不透明实色** —— 通透档（82%/55%）下把弹框玻璃「焊死」，只有悬浮（chip 半透明）才透，与用户的对照观察完全一致。
  - 修法：新增令牌 `popup-bg-inset`（= root 色 × 弹窗通透档 `popup-alpha`，与弹框底 `popup-bg` 同透明度来源；标准档 alpha=1.0，观感与此前逐像素一致），替换关闭弹框两块 + 会话弹框的选项 pill（`PillToggle`）、跳板机空态/链可视化、确认弹框详情大块共 5 处「弹框内衬」；输入框类内衬刻意保持实心（可读性优先）。**不用** `popup-bg` 本身 —— 同色 × 半透明叠在同色卡底上会「二次混色反而显形」（`#foot-flat` 的教训），内衬必须保持 root↔panel 的色差。
  - 回归（`tests/dialog_scrim_drag.rs` 新增 ②c 段）：颜色断言在此不可行（root 与 panel 只差 8 级、经 alpha 收敛更小），改用**棋盘壁纸做高频背景** —— 同档位下「无壁纸帧 → 棋盘帧」的变化像素比例：修复前 **0/3808 = 0.0%**（准确报红），修复后透出（内衬 delta 23 级、弹框底 delta 51），外加令牌 alpha 契约断言（档 2 → 140）。481 lib + 集成测试全绿。
- **Dialog insets now follow the popup-transparency setting (the close-dialog option cards are no longer welded shut).** The report, with a screenshot: "about transparency — why do the middle two have none, only on hover?" Root cause: the two close-dialog cards (Minimize to tray / Quit completely) used `Theme.bg-root` for their resting fill, but `frost()` only makes a surface translucent **when a wallpaper is active**; without one it returns an **opaque solid** — so at the translucent popup levels (82% / 55%) the cards sealed the glass off, and only hover (the translucent `chip`) let anything through, exactly matching the observation.
  - Fix: a new token `popup-bg-inset` (= root colour × the popup-transparency level `popup-alpha`, the same transparency source as the dialog background `popup-bg`; at the standard level alpha = 1.0, so the resting look is pixel-for-pixel unchanged) replaces five dialog insets: the two close-dialog cards, the session dialog's option pills (`PillToggle`), the jump-host empty state / chain visualisation, and the confirm dialog's large detail block. Input-style insets deliberately stay opaque (readability first). **Not** `popup-bg` itself — the same colour at partial alpha over the same-coloured card "double-blends and reappears" (the `#foot-flat` lesson); an inset must keep the root↔panel colour difference.
  - Regression (`tests/dialog_scrim_drag.rs`, new section ②c): colour assertions cannot work here (root and panel differ by only 8 levels, less after alpha convergence), so the test uses a **checkerboard wallpaper as a high-frequency background** and measures the changed-pixel ratio between the no-wallpaper and checkerboard frames at the same level: **0/3808 = 0.0%** before the fix (it fails red), and after the fix the checkerboard shows through (inset delta 23 levels, dialog background delta 51), plus a token-alpha contract assertion (level 2 → 140). All 481 lib tests plus the integration tests are green.

- **Nerd Font 图标在终端里放大绘制（不再被压成小点）。** 现象：同一台服务器、同样 13px，MTerm 里 `ls --color` 的图标比 iShell Pro 明显小。**先排除了两个常见猜测**：① 输出高亮 —— 不可能，它只改颜色不碰字号，且 `ls` 输出带 ANSI 色、不满足高亮那套「只作用于 `fg == Default`」的门槛；② 字体缺字形 —— 探针 `examples/probe_nerd_glyphs` 实测你机器上的 `JetBrainsMono Nerd Font`（及其三个变体）**私用区覆盖 56.39%、六个常用图标全部有字形**。
  - 真因是**网格 vs 自由排版的架构差异**。MTerm 是固定字符网格（`cell-w` 由一个 50 个 `M` 的隐藏探针量出，每个字符锁死一格），而 Nerd Font 的图标按 **em 方框**设计 —— 实测 13px 下**格宽只有 8~11px、同行数字高 20px，而图标被压成 4×4px**。iShell 那类终端走 GPU 文本整形，字形可溢出格子按自然尺寸绘制，于是看着大。
  - 整体改自由排版不可行（SSH/PTY 按「每字符 N 列 M 行」传输、`vim`/`htop` 用 `\e[row;colH` 定位光标、点选要靠 `col/row` 反算像素、列数变化要 reflow），所以采用**等效方案**：网格、协议寻址、点选**全都不动**，只把**纯私用区 span** 在自己那一格内放大 1.75 倍并纵向居中。判据 `is_nerd_icon_span` 与既有的 `contains_cjk` 同款范式（`TermSpan` 多一个 `nerd` 标志）；只认「整段都是图标字符（可含空白）」，混排文本（`📁backup.sql`）不放大 —— 逐字符定位需要另一套渲染路径。
  - 放大**不改字体度量**（`cell-w/h` 仍来自 `cell-probe`），所以 PTY 列数行数不会因此重排。
  - 顺带补一个诊断：`MONO_PROBE`（`A g 0 ─ │`）只验 ASCII + 制表符，**对图标区完全不敏感** —— 探针实测 `Monotype Corsiva` PUA 覆盖仅 0.28% 却能通过它。新增 `NERD_ICON_PROBE`（6 个常用图标码位）与 `report_nerd_icon_support()`：**不改选择逻辑**（多数机器没装 Nerd Font，据此拒绝会把字体链清空），只在选中字体没有图标字形时写一条 WARN 到日志，让「图标变空白」这个症状至少有说法。诊断细节跑 `cargo run --release --example probe_nerd_glyphs`。
- **Nerd Font icons are drawn larger in the terminal instead of being squashed.** The report: on the same server at the same 13 px, MTerm's `ls --color` icons looked markedly smaller than iShell Pro's. **Two obvious suspects were ruled out first**: (1) output highlighting — impossible, it only recolours and never touches the font size, and `ls` output carries ANSI colours so it never even satisfies that feature's "only `fg == Default` runs" gate; (2) missing glyphs — the probe `examples/probe_nerd_glyphs` measured the installed `JetBrainsMono Nerd Font` (and its three variants) at **56.39 % private-use-area coverage, all six common icons present**.
  - The real cause is **grid versus free-form layout**. MTerm draws on a fixed character grid (`cell-w` measured from a hidden 50×`M` probe, every character locked to one cell), while Nerd Font draws its icons on the **full em box** — measured at 13 px, the cell advance is only **8–11 px** and digits on the same line stand **20 px** tall, while icons come out **4×4 px**. Free-form-layout terminals shape text on the GPU and let glyphs overflow the cell, which is why they look bigger there.
  - Going fully free-form is not available: SSH/PTY transmits "N columns × M rows per character", `vim`/`htop` address the cursor with `\e[row;colH`, click-to-select inverts (row, col) back to pixels, and a column-count change reflows the scrollback. So the equivalent was applied instead: the **grid, PTY addressing and hit-testing are untouched**, and only a **pure private-use-area span** is drawn 1.75× larger and vertically centred inside its own cell. `is_nerd_icon_span` follows the existing `contains_cjk` pattern (one more `nerd` flag on `TermSpan`); it accepts only spans made entirely of icon characters (whitespace allowed), so mixed text like `📁backup.sql` is left alone — per-character positioning would need a separate rendering path.
  - Scaling **does not change font metrics** (`cell-w/h` still come from `cell-probe`), so PTY rows/columns never reflow because of it.
  - One diagnostic came out of this too: `MONO_PROBE` (`A g 0 ─ │`) checks only ASCII plus box-drawing and is **entirely blind to the icon ranges** — the probe measured `Monotype Corsiva` at 0.28 % PUA coverage while still passing it. Added `NERD_ICON_PROBE` (six common icon code points) and `report_nerd_icon_support()`: the **selection logic is unchanged** (most machines have no Nerd Font, and rejecting on that would empty the chain and leave things worse), but when the chosen family lacks icon glyphs a WARN lands in the log so "the icons go blank" has an explanation. For the full per-family coverage table run `cargo run --release --example probe_nerd_glyphs`.
- **亮色主题下 256 色调色板不再大面积不可读（对比度守卫取代拍脑袋的亮度阈值）。** 前两条修的是主题自己的 ANSI 16 色表，但用户第三次反馈「终端上面的显示依旧看不清楚」—— 截图取证（背景 `#FAF8F4` → MTerm·亮）后定位到**另一条根本不同的路径**：那批发青的字（`#00AF87`，占屏墨色第一位、只有 2646 px 却是最多的色）**不在任何主题的 `ansi16` 表里**，它是 **256 色调色板的固定值**（`55 + v*40`，恰好等于立方体索引 36），也就是远端程序发 `\e[38;5;36m` 之类色码时用的标准立方体 —— **不随主题变化**，所以前面修色表对它完全无效。
  - 旧守卫 `darken_light_fg` 的判断依据是 **HSL 亮度**：`L < 0.55` 就原样放行。可 HSL 的 L 是**通道均值**，高饱和的青/绿/黄天然偏低 —— 实测 `#00AF87` 的 L 只有 0.34，于是直接漏网。遍历全部 216 个立方体色：**158 个在 `#FAF8F4` 上低于 4.5:1，其中 70 个是守卫根本没动的**；最刺眼的是纯亮黄 `#FFFF00` 只有 **1.01:1**（等于背景）、纯绿 `#00FF00` 只有 1.29。
  - 现在改为**按真实对比度收敛**：先算 WCAG 对比度，不足则**保持色相**、从原亮度细步（1/1024）下降直到对**当前主题的 `tbg`** 达到 4.5:1。背景从「假设的近白」换成实际的 `tbg`（逐主题），所以 `kanagawa-lotus` 那种偏黄的浅底也一并正确。守卫同时接到了此前完全没设防的**立方体与灰阶路径**（`idx_to_rgb` 的 16~255 档原先原样透传）。暗色模式**逐值不变**。
  - 效果：216 个立方体色从 158 个不达标降到 **0**；截图里那批青绿 `#00AF87` 由 2.65:1 提到 4.52:1。另有一条回归锁住「**只许调亮度、不许抹色相**」—— 首版用 512 步粗扫时，饱和色（`#4DC4A8`/`#ABE0D1`）被直接推成纯黑，浅底上可读了但彩色信息全丢了。
  - 5 条新回归（`presentation.rs` 内联测试，因 `vt_span_colors` 是 `pub(crate)` 集成测试访问不到）：亮色 14 套 × 240 个索引全部 ≥4.5:1、24 位真彩同守、**暗色 14 套立方体与灰阶逐值不变**、已达标色原样透传、饱和色色相保留。
- **The 256-colour cube is readable in light themes again (a real contrast guard replaces a guessed lightness threshold).** The previous two entries fixed the themes' own ANSI-16 tables, but the third report — "the terminal display is still hard to read" — came from a **completely different path**. Screenshot forensics (background `#FAF8F4` → MTerm light) identified the greenish text as `#00AF87`: the single most common ink colour (2646 px) yet **absent from every theme's `ansi16`**. It is a **fixed value of the 256-colour cube** (`55 + v*40`, exactly cube index 36) — what remote programs emit via `\e[38;5;36m`-style codes. **It does not follow the theme**, which is why fixing the palettes had no effect on it.
  - The old guard branched on **HSL lightness**: `L < 0.55` passed through untouched. But HSL lightness is a channel average, so saturated cyans/greens/yellows measure intrinsically low — `#00AF87` measures L = 0.34 and slipped straight through. Across all 216 cube colours, **158 render below 4.5:1 on `#FAF8F4`, and 70 of those were never touched by the guard**; the worst were pure bright yellow `#FFFF00` at **1.01:1** (indistinguishable from the background) and `#00FF00` at 1.29.
  - The guard now **converges on real contrast**: compute the WCAG ratio, and if it falls short, **hold the hue** and step lightness down finely (1/1024) until the colour reaches 4.5:1 against the **palette's actual `tbg`** rather than an assumed near-white. Per-theme backgrounds mean a tinted light palette such as `kanagawa-lotus`'s `#F2ECBC` is now handled too. The guard was also extended to the **cube and grayscale arms of `idx_to_rgb`** (indices 16–255), which previously had no protection at all. Dark mode is **value-for-value unchanged**.
  - Result: cube colours below the line drop from 158 to **0**; the screenshot's `#00AF87` rises from 2.65:1 to 4.52:1. One regression locks in "**lightness only, hue never erased**" — the first cut used a coarse 512-step sweep that pushed saturated colours (`#4DC4A8`, `#ABE0D1`) all the way to black: readable, but every bit of colour information lost.
  - Five new regressions (inline in `presentation.rs`, since `vt_span_colors` is `pub(crate)` and invisible to integration tests): all 240 indices × 14 light palettes clear 4.5:1, 24-bit true colour is guarded too, **dark palettes keep cube and grayscale values verbatim**, already-readable colours pass through untouched, and saturated colours keep their hue.
- **亮色主题的「白色」也不可读了 —— 撤销白/灰/亮白三槽的对比度豁免。** 上一条只修了 12 个彩色槽（yellow 那两槽），用户复测仍报「白色和黄色在亮色主题还是有些看不清楚」。截图取证：背景 `#F2ECBC` → **Kanagawa·亮**。放大后看清「黄色」其实混了两处 —— `K`/`B` 单位是槽 3 yellow（**4.62:1，已达标**），数字部分 `4.0` 是**槽 7 white**（**2.93:1**）；而整片列表的**日期列是槽 8 brightBlack**（**2.23:1**）。根因在验收标准本身：`_ansi-group.json` 的 `_meta.ansi_check` 写着「12 彩色槽 ≥4.5:1，**0/7/8/15 按终端惯例只记录不判失败**」—— 那是暗底惯例（白/浅黑在深底上本就该弱），落到浅色 tbg 上直接塌：亮色 14 套里 **13 套**槽7/8 有一项不达标，极端例 **flexoki·亮槽7 = 1.00（等于背景，肉眼完全不可见）**，另有 8 套**槽 15 brightWhite**同样塌（最低 solarized 1.14 —— 加粗白字也读不出）。按用户拍板，**三槽撤销豁免、同样按 ≥4.5:1 判**，亮色 14 套全部压到达标线（4.70~~4.77）；暗色 14 套一行未动。改色脚本 `scripts/_audit/fix_light_grey_slots.py` 里有个必须记的坑：**不能用 HSL 的饱和度判断「这是不是灰」** —— 近白色在 HSL 里 S 会爆表（`#FFFCF0` 算出 S=100%、H=48°），「保色相降 L」会把白色档一路带成**暗黄 `#8A6E00`**（白色档变黄色档，flexoki / gruvbox 都中招）；改用**通道极差** max(r,g,b)−min(r,g,b) 三档判：≤48 锁 sat=0、48~~80 压到 0.12 封顶、>80 才算彩色档。另一处是**自家门面主题有额外纪律**：`mterm-light` 的槽 8 必须与 t3 同色相 ≤10°（`theme/mod.rs` 的灰族同源断言），不能压成中性灰 —— 脚本对该套走「朝 t3 色相对齐后降 L」，得 `#766E64`（色相差 2.0°、对比 4.73）。回归加到 `tests/ansi_light_yellow.rs`（共 7 条）：白/灰/亮白三槽达标、**三槽压深后仍须是中性灰**（结果侧兜住色相污染）、**暗色 14 套三槽与修正前逐值一致**。
- **Light themes' white was unreadable too — the contrast exemption for the white/grey/bright-white slots is withdrawn.** The previous entry fixed only the 12 colour slots (the two yellow ones); re-testing still reported "white and yellow are still hard to read in light themes". From the screenshot: the background `#F2ECBC` identifies **Kanagawa light**. Zooming in shows "yellow" actually mixed two things — the `K`/`B` units are slot 3 yellow (**4.62:1, already passing**), while the digits are **slot 7 white** (**2.93:1**); and the whole **date column is slot 8 brightBlack** (**2.23:1**). The root cause is the acceptance line itself: `_ansi-group.json`'s `_meta.ansi_check` said "12 colour slots ≥ 4.5:1, **0/7/8/15 recorded but not judged by convention**" — a dark-background convention (white and bright-black *should* be weak on a dark background) that collapses on a light tbg: **13 of the 14** light palettes have a slot 7/8 below the line, with **flexoki light's slot 7 at 1.00** (identical to the background, entirely invisible to the eye), and **8 palettes' slot 15 brightWhite** collapse too (solarized at 1.14 — even bold white text is unreadable). At the user's call, all three slots are withdrawn from exemption and judged at ≥4.5:1; all 14 light palettes are brought to the line (4.70–4.77) and **not one line of the 14 dark palettes changed**. One trap worth recording in the generator (`scripts/_audit/fix_light_grey_slots.py`): **you cannot use HSL saturation to tell whether a colour is grey** — near-whites blow up S (near-white `#FFFCF0` computes as S=100%, H=48°), so "hold hue, drop lightness" drags the white slot all the way to **mustard `#8A6E00`** (the white slot becomes a yellow one — flexoki and gruvbox both hit it). Use **channel spread** max(r,g,b)−min(r,g,b) in three bands instead: ≤48 locks sat to 0, 48–80 caps sat at 0.12, >80 counts as a real colour. A second trap: the **in-house facade theme carries an extra rule** — `mterm-light`'s slot 8 must stay within 10° of t3 on the hue circle (the grey-family assertion in `theme/mod.rs`), so it cannot simply become neutral grey; that palette takes a "rotate toward t3's hue, then drop lightness" path, landing on `#766E64` (2.0° apart, 4.73:1). The regressions went into `tests/ansi_light_yellow.rs` (7 tests total): the three slots meet the line, **the three slots stay neutral grey after darkening** (guards hue contamination from the result side), and **all 14 dark palettes keep their exact pre-fix values** across the three slots.
- **亮色主题的终端黄字不再看不见（9 套 ANSI 黄色重制）。** 用户反馈「亮色主题，终端里面的黄色字体看不见」。根因：那9 套的 yellow（ANSI 槽 3）/ brightYellow（槽 11）照搬上游原色，可那些黄本来是配深色底的 —— 落到浅色 tbg 上对比度只有 **2.05~4.50:1**（ayu-light 的亮黄最低 2.05，肉眼几乎读不出），而 `_ansi-group.json` 的 `_meta.ansi_check` 验收线本就是「12 彩色槽 ≥ 4.5:1」。现按色相环保 H/S 降 L 压到达标线（生成脚本 `scripts/_audit/fix_light_yellow.py`，真源 `scripts/_ansi-group.json`，`node scripts/gen_theme_rust.js` 重生成 `palettes.rs`），**暗色档一律不动**（深底上的黄本来就够读，压暗反而看不清）。槽 11 一律压到比槽 3 更深（≥5.2:1），沿用 mterm-light 早就有的「亮底 bright=更深同色相」纪律。明细：nord 4.50→4.63 / catppuccin 2.79→4.61 / rosepine 2.58→4.64 / everforest 3.01→4.61 / gruvbox 3.49→4.66 / ayu 2.44→4.63（亮黄 2.05→5.21）/ flexoki 3.24→4.61 / solarized 3.07→4.63 / darcula 3.20→4.63。新增 `tests/ansi_light_yellow.rs` 四条回归：亮色两槽达标、修过的 9 套 bright 严格深于 normal、**暗色 14 套黄色两槽与修正前逐值一致**（防止以后顺手压暗）、三套被点名过的锁死具体色值。
- **Yellow ANSI text is readable again in light themes (9 palettes reworked).** Reported as "in light themes the yellow terminal text is invisible". Root cause: those 9 palettes' yellow (ANSI slot 3) and brightYellow (slot 11) were taken straight from upstream, but those yellows were authored for dark backgrounds — on a light tbg they landed at only **2.05–4.50:1** (ayu-light's bright yellow is 2.05, effectively unreadable), while the acceptance line in `_ansi-group.json`'s `_meta.ansi_check` has always been "12 colour slots ≥ 4.5:1". Hue is now held while lightness drops until the line is met (generator `scripts/_audit/fix_light_yellow.py`, source of truth `scripts/_ansi-group.json`, regenerated into `palettes.rs` by `node scripts/gen_theme_rust.js`); **dark palettes are untouched** — their yellows already read fine on a dark background and darkening would only hurt. Slot 11 always drops deeper than slot 3 (≥5.2:1), following mterm-light's long-standing "on a light background, bright is a deeper same-hue" rule. Details: nord 4.50→4.63 / catppuccin 2.79→4.61 / rosepine 2.58→4.64 / everforest 3.01→4.61 / gruvbox 3.49→4.66 / ayu 2.44→4.63 (bright 2.05→5.21) / flexoki 3.24→4.61 / solarized 3.07→4.63 / darcula 3.20→4.63. Four new regressions in `tests/ansi_light_yellow.rs`: both light slots meet the line, the reworked 9 have bright strictly deeper than normal, **all 14 dark palettes keep their exact pre-fix yellows**, and the three named palettes are pinned to concrete values.
- **强调色预设网格：两块换成主流色。** 用户红框圈出「白光色 #FAFAFA」与「MTerm 青 #1FC8B4」，要求换成其它主流、与现有色块不冲突且美观的颜色。按色相环量化后替换为 **Stripe 紫 `#635BFF`** 与 **GitHub 绿 `#2DA44E`**：白光色是网格里唯一 sat=0 的块（深色弹框里看不出色相，纯粹占位），MTerm 青与相邻青绿 / 薄荷的 ΔE 只有 8~16、挤成一团；而色相环上最大的空档恰是 225°(靛蓝)→259°(仿紫) 断掉的 34°，Stripe 紫(243°)正落其中，GitHub 绿(137°)落在 126°(绝命毒药)→149°(微信绿) 之间。新块对网格其余各块的 minΔE(CIE76) = 31.1 / 13.1（网格中位数 18.9），深底 / 浅底对比度均过 3:1。两块新色**按色相插到对应分区**而非留在原位，网格顺序仍是「色相环顺排」。顺带修掉一处既存偏差：`ACCENT_PRESETS`（Rust，单一出处）此前比 `ui/app.slint` 的网格字面量少一项「微信绿」，两处不同源；现两处都是 48 项、逐项同源。两块的旧 key（`white-glow` / `mterm-cyan`）解析不到即回落 `Follow`，老配置不会指向已下线的色。
- **Two accent presets swapped for mainstream colours.** The report circled "White glow #FAFAFA" and "MTerm cyan #1FC8B4" and asked for mainstream replacements that don't clash with the existing swatches. After measuring the hue wheel they become **Stripe violet `#635BFF`** and **GitHub green `#2DA44E`**: white glow was the only zero-saturation swatch (no visible hue in the dark popup, purely a placeholder), and MTerm cyan sat ΔE 8–16 from its neighbours, clumping with them; meanwhile the wheel's largest gap is exactly the 34° break between 225° (indigo) and 259° (ultraviolet), where Stripe violet (243°) lands, with GitHub green (137°) between 126° (toxic green) and 149° (WeChat green). Their minΔE (CIE76) against the rest of the grid is 31.1 / 13.1 (grid median 18.9), and both clear 3:1 on dark and light backgrounds. The two new swatches are **placed in their hue partitions** rather than left in the old slots, so the grid still reads as one sweep of the wheel. This also fixes a pre-existing divergence: `ACCENT_PRESETS` (Rust, the single source of truth) was missing one entry ("WeChat green") relative to the literal list in `ui/app.slint`; both now hold the same 48 entries. The retired keys (`white-glow` / `mterm-cyan`) resolve to `Follow`, so old configs never point at a colour that's gone.
- **设置页主题按钮的色点改圆形；主题下拉搜索框改成真正的输入框。** ① 设置 › 主题 的下拉按钮里那颗强调色圆点由圆角方块（14px / 圆角 4）改为正圆（14px / 圆角 7），与主题下拉列表行内的色点（11px / 圆角 5.5）同一套形状语言 —— 用户反馈「这里面有两种样式的色块，统一用圆形的」。② 下拉浮层的搜索行由**扁平行**改为**输入框形态**：常驻 `field` 底 + `border-control` 1px 边，聚焦时边转 `accent` + 外扩 3px `accent-soft` 柔光环（同心 8+3=11），与 `PickerInput`（调色盘）/ `MiniInput`（会话弹框）完全同款；行底那条 1px 细线一并删除（输入框自带四边，底线等于双重边界，聚焦时还与边框同步变色、看着像框在抖）。用户反馈是「聚焦后样式都不一致，不怎么协调」：旧行自身 transparent、无底无边，聚焦那一帧才凭空冒出一圈柔光环，外径（行高 30 + 6 = 36）比行内控件都大，整行像被套了个更大的框。清空钮与「N 套」计数位置不变。新增 `tests/theme_row_align.rs::theme_search_row_is_an_input_not_a_flat_bar` 从像素侧钉住形态（未聚焦有 field 底、聚焦有环、环在框外 3px）。
- **The settings theme button's dot is now round, and the theme dropdown's search row is a real input.** ① The accent dot inside the Settings › Theme dropdown button changes from a rounded square (14 px / radius 4) to a circle (14 px / radius 7), matching the dot in the dropdown's own rows (11 px / radius 5.5) — reported as "there are two styles of swatch here, make them all round". ② The dropdown's search row changes from a **flat bar** to an **input**: a standing `field` background with a `border-control` 1 px border, turning `accent` on focus with a 3 px `accent-soft` ring expanding outward (concentric, 8 + 3 = 11) — identical to `PickerInput` (colour picker) and `MiniInput` (session dialog). The 1 px hairline along the row's bottom edge is removed too (an input already has four sides, so the hairline was a double boundary, and it changed colour together with the border, making the frame look like it was shaking). The report was "after focusing, the styles don't match and it doesn't look coordinated": the old row was transparent with no fill or border, so focusing made a ring appear out of nowhere whose outer size (row height 30 + 6 = 36) dwarfed the row's own controls, as if a bigger box had been slipped over it. The clear button and the "N themes" counter keep their positions. `tests/theme_row_align.rs::theme_search_row_is_an_input_not_a_flat_bar` pins the shape from the pixel side (field fill when idle, ring outside the frame on focus, 3 px out).
- **分段控件的内距统一 + 明度条去描边、渐变铺满。** ① `SegGroup`（RGB|HSV|HSL、设置页强调色分段等共用）：`gap` 由 2px 改为与 `pad` 同值，且 `pad` 过一遍 `snap-to-device` —— 原先滑块的「到外框」与「到相邻格」差 1px；且 3px 在 125% / 150% 缩放下是 3.75 / 4.5 物理像素，滑块 `y` 与 `height`（= h − 2·pad）取整方向相反，上下留白一个舍一个入（实测上 2px / 下 3px）。现在三条内距恒等。② 调色盘的明度条去掉 1px 描边（它在底端黑色处勾出一道半圆弧细线，孤立看像"一个圆形细线"；旁边的色轮本来就没有描边），并让渐变位图铺满整盒、圆角交给 `clip` 裁 —— 原先写死 24×238 贴 1px 内衬，分数缩放下底边会漏出一行面板色。
- **Uniform segment insets, and a borderless edge-to-edge brightness strip.** ① `SegGroup` (shared by the RGB|HSV|HSL tabs, the settings accent segment, …): `gap` now equals `pad`, and `pad` runs through `snap-to-device` — the thumb's "to the frame" and "to the next cell" insets used to differ by 1 px, and at 125 % / 150 % scaling 3 px is 3.75 / 4.5 device pixels, so the thumb's `y` and `height` rounded in opposite directions and its top and bottom gaps disagreed (measured 2 px vs 3 px). All three insets are now identical. ② The palette's brightness strip drops its 1 px outline (it drew a half-circle hairline across the black end — reading as a stray "circular line" — while the neighbouring colour wheel has no outline at all) and its gradient bitmap now fills the whole box with `clip` cutting the rounded ends; previously a hard-coded 24×238 inset by 1 px leaked a row of panel colour along the bottom at fractional scales.
- **取色信息卡（及调色盘的亮度气泡）跟随主题；设置页强调色色卡的悬浮提示换成同款信息卡。** ① `ColorInfoCard`（HEX / RGB / HSV / HSL 四行那张卡，强调色弹框 / 「最近」行 / 设置色卡共用）与调色盘亮度条的「亮度 N」气泡，底色原先**都写死暗色 `#111318`**，浅色主题下仍是深底 —— 用户反馈「没有跟随主题」。两者改用统一浮层 token `bg-elevated`：跟随主题，且比弹框自身的 `popup-bg`（= 面板底色）高一档，靠边框 + 投影勾出卡片轮廓。② 设置 › 主题页强调色色卡的悬浮提示，由一句「强调色 #hex」的气泡换成**同一张 ColorInfoCard**（用户要求与自定义弹框同款）。卡片**声明在根层最后**（与 theme-pick / accent-pop 同款桥：面板只报悬停态与色卡的窗口坐标，事件期取 absolute-position），放色卡**下方**并右对齐 —— 该行靠页顶，向上会越界；而画在行内又会被紧随其后的「界面通透度」整块盖住（Slint 绘制顺序 = 声明顺序，行内元素压不过后面的兄弟 —— 用户截图「被覆盖了无法看见」）。
- **The colour info card (and the palette's brightness bubble) now follow the theme, and the settings accent swatch shows the same card.** ① `ColorInfoCard` (the HEX / RGB / HSV / HSL card shared by the accent popup, the "recent" row and the settings swatch) and the brightness strip's "Bright N" bubble both had a hard-coded dark background (`#111318`) that stayed dark under a light theme — reported as "it doesn't follow the theme". Both now use the unified overlay token `bg-elevated`: theme-aware, and one step above the dialog's own `popup-bg` (= the panel colour) so the border and the shadow still draw the card's outline. ② The Settings › Theme accent swatch's hover tip swaps its one-line "Accent #hex" bubble for the **same ColorInfoCard** (asked to match). The card is **declared last at the root layer** (the same bridge as theme-pick / accent-pop: the panel only reports the hover state and the swatch's window coordinates, read from `absolute-position` inside the event) and sits below the swatch, right-aligned — above it would overflow the page, while drawing it inside the row let the following "interface transparency" block cover it (Slint paints in declaration order, so an in-row element can never out-paint a later sibling — reported as "it's covered and invisible").
- **「跟随系统」整个下线（界面两处入口 + 运行时行为）。** ① UI：设置 › 主题 页那一行整行删除；主题下拉框顶部只留「反向（亮↔暗）」，占满原来两键的行宽。② 行为：它原本**不是独立轴**，而是「主题为空」的别名 —— 挑任何一套主题都会把它写掉，两者抢同一个槽位，所以开着它再选主题会被静默顶掉（用户反馈「和主题的交互性并不怎么好」；在浅色系统的机器上表现为「不管什么主题都变成 MTerm·亮」）。现在默认值改为 `follow_system=false`、默认主题落成 `mterm-dark`，并加 **rev5 一次性迁移**：停留在跟随态的老配置落成具体主题（theme 为空 → mterm-dark）且跟随关闭。此后主题只有一个轴，不会再出现「没动主题、界面却随系统变亮」。配置字段与 Rust 通路保留（已无界面入口）。
- **"Follow system" is retired entirely (both UI entries and the runtime behaviour).** ① UI: the row is removed from Settings › Theme, and the theme dropdown's top row keeps only "Invert light/dark", filling the row that used to hold two buttons. ② Behaviour: it was never an independent axis but an alias for "the theme is empty" — picking any theme wrote it off, so the two fought over a single slot and enabling it then choosing a theme silently dropped it (reported as "it doesn't interact well with the theme"; on a light-OS machine it showed up as "whatever theme I pick, it turns into MTerm light"). The default is now `follow_system=false` with `mterm-dark` as the default theme, plus a **one-time rev 5 migration**: configs left in the following state resolve to a concrete theme (empty → mterm-dark) with following switched off. The theme now has exactly one axis and can no longer go light just because the OS did. The config field and its Rust path remain, with no UI entry.
- **命令栏三个控件统一到前景档。** 「快捷」chip（文字 + ∧ 图标）与「所有会话」原用次级档 `text-secondary`，而夹在中间的「∧」历史开关用前景档，三者亮度不齐 —— 用户反馈「命令栏中的字体色号也不协调」。现三者统一取 `text-primary`；「所有会话」的激活态仍是白字 on 强调色，「$」提示符（强调色）与占位符（弱化档）按既有设计不变。
- **The command bar's three controls now share the foreground tier.** The "Quick" chip (label + ∧ glyph) and "All sessions" used the secondary tier `text-secondary` while the history toggle between them used the foreground tier, leaving the three uneven — reported as "the command bar's font tiers don't match either". All three now take `text-primary`; "All sessions" keeps its white-on-accent active state, and the "$" prompt (accent) and the placeholder (faint tier) stay as designed.
- **窗口三键（最小化 / 最大化 / 关闭）的图标统一到前景档。** 三个键原本同用一个次级档 `text-secondary`，但压在深色标题栏上偏暗；又因字形墨量不同（✕ 的两条对角线跨度最大，− 只有一条 14 单位短线），肉眼看着并不齐 —— 用户反馈「只有关闭的是正确的，其两个有些暗了，不统一」。现统一取 `text-primary`（与左侧正文同档）。关闭键悬停仍是红底白字（`#e81123` + 白图标），未变。
- **The window controls (minimize / maximize / close) now use the foreground tier.** All three already shared the secondary tier `text-secondary`, but that reads dim on the dark title bar, and their differing ink coverage (the ✕'s two diagonals span the most, the − is a single 14-unit dash) made them look uneven — reported as "only close looks right, the other two are dim". They now take `text-primary`, the same tier as the body text on the left. Close keeps its white-on-red hover (`#e81123`).
- **调色盘 H 滑杆的彩虹条改为位图。** HSV / HSL 的 H 分量滑杆原本用 Slint 的**七停** `@linear-gradient` 画彩虹（其余分量是两停），实测出现可见断层。现改由 Rust 逐像素烘焙色相条位图（`bake_hue_image`，与色轮 / 明度条同一范式 —— Slint 侧只负责按尺寸贴图，内套 clip + 圆角保证两端圆头），H 条再无断层。位图与选色无关，开机烘一次即可。
- **The palette's H slider now uses a baked hue ramp.** The HSV / HSL hue slider drew its rainbow with Slint's **seven-stop** `@linear-gradient` (the other components use two-stop ones) and showed visible breaks. It now blits a per-pixel baked hue image from Rust (`bake_hue_image`, the same scheme as the wheel and the brightness strip — Slint only draws it at size, with an inner clip + radius for the rounded ends), so the ramp carries no seams. The image is colour-independent and baked once at startup.
- **调色盘三项调整。** ① HEX 色号行移到 B 之下（行序改为「RGB|HSV|HSL 分段 → R/G/B → HEX」），并给出与 R/G/B **同列**的左侧「HEX」标签（标签列统一 30px，各行标签与值各成一条竖线）。② RGB/HSV/HSL 分段的**悬停底色改为跟随当前选色**（原是中性灰 `bg-hover`，与这一屏「所有选中 / 当前语义都由选色表达」不一致，确认键同源）——淡染 16%（暗）/ 10%（亮），比激活滑层浅一档，免得看着像已选中。③ 对比度警示句末的预览色块改为**紧贴句尾「自动调亮为」**：补一块尾部吸空块（与强调色弹框 `#accent-pop-flow` 同一修法 —— 无 stretch 子项时 Slint 把余宽均摊给「可生长」的 Text，色块于是被顶到行尾）。
- **Three colour-picker adjustments.** ① The HEX row moves below B (order: format tabs → R/G/B → HEX) and gains a left-hand "HEX" label in the same column as R/G/B (the label column is unified at 30 px, so every row's label and value line up). ② The format tabs' hover fill now follows the current colour (it used to be the neutral bg-hover, out of step with a screen where every "selected / current" cue comes from the colour, and with the confirm button) — a 16 % (dark) / 10 % (light) tint, one step lighter than the active thumb so hovering never reads as selected. ③ The preview swatch at the end of the contrast warning now hugs the sentence: a trailing flexible spacer (the same fix as the accent popup's `#accent-pop-flow` — without one, Slint spreads the leftover width over the growable Texts and shoves the swatch to the row's end).
- **主题弹框的搜索框：补了聚焦态，并加了清空按钮。** ① 聚焦时底部细线转为强调色（120ms 过渡），外圈套一道强调色柔光环（与取色盘输入框 / MiniInput 同一套聚焦语言；浮层 6px 内边距足以容纳 3px 外扩，不会被卡片裁剪）。② 有输入时，「28 套」计数左侧出现一个 ✕ 清空钮（close 图标，悬停提亮一档），点击即清空并把列表恢复全量，焦点留在输入框、浮层不关。输入框右侧让位相应加宽，避免输入文字压到清空钮。
- **The theme picker's search field: focus state and a clear button.** ① On focus the bottom hairline turns accent (120 ms transition) and a soft accent ring wraps the row — the same focus language as the colour picker's fields and MiniInput (the popup's 6 px padding absorbs the 3 px ring, so the card's clip never cuts it). ② While there is text, a ✕ clear button (close glyph, brightening on hover) appears left of the "N themes" counter; clicking it clears the field and restores the full list, keeping focus in the field and the popup open. The field's right-hand reservation widens so typed text never runs under the button.
- **设置 › 主题页的「强调色」色卡加了悬停提示框。** 色卡（46×32 的方框，仅展示当前生效色）此前悬停无任何反馈。现在悬停即在其正上方弹出提示，显示当前**生效**色的 hex（跟随主题态 = 主题强调色；自定义态 = 3:1 对比度守卫校正后的色，即色卡实际画出的那颗）。气泡沿用顶栏统一浮层规格（bg-elevated / radius-sm / 22px 高 / 文字居中 / 阴影 8px，与存储页 PathAction 的气泡同源），画在色卡上方并右对齐 —— 避开设置页滚动视口的裁剪边；色卡原有的「点击等同卡片空白」行为保持不变。
- **The settings › Theme page's accent swatch now has a hover tip.** The swatch (the 46×32 well that only displays the effective accent) gave no feedback on hover. Hovering it now pops a tip directly above, showing the hex of the **effective** accent (under Follow theme it is the theme's accent; under Custom it is the 3:1-guard-corrected colour — the very colour the well paints). The bubble follows the top-bar overlay spec (bg-elevated / radius-sm / 22px tall / centred text / 8px shadow, the same one the storage page's PathAction bubble uses) and is drawn above the swatch, right-aligned, so the settings page's scroll viewport never clips it; the swatch's existing "click behaves like card blank space" semantics are unchanged.
- **主题下拉框去掉每行的「三色块」。** 用户反馈「好像没啥意义」：色阶条（root / panel / tbg 三色，26×15，含棋盘衬底与内描边）缩到行内已基本不可辨，主题观感由右侧八色带承担，故整块删除。行内现为「色点 + 名称 …… 八色带 / ✓ / 星标」。`ThemeEntry` 的 root / panel / tbg 字段保留（Rust 侧仍填，与早已无人读的 tfg 同理）。
- **The theme dropdown drops the per-row three-tone block.** Reported as meaningless: the root/panel/tbg swatch (26×15, checkerboard backing and inner stroke included) had shrunk past recognition in-row, and the ANSI band on the right already carries the theme's look — the block is gone. Rows now read "dot + name … ANSI band / ✓ / star". `ThemeEntry`'s root/panel/tbg fields stay populated from Rust (the same state as the long-unused tfg).
- **主题选择弹框四项整理。** ① 删「全部主题 …… N 套」总组头——它与紧随的暗/亮子组头计数重复，常驻区后直接分组（`build_theme_entries` 少 push 一条）。② 行内右侧组（八色带 / ✓ / 星标 chip）统一靠右：吸空块上移到名称之后，原跟随名称漂移的色阶条并入右组（下一轮已整块删除）。③ 「跟随系统 / 反向（亮↔暗）」两按钮补悬停动效（激活态强调色加深、常态换 bg-hover、边框提亮，120ms 过渡）。④ 星标 chip 的阴影区缩小为包裹「星 + 文案」并整体居中（此前铺满 84px、文案右对齐，左侧留一截空阴影）；悬停显形 / 进入提亮两档不变。⑤ 删右下角「常用写进 config」。
- **Four theme-picker tweaks.** ① The "All themes … N" section header is gone — it duplicated the dark/light sub-headers' counts, so the favourites block now runs straight into the grouped list (`build_theme_entries` pushes one fewer row). ② The row's right-hand group (ANSI band / ✓ / star chip) is uniformly right-aligned: the flex spacer moved to just after the name, pulling the 3-tone bar — which used to trail the name — into that group (removed outright in the next round). ③ "Follow system" and "Invert light/dark" gained hover feedback (the active state deepens the accent, idle switches to bg-hover, the border brightens, 120 ms transition). ④ The star chip's backdrop shrinks to wrap "star + label" and is centred as a whole (it used to fill the full 84 px with right-aligned text, leaving an empty shadow on the left); the show-on-hover / brighten-on-chip-hover two-step is unchanged. ⑤ The bottom-right "Favorites saved in config" note is removed.
- **调色盘第三轮细节。** ① HEX 行：去掉框内 "HEX" 标签（# 号已是常驻标识），# 缺席时自动补回（输入框永不出现无 # 的 hex）；「点入即全选」按反馈移除——第一次点击只落光标。② 复制键图标改**绝对居中坐标**（此前内层 HL 的 `alignment: center` 只管主轴，交叉轴 stretch 下固定高图标仍顶对齐——"只有悬浮背景居中了"）。③ 页脚警示行去掉 `wrap`：wrap 文本的 preferred-width 等于整行可用宽，色块被挤到行尾——现色块紧贴"自动调亮为"句尾。④ **色轮圆外四角改为可拖卡片**：`wheel-ta` 的 240×240 整框命中区把四角点击吃了（用户四角标记"无法拖动"），圆外按下 / 移动 = 卡片拖动。⑤ 卡片上留白 8 → 14px，与下留白一致。
- **Color-picker polish, round three.** ① The HEX row drops its in-field "HEX" label (the always-present # is the identifier) and auto-restores the # if it goes missing; the click-to-select-all behaviour is removed — the first click just places the caret. ② The copy glyph uses absolute centring coordinates — the inner layout's centre alignment only covers the main axis, so a fixed-height glyph still rode high under cross-axis stretch ("only the hover background was centred"). ③ The footer warning drops word-wrap: a wrapping text's preferred width equals the full row, which shoved the swatch to the far end — it now hugs the sentence. ④ The wheel's four outside-circle corners now drag the card — the wheel touch area's full 240×240 hit box used to swallow those clicks (the four corner markers in the report). ⑤ The card's top padding grows 8 → 14px to match the bottom.
- **调色盘细节优化（第二轮）。** ① HEX 行复制键放大到 26×26 / 图标 16px（与 13px 数值协调），保持居中；② 行标识（HEX / R / G / B）从 muted 10px 提亮为**前景色 + 13px**，与右侧数值同档同号（此前像"备注说明"）；③ 右上角 ✕ 与 18px 头部空条**整条删除**（关闭走 取消 / 确认 / 遮罩 / Esc，拖动走卡片空白区，与设置卡 / 自定义弹框同口径）；④ 页脚加**顶部横线**，取消 / 确认改为 **64×36**（与预设弹框按钮同规格，原 84×30 偏大偏矮）。
- **Color-picker polish, round two.** ① The HEX row's copy key grows to 26×26 with a 16px glyph (matching the 13px value text) and stays centred. ② Row identifiers (HEX / R / G / B) move from muted 10px to the foreground tier at 13px — the same tier and size as the values they label. ③ The top-right ✕ and the empty 18px header strip are removed entirely (closing goes through Cancel / Confirm / backdrop / Esc; dragging stays on the card's blank areas, the same convention as the settings and custom dialogs). ④ The footer gains a top divider line, and Cancel / Confirm resize to 64×36 — the preset popup's button spec (the old 84×30 was wider and shorter).
- **调色盘三项修复。** ① 输入框光标**行尾永不可达**：值列右对齐会把命中测试全映射到偏移 0（#picker-input-edit 自己诊断过的老问题，"点入即全选"只绕过第一次点击，再点落光标仍回行首）——改为左对齐，行尾可达。② HEX 行复制键：固定高方块在 HL 里顶对齐（图标偏上）+ Icon 未写宽高铺满整盒 → 包居中 VL + 显式 12×12；并新增「已复制」回执（1.4s 绿字气泡，每次点击重启，与侧栏 Copied 徽标同款）——此前点击无任何反馈。③ 页脚警示行重组：16px 色块原先塞在句中段，换行时夹在两行文字中间；改为两行版式（第一行警示句 + 句末色块，第二行操作说明）。
- **Three color-picker fixes.** ① The caret could never reach the end of a value — right-aligned inputs map all hit-testing to offset 0 (the defect already diagnosed during the click-to-select-all rework, which only worked around the first click); values are now left-aligned so the caret lands where you click. ② The HEX row's copy key: its fixed-height button sat top-aligned in the row (glyph rode high) and the glyph filled its whole box — now wrapped in a centring layout with explicit 12×12 sizing; a click also raises a green "Copied" receipt for 1.4s, restarting on every click (same language as the sidebar's Copied badge), where previously there was no feedback at all. ③ The footer's contrast warning is restructured into two lines — the warning sentence with the target swatch at its end, then the usage hint — instead of a mid-sentence swatch wedged between wrapped lines.
- **复制回执统一为一处；「数据目录」toast 的对勾归位；设置弹框每次打开都回默认页。** ① 复制路径此前有两套反馈：页面右上角的 toast（「路径已复制」）与按钮自带徽标——而 toast 只在 status「变化」时重新弹出，所以每次启动只有第一次复制会弹它、之后只剩徽标（用户实测的不一致）。现统一保留按钮徽标（贴着按钮、每次点击都有），复制不再触发页面 toast（toast 仍服务于迁移结果 / 错误）。② toast 里的对勾 / 错误图标此前未写显式宽高——`Icon` 在非布局父级里默认铺满整盒、内部再居中，图标飘到盒子中下部；补上 13×13 后 x/y 定位生效。③ 设置弹框此前「会话内记住上次页、重启回默认」；现统一为**每次打开都回默认页（主题）**（与速选面板 Tab 直达主题的既有重置同口径）。
- **Copy feedback unified; the directory toast's check mark fixed; the settings dialog now always opens on its default page.** ① "Copy path" used to give two kinds of feedback — a page-level toast ("Path copied") plus the button's own badge — but the toast only re-armed when the status string *changed*, so only the first copy after each launch showed it while later copies fell back to the badge alone (the inconsistency reported from testing). The badge (anchored at the button, shown on every click) is now the single channel; copying no longer fires the page toast, which stays for migration outcomes and errors. ② The toast's check/error glyph lacked explicit dimensions — inside a non-layout parent the `Icon` fills the whole box and re-centres itself, dropping the glyph into the middle of the toast; a 13×13 size makes the x/y placement stick. ③ The settings dialog used to remember the last page within a session but reset on restart; it now consistently reopens on its default page (Theme), matching the existing reset used by the theme-picker's Tab shortcut.
- **资源栏「内存 / 交换」数值对齐设计稿。** 这两个数值此前是全批「数值回前景档」里唯一漏网的一处：颜色停在描述灰、也没用设计稿的等宽数字字形。现按 mock `.ui-num` 规格改为**前景色 + mono + 600 字重**（与磁盘「可用/大小」、滑块百分比等数值同类同档）。
- **The resource dock's Memory / Swap figures now follow the design spec.** These two values were the last hold-out of the "resource figures return to the foreground tier" pass — left in description grey and without the spec's monospaced digits. They now use the mock's `.ui-num` treatment: **foreground tier + mono + 600 weight** (matching the disk Free/Total figures, slider percentages and the other resource values).
- **「数据目录」路径栏的复制按钮有了回执，悬停提示样式并入统一规范。** 点击「复制路径」此前没有任何确认——唯一的「消息」是悬停 tooltip，而点击又会被 tooltip 抑制逻辑（#tip-hold）收掉，表现为「只有第一次有动静」；且这个气泡是自绘的（`bg-active` / `radius-md` / 文字左靠），与顶栏 tooltip 统一浮层（`bg-elevated` / `radius-sm` / 22px 高 / 文字居中 / 阴影 8px）不是一个样式。现在：点击复制后原位弹出绿色「已复制」气泡 1.4 秒（**每次点击都重新计时**，与侧栏主机复制的 Copied 徽标同一语言）；tooltip 与回执共用同一气泡，样式逐项对齐顶栏统一浮层。
- **The path bar's copy button now gives a receipt, and its hover tip follows the app's tooltip spec.** Clicking "Copy path" used to give no confirmation at all — the only "message" was the hover tooltip, which the click then suppressed via the tip-hold logic, reading as "something only happens the first time"; the self-drawn bubble also used its own styling (`bg-active` / `radius-md` / left-aligned text) instead of the top bar's unified tooltip overlay spec (`bg-elevated` / `radius-sm` / 22px tall / centred text / 8px shadow). Now a green "Copied" bubble replaces it in place for 1.4s on **every** click (the timer restarts each time — the same language as the sidebar host-copy's Copied badge), and the tooltip and the receipt share one bubble aligned with the unified overlay spec.
- **弹框字段标签对齐设置面板：常态改用前景色。** 「新建会话」等弹框里每个输入框 / 下拉上方的字段标签（名称、连接协议、主机 / IP、端口、认证方式、用户名、密码…）原先停在描述灰（`text-secondary`），而设置面板的行标签早已是前景色——同为「行标识」，两处两种亮度。现把弹框侧统一到设置面板口径（`text-primary`）：改动 = 五处共享定义（`LabeledInput` 标签、带标签下拉的标签、`ProtoSelect`、`GroupCombo`、「认证方式」小节标签，所有弹框一次生效），外加自绘标签一次收口——同步输入弹框的左右栏头（当前窗口中的会话 / 频道 X 成员）、重命名 / 新建分组 / 快捷命令 / 下载目录 / SFTP 提示 / 优化权限等弹框内的同类标签、新建会话高级区的小节标签；分组对话框的名称预览两态一分为二（有名 = 前景色，空 = 弱化档占位）。占位符与辅助说明仍保持弱化档，label 亮 → 提示暗 的两级对比不受影响（分段控件未选中态等 hover 三元语义不变）。
- **Dialog field labels now match the settings panel: the resting colour moves to the foreground tier.** In "New session" and the other dialogs, the field label above each input or dropdown (Name, Protocol, Host / IP, Port, Authentication, Username, Password, …) sat one tier below the settings panel's row labels — description grey (`text-secondary`) versus `text-primary` for what is the same "row identifier" role. The dialogs are now aligned with the settings panel via five shared definition points (`LabeledInput`'s label, the labelled dropdown's label, `ProtoSelect`, `GroupCombo`, and the "Authentication" section label — every dialog updates at once), plus a sweep of hand-drawn labels (the sync-input dialog's two column headers, the rename / new-group / quick-command / downloads / SFTP-prompt / permissions dialogs, the new-session advanced section labels, and the group dialog's name preview, whose two states are now split: named = foreground, empty = faint placeholder). Placeholders and helper text keep the faint tier, so the bright-label / dim-hint contrast is unchanged (segmented-control idle states and other hover ternaries are untouched).
- **界面灰度补遗：分组名、资源栏、协议图标等回到前景档。** 上一轮灰度收敛扫的是裸值 `tint:`，漏了两类绑定：一是两个组件的自定义属性 `icon-color`（`ProtocolIcon` / `IconBtn`，7 处）；二是一批按高保真 mock 口径应为前景档、实际留在次级或弱化档的文字与图标——会话列表的分组名、资源栏的 CPU/内存/交换标签、↑↓ 流量数值、「服务器资源 / 本机资源」标题、本地网络图题「本机」、磁盘表头与磁盘行（mock 的 `.ui-metric` / `.ui-delta` / `.ui-disk` 与「服务器资源 / 本机」内联全部是前景色）。本批同时处理 mock 未覆盖界面的同类问题：菜单分区标签（两处「Move to」）与快捷键弹框四个分区标题从弱化档提到次级档、三个不透明度滑块的百分比数值、进程窗 MEM 列（原本比 CPU 列暗一档）、跳板信息卡 Host/User/Auth/Group/Note 五行（原本比同卡 Name 行暗）、以及三处图标与标签不同档的小 chip（快速命令「Quick」、存储路径 chip、同步窗口 chip）。窗口控制键、占位符、提示文字与字段标签维持原语义；全部只改目标档位，不动任何色值。
- **Grayscale follow-up: group names, the resource dock and protocol icons return to the foreground tier.** The previous grayscale rebalance swept bare `tint:` values and missed two kinds of bindings: the `icon-color` custom property on the `ProtocolIcon` / `IconBtn` components (seven sites), and a set of texts and icons that the high-fidelity mock puts in the foreground tier while the code had left them one or two steps down — session group names, the resource dock's CPU/Memory/Swap labels, the ↑↓ throughput figures, the "Server resources / Local resources" titles, the local network graph's "Local" caption, and the disk table's header and rows (the mock's `.ui-metric` / `.ui-delta` / `.ui-disk` and its inline "Server resources / Local" are all foreground). The batch also covers the same class of issues on surfaces the mock does not draw: the menu section labels ("Move to" in two menus) and the shortcuts dialog's four section titles move from the faint tier up to secondary, the three opacity sliders' percentage readouts, the process window's MEM column (it sat one tier below CPU), the jump-chain info card's Host/User/Auth/Group/Note rows (they sat one tier below the card's own Name row), and three chips whose icon disagreed with its label (the Quick-command chip, the storage path chip, the sync window chip). Window controls, placeholders, hints and field labels keep their semantics; only the target tier changes, and no colour value is touched.
- **调色盘那四个输入框，现在一次点击就能改。** 反馈是「第一次点只出聚焦边框、打不进字；能编辑之后光标又落在行首、怎么也到不了行尾」。用 Slint 官方测试后端真派发鼠标 / 键盘事件复现（`src/picker/probe_test.rs` 探针）后，两条真因都在输入行自身，与事件派发无关：其一，值列是右对齐，文字左边那一大片空白在命中测试里全部映射到偏移 0 —— 点哪儿光标都落「行首」，而文字右缘紧贴内边距、右边根本没有可点区域，靠鼠标永远到不了行尾（旧代码想用「聚焦时把选区设到末端」把光标带过去，但那次赋值发生在同一次按下的前半段，紧接着的光标定位又把它覆盖回 0）；其二，打字途中每按一个键，都被 `hex-apply` 钳过的值回灌进输入框，想输入 300 就会在第三个键上看到 255。现在改成数值框的通行做法：**点入即全选**（0ms 计时器推到本次事件派发之后执行，避开上面那次覆盖），打字 / 粘贴直接替换整段值；**再点一次才落光标**，想改哪一位点哪一位，右端空白也能点到行尾；**聚焦期间不回灌**，离焦（点空白或焦点移走）那一刻再收敛回事实值。
- **Palette value fields: a single click is now enough to edit.** The report was "the first click only draws the focus ring and nothing can be typed; once it does accept input the caret sits at the start of the line and cannot be moved to the end". Reproduced by dispatching real pointer/key events through Slint's official testing backend (probe in `src/picker/probe_test.rs`); both causes are in the field itself, not in event delivery. First, the value column is right-aligned, so all the empty space left of the text maps to offset 0 in hit-testing — the caret lands at the start wherever you click, and since the text's right edge sits against the inner padding there is no clickable area past it, so the end of the line is unreachable with the mouse (the old code tried to carry the caret to the end with a selection-offset assignment on focus, but that ran in the first half of the press and the caret placement right after it overwrote it back to 0). Second, every keystroke echoed the hex-apply-clamped value back into the field, so typing 300 showed 255 on the third key. Both are now handled the way numeric fields normally are: **clicking selects the whole value** (deferred through a 0 ms timer so it lands after the press's own caret placement), so typing or pasting replaces it outright; **a second click places the caret**, so any single digit can still be edited, including at the end; and while the field has focus the value is **not** echoed back — it reconciles on blur (blank click or focus moving away).
- **对比度说明并入页脚。** 「✓ 对面板对比度达标…」「⚠ 对面板对比度不足 3:1…」这两句原来自占一行，现在挪到页脚左侧，与「取消 / 确认」同一行，调色盘整卡矮了约 26px。页脚布局刻意不写 `alignment: end` —— Slint 的主轴 flex_grow 只在 `alignment: stretch`（HorizontalLayout 默认值）下开启，写了 end 提示框就吃不到剩余宽度。
- **The contrast note moved into the footer.** The "✓ Contrast OK…" / "⚠ Below 3:1…" line used to occupy a row of its own; it now sits at the left of the footer next to Cancel / Confirm, making the palette card about 26px shorter. The footer deliberately does not set `alignment: end`: Slint only enables main-axis flex_grow when `alignment: stretch` (the HorizontalLayout default), so setting it would stop the note from taking the leftover width.
- **调色盘的三行分量改成「标签 · 渐变滑杆 · 数字框」，HEX 行加了复制键。** 这是高保真对照稿里选定的方案 B：行高与原来同构（36px），右列总高、色轮尺寸、整卡高度都不变，只是把「看得见的值」变成「拖得动的值」——滑杆底是该分量从最小到最大的两端色现推出来的渐变（其余分量保持不变；色相 H 例外，用七停彩虹；HSL 的 L 例外，中间补一停当前色相，否则黑到白看着像灰阶），整行 36px 都是命中区、视觉条只有 6px，好抓好看，滚轮一格走 1 个单位；三种格式（RGB / HSV / HSL）各按自己的量程映射（rgb 0-255、H 0-360、S/V/L 0-100）。数字框仍是同一个输入行组件（点入全选、打字不回灌、离焦收敛那套一字未改），**手输与拖动汇进同一个出口**（写入后一律走 `hex-apply` + 回填取样点），不存在第二套钳制与联动逻辑；拖动时会先把正聚焦的输入框离焦，免得它留着不跟手。HEX 行尾的复制键复用既有剪贴板回调，点它不会让聚焦中的输入框离焦、也不会冲掉打字草稿。
- **The palette's three component rows are now "label · gradient slider · number box", and the HEX row gained a copy button.** This is option B from the high-fidelity comparison: row height stays 36px, so the right column's total height, the wheel's size and the card's height are all unchanged — the only difference is that the values are now draggable as well as readable. The slider's track is a gradient built on the fly from that component's minimum and maximum with the other components held fixed (hue is the exception and uses a seven-stop rainbow; HSL's lightness gets a middle stop at the current hue, otherwise black-to-white just reads as a grey ramp); the whole 36px row is the hit area while the visible bar is 6px, so it is easy to grab, and the wheel moves it one unit per notch. Range mapping follows the format (rgb 0-255, H 0-360, S/V/L 0-100). The number box is still the same input component (click-to-select-all, no echo-back while typing, reconcile on blur — unchanged), and **typing and dragging converge on the same exit** (`hex-apply` plus a sample-point refresh), so there is no second clamping/linking path; starting a drag first blurs any focused field so it cannot sit there stale. The HEX row's copy button reuses the existing clipboard callback, and clicking it neither blurs the focused field nor discards a half-typed draft.
- **强调色预设弹框「选定 / 调色盘」按钮的底边缝隙修掉了。** 这个两段合并按钮的色块与外框之间，底边夹着一行卡片底色，看着像色块没铺满；顶部同一结构严丝合缝。修了两处。一是弹框高度由内容累加得出、带着小数，在 125% / 150% 这类分数缩放下卡片框本身落不到设备像素网格上，现在高度、横纵落位与拖动落点全部对齐设备网格（与设置卡、主窗口描边同一套例程）。二是卡片边框与色块的「1 像素」内缩在 125% 下实为 1.25 物理像素，两者相接的公共边界永远带小数，填充与描边两条渲染路径取整不一致时便空出一行，这些纵向内缩现在统统改用 1 设备像素（hairline），与设置卡、主窗口描边同款。上下两边同时落在网格上，缝消失。
- **The accent popup's "Selected / Palette" button no longer shows a seam along its bottom edge.** Between the colour well and the card border a row of card background showed along the bottom edge only, reading as a well that failed to fill its box while the top edge sat flush. Two things were fixed. The popup's height is the sum of its contents and carries a fraction, so at 125% / 150% scaling the card itself fell off the device-pixel grid; height, position and drag landing now snap to the grid (the same routine the settings card and the window frame use). The card border and the well's 1-pixel inset are 1.25 physical pixels at 125%, so the boundary they share always carried a fraction and the fill and stroke paths rounded it differently, leaving a blank row; those vertical insets now use one device pixel (hairline), matching the settings card and the window frame. Both edges land on the grid and the seam is gone.
- **主题精简到 28 套，速选浮层按高保真重做，界面灰度收敛。** 色表从 38 套精简到 28 套：下线 12 套「伪重复」（Radix 六族各两套同底改色、Tokyo Night 双份、Ayu 暗档、GitHub 暗档），补回 3 套此前缺位的明暗配对亮档（Dracula 亮 / Everforest 亮 / One Dark 亮），并对 26 套做色值修正——门面 MTerm 两套的注释槽 / 强调色 / 24 色分组与频道色全部重算，t3 注释色全表优化，Rosé Pine 的状态槽与语义色解耦，亮档彩槽补足对比度，Solarized 与 Darcula 亮档重制。默认主题改为门面 **MTerm 暗 / 亮**（Ctrl+Alt+T 亦在其间循环），`islands-*` / `idea-*` 随改名归位 `mterm-*` / `darcula-*`，新增的 One Dark 亮档由 GitHub Light 重着色而来。主题速选浮层重做：顶部「常用」区（出厂 MTerm 暗 / 亮两套，可自选增删、上限 8 套，点星即整理）+「全部主题」滚动区；每行 = 强调色圆点 + 名称 + 3 色阶条（root/panel/tbg，带棋盘衬底，暗档也看得清）+ ANSI 八色带；悬停 **250ms** 即预览且**同步终端色表**（预览与落盘观感一致），八色带淡出让位给「设为常用 / 移出常用」；搜索从「只匹配中文名」扩到中文名 / id / 色相名；新增 ↑↓ / Enter / Esc / Tab 键盘导航与「跟随系统 / 反向（亮↔暗）」入口，底部常驻「悬停即预览 · 点击即落盘 / 常用写进 config」说明栏。界面灰度收敛：77 处图标与 4 处常态导航文字从 `text-secondary` 提到 `text-primary`（原先占全部文字 68% 的灰档在浅色主题下过淡），状态区分交回底色与竖条表达，**不改任何色值**。另：强调色网格首格的「深渊色」机能性最弱（极深色作强调色，在暗色面板上几乎不可见且必被对比度守卫改写），替换为「MTerm 青」。
- **28 themes instead of 38, a rebuilt theme picker, and a grayscale rebalance.** The palette shrinks from 38 to 28: twelve pseudo-duplicates are gone (the six Radix families each kept two themes that were the same base recoloured, plus the two Tokyo Night variants, Ayu Dark and GitHub Dark), three missing light counterparts are back (Dracula Light, Everforest Light, One Dark Light), and 26 themes had their colours corrected — the MTerm facade pair's muted/accent/24-colour group and channel colours are recomputed, the t3 muted slot is optimised across the table, Rosé Pine's state slots are decoupled from its semantic ones, the light themes' colour slots gain contrast, and Solarized Light and Darcula Light are rebuilt. The default theme is now the **MTerm facade** (Ctrl+Alt+T cycles the two MTerm themes), `islands-*` / `idea-*` are renamed `mterm-*` / `darcula-*`, and the new One Dark Light is GitHub Light recoloured. The quick-pick popup is rebuilt: a "Favorites" section on top (the two MTerm themes out of the box, self-editable up to 8, star to pin/unpin) over a scrolling "All themes" list; each row is an accent dot, the name, a 3-step root/panel/tbg swatch strip (checker-backed so dark themes stay legible) and an ANSI eight-colour band; hovering previews after **250ms** and **syncs the terminal palette** so preview and commit look identical, with the band fading out for "Pin / Unpin"; search now matches name / id / hue name instead of the Chinese name only; ↑↓ / Enter / Esc / Tab keyboard navigation and "Follow system / Invert" entries are added, with a footer that states "hover to preview · click to apply / favorites saved in config". The grayscale rebalance lifts 77 icons and 4 idle navigation labels from `text-secondary` to `text-primary` (that grey tier used to cover 68% of all text and washed out on light themes); state is expressed by background and accent bars instead, and **no colour value changes**. Also: the accent grid's first cell, "Abyss", was the weakest possible accent (near-black, invisible on dark panels and always rewritten by the contrast guard), so it is replaced by "MTerm Cyan".
- **修复内置 RDP 一改窗口尺寸就掉线的问题。** 选了「跟随标签页」的会话会把新的桌面大小发给服务端，服务端随即进入协议的停用—重激活流程；而内置客户端依赖的 ironrdp-client 0.1.0 在重激活后重建 FastPath 处理器时丢掉了连接时协商好的批量解压器（写死 `bulk_decompressor: None`），于是服务端随后发来的压缩画面数据被当成未压缩数据解码，必然失败、会话结束——表现为一改尺寸就「已断线」，点重连又能进去并撞上「另一个会话被阻止」。修复方式是把 ironrdp-client 0.1.0 收进 `vendor/` 打补丁（沿用 winit 后端本地补丁的先例）：重激活完成时按连接时协商的压缩类型重建解压器。上游 master 已把解压器移进 `ActiveStage` 从根本上修掉这个问题，但修复尚未发布（0.17.0 / 0.1.0 仍是最新版），故暂用本地补丁，`Cargo.toml` 的 patch 段写了重新应用的方法。
- **Fix the built-in RDP client dropping the session whenever the window is resized.** A "Follow tab" session sends the new desktop size to the server, which then runs the protocol's deactivation–reactivation sequence; ironrdp-client 0.1.0 rebuilt its FastPath processor afterwards without the bulk decompressor negotiated at connect time (hard-coded `bulk_decompressor: None`), so the server's next compressed frame was decoded as plaintext, failed, and ended the session — resizing once meant "Disconnected", and reconnecting worked but ran into "another session was blocked". The fix vendors ironrdp-client 0.1.0 under `vendor/` (same pattern as the local winit backend patch) and recreates the decompressor from the compression type negotiated at connect time. Upstream master fixes this properly by moving the decompressor into `ActiveStage`, but the fix is unreleased (0.17.0 / 0.1.0 are still the latest), so a local patch stands in; the `[patch.crates-io]` section of `Cargo.toml` documents how to re-apply it.
- **「跟随标签页」不再被存丢，新建内置 RDP 会话默认就用它。** 「跟随标签页」是唯一能让远端桌面铺满标签页的选项，但在会话对话框里选它**从来没生效过**：分辨率下拉只写 `draft.rdp_resolution`，而保存时另一条手写的构造路径直接读 `draft.rdp_follow_tab_size`（一个界面根本没有控件能写的字段，永远是 `false`），于是这个选择每次都被静默丢弃，桌面停在一个写死的尺寸上。现在保存路径与构造路径共用同一个判据（`session_models::follows_tab_size`：下拉选了 follow 或存量配置带了该标记都算），下拉与这个 bool 也同步改写——所以**从「跟随标签页」改回固定分辨率同样能生效**，不再单向卡死。其次，新建会话时切到「内置客户端」若分辨率还是出厂值，会自动预置为「跟随标签页」（编辑存量会话不碰已存值；切回「系统客户端」会自动还原），不必每次新建都手动挑一遍。
- **"Follow tab" is no longer dropped on save, and it is the default for a new built-in RDP session.** "Follow tab" is the only option that makes the remote desktop fill the tab, but picking it in the session dialog **never took effect**: the resolution dropdown writes only `draft.rdp_resolution`, while the save path read `draft.rdp_follow_tab_size` — a field with no control in the UI, therefore always `false` — so the choice was silently dropped every time and the desktop stayed at a hard-coded size. The save path and the constructor now share one judgement (`session_models::follows_tab_size`: the dropdown says follow, or a stored config carries the flag), and the dropdown keeps the two fields in sync — so **switching back from "Follow tab" to a fixed resolution works too**, instead of latching one-way. Also, switching a new session to the built-in client now presets the resolution to "Follow tab" when it is still at its factory value (an existing session's stored value is never touched; switching back to the system client restores the default), so a new built-in RDP session no longer needs the dropdown touched by hand.
- **删掉远程桌面标签页上的连接状态点。** 标签栏左侧的小圆点（绿=已连接 / 黄=连接中 / 红=断线或失败）已移除——远程桌面的连接状态本来就在 Tab 内容区里用整页状态（转圈 / 「重连中…」/ 「已断线 — <原因>」/ 「连接失败」）表达，标签栏再画一个点是同一件事的第二种说法，且颜色语义与内容区并不总是一致。左侧会话列表的在线状态点不受影响（那是另一处、另一种汇总口径）。
- **Remove the connection status dot from remote-desktop tabs.** The little dot on the left of the tab strip (green = connected, amber = connecting, red = disconnected or failed) is gone: a remote tab already states its connection in its content area — the full-page spinner, "Reconnecting…", "Disconnected — <reason>", "Connection failed" — so the dot was a second way of saying the same thing, and its colour semantics did not always agree with the content area. The online dot in the session list is unaffected (different place, different aggregation).
- **VNC 连接被拒绝时立刻说明原因，不再先空转 7 秒。** VNC 连不上、且原因是端口上没有服务在听（TCP 被拒）时，驱动此前会按「传输错误」重试 3 次（退避 1+2+4 秒）才报失败——而这 7 秒里界面停在「重连中…」，看不到任何原因，而「端口上没有服务」这个结论在第一次握手失败时就已经成立。现在 `ECONNREFUSED` 直接判失败，其余传输错误（超时、重置、不可达）仍然照旧重试。
- **A refused VNC connection now reports its reason immediately instead of spinning for seven seconds.** When a VNC connection fails because nothing is listening on the port (TCP refused), the driver used to treat it as a retryable transport error and try three times (backoff 1+2+4s) before failing — and for those seven seconds the UI sat on "Reconnecting…" with no reason, even though "nothing listens on this port" is already true at the first handshake. `ECONNREFUSED` now fails immediately; every other transport error (timeout, reset, unreachable) still retries as before.
- **「重新连接」从空转修成真能用、断线带上原因、状态页居中。** 远程桌面 Tab 上的「重新连接」此前是个**彻底的空操作**：它往一条在创建处就被丢掉接收端的通道里发消息（发送必然失败，且错误被吞掉），而这个按钮又只在会话已经结束（重连中 / 已断线 / 失败）时才出现 —— 那一刻驱动早已丢掉 client future、关掉 socket 并把会话标成已关闭，句柄上的任何发送都是空操作。现在它**在同一个 Tab 里重新起一次会话**：面板位置、分组、标题、只读标记全部保留，上一段的失败原因与残留画面在重开前清掉，凭据仍按原样取（会话密码 / Windows 凭据管理器），证书信任按指纹复用、不会重问；连会话配置都起不来的极端情况也会把原因写在页面上，而不是让 Tab 永远停在「连接中」。其次，「已断线」一直是真话（对端挂断、会话被接管、传输中断都落到这一态），但驱动手上那条断开原因原来只写进日志、没告诉用户 —— 现在会翻成一句话贴在状态后面（如「已断线 — 服务器断开了连接」「已断线 — 远端用户断开了连接」）；驱动说不出原因时仍是朴素的「已断线」，不会硬凑成「已断线 — 连接失败」。最后是版式：转圈与「重新连接」按钮此前都贴着 Tab 左边缘（状态区只做了垂直居中，这两个固定尺寸元素没有水平居中的包裹），现在转圈在文案上方、两者都居中。
- **The Reconnect button does something, a disconnect says why, and the status page is centred.** The Reconnect button on a remote-desktop tab used to be a **complete no-op**: it sent to a channel whose receiver was dropped where it was created (the send always failed, and the error was swallowed), and the button only appears once the session is already over (reconnecting / disconnected / failed) — by which point the driver has dropped its client future, closed its sockets and marked the session closed, so nothing sent through the handle can do anything. It now **starts a new session in the same tab**: position, group, title and read-only flag are all kept, the previous failure text and the stale picture are cleared first, credentials are read the same way as before (session password / Windows Credential Manager), and the certificate trust is reused by fingerprint rather than asked again. Even a session definition that cannot start reports its reason on the page instead of leaving the tab on "Connecting…" forever. Second, "Disconnected" was always true (the peer hung up, the session was taken over, the transport died) but the reason the driver held was only ever written to the log — it is now translated into a clause appended to the state, such as "Disconnected — the server closed the connection" or "Disconnected — the remote user disconnected". A driver with nothing to say keeps the bare "Disconnected" rather than inventing "Disconnected — connection failed". Finally, layout: the spinner and the Reconnect button used to hug the tab's left edge (the status area only centred vertically, and neither fixed-size child had a horizontal centring wrapper); the spinner now sits above the text and both are centred.
- **内置 RDP 在密码框留空时会改用 Windows 凭据管理器里已存的密码。** 此前「系统客户端」能免输密码直连、而「内置客户端」停在远端登录界面，差别从来不在主机，而在取密码的地方：系统模式把连接整个交给 `mstsc`，由 `mstsc` 自己去读凭据管理器里的 `TERMSRV/<主机>`；内置客户端只认密码框。现在密码框留空时，内置客户端也去读同一条（`普通` 类型）凭据，读到就用它认证（NLA，直达桌面）；没有条目、条目不可读或内容为空，则维持原行为（纯 TLS，由远端登录界面收密码）。存在凭据里的密码不会被读回到界面、也不会被回写。Windows 上会话编辑框的占位符相应改为「留空 — 用已保存的 Windows 凭据，否则连接时提示」。
- **The built-in RDP client now fills a blank password field from the Windows Credential Manager.** The system client could always connect without typing a password while the built-in one stopped at the remote logon screen, and the difference was never the host: system mode hands the connection to `mstsc`, which reads `TERMSRV/<host>` itself, whereas the built-in client only knew the password box. With a blank field it now reads that same entry (type "generic") and authenticates with it — NLA, straight to the desktop. No entry, an unreadable entry, or an empty one keeps the previous behaviour: plain TLS, with the server's own logon screen collecting the password. The stored password is never echoed back or written over. On Windows the dialog placeholder now reads "Leave blank — saved Windows credential, else prompt".
- **连接失败改为「一句人话 + 可展开的错误详情」。** 远程桌面（RDP/VNC）连接失败时，界面主标题不再直接铺驱动返回的原始错误文本（形如 `CredSSP: InvalidToken: CredSSP server returned an error status; status is STATUS_LOGON_FAILURE [0xc000006d]`），而是先给一句话——按 NTSTATUS 码与 socket 错误码归类为「用户名或密码不正确」「账户已被锁定」「目标主机拒绝了连接」「连接超时」「服务器证书未被接受」等；驱动原文一字不改地收进「显示错误详情」开关（排查与反馈仍需要它）。此前同一段原文还会在标题与详情两处各显示一遍，现在两处各司其职。
- **A failed connection now leads with one readable line, with the driver's own text one toggle away.** The headline used to be exactly what the driver returned (`CredSSP: InvalidToken: CredSSP server returned an error status; status is STATUS_LOGON_FAILURE [0xc000006d]`). It is now classified — by NTSTATUS value or socket error code — into a sentence such as "Wrong username or password", "The account is locked out", "The host refused the connection", "Connection timed out" or "The server certificate was not accepted". The original text is kept verbatim behind a "Show error details" toggle, because diagnosing and reporting still need it, and it no longer appears twice at once (headline and detail).
- **密码框改为「留空即写入空密码」。** 编辑会话时，密码框里的内容就是保存结果——清空它就把已存密码抹掉。此前「留空」表示"保留原密码"，于是没有任何办法把一个会话退回无密码（远程主机没设密码时正需要这个）。这一个框同时承担 SSH 密码、SSH 私钥口令、RDP 密码和 VNC 密码。**注意**：出于安全考虑该框仍不回显已存密码，所以**编辑任何字段并保存都会清掉已存密码**（只改个名字也一样），下次连接时由各协议重新询问。粘贴的私钥保持原语义（留空仍保留）。
- **Blank now means "no password" in the session dialog.** When editing a session the password box is the source of truth: clearing it stores an empty password. Previously "blank" meant *keep the saved one*, which left no way to take a password back off a session — exactly what a host without a password needs. The one box covers the SSH password, the SSH key passphrase, RDP and VNC. **Note**: the box still never echoes the stored secret for security, so **saving any edit — a rename included — clears a saved password**; the next connection asks again per protocol. The pasted private key keeps its old blank-keeps behaviour.
- **移除协议下拉里 VNC 的「即将」徽标。** VNC 已实装（内置 vnc-rs 客户端在标签页里承载桌面），却仍挂着「即将」。徽标本身与背后的 preview/soon 机制（`preview-kind`、`is-preview`、`badge-of`、`start-preview` 及「预览不可创建」的守卫）已一并删除 —— 五个协议现在全是实装项。
- **Remove VNC's "Soon" badge from the protocol dropdown.** VNC is implemented (the built-in vnc-rs client hosts a desktop in a tab) but still carried a "Soon" chip. The badge and the preview/soon machinery behind it (`preview-kind`, `is-preview`, `badge-of`, `start-preview` and the "preview cannot be created" guard) are gone — all five protocols are live.
- **新增「MTerm · 亮」主题（自有设计「素纸」），与「MTerm · 暗」配对。** 设置 → 主题的「浅色主题」组新增一套纸白底 + 单一青绿强调（`#1f766b`）的亮色主题，与既有的「MTerm · 暗」同族、在选择器里相邻。三条配色纪律写进了回归测试：①底与灰只留约 6% 暖相——质感来自低饱和而非明显米黄；②全屏一套灰——终端前景直接等于正文一级灰、ANSI 注释色等于最弱灰，不再有"界面暖灰 + 终端冷灰"的双灰并存；③彩色只有强调色一处——终端 ANSI 的青与强调色同族。终端 ANSI 为自建谱系（饱和收进一档靠明度分层，亮底上 bright 档取更深的同色相），12 个彩色槽在该套底色上最低 4.54:1。切换时终端调色板一并生效，独立进程/编辑器窗口同步跟随。
- **Add the "MTerm · Light" theme (an original "paper" design), pairing with "MTerm · Dark".** The Light themes group gains a paper-white theme with a single teal accent (`#1f766b`), same family as the existing "MTerm · Dark" and adjacent to it in the picker. Three colour rules are pinned by regression tests: (1) backgrounds and greys keep only ~6% warmth — the feel comes from low saturation, not visible beige; (2) one grey family on screen — the terminal foreground *is* the primary text grey and the ANSI comment grey *is* the faintest grey, so a warm UI grey and a cool terminal grey can never coexist; (3) the accent is the only hue — the terminal's cyan shares the accent's hue. The terminal ANSI palette is built in-house (saturation gathered into one band and separated by lightness, with bright slots taking a deeper shade of the same hue on a light background); the lowest of its twelve coloured slots measures 4.54:1 on this theme's background. The terminal palette switches together with the UI, and the detached process/editor windows follow along.
- **新增「MTerm」主题（上游 Islands Dark 移植）。** 设置 → 主题的「深色主题」组新增一套：深底浮岛造型，界面本体几乎无彩色、只有强调色与状态色说话。留白面、文字、成功/警告/危险与终端 ANSI 均采用上游原色（12 个彩色槽在该套底色上全部达到 4.5:1）；强调色按本项目的观感定档为上游的**青** `#2aacb8`（上游 `textLink`/`button` 原本用蓝，与既有「经典蓝/霓蓝」重复，故改用该套自己的青）。切换时终端调色板一并生效，独立进程/编辑器窗口同步跟随。
- **Add the "MTerm" theme (a port of Islands Dark).** The Dark themes group gains a deep-canvas, floating-panel look where the chrome stays almost colorless and only the accent and status colours speak. Canvas, text, success/warning/danger and the terminal ANSI palette all use upstream's values (all twelve coloured slots clear 4.5:1 on that theme's own background); the accent is set to that theme's own **cyan** `#2aacb8` (upstream's `textLink`/`button` used blue, which duplicated the existing "classic blue / neon blue" presets). The terminal palette switches together with the UI, and the detached process/editor windows follow along.
- **统一鼠标悬浮背景并让它跟随主题强调色。** 此前悬浮高亮有两套互不相干的实现——列表行用写死的反色蒙版，工具栏图标用主题里的固定灰色——同一屏里是两种观感，且各主题的"重"程度相差最多 12 倍。现在合并为一个跟随强调色的浅染（强调色压暗 10% 后叠 8%），列表行、工具栏、按钮、标签的悬浮完全一致；参数按 37 套主题 × 4 种底面的正文对比度逐一核算选出，可读性不劣于改动前。
- **Unify hover highlighting and make it follow the theme accent.** Hover used to be two unrelated implementations — a hard-coded inverted overlay for list rows and a fixed theme grey for toolbar icons — so one screen showed two different looks, and the "weight" of a hover varied by up to 12× across themes. They are now a single accent-tinted wash (the accent darkened 10%, laid down at 8%), identical on list rows, toolbars, buttons and tabs; the values were picked by checking body-text contrast across all 37 themes × 4 surfaces, so legibility is no worse than before.
- **修复分组脊线上的亮点与分组拖动的浮卡错位。** 组内成员之间的组色竖线不再因上下两段重叠而出现比线身更亮的短段；把一个分组拖到末位后再拖动它时，跟手的浮起卡片不再飞到列表顶部，落点判定也同步恢复正常。
- **Fix the bright spot on group spines and the misplaced drag card when dragging a group.** The group-colored spine no longer shows a brighter 2px segment between rows, and the floating drag card no longer jumps to the top of the list — the drop target tracks correctly again — when a group that was just moved to the end is dragged once more.
- **修复分组拖到末位后一直自我展开收缩。** 指针停在列表首/尾的越界区时，分组的"临时展开"与"拖离收起"会互相触发，形成每约 450ms 一次的展开收缩死循环（再点一次组名才停）；现在收起判定不再把越界停留当成"已离开"，组拖动期间也不收起，非拖动状态下更不会因残留的指针坐标自动展开。
- **Fix a group that keeps expanding and collapsing by itself after being dragged to the end.** With the pointer parked in the top/bottom overflow zone, a group's temporary expand and its "dragged away" collapse kept retriggering each other in a ~450ms loop (clicking the group name again was the only way to stop it). Collapsing no longer treats parking in the overflow zone as leaving, never runs during a group drag, and no longer auto-expands outside a drag at all.
- **拖到列表末尾的落点更好读，也不再把最后一组弹开。** 拖动分组到最下方时，折叠的最后一组不再被自动展开；插入线按最后一组的中线分区 —— 上半区仍是"插到它之前"，下半区改为"放到最后"：未展开时线落在组名下方，已展开时落在最后一个成员之后，并整体画在内容底下方一点，避免与成员行的组色脊线末端叠在一起。
- **Clearer drop feedback at the end of the list, and the last group no longer pops open.** Dragging a group to the bottom no longer expands a collapsed last group. The insert line now splits the last group at its middle: above means "before it", below means "at the end" — drawn under the group name when collapsed and after the last member when expanded, sitting just below the content bottom so it no longer overlaps the group-colored spine.
- **拖动成员到组内第一位时组名不再让位，落点框与让位同步推进。** 组头只在"待出组 / 出组"时下移（那才是抽屉动作），指针瞄准组内某一位时组名钉住不动，第一位才拖得进去；虚线框改为跟随让位位移、钉在真实插入位上，成员依次级联让位，不再出现"框先跳到组名下面、成员还没动"的错拍；浮卡顶边压住哪一行，那一行左侧与竖线相接的枝干横线就点亮。
- **The group name no longer yields while dragging a member to the first slot, and the drop box moves in step with yielding.** The group header now only shifts in the pending/leaving states (the real drawer motion), so aiming at a slot inside the group keeps the name in place and the first slot stays reachable. The dashed drop box follows the row's yield offset and stays pinned to the actual insertion point, members cascade in step, and the branch line of any row the card's top edge covers lights up.
- **成员拖动的手感与落点修正。** 拖动方向改由**指针位移**判定（原先看"指针是否在被拖行下方"，而拖起瞬间指针本就在行内，落点会凭空深一行、把虚线框推到下面去）；组名作为容器标题**恒定不动**——整条"组头让位"逻辑删除，ghost 碰到组名下边不再带动组名；组名的中线成为"上一组末位"与"本组第一位"的分界，跨组时虚线框跟着 ghost 翻到上一组末尾。
- **Drag feel and drop targeting for member sorting.** The drag direction now comes from the pointer's displacement (it used to ask whether the pointer sat below the dragged row — always true at pickup, which pushed the drop target a row too deep and sent the dashed box downwards). The group name now stays put as a container title: the whole header-yield path is gone, so the ghost touching the name no longer moves it. The name's midline divides "end of the previous group" from "first slot of this group", so the box flips to the previous group's tail as the ghost crosses it.
- **组排序插入线跟着浮卡走，不再提前一格。** 插入线原来按**指针**判定、且命中区覆盖整个组段（组头 + 全部成员），于是 ghost 还在组的中间或尾部时，线已经挂到该组名上方一格（拖动组别上移时尤其明显）。现在判定基准换成浮卡自己的**前导边**，命中区收窄成"本组头上方 → 下一组头上方"这一段，线与卡同拍换手。
- **成员拖动的落点框与让位严格对齐。** 落点框的位置改为"本行原位减去本行的让位位移"，唯一来源、不再叠加方向偏移（旧式在下移时会把框甩到**两个身位**之外）；命中行也改为"**前导边刚越过中线的那一行**"——也就是真正让位的那一行，于是"ghost 只负责定位、最终落点就是框的位置"这条规则成立：上移看顶边、下移看底边，越过成员一半即让位，框同时落到那一格。
- **新增列表空白处右键菜单，并统一成员拖动的命中基准。** 在列表空白处右键可"全部展开 / 全部折叠"分组，以及隐藏或显示"本地终端"保留组；组头右键菜单里同样放了一份，文案随当前状态切换。成员拖动时命中行与让位行共用浮卡的前导边，虚线框不会再压在成员身上、也不会甩出一个身位。
- **Right-click menu for empty list space, plus a unified hit basis for member dragging.** Right-clicking empty space in the list now offers "Expand all", "Collapse all", and hiding/showing the "Local Terminals" reserved group; the group header menu carries the same hide/show entry with a label that follows the current state. Member dragging derives both its hit row and its yield row from the drag card's leading edge, so the dashed box never covers a row or lands a slot off.

## [0.7.3] - 2026-09-07

- **停止 Android Beta 支持。** 移除 Android 客户端源码、APK 构建和发布任务，发布流程仅保留桌面平台。
- **Discontinue Android Beta support.** Remove the Android client source, APK builds, and release tasks; releases now target desktop platforms only.
- **修复内置文本查看器和编辑器的行号错位。** 行号按正文自动折行后的显示高度排列，保留空行与末尾换行，并随编辑、替换和窗口宽度变化同步更新；查找与替换输入框的文字和光标统一垂直居中。
- **Fix line-number alignment in the built-in text viewer and editor.** Align the gutter with wrapped text, preserve blank and trailing lines, and update it after edits, replacements, and width changes. Vertically center text and cursors in the find and replace fields.
- **修复串口会话误显示 SSH 端口（#406）。** 左侧和完整会话列表现在显示串口设备名、波特率及通信参数（如 `115200 baud · 8N1`），不再显示无关的 `:22`。
- **Fix serial sessions showing an SSH port (#406).** The sidebar and full session list now show the serial device, baud rate, and framing (such as `115200 baud · 8N1`) instead of an unrelated `:22`.
- **修复会话输入框提示文字重叠（#415）。** 提示文字置于输入控件下层，获得焦点时隐藏，避免与输入法尚未提交的组合文字重叠；覆盖普通输入框、分组框和端口转发字段。
- **Fix overlapping placeholders in session fields (#415).** Placeholders render beneath input controls and hide on focus to avoid overlapping uncommitted IME composition text, covering labeled inputs, the group field, and port-forward fields.
- **修复命令输入框全选与多行滚动（#416，输入框部分）。** `Ctrl+A` / `Cmd+A` 现在全选文本，支持滚动查看多行内容，并在选择时跟随光标。该更新不包含 `top` 无法通过 `Ctrl+C` 退出问题的修复。
- **Fix command-input select-all and multiline scrolling (input-field portion of #416).** `Ctrl+A` / `Cmd+A` now selects all text. Multiline content can scroll, and selection keeps the cursor visible. This update does not resolve the reported inability to exit `top` with `Ctrl+C`.
- **保留 Intel Mac 安装包。** 正式发布同时提供 Intel (`macos-x86_64`) 和 Apple Silicon (`macos-aarch64`) 安装包。
- **Keep Intel Mac packages.** Releases provide packages for both Intel (`macos-x86_64`) and Apple Silicon (`macos-aarch64`).

## [0.7.2] - 2026-09-04

- **修复旧版 macOS 启动闪退。** 在 macOS 上禁用 Slint/winit 的 AppKit DisplayLink 帧节流，回退到计时器帧节流，避免旧系统收到不存在的 `displayLinkWithTarget:selector:` 消息。
- **Fix startup crashes on older macOS.** Disable Slint/winit's AppKit DisplayLink frame throttling on macOS and use timer throttling instead, avoiding calls to the unavailable `displayLinkWithTarget:selector:` method on older systems.
- **支持终端鼠标追踪与 VT100 线框字符（#399、#400）。** 鼠标事件现在可转发给启用追踪的 TUI 程序，并正确解析 DEC Special Graphics 字符集；会话设置可按需关闭线框转换。
- **Support terminal mouse tracking and VT100 line drawing (#399, #400).** Mouse events are now forwarded to TUI applications that enable tracking, and the DEC Special Graphics character set is rendered correctly, with an option to disable line-drawing conversion per session.
- **修复多项 SFTP 文件操作问题（#401、#402、#403）。** 上传完成后会正确清空选择计数，调整工具栏拖拽手柄与路径输入框布局，并在批量归档、复制路径时安全处理符号链接，避免空文件、卡死或越界访问。
- **Fix several SFTP file-operation issues (#401, #402, #403).** Clear the selection count after uploads, improve the transfer toolbar and path-field layout, and handle symbolic links safely during batch archives and path copying to prevent empty files, hangs, and traversal outside the selected tree.
- **修复 Wayland 启动后出现多个任务栏窗口（#404）。** 调整 Linux 窗口初始化与资源信息展示，避免应用启动时创建多余的可见窗口。
- **Fix duplicate taskbar windows after startup on Wayland (#404).** Adjust Linux window initialization and resource presentation so startup no longer creates extra visible windows.
- **支持在多个标签栏之间拖动标签页（#408）。** 标签页现在可以跨分屏和多标签容器拖动，并在目标位置正确重排，方便整理复杂的终端工作区。
- **Support dragging tabs across tab bars (#408).** Tabs can now be moved between split panes and multi-tab containers and reordered at the destination, making complex terminal workspaces easier to organize.
- **修复右键重命名标签页时闪退（#411）。** 调整窗口菜单事件处理，避免从标签页上下文菜单执行重命名时发生重入崩溃。
- **Fix crashes when renaming tabs from the context menu (#411).** Adjust window-menu event handling to prevent re-entrant crashes when renaming a tab from its context menu.
- **优化终端内存管理与跨平台分配器（#410）。** 终端关闭后会及时释放缓存及关联状态，并按平台选择合适的全局内存分配器，降低长时间、多会话使用时的内存占用。
- **Improve terminal memory management and cross-platform allocation (#410).** Release terminal buffers and associated state promptly after tabs close, and select an appropriate global allocator per platform to reduce memory use during long-running, multi-session workloads.
- **修复通过 JumpServer/Koko 连接后 Shell 自动断开（#359）。** SSH 兼容模式现在只建立单一 PTY 连接，不再自动启动独立 SFTP 旁路；界面同步隐藏不可用的 SFTP 状态，避免堡垒机关闭被代理的 Shell。
- **Fix shells disconnecting through JumpServer/Koko (#359).** SSH compatibility mode now keeps a single PTY connection instead of automatically starting a separate SFTP side channel, and the UI reflects that SFTP is unavailable so bastions do not close the proxied shell.

## [0.7.2] - 2026-09-01

- **SFTP 子系统不可用时自动回退到 SCP 传输。** 当 SFTP 通道因服务器不支持或配置禁用而无法建立时，自动以 SCP 协议完成上传与下载，保证文件传输不中断。
- **Fall back to SCP when SFTP is unavailable.** When the SFTP subsystem cannot be started (server unsupported or disabled in config), uploads and downloads automatically use SCP so transfer always works.
- **SFTP 工具栏「下载」按钮始终提供「逐个下载 / 打包下载」菜单；下载完成后保留勾选状态。** 即便只勾选 1 项也保留该菜单，因为该文件可能是文件夹，需要打包下载。保留勾选便于对同批文件重复下载、删除或移动。
- **SFTP toolbar's Download button always offers the individual / archive menu; selection persists after downloads.** Even with a single row selected (it could be a folder that needs archiving), the menu stays available. Keeping the selection makes it easy to re-download, remove, or move the same batch.
- **打包下载统一为 `.zip` 并正确跟随符号链接。** 所有打包下载（含单文件右键）统一为 `.zip`；修复批量下载 `.pem` 等符号链接文件时退化为空/破损连接的缺陷（`tar -cf` 加上 `-h` 旗标跟随 symlink）。
- **Unify archive downloads to `.zip` and follow symlinks.** All archive downloads (including the single-file right-click) use `.zip`. Fixes batch-downloading symlinked files such as `.pem` becoming empty/broken links (`tar -cf` now passes `-h`).
- **删除确认框行为一致。** 勾选恰好 1 项时，右键删除与右上角删除按钮均弹出单文件确认框（与无勾选右键删除一致）；勾选 ≥2 项走批量确认框；有勾选时两者共用同一批量确认框。
- **Unify delete-confirmation dialogs.** With exactly one row selected, both the right-click menu and the toolbar's Delete button show the single-file dialog; two or more rows use the batch dialog; both entry points share the same batch dialog when rows are selected.
- **修复 SFTP 工具栏拖拽手柄与 Files 标签重叠、路径输入框过窄的问题。** 手柄不再悬浮粘连，路径输入框在中等宽度面板下保持可用宽度；紧凑工具栏与折叠态的布局、置灰及悬浮交互优化。
- **Fix the SFTP toolbar drag handle overlapping the Files label and the too-narrow path input.** The handle no longer sticks, the path input keeps a usable width at medium panel sizes, and the compact/collapsed toolbar layout, dimming, and hover behavior are improved.
- **视觉细节优化。** 列头分隔线常驻不闪；右键菜单位置随滚动贴住点击点；tooltip 渲染到按钮上方不再被遮挡；目录切换「加载中」改为淡入淡出。
- **Visual polish.** The column-header divider stays put instead of flickering; right-click menus stick to the click point while scrolling; tooltips render above buttons so they are no longer clipped; the directory-switching "loading" indicator now fades in and out.
- **修复点击「复制路径」时面板卡死的缺陷。** 剪贴板写入移至后台线程，避免阻塞 UI 事件循环。
- **Fix the panel freeze when clicking "Copy path".** The clipboard write now runs on a background thread so the UI event loop is never blocked.
- **修复上传文件后，工具栏选中计数与批量下载/删除按钮未随列表刷新同步清零的问题。** 计数在模型重建处统一重置，所有重建路径自动兜底。
- **Fix stale selection count after uploads.** The toolbar's selection count and the batch-download/delete buttons now reset in lockstep with the rebuilt file list, so every code path that rebuilds the model is covered.
- **修复 Wayland 下启动后任务栏出现三个窗口的问题。**
- **Fix three taskbar windows appearing on Wayland at startup.**
- **忽略构建/发布产物二进制。** `.gitignore` 加入发布 `.exe` 与验证产物路径，避免误提交。
- **Ignore build and release artifacts.** The `.gitignore` now excludes release `.exe` files and the verify package to keep them out of the repo.
- **同步上游 main 的 10 个提交。** 包括终端鼠标事件转发、VT100 线框绘制、Linux/macOS 若干修复与 CI 改进。
- **Sync 10 upstream commits from main.** Includes terminal mouse-event forwarding, VT100 line-drawing, several Linux/macOS fixes, and CI improvements.

## [0.7.1] - 2026-08-29

- **完善终端内 ZMODEM `rz` 上传（#308）。** 采用贡献者实现，补充完整关闭握手、远端跳过与握手失败反馈，并限制数据块大小以提升与 `lrzsz` 的兼容性；`sz` 下载保持不变。
- **Improve in-terminal ZMODEM `rz` uploads (#308).** Adopt the contributor implementation with a complete close handshake, remote-skip and handshake-failure feedback, plus capped data blocks for better `lrzsz` compatibility. Existing `sz` downloads remain unchanged.
- **优化文件传输工具栏与拖拽上传（#397）。** 改善传输列表工具栏、拖拽上传交互、缩放手柄和传输状态呼吸灯显示。
- **Polish the file-transfer toolbar and drag-and-drop uploads (#397).** Improve the transfer-list toolbar, drag-and-drop upload interaction, resize handle, and transfer-state breathing indicator.

## [0.7.0] - 2026-08-27

- **终端内 ZMODEM 支持 `rz` 上传（#308）。** 远端执行 `rz` 后会自动打开本地多文件选择器，通过当前 SSH PTY 上传所选文件，并在传输列表显示进度；取消选择、协议错误或远端取消时会发送标准取消序列，避免会话卡住。握手检测现在会区分 `sz` 的 `ZRQINIT` 与 `rz` 的 `ZRINIT`，已有 `sz` 下载保持不变。
- **Support in-terminal ZMODEM uploads with `rz` (#308).** Running `rz` remotely now opens a local multi-file picker, uploads the selected files over the current SSH PTY, and reports progress in the transfer list. Cancelling the picker, protocol failures, and remote aborts send the standard cancel sequence so the shell does not hang. Handshake detection now distinguishes `sz`'s `ZRQINIT` from `rz`'s `ZRINIT`, while existing `sz` downloads remain unchanged.
- **新增 Android Beta 安装包。** 发布流水线现在构建 ARM64 APK，以 `-beta.apk` 后缀作为工作流产物并自动附加到 GitHub Release。首个实验版提供密码认证、交互式 SSH Shell 和每次连接前的服务器 SHA-256 密钥指纹确认；桌面版的构建与功能保持不变。
- **Add an Android Beta package.** The release workflow now builds an ARM64 APK, stores it as a `-beta.apk` workflow artifact, and attaches it to tagged GitHub Releases. This first experimental build provides password authentication, an interactive SSH shell, and per-connection SHA-256 server-key confirmation; desktop builds and features remain unchanged.
- **修复 SSH 私钥选择器与多行粘贴键盘操作（#392、#393）。** 私钥选择器在所有平台均可显示 `id_rsa`、`id_ed25519` 等无扩展名文件；多行粘贴安全确认框可使用 Enter 确认，且不会将该快捷键扩展到删除或主机密钥等高风险弹窗。
- **Fix SSH key selection and multiline-paste keyboard handling (#392, #393).** The private-key picker now shows extensionless files such as `id_rsa` and `id_ed25519` on every platform. Multiline-paste confirmation accepts Enter without enabling keyboard confirmation for destructive delete or host-key prompts.

## [0.6.16] - 2026-08-24

- **新增 Debian 与 Flatpak 安装包。** 发布流水线现在为 Linux x86_64 和 ARM64 生成 `.deb`，并为 x86_64 生成可直接安装的 Flatpak bundle；这些安装包会作为工作流产物保存，并在标签发版时自动附加到 GitHub Release。现有 tar.gz 与 AppImage 保持不变。
- **Add Debian and Flatpak packages.** The release workflow now builds `.deb` packages for Linux x86_64 and ARM64 and an installable Flatpak bundle for x86_64. They are retained as workflow artifacts and automatically attached to tagged GitHub Releases, alongside the existing tar.gz and AppImage assets.
- **修复 Linux 安装包流水线并改善 Windows 占用升级。** DEB 构建现在为 `dpkg-shlibdeps` 提供所需的软件包元数据，避免 amd64 与 ARM64 任务以代码 25 退出；Flatpak 改由独立 job 在 Flathub 官方 25.08 容器中打包，不再依赖 Ubuntu 22.04 的旧工具链。Windows MSI 升级检测到 MeatShell 或无窗口 MCP 服务仍在运行时会提示用户关闭，必要时经用户确认后结束旧进程，避免 `meatshell.exe` 被占用而无法替换。
- **Fix Linux package CI and Windows in-use upgrades.** DEB builds now provide the package metadata required by `dpkg-shlibdeps`, preventing exit code 25 on amd64 and ARM64. Flatpak packaging now runs as a separate job in Flathub's official 25.08 container instead of relying on Ubuntu 22.04's older toolchain. During a Windows MSI upgrade, a running MeatShell or windowless MCP server now produces a close/retry prompt and can be terminated after explicit user confirmation so the installer can replace `meatshell.exe`.

## [0.6.15] - 2026-08-24

- **修复 Fedora 等非 Debian Linux 桌面按下 Ctrl 即触发终端快捷键的问题（#369）。** Linux 下 Slint/winit 可能把裸 Control 按键上报为 `U+0011` 或 `U+0016`；过滤范围现已从 Debian 系扩展到所有 Linux 发行版，避免 nano 在只按 Ctrl 时误触发搜索或其他操作。真正的 Ctrl+Q、Ctrl+V、Ctrl+X 等组合键仍由最终字母事件生成；Windows 与 macOS 保持各自独立的输入适配。
- **Fix bare Ctrl triggering terminal shortcuts on Fedora and other non-Debian Linux desktops (#369).** Slint/winit may report a physical Control press as `U+0011` or `U+0016` on Linux. Filtering now applies to every Linux distribution instead of only the Debian family, preventing nano and other terminal programs from reacting when Ctrl alone is pressed. Genuine Ctrl+Q, Ctrl+V, Ctrl+X, and similar chords still come from the final letter event, while Windows and macOS retain their separate input handling.
- **新增 MCP 与 CLI 自动化入口。** `meatshell mcp serve` 通过本机 stdio 提供会话查询、SSH 命令执行、SFTP 目录浏览、有界文本读取以及文件上传/下载，并在“设置 → 界面 → MCP”中独立控制服务、保存凭据、任意命令及文件传输权限；密码、私钥和代理凭据不会出现在协议响应中。`meatshell cli` 复用同一核心能力，提供人类可读输出及 `--json`。
- **Add MCP and CLI automation entry points.** `meatshell mcp serve` exposes session discovery, SSH command execution, SFTP directory listing, bounded text reads, and file uploads/downloads over local stdio, with separate service, saved-credential, arbitrary-command, and file-transfer controls under Settings → Interface → MCP. Passwords, private keys, and proxy credentials never appear in protocol responses. `meatshell cli` reuses the same core and supports both human-readable output and `--json`.


## [0.6.12] - 2026-08-16

- **降低键盘输入后的字符回显延迟。** 检测到真实按键发送后，对应会话会在短暂交互窗口内把终端刷新间隔从约 33 ms 降至约 8 ms，使本地 CMD、PowerShell、WSL 和低延迟 SSH 会话的逐字输入更跟手；停止输入后自动恢复原有日志流限速，滚屏阅读仍使用低频刷新。
- **Reduce character-echo latency after keyboard input.** After a real key is sent, the corresponding session temporarily lowers its terminal refresh interval from roughly 33 ms to 8 ms, making typing in local CMD, PowerShell, WSL, and low-latency SSH sessions feel more immediate. It automatically returns to the existing firehose throttle when typing stops, while scrolled-back views retain their lower refresh rate.
- **自动格式化并分类着色 JSON 输出（#350）。** 终端现在只对占据完整一行、能够正确解析的 JSON 对象或数组进行缩进展开，并分别着色键、字符串、数字、布尔值和 `null`；已有 ANSI SGR 颜色会先安全归一化，提示符、半截 JSON、标量以及包含光标控制的输出保持原样。可在“设置 → 输出高亮”中关闭。
- **Automatically format and syntax-colour JSON output (#350).** The terminal now pretty-prints only complete, whole-line JSON objects or arrays and assigns distinct colours to keys, strings, numbers, booleans, and `null`. Existing ANSI SGR colours are safely normalized first, while prompts, partial JSON, scalars, and cursor-controlled output remain untouched. The behavior can be disabled under Settings → Output Highlighting.
- **支持双击标签复制连接（#340）。** 双击任意终端会话标签会创建一条独立的新连接，与右键“复制连接”一致；欢迎页标签不会被复制，现有单击选择和拖动排序/分屏行为保持不变。
- **Support duplicating connections by double-clicking tabs (#340).** Double-clicking any terminal session tab now opens an independent duplicate connection, matching the context-menu action. The Welcome tab is excluded, and existing single-click selection plus drag reorder/split behavior remain unchanged.
- **在隐藏运行状态时暂停采样（#340）。** 收起运行状态面板或开启专注模式后，本地 `sysinfo` 采样和 SSH 远端资源/进程监控会暂停，重新展开时自动恢复，避免隐藏面板继续消耗本机与服务器资源。
- **Pause hidden status sampling (#340).** Collapsing the status panel or enabling Zen mode pauses local `sysinfo` sampling plus remote SSH resource/process monitoring, and expanding it resumes monitoring automatically so hidden UI no longer consumes local or server resources.
- **命令栏新增历史命令自动提示（#349）。** 输入命令时会自动筛选并向上展示曾执行过的匹配命令，可用方向键选择、Tab 补全、Esc 关闭；原有的历史按钮、`Ctrl+R` 搜索、复制、删除和直接运行功能保持不变，且无需修改远端 Shell 配置。
- **Add command-history autocomplete to the command bar (#349).** Typing now filters previously executed commands into an upward suggestion list with arrow-key selection, Tab completion, and Esc dismissal. Existing history-button and `Ctrl+R` search, copy, delete, and run actions remain available, with no remote shell configuration required.
- **快捷命令支持手动排序（#310）。** 管理列表中的每条命令新增上移/下移按钮，可在当前分组内调整并持久化顺序；不同分组不会被意外交叉，现有 GUI 编辑、分组和滚动管理方式保持不变。
- **Support manual quick-command ordering (#310).** Each command in the management list now has move-up and move-down controls that persist its order within the current group. Commands never cross group boundaries accidentally, and the existing GUI editing, grouping, and scrollable management remain intact.
- **完善快捷命令 GUI 管理（#310）。** 管理窗口现在可通过右下角拖动放大，并默认提供更宽、更高的工作区；命令编辑框支持多行内容，长列表仍可滚动，配合组内上移/下移即可在不编辑配置文件的情况下完成批量整理。
- **Complete the quick-command GUI manager (#310).** The manager can now be enlarged from its bottom-right resize grip and opens with a wider, taller workspace. Its command editor accepts multiline content, long lists remain scrollable, and in-group move controls allow bulk organization without editing configuration files.
- **修复右键内置 WSL/PowerShell/CMD 时显示空白菜单（#336）。** 内置会话没有编辑、移动或删除操作，因此不再打开所有项目均被隐藏的右键弹窗；普通保存会话的上下文菜单保持不变。
- **Fix the blank context menu on built-in WSL/PowerShell/CMD rows (#336).** Built-in sessions have no edit, move, or delete actions, so they no longer open a popup whose items are all hidden. Context menus for regular saved sessions are unchanged.
- **支持配置多个 WSL 启动项和默认目录（#336）。** Windows 设置新增 WSL 页面，可添加多个命名启动项、指定发行版，并通过目录选择器或直接粘贴路径设置启动目录；目录留空时统一回退到 `~`，因此默认进入所选 Linux 用户的主目录。旧配置继续显示原有的默认 WSL 入口。
- **Support multiple WSL entries and startup directories (#336).** Settings on Windows now include a WSL page for adding named launch entries, selecting a distribution, and choosing or pasting a startup directory. Blank directories consistently fall back to `~`, opening the selected Linux user's home, while existing configurations retain the original default WSL entry.
- **彻底修复切换“欢迎页设为侧栏”时闪退（#323）。** 设置开关不再在自身回调栈内修改双向绑定并同步销毁 Welcome/设置组件树；属性应用、配置保存和分屏刷新现在整体延迟到下一次界面事件循环，覆盖 0.6.11 中仍可复现的 Windows 闪退及错误配置导致的后续启动问题。
- **Fully fix crashes when toggling “Welcome page as sidebar” (#323).** The settings switch no longer changes its two-way binding and synchronously destroys the Welcome/settings component tree from inside its own callback. Property application, persistence, and pane refresh are now deferred together to the next UI turn, covering the Windows crash still reproducible in 0.6.11 and the resulting bad-startup state.
- **优化侧栏快速连接的服务器信息显示（#339）。** 窄侧栏中的会话行改为两行紧凑布局，首行显示名称，次行始终显示 `用户@主机:端口`，避免固定宽度列把 IP 地址裁出可视区域；完整欢迎页继续保留原来的分栏布局。
- **Improve server details in the Quick Connect sidebar (#339).** Session rows in the narrow sidebar now use a compact two-line layout with the name first and `user@host:port` always visible beneath it, preventing fixed-width columns from clipping the IP address. The full Welcome page retains its existing column layout.
- **SSH config 导入支持 `Include`（#341）。** 导入 `~/.ssh/config` 时会在指令原位置递归读取包含文件，支持 `~`、相对 `~/.ssh` 的路径和 glob，并通过循环检测与深度上限避免恶意或误配置的递归引用。
- **Support `Include` while importing SSH config (#341).** Importing `~/.ssh/config` now recursively expands included files at the directive position, supports `~`, paths relative to `~/.ssh`, and globs, and guards against cyclic or excessively deep include chains.
- **防止不同服务器的同名文件在外部编辑时互相覆盖（#318）。** 外部打开/编辑使用按连接隔离的临时目录，并在本地文件名中加入服务器地址；编辑监视器会把修改上传到原始远端完整路径，而不再从带前缀的临时文件名推导目标，因此同时编辑多台服务器上的 `nginx.conf` 不会共用文档或传错服务器。
- **Prevent same-named files from different servers colliding during external editing (#318).** External open/edit now uses a connection-isolated temporary directory and includes the server address in the local filename. The edit watcher uploads changes to the exact original remote path instead of deriving a target from the prefixed temp name, so concurrent `nginx.conf` edits across servers cannot share a document or upload to the wrong server.
- **修复 Windows 右键打开终端查找时闪退（#343）。** 从右键菜单创建查找栏时，输入框现在会等弹窗点击事件和首轮布局完成后再获取焦点，避免 Windows 的 IME/无障碍焦点处理重入 Slint 运行时；`Ctrl+F` 与右键“查找”仍会自动聚焦搜索框。
- **Fix the Windows crash when opening terminal Find from the context menu (#343).** When the find bar is created from the right-click menu, its input now waits for popup event dispatch and the first layout pass to finish before taking focus, avoiding re-entry into Slint through Windows IME/accessibility focus handling. Both `Ctrl+F` and right-click → Find still focus the search field automatically.
- **SSH 终端支持多字符集（#338）。** 会话高级设置新增字符集选择，可使用 UTF-8、GBK（兼容 GB2312）、Big5、Shift_JIS、EUC-KR 和 Windows-1252；远端输出与键盘输入/粘贴会在 SSH PTY 边界双向转码，并通过有状态解码正确处理跨网络包拆分的多字节字符。旧会话继续默认使用 UTF-8。
- **Support multiple character encodings in SSH terminals (#338).** Session advanced settings now offer UTF-8, GBK (including GB2312), Big5, Shift_JIS, EUC-KR, and Windows-1252. Remote output and keyboard/paste input are transcoded bidirectionally at the SSH PTY boundary, with stateful decoding for multibyte characters split across network packets. Existing sessions continue to default to UTF-8.
- **修复通过命令栏启动 `top`、`iftop` 等程序后 `q` 难以退出（#345）。** 命令框、快捷命令和历史记录的所有“立即执行”入口现在会在发送命令后把键盘焦点安全地交还终端，因此后续 `q`、方向键及其他交互按键会直接到达远端 TUI；仅填充但不执行命令时仍保留输入框焦点。
- **Fix `q` not exiting `top`, `iftop`, and similar programs launched from the command bar (#345).** Every immediate-execution path—command input, quick commands, and history—now safely returns keyboard focus to the terminal after sending, so `q`, arrows, and other interactive keys reach the remote TUI. Filling a command without executing it still keeps focus in the input field.
- **支持在设置中关闭多行粘贴确认（#346）。** 多行粘贴安全提示默认开启；用户可在“设置 → 界面 → 确认多行粘贴”中持久化关闭或重新开启。超过 100 KiB 的超大粘贴始终强制确认。
- **Allow disabling multiline-paste confirmation in Settings (#346).** The safety prompt remains enabled by default and can be persistently disabled or re-enabled under Settings → Interface → Confirm multiline paste. Pastes over 100 KiB always require review.
- **修复 macOS 使用 `Ctrl+Space` 切换输入法时误删字符（#348）。** 部分 macOS 输入法会把裸 Control 错误上报为 `U+0008`；该标记现在仅在 macOS 输入边界被过滤，不再作为 Backspace 发送到终端。Windows、Linux 及真正由最终字母事件生成的 Ctrl+H 不受影响。
- **Fix character deletion when switching macOS input methods with `Ctrl+Space` (#348).** Some macOS input methods report bare Control as `U+0008`; that marker is now filtered only at the macOS input boundary instead of being sent as Backspace. Windows, Linux, and genuine Ctrl+H generated from the final letter event remain unchanged.
- **彻底阻止 zsh 泄漏 shell integration 初始化命令（#344）。** 不再依赖回显文本形态、软换行或 16 KiB 容量上限来判断初始化结束；注入命令现在输出专用的不可见完成标记，客户端在收到标记前持续隐藏并滚动丢弃内部回显。zsh/ZLE 即使反复重绘长命令也不会再把 `test -z "$FISH_VERSION" ...` 放到终端首屏，同时缓冲内存保持有界。
- **Reliably prevent zsh shell-integration setup leaks (#344).** Setup completion no longer depends on echoed text shape, soft wrapping, or a 16 KiB threshold. The injected command now emits a private invisible completion marker, while the client hides and rolls over internal echo until that marker arrives. Repeated zsh/ZLE redraws can no longer expose `test -z "$FISH_VERSION" ...`, and buffering remains bounded.
- **修复 macOS 无法在文件选择器中选择无扩展名 OpenSSH 私钥的问题（#325）。** macOS 的私钥浏览器不再按扩展名过滤，因此可直接选择 `~/.ssh/id_ed25519`、`id_rsa` 等标准无扩展名私钥；Windows 和 Linux 保持原有文件过滤行为。
- **Fix selecting extensionless OpenSSH private keys on macOS (#325).** The macOS private-key picker no longer filters by extension, allowing standard files such as `~/.ssh/id_ed25519` and `id_rsa` to be selected directly. Windows and Linux retain their existing file filters.
- **完善 macOS `Ctrl+X` 修复（#312）。** 真机日志确认部分 macOS 26.5 设备会在物理 Control 按住期间连续产生 `U+0017`（Ctrl+W）裸标记，导致 nano 在收到真正的 Ctrl+X 前先打开搜索。现在会一并过滤该设备相关标记，同时仍由组合键最后的字母事件生成真正的 Ctrl+W/Ctrl+X 控制码。
- **Complete the macOS `Ctrl+X` fix (#312).** On-device logs confirmed that some macOS 26.5 systems repeatedly emit a bare `U+0017` (Ctrl+W) marker while physical Control is held, making nano open search before the real Ctrl+X arrives. That device-specific marker is now filtered while the chord's final letter event continues to generate genuine Ctrl+W/Ctrl+X bytes.
- **macOS 新增 CPU 渲染并设为默认。** “设置 → 界面 → 渲染”现在可选择 CPU、FemtoVG 和 Skia；新安装及未保存渲染器偏好的配置默认使用 CPU 渲染，已有的 FemtoVG/Skia 明确选择保持不变。设置在重启 MeatShell 后生效，`SLINT_BACKEND` 环境变量仍具有最高优先级。
- **Add CPU rendering on macOS and make it the default.** Settings → Interface → Rendering now offers CPU, FemtoVG, and Skia. New installations and configurations without a saved renderer preference default to CPU rendering, while explicit existing FemtoVG/Skia choices are preserved. Changes apply after restarting MeatShell, and `SLINT_BACKEND` keeps the highest priority.

## [0.6.10] - 2026-08-05

- **修复关闭“欢迎页设为侧栏”时闪退（#323）。** 欢迎页在侧栏与标签页之间切换时，分屏模型现在会延迟到下一次界面事件循环再刷新，并跳过尺寸和内容均未变化的重复更新，避免 Windows 下递归重建界面导致当前进程及后续启动闪退。
- **Fix crashes when disabling “Welcome page as sidebar” (#323).** Switching the welcome page between sidebar and tab mode now defers pane-model rebuilding to the next UI event-loop turn and skips unchanged model updates, preventing recursive UI reconstruction on Windows during the toggle and subsequent launches.
- **修复快速连接中重复的 system 分组和空白右键菜单（#316、#324）。** 内置 `system` 与隐式 `default` 现在统一视为保留分组，不再出现在服务器的“移动到”目标中，也无法通过新建、重命名、编辑或导入写入；新建或重命名为已有分组时会保留弹窗并提示“分组已存在”。升级时会自动把旧版本误放进 `system` 的服务器迁回 `default`。内置终端行改用显式标记识别，包括空密码 SSH 会话在内的普通服务器不会再因组名碰撞而隐藏右键操作。
- **Fix duplicate system groups and empty context menus in Quick Connect (#316, #324).** The built-in `system` and implicit `default` groups are now reserved across move, create, rename, edit, and import paths. Creating or renaming to an existing group keeps the dialog open and reports that the group already exists. Existing servers accidentally filed under `system` are migrated back to `default`, while explicit built-in-row markers prevent ordinary servers, including passwordless SSH sessions, from losing their context-menu actions due to a group-name collision.
- **回复终端状态与设备属性查询（#328）。** 远端程序发送 DSR 状态/光标位置查询或 DA1 主设备属性查询时，MeatShell 现在会立即向 PTY 返回对应的状态、CPR 光标坐标或保守的 VT100 能力标识；查询序列即使被拆分到多个 SSH 输出块也能正确识别，依赖终端握手的交互式程序不再等待超时或卡死。
- **Respond to terminal status and device-attribute queries (#328).** MeatShell now immediately returns status, CPR cursor coordinates, or a conservative VT100 identity when remote programs issue DSR or primary DA1 queries. Split query sequences are recognized across SSH output chunks, preventing interactive applications that rely on terminal handshakes from timing out or hanging.
- **修复内置编辑器打开大文件时崩溃，并调整历史命令排序（#331）。** 内置查看/编辑现在采用有界读取，并在文件超过 512 KB、行数过多或存在超长单行时安全拒绝并引导使用外部打开/编辑；历史命令弹窗改为从新到旧显示，同时保留输入框 ↑/↓ 的原有回溯顺序。
- **Prevent large-file editor crashes and reorder command history (#331).** Built-in viewing/editing now uses bounded reads and safely redirects files over 512 KB, excessive line counts, or exceptionally long lines to external tools. The history popup now lists newest commands first while preserving the input field's existing ↑/↓ recall order.
- **修复 zsh 中 Home 和 End 按键无效（#329）。** 远端 shell 启用应用光标模式时，Home/End 现在会像方向键一样改用对应的 SS3 控制序列，恢复 oh-my-zsh/ZLE 中的行首和行尾移动。
- **Fix Home and End keys in zsh (#329).** When the remote shell enables application cursor mode, Home/End now use their corresponding SS3 sequences like the arrow keys, restoring beginning/end-of-line movement in oh-my-zsh/ZLE.
- **Linux 支持选择界面渲染器（#330）。** “设置 → 界面 → 渲染”现在可选择自动、GPU 与软件模式，默认继续使用 Slint 自动选择；设置在重启 MeatShell 后生效，`SLINT_BACKEND` 环境变量仍具有最高优先级。
- **Select the UI renderer on Linux (#330).** Settings → Interface → Rendering now offers Automatic, GPU, and Software modes while retaining Slint's automatic selection by default. Changes apply after restarting MeatShell, and `SLINT_BACKEND` keeps the highest priority.

## [0.6.9] - 2026-07-31

### OpenWrt SSH shell integration fix / OpenWrt SSH shell 集成修复

- **修复 OpenWrt SSH 登录时泄露并卡在 shell 集成初始化命令的问题（#314、#317）。** 连接现在先通过独立的非交互 SSH 通道识别远端 shell，只向真正支持该集成的 Bash/Zsh 会话发送提示符钩子；BusyBox ash、fish 与未知 shell 不再收到 `test -z \"$FISH_VERSION\" ...` 长命令。
- **Prevent shell-integration setup from leaking or hanging OpenWrt SSH sessions (#314, #317).** MeatShell now identifies the remote shell through a separate non-interactive SSH channel and sends the prompt hook only to Bash/Zsh; BusyBox ash, fish, and unknown shells no longer receive the long `test -z \"$FISH_VERSION\" ...` command.

### 修复 / Fixed

- **修复终端粘贴、命令框、光标、选区与回滚历史问题（#319）。** 括号粘贴现在也会把 Windows CRLF 规范化为单个终端换行，命令框保留多行 heredoc 的原始换行；Vim/nano 的竖线光标不再右移，事件积压时真实 Backspace 不再被实时键状态误判，按键时会恢复光标可见；普通单击不再复制字符，`ESC[3J` 同时清除 MeatShell 自己维护的回滚与重放缓存。
- **修复 macOS 下 `Ctrl+X` 在 nano 中错误打开搜索的问题（#312）。** Slint 在 macOS 上会先把单独按下的物理 Control 键报告为控制键标记，再发送组合键字符；程序现在会在平台事件边界过滤该标记，只把随后的真实 `Ctrl+X` 控制字符发送到 PTY，同时保持 Command 应用快捷键与其他平台行为不变。
- **改善超大终端输出的渐进显示与响应性（#311）。** 持续输出现在按累计字节预算、在完整输出块边界提交 UI 快照，并用可等待的请求代次消除丢失通知和重复渲染；当事件积压过大或夹有连接状态事件时会优先追赶队列，避免节奏等待造成内存增长或延迟 `Connected` / `Closed`。隐藏标签、标签关闭及事件循环退出均不会让输出泵固定空等。

---

### Fixed

- **Fix terminal paste, command bar, cursor, selection, and scrollback behavior (#319).** Bracketed paste now normalizes Windows CRLF to one terminal newline, while the command bar preserves multiline heredocs. Vim/nano bar cursors no longer shift right, genuine Backspace events survive delayed dispatch, keyboard input restores cursor visibility, plain clicks no longer copy a character, and `ESC[3J` clears MeatShell's own scrollback and replay buffers.
- **Fix `Ctrl+X` opening search instead of exiting nano on macOS (#312).** Slint reports a standalone physical Control press as a modifier marker on macOS before delivering the chord character. MeatShell now filters that marker at the platform event boundary and forwards only the real `Ctrl+X` control character to the PTY, without changing Command shortcuts or other platforms.
- **Improve progressive rendering and responsiveness under very large terminal output (#311).** Sustained output now commits UI snapshots at complete output-chunk boundaries after a cumulative byte budget, while generation-based wait tickets eliminate lost notifications and redundant renders. A large event backlog or pending connection-state event switches to catch-up mode so pacing cannot inflate memory use or delay `Connected` / `Closed`; hidden tabs, tab closure, and event-loop shutdown no longer cause repeated timeout waits.

### 改进 / Changed

- **快速连接分组默认收起并记住展开状态。** 首次启动时快速连接中的系统与会话目录保持收起；用户展开或收起目录后会立即保存该状态，刷新会话列表及重启应用后仍保持原样。

### 性能 / Performance

- **提升终端鼠标拖选文字的响应速度。** 拖动选区时只刷新轻量选区图层，不再为每次鼠标移动重新生成整块终端文本与样式，长回滚记录下也能即时显示选中内容。

---

### Changed

- **Default Quick Connect groups to collapsed and remember their state.** System and session folders start collapsed, while later expand/collapse choices are saved immediately and survive session-list refreshes and application restarts.

### Performance

- **Improve terminal text-selection responsiveness.** Dragging now refreshes only the lightweight selection overlay instead of rebuilding all terminal text and styling for every mouse movement, keeping selection immediate with long scrollback histories.

## [0.6.8] - 2026-07-26

### 改进 / Changed

- **Windows MSI 会沿用已有安装目录并创建桌面快捷方式（#293）。** 安装程序会优先读取 MeatShell 记录的目录，并可通过旧版主程序组件位置迁移 v0.6.5-v0.6.7 的自定义安装路径；全新安装仍默认使用 `Program Files`。MSI 安装时还会在当前用户桌面创建 MeatShell 快捷方式，卸载时一并移除。

### 修复 / Fixed

- **修复内置壁纸覆盖已保存浅色/深色主题的问题。** 启动时恢复内置壁纸不再根据壁纸亮度强制切换主题，用户选择的浅色或深色模式会在重启后保留；仅在用户主动选择内置壁纸时应用其推荐主题。
- **修复新建 Telnet 会话的默认端口（#303）。** 从 SSH 或串口切换到 Telnet 时，端口现在会从 SSH 默认值 `22` 自动调整为 Telnet 标准端口 `23`；切回 SSH 时恢复为 `22`，非默认端口保持不变。
- **修复 AUR 发布工作流校验失败。** 发布步骤现在通过作业环境变量判断所需的 AUR 仓库 Secret 是否完整配置，避免在 `if` 条件中直接引用不受支持的 `secrets` 上下文而导致工作流无法运行。
- **修复手动产物构建的版本校验。** 从分支手动运行 Release 工作流时改为根据 `Cargo.toml` 校验二进制版本，不再把分支名误当成版本号；手动构建仍只上传工作流产物，不创建 GitHub Release。
- **修复 Windows 使用 `Ctrl+Space` 切换输入法后 Ctrl 状态残留（#309）。** 当微软输入法把 Ctrl 松开标记为 `VK_PROCESSKEY` 时，程序现在会根据事件保留的左右 Ctrl 物理键信息向 Slint 补齐对应的松开事件；正常终端 Ctrl 快捷键及其他操作系统不受影响。

---

### Changed

- **Preserve the existing Windows MSI install location and add a desktop shortcut (#293).** Setup now prefers MeatShell's recorded directory and can migrate custom v0.6.5-v0.6.7 locations from the legacy executable component; clean installs still default to Program Files. MSI installs also create a MeatShell shortcut on the current user's desktop and remove it during uninstall.

### Fixed

- **Preserve the saved light/dark theme when restoring a built-in wallpaper.** Startup no longer forces a theme from the built-in wallpaper's luminance, so the user's light or dark preference survives a restart. The recommended paired theme is applied only when the user actively selects a built-in wallpaper.
- **Use the standard Telnet port for new sessions (#303).** Switching from SSH or Serial to Telnet now changes the SSH default `22` to the standard Telnet port `23`; switching back to SSH restores `22`, while non-default ports are preserved.
- **Fix AUR publishing workflow validation.** The publishing step now checks the required AUR repository secrets through job environment variables, avoiding the unsupported direct use of the `secrets` context in an `if` condition that prevented the workflow from running.
- **Fix version verification for manually dispatched artifact builds.** Release workflow runs started from a branch now verify the binary against the package version in `Cargo.toml` instead of treating the branch name as a version. Manual builds continue to upload workflow artifacts without creating a GitHub Release.
- **Fix the Ctrl modifier remaining active after switching the Windows IME with `Ctrl+Space` (#309).** When Microsoft IME labels a Ctrl release as `VK_PROCESSKEY`, the application now uses the retained left/right physical Ctrl identity to deliver the matching release to Slint. Normal terminal Ctrl shortcuts and other operating systems are unaffected.

### 新增 / Added

- **macOS 支持在设置中选择界面渲染器。** “设置 → 界面 → 渲染”现在在 macOS 上提供 FemtoVG 和 Skia 两种后端，遇到文字缺失或显示异常时可以直接切换，重启 MeatShell 后生效；`SLINT_BACKEND` 环境变量仍优先于界面设置。
- **支持使用快捷键循环切换标签页（#294）。** `Ctrl+Tab` 切换到当前分栏的下一个标签页，`Ctrl+Shift+Tab` 切换到上一个，并在首尾循环；macOS 使用物理 Control 键。快捷键面板新增“标签页”分组并列出两项操作。

### 修复 / Fixed

- **修复 SSH 内部初始化命令污染远端历史（#289）。** Shell integration 初始化完成后会主动清理当前初始化项及旧版本残留，不再依赖远端是否启用 `HISTCONTROL=ignorespace`；迟到回显过滤也只在连接初始化阶段生效，切换标签页后按上键不会再召回内部命令、清空终端行或破坏首屏内容。
- **修复 SSH 会话中 Bash 历史命令重绘错位（#289）。** 隐藏 shell integration 初始化命令后会主动复位并清空当前终端行，使本地 VT 光标与远端 PTY 重新同步；在 Debian 等桌面系统中使用上下方向键浏览历史时，提示符和命令不再相互重叠或依次拼接。

### 性能 / Performance

- **优化大容量终端回滚历史（#290）。** 终端历史缓冲改用双端队列；超过 100,000 行上限时从队首逐行回收，不再通过 `Vec::drain` 搬移全部剩余记录，持续输出大量内容时的裁剪开销更加稳定。

---

### Added

- **Select the UI renderer from Settings on macOS.** Settings → Interface → Rendering now offers the FemtoVG and Skia backends on macOS, allowing users to switch when text is missing or rendered incorrectly. Changes apply after restarting MeatShell, and `SLINT_BACKEND` continues to override the saved setting.
- **Add keyboard shortcuts for cycling tabs (#294).** `Ctrl+Tab` selects the next tab in the focused pane, while `Ctrl+Shift+Tab` selects the previous one, wrapping at both ends; macOS uses the physical Control key. The shortcuts panel now includes both actions in a dedicated Tabs section.

### Fixed

- **Prevent SSH shell setup from polluting remote history (#289).** Shell integration now removes both its current initialization entry and leftovers from older versions from Bash history instead of relying on the remote `HISTCONTROL=ignorespace` setting. Late-echo filtering is limited to connection setup, so pressing Up after switching tabs no longer recalls internal commands, clears terminal rows, or damages the initial screen.
- **Fix misaligned Bash history repainting in SSH sessions (#289).** After hiding the shell-integration setup command, MeatShell now resets and clears the current terminal row to resynchronize the local VT cursor with the remote PTY. Browsing history with the arrow keys on Debian and other desktops no longer overlaps the prompt or appends recalled commands beside one another.

### Performance

- **Optimize large terminal scrollback histories (#290).** The terminal history buffer now uses a double-ended queue. Once the 100,000-line cap is reached, old rows are reclaimed from the front without shifting every retained row through `Vec::drain`, keeping pruning costs stable during sustained high-volume output.

## [0.6.7] - 2026-07-25

### 新增 / Added

- **Windows 支持选择界面渲染器（#280）。** “设置 → 界面 → 渲染”新增自动、GPU 与软件三种模式；自动模式直接使用 Slint 的渲染器初始化与软件回退机制，不额外扫描显卡或启动检测窗口。现有安装继续默认使用软件渲染以保留高分屏、虚拟机和远程桌面的兼容性，设置将在下次启动时生效，`SLINT_BACKEND` 环境变量仍可用于诊断覆盖。

### 改进 / Changed

- **按业务功能重组源码目录。** 除 `main.rs` 与 `app.rs` 外，配置、终端、SSH、SFTP、布局、资源监控等 Rust 模块均迁入 `src/<功能>/` 业务包；数据结构放入 `struct/`，实现放入 `impls/`，并由各包的 `mod.rs` 提供统一入口，减少根目录文件和跨模块维护成本。

---

### Added

- **Add selectable UI renderers on Windows (#280).** Settings → Interface → Rendering now offers Automatic, GPU, and Software modes. Automatic relies directly on Slint's renderer initialization and software fallback without scanning the GPU or opening a probe window. Existing installations keep software rendering as the compatibility default for high-DPI displays, virtual machines, and remote desktops. Changes apply on the next launch, while `SLINT_BACKEND` remains available as a diagnostic override.

### Changed

- **Reorganize source files into feature-oriented packages.** Rust modules other than `main.rs` and `app.rs` now live under `src/<feature>/`, covering configuration, terminal, SSH, SFTP, layout, resource monitoring, and other features. Data structures live in `struct/`, implementations in `impls/`, and each package exposes a focused `mod.rs` facade.

## [0.6.6] - 2026-07-23

### 新增 / Added

- **快捷指令支持主窗口四向停靠（#215）。** “设置 → 界面 → 侧栏”新增“快捷命令作为侧栏”开关；开启后，现有快捷指令弹窗底部“管理”旁才显示拖拽把手，可将快捷指令拖到与快速连接、资源面板相同的主窗口停靠层，并在左、右、上、下边缘显示吸附高亮。成功停靠后，命令栏里的原快捷入口会隐藏，只保留侧栏或收起后的单个闪电图标；同边已有收起工具条时，闪电、快速连接与资源图标会合并到同一列（上/下停靠时为同一行），不再各占一列。只有该边没有其他收起面板时才单独创建工具条。同一边缘只允许一个面板展开，面板方向、尺寸及收起状态会持久保存。

### 修复 / Fixed

- **改进侧边停靠的 SFTP 面板窄宽布局。** 提高面板最小宽度，提前切换双行紧凑工具栏，并将次要路径操作收纳到“更多”菜单；路径输入框在独立行中保留稳定间距，避免拖窄时控件压缩、重叠或错位。
- **修复 KDE Wayland 下窗口恢复后的点击偏移（#286）。** Wayland 现在采用合成器实际配置的启动尺寸，不再调用仅具建议性质的窗口尺寸恢复请求，避免渲染位置与点击坐标不一致。
- **简化会话端口转发规则编辑（#277）。** 端口转发改为可直接保存的多行表单：“添加”只新增一组输入框，所有已填写规则会在保存时统一校验并持久化，不再同时显示“待添加表单”和“已添加摘要”两套界面。
- **修复 Windows 输入法切换后输入框失效（#236）。** 反复切换中英文输入法后，用户名、密码及其他输入框现在仍可继续输入。

---

### Added

- **Dock quick commands on any main-window edge (#215).** Interface → Sidebars now includes a “Quick commands as a sidebar” switch. Once enabled, a drag handle appears beside “Manage” in the existing popup and can dock the commands at the same main-window layer as Quick Connect and Resources, with snap highlighting on every edge. After docking, the original command-bar entry is hidden and only the sidebar or its single collapsed lightning icon remains. On an edge with an existing collapsed tool strip, the Quick Commands, Quick Connect, and Resources icons share one column (or one row for top/bottom docks) instead of reserving separate strips; a standalone strip is created only when needed. Only one panel may stay expanded on an edge, and the dock edge, size, and collapsed state persist.

### Fixed

- **Improve the narrow layout of side-docked SFTP panels.** Increase the panel minimum width, switch to the compact two-row toolbar earlier, and move secondary path actions into the More menu. The path input keeps stable spacing on its own row, preventing controls from being squeezed, overlapped, or misaligned when the panel is narrowed.
- **Fix click offsets after restoring a window on KDE Wayland (#286).** Wayland now uses the startup size actually configured by the compositor instead of requesting restoration through advisory-only window sizing, keeping rendered content aligned with pointer coordinates.
- **Simplify session port-forward rule editing (#277).** Port forwards now use a directly savable multi-row form. Add only creates another input row, and every completed rule is validated and persisted together on save instead of showing separate pending and added-rule interfaces.
- **Fix text fields becoming unusable after switching IMEs on Windows (#236).** Username, password, and other text fields continue accepting input after repeatedly switching between Chinese and English input methods.

## [0.6.5] - 2026-07-17

### 新增 / Added

- **支持自定义终端输入光标（#275）。** 设置 → 字体新增块状、竖线和下划线三种光标形状；点击颜色预览可从弹出色板选择常用颜色，也可通过十六进制 RGB 输入精确自定义。选择会即时应用于所有已打开及新建终端并持久保存；Vim 等全屏编辑器中的竖线光标会显示在当前字符右边缘，避免行尾双宽 emoji 遮住光标。隐藏的 IME 输入锚点不再额外绘制白色系统插入线，终端中只保留一个可见光标。既有配置默认继续使用块状光标和主题前景色。

### 修复 / Fixed

- **修复 SFTP 左右停靠时工具栏越界（#285）。** 侧边 SFTP 面板增加与紧凑工具栏匹配的最小宽度，窄宽度下文件与隧道标签切换为图标并隐藏次要批量操作，面板内容同时启用裁剪，不再覆盖相邻 terminal。
- **缩短 SSH 会话终端首屏等待时间。** SSH 认证和 PTY 建立后会立即读取并显示首个终端输出，不再同步等待 shell integration、SFTP、资源监控和进程监控；SFTP 在终端就绪后启动，轻量资源采样、进程列表与一次性详细系统信息再按优先级分阶段后台加载。新增 `[SESSION_START]` 分阶段耗时日志，便于区分网络握手、认证、PTY 和首屏输出耗时。
- **修复跨平台多行文本粘贴格式错位（#284）。** 终端现在会跟随远端 shell、编辑器或复用器请求的括号粘贴模式，将剪贴板内容作为单个受保护的数据块发送，从而保留 Windows 到 Linux 粘贴时的换行、缩进和多行布局；未启用该模式的程序仍会把 CRLF/LF 统一转换为终端回车。
- **优化当前标签的高亮样式（#283）。** 移除标签内部突兀的顶部横条，改用主题强调色光条完整包裹当前标签，在壁纸和不同明暗主题下保持清晰可辨。
- **修复“测试连接”未验证 SSH 凭据的问题（#276）。** SSH 测试现在复用正式终端连接的握手与认证流程，实际校验密码、keyboard-interactive、私钥及口令，并遵循代理、跳板机和主机密钥验证；编辑连接时留空的密码会继续使用已保存凭据。新增包含空格、符号和中文的密码加密落盘回归测试，避免端口可达被误报为登录成功。

---

### Added

- **Add customizable terminal insertion cursors (#275).** Settings → Font now offers block, bar, and underline cursor shapes. Clicking the color preview opens a palette of common colors, while the hexadecimal RGB field supports precise custom values. Changes apply immediately to every open and new terminal and persist across launches. In full-screen editors such as Vim, a bar cursor is placed at the current cell's trailing edge so a double-width emoji cannot obscure the end-of-line caret. The hidden IME input anchor no longer paints an extra white system caret, leaving exactly one visible terminal cursor. Existing configurations keep the block cursor and theme foreground color by default.

### Fixed

- **Fix SFTP toolbar overflow when docked left or right (#285).** Side-docked SFTP panels now enforce a minimum width matching the compact toolbar. At narrow widths, Files and Tunnels switch to icons and secondary batch actions are hidden, while panel clipping prevents any content from covering the adjacent terminal.
- **Reduce SSH terminal time-to-first-frame.** The first PTY output is now read and displayed immediately after SSH authentication and terminal creation instead of synchronously waiting for shell integration, SFTP, resource monitoring, and process monitoring. SFTP starts after the terminal is ready, followed by staged background loading for lightweight resources, processes, and one-shot detailed system information. New `[SESSION_START]` stage timings distinguish transport, authentication, PTY, and first-output latency.
- **Fix cross-platform multi-line paste formatting (#284).** The terminal now honors bracketed-paste mode requested by the remote shell, editor, or multiplexer and sends clipboard contents as one protected payload, preserving line endings, indentation, and multi-line layout when pasting from Windows to Linux. Applications without bracketed-paste support keep the existing CRLF/LF-to-terminal-return normalization.
- **Improve active-tab highlighting (#283).** The distracting inset top bar is replaced with a complete accent-colour outline around the active tab, keeping it identifiable across wallpapers and light or dark themes.
- **Fix connection tests that did not validate SSH credentials (#276).** SSH tests now reuse the real terminal handshake and authentication flow, validating passwords, keyboard-interactive authentication, private keys and passphrases while honoring proxies, jump hosts, and host-key verification. Blank password fields while editing reuse saved credentials, and a persistence regression test covers passwords containing spaces, symbols, and Chinese text so an open port is no longer reported as a successful login.

## [0.6.4] - 2026-07-17

### 新增 / Added

- **原生支持 PuTTY / MobaXterm 的 PPK 私钥（#281）。** SSH、SFTP 与跳板连接现在可直接加载 PPKv2 和 PPKv3 文件，支持 RSA、Ed25519 及 NIST ECDSA 密钥，并支持 PPKv2 的 SHA-1 派生及 PPKv3 的 Argon2 加密私钥。PPK 会先在内存中完成 AES-CBC 解密和 HMAC 完整性校验，再转换为 russh 使用的密钥对象；不会调用外部 `puttygen`，也不会将转换后的私钥写入临时文件。私钥选择器同时加入 `.ppk` 类型，粘贴的 PPK 内容也可自动识别。

### 修复 / Fixed

- **修复主窗口无法记住上次调整大小的问题（#278）。** 配置中的窗口尺寸会由首个原生 `Resized` 事件驱动恢复；若 Slint 的默认 `1440×900` 初始化随后覆盖请求，后续尺寸事件会继续重新应用保存值，直到原生窗口实际达到目标尺寸后才允许写回配置，不再依赖固定延迟或机器启动速度。窗口尚未归属具体显示器时会回退到主显示器，最大化、最小化及安装更新期间的临时尺寸不会覆盖用户选择；恢复过程会写入 `[WINDOW_SIZE]` 诊断日志。
- **修复 Windows 运行期间无法通过安装程序更新的问题（#267）。** 用户确认退出后，MeatShell 会记录关闭状态、隐藏主窗口及附属窗口、主动关闭 SSH/本地终端/SFTP 工作线程并清空活动会话，然后退出事件循环。Windows Installer 或 Restart Manager 随后重复发送的关闭请求会直接放行，不再反复弹出确认框或进入无法关闭的状态；确认按钮连续点击也只执行一次退出流程。
- **修复 Debian 桌面端 nano 中 `Ctrl+X` 错误进入搜索的问题（#274）。** Slint 在 Debian 及其衍生桌面环境按下 Ctrl 时可能先产生 `U+0011` 或 `U+0016` 裸修饰键标记；MeatShell 会依据客户端本机 `/etc/os-release` 仅在 Debian 系发行版精准过滤这些标记，避免 nano 先收到 `Ctrl+Q`。真正的 `Ctrl+X`、`Ctrl+R` 等控制组合仍按原字节发送，Windows、macOS 及其他 Linux 发行版保持原逻辑。
- **修复超长多行粘贴无法确认的问题（#271）。** 短内容继续使用紧凑确认框；超过 600 个字符或 12 行的 AI 提示词及其他长文本会自动切换为最大 `1160×760`、受主窗口边界约束的滚动审阅界面，完整显示粘贴内容，并将取消/粘贴按钮固定在底部。关闭应用时会临时隐藏粘贴审阅框，确保退出确认始终位于最上层；取消退出后，未发送的粘贴内容会原样恢复。

---

### Added

- **Add native PuTTY / MobaXterm PPK private-key support (#281).** SSH, SFTP, and jump-host connections can now load PPKv2 and PPKv3 files directly, including RSA, Ed25519, and NIST ECDSA keys, with encrypted keys supported through the PPKv2 SHA-1 derivation scheme and PPKv3 Argon2. PPK data is decrypted with AES-CBC, authenticated with HMAC, and converted to russh key objects entirely in memory. MeatShell neither invokes an external `puttygen` nor writes converted private keys to temporary files. The key picker includes `.ppk`, and pasted PPK content is detected automatically.

### Fixed

- **Fix the main window forgetting its last adjusted size (#278).** The first native `Resized` event now drives restoration from configuration. If Slint's default `1440×900` initialization subsequently overrides the request, later resize events keep reapplying the saved dimensions until the native window actually reaches its target; only then can resize events write configuration. This removes fixed-delay and machine-speed assumptions. Startup falls back to the primary monitor when necessary, transient maximize, minimize, and installer geometry is ignored, and restoration emits `[WINDOW_SIZE]` diagnostics.
- **Fix updates getting stuck while MeatShell is running on Windows (#267).** After the user confirms exit, MeatShell records the shutdown state, hides the main and detached windows, actively closes SSH/local-terminal/SFTP workers, clears live sessions, and then exits the event loop. Repeated close requests from Windows Installer or Restart Manager pass through without reopening the prompt or leaving the application uncloseable, and repeated clicks execute shutdown only once.
- **Fix `Ctrl+X` opening search instead of exiting nano on Debian desktops (#274).** Slint may emit a bare `U+0011` or `U+0016` modifier marker when Ctrl is pressed on Debian and derivative desktops. MeatShell now consults the client's local `/etc/os-release` and filters those exact markers only for Debian-family distributions, preventing nano from receiving an unintended `Ctrl+Q`. Real `Ctrl+X`, `Ctrl+R`, and other control combinations keep their original bytes, while Windows, macOS, and other Linux distributions retain their previous behaviour.
- **Fix unconfirmable long multi-line pastes (#271).** Short content keeps the compact confirmation card, while AI prompts and other text exceeding 600 characters or 12 lines automatically use a scrollable review surface up to `1160×760`, constrained to the main window. The complete paste remains reviewable and the Cancel/Paste actions stay fixed at the bottom. Requesting application exit temporarily hides paste review so the quit confirmation is always topmost; cancelling exit restores the unsent paste unchanged.

## [0.6.3] - 2026-07-16

### 新增 / Added

- **终端支持彩色 emoji（#269）。** 终端输出会按完整 Unicode grapheme 识别普通 emoji、肤色修饰、旗帜和 ZWJ 家庭/职业组合，并以缓存的 Twemoji 彩色图像替代 Slint software/femtovg 渲染器产生的单色字体轮廓；图像仍严格占用原终端单元格，普通文字、ANSI 样式、CJK、选择与光标定位保持不变。
- **进程监视支持居中打开、复制 PID 与安全结束进程。** 进程窗口每次打开时会在主窗口当前所在屏幕居中，并保留用户调整后的窗口尺寸。窗口中的 PID 现在可点击复制；右键进程行可发起结束操作，菜单在右键松开后才显示，避免同一次鼠标事件跳过确认。非 root 登录者结束本人进程时会先二次确认，结束 root 或其他用户的进程则必须输入当前登录用户的管理员（sudo）密码；登录用户本身为 root 时无需再次输入密码。操作通过独立 SSH exec 通道执行，不写入交互终端或命令历史；密码输入关闭回显并使用可清零密文承载，执行前还会重新校验来源会话、PID 与属主，避免切换标签后误操作。
- **纯文本日志级别自动高亮。** 终端会为未携带 ANSI 颜色的 `TRACE`、`DEBUG`、`INFO`、`NOTICE`、`WARN`、`WARNING`、`ERROR`、`FATAL`、`CRITICAL` 和 `PANIC` 级别标记增加主题自适应颜色；结构化日志中的 `level=error` / JSON level 字段也受支持。远端程序已有的 ANSI 样式以及 vim、nano、htop 等全屏 TUI 保持不变。
- **新增输出高亮设置与 DevOps 规则集。** 设置 → 输出高亮可即时启用或关闭客户端高亮，并在保守的“日志级别”与扩展的“DevOps”规则集之间切换；DevOps 模式额外识别部署结果、重试、健康状态及结构化 `status` / `state` / `result` 字段，切换时会同步重绘当前终端与历史输出。
- **支持自定义输出高亮规则。** 设置 → 输出高亮现在可添加关键词或正则表达式规则，选择是否区分大小写、仅高亮匹配文字或高亮整行，并从红、黄、绿、青、紫、灰六种颜色中选择；规则可单独启停或删除，会持久化并即时应用于当前终端和历史输出。无效正则会在保存前提示，用户规则优先于内置规则但不会覆盖远端 ANSI 样式。

### 修复 / Fixed

- **不再采集或显示本地系统详情（#268）。** 详细系统信息现在仅对当前已连接的 Linux SSH 会话开放；欢迎页、本地终端、连接中及已断开的会话会隐藏侧栏信息按钮并禁用标题入口，同时清空详情模型。Windows 启动时不再为本机 GPU 等详情启动 PowerShell/CIM 探测，避免启动卡顿和窗口闪烁；侧栏的本机 CPU、内存、网络及磁盘概览保持不变。
- **修复 Windows 11 双屏不同分辨率/缩放下的窗口定位与最大化恢复（#254、#272）。** 设置卡片在被拖动或缩放后，每次重新打开都会按主窗口当前可用尺寸重新约束大小并居中，不再沿用另一块屏幕上的旧逻辑坐标；主窗口从遮挡、失焦或 DPI 切换中恢复时会主动触发两次重绘，并在原生最大化标记与当前显示器几何尺寸明显不一致时重新应用最大化，避免窗口右侧消失或只显示旧屏幕大小的渲染区域。
- **修复进程列表停止刷新以及 root 进程无法结束。** 进程采样现已拆分到独立的轻量 SSH 通道，不再被可能卡住的 `df`、`lspci` 等系统信息探测拖死；提权改为与手工操作一致的 `sudo -S`，使用关闭回显的 PTY，等待密码提示后再以回车提交当前登录用户的 sudo 密码，避免 `su root` 在 root 账户被锁定时始终认证失败。进程控制各阶段、远端安全输出及耗时会写入 `error.log`，密码内容始终脱敏。结束操作仍使用可正常清理资源的 `SIGTERM`（`kill -15`）。
- **修复结束 root 进程时密码弹窗导致程序闪退。** 条件弹窗中的密码输入框不再在 Slint 布局初始化阶段同步抢占焦点，而是在下一轮事件循环、布局完成后自动聚焦，避免触发属性递归检测；同一修复也覆盖 MFA 动态输入框。
- **移除终端底部多余的焦点恢复行。** 终端输出区原先保留的 16px 焦点恢复条会显示第二个 I-beam 鼠标光标并占用约一行空间；现已由覆盖整个终端主体的聚焦层接管，隐藏 IME 输入点缩为跟随真实终端光标的 1×1 锚点，释放完整终端高度。

---

### Added

- **Render color emoji in the terminal (#269).** Terminal output recognizes complete Unicode graphemes—including ordinary emoji, skin tones, flags, and ZWJ family/profession sequences—and replaces the monochrome glyphs produced by Slint's software/femtovg renderers with cached Twemoji color images. Images retain their exact terminal-cell footprint, leaving ordinary text, ANSI styling, CJK, selection, and cursor positioning unchanged.
- **Center the process monitor, copy PIDs, and safely terminate remote processes.** The process window now opens centered on the main window's current screen while preserving its user-adjusted size. PIDs are clickable to copy, and a row context menu can terminate a process; the menu opens on right-button release so the same pointer event cannot skip confirmation. Non-root logins must confirm their own processes and provide the connected user's administrator (sudo) password for root or other users' processes, while a root login needs no additional password. Actions run through a separate SSH exec channel without entering the interactive terminal or command history, password echo is disabled, secrets use zeroizing storage, and the source session, PID, and owner are revalidated before execution.
- **Automatically highlight plain-text log levels.** The terminal now adds theme-aware colours to unstyled `TRACE`, `DEBUG`, `INFO`, `NOTICE`, `WARN`, `WARNING`, `ERROR`, `FATAL`, `CRITICAL`, and `PANIC` markers, including structured `level=error` and JSON level fields. Existing ANSI styling and alternate-screen TUIs such as vim, nano, and htop remain untouched.
- **Add output-highlighting settings and a DevOps preset.** Settings → Output Highlighting can now enable or disable client-side highlighting immediately and switch between the conservative Log Levels preset and an expanded DevOps preset. DevOps mode also recognises deployment results, retries, health states, and structured `status` / `state` / `result` fields; switching presets redraws both live and historical output.
- **Support custom output-highlighting rules.** Settings → Output Highlighting can now add keyword or regular-expression rules with case sensitivity, matching-text or whole-line scope, and red, yellow, green, cyan, magenta, or gray colours. Rules can be enabled individually or removed, persist across launches, and apply immediately to live and historical output. Invalid regexes are rejected before saving; user rules take priority over built-in presets without overriding remote ANSI styling.

### Fixed

- **Stop collecting and exposing local system details (#268).** Detailed system information is now available only for the active connected Linux SSH session. Welcome, local-terminal, connecting, and disconnected states hide the sidebar info button, disable the resource-title entry point, and clear detail models. Windows startup no longer launches PowerShell/CIM probes for local GPU details, preventing startup stalls and console flashes, while the sidebar's local CPU, memory, network, and disk summary remains available.
- **Fix window placement and maximized recovery on Windows 11 mixed-resolution/DPI displays (#254, #272).** After the settings card has been dragged or resized, every reopen now constrains it to the main window's current available size and recenters it instead of retaining logical coordinates from another display. When the main window returns from occlusion, lost focus, or a DPI transition, it requests two redraws and reapplies maximization if the native maximized flag no longer matches the current monitor geometry, preventing a missing right side or a render surface stuck at the previous display size.
- **Fix frozen process lists and root-process termination.** Process sampling now runs on a dedicated lightweight SSH channel, so a blocked `df`, `lspci`, or other system-information probe cannot freeze stale PIDs in the process window. Privilege elevation now matches manual operation through `sudo -S`: an echo-disabled PTY waits for the prompt and submits the connected user's sudo password, avoiding the guaranteed failure of `su root` on hosts with a locked root account. Process-control stages, safe remote output, and timings are written to `error.log` with the password always redacted. Termination still uses the cleanup-friendly `SIGTERM` (`kill -15`).
- **Fix the crash when opening the root-password prompt.** Password fields created inside conditional dialogs now request focus on the next event-loop turn, after Slint has completed layout, instead of synchronously during initialization and triggering the property-recursion guard. The same fix also protects dynamic MFA inputs.
- **Remove the redundant terminal focus-recovery row.** The terminal previously reserved a 16px focus strip that showed a second I-beam pointer and consumed roughly one output row. Full-body focus handling now replaces it, while the hidden IME input is a 1×1 anchor that follows the real terminal cursor, restoring the full terminal height.

## [0.6.2]

### 新增 / Added

- **新增终端字体加粗开关 (#262)。** 设置 → 字体新增“终端字体加粗”，可将普通终端输出强制渲染为加粗字重，并会随配置持久化；ANSI 自带粗体仍然照常生效。
- **新增详细系统信息窗口。** 点击侧栏“服务器资源 / 本机资源”标题或信息按钮可打开独立窗口，按概览、CPU、GPU、CPU 占用、内存、交换、网络接口与文件系统分区展示当前资源来源的详细状态，并跟随侧栏数据实时刷新。
- **新增本地终端入口。** 快速连接列表新增内置 `system` 分组，可直接打开本机 PowerShell、CMD、WSL（Windows 可用时）或当前系统 Shell；内置项不写入配置，也不允许编辑、删除或移动分组。

### 修复 / Fixed

- **修复 SFTP 文件列表下方出现大块空白的问题 (#259)。** “文件 / 隧道”两个内容区改为互斥渲染，隐藏的隧道面板不再继续占用 SFTP 面板布局高度，文件列表可用空间恢复正常。
- **修复 oh-my-zsh 首屏显示 shell integration 注入命令的问题 (#257)。** 连接 zsh/oh-my-zsh 服务器时，隐藏注入命令回显的逻辑现在能处理 `\r` 与软换行，并优先按注入命令尾部定位删除范围，避免 `test -z "$FISH_VERSION" ...` 泄露到终端首屏。

---

### Added

- **Add a bold terminal text option (#262).** Settings → Font now includes a bold terminal text toggle that persists in the config and forces regular terminal output to render with a bold face while preserving ANSI bold behavior.
- **Add Ctrl multi-selection and Shift range extension in the terminal (#262).** Hold Ctrl to add separate selection ranges and Shift to extend the active range; copied text preserves selection order.
- **Add a multi-line paste safety prompt (#262).** Clipboard content containing line breaks now requires explicit review and confirmation before it is sent to the terminal.
- **Collapse the built-in system terminal group by default.** WSL is listed only when `wsl.exe --status` reports that WSL is available.
- **Add a server resource details window.** Clicking the sidebar “Server resources / Local resources” title or info button opens a detached system-information window with CPU, memory, swap, network, and filesystem status that updates with the sidebar data.

### Fixed

- **Fix the long shell-integration setup command appearing after SSH login (#266).** Late-echoed initialization commands are now filtered even when they arrive after the initial connection-output suppression window.
- **Fix a large blank area under the SFTP file list (#259).** The Files / Tunnels content panes now render mutually exclusively, so the hidden tunnel panel no longer consumes SFTP panel layout height and the file list regains its available space.
- **Fix shell-integration setup echo leaking on oh-my-zsh servers (#257).** When connecting to zsh/oh-my-zsh hosts, the setup-echo suppression now handles `\r` and soft-wrapped output and anchors removal on the setup command suffix, preventing the `test -z "$FISH_VERSION" ...` command from appearing on the first terminal screen.

## [0.6.1] - 2026-07-11

### 新增 / Added

- **新增运行时 SSH 隧道面板 (#206)。** SFTP 底部面板新增“文件 / 隧道”切换，已连接 SSH 会话可在运行中新增和停止本地转发 `-L` 与 SOCKS 动态转发 `-D`，现有会话配置不会被自动改写。

### 修复 / Fixed

- **移除点击标签页时复制标签名称的旧行为。** 点击标签页现在只负责切换会话，多窗口/分屏标签交互不再污染剪贴板。

---

### Added

- **Add a runtime SSH tunnel panel (#206).** The SFTP bottom panel now has Files / Tunnels tabs, allowing connected SSH sessions to add and stop local `-L` forwards and SOCKS dynamic `-D` forwards at runtime without modifying the saved session configuration.

### Fixed

- **Remove the old copy-title-on-tab-click behavior.** Clicking a tab now only switches sessions, so multi-window/split-pane tab interaction no longer overwrites the clipboard.

## [0.6.0] - 2026-07-10

### 修复 / Fixed

- **修复拖动资源侧栏宽度时突然变宽的问题 (#244)。** 资源侧栏分隔条改为使用稳定的窗口绝对坐标计算宽高，不再把移动中的分隔条局部坐标反复累加到当前宽度，避免拖动时宽度突然跳变。
- **修复 Windows 10 无边框窗口点击坐标整体错位的问题 (#193)。** Windows 创建 Slint/winit 窗口前会禁用 undecorated shadow 兼容层，避免部分 Win10 环境把隐藏边框计入命中区域，导致渲染位置与鼠标点击位置产生垂直偏移。
- **改善 Windows 高 DPI 缩放下字体发糊的问题 (#224)。** Windows 默认使用 Slint software 渲染器，避开 2K/4K 屏幕开启 125%/150% 等缩放时 OpenGL/FemtoVG 路径可能导致的 UI 与设置页文字偏糊问题；仍可通过 `SLINT_BACKEND` 手动切换渲染器。
- **修复 SFTP 面板拖动后拖拽上传命中区域错误的问题 (#253)。** 文件拖拽上传的落点判断会跟随 SFTP 面板的左、右、上、下停靠位置，只在当前文件列表区域内触发上传，不再固定只识别默认底部文件区。
- **支持显式选择键盘交互认证 (#249)。** SSH 会话新增“键盘交互”认证方式，可直接走 keyboard-interactive 登录；密码 / 首次应答会自动用于第一条普通提示，MFA / OTP 等额外提示继续弹窗询问，SFTP 连接也复用同一认证路径。
- **修复 macOS 触控板无法滚动终端的问题 (#252)。** 终端滚动命中层显式铺满输出区域，并在 macOS 上增加 winit 级触控板滚轮兜底；触控板双指滚动会进入终端回滚/alt-screen 滚轮逻辑，不再只能拖动右侧滚动条。

---

### Fixed

- **Fix sudden resource-sidebar width jumps while resizing (#244).** The resource-sidebar splitter now computes size from stable window-space coordinates instead of repeatedly adding local splitter deltas to the current width, preventing resize jumps while dragging.
- **Fix whole-window click offset on Windows 10 frameless windows (#193).** Windows now disables winit's undecorated-shadow compatibility layer before creating Slint/winit windows, preventing some Win10 environments from counting hidden frame space in hit testing and shifting clicks vertically away from rendered pixels.
- **Improve blurry text on Windows high-DPI scaling (#224).** Windows now defaults to Slint's software renderer, avoiding the OpenGL/FemtoVG path that can make UI and settings text look soft on 2K/4K displays using 125%/150% scaling; `SLINT_BACKEND` can still override the renderer manually.
- **Fix drag-and-drop upload hit testing after moving the SFTP panel (#253).** File-drop upload detection now follows the SFTP panel on the left, right, top, or bottom dock and only triggers inside the current file-list area instead of staying fixed to the default bottom panel.
- **Support explicit keyboard-interactive authentication (#249).** SSH sessions can now choose keyboard-interactive directly; the saved password / first answer is used for the first regular prompt, MFA / OTP prompts still ask interactively, and SFTP reuses the same auth path.
- **Fix terminal scrolling with the macOS trackpad (#252).** The terminal scroll hit layer now explicitly covers the output area, and macOS gets a winit-level trackpad wheel fallback; two-finger scrolling feeds the terminal scrollback/alt-screen wheel path instead of requiring the scrollbar thumb.

## [0.5.71] - 2026-07-10

### 新增 / Added

- **SFTP 文件列表支持排序 (#248)。** 文件列表的名称、大小、修改时间表头支持点击排序，点击循环为升序、降序、恢复默认；右键菜单新增“清除排序”，可直接恢复默认排序。

### 改进 / Changed

- **WebDAV 上传前自动创建缺失目录。** 上传连接配置前会逐级检查远端父目录，目录不存在时尝试通过 WebDAV `MKCOL` 创建；如果文件夹不存在且无权限创建，会返回明确提示“文件夹不存在也无权限创建”。

### 修复 / Fixed

- **修复顶部工具栏与 SFTP 操作栏间距问题 (#245)。** 统一沉浸式标题栏场景下顶部工具栏图标的 Y 轴计算，并增加 SFTP 文件面板操作栏高度和上下内边距，避免上传按钮贴近拖动条。
- **修复复制终端软换行文本时多出换行的问题 (#241)。** 终端行会保留 vt100 的软换行标记，复制选区时跳过由终端宽度造成的自动折行换行，只在真实换行处插入换行符。
- **修复部分旧服务器 SFTP 认证失败的问题 (#186)。** SFTP 在 password 认证被拒后会像终端连接一样重连并尝试 keyboard-interactive，兼容只开放键盘交互认证的 GBK/旧服务器。

---

### Added

- **Support sorting the SFTP file list (#248).** The name, size, and modified-time headers now cycle through ascending, descending, and default order when clicked. The context menu also adds “Clear sort” to restore the default order.

### Changed

- **Create missing WebDAV folders before upload.** Before uploading the connection config, WebDAV now checks each remote parent folder and creates missing folders with `MKCOL`; if a folder is missing and cannot be created, it reports “folder does not exist and cannot be created”.

### Fixed

- **Fix top toolbar and SFTP action bar spacing (#245).** The top toolbar Y offset is now centralized for immersive-titlebar layouts, and the SFTP file panel action bar has more height and vertical padding so the upload button no longer sits too close to the splitter.
- **Preserve logical lines when copying soft-wrapped terminal text (#241).** Terminal rows now retain vt100 soft-wrap metadata, and selection copy skips newlines introduced only by terminal-width wrapping while preserving real line breaks.
- **Fix SFTP authentication on some legacy servers (#186).** When password authentication is rejected, SFTP now reconnects and tries keyboard-interactive like terminal sessions do, improving compatibility with GBK/legacy servers that only allow keyboard-interactive auth.

## [0.5.7] - 2026-07-08

### 新增 / Added

- **新增默认壁纸 MS (#231)。** 新用户首次启动默认使用 `assets/ms.jpg` 作为壁纸；已有用户配置不会被迁移或覆盖，保留原本的壁纸选择。

### 改进 / Changed

- **优化标签页标题显示与复制 (#228)。** 标签页宽度会按标题显示宽度动态调整，中文等非 ASCII 字符按双宽估算，长标题会适当缩小字号以减少截断；移除悬浮提示以避免影响顶部拖动交互，单击标签页可复制完整标题。
- **优化壁纸设置面板滚动。** 内置壁纸列表过宽时会在设置面板内横向滚动，避免缩放或窗口较窄时壁纸选项溢出。
- **欢迎页作为侧栏时隐藏空会话提示。** 开启“欢迎页为侧栏”后，不再在主区域显示“从左侧选择一个会话开始”，避免提示遮挡壁纸和主界面。

### 修复 / Fixed

- **修复版本号与发布校验问题 (#226, #229, #236)。** 命令行支持 `--version` / `-V`，发布脚本与工作流会校验 Cargo 版本、锁文件版本和产物版本，避免再次发布版本号不一致的构建。
- **修复程序目录配置被卸载删除后连接丢失的问题。** 启动时如果程序目录配置没有连接，会从用户目录配置副本恢复 `sessions.json`、`secret.key` 和 `known_hosts`；后续保存会同步写入程序目录和用户目录两份配置，降低更新/卸载误删风险。
- **修复串口历史输出搜索不到的问题 (#233)。** 当前可见行没有命中时会继续搜索 scrollback 历史，找到匹配后自动滚动到对应位置并重绘高亮。
- **修复 Windows 无边框窗口恢复后超出屏幕的问题 (#234)。** 启动与保存布局时按当前显示器尺寸钳制窗口大小，并读取 native 最大化状态，避免 Win11 下恢复到大于屏幕的窗口尺寸。

---

### Added

- **Add MS as the default wallpaper (#231).** New users now start with `assets/ms.jpg` as the default wallpaper; existing user configurations are not migrated or overwritten.

### Changed

- **Improve tab title display and copy (#228).** Tab widths now adapt to the title's display width, counting Chinese and other non-ASCII characters as double width, and long titles use a smaller font to reduce truncation. Hover tooltips were removed to avoid interfering with top-bar drag interactions, while clicking a tab copies the full title.
- **Improve scrolling in wallpaper settings.** Built-in wallpaper choices now scroll horizontally inside the settings panel instead of overflowing on smaller or scaled windows.
- **Hide the empty-session prompt when the welcome page is docked as a sidebar.** The main pane no longer shows "Select a session from the left to start" when the welcome sidebar is already visible.

### Fixed

- **Fix version and release validation issues (#226, #229, #236).** The CLI supports `--version` / `-V`, and release scripts/workflows validate Cargo versions, lockfile versions, and built artifact versions to prevent mismatched releases.
- **Recover connections when the program-directory config was removed during uninstall/update.** If the program config has no connections, startup restores `sessions.json`, `secret.key`, and `known_hosts` from the user config backup; future saves write both the program directory and user directory copies.
- **Fix searching serial scrollback history (#233).** When no visible row matches, search continues through scrollback history, scrolls to the first match, and redraws highlights.
- **Clamp restored frameless windows to the current monitor on Windows (#234).** Startup and layout saving now clamp window size to the visible monitor and read the native maximized state, preventing Win11 restores from exceeding the screen.

## [0.5.6] - 2026-07-06

### 修复 / Fixed

- **修复分屏终端在右侧资源面板展开时被错误压缩到 10 列。** 显式绑定分屏内容区尺寸，并仅在终端视图拥有真实布局尺寸后才上报 resize，避免右侧资源面板展开时的瞬时 0 宽布局被折算成 10 列，导致右侧终端内容异常换行。

---

### Fixed

- **Avoid transient 10-column terminal resize in split panes.** Explicitly size the split-pane content area and only report terminal resize events after the terminal view has a real layout size. This prevents right resource panel changes from turning a transient zero-width layout into a 10-column terminal resize.

## [0.5.5] - 2026-07-05

### 新增 / Added

- **支持分屏合并 (#216)。** 标签页右键菜单新增「Merge panes」，可将当前分屏的所有标签合并到其它 pane，并自动折叠空分屏。
- **支持拖到标签栏合并分屏。** 像 IDEA 一样，将某个分屏里的标签拖到另一个分屏的标签栏后释放，即可把标签移入目标标签组；源分屏移空后会自动合并回单窗口。

### 改进 / Changed

- **优化当前标签页识别度 (#200)。** 暗色模式下当前标签页使用更明确的层级底色；开启壁纸时会从壁纸取色，让标签栏更沉浸。
- **标签页关闭按钮固定在右侧。** 短标签名不再把关闭按钮挤到中间，所有标签的关闭入口位置保持一致。
- **关闭确认弹窗按钮接入主题色。** 关闭应用确认弹窗不再使用系统默认蓝色按钮，主按钮会跟随主题和壁纸取色。

### 修复 / Fixed

- **修复 less 搜索高亮不可见 (#217)。** 正确处理默认前景/背景色下的 reverse-video 反色序列，使 `less` 中 `/` 或 `?` 搜索命中能够正常显示高亮。

---

### Added

- **Split-pane merging (#216).** The tab context menu now includes "Merge panes", moving all tabs from the current split pane into another pane and collapsing the emptied pane automatically.
- **Drag-to-tab-strip pane merge.** Like IDEA, dragging a tab from one split pane onto another pane's tab strip moves it into that tab group; if the source pane becomes empty, it collapses back into a single window.

### Changed

- **Improve active-tab visibility (#200).** Dark mode now gives the active tab a clearer surface level, while wallpaper mode derives the active-tab colour from the wallpaper for a more immersive look.
- **Pin tab close buttons to the right edge.** Short tab names no longer pull the close button toward the middle, keeping the close affordance aligned across tabs.
- **Theme the close-confirmation buttons.** The app-close confirmation dialog now uses themed buttons instead of the platform-default blue button, so the primary action follows the current theme and wallpaper accent.

### Fixed

- **Fix invisible `less` search highlights (#217).** Reverse-video sequences with default foreground/background colours now render a visible background, so `/` and `?` matches in `less` are highlighted correctly.

## [0.5.4] - 2026-07-04

### 新增 / Added

- **SSH 跳板机（堡垒机）支持 (#211)。** 会话可指定另一个已保存的 SSH 会话作为跳板机（类似 OpenSSH 的 ProxyJump）：先连上跳板并认证，在其上开一条 direct-tcpip 通道到目标机，再完成目标机的 SSH 握手。终端与 SFTP 两条连接都经跳板走，跳板连接在整个会话期间保活、会话关闭时一并断开。跳板复用被引用会话自身的主机/账号/密钥/keyboard-interactive 认证，未存凭据时沿用原有登录弹窗；自动忽略指向自己或已删除会话的无效引用；当前仅支持单级跳板。会话对话框「高级」区（仅 SSH）新增「跳板机（可选）」下拉。

### 修复 / Fixed

- **修复多会话大量输出导致界面卡死 (#209)。** 将 VT100 解析移出 UI 线程、按约 30fps 节流渲染、合并输出块并改用按标签页独立的缓冲锁，某台服务器解压大量文件时不再拖垮其它会话与整个界面。
- **修复欢迎页会话面板首次建会话时高度跳变 (#214)。** 快速连接卡片现在始终撑满剩余高度、空状态占位区随之拉伸，建会话前后面板高度保持一致、不再跳动。

---

### Added

- **SSH jump host (bastion) support (#211).** A session can tunnel through another saved SSH session as a jump host (like OpenSSH's ProxyJump): connect and authenticate to the jump, open a direct-tcpip channel to the target, and run the target's SSH handshake over it. Both the shell and SFTP connections go through the jump, which is kept alive for the whole session and torn down when it closes. The jump reuses the referenced session's own host/user/key/keyboard-interactive auth, falling back to the usual login prompt when no credentials are stored; dangling or self references are ignored; single hop only for now. The session dialog's Advanced section (SSH only) gains an optional "Jump host" dropdown.

### Fixed

- **Fix UI freeze on heavy output across multiple sessions (#209).** VT100 parsing moved off the UI thread, rendering throttled to ~30fps, output chunks coalesced, and per-tab buffer locks adopted, so unzipping many files on one server no longer stalls other sessions or the whole UI.
- **Stop the welcome session panel height jump on the first session (#214).** The quick-connect card now always fills the remaining height and the empty-state placeholder stretches to match, keeping the panel height stable before and after adding a session.

## [0.5.3] - 2026-07-04

### 新增 / Added

- **欢迎页侧栏支持四向停靠与持久化。** “欢迎页为侧栏”开启后，快速连接面板现在可以拖到左/右/上/下任一侧，并记住停靠位置、宽度和收起状态。
- **资源面板收起状态持久化。** 资源面板的展开/收起会按最后一次用户操作恢复；只有老配置里没有该状态时，才回退读取“设置 - 界面 - 侧栏”的默认收起设置。
- **同边收起图标栏合并与外侧停靠。** 快速连接和资源面板在同一侧时，只允许一个面板展开；两个都收起时图标位于同一列/同一行；一个展开一个收起时，收起图标栏始终贴在该侧最外侧。
- **资源面板新增内部浅色内容底。** 资源面板现在和快速连接一样有 inset 的圆角浅色/磨砂内容底，视觉层级更统一。
- **开发构建提速配置。** dev profile 下本 crate 取消优化、依赖保持轻度优化，并在 Windows MSVC 下使用 `rust-lld.exe`，提升日常增量 build/check 速度。

### 修复 / Fixed

- **修复欢迎页为侧栏时 tab 栏仍显示“+”的问题。** 开启欢迎页侧栏后，tab 栏不再显示新建欢迎页入口，避免无法切回欢迎页的误导。
- **修复启动时同边双面板同时展开的兜底问题。** 如果配置恢复后快速连接和资源面板同边且都处于展开状态，启动时会自动收起资源面板，保持“一侧只展开一个面板”。
- **修复不同停靠方向下收起箭头位置和图标不一致。** 收起按钮按停靠方向放到合适一侧，并统一使用 Material Icons 箭头。

---

### Added

- **Four-edge docking and persistence for the welcome sidebar.** When “Welcome page as sidebar” is enabled, the quick-connect panel can now dock to the left, right, top, or bottom, and remembers its dock edge, size, and collapsed state.
- **Persistent resource-panel collapse state.** The resource panel now restores the user’s last expanded/collapsed state; older configs without this state still fall back to the Interface sidebar default.
- **Merged outer collapse strips for same-edge panels.** When quick connect and the resource panel share an edge, only one panel expands at a time; collapsed icons share one column/row, and a collapsed strip stays on the outer edge when the other panel is expanded.
- **Inset light content surface for the resource panel.** The resource panel now matches quick connect with an inset rounded frosted content surface.
- **Faster development builds.** The dev profile now leaves the local crate unoptimized, keeps dependencies lightly optimized, and uses `rust-lld.exe` on Windows MSVC for faster incremental build/check cycles.

### Fixed

- **Hide the tab “+” button when the welcome page is a sidebar.** This avoids a misleading new-welcome-tab entry when the welcome page already lives in the sidebar.
- **Prevent same-edge double-expanded panels on startup.** If restored config would expand both quick connect and the resource panel on the same edge, the resource panel is collapsed at startup.
- **Normalize collapse arrow placement and icons across dock edges.** Collapse buttons now use Material Icons and move according to the current dock edge.

## [0.5.1] - 2026-07-01

### Added / 新增

- \*\*会话备注 + 快捷命令「点击是否自动回车」选项(B站建议)
  1. 会话备注:新建/编辑会话对话框加「备注(可选)」字段,存跳板机信息、  
     账号提示、负责人等任意文字。Session 新增 note 字段(serde 默认空,兼容  
     老配置),随会话持久化。2. 快捷命令回车选项:新增/编辑快捷命令时多一个开关「点击即发送执行」  
     (默认开,保持现状)。关闭后,点击该快捷命令只把命令填入输入框、不加  
     末尾回车、不发送,方便先微调再回车发送(仿 FinalShell)。QuickCommand  
     新增 send_enter 字段(serde 默认 true,老命令照常点击即执行)。 涉及 config.rs(两个字段 + default_true)、app.rs(两条 plumbing)、 session_dialog.slint(备注字段)、app.slint(对话框属性/回调/复选框)、 terminal_view.slint(QuickCmd.send-enter + 点击分支)。

---

per-session note + quick-command "send on click" toggle (bilibili suggestions)

1. Session note: the new/edit session dialog gains an optional "Note" field for  
   stashing jump-host details, credential hints, an owner, etc. Session gets a  
   `note` field (serde default empty, so old configs load) persisted with it.2. Quick-command Return toggle: adding/editing a quick command now has a "Send +  
   run on click" switch (on by default = current behaviour). When off, clicking  
   the command only drops it into the input box — no trailing Return, not sent —  
   so it can be tweaked before sending (like FinalShell). QuickCommand gets a  
   `send_enter` field (serde default true, existing commands keep executing).Touches config.rs (two fields + default_true), app.rs (both plumbings), session_dialog.slint (note field), app.slint (dialog props/callbacks/checkbox), terminal_view.slint (QuickCmd.send-enter + the click branch).

## [0.5.0] - 2026-06-30

### Added / 新增

- **分屏(IDEA 式拖动分屏)。** 标签右键「向右拆分 / 向下拆分」,或直接把标签拖到某个 pane 的上/下/左/右  
  边缘(带高亮预览)即可分屏;中间的分隔条可拖动调整两边比例,支持任意嵌套。每个 pane 有自己独立的标签组  
  和当前标签;关掉某个 pane 的最后一个标签会自动折叠回去。  
  **Split panes (IDEA-style drag-to-split).** Right-click a tab → "Split right / Split down", or just  
  drag a tab to a pane's top / bottom / left / right edge (with a highlight preview) to split; drag the  
  splitter between panes to rebalance, nest arbitrarily. Each pane has its own tab group and active tab;  
  closing a pane's last tab collapses it back.
- **复制打开(再开一个当前会话)。** 终端标签右键「复制打开」,对当前会话再开一条独立连接,落在同一个 pane。  
  **Duplicate connection.** A terminal tab's right-click menu opens a second, independent connection to  
  the same session, landing in the same pane.
- **欢迎页可设为侧栏 + IDEA 式收起抽屉。** 设置 → 界面 → 欢迎页,可把会话列表停靠到左侧(不再占用一个「新  
  标签页」标签),打开的会话占据中央;欢迎页 / 资源面板 / SFTP 收起后都变成沿停靠边的「图标条」(点图标  
  展开),取代原来的箭头展开按钮。欢迎侧栏宽度可拖动调整。  
  **Welcome page as a sidebar + IDEA-style collapse drawers.** Interface → Welcome page can dock the  
  session list on the left (no more "New tab" tab); opened sessions fill the centre. The welcome /  
  resource / SFTP panels now collapse to an icon strip along their docked edge (click to re-open),  
  replacing the old arrow expand button. The welcome sidebar is drag-resizable.
- **沉浸式壁纸遮罩透明度可调 + 界面字体大小。** 设置 → 界面 → 壁纸 新增「壁纸遮罩透明度」拖动条,自己调  
  壁纸透出的程度(只影响背景填充,不动文字);设置 → 界面 → 字体 新增「界面字体大小」,可单独放大设置面板。  
  **Adjustable wallpaper-overlay opacity + settings font size.** Interface → Wallpaper adds a  
  "Wallpaper transparency" slider controlling how much the wallpaper shows through (background fills  
  only, text untouched); Interface → Font adds a "Settings font size" control to enlarge the panel.
- **每个会话独立记住自己的 SFTP 状态。** SFTP 的收起 / 高度 / 宽度改为按会话独立,分屏下各 pane 互不影响  
  (新会话默认读「默认收起 SFTP」等公共配置)。  
  **Per-session SFTP state.** SFTP collapse / height / width are now remembered per session, so split  
  panes no longer interfere with one another (new sessions seed from the shared defaults).

### Fixed / 修复

- **设置面板字体发虚。** 设置卡片居中后落在半像素位置,导致面板内文字渲染在亚像素偏移上而发虚;现对齐到  
  整数逻辑像素,和资源面板一样清晰。  
  **Blurry settings-panel text.** The settings card was centred on a half-pixel, so its text rendered on  
  subpixel offsets and looked soft; it now snaps to whole logical pixels, as crisp as the resource panel.

## [0.4.20] - 2026-06-28

### Added / 新增

- **终端内容随窗口缩放重排 (reflow) (#169)。** 此前拖动窗口宽度时,已打印的内容会被截断(变窄丢右半、  
  变宽接不回)。现保留喂给 vt100 的字节流(限长 2 MiB),窗口宽度变化时用新宽度重放一遍,历史与当前屏  
  按新宽度重新折行——变窄换行、变宽接回,全程不丢字符;对齐 FinalShell。alt-screen(tmux/vim)仍由  
  远端 SIGWINCH 重绘。  
  **Terminal content reflows on window resize (#169).** Dragging the width used to clip already-printed  
  lines. meatshell now retains a capped (2 MiB) copy of the byte stream and replays it at the new width,  
  rewrapping scrollback and the live screen with no lost characters, matching FinalShell. Alt-screen  
  programs (tmux/vim) still rely on the remote's SIGWINCH redraw.
- **alt-screen 里把鼠标滚轮转发给程序 (#170)。** 之前在 tmux / less / vim 等全屏程序里滚轮被吞掉、滚不动。  
  现转发给远端:程序开了鼠标跟踪(如 tmux `mouse on`)就发滚轮鼠标事件(SGR / X10),否则退化为方向键  
  (alternate-scroll),让 less / man / vim 也能滚。对齐 FinalShell / MobaXterm。  
  **Forward the mouse wheel to alt-screen programs (#170).** In tmux / less / vim the wheel did nothing.  
  It is now forwarded — a mouse-wheel event when the app tracks the mouse (e.g. tmux `mouse on`), else  
  arrow keys (alternate-scroll) so less / man / vim scroll. Matches FinalShell / MobaXterm.
- **文本批量导入 SSH 连接 (#150)。** 设置菜单新增「批量导入(文本)」,每行 `host|port|user|password|name`  
  (后面字段可省略),一次粘贴导入多台主机;按 host+user+port 去重,密码加密入库。  
  **Batch-import SSH connections from text (#150).** A new "Batch import (text)" menu item accepts  
  `host|port|user|password|name` per line (trailing fields optional), importing many hosts from one  
  paste; dedupes by host+user+port, passwords encrypted at rest.
- **新建 / 编辑会话的「分组」改为可输入下拉框 (#179)。** 既能手输新分组,也能点 ▼ 从已有分组里选。  
  **The session dialog's "Group" field is now an editable dropdown (#179).** Type a new group, or pick  
  an existing one from the ▼ list.
- **终端支持 Shift+Insert 粘贴 (#144)。** X11 / xterm 的经典粘贴键现在也认。  
  **Paste with Shift+Insert in the terminal (#144).** The classic X11 / xterm paste shortcut now works.

### Fixed / 修复

- **主机密钥被拒后不再卡死新连接 (#152)。** 首次连接弹「未知主机」确认框时,若误点卡片外背景把它关掉,  
  之前会把该主机缓存成「拒绝」,导致这一轮运行里之后每次连它都直接报 "Unknown server key",必须重启。  
  现在:拒绝不再写缓存(下次连接照常再弹),且安全确认框不再响应背景点击关闭。  
  **A rejected host key no longer locks out new connections (#152).** Dismissing the "Unknown host"  
  dialog by clicking the backdrop used to cache a reject for the whole run, so every later connect  
  failed with "Unknown server key" until restart. Rejections are no longer cached (the next connect  
  prompts again), and the security dialog no longer dismisses on a backdrop click.
- **兼容旧服务器算法,修复 "No common algorithm" 连不上 (#172)。** russh 默认只协商现代算法,老服务器 /  
  网络设备只支持旧 KEX(group14 / group1-sha1)或 CBC 加密时握手失败。现把这些作为兜底追加(现代算法  
  仍优先),终端与 SFTP 一致,旧设备也能连。  
  **Reach legacy servers, fixing "No common algorithm" (#172).** russh's defaults negotiate only modern  
  algorithms; old servers / gear that only speak legacy KEX (group14 / group1-sha1) or CBC ciphers failed  
  the handshake. These are now offered as fallbacks (modern still preferred) for both the shell and SFTP.
- **SFTP 文件修改时间按本地时区显示 (#168)。** 之前按 UTC 显示,UTC+8 用户看到的时间差 8 小时。现用本机  
  时区换算(跟随系统,不写死)。  
  **SFTP file mtime shown in local time (#168).** It was rendered as UTC (8 h early for a UTC+8 user);  
  now converted to the machine's local timezone.
- **Linux 缩放窗口后鼠标卡在缩放态 (#159)。** 从边角缩放后,窗口管理器吃掉了结束缩放的松开事件,Slint  
  一直保持着对缩放热区的指针抓取——之后到处点都触发缩放。现在缩放后主动补一个释放事件让 Slint 丢掉抓取  
  (X11 必现、Wayland 偶发都修)。  
  **Mouse stuck in resize mode after a Linux window resize (#159).** The window manager consumed the  
  button-release that ends a resize, so Slint kept its pointer grab on the resize handle and every click  
  re-started a resize. We now dispatch a synthetic release afterwards so Slint drops the grab (fixes both  
  the reliable X11 case and the occasional Wayland one).
- **shell 集成回显抑制窗口 1.2s→2s (#176)。** 慢速 PTY / SSH 上,注入的初始化命令回显 + OSC 7 晚于 1.2s  
  到达,导致回退过早、注入行泄漏到终端。放宽到 2s。  
  **Widen the shell-integration echo-suppression window 1.2s→2s (#176).** On slow PTY / SSH the injected  
  setup line's echo + OSC 7 arrived after 1.2 s, so it fell back early and the line leaked into the  
  terminal.
- **设置菜单加批量导入后溢出 (#150)。** 设置下拉菜单原来写死高度,加第 8 项后末项溢出圆角背景;改为跟随  
  内容高度自适应。  
  **Settings menu overflowed after the batch-import entry (#150).** The dropdown had a hardcoded height;  
  it now sizes to its content.

### Performance / 性能

- **合并 shell 输出事件,修复 tail -f 等高频输出导致界面假死 (#171)。** 事件泵原来逐个把每段输出投递到 UI  
  线程并整屏渲染,`tail -f` / 大文件刷屏时 UI 被淹没成假死。现一次性扫空已排队事件、合并相邻输出,一波  
  突发只解析 + 渲染一次,界面保持响应。  
  **Coalesce shell output events, fixing the tail -f UI freeze (#171).** The pump dispatched every output  
  chunk to the UI thread for a full render; under high-frequency output the UI drowned. It now drains  
  queued events at once and merges adjacent output, parsing + rendering a burst once.

## [0.4.19] - 2026-06-28

### Added / 新增

- **macOS 沉浸式标题栏 (#162)。** 此前 Mac 保留原生标题栏,暗模式下顶部是一条突兀的白条。现把  
  原生标题栏设为透明并让窗口内容延伸到其下(fullSizeContentView),标题栏改为显示窗口底色 / 壁纸,  
  跟随暗 / 浅色;顶部预留交通灯按钮的位置并做磨砂,与其它面板统一。Windows/Linux 不受影响。  
  **Immersive title bar on macOS (#162).** macOS kept the native title bar, which showed a jarring  
  white strip at the top in dark mode. The native title bar is now transparent with the window content  
  extending under it (fullSizeContentView), so it shows the window background / wallpaper and follows  
  dark / light; the top reserves room for the traffic-light buttons and is frosted to match the other  
  panels. Windows/Linux unaffected.

### Fixed / 修复

- **修正 macOS 快捷键映射,0.4.18 写反了 (#158)。** Slint 在 macOS 上把 `control` 报成 Cmd(⌘)、  
  `meta` 报成物理 Ctrl,0.4.18 正好用反,导致 ⌘ 快捷键不触发、物理 Ctrl 反而误触发,基本不可用。  
  本版改正并在真机(Mac mini M4)逐一验证:⌘C / ⌘V / ⌘F / ⌘⇧R / ⌘S 正常触发,物理 Ctrl 的  
  ^C / ^X / ^U / ^W 正常直达 shell。另外 macOS 上 Cmd+字母 经 Slint 送来的是控制字符(⌘S = `\u{13}`),  
  编辑器保存据此补上识别,修复 ⌘S 失灵。  
  **Corrected the macOS shortcut mapping that 0.4.18 had backwards (#158).** On macOS Slint reports  
  `control` as Cmd (⌘) and `meta` as the physical Ctrl; 0.4.18 used them the wrong way round, so ⌘  
  shortcuts did nothing and the physical Ctrl triggered them instead — essentially unusable. This  
  release fixes it and verifies every case on real hardware (Mac mini M4): ⌘C / ⌘V / ⌘F / ⌘⇧R / ⌘S  
  fire correctly and the physical Ctrl's ^C / ^X / ^U / ^W reach the shell. Also, on macOS Cmd+letter  
  arrives as a control char (⌘S = `\u{13}`), so the editor's save now recognizes that form too, fixing ⌘S.

## [0.4.18] - 2026-06-26

### Added / 新增

- **Windows 11 圆角窗口 + 投影 (#162 / #166)。** 自绘标题栏的无边框窗口此前是直角、无阴影,  
  不符合 Win11 风格。现用 DWM 给它补上系统**圆角**(DWMWA_WINDOW_CORNER_PREFERENCE)和**投影**  
  (DwmExtendFrameIntoClientArea);Win10 自动忽略圆角属性,其他平台无影响。  
  **Native rounded corners + drop shadow on Windows 11 (#162 / #166).** The frameless window (custom  
  title bar) had square corners and no shadow. DWM now gives it the system rounded corners and  
  shadow; ignored on Windows 10, a no-op elsewhere.
- **macOS 上 app 快捷键改用 Cmd(⌘),释放 Ctrl 给终端 (#158)。** Mac 有 Ctrl 和 Cmd 两个键,之前  
  app 快捷键全占了 Ctrl,导致 nano 里 ^X 等控制键发不到 shell。现按 macOS 习惯:查找 / 复制 /  
  粘贴 / 历史 / 保存用 ⌘,物理 Ctrl 原样直达 shell;命令框的 Ctrl+A/E/K/U 行编辑保留 Ctrl;设置 →  
  快捷键也显示 ⌘ / ⌃。Windows/Linux 不变。  
  **macOS app shortcuts now use Cmd (⌘), freeing Ctrl for the terminal (#158).** macOS has both Ctrl  
  and Cmd; app shortcuts all used Ctrl, so terminal control keys (^X in nano…) couldn't reach the  
  shell. Now find / copy / paste / history / save use ⌘ and the physical Ctrl passes straight through;  
  the command box's Ctrl+A/E/K/U line editing stays on Ctrl; Settings → Shortcuts shows ⌘ / ⌃.  
  Windows/Linux unchanged.

### Fixed / 修复

- **SFTP 闲置一段时间后失效 (#160)。** SFTP 连接没有 keepalive,空闲时被 NAT / 防火墙 / 服务器空闲  
  超时掐断,之后点目录"文件夹读取失败"、增删改全废。两条连接(终端 + SFTP)现都加 30s keepalive  
  保活,真死了由 keepalive_max 关闭。  
  **SFTP stopped working after sitting idle (#160).** The SFTP connection had no keepalive, so it was  
  silently dropped by NAT / firewall / server idle timeouts; afterwards every operation failed. Both  
  connections now send a 30 s keepalive, with keepalive_max still closing a genuinely dead one.
- **git clone / curl 输出在极窄列(~10)乱折行 (#163)。** 布局回流时 root.width 瞬间读成 ≈0,终端  
  列数塌到下限 10 并立刻 resize 远程 PTY,正在跑的输出就按 10 列乱折。现给 PTY resize 加 150ms  
  防抖,只应用静置后的尺寸,一闪而过的坏值不再发到服务器。  
  **git clone / curl output wrapped at ~10 columns (#163).** A layout reflow momentarily reported a  
  near-zero width, collapsing the terminal column count to its floor of 10 and resizing the remote  
  PTY, which garbled in-flight output. PTY resizes are now debounced (150 ms) so only the settled  
  size reaches the server.
- **设置 / 下载下拉菜单在资源面板停靠时错位。** 资源面板停右 / 上时齿轮 / 下载按钮会随工具栏位移,  
  但两个下拉用的是固定坐标,会飘到资源面板上。现让下拉跟随按钮位移。  
  **Settings / download dropdowns floated over the docked resource panel.** The gear / download  
  buttons shift with the toolbar when the resource panel docks right / top, but the two dropdowns  
  used fixed coordinates. They now follow their buttons.
- **浅色模式次要 / 弱化文字太浅。** 沉浸壁纸 + 浅色下,副标题、磁盘 / 路径标签、说明文字等灰得发飘、  
  对比度差。浅色模式的 text-secondary / text-muted 已加深(深色模式不变)。  
  **Faint secondary / muted text in light mode.** With the immersive wallpaper + light mode,  
  subtitles, disk / path labels and hints were too pale on the bright background. Light-mode  
  secondary / muted text is darkened (dark mode unchanged).
- **沉浸壁纸下,收起的资源面板展开按钮旁露出"黑块"。** 给展开按钮预留的 30px 空位没有背景,露出  
  底下深色壁纸,浅色主题里就是一小块黑。现补上与按钮一致的磨砂背景。  
  **A "black block" next to the collapsed resource-panel expand button in immersive mode.** The 30px  
  gap reserved for the expand button had no background and showed the raw (dark) wallpaper; it now  
  uses the same frosted background as the button.

### Security / 安全

- **记录 RUSTSEC-2026-0154 不可达,暂不升级 russh (#151)。** 该 DoS 在 russh 的 ssh-agent 帧解析,  
  meatshell 完全不用 ssh-agent,漏洞代码路径在本二进制里不可达、不可利用;而唯一修复版  
  (russh ≥ 0.60.3)会引入一堆**预发布**加密库(ed25519-dalek pre、aes-gcm rc…),在 SSH 客户端里  
  用未审计的 rc 密码库风险更大。故 russh 暂留 0.49,等其依赖脱离 -rc 再迁移;新增 audit.toml 附完整理由。  
  **Documented RUSTSEC-2026-0154 as unreachable; holding russh at 0.49 (#151).** The DoS is in russh's  
  ssh-agent frame parsing, which meatshell never uses — the path is dead code here. The only patched  
  russh (>= 0.60.3) drags in a stack of pre-release crypto crates, a worse trade than this unreachable  
  DoS, so russh stays at 0.49 until its deps leave the -rc channel. Adds audit.toml with the full  
  rationale.

## [0.4.17] - 2026-06-24

### Added / 新增

- **MFA / 验证码登录(JumpServer 等强制开启 MFA 的堡垒机)(#86)。** 这类堡垒机在  
  keyboard-interactive 里先要密码、再要动态验证码;旧版对每个提示都回填密码,验证码那步  
  必然失败(这正是"不支持 JumpServer"的真实原因)。现在密码挑战自动用已保存密码应答,  
  其余挑战(MFA / 验证码)弹出「双重验证」对话框向你索取,回车即继续;终端与 SFTP 并发  
  连接只问一次,输错码重连会重新弹框,而非静默重放旧码。  
  **MFA / verification-code login on bastions that force MFA (JumpServer etc.) (#86).** Such  
  bastions ask for the password then a one-time code over keyboard-interactive; the old code  
  answered every prompt with the password, so the code step always failed. Now the password  
  challenge is answered automatically and any other challenge pops a "Two-factor (MFA)" dialog  
  showing the server's prompt; the shell and SFTP ask once, and a wrong code re-prompts on  
  reconnect instead of being replayed.
- **纯键盘命令历史检索(#140)。** 命令框按 `Ctrl+R` 唤出历史并聚焦搜索框,`↑↓` 选择、  
  回车执行、`Esc` 关闭并回到终端;终端里 `Ctrl+Shift+R` 跳过去(用 Shift 保留 shell 自身的  
  反向搜索)。整个历史检索可全程不碰鼠标。  
  **Keyboard-only command-history search (#140).** `Ctrl+R` in the command box opens the  
  history with its search box focused; `↑↓` select, Enter runs, `Esc` closes and returns to  
  the terminal. `Ctrl+Shift+R` jumps there from the terminal (Shift keeps the shell's own  
  reverse-search).
- **会话选项:禁用 shell 集成(Windows / pwsh 服务端)(#140)。** 会话编辑「高级」里新增  
  开关,勾上后跳过 cwd 跟随注入与远程资源监控——专为非 POSIX shell(Windows pwsh/cmd)  
  准备,避免注入破坏 shell。  
  **Session option: disable shell integration (Windows / pwsh server) (#140).** A new toggle  
  in the session dialog's advanced section skips the cwd-follow hook and the resource monitor —  
  for non-POSIX shells (Windows pwsh/cmd) where injecting them breaks the shell.

### Changed / 优化

- **空闲降耗:失焦停光标闪烁 + 后台暂停/降频系统采样(#127)。** 几个周期性定时器原先即便  
  窗口在后台、终端空闲也照常触发整窗重绘。现在:光标在窗口失焦时停止闪烁(改为常亮);  
  系统采样在最小化/被遮挡时暂停、仅失焦时降到 ~5s。实测后台空闲 CPU 从约 10% 降到接近 0。  
  **Cut idle CPU: stop the cursor blink when unfocused, pause/throttle the system sampler in  
  the background (#127).** Periodic timers used to repaint the whole window even backgrounded  
  and idle. The cursor now stops blinking (shows solid) when the window is unfocused, and the  
  sampler pauses when minimized/occluded and backs off to ~5 s when merely unfocused.
- **弹窗交互:Esc 关闭 + 关闭确认抢焦点(#140)。** 设置 / 关闭确认 / 凭据 / MFA 现在都能用  
  `Esc` 关闭;关闭确认弹窗会抢走键盘焦点——回车/空格关闭、`Esc` 取消(「点叉 + 空格」一气  
  呵成),终端背后不再被误输入。快捷键弹窗内容过多时可滚动。  
  **Dialog interaction: Esc-to-close + focus the close-confirm dialog (#140).** Settings /  
  close-confirm / credential / MFA dialogs now close with `Esc`; the close-confirm dialog grabs  
  keyboard focus (Enter/Space close, `Esc` cancels) so X-then-Space closes it and the terminal  
  no longer keeps receiving input behind it. The shortcuts dialog scrolls when it's too tall.

### Fixed / 修复

- **Windows / pwsh 服务端 shell 失效(回归,自 0.4.7)(#140)。** cwd 跟随注入的是 POSIX  
  专用 hook,pwsh/cmd 既跑不了、也不回吐它等待的 OSC 7,导致客户端一直屏蔽终端输出、空白  
  卡死(SFTP 是独立通道,所以不受影响)。现给屏蔽窗口加 1.2s 超时兜底:非 POSIX shell 也能  
  正常显示;配合上面的「禁用 shell 集成」开关可做完全干净的处理。  
  **Windows / pwsh server shell stopped working (regression since 0.4.7) (#140).** The  
  cwd-follow setup injects a POSIX-only hook; a Windows pwsh/cmd shell can't run it and never  
  echoes the OSC 7 the client waits for, so output stayed hidden and the terminal went blank.  
  The suppression now has a 1.2 s timeout so a non-POSIX shell is usable again; pair it with the  
  new "disable shell integration" toggle for a fully clean result.
- **修正 macOS 安装说明(#135)。** README 写的是 `tar -xzf …macos-*.tar.gz` + 裸 `meatshell`  
  二进制,但实际发布产物是 `.zip` + `meatshell.app` 应用包,三条命令全对不上。已改为:解压  
  `.zip` →(可选)移入 `/Applications` → 去 `com.apple.quarantine` 隔离属性 → `open`。  
  **Fixed the macOS install instructions (#135).** The README said to `tar -xzf …macos-*.tar.gz`  
  and run a bare `meatshell` binary, but the release artifact is a `.zip` containing  
  `meatshell.app`. Updated to: unzip → optionally move to `/Applications` → clear  
  `com.apple.quarantine` → `open`.

## [0.4.16] - 2026-06-23

### Added / 新增

- **沉浸式壁纸主题(可换壁纸 + 全局沉浸配色)。** 新增「设置 → 界面 → 壁纸」,提供  
  macOS 风格的缩略图选择器:内置 **3 张**(简约·浅、简约·暗、**幻想3048**——赛博朋克合成波,  
  星空 + 发光星球 + 霓虹网格,均为程序化绘制、无图片资源),也可「选择文件…」用自己的图片。  
  选定后壁纸铺满整个窗口(含终端、侧栏、SFTP,以及独立的进程窗),各面板**磨砂半透**让壁纸  
  透出,同时从壁纸提取主色**自动重着色强调色并微调背景**,深浅由壁纸亮度决定(内置款),  
  自定义照片则交给主题开关手动控制可读性。**下个版本默认即「幻想3048 + 暗色」。**  
  **Immersive wallpaper theming (custom wallpaper + global tinting).** Adds Settings →  
  Interface → Wallpaper with a macOS-style thumbnail picker: **3 built-ins** (Meat Light,  
  Meat Dark, and **Fantasy 3048** — a cyberpunk synthwave scene with a starfield, a glowing  
  planet and a neon grid, all drawn procedurally with no image assets) plus a "Choose  
  file…" option for your own image. The wallpaper fills the whole window (terminal,  
  sidebars, SFTP and the detached process window), panels **frost translucently** to let it  
  show through, the accent is **recoloured from the image's dominant colour** and surfaces  
  are subtly tinted, with light/dark taken from the wallpaper's brightness (built-ins) while  
  custom photos leave light/dark to the theme toggle for readability. **The next release  
  ships with "Fantasy 3048 + dark" as the default.**
- **便携模式:配置改存到程序同目录的 `config/`(#141)。** 用户数据(`sessions.json`、  
  加密密钥、`known_hosts`、`error.log`)现在优先存放在**可执行文件旁的 `config/` 文件夹**,  
  整个程序可以随 U 盘携带,也不再往用户目录(`%APPDATA%`)里塞东西。当程序装在只读位置  
  (如 Program Files / `/usr`)时,自动回退到原来的「按用户的系统配置目录」——这也是旧版本  
  的存放位置,所以**老安装原样可用**。首次切到便携目录时,会把旧用户目录里的数据**复制**  
  过去(只复制不删除、不覆盖已存在文件,作为兜底),升级用户不会丢失已保存的会话。  
  **Portable mode: config moves to a `config/` folder next to the app (#141).** User data  
  (`sessions.json`, the encryption key, `known_hosts`, `error.log`) is now stored, by  
  preference, in a **`config/` folder beside the executable**, so the whole app can travel  
  on a USB stick and stops cluttering the user profile (`%APPDATA%`). When the app is  
  installed somewhere read-only (Program Files / `/usr`), it falls back to the per-user OS  
  config dir — the same place older versions used, so **existing installs keep working  
  untouched**. On the first launch that lands on the portable dir, data from the legacy  
  per-user dir is **copied** over (copy-not-move, never overwriting, as a safety net) so  
  upgrading users don't lose saved sessions.
- **终端内查找:Ctrl+F 唤出查找栏。** 在会话里按 Ctrl+F 即可弹出顶部查找栏(与右键菜单  
  → 查找一致),输入即时高亮所有匹配,Esc 关闭;已在「设置 → 快捷键」中登记。  
  **Find in terminal: Ctrl+F opens the find bar.** Press Ctrl+F in a session to bring up  
  the find bar (same as right-click → Find); matches highlight as you type and Esc closes  
  it. Now listed under Settings → Shortcuts.
- **面板可拖动吸附停靠(资源面板 + SFTP)。** 资源面板和 SFTP 面板现在都能拖到四条边  
  (上/下/左/右):拖动面板手柄时,四条边浮现高亮放置区,松手即吸附到那条边。两个面板都  
  可拖动调节大小;折叠后会缩成停靠边缘的一个小展开按钮(彻底隐藏面板)。**自适应:** 资源  
  面板横向(上/下)停靠时,内部小组件自动改为横排;SFTP 竖向(左/右、窄)停靠时隐藏目录树,  
  并随宽度**渐进隐藏「大小→时间」列**(名称快被挤成「…」时才让位),横向(上/下、宽)停靠  
  则恒显示全部列。SFTP 工具栏左侧新增专用拖动手柄,密集控件下也能稳稳拖动。  
  **Drag-to-dock panels (resource panel + SFTP).** Both the resource panel and the SFTP  
  panel can now be dragged to any edge (top / bottom / left / right): dragging the  
  panel's handle shows highlighted drop zones on all four edges, and releasing snaps it  
  there. Both panels are drag-resizable and collapse to a small expand button on their  
  docked edge (fully hiding the panel). **Responsive:** the resource panel lays its  
  widgets out in a row when docked horizontally; the SFTP panel hides its directory tree  
  when docked vertically (narrow) and progressively drops the **Size → Modified** columns  
  as it narrows (only once the Name would elide to “…”), while a horizontal (wide) dock  
  always shows every column. A dedicated drag grip was added to the SFTP toolbar so the  
  panel is grabbable even though its toolbar is full of controls.
- **布局持久化。** 两个面板的停靠边与宽/高,以及父窗口大小,都会在退出时保存、下次启动  
  恢复——可以保留你喜欢的窗口尺寸和面板布局。  
  **Layout persistence.** Each panel's docked edge and size, plus the window size, are  
  saved on exit and restored on the next launch — so your preferred window size and  
  panel arrangement stick.

### Changed / 优化

- **历史命令的搜索框移到下拉框底部 (#131)。** 命令历史下拉向上展开,搜索框原先在顶部、  
  位置随历史条数上下浮动、不好找;现在固定在下拉框**底部**(紧挨命令输入框),列表在其上方  
  填充并可滚动——位置稳定、一眼可见,和 FinalShell 一致。  
  **History search box moved to the bottom of the dropdown (#131).** The command-history  
  dropdown opens upward; the search box used to sit at the top, drifting up and down with  
  the number of entries and hard to find. It's now pinned to the **bottom** of the  
  dropdown (right above the command input), with the scrollable list filling the space  
  above it — a fixed, immediately visible spot, matching FinalShell.
- **SFTP 折叠按钮与资源面板统一,并保持在右侧。** 两个面板现在共用同一个展开按钮组件;  
  SFTP 的控件本就在右侧,折叠后的展开按钮也随之停在右下/右上,不再突兀地跳到左边。  
  **SFTP collapse button unified with the resource panel, kept on the right.** Both panels  
  now share one expand-button component; since SFTP's controls live on the right, its  
  collapsed expand button stays at the bottom-/top-right instead of jumping to the left.

### Fixed / 修复

- **含中文的行复制/查找列错位 (#132)。** 终端纯文本按「一字一字符」存储,而中文(CJK)字  
  在网格上占两列;复制时把选区列号当作字符下标,导致丢失的字符数恰好等于选区前面的中文  
  字数(如选「1pctl update password」实际只复制到「e password」)。现引入 unicode-width  
  做「字符↔网格列」换算:复制所见即所得,在宽字形第二格起选也会整字纳入;查找高亮框(同源  
  问题)改按网格列绘制,中文行之后也能精确罩住文字。  
  **Copy & find column drift on lines with CJK glyphs (#132).** The terminal's plain text  
  stores one char per glyph, but a wide (CJK) glyph spans two grid cells; copy treated a  
  selection's column as a char index, dropping as many characters as there were wide glyphs  
  before the selection (selecting “1pctl update password” yielded only “e password”). A  
  char-to-column conversion (via unicode-width) makes copy WYSIWYG — anchoring on the  
  second cell of a wide glyph still grabs the whole glyph — and find highlights (same root  
  cause) now sit on grid columns so they line up after CJK.
- **macOS 欢迎页布局错位。** 欢迎页的标题、副标题、快速连接卡片在 macOS 上被拉开(标题与  
  副标题间出现大空隙)。现在 Welcome 显式填满内容区、头部固定在顶部按自然高度排列,卡片填满  
  其余空间。  
  **macOS welcome-page layout spread apart.** The title, tagline and quick-connect card  
  were spaced out on macOS (a large gap between the title and tagline). The Welcome view  
  now explicitly fills the content area and the header is pinned to the top at its  
  natural height, with the card filling the rest.

## [0.4.13] - 2026-06-21

### Fixed / 修复

- **堡垒机(JumpServer 等)密码登录“认证失败” (#86)。** 这类堡垒机默认只放行  
  `keyboard-interactive` 认证、关闭 `password` 方法,旧版只尝试 `password`,因此直接  
  “认证失败”——Xshell/MobaXterm/WindTerm 能登正是因为会自动回退。现在密码认证失败后会  
  断开并重连一条全新连接,改用 `keyboard-interactive` 以密码应答服务器提示。注意:russh  
  在一次失败的认证后无法在同一句柄上切换认证方法(会卡死),因此回退必须重连。已在真实的  
  keyboard-interactive-only sshd 上验证登录成功。  
  **Password login through bastions (JumpServer etc.) failed with “authentication  
  failed” (#86).** Such bastions disable the `password` SSH method and only accept  
  `keyboard-interactive`; the old code only tried `password`, so it failed outright —  
  other clients get in because they fall back automatically. Now, on password-auth  
  failure we disconnect and reconnect on a fresh handle, then authenticate via  
  `keyboard-interactive`, answering each prompt with the password. (russh hangs if a  
  second auth method is attempted on a handle whose first attempt already failed, so a  
  reconnect is required.) Verified against a real keyboard-interactive-only sshd.

### Changed / 优化

- **设置·界面简约重做。** 右侧从竖排改为「分区 + 标签左·控件右」的紧凑布局(iOS 风  
  开关、`[− 值 +]` 步进器、固定字号不随界面缩放放大,解决“字体过大”观感);仍为内嵌  
  模态浮层,打开时遮罩吞鼠标 + 抢焦点吞键盘,禁止对主窗口的一切输入,卡片只能在窗口内拖动。  
  **Redesigned Interface settings.** The right pane moves from stacked fields to a  
  compact “section + label-left · control-right” layout (iOS-style switches,  
  `[− value +]` steppers, fixed typography that ignores UI scale — fixing the  
  “fonts too big” feel). Still an embedded modal overlay that blocks all input while  
  open (veil swallows mouse, focus scope swallows keys); the card only drags within  
  the window.
- **初始窗口放大到 1440×900。** 从 1200×760 提升到更舒适的默认尺寸,对齐同类客户端。  
  **Larger default window, 1440×900.** Up from 1200×760, matching comparable clients.
- **Quieter startup logs.** Silenced fontdb's harmless "malformed font" warning for  
  system fonts it can't parse but skips anyway (e.g. Windows' `mstmc.ttf`), and  
  demoted the routine UI-font-selection line to `debug` — only an actual font-load  
  failure still warns. `error.log` stays clean.  
  **更安静的启动日志。** 屏蔽 fontdb 对无法解析(但会自动跳过)的系统字体发出的  
  「malformed font」无害告警(如 Windows 的 `mstmc.ttf`),并把常规的界面字体选择日志  
  降为 `debug`——只有真正的字体加载失败才会告警。`error.log` 保持干净。

### Added / 新增

- **侧栏可拖动调宽。** 在资源面板与主区之间加了可拖动分隔条,宽度可在 160–520px 间  
  调节并持久化到配置(重启保留);折叠侧栏时分隔条自动隐藏,拖动期间禁用折叠动画以跟手。  
  **Drag-resize the sidebar.** A draggable splitter sits between the resource panel and  
  the main area; the width is adjustable within 160–520px and persisted to config  
  (survives restart). The splitter hides when the sidebar is collapsed, and the  
  collapse animation is disabled while dragging for 1:1 tracking.
- **进程监视独立窗口。** 进程监视从内嵌浮层提升为真正的独立 OS 窗口,可拖出主窗口、  
  拖到第二块屏幕;无边框自绘标题栏 + 右下角缩放手柄,与主窗口实时共享同一份进程数据。  
  **Detachable process-monitor window.** The process monitor is now a real top-level OS  
  window that can be dragged outside the main window or onto a second monitor, with a  
  frameless custom titlebar and a bottom-right resize grip; it shares one live process  
  model with the main window.
- **Group quick commands, collapsible (#55).** Quick commands now take an optional  
  group/folder name. Leaving it empty drops the command into the implicit  
  "default" group. In the command-bar popup each group shows a header that can be  
  clicked to collapse/expand it — same behaviour as the welcome page's quick-connect  
  session groups. The manage dialog gained a "Group (optional)" field and shows the  
  grouping.  
  **快捷命令支持分组、可收起 (#55)。** 快捷命令新增可选的分组名,留空则归入隐式的  
  「default」分组。命令栏弹窗里每个分组带标题,点击即可收起/展开——和欢迎页快速连接的  
  会话分组体验一致。管理对话框新增「分组（可选）」输入框并按分组展示。
- **Full quick-command management, mirroring the session panel (#55).** Right-click  
  a command — in the command-bar popup or the manage dialog — for Edit / Duplicate /  
  Delete / Move to group, and right-click a group header for Rename / Delete (empty) /  
  New group; the manage dialog also has a "+ New group" button. Same right-click  
  model as the welcome page's quick-connect sessions. Groups start **collapsed** by  
  default, and empty groups persist so you can pre-create folders.  
  **快捷命令完整管理,对齐会话面板 (#55)。** 在命令栏弹窗或管理对话框里右键命令(编辑、  
  复制、删除、移动到分组),右键分组标题(重命名、删除空分组、新建分组),管理对话框另有  
  「+ 新建分组」按钮——与欢迎页快速连接会话的右键体验一致。分组**默认收起**,空分组会被  
  保留以便预先建好文件夹。

## [0.4.12] - 2026-06-20

### Fixed / 修复

- **macOS 26 blank text — switch the default CJK UI font to one femtovg can render  
  (#129, #108).** Root cause finally pinned: on some macOS 26 machines femtovg  
  cannot rasterize the *modern* system CJK fonts (PingFang SC, Hiragino) — fontdb  
  finds them but every glyph comes out blank — while the older Heiti/STHeiti/Songti  
  faces render perfectly (verified per-font on an M2 / macOS 26). It was never the  
  renderer (0.4.11's femtovg revert alone didn't help) nor font *loading* (fontdb  
  loaded 900+ faces). The UI now prefers the reliably-rendering "Heiti SC" (a clean  
  sans-serif that ships on every macOS), with STHeiti/Songti as further fallbacks  
  and the embedded "Meatshell Mono" as a last resort so the window is never blank.  
  A `MEATSHELL_UI_FONT="<family>"` env var can force any family without a rebuild.  
  **修复 macOS 26 文字全白——默认中文界面字体改用 femtovg 能渲染的字体 (#129, #108)。**  
  根因最终定位:部分 macOS 26 机器上 femtovg 无法栅格化*新版*系统中文字体(PingFang  
  SC、Hiragino)——fontdb 能找到它们,但每个字形都画成空白;而老字体  
  Heiti/STHeiti/Songti 渲染完全正常(已在 M2 / macOS 26 上逐字体实测)。既不是渲染器  
  (0.4.11 单独退回 femtovg 没用),也不是字体*加载*(fontdb 加载了 900+ 个 face)。  
  界面现在优先用稳定渲染的「Heiti SC」(所有 macOS 自带的干净黑体),STHeiti/Songti  
  作为后备,内置「Meatshell Mono」兜底,确保窗口永不全白。可用环境变量  
  `MEATSHELL_UI_FONT="<字体名>"` 免重编强制指定任意字体。

## [0.4.11] - 2026-06-20

### Fixed / 修复

- **macOS text-invisible regression — renderer no longer force-switched (#129, #108).**  
  0.4.10 force-set the Skia renderer on macOS to work around femtovg failing to  
  render text on macOS 26 (#108). That shipped unverified and broke a *different*  
  set of Macs (Apple Silicon, macOS 26.5): Skia could not resolve the "PingFang SC"  
  UI font, so all text vanished there instead (icons survived because they use an  
  embedded font). The default now stays femtovg (known-good for the majority);  
  Skia is still compiled in on macOS and can be opted into at launch with  
  `SLINT_BACKEND=winit-skia` for machines where femtovg fails.  
  **修复 macOS 文本全部消失的回退问题——不再强制切换渲染器 (#129, #108)。** 0.4.10 为  
  绕过 macOS 26 上 femtovg 取字失败(#108),在 macOS 强制改用 Skia 渲染器;该改动  
  未经真机验证就发布,反而弄坏了另一批 Mac(Apple Silicon / macOS 26.5):Skia 无法  
  解析「PingFang SC」界面字体,导致这些机器上文字全部消失(图标因使用内嵌字体而正常)。  
  现默认改回 femtovg(对绝大多数机器正常);macOS 仍编译 Skia,femtovg 失效的机器可在  
  启动时用 `SLINT_BACKEND=winit-skia` 手动启用。

### Added / 新增

- **Cancel an in-progress upload, with remote cleanup (#100).** Uploads can now be  
  cancelled like downloads; cancelling removes the half-written file on the remote  
  so no partial junk is left behind.  
  **上传也支持取消并清理远端半成品 (#100)。** 上传可像下载一样取消;取消会删除远端已  
  写入的半成品文件,服务端不留垃圾。
- **Sponsor / donation link in the README.** Added a WeChat sponsor QR for anyone  
  who'd like to support development.  
  **README 增加赞助/捐赠入口。** 加入微信赞助二维码,欢迎支持项目开发。

### Changed / 优化

- **Silenced ICU4X segmentation-data log noise.** Suppressed the spurious ICU4X  
  data-error warnings so they no longer clutter the log / error.log.  
  **屏蔽 ICU4X 段落数据噪音日志。** 抑制无意义的 ICU4X data-error 警告,不再污染日志  
  与 error.log。

## [0.4.10] - 2026-06-19

### Added / 新增

- **SFTP multi-select with one-archive download (#100).** Check multiple files in  
  the SFTP panel and download them together: the selection is packed into a single  
  `tar` on the remote (named after the first item, e.g. `11等文件.tar`), pulled in  
  one transfer, then the temp is removed. Any download action (right-click, row,  
  toolbar) packs the whole checked set when 2+ are checked; a single selection  
  downloads as a plain file. Batch delete is also supported, and an empty folder  
  is reported instead of creating an empty local directory.  
  **SFTP 文件多选 + 打包下载 (#100)。** 在 SFTP 面板勾选多个文件即可一起下载:选中  
  项在远端打包成单个 `tar`(以第一个文件命名,如 `11等文件.tar`),一次性下载后删除  
  临时包。勾选 ≥2 项时,任意下载动作(右键/行内/工具栏)都打包整组;单选则按普通  
  文件下载。同时支持批量删除;下载空文件夹会给出提示而非创建空目录。
- **Cancel an in-progress transfer (#100).** Each transfer row shows a cancel  
  button while active or preparing; cancelling removes the partial local file and,  
  for archive downloads, the remote temp archive — no junk left on either side.  
  **可取消进行中的传输 (#100)。** 传输记录每行在下载中/准备中时显示取消按钮;取消会  
  删除本地半成品文件,打包下载还会删除远端临时包,本地与服务端都不留垃圾。
- **Name port-forward rules (#100).** Port-forward rules can be given an optional  
  name so they're easy to tell apart in the list.  
  **端口转发规则可命名 (#100)。** 转发规则可设置可选名称,便于在列表中区分。
- **Global UI scale setting (#100 #117 #118).** A scale control in Interface  
  settings zooms the whole UI (fonts, spacing, radii) from 80% to 200%.  
  **界面整体缩放设置 (#100 #117 #118)。** 界面设置新增缩放控件,可将整个界面(字体、  
  间距、圆角)从 80% 到 200% 缩放。

### Changed / 优化

- **Much faster downloads (#100).** Downloads now use a dedicated, pipelined SFTP  
  channel that keeps many READ requests in flight at once (like uploads already  
  did), hiding round-trip latency — large files and archive bundles download  
  noticeably faster.  
  **下载大幅提速 (#100)。** 下载改用专用、流水线化的 SFTP 通道,多个读请求并发在途  
  (与上传一致),掩盖往返延迟 —— 大文件和打包包下载明显更快。
- **Switch directories during transfers.** SFTP transfers run on their own task,  
  so listing and changing directories stays responsive while files move.  
  **传输时仍可切换目录。** SFTP 传输在独立任务上运行,文件传输期间列目录、切换目录  
  依然流畅。

### Fixed / 修复

- **macOS 26 (Tahoe): all UI text invisible (#108).** The default femtovg renderer  
  failed CoreText font lookup on macOS 26, blanking every glyph including the  
  embedded mono font. macOS now uses the Skia renderer (Windows/Linux unchanged).  
  **macOS 26 (Tahoe) 界面文本全部消失 (#108)。** 默认 femtovg 渲染器在 macOS 26 上  
  取字失败,所有文字(含内嵌等宽字体)消失。macOS 现改用 Skia 渲染器(Windows/Linux  
  不变)。
- **Welcome session list now scrolls (#116).** When there are more sessions than  
  fit, the welcome screen's session list scrolls instead of clipping.  
  **欢迎页会话列表可滚动 (#116)。** 会话过多时,欢迎页的会话列表可滚动,不再被裁切。

## [0.4.9] - 2026-06-19

### Added / 新增

- **Searchable command-history dropdown (#101).** The command-history list is now  
  filterable — type in the search box to narrow entries instantly, then click or  
  press Enter to run the match.  
  **命令历史下拉支持搜索 (#101)。** 历史列表新增搜索框,输入关键字即可实时过滤,  
  点击或回车直接执行匹配项。
- **Readline keys in the command box + shortcuts reference (#103).** The command  
  box now honours common readline bindings (Ctrl+A/E/K/U/W, Alt+B/F/D/Backspace,  
  etc.) for fast inline editing; a keyboard-shortcuts reference panel is also  
  added so users can discover available bindings at a glance.  
  **命令输入框支持 Readline 快捷键 + 快捷键参考 (#103)。** 命令框现在支持常见  
  readline 绑定(Ctrl+A/E/K/U/W、Alt+B/F/D/Backspace 等)进行快速行内编辑;  
  另加快捷键参考面板,方便用户一览可用组合键。
- **Scroll arrows when tabs overflow (#122).** When open tabs exceed the tab bar  
  width, left/right arrow buttons appear so users can scroll through the hidden  
  tabs instead of losing access to them.  
  **标签溢出时显示滚动箭头 (#122)。** 当打开的标签超出标签栏宽度时,左右箭头  
  按钮出现,可滚动查看被遮挡的标签。
- **Slim scrollbar for the terminal output area (#103).** The terminal's vertical  
  scrollbar is now a thin, auto-hiding overlay that doesn't eat into the column  
  count, giving more screen real estate to the actual output.  
  **终端输出区窄滚动条 (#103)。** 终端纵向滚动条改为细窄的自动隐藏覆盖层,  
  不再占用列数,把更多屏幕空间留给实际输出。

### Fixed / 修复

- **Preserve the MOTD/banner when hiding the injected setup line (#98).** The  
  previous approach stripped too aggressively and could swallow the server's  
  MOTD/banner that arrives before the shell prompt; the matcher now only discards  
  the single injected line, leaving the banner intact.  
  **隐藏注入设置行时保留 MOTD/横幅 (#98)。** 之前的做法剥离过度,会把 shell 提示符  
  之前到达的服务器 MOTD/横幅一并吞掉;现在匹配器仅丢弃注入的那一行,横幅原样保留。
- **Reserve space for toolbar icons + scroll overflowing tabs (#122).** The tab  
  bar now leaves a right margin so the last tab's close button isn't hidden  
  behind the toolbar icons; tabs that still overflow are scrollable.  
  **为工具栏图标预留空间 + 溢出标签可滚动 (#122)。** 标签栏右侧留出余量,  
  最后一个标签的关闭按钮不再被工具栏图标遮挡;仍然溢出的标签可滚动查看。

## [0.4.8] - 2026-06-18

### Added / 新增

- **Immersive frameless title bar.** On Windows/Linux the app draws its own  
  themed title bar (app icon + name, minimize/maximize/close, draggable to move,  
  double-click to maximize, edge/corner resize) instead of the OS chrome — so the  
  top follows the light/dark theme instead of staying a mismatched native bar.  
  macOS keeps its native decorations. (#119)  
  **沉浸式无边框标题栏。** Windows/Linux 下自绘主题色标题栏(应用图标+名称、  
  最小化/最大化/关闭、拖动移动、双击最大化、边角缩放),不再使用系统标题栏,顶部  
  跟随明暗主题;macOS 保留原生标题栏。

### Fixed / 修复

- **htop/btop box-drawing and braille no longer render as tofu** on machines  
  without Cascadia Mono installed (e.g. Win11 Home). The embedded font is now a  
  uniquely-named family ("Meatshell Mono") so the OS can't substitute a  
  glyph-poor fallback for it. (#114)  
  **htop/btop 的线框和盲文字符不再显示为方块**(在未安装 Cascadia Mono 的机器上,  
  如 Win11 家庭版)。内嵌字体改用独一无二的族名「Meatshell Mono」,系统无法再用  
  缺字形的字体顶替它。
- **The injected setup line no longer leaks to the terminal on connect**, even  
  when it wraps across the terminal width. Output is buffered until the hook's  
  OSC sequence arrives, then everything up to it is discarded. (#98)  
  **连接后不再出现注入的设置命令**,即使它按终端宽度换行也能正确隐藏。
- **Smooth scrollback across the live/scrolled boundary.** After shrinking then  
  restoring the terminal (e.g. dragging the SFTP panel over it and back),  
  scrolling back through history no longer jumps near the bottom. (#119)  
  **回滚历史在实时/滚动边界处平滑。** 把 SFTP 面板拉上来盖住终端再放下后,往回翻  
  历史时接近底部不再跳。
- **Fast drag-selection in the terminal works again.** A quick drag is no longer  
  stolen by the Flickable, so selecting text by dragging fast still selects. (#119)  
  **终端里快速拖动选择恢复正常。** 快速拖动不再被滚动容器抢走,快速拖选也能选中。
- **The Interface dialog's close button can't be dragged off-screen.** Its drag  
  is clamped inside the window, so the modal dialog can no longer become  
  unclosable. (#119)  
  **「界面」设置对话框的关闭按钮不会被拖出屏幕。** 拖动被限制在窗口内,模态对话框  
  不会再变得无法关闭。

## [0.4.7] - 2026-06-16

### Added / 新增

- **Host-key verification with a first-connect confirmation dialog.** On first  
  contact a dialog shows the host, key type and SHA256 fingerprint; the key is  
  remembered (a known_hosts file beside sessions.json) only after you trust it.  
  A later key that differs is flagged as a possible MITM and needs re-confirming.  
  Replaces the previous "accept any key" behaviour. (#109)  
  **主机密钥校验 + 首次连接确认弹窗。** 首次连接会弹窗显示主机、密钥类型和 SHA256  
  指纹,确认信任后才记住(known_hosts 文件,与 sessions.json 同目录);之后密钥若  
  变化会作为疑似中间人攻击提示并要求重新确认。取代了原先「接受任意密钥」的行为。
- **Quick-connect login, Xshell-style.** New SSH/Telnet sessions now require a  
  host. The username no longer defaults to `root`; if a session is missing its  
  username and/or (password-auth) password, you're prompted for them on connect,  
  with an optional "remember". Auto-naming uses `user@host`, or just the host  
  when no username is given. (#110)  
  **类 Xshell 的快速连接登录。** 新建 SSH/Telnet 会话需填主机;用户名不再默认  
  `root`;会话缺用户名 和/或(密码认证)密码时,连接时弹窗补充,可勾选「记住」。  
  自动命名用 `user@host`,无用户名时仅用主机名。
- **Commands typed in the terminal now join the command history.** Captured via  
  the shell integration hook (bash/zsh), so the command box and ↑/↓ recall  
  include what you ran in the terminal — passwords typed at prompts are never  
  captured. (#113)  
  **终端里直接敲的命令现在也进命令历史。** 通过 shell 集成钩子(bash/zsh)捕获,  
  命令栏和 ↑/↓ 回溯都会包含;在提示符处输入的密码不会被捕获。

### Changed / 变更

- **Command history is de-duplicated, most-recent last.** Re-running a command  
  moves it to the end instead of leaving duplicates; existing history is cleaned  
  up on load. (#113)  
  **命令历史全局去重,最近使用排在最后。** 重复执行只会把命令移到末尾而不再留重复  
  项;已有历史在加载时清理一次。

### Fixed / 修复

- **The injected prompt-setup line no longer leaks to the terminal on connect.**  
  When the echoed setup line was split across packets the matcher missed it,  
  showing `test -z "$FISH_VERSION" && eval '…'`; output is now buffered until the  
  line is complete so it's reliably stripped however it's chunked. (#98)  
  **连接后不再出现注入的设置命令。** 该回显行被分包拆开时旧逻辑匹配不到,会显示  
  `test -z "$FISH_VERSION" && eval '…'`;现在缓冲到该行完整再剥离,无论如何分块都能隐藏。
- **ZMODEM `sz a b c` now receives every file**, not just the first — ZEOF ends a  
  file, not the session. (#109)  
  **ZMODEM `sz a b c` 现在会接收每个文件**,而不只是第一个(ZEOF 表示单个文件结束,  
  而非整个会话结束)。
- **A denied directory listing is handled gracefully.** Instead of spinning  
  forever on a permission error, the panel stops loading and shows a clear  
  "permission denied" message while keeping the current view. (#112)  
  **目录无权限时优雅处理。** 不再卡在加载转圈,面板会停止加载并明确提示「权限不足」,  
  同时保留当前视图。
- **IPv6 bind addresses are bracketed** for `-L`/`-D` port forwards  
  (`[::1]:8080`). (#109/#105)  
  **端口转发的 IPv6 绑定地址加方括号**(`[::1]:8080`),`-L`/`-D` 现在可用。

## [0.4.6] - 2026-06-14

### Fixed / 修复

- **Session-sync upload now works for drag-and-drop too.** Dropping a file onto  
  the SFTP panel used a separate code path that skipped the session-sync mirror;  
  now both the upload button and drag-and-drop mirror the file to every other  
  online session, each into its own current SFTP directory. (Removed the  
  temporary upload diagnostics added in 0.4.5.)  
  **会话同步上传现在对「拖拽」也生效。** 拖文件到 SFTP 面板走的是另一条代码路径,  
  之前漏掉了会话同步;现在上传按钮和拖拽都会把文件同步到其他在线会话(各进各自  
  当前目录)。(移除了 0.4.5 加的临时上传诊断日志。)

## [0.4.5] - 2026-06-14

### Fixed / 修复

- **Session-sync upload now targets each session's own current directory.**  
  Uploading from one session no longer reuses that session's path for the others  
  (which failed when paths differed, e.g. /home/jeff vs /home/root); each session  
  receives the file in its own current SFTP directory. (Includes temporary  
  diagnostics to nail down a remaining report.)  
  **会话同步上传改为各会话用自己的当前目录。** 从某会话上传不再把它的路径套用到  
  其他会话(路径不同就会失败,如 /home/jeff 与 /home/root);每个会话都收到文件到  
  它自己当前的 SFTP 目录。(含临时诊断日志以定位残留问题。)

## [0.4.4] - 2026-06-14

### Added / 新增

- **Session sync / broadcast input.** A new ⟳ toggle in the top-right bar  
  mirrors keystrokes typed in any terminal to every online session  
  (Xshell-style). Off by default, runtime-only. Settings → Session sync also  
  adds "Sync file uploads during session sync": an upload is mirrored to the  
  same path on each session (or that session's current SFTP dir if the path  
  doesn't exist there).  
  **会话同步 / 广播输入。** 右上角新增 ⟳ 开关,把任意终端里敲的键同步到所有在线  
  会话(Xshell 风格);默认关闭、仅本次运行有效。设置 → 会话同步 还有「会话同步时  
  文件上传同步」:上传会同步到各会话的相同路径(该路径不存在则用该会话当前 SFTP  
  目录)。
- **Tooltips on the top-bar icons** (theme / download / settings / session sync).  
  **右上角图标悬停提示**(切换主题 / 下载 / 设置 / 会话同步)。

### Fixed / 修复

- **Light-mode dialogs.** Inputs and buttons in the group / rename / quick-command  
  dialogs no longer blend into the background under the light theme — Slint's  
  std-widget palette now follows the app theme.  
  **浅色模式对话框。** 分组 / 重命名 / 快捷命令等对话框里的输入框和按钮在浅色主题  
  下不再与背景融为一体——std-widget 调色板现在跟随应用主题。
- **Empty session groups** now show a collapse chevron and can be expanded /  
  collapsed, lining up with non-empty groups.  
  **空会话分组** 现在也显示折叠箭头、可展开 / 收起,与非空分组对齐。

## [0.4.3] - 2026-06-14

### Fixed / 修复

- **Wide CJK glyphs are grid-aligned in the terminal.** With a Chinese path, the  
  trailing `/` after `ll`, the cursor after `cd`, and the prompt `$` no longer  
  overlap or drift away from the last CJK character — each wide character now  
  occupies exactly its two terminal cells.  
  **终端里的中文(宽字符)对齐到网格。** 中文路径下,`ll` 后目录名的 `/`、`cd`  
  之后的光标、提示符 `$` 不再与中文末字重叠或拉开很远——每个宽字符现在正好  
  占它的两个终端格。

## [0.4.1] - 2026-06-14

### Added / 新增

- **Run / copy / delete actions on command history (#96).** Each entry in the  
  command-history dropdown now has run (▶), copy (⧉) and delete (🗑) buttons —  
  run executes it immediately, copy puts it on the clipboard, delete removes it.  
  **命令历史的运行 / 复制 / 删除 (#96)。** 历史下拉里每条记录新增 ▶ 运行、  
  ⧉ 复制、🗑 删除按钮——运行即时执行,复制到剪贴板,删除移除该条。
- **Default-collapse settings for the sidebars (#78).** Settings → Interface →  
  Sidebars adds two checkboxes to collapse the left resource panel and the bottom  
  SFTP panel on startup — handy on low-spec jump hosts.  
  **侧栏默认收起设置 (#78)。** 设置 → 界面 → 侧栏 新增两个复选框,可在启动时  
  收起左侧资源面板和底部 SFTP 面板——适合低配跳板机。

## [0.4.0] - 2026-06-14

### Added / 新增

- **SSH port forwarding / tunnels (#56).** Per-session tunnels configured in the  
  session dialog's Advanced section: local (-L), remote (-R) and dynamic  
  (-D / SOCKS5). They auto-establish on connect and tear down on disconnect.  
  **SSH 端口转发 / 隧道 (#56)。** 在会话对话框「高级」里按会话配置:本地 -L、  
  远程 -R、动态 -D（SOCKS5）。连接时自动建立,断开时拆除。
- **Quick commands, command box & history (#55).** A command bar below the  
  terminal: save named commands and click to run them, type into a command box  
  (with an "all sessions" broadcast toggle), and recall history with ↑/↓.  
  **快捷命令、命令输入框与历史 (#55)。** 终端下方的命令栏:保存命名命令点击即发、  
  命令输入框（含「所有会话」群发开关）、↑/↓ 回溯历史。
- **Remote process monitor (#23).** The server-resource panel gains a "Processes"  
  button that opens a read-only table (PID / user / CPU% / MEM% / command), sorted  
  by CPU and refreshed live.  
  **远端进程监控 (#23)。** 服务器资源面板新增「进程」按钮,打开只读进程表  
  （PID / 用户 / CPU% / 内存% / 命令）,按 CPU 排序、实时刷新。
- **Encrypted private keys (#90).** Key auth now accepts a passphrase for  
  encrypted private keys.  
  **加密私钥 (#90)。** 私钥认证支持为加密私钥输入密码短语。

### Fixed / 修复

- **CJK rendering (#54).** Chinese (especially isolated punctuation like 、：（）)  
  no longer renders as tofu in editable inputs or the terminal — editable fields  
  and the window now use a CJK-capable font, and CJK terminal spans fall back  
  correctly.  
  **中文渲染 (#54)。** 输入框和终端里的中文（尤其「、：（）」这类孤立标点）不再  
  显示为方块——可编辑控件与窗口改用支持 CJK 的字体,终端的 CJK 片段也正确回退。
- **SFTP cd-follow under zsh (#91).** The SFTP panel now follows `cd` under zsh  
  (and other shells), not just bash — the cwd notification is registered via the  
  shell's proper hook instead of bash's `PROMPT_COMMAND` only.  
  **zsh 下 SFTP 跟随 cd (#91)。** SFTP 面板现在在 zsh（及其它 shell）下也能跟随  
  `cd`,不再只支持 bash——按各 shell 正确的钩子注册 cwd 通知。
- **Compact, scrollable session dialog.** Proxy and port-forwarding settings are  
  collapsed under an "Advanced" toggle, and the dialog scrolls instead of being  
  clipped when it would exceed the window.  
  **会话对话框更紧凑、可滚动。** 代理与端口转发收进「高级」折叠;对话框超出窗口  
  时内部滚动而非被截断。

## 0.3.8 - 2026-06-12

### Added / 新增

- **Confirm before closing when there are active sessions (#88).** Double-clicking  
  the title-bar icon (or X / Alt+F4) no longer silently drops live sessions — a  
  confirm dialog appears; with no sessions the window closes as before.  
  **有活动会话时关闭前先确认 (#88)。** 双击标题栏图标(或点 X / Alt+F4)不再静默  
  断开正在进行的会话——会弹出确认框;没有会话时则照旧直接关闭。
- **"Always ask where to save" download option (#87).** Settings → Interface →  
  Download adds a checkbox (default off); when on, every download prompts for the  
  folder instead of using the preset.  
  **「总是询问保存去何处」下载选项 (#87)。** 设置 → 界面 → 下载 新增复选框  
  (默认关闭);勾选后每次下载都询问保存位置,而非直接用预设目录。
- **The transfers popup opens automatically when a download starts**, so progress  
  is visible without opening it by hand.  
  **下载开始时自动弹出传输面板**,无需手动打开即可看到进度。
- **Capped diagnostic log file (groundwork for #86).** Writes to  
  `<config_dir>/error.log` at WARN and above — a single file capped at 5 MiB that  
  auto-overwrites when full — so users can share what went wrong (e.g. a bastion  
  disconnect reason) without setting RUST_LOG.  
  **容量受限的诊断日志文件(为 #86 铺路)。** 写入 `<配置目录>/error.log`  
  (WARN 及以上)——单文件、上限 5 MiB、满了自动覆盖——用户无需设置 RUST_LOG 即可  
  把出错信息(如堡垒机断开原因)发来。

### Fixed / 修复

- **Settings checkboxes now persist visually after reopening the dialog (#87).**  
  "Always ask where to save" and "SFTP follows cd" used a one-way binding, so  
  reopening the settings dialog (without restarting) showed the stale state even  
  though the value was saved; switched to a two-way binding.  
  **设置里的复选框重新打开对话框后状态保持 (#87)。** 「总是询问保存去何处」和  
  「SFTP 跟随 cd」原用单向绑定,不重启 app 时重开设置对话框会显示旧状态(尽管值  
  已保存);改为双向绑定。

## 0.3.7 - 2026-06-12

### Added / 新增

- **Right-click anywhere in the SFTP file list (#84).** The list fills the panel  
  height, so right-clicking the whitespace below the items works too; item  
  actions grey out when nothing was hit, leaving new folder / new file / refresh.  
  **SFTP 文件列表任意处可右键 (#84)。** 列表填满面板高度,条目下方空白也能右键;  
  未点中条目时,条目相关操作置灰,仅保留 新建文件夹 / 新建文件 / 刷新。
- **Visual permissions dialog (#84).** Permissions opens a checkbox matrix  
  (Owner/Group/Other × Read/Write/Execute) prefilled from the file's current  
  mode, instead of typing an octal string.  
  **可视化权限对话框 (#84)。** 「权限」打开勾选矩阵(所有者/组/其他 × 读取/写入/  
  执行),按文件当前权限预填,不再手输八进制。
- **Open / Edit externally (#81).** New SFTP context-menu items hand the file to  
  the OS default app (e.g. VS Code) for syntax highlighting / large files; edit  
  mode watches the temp copy and re-uploads on change.  
  **外部程序查看 / 编辑 (#81)。** SFTP 右键新增菜单项,把文件交给系统默认程序  
  (如 VS Code)打开以获得语法高亮 / 处理大文件;编辑模式监听临时副本并自动重传。
- **Line numbers in the built-in editor (#81).**  
  **内置编辑器加行号 (#81)。**
- **Upload a whole folder from the Upload button (#85).** The button now offers  
  "Upload file" (multi-select) / "Upload folder".  
  **上传按钮支持整个文件夹 (#85)。** 现在提供「上传文件」(可多选) /「上传文件夹」。
- **Linux ARM64 release build (#82)** and **AUR packaging scaffolding (#61).**  
  **Linux ARM64 发布构建 (#82)** 与 **AUR 打包脚手架 (#61)。**

### Changed / 变更

- **Downloads default to the user's Downloads folder (#85)** instead of prompting  
  every time; the settings button reads "Choose save path".  
  **下载目录默认设为用户的「下载」文件夹 (#85)**,不再每次询问;设置按钮文案改为  
  「选择保存路径」。

### Fixed / 修复

- **No more error under fish (#71).** The OSC 7 prompt injection is guarded with  
  `test -z "$FISH_VERSION"`, so it's a no-op under fish (which emits OSC 7 on its  
  own, keeping cd-follow working) and unchanged under bash/zsh/sh.  
  **fish 下不再报错 (#71)。** OSC 7 提示符注入加了 `test -z "$FISH_VERSION"` 守卫,  
  在 fish 下为空操作(fish 自带 OSC 7,cd 跟随照常),bash/zsh/sh 行为不变。

## [0.3.5] - 2026-06-12

- 我和claude都快冒烟了，不想写，让我摆烂一会吧。Claude and I are both about to explode with frustration. We don't want to write anymore. Let me just laze around for a while.

## 0.3.3 - 2026-06-11

### Added / 新增

- **In-app new-version notification (#48).** On startup a background thread  
  checks the GitHub releases API; if a newer version exists, a dismissible  
  top-centre banner offers a Download button (opens the release page). Purely  
  informational — the app keeps working on the current version, never forced.  
  **应用内新版本通知 (#48)。** 启动时后台线程查询 GitHub releases API,若有更  
  新版本则在顶部居中显示一个可关闭的横幅,带"下载"按钮(打开 release 页)。纯  
  提示性质——应用照常在当前版本运行,绝不强制更新。
- **Editable SFTP path bar with copy & paste-to-jump (#54).** The path bar is  
  now an input: type or paste a path and press Enter to jump there, plus a copy  
  button and a paste-and-jump button.  
  **可编辑的 SFTP 路径栏,支持复制和粘贴跳转 (#54)。** 路径栏现在是输入框:输入  
  或粘贴路径后回车即跳转,另有复制按钮和粘贴跳转按钮。

### Fixed / 修复

- **SFTP panel no longer gets stuck "loading" after manual navigation (#59).**  
  The panel only follows a real cd now (cwd actually changed), not the OSC 7 that  
  every prompt re-emits — so manual browsing isn't overridden and a later cd  
  reloads correctly instead of hanging.  
  **SFTP 面板手动导航后不再卡"加载中" (#59)。** 面板现在只跟随真正的 cd(cwd 确  
  实变化),而非每个命令提示符都重发的 OSC 7——手动浏览不被覆盖,之后的 cd 也能  
  正确重新加载而非卡住。
- **SFTP path bar renders Chinese instead of tofu (#54).** The embedded Cascadia  
  Mono has no CJK glyphs and native TextInput doesn't glyph-fallback; editable  
  inputs now use a CJK-capable system font.  
  **SFTP 路径栏正常显示中文,不再是豆腐块 (#54)。** 嵌入的 Cascadia Mono 没有  
  CJK 字形,而原生 TextInput 不做字形回退;可编辑输入现改用含 CJK 的系统字体。

## 0.3.2 - 2026-06-11

### Added / 新增

- **MSI installer for Windows (best-effort).** Built by cargo-wix with a  
  WixUI_InstallDir wizard, so you can change the install location during setup.  
  **Windows MSI 安装包(尽力而为)。** 由 cargo-wix 构建,带 WixUI_InstallDir  
  向导,安装时可更改安装位置。
- **Explicit session folders (#41).** Groups are now first-class: create / rename  
  / delete them, keep empty folders, and right-click a group header to manage it.  
  Right-click a session to "move to" any group (incl. empty ones).  
  **显式会话文件夹 (#41)。** 分组成为一等公民:可新建 / 重命名 / 删除,可保留空  
  文件夹,右键分组标题进行管理;右键会话可"移动到"任意分组(含空文件夹)。

### Changed / 变更

- **Interface settings dialog** is now draggable (by its title bar), truly modal  
  (background click no longer closes it), and its font controls no longer span  
  the full pane.  
  **「界面」设置对话框**现在可拖动(拖标题栏)、真模态(点背景不再关闭),字体控件  
  也不再占满整个面板。
- **Session right-click menu reordered:** Edit / Duplicate / Delete above the  
  divider, the "move to group" list below it with a header hint.  
  **会话右键菜单重排:** 编辑 / 复制副本 / 删除在分割线上方,"移动到分组"列表带  
  提示在下方。

### Fixed / 修复

- **Wide (CJK) characters no longer misalign the cursor (#60).** A wide glyph's  
  blank continuation cell was being filled with a space, pushing the line and  
  cursor one cell right per character; it now emits nothing.  
  **宽(CJK)字符不再导致光标错位 (#60)。** 宽字形的空白延续格此前被补了空格,  
  每个字符把行和光标右推一格;现在延续格不输出任何内容。
- **Download & settings popups close on click-outside.** A full-window backdrop  
  under each popup closes it when you click outside.  
  **下载/设置弹窗点击外部即关闭。** 每个弹窗下方铺一层全窗背景,点击外部即关闭。
- **Disk tooltip clears when the pointer leaves the panel**, and the OSC 7  
  shell-integration command no longer leaves an extra blank prompt on connect.  
  **磁盘 tooltip 在指针离开面板时消失**,OSC 7 shell 集成命令也不再在连接时留下  
  多余的空提示符。

## 0.3.1 - 2026-06-10

### Security / 安全

- **Restrict sessions.json to owner-only on Unix (#34).** The config file holds  
  (encrypted) credentials, so it is now written with mode 0600 — like  
  secret.key — and other local accounts can't read it. Windows %APPDATA% is  
  already owner-restricted by default ACLs.  
  **将 sessions.json 限制为仅属主可读(Unix)(#34)。** 配置文件含(加密的)凭据,  
  现在以 0600 权限写入(与 secret.key 一致),其它本地账户无法读取。Windows 的  
  %APPDATA% 默认 ACL 已限制为属主。

### Build / 构建

- **Build macos-x86_64 by cross-compiling on Apple Silicon runners.** The  
  dedicated Intel (macos-13) runners queue for ages and often time out, so the  
  x86_64 Mac binary is now cross-compiled on a plentiful macos-14 runner.  
  **在 Apple Silicon runner 上交叉编译 macos-x86_64。** 专用 Intel(macos-13)  
  runner 排队极久且常超时,x86_64 Mac 二进制改为在充足的 macos-14 runner 上  
  交叉编译。

## 0.3.0 - 2026-06-10

### Added / 新增

- **Interface settings — terminal font & size.** The gear menu's new "Interface"  
  item opens a modal dialog (27% nav / 73% content) whose Font page lets you pick  
  from the system's installed monospace fonts and set the size (8–32 px) with a  
  live preview. Both apply immediately (cell size / cols / rows re-derive) and  
  persist.  
  **「界面」设置 —— 终端字体与字号。** 齿轮菜单新增「界面」项,打开模态对话框  
  (27% 导航 / 73% 内容),「字体」页可从系统已安装的等宽字体中选择并设置字号  
  (8–32 px),带实时预览。两者即时生效(cell 尺寸 / 列 / 行重新推导)并持久化。
- **Session folders / groups (#41).** Each session can belong to an optional  
  group; Quick Connect shows folder headings (sorted; ungrouped under "default")  
  that collapse when clicked. Right-click a session to move it to another group  
  or duplicate it; right-click a group header to create a new group.  
  **会话文件夹 / 分组 (#41)。** 每个会话可归入可选分组;「快速连接」按分组显示  
  文件夹标题(排序;未分组归入「default」),点击标题可折叠。右键会话可移动到其它  
  分组或复制副本;右键分组标题可新建分组。
- **Recursive SFTP folder transfer (#50).** Upload, download and delete whole  
  directory trees: drag a folder onto the panel to upload, right-click a folder  
  to download, and delete now removes non-empty directories too.  
  **SFTP 文件夹递归传输 (#50)。** 可上传、下载、删除整个目录树:把文件夹拖到面板  
  上传,右键文件夹下载,删除现在也能删非空目录。
- **Collapsible panels (#41).** Both the left sidebar and the SFTP panel can be  
  minimized to reclaim screen space.  
  **可折叠面板 (#41)。** 左侧栏与 SFTP 面板都可最小化以腾出屏幕空间。
- **Export / import connections (#46).** New "Export connections" / "Import  
  connections" in the settings menu to migrate sessions between machines. The  
  exported JSON keeps host/user/port in plaintext and obfuscates only the  
  password with a built-in key, so it opens on any machine; key-auth sessions  
  export the key path. Imports skip duplicates (host+user+port+kind).  
  **导出 / 导入连接 (#46)。** 设置菜单新增「导出连接」「导入连接」,用于在多台  
  机器间迁移会话。导出的 JSON 中 host/user/port 为明文,仅密码用内置 key 混淆,  
  因此在任意机器都能打开;密钥认证的会话导出私钥路径。导入会跳过重复  
  (host+user+port+kind)。
- **Pick SOCKS5 or HTTP proxy type in the session dialog (#46).** The proxy  
  field is now a None / SOCKS5 / HTTP selector plus a `host:port` input. HTTP  
  CONNECT was already supported by the backend but wasn't selectable in the UI.  
  The stored proxy URL format is unchanged.  
  **会话对话框选择 SOCKS5 或 HTTP 代理类型 (#46)。** 代理项改为 不使用 / SOCKS5 /  
  HTTP 选择器加 `host:port` 输入框。HTTP CONNECT 后端早已支持,只是 UI 无法选择。  
  存储的代理 URL 格式不变。
- **Confirmation prompt before deleting a remote file (#28).** SFTP delete is  
  irreversible (there is no trash), so the context-menu *Delete* now asks for  
  confirmation — showing the full path — before removing anything; a misclick  
  no longer silently destroys a file.  
  **删除远程文件前先确认 (#28)。** SFTP 删除不可撤销(没有回收站),右键菜单的  
  「删除」现在会先弹出确认框(显示完整路径)再执行,误点不会再悄悄删掉文件。
- **Serial port sessions (#14, #17).** New session type for connecting to  
  switches, routers and embedded devices over a serial console. Pick  
  **Serial** in the session dialog and set the port (`COM3`, `/dev/ttyUSB0`),  
  baud rate, data/stop bits, parity and flow control. The serial line reuses  
  the full terminal pipeline (output, input, scrollback, copy/paste); SFTP and  
  the resource monitor are not applicable and are hidden.  
  **串口会话 (#14, #17)。** 新增串口会话类型,用于通过串口控制台连接交换机、  
  路由器和嵌入式设备。在会话对话框选择 **串口**,填写串口号(`COM3`、  
  `/dev/ttyUSB0`)、波特率、数据/停止位、校验位和流控。串口复用完整的终端管线  
  (输出、输入、回滚、复制粘贴);SFTP 和资源监控不适用,已隐藏。
- **Telnet sessions (#17).** New session type for legacy gear that only speaks  
  Telnet. Handles RFC 854 option negotiation (suppress-go-ahead / echo /  
  window-size), strips IAC sequences from the stream, and tunnels through the  
  same SOCKS5 / HTTP proxy as SSH when configured.  
  **Telnet 会话 (#17)。** 新增 Telnet 会话类型,用于只支持 Telnet 的老旧设备。  
  处理 RFC 854 选项协商(抑制 Go-Ahead / 回显 / 窗口大小),从数据流中剥离 IAC  
  序列,并可经与 SSH 相同的 SOCKS5 / HTTP 代理隧道连接。

### Performance / 性能

- **Pipelined SFTP upload (#16).** Uploads now keep ~32 WRITE requests in flight  
  on a dedicated SFTP channel instead of writing one chunk and waiting for each  
  ack, hiding the round-trip latency that made transfers ~15x slower than `scp`.  
  Out-of-order completion is safe (every chunk carries its absolute offset).  
  **SFTP 上传流水线化 (#16)。** 上传改为在专用 SFTP 通道上保持约 32 个 WRITE 请求  
  并发在途,而不是写一块等一块的 ack,消除了让传输比 `scp` 慢约 15 倍的往返延迟。  
  乱序完成也安全(每块都带绝对偏移)。

### Fixed / 修复

- **Drag-select no longer auto-scrolls within the visible area (#41).** Selecting  
  text now only scrolls once the drag leaves the viewport edge, so the view no  
  longer jumps and the selection no longer snaps to an edge row.  
  **拖动选择在可见区内不再自动滚动 (#41)。** 现在仅当拖动离开视口边缘才滚动,  
  视图不再乱跳、选区也不再吸附到边缘行。
- **Alt no longer clears the typed command (#43).** Slint encodes a lone  
  modifier key as a C0 code point (Alt=0x12); pressing Alt (e.g. to Alt+Tab  
  away) sent ESC+0x12 to the PTY, which bash/readline treated as Meta and  
  discarded the input line. Bare modifier codes are now dropped, with a guard  
  that preserves a real Ctrl+P..Ctrl+X.  
  **按 Alt 不再清空已输入命令 (#43)。** Slint 把单独的修饰键编码成 C0 码位  
  (Alt=0x12);按 Alt(如 Alt+Tab 切换)会向 PTY 发送 ESC+0x12,被 bash/readline  
  当作 Meta 而丢弃输入行。现在丢弃单独的修饰键码位,并保留真实的 Ctrl+P..Ctrl+X。
- **Multi-line / backslash-continued commands now paste intact.** Pasted text  
  kept its CRLF/LF line breaks, but the terminal expects CR for Enter, so CRLF  
  made the shell see two breaks per line and end a `\`-continued command early.  
  Pasted line endings are normalised to a single CR.  
  **多行 / 反斜杠续行命令现在能完整粘贴。** 粘贴文本保留了 CRLF/LF 换行,而终端  
  回车应为 CR,CRLF 会让 shell 每行看到两个换行、提前结束 `\` 续行命令。现在把  
  粘贴的换行统一规范为单个 CR。
- **Session dialog no longer mis-lays-out when switching connection type.** The  
  card had a fixed height, so Telnet/Serial (with fewer fields) left slack space  
  that stretched inputs apart. The card height now follows its content.  
  **切换连接类型时会话对话框不再排版错乱。** 卡片此前固定高度,Telnet/串口  
  (字段更少)会留出空白把输入框撑开。现在卡片高度跟随内容。
- **Copy/paste works on Wayland sessions (#47).** arboard's default Linux  
  backend is X11, which fails on Wayland (Debian sid / KDE) without XWayland.  
  Enabled arboard's native `wayland-data-control` backend, and copy now uses  
  set().wait() so the selection survives after the clipboard handle is dropped.  
  **Wayland 会话下复制粘贴恢复可用 (#47)。** arboard 默认 Linux 后端是 X11,在  
  无 XWayland 的 Wayland(Debian sid / KDE)下失效。启用 arboard 原生  
  `wayland-data-control` 后端,复制改用 set().wait() 使选区在剪贴板句柄 drop 后  
  仍然有效。
- **Hide the shell-integration command from the terminal.** meatshell injects a  
  one-line `PROMPT_COMMAND` (OSC 7) on connect so the SFTP panel can follow the  
  terminal's working directory. Its echo used to show up on every connect (and  
  pollute shell history); the line now carries a leading space (kept out of  
  history) and its echo is stripped from the output before display.  
  **隐藏 shell 集成注入命令。** meatshell 连接时会注入一行 `PROMPT_COMMAND`  
  (OSC 7),让 SFTP 面板跟随终端当前目录。此前它的回显每次连接都显示在终端  
  (并污染命令历史);现在该行带前导空格(不进历史),回显也会在显示前被剥离。
- **Dragging the SFTP panel up no longer clears terminal output (#18).** vt100's  
  shrink truncated the grid from the bottom, dropping the most recent output;  
  before shrinking we now save the top rows to scrollback and scroll so the  
  bottom (recent) rows stay visible. Two follow-ups: (1) the shrink now only  
  scrolls off as many rows as needed to keep the cursor visible, so rapid  
  up/down dragging on a not-yet-full screen no longer pushes the prompt into  
  scrollback and strands the cursor at the top (also reported as #24); (2) drag-selection is now stored  
  in absolute scrollback coordinates, so selecting from the top of the history  
  down through several screens copies every line instead of losing everything  
  above the final window when the view auto-scrolls.  
  **上拉 SFTP 面板不再清空终端输出 (#18)。** vt100 缩小时从底部截断,丢掉最近输出;  
  现在缩小前把顶部行存入回滚区并滚动,使底部(最近)行保持可见。两处后续修复:  
  (1) 缩小时只滚走"保持光标可见所需"的行数,疯狂上下拖动未填满的屏幕时不再把  
  提示符推进回滚区、光标卡在顶部;(2) 拖选改用绝对回滚坐标存储,从历史顶部往下  
  跨多屏选择时能复制到每一行,而不是在视图自动滚动后丢掉最后一屏以上的内容。

### Security / 安全

- **Redact proxy credentials and zero them in memory (#32).** The HTTP/SOCKS  
  proxy password is now wrapped in `Secret` (zeroed on drop) and ProxyConfig has  
  a manual Debug that redacts auth, so credentials can't leak via {:?}/tracing  
  or linger in core dumps.  
  **代理凭据脱敏并在内存清零 (#32)。** HTTP/SOCKS 代理密码改用 `Secret` 包装  
  (drop 时清零),ProxyConfig 手写 Debug 对凭据脱敏,使其无法经 {:?}/tracing  
  泄露或残留于 core dump。
- **Validate HostName when importing ~/.ssh/config (#33).** Imported HostName  
  values are now checked — IP literals and DNS hostnames accepted; shell  
  metacharacters, whitespace and scheme prefixes rejected — and invalid entries  
  are skipped with a warning.  
  **导入 ~/.ssh/config 时校验 HostName (#33)。** 现在校验导入的 HostName(接受  
  IP 字面量与 DNS 域名,拒绝 shell 元字符、空白与协议前缀),非法条目跳过并告警。
- **Harden the remote resource monitor against a hostile server (#27).** The  
  monitor runs a small loop over an SSH exec channel. It now (1) resets `PATH`  
  to the standard system dirs so a server with a hijacked `PATH`/`BASH_ENV`  
  can't shadow `awk`/`cat`/`df`/`sleep`; (2) caps the reassembly buffer at 1 MiB  
  so a server that streams data without the sync marker can't exhaust memory;  
  and (3) parses `/proc` and `df` output with saturating arithmetic and a  
  64-row cap per sample, so crafted huge values or a flood of fake interfaces  
  can't overflow-panic or swamp the sidebar.  
  **加固远程资源监控以防恶意服务器 (#27)。** 监控通过 SSH exec 通道跑一个小循环。  
  现在:(1) 重置 `PATH` 为标准系统目录,使被劫持 `PATH`/`BASH_ENV` 的服务器无法  
  替换 `awk`/`cat`/`df`/`sleep`;(2) 重组缓冲上限 1 MiB,防止只发数据不发同步标记  
  的服务器耗尽内存;(3) 解析 `/proc` 与 `df` 输出改用饱和运算并对每次采样限 64 行,  
  使构造的超大数值或伪造网卡洪流无法触发溢出 panic 或拖垮侧栏。
- **Sanitize remote file names before saving downloads (#26).** SFTP downloads  
  built the local path straight from the server-supplied name, so a malicious  
  server could use path separators, shell-special characters or a Windows  
  reserved device name (`CON`, `NUL`, `COM1`…) to write outside the chosen  
  folder or hit a device. Downloads now run the name through `sanitize_filename`  
  (already used by the open/edit flow), which also gained reserved-device-name  
  and leading-whitespace handling.  
  **保存下载前清洗远程文件名 (#26)。** SFTP 下载直接用服务器给的文件名拼本地路径,  
  恶意服务器可借路径分隔符、shell 特殊字符或 Windows 保留设备名(`CON`、`NUL`、  
  `COM1`…)写到目标目录之外或命中设备。现在下载会先经 `sanitize_filename`  
  (查看/编辑流程已在用)清洗,并新增了保留设备名与前导空白的处理。
- **Stop logging raw keystroke bytes (#15).** Debug logs recorded the hex of SSH  
  input, which could include passwords; now they record only the byte length.  
  A follow-up found two more leak sites in the key handler: `send_key` logged  
  the raw key string (`key={:?}`) at debug level, and the `[KEY_DIAG]` IME  
  diagnostic logged each Shift-typed key's code point at **info** level (no  
  `RUST_LOG` needed) — both could expose password characters. They now go  
  through a `redact_key` helper that reveals only C0/C1 control codes (what the  
  IME diagnostics actually need) and masks every printable character.  
  **不再记录原始按键字节 (#15)。** debug 日志原本记录 SSH 输入的十六进制(可能含  
  密码),现在只记录字节长度。后续又发现按键处理里还有两处泄露:`send_key` 以  
  debug 级打印按键原文(`key={:?}`),`[KEY_DIAG]` IME 诊断更是以 **info 级**  
  (无需 `RUST_LOG`)打印每个带 Shift 按键的码位——都可能暴露密码字符。现在统一  
  经 `redact_key` 处理,只保留 C0/C1 控制码(IME 诊断真正需要的),可打印字符一律掩码。

## [0.2.3] - 2026-06-05

### Added / 新增

- **Proxy support for SSH / SFTP (#7).** Connections can tunnel through a  
  **SOCKS5** (`socks5://`) or **HTTP CONNECT** (`http://`) proxy, with optional  
  `user:pass@` credentials. Set it per session in the dialog, or leave it blank  
  to use the `$ALL_PROXY` environment variable; empty = direct.  
  **SSH / SFTP 代理支持 (#7)。** 连接可经 **SOCKS5**(`socks5://`)或  
  **HTTP CONNECT**(`http://`)代理(支持 `user:pass@` 认证)。会话对话框里按需  
  填写,留空则用 `$ALL_PROXY` 环境变量,再空则直连。
- **Import hosts from `~/.ssh/config` (#1).** The "Import ~/.ssh/config" action  
  (in the settings menu) parses the standard SSH config (`Host` / `HostName` /  
  `User` / `Port` / `IdentityFile`, wildcard `Host *` blocks skipped) and adds  
  each host as a session, skipping duplicates. Hosts with an `IdentityFile`  
  default to key auth.  
  **从 `~/.ssh/config` 导入主机 (#1)。** 设置菜单里的「导入 ~/.ssh/config」解析  
  标准 SSH 配置(`Host` / `HostName` / `User` / `Port` / `IdentityFile`,跳过  
  `Host *` 通配块),将每个主机加为会话并跳过重复;带 `IdentityFile` 的默认用密钥。
- **GitHub Actions release workflow** building native binaries for Windows /  
  Linux / macOS (arm64 + x86_64) on each `v*` tag.  
  **GitHub Actions 发布工作流**,每个 `v*` 标签自动构建 Windows / Linux /  
  macOS(arm64 + x86_64)三平台二进制。

### Fixed / 修复

- The full-width `＋` before "New session" rendered as a tofu box in English;  
  switched to an ASCII `+`.  
  英文下「New session」前的全角 `＋` 显示为豆腐块,改用 ASCII `+`。
- `install-linux.sh` now auto-detects the `meatshell` binary sitting next to it  
  in a release package, so it works with no arguments (it previously defaulted to  
  the source-tree `./target/release` path and failed for end users).  
  `install-linux.sh` 现在自动识别发布包里同目录的 `meatshell`,无需传参即可使用  
  (之前默认指向源码树的 `./target/release`,普通用户直接跑会报错)。

## 0.2.2 - 2026-06-05

### Security / 安全

- **Fix Windows command injection (#12)** — `open_with_os` no longer shells out  
  via `cmd /C start`; it calls `ShellExecuteW` directly so a malicious remote  
  file name (e.g. `foo&calc.exe`) can't inject commands. Added `sanitize_filename`  
  as defence-in-depth.  
  **修复 Windows 命令注入 (#12)** —— 打开文件不再经 `cmd /C start`，改用  
  `ShellExecuteW` 直接打开，恶意远程文件名（如 `foo&calc.exe`）无法注入命令；  
  并新增 `sanitize_filename` 清洗作为纵深防御。
- **Stop echoing the saved password when editing a session (#10)** — the field  
  is left blank with a "leave blank to keep" hint; an empty field on save keeps  
  the existing password.  
  **编辑会话时不再回显已保存密码 (#10)** —— 密码框留空并提示「留空则不修改」，  
  保存时为空则保留原密码。
- **Zero passwords in memory on drop (#8)** — passwords now use a `Secret` type  
  (`zeroize`) that wipes its heap buffer on drop and redacts itself in logs; the  
  on-disk JSON format is unchanged.  
  **密码内存清零 (#8)** —— 密码改用 `Secret` 类型（`zeroize`），Drop 时清零堆  
  内存、日志中脱敏；磁盘 JSON 格式不变。

### Added / 新增

- **Internationalization — Chinese / English with runtime switching (#9).**  
  Static UI uses Slint `@tr` + bundled `.po`; dynamic Rust strings use a `t()`  
  helper. Switch via the gear menu; the choice is persisted and the default  
  follows the system locale.  
  **国际化 —— 中 / 英双语，运行时实时切换 (#9)。** 静态界面用 Slint `@tr` +  
  bundled `.po`；Rust 动态文本用 `t()`。设置菜单里切换，选择会持久化，首次启动  
  跟随系统语言。
- **Private-key file picker** in the session dialog, plus `.pub` fallback (auto  
  strips the suffix to load the matching private key) and uniform `/` path  
  separators across platforms.  
  **会话弹窗的私钥文件选择器**，并支持 `.pub` 容错（自动去后缀加载对应私钥）、  
  路径分隔符统一为 `/`。
- **Linux desktop integration** — `assets/meatshell.desktop` + `install-linux.sh`  
  and an `xdg_app_id` so the GNOME/Ubuntu dock shows the app icon on Wayland.  
  **Linux 桌面集成** —— `assets/meatshell.desktop` + `install-linux.sh`，并设置  
  `xdg_app_id`，使 Wayland 下 GNOME/Ubuntu 任务栏显示应用图标。
- **Screenshots in the README** (`docs/screenshots/`, sensitive info redacted).  
  **README 增加截图**（`docs/screenshots/`，敏感信息已打码）。
