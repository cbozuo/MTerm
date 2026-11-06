# 多行粘贴确认 · 竞品与协议调研（2026-11-06）

范围：为 `docs/design/multiline-paste-spec-2026-11-06.md` §5.3 的 D1–D5 提供外部证据。
证据等级：源码文件行 > 官方文档/发行说明 > 博客。凡本次未能打开原文的，一律标注「未证实」，不填默认值、不描述未见的界面。

## ① 结论速览表

| 产品 | 机制 | 默认焦点 | 是否可记忆 |
| --- | --- | --- | --- |
| Windows Terminal | 多行粘贴 ContentDialog（`Automatic` 下仅当 bracketed paste 关闭才弹）＋独立的 5 KiB 大粘贴对话框 | Primary「Paste anyway」（`DefaultButton="Primary"`） | 无复选框；只有 profile 设置项 |
| iTerm2 | `iTermWarning` 多行粘贴警告，附「Paste Without Newline」按钮；大文本另有「Paste Limit Exceeded」 | 首选动作 `Paste`（按代码顺序） | 有：按 identifier 永久/临时静默；无复选框证据（未证实） |
| VTE / GNOME Terminal | 无确认框：净化控制码 + 换行→CR + bracketed 包裹 | 不适用 | 不适用 |
| PuTTY | 无确认框：默认过滤控制字符，另可整体关闭 bracketed paste | 不适用 | 不适用 |
| kitty | `paste_actions` 动作集合，默认 `quote-urls-at-prompt,confirm`；含终端控制码才确认 | 未证实（未读到弹框代码） | 有：改 `paste_actions` 去掉 `confirm` |
| WezTerm | 无确认框，仅 `canonicalize_pasted_newlines` | 不适用 | 不适用 |
| VS Code | 多行粘贴 `prompt`（附「Paste as one line」）＋「Do not ask me again」复选框 | 未证实（代码未声明） | 有：勾选后持久化为 `never` |
| Tabby | `showMessageBox` warning（仅非 alternate screen） | `defaultId: 0` → Paste | 无复选框；只有设置开关 |
| MobaXterm | 设置项「Warn before pasting multiple lines」的确认消息 | 未证实 | 有：设置项可关 |
| FinalShell | 未证实（官网更新日志未见；源码不可得） | 未证实 | 未证实 |
| 阿里云 Workbench | 未证实（官方概览未记载） | 未证实 | 未证实 |
| bracketed paste 协议 | `ESC[200~` 包裹文本 + `ESC[201~`，程序据此区分粘贴与键入 | 不适用 | 不适用 |

## ② 逐产品条目

