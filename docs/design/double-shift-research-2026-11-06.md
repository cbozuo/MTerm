# Double Shift 全局搜索 · 竞品与机制调研（2026-11-06）

范围：MTerm 桌面端（Rust + Slint）的「双击 Shift 打开全局搜索」交互；为 `docs/design/double-shift-spec-2026-11-06.md` 提供竞品、源码与机制证据，不讨论具体 Rust/Slint 实现。

证据等级：源码文件行 > 官方文档/发行说明 > 博客。凡本次未能打开原文的，一律标注「未证实」，不填默认值、不描述未见的界面。本文中的「推断」均为研究判断，不等于产品事实。

标记：`【官方】` 官方文档或发行说明；`【源码】` 可打开的源码文件；`【推断】` 本研究推导；`【未证实】` 本次未取得原文或未验证。

---

## ① 结论速览表

| 产品 | 入口 | 搜索范围 | 分类切换 | 键盘导航 | 预览 | 可借鉴点 |
|------|------|----------|----------|----------|------|----------|
| IntelliJ IDEA Search Everywhere | 连按两次 `Shift`；主菜单 Navigate \| Search Everywhere | 项目内外 files / actions / classes / symbols / settings / UI elements / Git；默认显示 recent files | `All` + Classes / Files / Symbols / Actions / Database / Text / URL 等 contributor tabs；`All` 有分组标题 | `Tab` 循环分类；鼠标悬停 tab 显示直达快捷键；`Alt+N` 或再次双击 Shift 切换 Include non-project；`Enter` 打开；`Esc` 关闭 | 有：PreviewAction 控制右侧预览；源码见 `SearchEverywherePreviewGenerator` | All tab + 分组 + 预览 + 键盘优先，最接近本项目目标 |
| VS Code Quick Open | `Ctrl+P`（macOS `⌘P`） | 按文件名搜索并打开文件；`?` 打开命令建议；`edt` / `term` + 空格进入命令模式；重复按快捷键循环最近文件 | 输入前缀/模式，不是顶部分类 Tab | 上下选择、`Enter` 打开、`Right Arrow` 后台打开多个文件；`Esc` 关闭（官方页未逐键列出） | 无独立预览面板；文件直接打开 | 轻量单输入框、最近文件循环、后台打开 |
| VS Code Command Palette | `Ctrl+Shift+P`（macOS `⇧⌘P`） | 当前上下文可用的全部命令；可打开文件、搜索符号、显示 quick outline | 无分类 Tab；命令列表随上下文变化 | 方向键选择、`Enter` 执行、`Esc` 关闭；命令旁显示默认快捷键 | 无 | 上下文过滤、命令旁快捷键提示、可拖动位置 |
| Windows Terminal Command Palette | `Ctrl+Shift+P`（可改键） | Terminal actions；删除输入框 `>` 进入 command line mode 运行 `wt` 命令 | 不是 Tab，而是 Action / Commandline / TabSearch / TabSwitch 模式；`>` 是 ActionMode 前缀 | 源码支持 `↑/↓` 环绕、PageUp/PageDown、`Ctrl+Home/End`、`Enter`、`Esc`、`Backspace`（空文本回 Commandline）、`Right` 填入命令 | 无独立预览面板；官方文档未提预览，源码本次未确认预览实现（未证实） | 用 fzf matcher 过滤动作；`>` 前缀模式清晰 |
| Raycast | 默认 `⌘ Space` / `Alt Space`（手动文档两页说法不一致） | Apps、命令、文件、剪贴板/历史、扩展、日历、联系人、计算器等；文件搜索可开启内容搜索 | Root Search 混排；`Search Files` 是独立命令；Action Panel 承载动作 | `↑/↓`、`⌥↑/↓` 翻页、`⌘↑/↓` 跳 section、`Enter` 主操作、`⌘K` Action Panel、`⌘Enter` 次操作、`Esc` 返回/关闭、`⌘Y` Quick Look | 有：`⌘Y` Quick Look；Search Files 有 metadata details panel | frecency 排序、alias、Action Panel 分层、空查询最近使用 |
| Alfred | 默认 `⌥ Space`（Alt + Space），可改 | Default Results（Applications / Contacts / Preferences）；File Search 用 `open` / `find` / `in` 等 keyword 扩展 | keyword 前缀切换，不是 Tab；无前缀时是 Default Results | 本次未取得完整键盘表；官方确认 `Return` 启动、`⌥Space` 打开（其余未证实） | 有：Actions panel 与 File Navigation 富预览；`Shift` 触发 QuickLook 大预览 | 使用习惯指纹（4 周滚动）、keyword latching、富预览 |
| PowerToys Run | `Alt+Space`（可改） | Apps / folders / files、运行中进程、系统命令、计算器、时间日期、单位转换、hash、GUID、网页搜索等 | 插件/前缀模式，例如 `>` 进入 Shell Plugin；不是分类 Tab | 本次未取得官方键盘表（未证实） | 未证实 | 单入口 + 插件前缀；v2 正迁移为 Command Palette |
| fzf（算法/性能参考） | CLI，无 GUI | stdin 行、命令历史、文件列表等 | 无分类；用 query 语法（精确/前缀/后缀/排除/OR） | `Ctrl-K/J` 或 `Ctrl-P/N` 移动、`Enter` 选中、`Esc/Ctrl-C/Ctrl-G` 退出、`Tab` 多选 | 不是 GUI；预览能力本次未取证 | Smith-Waterman V2 评分、词首/ camelCase 权重、gap penalty |

---

## ② 逐产品条目

### 1. IntelliJ IDEA Search Everywhere（重点）