### 1. Windows Terminal
- 机制：`warnMultiLine` 与 `warnLargeText` 两条独立判据，共用一个 ContentDialog 通道；取消即中止粘贴。
- 触发条件：`windowSettings.WarnAboutMultiLinePaste()` 为 `Automatic` 时 `warnMultiLine = !bracketedPaste`（即 bracketed 开着就不警告），`Always`→true，`Never`→false，且文本必须含 `\r`/`\n`；大文本判据为 `minimumSizeForWarning = 1024 * 5` 且 `WarnAboutLargePaste` 为真。来源：`TerminalPage.cpp:3171-3195`。
- 界面形态：窄 ContentDialog，文案「You are about to paste text that contains multiple lines. If you paste this text into your shell, it may result in the unexpected execution of commands. Do you wish to continue?」，标题 `Warning`；正文用 `MaxHeight="100"` 的 ScrollViewer 显示剪贴板文本（Cascadia Mono），超长会被截断加省略号。来源：`TerminalPage.xaml:88-104`、`Resources.resw:549-572`、`TerminalPage.cpp:3197-3215`。
- 按钮与默认焦点：`Paste anyway`（Primary）/ `Cancel`（Close）；`DefaultButton="Primary"` → 默认焦点在「粘贴」。对照：`CloseReadOnlyDialog` 用 `DefaultButton="Close"`。来源：`TerminalPage.xaml:78, :86, :88`。
- 键盘：微软文档对 `DefaultButton` 的定义逐字为——「The button will respond to the ENTER key automatically」「When the user presses the ENTER key on the keyboard, the click handler associated with the Default Button will fire」「The button will receive focus automatically when the Dialog is opened unless the dialog's content contains focusable UI」；同页要求「your dialog has at least one button corresponding to a safe, nondestructive action like "Got it!", "Close", or "Cancel"」（即 CloseButton = 安全出口）。Esc→Close 的绑定未在该页证实。来源：MS Learn「Dialog controls」。
- 可选项：`warning.multiLinePaste`（`Automatic`/`Always`/`Never`）、`warning.largePaste`（bool）。旧全局键 `multiLinePasteWarning`/`largePasteWarning` 已迁移到 profile。来源：`MTSMSettings.h:60-61`、`GlobalAppSettings.cpp:29-30, :169-170`、`EventArgs.idl:41-46`。
- 为什么这样做：微软自己的注释承认这条规则不安全——「a shell that doesn't support bracketed paste will allow an attacker to enable the mode, not realize that, and then accept the paste as if it was a series of legitimate commands. See GH#13014.」`TerminalPage.cpp:3174-3179`；GH#13014 正是一个用 OSC 52 写剪贴板 + `ESC[?2004h` 绕过警告的实例。这是本项目最重要的反面教材。

### 2. iTerm2
- 机制：`- (BOOL)maybeWarnAboutMultiLinePaste:(PasteEvent *)pasteEvent` 统一裁决；先看超大文本，再看多行，再按「是否在 shell 提示符」细分文案与记忆键。
- 触发条件：`kPasteFlagsRemovingNewlines` 置位则不问；文本无换行不问；`suppressMultilinePasteWarningWhenPastingOneLineWithTerminalNewline` 且换行在末尾不问；`suppressMultilinePasteWarningWhenNotAtShellPrompt` 且不在提示符不问；`pasteEvent.suppressMultilinePasteWarning` 置位不问；超大文本用 `alwaysWarnBeforePastingOverSize` 字符数阈值。来源：`iTermPasteHelper.m:878-938`。
- 界面形态：`iTermWarning` 警告面板（非 ContentDialog），正文直接给行数/字符数与记忆键；行数>1 时标题「OK to paste %ld lines at shell prompt?」或「OK to paste %ld lines?」，0/1 行时标题为「OK to paste one line ending in a newline at shell prompt?」；大文本标题「Paste Limit Exceeded」+「OK to paste %ld characters?」。来源：`iTermPasteHelper.m:880-895, :957-975`。
- 按钮与默认焦点：多行场景动作顺序 `[Paste, Cancel]`；≤1 行时插入 `Paste Without Newline`（内部 `[pasteEvent trimNewlines]`，注释「Button that pastes text after trimming a trailing newline」）→ `[Paste, Paste Without Newline, Cancel]`；大文本场景 `[OK, Cancel, Advanced…]`（Advanced 打开 Paste Special）。首选动作即 `Paste`。来源：`iTermPasteHelper.m:943-956`。
- 键盘：`iTermWarning` 会给无显式 `keyEquivalent` 的按钮自动分配 Cmd 组合键，且「kept apart from keyEquivalent so the caller's choice is never overwritten」；Esc/Enter 绑定未证实（未读到）。来源：`iTermWarning.m:28-29, :628-696`。
- 可选项：`noSyncDoNotWarnBeforeMultilinePaste`、`noSyncDoNotWarnBeforePastingOneLineEndingInNewlineAtShellPrompt`、`suppressMultilinePasteWarningWhenPastingOneLineWithTerminalNewline`（三者默认 `NO`）；`suppressMultilinePasteWarningWhenNotAtShellPrompt`、`promptForPasteWhenNotAtPrompt`、`alwaysWarnBeforePastingOverSize` 默认值未证实（本次输出被截断）。静默机制：`kiTermWarningTypeTemporarilySilenceable`/`SilenceableForOneMonth`/`PermanentlySilenceable`，临时静默 600 秒，选 Cancel 会撤销静默（issue 5115）。来源：`iTermAdvancedSettingsModel.m:797-809`、`iTermWarning.m:13, :271-406, :720-763, :547-565`。
- 为什么这样做：它不是「多行就拦」，而是把警告绑到真实风险场景（在 shell 提示符、末尾换行会被立即执行），并把「去掉尾部换行」做成与「粘贴」并列的一等动作——这正是 MTerm 缺的那个按钮。

### 3. GNOME Terminal / VTE
- 机制：不做确认，做净化。VTE 的 `pastify_string` 剔除 C0 控制码（保留 NUL/HT/CR/LF）与 C1 控制码，把换行统一成 CR，再按需用 bracketed 序列包裹。
- 触发条件：任何粘贴/拖放路径，无阈值、无对话框。来源：`vte/src/pastify.cc:27-45`。
- 界面形态：无。GNOME Terminal 的用户手册（copy/paste、user input、select text 三页）未出现任何「粘贴确认/警告」条目。来源：`gnome-terminal/help/C/txt-copy-paste.page`。
- 按钮与默认焦点 / 键盘：不适用。
- 可选项：无相关选项（未证实存在）。
- 为什么这样做：Linux 桌面把「控制码注入」当净化问题而非用户决策问题，代价是用户没有最后一道复核；MTerm 已知远端能力探测成本低，可以两者兼得（净化 + 确认）。

### 4. PuTTY
- 机制：无确认框。防护靠两条：默认过滤剪贴板里的控制字符（只放行换行/制表/退格/DEL），以及可整体关闭 bracketed paste。
- 触发条件：不适用（无确认）。来源：手册 §4.11.5「'Permit control characters in pasted text'」、§4.6.12「Disabling bracketed paste mode」。
- 界面形态：无对话框；相关项在 Selection 面板（§4.11.3/§4.11.4：默认右键粘贴、中键扩展选择，可配置粘贴动作与来源剪贴板）。
- 按钮与默认焦点 / 键盘：不适用（右键/中键/Shift-Ins 等为粘贴入口）。
- 可选项：「Permit control characters in pasted text」（勾选后「any character on the clipboard is sent to the session uncensored」）、「Disable bracketed paste mode」。
- 为什么这样做：文档自陈 bracketed paste「introduces bigger problems than the ones it solves」，于是给用户一个关掉它的开关；这印证了 bracketed 状态不能当作「安全」的充分条件。
- 缺口：本章全文检索未发现多行粘贴确认选项；源码级确认未做 → 源码未证实。

### 5. kitty
- 机制：`paste_actions` 是一组可组合动作（`filter`、`quote-urls-at-prompt`、`replace-dangerous-control-codes`、`replace-newline`、`confirm`、`confirm-if-large`、`no-op`），默认值 `quote-urls-at-prompt,confirm`。来源：`kitty/options/definition.py:1098-1127`、`kitty/options/utils.py:1250-1255`。
- 触发条件：`confirm` = 「Confirm the paste if the text to be pasted contains any terminal control codes as this can be dangerous, leading to code execution if the shell/program running in the terminal does not properly handle these.」；`confirm-if-large` = 大于 16KB 才确认；`replace-newline` = 不确认直接替换换行；二者互斥（`replace-newline` 在 `confirm` 存在时不生效，见 `window.py:2349`、changelog 修复 `:iss:7374`）。
- 界面形态：弹框文案/尺寸未证实（未读到 `window.py:2353` 起的确认实现）。
- 按钮与默认焦点：未证实。
- 键盘：未证实。
- 可选项：改 `paste_actions`；`filter` 可挂 `filter_paste()` 自定义函数。
- 为什么这样做：把「危险」定义在控制码而不是换行上，且用「动作集合」而非布尔开关表达策略，使「确认」与「替换」可以正交组合；其 changelog 也记录了因控制码粘贴风险而加入确认（「When pasting, if the text contains terminal control codes ask the user for permission… Thanks to David Leadbeater for discovering this.」）。