- **机制/触发条件**：官方帮助写明「In the main menu, go to Navigate | Search Everywhere or press Shift twice to open the search window. By default, IntelliJ IDEA displays the list of recent files.」`【官方】`（[searching-everywhere](https://www.jetbrains.com/help/idea/searching-everywhere.html)）。JetBrains Guide 同样写「Press ⇧⇧ (macOS) / Shift+Shift (Windows/Linux) for Search Everywhere」，并明确「To dismiss the popup, press Escape.」`【官方】`（[JetBrains Guide](https://www.jetbrains.com/guide/tips/search-everywhere/)）。
- **搜索范围**：一个入口可查项目内外 files、actions、classes、symbols、settings、UI elements、Git 内容；默认 recent files；`Include non-project items` 扩展至外部项；切到其他 tab 时选 `All Places` 同样扩展范围。`【官方】`（同上）。
- **同义词**：官方页举例输入 `toggle presentation mode` 可命中 `Enter Presentation Mode`，说明 Search Everywhere 不只是字面前缀，还支持动作同义词/别名。`【官方】`（同上）。
- **分类与分组**：源码 `SearchEverywhereHeader.java` 的 `createTabs` 在 contributor 多于一个时先创建 `All` tab，再按 `isShownInSeparateTab()` 创建独立 tab；`SearchEverywhereUI.java` 的 `switchToTab` 明确「reset cell renderer to show/hide group titles in "All" tab」，即 **All tab 显示分组标题，单 contributor tab 不显示**。`【源码】`（[SearchEverywhereHeader.java](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereHeader.java)、[SearchEverywhereUI.java](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereUI.java)）
- **tab 与快捷键映射**：`SearchEverywhereTabsShortcutsUtils.kt` 把 All contributor 的标签绑定为 `LangBundle.message("double.shift")`，各 tab 依次对应 `GotoClass`、`GotoFile`、`GotoSymbol`、`GotoAction`、`GotoDatabaseObject`、`TextSearchAction`、`GotoUrlAction`；`addShortcut` 通过 `ActionManager.getInstance().getKeyboardShortcut(actionID)` 和 `KeymapUtil.getShortcutText(shortcut)` 取当前 keymap 的快捷键文本。`【源码】`（[SearchEverywhereTabsShortcutsUtils.kt](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereTabsShortcutsUtils.kt)）
- **键盘模型**：Guide 明确「Those tabs show the kinds of things that can be searched. Hit Tab to go Classes, then again to go to Files.」以及「Mouseover on the tabs to show key bindings to open directly.」`【官方】`。源码 `SearchEverywhereHeader` 有 `switchToNextTab()` / `switchToPrevTab()`，tab tooltip 取 `myShortcutSupplier.apply(tab.getID())`，说明 **Tab / Shift+Tab 循环切换，且每个 tab 有直达键**。`【源码】`。`Alt+N` 可切换 Include non-project items；再次双击 Shift 也能切换。`【官方】`。`Enter` 打开选中项、`Esc` 关闭来自正文/Guide；`Ctrl+Enter` 的 IDEA 行为本次未在可读正文中确认。`【未证实：Ctrl+Enter 的具体语义；Ctrl+Down/Ctrl+Up 等列表键位】`
- **空查询行为**：官方明确默认显示 recent files；源码 `SearchEverywhereManagerImpl` 有按 tab 的搜索历史和 `SearchTextField.SHOW_HISTORY_SHORTCUT` / `ALT_SHOW_HISTORY_SHORTCUT`，并会保存搜索文本和上次选中项。`【官方】【源码】`（[SearchEverywhereManagerImpl.java](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereManagerImpl.java)）
- **界面形态**：单输入框 + 顶部 tabs + 右侧预览面板 + footer 提示；Guide 说「You can resize this window... remembers dimensions」「Click Open in Find Tool Window to get the full find treatment」「Search Everywhere is also a desktop calculator」。`【官方】`。源码 popup builder 使用 `LOCATION_SETTINGS_KEY = "search.everywhere.popup"`、`setResizable(true)`、`setMovable(true)`、`setDimensionServiceKey(...)`，即弹窗位置/尺寸会记忆。`【源码】`
- **预览面板**：`PreviewAction.kt` 定义 `const val PREVIEW_ACTION_ID: String = "Search.Everywhere.Preview"`，继承 `DumbAwareToggleAction`，图标 `AllIcons.General.PreviewHorizontally`，`isSelected` 读 `UISettings.getInstance().showPreviewInSearchEverywhere`，`setSelected` 记录事件并 `toggleSearchPreview(state)`。`【源码】`。`SearchEverywhereManagerImpl.isPreviewEnabled()` 还受 `PreviewExperiment.isExperimentEnabled()` 与 `!PlatformUtils.isJetBrainsClient()` 约束；创建 popup 时调用 `OpenInRightSplitAction.Companion.overrideDoubleClickWithOneClick(...)`，即预览/右侧打开场景把双击改成一击。`【源码】`。`SearchEverywhereUI` 在 project 非空时创建 `SearchEverywherePreviewGenerator` 并更新 `myUsagePreviewPanel`。`【源码】`
- **性能/索引策略**：本次未找到 JetBrains 公开的 Search Everywhere 索引构建/缓存文档。源码可验证的是结果列表分页上限：`SINGLE_CONTRIBUTOR_ELEMENTS_LIMIT = 30`、`MULTIPLE_CONTRIBUTORS_ELEMENTS_LIMIT = 15`；`searchFinished` 在无结果且当前 scope 可扩展时自动 `autoSetEverywhere(true)`，即先扩大范围再提示。`【源码】`。索引本身由各 contributor 提供，具体数据源不在本次开源 UI 文件的可见范围。`【未证实：索引实现；推断：classes/files/symbols 依赖 IDE 项目索引】`
- **可借鉴/为什么**：IDEA 把「找什么」用 tab 表达，把「找多广」用 Include non-project / All Places 表达，把「怎么执行」留给 Enter 与预览；用户无需先选搜索类型，只需开始输入。这是 MTerm「键盘优先 + 搜索即执行」最值得复用的骨架。`【推断】`
- **来源**：官方帮助、JetBrains Guide、上述 GitHub 源码路径（URL 见本节与参考链接）。

### 2. VS Code Quick Open + Command Palette

- **入口与触发**：Quick Open 默认 `Ctrl+P` / `⌘P`；官方文档写「Type `kb(workbench.action.quickOpen)` (**Quick Open**) to quickly search and open a file by its name.」`【官方】`（[userinterface.md L167](https://github.com/microsoft/vscode-docs/blob/main/docs/editing/getting-started/userinterface.md)）。Command Palette 默认 `Ctrl+Shift+P` / `⇧⌘P`，官方写「The most important key combination to know is `kb(workbench.action.showCommands)`, which brings up the Command Palette.」`【官方】`（[userinterface.md L325-L327](https://github.com/microsoft/vscode-docs/blob/main/docs/editing/getting-started/userinterface.md)）
- **Quick Open 搜索范围**：按文件名快速搜索并打开文件；输入 `?` 查看命令建议；输入 `edt` 或 `term` 后接空格出现命令模式下拉；重复按 Quick Open 快捷键在最近打开文件间循环；右方向键把当前文件后台打开并继续选择。`【官方】`（[tips-and-tricks.md L486-L510](https://github.com/microsoft/vscode-docs/blob/main/docs/editing/getting-started/tips-and-tricks.md)）
- **Command Palette 搜索范围**：官方描述「Access all available commands based on your current context」；可运行编辑器命令、打开文件、搜索符号、quick outline；命令旁显示默认快捷键；窗口可拖动标题栏移动。`【官方】`（[tips-and-tricks.md L50-L65](https://github.com/microsoft/vscode-docs/blob/main/docs/editing/getting-started/tips-and-tricks.md)、[userinterface.md L325-L344](https://github.com/microsoft/vscode-docs/blob/main/docs/editing/getting-started/userinterface.md)）
- **分类切换**：VS Code 没有 IDEA 式常驻分类 Tab；Quick Open 用输入前缀/模式切换（`?` 命令建议、`edt`/`term` + 空格），Command Palette 本身是命令列表。`【官方】`。`【推断】`：它的「分类」由输入模式和当前上下文决定，而不是显式 tab。
- **键盘模型**：Quick Open 的官方页明确 `Right Arrow` 后台打开多个文件；Command Palette 的命令旁显示默认快捷键。方向键选择、`Enter` 执行、`Esc` 关闭属于 Quick Input 的通用行为，但本页未逐项列出。`【官方/未证实：完整逐键表】`
- **空查询行为**：Quick Open 的「重复按快捷键循环最近打开文件」等价于把最近文件作为空查询/重复触发时的默认内容；Command Palette 空查询显示当前上下文可用命令列表。`【官方/推断】`
- **预览面板**：两者均无独立预览面板；Quick Open 选中文件后直接打开，Command Palette 选中命令后直接执行。`【官方】`
- **模糊匹配与排序**：`fuzzyScorer.ts` 的 `scoreFuzzy(target, query, queryLower, allowNonContiguousMatches)` 返回 `FuzzyScore = [number, number[]]`，第二项是 match positions，可直接用于高亮。评分规则：每字符匹配 `+1`；连续匹配序列前 3 个每字符 `+6`、之后每字符 `+3`；同大小写 `+1`；词首 `+8`；分隔符后 Slash/Backslash `+5`，Underline/Dash/Period/Space/SingleQuote/DoubleQuote/Colon `+4`；camelCase 大写且非连续匹配 `+2`；斜杠/反斜杠视为相等。`【源码】`（[fuzzyScorer.ts](https://github.com/microsoft/vscode/blob/main/src/vs/base/common/fuzzyScorer.ts)）
- **稳定 tie-break**：`compareItemsByFuzzyScore` 先比较路径完全相同 `PATH_IDENTITY_SCORE = 1 << 18`，再让纯 label 命中高于 label+description（`LABEL_SCORE_THRESHOLD = 1 << 16`、`LABEL_PREFIX_SCORE_THRESHOLD = 1 << 17`）；label 前缀命中按 query 占 label 比例加分；非前缀时更紧凑匹配、更短 label 优先；最后 `fallbackCompare` 依次比较 label+description 长度、path 长度和 `compareAnything`。`【源码】`
- **查询解析**：双引号包裹表示 exact substring（`expectContiguousMatch`）；空格分隔多个 query piece，全部 piece 必须匹配；归一化会去掉 `*`、空白、引号等字符。`【源码】`
- **性能/索引**：Quick Open 的文件候选与 fuzzy 评分在内存中完成；官方页没有给文件索引策略。`【未证实：索引细节】`
- **来源**：VS Code 官方文档源码、`fuzzyScorer.ts`（URL 见参考链接）。

### 3. Windows Terminal Command Palette

- **入口与触发**：官方文档写「The command palette shows you which actions you can run inside Windows Terminal.」；`Ctrl+Shift+P` 打开，可自定义 key binding：`{ "command": "commandPalette", "keys": "ctrl+shift+p" }`。`【官方】`（[command-palette](https://learn.microsoft.com/en-us/windows/terminal/command-palette)）
- **搜索范围与模式**：palette 列出可运行 actions；删除输入框里的 `>` 字符进入 command line mode，可输入 `wt` 命令并在当前窗口执行；也可绑定 `{ "command": "commandPalette", "launchMode": "commandLine", "keys": "" }`。源码 `CommandPalette.cpp` 还有 `ActionMode`、`CommandlineMode`、`TabSearchMode`、`TabSwitchMode` 等 mode；`_evaluatePrefix` 按输入是否以 `>` 开头决定 ActionMode/CommandlineMode。`【官方】【源码】`（[CommandPalette.cpp](https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalApp/CommandPalette.cpp)）
- **分类切换**：不是 IDEA 式分类 Tab，而是模式切换：ActionMode 的输入前缀是 `>`；CommandlineMode 无前缀；`Backspace` 在空 ActionMode 下会切回 CommandlineMode。`【源码】`
- **键盘模型（源码）**：`↑`/`↓` 移动并环绕；`PageUp`/`PageDown` 翻页；`Ctrl+Home`/`Ctrl+End` 跳到首/尾；`Enter` 派发选中命令；`Esc` 在搜索文本非空时清空/退出，空文本时关闭；`Right` 在 CommandlineMode 把选中命令填回搜索框；`Ctrl+C`/`Ctrl+Insert` 复制、`Ctrl+V` 粘贴。`【源码】`（同上）
- **过滤算法**：`_collectFilteredActions` 使用 `fzf::matcher::Pattern`、`ParsePattern(searchText)`、`impl->UpdateFilter(pattern)`，跳过 weight 为 0 的动作（空查询除外），ActionMode 用 `FilteredCommand::Compare` 排序。`【源码】`。这使 Terminal 的 command palette 与 fzf 算法共享「词首/边界优先」的匹配思路。
- **空查询行为**：默认 ActionMode 显示 action 列表；源码按 weight 过滤，空查询应保留默认动作/最近动作（具体排序逻辑在 `FilteredCommand` 内，本次未展开）。`【源码/未证实：空查询完整排序】`
- **预览**：官方文档未给预览面板；本次未在可读源码部分确认预览实现。`【未证实：预览呈现】`
- **性能/索引**：源码用 fzf matcher 对动作列表逐次过滤，未见外部索引文件；官方未公开性能数字。`【源码/未证实：内部数据结构与性能数据】`
- **图标**：action 可设置 `icon`（图片路径、Segoe MDL2 symbol 或任意字符含 emoji），说明 command palette 的每一项是「动作」，不是文件。`【官方】`
- **来源**：Learn 文档、`CommandPalette.cpp`（URL 见参考链接）。

### 4. Raycast

- **入口与触发**：`search-bar` 页写 `⌘Space / CtrlSpace` 打开；`keyboard-shortcuts` 页写 `⌘ Space` / `Alt Space`，两页不一致。本文引用时保留两处，默认以 keyboard-shortcuts 页的「⌘ Space / Alt Space」为准并标注冲突。`【官方】`（[search-bar](https://manual.raycast.com/search-bar)、[keyboard-shortcuts](https://manual.raycast.com/keyboard-shortcuts)）
- **搜索范围**：Root Search 是一个跨本机与扩展的实时搜索入口；支持 apps、alternate names、commands、files、calendar、contacts、calculator、quicklinks、AI commands、script commands、settings、system settings、web URLs、color conversion 等；文件搜索见下。`【官方】`（[search-bar](https://manual.raycast.com/search-bar)）
- **分类与切换**：Root Search 把 apps/commands/files/history 等混排；`Search Files` 是独立命令，提供更宽的文件列表与 metadata details panel；Action Panel 以 `⌘K` 打开，部分条目 `Enter` 也可打开。`【官方】`（[search-bar](https://manual.raycast.com/search-bar)、[file-search](https://manual.raycast.com/file-search)）
- **排序**：Root Search 的优先级（高→低）为 1) Exact alias match；2) Alias prefix match；3) Title fuzzy match score；4) Subtitle and keyword matches；5) Frecency（frequency + recency 的混合）。fuzzy 示例：`msg` → Messages、`slk` → Slack；Search Sensitivity 有 High / Medium（默认）/ Low 三档。`【官方】`
- **键盘模型**：`↑/↓` 列表移动；`⌥↑/⌥↓` 上一页/下一页；`⌘↑/⌘↓` 上一个/下一个 section；`Enter` 主操作；`⌘K` Action Panel；`⌘Enter` 次操作；`Esc` 返回/在 Root Search 关闭；`⌘Y` Quick Look；`⌘O` open item；`⇧⌘O` reveal；`Shift+Tab` 文件导航返回父目录；列表顶/底继续按 `↑/↓` 可循环搜索历史。`【官方】`（[keyboard-shortcuts](https://manual.raycast.com/keyboard-shortcuts)、[search-bar](https://manual.raycast.com/search-bar)）
- **空查询**：Root Search 空查询显示 Recently Used；Search Bar 页面还提到 Favorites、Calendar events 会出现在顶部（不同版本/设置的展示顺序可能不同）。`【官方】`
- **预览**：`⌘Y` Toggle Quick Look 预览；Search Files 命令有 metadata details panel；Root Search 没有常驻预览面板。`【官方】`
- **性能/索引**：文件索引默认覆盖 home 目录，macOS 额外索引 `/Applications`；排除隐藏文件，遵守 `.gitignore`、`.ignore`、Git exclude、`.rayignore`，过滤 `node_modules`、临时文件、缓存；索引保存在本地、不联网同步；Content Search 使用系统搜索索引（macOS Spotlight / Windows Search）。`【官方】`（[file-search](https://manual.raycast.com/file-search)）
- **可借鉴**：alias + frecency 的组合比单纯 fuzzy 更像「用户会用」的排序；Action Panel 将主操作与次操作分层，适合 MTerm 的 `Ctrl+Enter` 次操作。`【推断】`

### 5. Alfred

- **入口与触发**：默认热键 `⌥Space (Alt + Space)`，可配置；输入应用名后按 `Return` 启动。`【官方】`（[first-5-minutes](https://www.alfredapp.com/help/getting-started/first-5-minutes/)）
- **搜索范围**：Default Results 是「不带 keyword 前缀搜索时」的结果，默认包含 Applications、Contacts、Preferences；可设置 file types、search scope、fallback searches。Applications Matching 支持大写字母缩写（`gc` → Google Chrome）或按词边界全模糊（`ps` → Photoshop）。`【官方】`（[default-results](https://www.alfredapp.com/help/features/default-results/)）
- **文件搜索**：Default Results 只覆盖常用类型；扩大范围用 keyword：`open` 打开文件（或按空格作为快捷方式）、`find` 在 Finder 中显示、`in` 搜索文件内容；keyword 可在 Features > File Search 自定义；可排除文件类型、限制结果数量；`/` 跳到根目录、`~` 家目录。`【官方】`（[file-search](https://www.alfredapp.com/help/features/file-search/)）
- **分类切换**：没有顶部 Tab；用 keyword/前缀切换搜索域，无前缀时是 Default Results；fallback searches 在没有本地结果时接管。`【官方/推断】`
- **键盘模型**：本次只确认默认 `⌥Space` 打开、`Return` 启动；完整的上下选择/翻页/Esc 表未在本次抓取到（`/help/getting-started/keyboard-shortcuts/` 404，`using-alfred` 404）。`【未证实：完整键盘表】`
- **空查询**：显示 Default Results（Applications / Contacts / Preferences）；具体排序在未学习到使用习惯前属于 Default File Ordering，官方页该段被截断。`【官方/未证实：默认文件排序】`
- **预览**：Alfred 在 Actions panel 与 File Navigation 中展示富文件预览；按 `Shift` 可 QuickLook 更大预览；默认预览显示在 File Navigation 窗口右半部分；Quick Look 与 Previews 默认开启、可分别关闭。`【官方】`（[previews](https://www.alfredapp.com/help/features/previews/)）
- **学习/排序**：Alfred 维护「heuristic fingerprint of your usage, maintained on a 4 week rolling window」；Keyword Latching 默认把输入短语与选中结果绑定；示例：Calendar.app 每天用 100 次、Calculator.app 每天 10 次，但输入 `c` 时连续 2–3 次选择 Calculator 后，`c` 的顶部结果会变为 Calculator；且按精确输入分别学习（`c` 与 `ca` 各自不同）。`【官方】`（[understanding-result-ordering](https://www.alfredapp.com/help/kb/understanding-result-ordering/)）
- **可借鉴**：Alfred 的「关键词域 + 使用习惯指纹 + 富预览」适合 MTerm 的会话/命令两类；其 keyword 方式可避免在单输入框中引入复杂语法。`【推断】`

### 6. PowerToys Run

- **入口与触发**：`Alt+Space` 启动并开始输入；快捷键可在设置中修改。`【官方】`（[run](https://learn.microsoft.com/en-us/windows/powertoys/run)）
- **搜索范围**：Search for applications, folders or files；Search for running processes（原 Window Walker）；Clickable buttons with keyboard shortcuts（如 Open as administrator / Open containing folder）；Shell Plugin `>`（例如 `> Shell:startup` 打开启动文件夹）；calculator；system commands；time/date；unit conversion；hash；GUID；web pages / web search。`【官方】`
- **分类切换**：没有分类 Tab；插件/前缀模式，`>` 进入 Shell Plugin；其他插件按各自前缀/触发词。`【官方/推断】`
- **键盘模型**：本次未取得官方完整键盘表（页面在 Features 后截断）。`【未证实】`
- **空查询/预览**：官方本页未给出空查询行为与预览面板；`【未证实】`.
- **性能/索引**：PowerToys Run 是 quick launcher，模块化插件；官方未公开索引结构；页面注明正在升级 v2 → Command Palette，可作为「launcher 向 command palette 演进」的旁证。`【官方】`
- **可借鉴**：`>` 这类前缀模式在单输入框中非常有效；MTerm 可借鉴但不能让用户记太多前缀，建议只保留 `>` 命令/动作这类高价值模式。`【推断】`

### 7. fzf（算法/性能参考）

- **定位**：README L28「fzf is a general-purpose command-line fuzzy finder and an interactive terminal toolkit.」；L41「**Fast** // Optimized to process millions of items in milliseconds」（官方 marketing 语，不是本次实测基准）。`【官方】`（[fzf README](https://github.com/junegunn/fzf/blob/master/README.md)）
- **搜索语法（extended-search mode）**：`sbtrkt` fuzzy-match；`'wild` exact-match；`'wild'` exact-boundary-match；`^music` prefix-exact-match；`.mp3$` suffix-exact-match；`!fire` inverse-exact-match；`!^music` inverse-prefix；`!.mp3$` inverse-suffix；`--exact` 关闭模糊；单个 `|` 表示 OR（例 `^core go$ | rb$ | py$`）。`【官方】`（README）
- **键盘模型**：`CTRL-K`/`CTRL-J`（或 `CTRL-P`/`CTRL-N`）上下移动；`Enter` 选中；`CTRL-C`/`CTRL-G`/`ESC` 退出；多选模式 `TAB`/`Shift-TAB` 标记；支持 Emacs 键位与鼠标；`--scheme=history` 适合命令历史、按时间排序。`【官方】`（README）
- **算法**：`src/algo/algo.go` 头注释说明 FuzzyMatchV1 在 O(n) 找到首个 fuzzy 匹配但不保证最高分；FuzzyMatchV2 是「a modified version of Smith-Waterman algorithm」求最高分，不允许 pattern 字符省略或错配；V2 匹配时 O(nm)、否则 O(n)，可用 `--algo=v1` 换速度。`【源码】`（[algo.go](https://github.com/junegunn/fzf/blob/master/src/algo/algo.go)）
- **评分常量（algo.go:113-138）**：`scoreMatch = 16`；`scoreGapStart = -3`；`scoreGapExtension = -1`；`bonusBoundary = scoreMatch / 2`；`bonusNonWord = scoreMatch / 2`；`bonusCamel123 = bonusBoundary + scoreGapExtension`；`bonusConsecutive = -(scoreGapStart + scoreGapExtension)`（=4）。`【源码】`（同上）
- **评分思想**：偏好词首、camelCase 大写等特殊位置；特殊位置命中多者优先，即使总匹配更长；pattern 首字符命中特殊位置时 bonus 乘常数；gap penalty 随间隔增长，间隔超过 8 字符时 bonus 被抵消；连续匹配有额外 bonus。`【源码】`（同上）
- **对 MTerm**：fzf 的 V2 适合会话/命令这种长列表（可一次预过滤再评分）；但 MTerm 索引上限 8,000 条、查询长度短，直接用 V1 式 O(n) 扫描 + 分层评分可能已足够，V2 的 O(nm) 只应在必要时启用。`【推断】`

---

## ③ 双击 Shift 的时序证据

### 3.1 JetBrains 是否公开阈值

- JetBrains 的**官方帮助页没有公开**双击修饰键的时间阈值；本次能打开的 `searching-everywhere.html` 只描述「press Shift twice」「double Shift again」，没有毫秒数。`【官方：确认未公开】【未证实：官方阈值】`
- 可打开的源码 `platform/platform-impl/src/com/intellij/openapi/keymap/impl/ModifierKeyDoubleClickHandler.kt`（GitHub API 200，size 27632，sha `e8c5606918cd7721967cd87901cf225a7642ee03`）中可读到 300ms 判定片段：`event.when - lastTimePressed.get() > 300` 时 `resetState()`；据此 JetBrains 的**代码层**双击修饰键阈值约为 **300ms**。这不是官方文档承诺，用户仍可通过 `SearchEverywhereAction` 中的 `ide.suppress.double.click.handler` 高级设置避开双击处理器。`【源码】【推断：该值用于双击修饰键判定】`
- 结论：**没有找到 JetBrains 公开的官方阈值；300ms 只能标为源码观察。** `【未证实：官方阈值】`

### 3.2 Windows `GetDoubleClickTime`

- 官方 API 文档：「The double-click time is the maximum number of milliseconds that may occur between the first and second click of a double-click. The maximum double-click time is 5000 milliseconds.」`GetDoubleClickTime` 返回当前双击时间。`【官方】`（[GetDoubleClickTime](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdoubleclicktime)）
- `SetDoubleClickTime(0)` 时「the system uses the default double-click time of 500 milliseconds」；传入大于 5000 时系统按 5000 处理。`【官方】`（[SetDoubleClickTime](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setdoubleclicktime)）

### 3.3 macOS `NSEvent.doubleClickInterval`

- Apple 文档摘要：「The maximum number of seconds in which a second mouse click must occur for an event to be a double-click event.」；类型 `TimeInterval`，macOS 10.6+；正文 Returns 为「The double-click time interval, in seconds.」；Discussion 明确「This is a system setting. You can't change the value by overriding this method.」`【官方】`（[NSEvent.doubleClickInterval](https://developer.apple.com/documentation/appkit/nsevent/doubleclickinterval)）
- **Apple 官方文档未给默认值**；因此「macOS 默认 500ms」在本文只能标为未证实（项目 spec 中的「默认 500ms 左右」应保留为工程假设，不应写成 Apple 规范）。`【未证实】`

### 3.4 对「跟随系统双击间隔、clamp 220–600ms」的判断

- **合理**：220–600ms 覆盖 Windows 默认 500ms 与 JetBrains 源码的 300ms；对一般用户，600ms 上限能挡住「慢速两次 Shift」造成的误触，220ms 下限能挡住键盘抖动。`【推断】`
- **需要写清楚边界**：Windows `GetDoubleClickTime` 最大可返回 5000ms，macOS 是系统设置；用户把双击间隔调到 900ms/1000ms 时，clamp 到 600ms 就不是真正「跟随系统」，而是产品折中。建议在设置/调试日志中记录「系统值 vs 实际使用值」，并在 release note 说明。`【推断】`
- **建议**：系统值可用时优先读取；若 API 不可用/返回 0，回退 500ms；clamp 采用 220–600ms 作为 v1 安全区间；另外用 `MIN_GAP` 80ms 与 `IME_SETTLE` 120ms 做 IME 防抖（后者来自项目 spec，不是外部产品事实）。`【推断】`
- **注意**：操作系统的 `GetDoubleClickTime`/`doubleClickInterval` 是**鼠标双击**语义，不是键盘双击 Shift 的标准；用它作为键盘手势的上限是产品约定，不是平台保证。`【推断】`

---

## ④ 模糊匹配与排序

### 4.1 分层模型（建议 MTerm 采用）

| 层级 | 规则 | 处理 | 适用 |
|------|------|------|------|
| L0 exact | 完整相等（忽略大小写） | 最高分，直接锁定 | 会话别名、命令名、设置标题、动作名 |
| L1 prefix | 查询是标题前缀 | 高权重 | 快速输入前几个字符 |
| L2 word boundary | 命中一个或多个词首（空格、`-`、`_`、`.`、`/` 等后） | 按边界数量和连续性加权 | 命令、设置、动作 |
| L3 substring | 连续子串 | 中权重 | 用户记得完整片段 |
| L4 subsequence | 按顺序但不连续 | 低权重，必须受长度/密度约束 | 长会话名、长路径、模糊记忆 |

- 这四层是**合成建议**，不是某个产品官方文档的原文；VS Code 的 `fuzzyScorer` 采用「匹配/连续/同大小写/词首/分隔符/camelCase」组合，fzf 采用 Smith-Waterman 式最高分，思路可映射到该分层。`【推断】`
- 对 1–2 个字符的查询，建议禁用或降权 L4 subsequence，避免把「随便两个字母」匹配到大量会话；3 个字符以上再逐步启用。`【推断】`
- 短查询优先 exact/prefix；长查询允许 word boundary/substring/subsequence。`【推断】`

### 4.2 高亮

- VS Code `FuzzyScore` 的第二项 `number[]` 是匹配位置，官方源码直接把它用于高亮；MTerm 也应在 scorer 内返回 `match_spans`，而不是在高亮阶段用 `indexOf` 重新猜。`【源码/推断】`
- 多 query piece（空格分隔）时，高亮应只标出每个 piece 的命中，不要把中间 gap 连成一个 span；否则用户会误以为整段连续匹配。`【推断】`
- 高亮样式要与选中态叠加：建议文字色保持高对比，背景用 `Theme.ac` 16% alpha，命中字符用加粗/下划线而不是单独换色，避免亮暗主题不可读。`【推断】`

### 4.3 稳定 tie-break

- VS Code 的 `compareItemsByFuzzyScore` 给出了可运行的 tie-break 顺序：路径完全相同 > 纯 label 命中 > label 前缀 > 匹配紧凑度/短 label > label+description 分 > 匹配距离 > `fallbackCompare`。MTerm 可直接借鉴「先身份、再前缀、再紧凑度、再短标题」的顺序。`【源码】`
- 对 MTerm 还应加两个稳定字段：`kind_rank`（会话 0 < 命令 1 < 设置 2 < 动作 3）与原始索引顺序。相同分数时不再随机；这样 `ListView` 重绘不会跳行。`【推断，与 spec §4/§7 一致】`
- frecency 应作为**最后 additive boost**，不能覆盖 exact/prefix 的层级分；Raycast 的 ranking 与 Alfred 的 4 周 usage fingerprint 都说明「使用频率/最近使用」是排序因子而非匹配替代品。`【官方/推断】`

### 4.4 各数据源的建议权重

| 数据源 | 主匹配字段 | 建议权重 | 备注 |
|--------|------------|----------|------|
| 会话 | title / alias / user@host | exact/prefix 高；substring 中；frecency + opened boost | 已打开标签并入会话，`opened` 提升 |
| 命令 | name / alias / first token | exact/prefix 高；word boundary 高；history frecency | 命令历史可使用 fzf-style scheme=history 思路 |
| 设置 | title / path / keywords | exact/prefix 高；word boundary 高；substring 低 | 设置名称短，少用 subsequence |
| 动作 | title / action id / shortcut keywords | exact/prefix 高；shortcut 文本可作为 keywords | 不可用动作仍显示但灰显（spec §3.4） |

- 以上是 MTerm 的设计建议；VS Code/Raycast/Alfred 的细节分别见对应小节。`【推断】`
- 终端客户端的命令历史尤其适合 `--scheme=history` 式的时间衰减；会话适合 frecency + `opened`；设置/动作更适合严格 exact/prefix，避免「模糊到看不懂」。`【推断】`

---

## ⑤ 对 MTerm 的启示

### 5.1 Adopt（直接采用）

| 做法 | 来源/证据 | MTerm 落地 |
|------|-----------|------------|
| 双击 Shift 全局入口 + 单输入框 | IDEA 官方帮助/Guide | 入口只做「打开搜索」，不依赖当前焦点在终端/侧栏 |
| 默认空查询显示最近使用/最近文件 | IDEA 官方 + Raycast + Alfred | 空查询显示最近会话 + 最近命令，按 kind 分组 |
| All 视图 + 分组标题 | IDEA `SearchEverywhereHeader` / `SearchEverywhereUI` | 「全部」按 kind 分组，每组最多 8、总 40（与 spec 一致） |
| 键盘优先（Tab/Shift+Tab、Enter、Esc） | IDEA Guide + MTerm spec | `Tab/Shift+Tab` 循环分类，`Enter` 主操作，`Esc` 关闭 |
| 预览跟随选中，不抢焦点 | IDEA `PreviewAction` + `SearchEverywherePreviewGenerator` | 右侧预览面板显示会话/命令/设置详情；打开预览不改变终端焦点 |
| 使用习惯/最近使用加权 | Raycast frecency + Alfred 4 周指纹 | 按 session/command 的最近使用与频率排序，但只做 additive boost |
| 模糊评分返回 match positions | VS Code `FuzzyScore` | scorer 返回 `match_spans`，UI 只渲染 |
| 稳定 tie-break | VS Code `compareItemsByFuzzyScore` | `kind_rank` + 原始索引 + recency 的确定顺序 |

### 5.2 Adapt（按 MTerm 改造）

| IDEA/竞品原做法 | MTerm 改造 | 理由 |
|----------------|------------|------|
| IDEA 的 Classes / Files / Symbols / Actions / Database / Text / URL tabs | MTerm 五类 tab：全部 / 会话 / 命令 / 设置 / 动作；已打开标签并入会话 | 终端客户端没有「类/符号/数据库」，多了「命令/会话」；五类正好对应 `Alt+1..5` |
| IDEA 的 Include non-project items | MTerm 的「包含未打开会话 / 全部命令历史」范围开关 | 终端项目的「非项目项」是未打开会话/全局命令，不是文件系统 |
| IDEA PreviewAction 的右侧编辑器预览 | MTerm 右侧轻量详情面板（会话信息、命令说明、设置路径、动作快捷键） | 终端客户端没有代码编辑器；预览必须廉价、不能拖慢 8ms 目标 |
| IDEA 的 per-tab 搜索历史 | 每个 kind 保留最近 query 历史，供空输入/↑↓回忆 | 与终端用户重复搜索命令的习惯一致 |
| Raycast Action Panel 的多层操作 | `Enter` 主操作、`Ctrl+Enter` 次操作（如打开新标签、填入命令栏、复制设置路径、显示动作快捷键） | 与 spec §6 键盘表一致；避免每个结果再加二级菜单 |
| Windows Terminal 的 `>` 命令模式 | 在 MTerm 中保留 `>` 作为命令/动作前缀（可选） | 终端用户对 `>` 有认知；但 v1 可先不加，保持输入即搜索 |

### 5.3 Avoid（不要照搬）

| 不要照搬 | 原因 | 替代 |
|----------|------|------|
| 照搬 IDEA 的 300ms 固定阈值 | JetBrains 源码是 300ms，但官方未公开；终端用户的键盘/IME 差异更大 | 跟随系统双击间隔 + clamp 220–600ms，日志记录系统值 |
| 无结果时静默 `autoSetEverywhere(true)` | IDEA 源码会在无结果时自动扩大 scope，用户可能不知道搜索已越界 | 显示「已扩大范围」的 chip/提示，或明确开关 |
| 每次按键重建索引 | 8ms 键入到列表更新目标不允许 | 索引与 query 解耦；按键只做 query/rank（spec §8.2 已定） |
| 用纯 subsequence 排序短查询 | 1–2 个字符会把大量结果排到顶部 | 短查询走 exact/prefix/word boundary |
| 给动作/设置加过多预览和动画 | 终端是低延迟工具，弹层动画会干扰 Alt+Tab/终端输入 | 预览只在选中变化时更新；120/140ms 内完成 |
| 让 IME 组合态的 Shift 事件参与双击判定 | JetBrains IJPL-251845 已证明会误触发 Search Everywhere | 组合态短路；`IME_SETTLE` 120ms；见 §6 |

### 5.4 IDEA 分类 Tab + 分组结果 + 预览 + 键盘优先如何映射到 MTerm 五类

- **IDEA 的 All tab** → MTerm 的「全部」视图：`SearchKind` 四类分组显示；每组最多 8、总 40（spec §4/§7）。`【源码/推断】`
- **IDEA 的独立 contributor tab** → MTerm 的「会话 / 命令 / 设置 / 动作」四个 tab；`Alt+1..5` 对应「全部/会话/命令/设置/动作」，与 spec §6 一致。`【推断】`
- **已打开标签** → 不单独设 tab，并入「会话」的 `opened` 字段；主操作 `ActivateSession`，次操作 `OpenSessionTab`（spec 的 `SearchAction`）。`【推断】`
- **IDEA 的 PreviewAction** → MTerm 右侧预览面板，随选中项更新；预览不抢焦点，`Esc` 关闭时焦点回终端。`【推断】`
- **IDEA 的 per-tab 快捷键 tooltip** → MTerm 可在 tab 上显示 `Alt+1..5`；不要求用户记 IDEA 的 GotoClass/GotoFile 组合。`【推断】`
- **IDEA 的 Include non-project** → MTerm 的「范围」开关：是否包含未打开会话、全部命令历史、全局设置；默认关闭，开关状态在本次会话内记忆。`【推断】`

---

## ⑥ IME 与 Shift 冲突

- **JetBrains IJPL-251845**：`Double-modifier shortcuts (double Shift / double Ctrl) misfire while typing with an IME`，环境为 WebStorm 2026.2.0.1（WS-262.8665.341）、Debian Linux KDE Plasma 6.7.2 Wayland、fcitx5 + fcitx5-skk 日文、KDE virtual keyboard，`XMODIFIERS`/`GTK_IM_MODULE`/`QT_IM_MODULE` 为空。`【官方 issue】`（[IJPL-251845](https://youtrack.jetbrains.com/issue/IJPL-251845)）
- 该 issue 的 Cause 原文：「Characters consumed by the IME never reach the IDE as KeyEvents — they arrive as InputMethodEvents, which ModifierKeyDoubleClickHandler.MyEventDispatcher.dispatch() discards at `if (e !is KeyEvent) return false`. Modifier keys, however, pass through. As a result, a sequence of two `modifier+character` inputs is observed by the handler as a bare double tap of that modifier.」`【源码/issue】`。日志证据：`12:40:01.710 Shift KEY_PRESSED` → `12:40:01.904 Shift KEY_RELEASED (+194ms)` → `12:40:01.961 Shift KEY_PRESSED (+57ms)` → `12:40:02.208 Shift KEY_RELEASED (+247ms)`，四个 Shift 事件之间没有其他 KeyEvent；日文 SKK 常用 `Shift+letter` 与 `Ctrl+J`。`【issue】`
- **JetBrains IJPL-249709**：`Search Everywhere not opening when Double Shift is pressed`，Ubuntu 26.04 / IDEA 2026.1.4；报告者试过 Advanced Settings 的 `Disable double modifier key shortcuts`，仍无效。`【官方 issue】`（[IJPL-249709](https://youtrack.jetbrains.com/issue/IJPL-249709)）
- **VS Code**：本次用 GitHub Issues 搜索 `repo:microsoft/vscode "double shift"` 得到 `total_count: 79`，首条是 `Outline: Shift+click to select the symbol's full range`，与 IME/双击 Shift 无关；未找到可引用的 VS Code IME issue。`【未证实/未找到】`
- **MTerm 现状**：spec 已有 `src/app.rs:12033-12068` 对百度拼音左 Shift `U+0015`、右 Shift `U+0010`、尾随 `U+0008` 的 C0 过滤，以及 `IME_SETTLE` 120ms；本次未做本地 KEY_DIAG，故不能把 JetBrains 的结论直接当 MTerm 结论。`【项目源码/未证实：MTerm 本地实测】`
- **建议**：IME composing 时任何 Shift keydown 都不允许累计双击状态；收到 InputMethodEvent/commit 后重置状态并等待 `IME_SETTLE`；双击判定必须要求两次都是「bare Shift」且中间无字符、无 C0 标记、无鼠标/焦点变化；用 KEY_DIAG 记录 keyCode、scanCode、`is_ime`、`composition`、时间差。`【推断】`
- **结论**：JetBrains 的 IME 误触是真实存在的公开案例；MTerm 必须在设计阶段把「IME 组合态」当作双击 Shift 的一等公民，而不是上线后再补。`【推断/依据 IJPL-251845】`

---

## ⑦ 未证实清单

- **JetBrains 官方双击阈值**：官方帮助页未公开毫秒数；300ms 仅来自 `ModifierKeyDoubleClickHandler.kt` 源码观察，不是 JetBrains 官方承诺。`【未证实】`
- **macOS `NSEvent.doubleClickInterval` 默认值**：Apple 文档只说明这是系统设置、无法通过 override 改变，未给默认值；本项目的「默认 500ms 左右」是工程假设。`【未证实】`
- **IDEA 完整键盘矩阵**：本次官方页在 synonyms 段后截断，未读到完整的 `Ctrl+Down/Ctrl+Up`、`Ctrl+Enter` 等说明；只采用 Guide 明确写出的 Tab 切换、Esc 关闭、鼠标悬停显示直达键，以及源码的 tab 切换/快捷键映射。`【未证实】`
- **Windows Terminal command palette 的官方键盘表**：官方 Learn 页只描述入口、command line mode、icon；本文的 ↑/↓、PageUp/PageDown、Ctrl+Home/End、Backspace/Right 来自 `CommandPalette.cpp` 源码，不是官方文档。`【未证实：官方键盘表】`
- **PowerToys Run 键盘表/空查询/预览**：官方页在 Features 后截断；未取得完整 Settings 表和键盘表，相关行为未证实。`【未证实】`
- **Alfred 完整键盘导航**：`/help/getting-started/keyboard-shortcuts/` 与 `using-alfred` 均 404；只确认默认热键与 Return 启动。`【未证实】`
- **Raycast 默认全局快捷键冲突**：`search-bar` 页写 `⌘Space / CtrlSpace`，`keyboard-shortcuts` 页写 `⌘ Space` / `Alt Space`；本文保留两处，未判定哪一个是最终默认。`【未证实】`
- **VS Code IME 双击 Shift issue**：GitHub Issues 搜索被 shift+click 结果污染，未找到可引用 issue，不能据此说「VS Code 没有这个问题」。`【未证实】`
- **性能/索引**：除 fzf README 的 marketing 句和 VS Code/Raycast 的官方索引说明外，IDEA/Windows Terminal/PowerToys 的索引实现均未在本次抓取中公开。`【未证实】`

---

## 参考链接

### JetBrains

- [Search everywhere（官方帮助）](https://www.jetbrains.com/help/idea/searching-everywhere.html)
- [How to Search Everywhere in Any JetBrains IDE（JetBrains Guide）](https://www.jetbrains.com/guide/tips/search-everywhere/)
- [SearchEverywhereAction.java](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/SearchEverywhereAction.java)
- [SearchEverywhereManagerImpl.java](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereManagerImpl.java)
- [SearchEverywhereHeader.java](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereHeader.java)
- [SearchEverywhereUI.java](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereUI.java)
- [SearchEverywhereTabsShortcutsUtils.kt](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywhereTabsShortcutsUtils.kt)
- [PreviewAction.kt](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/PreviewAction.kt)
- [SearchEverywherePreview.kt](https://github.com/JetBrains/intellij-community/blob/master/platform/lang-impl/src/com/intellij/ide/actions/searcheverywhere/SearchEverywherePreview.kt)
- [ModifierKeyDoubleClickHandler.kt](https://github.com/JetBrains/intellij-community/blob/master/platform/platform-impl/src/com/intellij/openapi/keymap/impl/ModifierKeyDoubleClickHandler.kt)
- [IJPL-251845（IME 下双击 Shift/Ctrl 误触）](https://youtrack.jetbrains.com/issue/IJPL-251845)
- [IJPL-249709（Double Shift 无法打开 Search Everywhere）](https://youtrack.jetbrains.com/issue/IJPL-249709)
- [IDEA-148090（Double shift not work，历史 issue）](https://youtrack.jetbrains.com/issue/IDEA-148090)

### VS Code

- [VS Code userinterface.md](https://github.com/microsoft/vscode-docs/blob/main/docs/editing/getting-started/userinterface.md)
- [VS Code tips-and-tricks.md](https://github.com/microsoft/vscode-docs/blob/main/docs/editing/getting-started/tips-and-tricks.md)
- [VS Code fuzzyScorer.ts](https://github.com/microsoft/vscode/blob/main/src/vs/base/common/fuzzyScorer.ts)

### Windows Terminal / Windows API

- [Windows Terminal command palette](https://learn.microsoft.com/en-us/windows/terminal/command-palette)
- [CommandPalette.cpp](https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalApp/CommandPalette.cpp)
- [GetDoubleClickTime](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdoubleclicktime)
- [SetDoubleClickTime](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setdoubleclicktime)

### macOS

- [NSEvent.doubleClickInterval](https://developer.apple.com/documentation/appkit/nsevent/doubleclickinterval)

### Raycast

- [Raycast Search Bar](https://manual.raycast.com/search-bar)
- [Raycast Keyboard Shortcuts](https://manual.raycast.com/keyboard-shortcuts)
- [Raycast File Search](https://manual.raycast.com/file-search)

### Alfred

- [Alfred Default Results](https://www.alfredapp.com/help/features/default-results/)
- [Alfred File Search](https://www.alfredapp.com/help/features/file-search/)
- [Alfred Previews](https://www.alfredapp.com/help/features/previews/)
- [Alfred Understanding Result Ordering](https://www.alfredapp.com/help/kb/understanding-result-ordering/)
- [Alfred First 5 Minutes](https://www.alfredapp.com/help/getting-started/first-5-minutes/)

### PowerToys Run / fzf

- [PowerToys Run](https://learn.microsoft.com/en-us/windows/powertoys/run)
- [fzf README](https://github.com/junegunn/fzf/blob/master/README.md)
- [fzf algo.go](https://github.com/junegunn/fzf/blob/master/src/algo/algo.go)