### 6. WezTerm
- 机制：无粘贴确认。官方配置项全集中与粘贴相关的只有 `canonicalize_pasted_newlines`。
- 触发条件：不适用。来源：`wezterm/config/src/config.rs:859`（`Option<NewlineCanon>`）、`:1998-2005`（`None/LineFeed/CarriageReturn/CarriageReturnAndLineFeed`）；粘贴路径 `wezterm-gui/src/termwindow/clipboard.rs:26-59` 直接 `pane.send_paste(&clip).ok()`。
- 界面形态 / 按钮与默认焦点 / 键盘：不适用。
- 可选项：`canonicalize_pasted_newlines`（默认值未证实）。
- 为什么这样做：把问题降级为「换行规范化」，把「会不会误执行」完全交给 shell 的 bracketed paste 处理——与本项目「远端不一定支持 bracketed」的前提冲突，故不可照搬。

### 7. VS Code 集成终端
- 机制：`terminal.integrated.enableMultiLinePasteWarning`（`auto`/`always`/`never`，旧布尔值映射为 `true→auto`、`false→never`）+ 一条静默的「去掉尾部空行」规则 + 一个复选框。
- 触发条件：单行永不警告；`never` 直接放行；`auto` 且 bracketed paste mode 开启则放行；否则弹框。来源：`terminalClipboard.ts:17-51`、`terminalConfiguration.ts:164-171`。
- 界面形态：`dialogService.prompt`，message「Are you sure you want to paste {0} lines of text into the terminal?」，detail 为「Preview:」+ 前 3 行（每行截断到 30 字符加 `…`，还有更多行则再补一行 `…`）。来源：`terminalClipboard.ts:63-75`。
- 按钮与默认焦点：按钮顺序 `Paste` → `Paste as one line`（后者执行 `text.replace(/\r?\n/g, '')`），`cancelButton: true`，复选框 `Do not ask me again`；默认焦点未证实（代码未声明）。来源：`terminalClipboard.ts:77-103`。
- 键盘：源码只声明了「存在取消出口」（`cancelButton: true`），Esc/Enter 到具体按钮的绑定未证实。
- 可选项：设置项三态 + 复选框（勾选且确认后 `updateValue(..., 'never')` 持久化）。
- 为什么这样做：真正的杀手锏是 `terminalClipboard.ts:53-60` 的注释——「When a command is copied with a trailing new line, strip the trailing newline so the command is not automatically executed on paste. This mitigates a clipboard hijacking scenario where a malicious page replaces clipboard contents with a command followed by a newline… The user can still review the pasted text and press Enter to run it.」条件是「两行且第二行为空」→ 静默只粘贴第一行。

### 8. Tabby 与 MobaXterm
**Tabby**（源码可查）
- 机制：先归一化（Windows: `\r\n`→`\r`，其它: `\n`→`\r`），再按设置替换/裁剪，最后在非 alternate screen 下弹确认。
- 触发条件：`(data.includes('\r') || data.includes('\n')) && warnOnMultilinePaste`，且 `!alternateScreenActive`（全屏程序里不打扰）。来源：`baseTerminalTab.component.ts:545-580`。
- 界面形态：`platform.showMessageBox` warning，message「Paste multiple lines?」，detail 为前 1000 字符原文预览。
- 按钮与默认焦点：`['Paste','Cancel']`，`defaultId: 0`（默认焦点=Paste）、`cancelId: 1`；`result === 1` 即中止粘贴（连设置里已有的裁剪也不会执行）。
- 键盘：由 Electron `showMessageBox` 决定（Enter→默认按钮，未证实细节）。
- 可选项：`warnOnMultilinePaste`（默认 `true`）、`bracketedPaste`（`true`）、`replaceNewlinesWithSpacesOnPaste`（`false`）、`trimWhitespaceOnPaste`（`true`）、`pasteOnMiddleClick`；设置界面文案「Warn on multi-line paste / Show a confirmation box when pasting multiple lines」「Bracketed paste (requires shell support) / Prevents accidental execution of pasted commands」。来源：`tabby-terminal/src/config.ts:27,37,52,66,67`、`terminalSettingsTab.component.pug:138-163`。
- 无「不再询问」复选框，只能去设置里关。

**MobaXterm**（仅官方文档）
- 机制/触发条件：「Warn before pasting multiple lines — Show a confirmation message before pasting multiple lines into the terminal」（全局设置 §3.2，锚点 `#3_2_11`）。
- 界面形态/按钮/焦点/键盘/默认值：未证实（官方文档只有这一句，无截图描述、无默认值）。
- 可选项：该设置项本身即可开关。
- 来源：https://mobaxterm.mobatek.net/documentation.html

**FinalShell（项目灵感来源，仅作备注）**：官网更新日志与文档中未检索到多行粘贴确认相关条目，软件闭源不可查 → 未证实。（参考：https://www.hostbuf.com/t/989.html）

### 9. 云控制台 Web 终端（阿里云 Workbench）
- 机制：Workbench 是浏览器内免安装的远程连接工具（SSH/RDP），官方概览页未记载任何粘贴确认机制；浏览器剪贴板粘贴由前端控件与浏览器权限模型直接处理。
- 触发条件/界面形态/按钮与默认焦点/键盘/可选项：未证实（未实测，官方文档未见相关描述）。
- 来源：https://www.alibabacloud.com/help/en/ecs/user-guide/workbench-overview/
- 腾讯 OrcaTerm `https://cloud.tencent.com/document/product/1976`、华为 CloudShell 文档页本次均返回 404，未取得证据 → 未证实。
- 为什么这样做（可推断部分，标注为推断）：云控制台把「误执行」风险让渡给会话录制/审计与操作确认弹窗体系，而不是终端层。

### 10. bracketed paste 协议与注入风险
- 语义：xterm 控制序列文档原文——「When bracketed paste mode is set, pasted text is bracketed with control sequences so that the program can differentiate pasted text from typed-in text. When bracketed paste mode is set, the program will receive: `ESC [ 2 0 0 ~`, followed by the pasted text, followed by `ESC [ 2 0 1 ~`.」由应用通过私有模式 2004 自行开启/关闭。来源：`xterm-snapshots/ctlseqs.txt:2821-2831`；模式列表 2004/2005 原文未证实（未定位到行文，且 invisible-island.net 直取返回 403）。
- 触发条件：远端应用（shell/REPL）发 `ESC[?2004h` 即进入该模式；MTerm 侧由 `terminal_uses_bracketed_paste(&bufs, tab)` 探测（`src/app.rs:12749`），仅在探测为真时用 `ESC[200~…ESC[201~` 包裹（`src/terminal/impls/input.rs:62`）。
- 界面形态：协议层无界面。
- 注入风险（三层证据）：
  1. 模式可被远端伪造/开启：WT GH#13014 的复现步骤是在 pwsh 里执行 `echo "`e[?2004h`e]52;;ZGlyCnJlbQo=`e\"`（OSC 52 写剪贴板 + 打开 bracketed paste），然后右键粘贴即无提示执行多行命令；报告人结论「we've got a security feature (the multiline paste warning), which can be trivially disabled with an escaped sequence (bracketed paste)」。
  2. 剪贴板劫持 + 尾换行：VS Code 的注释明确把「命令 + 尾换行」的剪贴板内容视为劫持场景，并以静默去尾换行缓解（`terminalClipboard.ts:53-60`）。
  3. 控制码粘贴本身即风险：kitty 因控制码粘贴风险新增 `confirm`（changelog 安全条目，致谢 David Leadbeater）。
- 可选项（各实现）：WT 允许用户关掉 bracketed paste；PuTTY 同理；kitty 用 `paste_actions`。
- 为什么这样做 / 对本项目的含义：bracketed paste **只表示「远端支持该模式」，不代表「远端不会执行粘贴内容」**（PowerShell/cmd 就是支持模式但不做多行保护的反例，见 GH#13014）。因此 MTerm 不能用「bracketed 已开」来免除多行确认（与 WT 的 `Automatic` 相反，WT 自己的代码注释也承认这点 unsafe）；但可以用它来**调整提示强度与文案**。

## ③ D1–D5 建议

**D1 默认动作**
- 建议：主按钮为「粘贴」（原样，含尾部回车）；但当剪贴板内容为「单个逻辑行 + 末尾换行」时，默认动作降级为「粘贴（不含尾部回车）」，并把「去掉尾部回车」始终作为并列的一等按钮。
- 理由：尾回车等于立即执行且不可逆；VS Code 对单行尾换行静默去尾以防剪贴板劫持，iTerm2 专门提供 `Paste Without Newline`，二者恰好覆盖「最容易被顺手执行」的场景，而多行正文保留原样是主流默认（WT/Tabby 主按钮均为原样粘贴）。
- 来源：VS Code `terminalClipboard.ts:53-60`；iTerm2 `iTermPasteHelper.m:943-956`；WT `TerminalPage.xaml:88` + `Resources.resw`（`Paste anyway`）。
- 判定：改造后采纳（保留原样粘贴为主，单行+尾换行默认去尾）。

**D2 回车键语义**
- 建议：Enter = 安全动作（取消，不粘贴）；确认需显式点击主按钮或 `Alt+P`；Esc 亦为取消（Esc 与 CloseButton 的绑定关系未证实，实现时需显式指定）。
- 理由：多行粘贴框之后，用户的下一个动作往往就是「回车执行」，而微软文档明确「ENTER 会自动触发 DefaultButton」——若沿用主按钮为默认焦点，Enter 会把「确认粘贴」和「执行」压成两次连续按键，正是本确认框要拦住的失误链；微软规范同时要求对话框必须有一个安全非破坏按钮（"Close"/"Cancel"）；而 WT/iTerm2/Tabby 都把确认按钮设为默认焦点（`TerminalPage.xaml:78/:88`、Tabby `defaultId: 0`）——在 SSH 客户端作用于生产主机的前提下，建议此处背离主流。
- 来源：MS Learn「Dialog controls」（DefaultButton 与 ENTER 逐字条款、安全按钮条款）；WT `TerminalPage.xaml:78, :86, :88`；Tabby `baseTerminalTab.component.ts:568-577`。
- 判定：不采纳主流做法（采用安全默认），代价是键盘流多一次按键。

**D3「不再询问」**
- 建议：提供复选框「不再提示多行粘贴」；勾选并确认后写回现有 `paste_confirm_enabled = false`（不新增设置键）；100 KB 强制复核路径与「危险命令」提示不受该开关影响。
- 理由：VS Code 用同一个复选框把设置持久化为 `never`，WT 的 `DontAskAgainCheckBox` 是同类范例；MTerm 已有 `paste_confirm_enabled`（默认 `true`），复用可避免设置项膨胀，而把强制复核/危险标记排除在静默范围外可保留兜底。
- 来源：VS Code `terminalClipboard.ts:92-103`；WT `TerminalPage.xaml:75-80`；MTerm `src/app.rs:12759-12760`、`ui/app.slint:1701-1704`。
- 判定：采纳。

**D4 危险命令行级标记**
- 建议：不阻断，只在预览区做**行级徽标 + 中文类别名**，并在框顶汇总「本段含 N 处危险命令」；A 级（不可逆破坏）行标红并额外将默认动作降级为「去掉尾部回车」，B 级（提权/管道执行）标黄。
- 理由：终端无法解析 shell 语义（引号/变量/别名/函数），正则只能提示不能判定；kitty 对危险控制码也只做确认而非拒绝，行级标记可解释、可容忍误报。
- 来源：kitty `definition.py:1109-1116`（危险控制码的确认而不替换）；模式清单无外部权威清单可比对 → **模式清单为本设计建议**（非引用）。
- 判定：改造后采纳（仅提示、不升级为阻断）。

| 匹配模式（示意，归一化 CR→LF 后逐行） | 中文类别名 | 级别 |
| --- | --- | --- |
| `\brm\s+(-[A-Za-z]*[rf][A-Za-z]*\s+)` | 递归强制删除 | A |
| `\bmkfs(\.\w+)?\b` | 格式化文件系统 | A |
| `\bdd\b[^\n]*\bof=/dev/` | 直写块设备 | A |
| `>\s*/dev/(sd\|nvme\|hd\|vd)\w*` | 覆盖磁盘设备 | A |
| `:\(\)\s*\{.*\};\s*:` | fork 炸弹 | A |
| `\btruncate\s+-s\s*0\b`、`:\s*>\s*\S+` | 清空/截断文件 | A |
| `\b(shutdown\|reboot\|halt\|poweroff)\b`、`\binit\s+[06]\b` | 关机或重启 | A |
| `\bsudo\b`、`\bsu\s+-` | 权限提升 | B |
| `\bchmod\s+(-R\s+)?[0-7]*777\b`、`\bchown\s+(-R\s+)?\S+:\S+\s+/` | 权限全开 | B |
| `\b(curl\|wget)\b[^\n]*\|\s*(sudo\s+)?(ba\|z\|d)?sh\b` | 管道执行远端脚本 | B |
| `\b(useradd\|usermod\|passwd\|visudo)\b` | 账户与凭据变更 | B |
| `\b(iptables\|nft\|ufw\|firewall-cmd)\b` | 防火墙变更 | B |
| `\b(kill\|pkill\|killall)\s+-9\b` | 强制终止进程 | B |
| `\bhistory\s+-c\b`、`\bcrontab\s+-r\b` | 痕迹清理与持久化 | B |
| `\bgit\s+(reset\s+--hard\|clean\s+-[a-z]*f)` | 丢弃本地改动 | B |
| `\b(apt(-get)?\s+(remove\|purge)\|yum\s+remove\|dnf\s+remove)\b` | 卸载软件包 | B |
| `\bDROP\s+(DATABASE\|TABLE)\b`、`\bdropdb\b` | 数据库破坏 | A |

**D5 会话上下文**
- 建议：在弹框内显示目标 tab 名、`user@host:port`，以及 bracketed paste 的开/关徽标；bracketed 为关时把主按钮文案改为「仍然粘贴」并在顶部给出「远端不支持 bracketed paste，多行将按逐行执行」的说明。
- 理由：WT 正是以 `!bracketedPaste` 作为 `Automatic` 的警告条件（说明该状态是风险判据），VS Code 的 `auto` 把「bracketed paste mode 开启」解释为「shell 原生支持多行粘贴」，把该判据显式化能解释「为什么弹这个框」，也便于用户判断这次粘贴打到哪台机器。
- 来源：WT `TerminalPage.cpp:3174-3179`；VS Code `terminalConfiguration.ts:164-171`；MTerm `src/app.rs:12749`。
- 判定：采纳。

## ④ 参考链接清单

1. Windows Terminal 设置文档（`multiLinePasteWarning` / `largePasteWarning`）：https://learn.microsoft.com/en-us/windows/terminal/customize-settings/interaction
2. 微软对话框规范（按钮与安全出口）：https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/dialogs-and-flyouts/dialogs
3. WT 绕过警告的实例：https://github.com/microsoft/terminal/issues/13014
4. WT 触发逻辑：https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalApp/TerminalPage.cpp#L3171-L3252
5. WT 对话框 XAML：https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalApp/TerminalPage.xaml#L88-L104
6. WT 文案：https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalApp/Resources/en-US/Resources.resw#L549-L572
7. WT 设置模型：https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalSettingsModel/MTSMSettings.h#L60-L61 ／ https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalControl/EventArgs.idl#L41-L46
8. iTerm2 警告裁决：https://github.com/gnachman/iTerm2/blob/master/sources/Pasting/iTermPasteHelper.m#L878-L1000
9. iTerm2 警告机制与静默：https://github.com/gnachman/iTerm2/blob/master/sources/Infrastructure/iTermWarning.m#L13-L30 （另见 `:271-406`, `:547-565`, `:628-763`）
10. iTerm2 高级设置默认值：https://github.com/gnachman/iTerm2/blob/master/sources/Settings/iTermAdvancedSettingsModel.m#L797-L809
11. VTE 粘贴净化 `pastify_string`：https://github.com/GNOME/vte/blob/master/src/pastify.cc#L27-L45
12. GNOME Terminal 用户手册（复制/粘贴页，无确认项）：https://github.com/GNOME/gnome-terminal/blob/master/help/C/txt-copy-paste.page
13. PuTTY 手册 §4.11.5 / §4.6.12：https://the.earth.li/~sgtatham/putty/latest/htmldoc/Chapter4.html
14. kitty `paste_actions` 定义与文案：https://github.com/kovidgoyal/kitty/blob/master/kitty/options/definition.py#L1098-L1127
15. kitty 取值校验：https://github.com/kovidgoyal/kitty/blob/master/kitty/options/utils.py#L1250-L1255
16. kitty 粘贴动作实现：https://github.com/kovidgoyal/kitty/blob/master/kitty/window.py#L2324-L2408
17. kitty 发行说明（安全条目、7374 修复）：https://github.com/kovidgoyal/kitty/blob/master/docs/changelog.rst
18. WezTerm 配置项全集（只有 `canonicalize_pasted_newlines`）：https://wezterm.org/config/lua/config/index.html
19. WezTerm 剪贴板粘贴路径：https://github.com/wez/wezterm/blob/main/wezterm-gui/src/termwindow/clipboard.rs#L26-L59 ／ 选项类型见 `config/src/config.rs#L859,L1998-L2005`
20. VS Code 多行粘贴对话框与静默去尾换行：https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/terminalContrib/clipboard/browser/terminalClipboard.ts#L14-L110
21. VS Code 设置项三态：https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/terminal/common/terminalConfiguration.ts#L164-L171
22. Tabby 粘贴实现：https://github.com/Eugeny/tabby/blob/master/tabby-terminal/src/api/baseTerminalTab.component.ts#L545-L595
23. Tabby 设置项默认值：https://github.com/Eugeny/tabby/blob/master/tabby-terminal/src/config.ts#L27-L67
24. Tabby 设置界面文案：https://github.com/Eugeny/tabby/blob/master/tabby-terminal/src/components/terminalSettingsTab.component.pug#L138-L168
25. MobaXterm 文档（§3.2「Warn before pasting multiple lines」）：https://mobaxterm.mobatek.net/documentation.html
26. FinalShell 官方更新日志（未见相关条目，仅作备注）：https://www.hostbuf.com/t/989.html
27. 阿里云 Workbench 概览（未记载粘贴确认）：https://www.alibabacloud.com/help/en/ecs/user-guide/workbench-overview/
28. xterm 控制序列文档（bracketed paste 语义）：https://github.com/ThomasDickey/xterm-snapshots/blob/master/ctlseqs.txt#L2821-L2831

**未证实项（共 11 条）**：iTerm2 三个高级设置默认值；iTerm2 警告面板的 Esc/Enter 绑定；VS Code 对话框默认焦点；kitty 确认弹框文案/按钮/焦点；kitty 2004/2005 模式原文与 xterm FAQ 条目（直取 403）；WezTerm `canonicalize_pasted_newlines` 默认值；Tabby 键盘细节；MobaXterm 默认值与按钮布局；PuTTY 源码级确认；WinUI ContentDialog 的 Esc→CloseButton 绑定；FinalShell 与三家云控制台 Web 终端的行为。
