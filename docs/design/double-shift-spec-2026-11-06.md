# Double Shift 全局搜索 · 交互与实现规格（2026-11-06）

- 状态：**设计定稿 v1（待评审）**
- 范围：MTerm 桌面端主窗口（Rust 1.92 / Slint 1.18.1）
- 配套：`docs/design/double-shift-research-2026-11-06.md`（外部证据）、`docs/design/double-shift-hifi-2026-11-06-v1.html`（可交互高保真）
- 交付边界：本次只交付**高保真视觉稿 + 交互/实现规格**，不修改 `ui/*.slint`、不修改 `src/*`。文中的文件路径与行号是设计依据，不是已落地代码。
- 一句话：**两次 Shift，找到任何东西；搜索的结果可以直接执行。**

---

## 0. 设计判断（先定取舍）

| # | 判断 | 理由 | 对设计的约束 |
|---|------|------|--------------|
| 1 | 键盘不是鼠标的备选，是终端用户的主输入 | MTerm 用户 90% 时间手在键盘上；点菜单→找入口的成本远高于两次 Shift | 打开、输入、切类、选择、执行全链路可不用鼠标；鼠标只作为补充 |
| 2 | 搜索不是"跳转"，是"执行" | 找到会话的下一步是连接，找到命令的下一步是发送/填入，找到设置的下一步是修改 | 结果行必须携带**主操作 + 次操作**，Enter 直接产生结果，不要求二次定位 |
| 3 | 一张本地索引，五类数据 | 会话、命令、设置、动作的数据量都在内存里（会话 <1k、快捷命令 <1k、历史 ≤200、设置 ~40、动作 ~30） | 启动时一次性建索引；按键只查内存；键入期间零磁盘 I/O、零网络 |
| 4 | 慢 16ms 就算卡 | 浮层是高频入口，任何等待都会让用户回到"鼠标找菜单" | 不做防抖、不做骨架屏、不做异步请求；打开首帧 ≤16ms、键入到列表更新 ≤8ms |

设计气质：**IDE 的搜索能力，终端的速度感**。视觉上不做一个独立网页，而是 MTerm 主窗口上方"长出来"的一层；切换时背景仍可见、可感知上下文。

---

## 1. 代码事实基线（设计不能违背的现状）

> 证据等级：源码 > 官方文档 > 推断。以下均来自当前工作区源码。

| 事实 | 位置 | 对设计的硬约束 |
|------|------|----------------|
| 窗口不能绑定 `key-pressed`，由包住主内容的 `shortcut-scope: FocusScope` 兜底未被后代消费的快捷键 | `ui/app.slint:2511-2521` | 双击 Shift 的窗口级接收点只能放在 `shortcut-scope` 或各输入控件内；不能假设 `Window.key-pressed` |
| 终端 `TerminalView` 的隐藏 `TextInput` 对**每个键**返回 `accept` 并 `send-key` 转发，只有 IME commit 走 `edited` | `ui/terminal_view.slint:2250-2261` | 终端内双击 Shift 必须在 `TerminalView.key-pressed` 内先判断，不能依赖事件冒泡到窗口层 |
| 裸修饰键在 Slint 中 `key=""`；现有代码已记录 lone-Shift 时间戳，用于 IME 注入 Backspace 的过滤 | `src/app.rs:11908-11910`、`12028-12031` | 双击 Shift 检测可复用这条事件，但**不得破坏**既有 `last_shift_time` 的 Backspace 过滤语义 |
| 百度拼音在 Shift 按下时注入 C0：左 Shift `U+0015`、右 Shift `U+0010`，可能尾随 `U+0008`（Backspace）；代码把 0x01–0x1f 的非独立 C0 视为 IME 标记并吞掉 | `src/app.rs:12033-12068` | 触发状态机必须同时兼容 `key=""` 与 C0 标记两条来源，并识别 C0+Backspace 污染序列 |
| 现有快捷键：Ctrl+Tab 切标签、Ctrl+B 左栏、Ctrl+Alt+B 右栏、Ctrl+F 查找、Ctrl+Shift+R 历史搜索、Ctrl+Shift+N 新窗口、Ctrl+Alt+Z 禅模式、Ctrl+=/-/0 字体缩放、Ctrl+Shift+G 分组管理、Ctrl+Shift+E 发送命令 | `ui/terminal_view.slint:2311-2422`、`ui/app.slint:2544-2600` | 双击 Shift 无冲突；建议保留 `Ctrl+Shift+Space` 作为 IME 用户的等价入口（当前未占用） |
| `TabInfo` 字段：`id/title/kind/connected/session_id/state/group_color_hex/host/user/auth/group_name` 等 | `ui/tabs.slint:5-46` | "已打开"标记直接由 `tabs_model` 匹配 `session_id`，不额外维护状态 |
| `SessionInfo` 字段：`id/name/kind/host/port/user/auth/group-id/group/depth/collapsed/builtin/conn-state` | `ui/left_sidebar.slint:5-45` | 搜索索引使用**未折叠的完整会话表**（`ConfigStore.sessions()`），不能用左侧树拍平后的可见行，否则折叠组搜不到 |
| `Session` 结构：`id/name/host/port/user/auth/password/private_key_path/proxy/jump_session_id/jump_chain/last_used/group_id/group` | `src/config/struct/session.rs:176-212` | 会话搜索结果可显示 用户@主机:端口 / 分组 / 认证 / 最近使用 |
| `QuickCommand`：`name/command/group/note/send_enter` | `src/config/struct/quick_command.rs:5-21` | 命令结果的主操作按 `send_enter` 决定"发送并回车"还是"只填入" |
| `HistoryEntry`：`cmd/last_used/uses/pinned`；容量 200，存储序最旧在前 | `src/config/struct/config_file.rs:49-64`、`src/config/impls/config.rs:319` | 历史命中按 `last_used` + `uses` 加权；`pinned` 置顶可优先 |
| 命令提示框（内联补全）只保留 exact 100 / prefix 80，刻意不支持模糊；来源优先级 快捷命令 0 < 内置 1 < 历史 2 | `src/command_hint/impls/suggest.rs:86-97` | **全局搜索不是内联补全**：必须支持 word/substring/subsequence（IDEA 心智）；但沿用"分层纯逻辑 + 稳定 tie-break + 用户来源优先"的纪律，不能把两套语义混用 |
| 领域层分层：`struct/model.rs` 纯数据、`struct/knowledge.rs` 静态表、`impls/suggest.rs` 纯逻辑；宿主桥 `src/app/command_hint_ui.rs`；测试按特性归档 `tests/app/command_hint/mod.rs` | `src/command_hint/mod.rs:18-56` | `src/search/` 按同一模式拆；UI 桥独立；纯函数可测 |
| 设置面板左导航页面：外观 / 显示与字体 / 输入 / 渲染 / 输出高亮 / SFTP / WSL / 侧栏 / 会话同步 / WebDAV / 下载 / 窗口 / 数据目录 / MCP | `ui/interface_panel.slint:1191-1240` | 设置索引的 `page` 直接使用 `ifd-page` 的 id（`theme`/`font`/`input`/...），打开设置时设置 `ifd-page` 即可直达 |

**结论**：双击 Shift 是一个"窗口级手势 + 终端内必须提前拦截"的问题；搜索是"内存索引 + 纯函数排序"的问题；两者都不需要新增大依赖。

---

## 2. 触发：双击 Shift 状态机

### 2.1 状态定义

```text
Idle ──Shift↓(t0)──> Armed ──Shift↓(t1)──> Triggered ──> Idle
  ^                    │
  └── 任意真实按键 / 鼠标按下 / 窗口失焦 / Esc / 超过窗口 ──┘
```

| 参数 | 值 | 说明 |
|------|-----|------|
| `MIN_GAP` | 80 ms | 低于此间隔视为一次抖动/连发，不计第二击 |
| `MAX_GAP` | 360 ms（固定） | IDEA `ModifierKeyDoubleClickHandler.kt` 采用 300ms；这里留 60ms 宽容窗，不依赖系统双击设置，保证不同机器/远程桌面一致；手速偏慢的用户可用 `Ctrl+Shift+Space` 等价入口 |
| `IME_SETTLE` | 120 ms | 仅 IME C0 标记来源需要等 120ms 确认没有 Backspace 尾随 |

### 2.2 事件规则

1. **裸 Shift**（`key == "" && shift && !ctrl && !alt && !meta && !repeat`）：
   - 第一击 → 记录 `t0`，进入 `Armed`，不发给 PTY（裸 Shift 本来也不产生字节）。
   - 第二击且 `MIN_GAP ≤ t1-t0 ≤ MAX_GAP` → **在 keydown 时立即触发**，打开浮层，吞掉该事件。
   - 超过 `MAX_GAP` → 以第二击为新 `t0`，重新 `Armed`（不是清零）。
2. **IME C0 标记**（`key` 为单字符 0x15/0x10，`!ctrl && !alt`）：
   - 作为**候选 tap** 进入 `PendingIme`，等待 120ms。
   - 120ms 内收到 `0x08`（Backspace）→ 判定为百度拼音的"Shift 误触 + 退格"污染，撤销候选，不触发。
   - 120ms 内没有污染且与上一 tap 满足间隔 → 计为一次有效 tap。
   - 因此中文输入法用户的双击路径最多多 120ms 延迟；这是为了不误弹浮层。
3. **重置条件**（任一发生即 `Idle`）：
   - 任何非 Shift、非 C0 标记的按键（包括字母、数字、方向键、Enter、Tab、Esc、Ctrl/Alt 组合键）；
   - 鼠标按下（任意按钮）、滚轮、触摸；
   - 窗口失焦、标签切换、打开其它模态框（设置、会话编辑、发送命令、命令管理等）；
   - 浮层已经打开时的任何 Shift 不再触发（`visible == true` 短路）。
4. **抑制场景**：
   - `Shift+Click` 选择文本：鼠标按下即重置；Shift keydown 也不计入第二击。
   - `Shift+Enter`、`Shift+方向键`：后续按键重置，不会先弹出浮层。
   - IME 组合中（最近 800ms 有 composition/候选窗口活动）：不把 C0 标记计入；裸 Shift 若在组合中也不触发（由输入法消费）。
   - 远程桌面（RDP/VNC）标签内：默认禁用双击 Shift（键盘被远程画面接管），仅保留 `Ctrl+Shift+Space`；避免与远程系统快捷键冲突。

### 2.3 接收点与去重

| 焦点位置 | 接收点 | 处理 |
|----------|--------|------|
| 终端（`TerminalView`） | `terminal_view.slint` 的 `key-pressed`，在现有快捷键链最前面加 `lone-Shift` 分支 | 通过 `callback shift-tap()` 上报，**不**调用 `send-key` |
| 主窗口非终端区域（侧栏空白、标签栏、欢迎页） | `ui/app.slint` 的 `shortcut-scope.key-pressed` | 同样上报到宿主桥 |
| 侧栏搜索框 / 命令栏等 `TextInput` | 若裸 Shift 被 TextInput 消费（POC 验证），给该控件加 `shift-tap()` 回调 | v1 至少覆盖侧栏搜索框、底部命令栏两处 |
| 模态对话框打开时 | 统一在宿主桥判断 `modal_open` | 直接忽略，不启动状态机 |

**去重**：`src/search/impls/trigger.rs` 的 `ShiftTapDetector` 是唯一状态机；两条 UI 路径都调用同一个 `observe_shift_tap(source, at)`。若同一物理事件被两处上报，`dedupe_window = 30ms` 内相同 source 只计一次。

### 2.4 备选入口

- `Ctrl+Shift+Space`：与双击 Shift 完全等价，给 IME 用户和 RDP 标签用；加入动作表，可在设置中改键（v2）。
- 标题栏/侧栏的搜索图标点击：打开浮层，照顾鼠标用户。
- 浮层内 `Esc` 关闭并把焦点还给打开前的控件；关闭后 250ms 内不响应双击 Shift（防止连击重开）。

---

## 3. 索引模型

### 3.1 五类数据

| 分类 | 数据源 | 估算条数 | 主操作 | 次操作 |
|------|--------|----------|--------|--------|
| 会话 `session` | `ConfigStore.sessions()` + `tabs_model` 匹配已打开 | 9–500 | 连接 / 激活已打开标签 | 新标签打开 |
| 命令 `command` | `store.quick_commands()` + `store.history_entries()`（≤200） | 10–1,000 | 发送到当前会话（按 `send_enter`） | 只填入命令栏 |
| 设置 `setting` | `src/search/struct/sources.rs` 静态表，映射 `ifd-page` | 30–60 | 打开对应设置页并定位 | 复制设置路径 |
| 动作 `action` | `ACTION_ENTRIES` 静态表（见 §3.4） | 20–40 | 执行 | 查看快捷键 |

### 3.2 `SearchItem` 数据契约

```rust
// src/search/struct/model.rs —— 纯数据，不依赖 Slint
pub(crate) enum SearchKind { Session, Command, Setting, Action }

pub(crate) struct SearchItem {
    pub id: String,          // 稳定唯一：session:<id> / cmd:q:<idx> / cmd:h:<idx> / set:<page>:<anchor> / act:<id>
    pub kind: SearchKind,
    pub title: String,       // 主标题：会话名 / 快捷命令名 / 设置项标题 / 动作名
    pub subtitle: String,    // 会话 user@host:port / 命令原文 / 设置路径 / 快捷键+说明
    pub keywords: String,    // 预拼接的可搜索关键词（名称、分组、host、备注、别名）
    pub group: String,       // 会话分组 / 命令分组 / 设置页 / 动作分类
    pub action: SearchAction,
    pub recency: i64,        // unix 秒；0 = 未知
    pub opened: bool,        // 会话是否已在 tabs_model 中
    pub shortcut: String,    // 动作快捷键的显示串（其它为空）
}

pub(crate) enum SearchAction {
    ActivateSession { session_id: String },
    OpenSessionTab { session_id: String },
    SendCommand { command: String, send_enter: bool },
    FillCommand { command: String },
    OpenSetting { page: String, anchor: String },
    CopySettingPath { path: String },
    RunAction { action_id: &'static str },
}
```

### 3.3 索引构建与刷新

- `SearchIndex::build(&ConfigStore, &tabs_model)` 在窗口创建时调用一次；纯内存，目标 ≤30ms（5,000 条）。
- 刷新触发：
  - 会话新增/删除/改名、分组变更 → `refresh_sessions()`；
  - 快捷命令/历史变化 → `refresh_commands()`；
  - 标签打开/关闭/切换 → 只更新 `opened` 位与 recency，不重建全表；
  - 设置值变化 → 只更新对应条目的 `subtitle`（值延迟到预览时读也可）。
- 不做后台线程全量重建：数据源都在 `ConfigStore` 内存缓存里，重建 5k 条在毫秒级；避免线程同步复杂度。
- 索引容量上限 8,000 条（历史 200 + 快捷命令 + 会话 + 静态表，远超实际）；超过时按 kind 丢弃最旧历史。

### 3.4 动作表（首批 19 个）

| id | 名称 | 快捷键 | 可用条件 |
|----|------|--------|----------|
| `new-window` | 新建窗口 | Ctrl+Shift+N | 总是 |
| `new-session` | 新建会话 | Ctrl+N | 左栏可见 |
| `toggle-left` | 切换左侧栏 | Ctrl+B | 总是 |
| `toggle-right` | 切换右侧栏 | Ctrl+Alt+B | 总是 |
| `zen` | 禅模式 | Ctrl+Alt+Z | 总是 |
| `open-settings` | 打开设置 | — | 总是 |
| `open-command-manager` | 打开命令管理 | — | 总是 |
| `open-send-command` | 打开发送命令窗口 | Ctrl+Shift+E | 总是 |
| `toggle-theme` | 切换主题（深/浅） | Ctrl+Alt+T | 总是 |
| `font-inc` | 字体放大 | Ctrl+= | 存在终端 |
| `font-dec` | 字体缩小 | Ctrl+- | 存在终端 |
| `font-reset` | 重置字体 | Ctrl+0 | 存在终端 |
| `open-history` | 打开命令历史 | Ctrl+R | 存在终端 |
| `find` | 终端查找 | Ctrl+F | 存在终端 |
| `ping` | Ping 检测 | — | 当前会话存在 |
| `system-info` | 系统信息 | — | 当前会话已连接 |
| `process-manager` | 进程管理 | — | 当前会话已连接 |
| `reconnect` | 重连当前会话 | — | 当前标签已断开 |
| `about` | 关于 MTerm | — | 总是 |

> 动作的 `可用条件` 在索引构建时求值；不可用的动作**仍然可搜到**，但结果行灰显并在预览中说明"当前不可用"，Enter 不执行（IDEA 同款：先找到，再解释为什么不能用）。

### 3.5 设置索引

- 静态表放 `src/search/struct/sources.rs`；每条：
  ```rust
  pub(crate) struct SettingEntry {
      pub page: &'static str,     // 对应 ui/interface_panel.slint 的 ifd-page
      pub anchor: &'static str,   // 设置行 id，用于滚动定位
      pub title: &'static str,    // "主题" / "字体" / "多行粘贴确认" ...
      pub path: &'static str,     // "外观 › 主题"
      pub keywords: &'static str, // 英文/别名/旧名称
  }
  ```
- 首批覆盖 14 个设置页的 40+ 行；v1 至少包含：主题、强调色、字体、字号、光标样式、光标闪烁、多行粘贴确认、快捷键›全局搜索、显示资源监控、连接超时、SFTP 下载目录、命令完成通知、日志级别、界面缩放、壁纸、透明度、减少动效。
- 打开设置：设置 `window.ifd-page = page`，再调用 `scroll-to-setting(anchor)`（`ui/interface_panel.slint` 内已有 ScrollView，加一个 `in property <string> focus-anchor` + `changed` 滚动即可；不改动其它设置逻辑）。

---

## 4. 查询与排序

### 4.1 归一化

```text
raw query
  → trim
  → 按空白切分（全角空格 U+3000 也切）
  → 每 token 截断到 64 字符；最多 8 个 token
  → 小写（Unicode simple lowercase；中文不受影响）
  → 空 query = 最近使用模式
```

### 4.2 匹配层级

对每个 item 的每个字段（title / keywords / subtitle，权重 1.0 / 0.85 / 0.7）：

| 命中方式 | 基础分 | 说明 |
|----------|--------|------|
| 精确 | 1200 | 字段小写 == token |
| 前缀 | 1000 | `field.starts_with(token)` |
| 词边界 | 850 | token 出现在 ` `、`-`、`_`、`.`、`/`、`:`、`›` 之后 |
| 子串 | 700 | `field.contains(token)` |
| 关键词前缀 | 650 | 仅 keywords 字段的前缀（避免 title 前缀重复计分） |
| 关键词子串 | 550 | 仅 keywords 字段 |
| 子序列 | 380 | token 长度 ≥2，字符按序出现且间隔 ≤ 8（避免跨半屏乱匹配） |

- 每个 item 的 token 得分 = `max(tier_score × field_weight)`；多 token 取**几何平均**（惩罚只命中一个 token 的结果）：`score = (∏ token_score)^(1/n)`。
- 拼音：keywords 预拼接全拼与首字母（如 主题 → `zhuti zt`；新建窗口 → `xinjianchuangkou xjck`；查看内存 → `chakan neicun chakanneicun cknc`）；多音节标题同时收录分词与连写两种形式，覆盖不同输入习惯。拼音命中走 keywords 档（前缀 650 / 子串 550），不单独提权。
- 子序列命中额外检查：所有字符必须落在同一字段；title 命中优先于 keywords。
- 不匹配任何 token → 排除。

### 4.3 加成与排序

```text
final = base_score
      + min(60, recency_bonus)        // 最近使用：7 天内线性 60→0；无时间戳 0
      + (opened ? 40 : 0)             // 已打开标签
      + (pinned ? 25 : 0)             // 历史命令置顶
```

排序键（稳定）：

1. `final` 降序；
2. `kind_rank`：会话 0 < 命令 1 < 设置 2 < 动作 3（平手时用户资产优先于系统项）；
3. `title` 字典序（中文按 Unicode，确定性优先于"感觉"）；
4. `id` 字典序（最终兜底，保证跨平台稳定）。

### 4.4 结果组织

| 视图 | 规则 |
|------|------|
| 全部 | 按 kind 分组渲染：会话 → 命令 → 设置 → 动作；每组最多 8 条；总计最多 40 条 |
| 单分类 | 该 kind 最多 40 条；其余 kind 不渲染 |
| 空 query | "最近搜索" chips（本地存储最近 4 个非空 query）+ "最近使用" 5 条（跨 kind，按 recency/opened 排） |
| 无结果 | 不渲染分组头；显示空态 |
| 命令去重 | 同一 `command` 字符串同时出现在快捷命令与历史时，保留快捷命令（有 name/group 更可读），历史条目降级为副标题来源 |

### 4.5 高亮

- 每个结果行返回 `title_spans` 和 `subtitle_spans`（`Vec<Range<usize>>`，字节或字符索引需统一为字符索引）。
- 多 token 命中可能重叠：合并区间后渲染。
- 子序列命中：每个字符单独一个 span（视觉上逐字高亮），但相邻字符可合并为连续区间以减少节点数。
- UI 侧只做区间切片 + `Text` 的 `color` 分段；禁止在 Rust 里拼 HTML。

---

## 5. UI 结构

### 5.1 组件层次

```text
ui/app.slint
└── AppWindow
    ├── ... 既有 titlebar / sidebar / tabs / terminal ...
    ├── QuickSearchOverlay { visible: root.search-visible; ... }   // 只加一个实例
    └── shortcut-scope（既有）

ui/quick_search.slint
├── QuickSearchOverlay        // 遮罩 + 920×568 面板 + 焦点管理
│   ├── SearchInput            // TextInput + 搜索图标 + Esc 键帽
│   ├── KindTabs               // 全部/会话/命令/设置/动作 + 数量徽标 + 指示条
│   ├── ResultList             // ListView + 分组头 + SearchRow
│   ├── PreviewPanel           // 标题/meta 表格/描述/主次按钮
│   └── FooterBar              // 键盘提示 + 索引统计
├── SearchRow
├── KindTab
├── KeyCap
└── PreviewPanel
```

### 5.2 尺寸（与高保真稿一致）

| 元素 | 值 |
|------|-----|
| 面板 | 920 × 568 px，top 76px 居中，圆角 10px，1px 边框 |
| 遮罩 | `rgba(5,6,8,.46)`，120ms |
| 头部 | 52px；18px 搜索图标；输入 15px；Esc 键帽 22×20px |
| 分类条 | 38px；tab 高 30px；2px 指示条 |
| 列表 / 预览 | 56% / 44%，中间 1px 分隔 |
| 分组头 | 24px；10px 大写字距 0.08em |
| 结果行 | 48px；图标 28px；标题 13px；副标题 11.5px |
| 预览 | padding 20px；标题 15px；meta label 10px / value 12px；代码块 12px 等宽 |
| 脚注 | 34px；10.5px 文本 |
| 动效 | 遮罩 120ms；面板 140ms `cubic-bezier(.2,0,0,1)`；内容 60ms |

### 5.3 焦点与层级

- 浮层是 `PopupWindow` 还是主窗口内绝对定位？**v1 用主窗口内绝对定位**，理由：
  - `PopupWindow` 在 Windows 上有独立原生窗口，首次创建可能 20–50ms，违背 16ms 首帧；且主题/缩放同步麻烦。
  - 主窗口内绝对定位可以复用现有 `Scrim`/`Rectangle`，与 `settings_dialog.slint` 的嵌入模式一致（该文件注释明确"In embedded panel, not a window"）。
- 打开时记录 `previous-focus-id`；`search-input.focus()` 用下一帧延迟（参考 `ui/terminal_view.slint:1670-1676` 的 `focus-defer-timer` 模式，避免 IME/AccessKit 读到半建布局）。
- 关闭时 `previous-focus.focus()`；若原控件已销毁，焦点回到终端 `ime-input`。
- 层级：`z-index` 概念在 Slint 用声明顺序；`QuickSearchOverlay` 放在主内容之后、其它模态框之前。设置/会话对话框打开时浮层不可见（`modal_open` 短路）。

### 5.4 主题

- 全部颜色引用 `Theme.*`，不写死：
  - 面板底 `Theme.panel`；列表底 `Theme.root`；选中 `Theme.ac` 的 16% alpha + 左侧 2px `Theme.ac`；
  - 主文字 `Theme.t1`；副文字 `Theme.t2`；弱文字 `Theme.t3`；分隔 `Theme.line`；
  - 成功/警告/错误用 `Theme.ok/wr/dg`。
- 亮色主题自动跟随；高保真稿提供 `data-theme` 切换用于验收对比。
- 对比度：主文字对面板 ≥7:1，副文字 ≥4.5:1，选中行文字 ≥4.5:1；用现有 `contrast.mjs` 口径检查。

---

## 6. 键盘与鼠标

| 按键 | 行为 | 备注 |
|------|------|------|
| `↑` / `↓` | 上/下移动选中 | 循环？**不循环**，到头停住（IDEA 同款，避免误跳） |
| `Ctrl+P` / `Ctrl+N` | 同上 | Emacs 风格，兼容终端用户习惯 |
| `Home` / `End` | 跳到列表首/尾 | |
| `PageUp` / `PageDown` | 翻页（每页 8 行） | |
| `Enter` | 主操作 | 会话=连接/激活；命令=发送；设置=打开；动作=执行 |
| `Ctrl+Enter` | 次操作 | 会话=新标签（已打开则复制 SSH 命令）；命令=填入命令栏；设置=复制路径；动作=显示快捷键并关闭 |
| `Tab` / `Shift+Tab` | 循环切换分类 | `preventDefault`；焦点不离开面板 |
| `Alt+1..5` | 直选分类 | 全部/会话/命令/设置/动作 |
| `Esc` | 关闭 | 焦点归还 |
| 鼠标单击行 | 选中并预览 | 不执行 |
| 鼠标双击行 | 执行主操作 | 与 Enter 等价 |
| 点击遮罩 | 关闭 | 不执行 |
| 滚轮 | 列表滚动 | 选中项不变（避免滚动导致误执行） |

---

## 7. 执行语义

| 分类 | Enter | Ctrl+Enter | 失败/不可用 |
|------|-------|------------|-------------|
| 会话（未打开） | 新建标签并连接；关闭浮层；toast「正在连接 …」 | 新建标签并连接（强制新标签，不替换当前） | 会话不存在 → toast「会话已被删除」并刷新索引 |
| 会话（已打开） | 激活对应标签；关闭浮层 | 复制 `ssh user@host -p port` 到剪贴板 | 标签已关闭 → 回退为新建连接 |
| 命令（快捷命令） | 发送到当前活动会话；`send_enter` 决定是否回车；关闭浮层 | 只填入底部命令栏，不发送 | 无活动终端 → toast「请先打开一个终端」 |
| 命令（历史） | 同上，发送后把该条移到历史顶部 | 只填入命令栏 | 同上 |
| 设置 | 打开设置面板并滚动到对应行；关闭浮层 | 复制设置路径「外观 › 主题」 | 页面不存在 → 打开设置首页 |
| 动作 | 执行动作；关闭浮层 | 打开快捷键说明（toast/inline） | 条件不满足 → 结果行灰显，Enter 只 toast 原因 |

**执行后行为**：统一 `close_palette(restore_focus: true)`；动作如果会改变焦点（如打开设置），则不恢复旧焦点。所有执行路径打点 `search.execute kind=... id=... ms=...`，便于验收。

---

## 8. 性能与"无卡顿"

### 8.1 预算

| 指标 | 预算 | 测量方式 |
|------|------|----------|
| 打开首帧（按键到面板可见） | P95 ≤16ms | 宿主桥记录 `Instant::now()` 到 `search-visible=true` 后首帧回调 |
| 键入到列表更新 | P95 ≤8ms（5,000 条） | 每次 `query-changed` 记录搜索耗时；UI 更新在同一帧 |
| 索引构建 | ≤30ms（5,000 条） | 启动日志 `search.index_build_ms` |
| 滚动 | 60fps（16.7ms/帧） | 40 行固定高度，无每行阴影/模糊 |
| 额外内存 | ≤8MB（5,000 条） | 估算：每条约 300B 字符串 + 结构体 ≈ 1.5MB；预留 4 倍余量 |
| 压力测试 | 6,000 条合成 + 原有 ≈ 6,842 条，`web` 查询 <16ms | 高保真稿内置压力按钮；实现后用 `cargo test --release` 基准 |

### 8.2 为什么不做这些

| 常见做法 | 为什么不做 |
|----------|------------|
| 输入防抖 150–300ms | 防抖等于人为制造卡顿；本地索引查询 <1ms，不需要 |
| 骨架屏/加载态 | 索引已就绪，打开即完整；骨架屏只会闪一下 |
| 异步搜索线程 | 5k 条纯内存搜索 <3ms，线程切换和模型回传反而更慢、更复杂 |
| 每次按键重建索引 | 索引与 query 解耦；按键只走 query/rank |
| 虚拟列表 | v1 最多渲染 40 行，固定行高；`ListView` 已足够，虚拟化增加复杂度 |
| WebView/前端框架 | 原生 Slint 组件，零 WebView 启动成本 |

### 8.3 实现要点

- 打开浮层只做 `visible = true` + `focus()` + `request-redraw()`；不构建 DOM、不加载资源。
- 结果模型 `ModelRc<SearchRowData>` 一次性替换；只在选中项变化时更新预览属性，避免整列表重绘。
- 搜索在 Rust 侧同步执行，`query-changed` 回调内完成；UI 线程只做属性赋值。
- 日志：`search.open_ms`、`search.query_ms`、`search.index_build_ms` 用 `tracing::debug!`；发布版不显示在 UI，开发版可在脚注显示。

---

## 9. 项目结构（落地时）

```text
src/
├── lib.rs                       # + pub mod search;（在 command_hint 之后）
├── app.rs                       # + mod search_ui;（一行）
├── app/
│   └── search_ui.rs             # 宿主桥：SearchIndex ↔ Slint 属性/回调；执行动作
└── search/
    ├── mod.rs                   # 模块导出 + 文档注释 + 测试路径声明
    ├── struct/
    │   ├── model.rs             # SearchKind / SearchItem / SearchAction / SearchRowData
    │   └── sources.rs           # SETTING_ENTRIES / ACTION_ENTRIES 静态表
    └── impls/
        ├── index.rs             # SearchIndex::build / refresh_* / 最近使用
        ├── query.rs             # normalize / tokenize / match_spans
        ├── rank.rs              # tier 评分 / 加权 / 排序 / 分组截断
        └── trigger.rs           # ShiftTapDetector 状态机（纯逻辑，可单测）

ui/
└── quick_search.slint           # 纯视觉组件；不写业务判断

tests/
└── app/search/
    └── mod.rs                   # 纯函数测试：触发/评分/排序/截断/高亮
```

**接入点（最小改动）**：

- `src/lib.rs`：加 `pub mod search;`（1 行）。
- `src/app.rs`：加 `mod search_ui;`（1 行），并在窗口初始化处调用 `search_ui::install(&window, store, tabs_model)`。
- `ui/app.slint`：加 `QuickSearchOverlay { ... }` 实例（约 10 行）+ `open-quick-search` 回调（2 行）+ 两个属性（`search-visible`、`search-query`）。
- `ui/terminal_view.slint`：在 `key-pressed` 最前加 `lone-Shift` 分支（约 4 行）+ `callback shift-tap()`（1 行）。
- 其余文件不改。

**为什么这样拆**：对齐 `src/command_hint/` 的 `struct/ + impls/` 分层与 `tests/app/command_hint/` 的按特性测试归档；UI 只做渲染，业务逻辑（排序、执行决策、触发）全部在 Rust 纯函数里，方便单测和后续复用（例如 v2 的 SFTP 文件搜索只需加一个 source）。

---

## 10. 无障碍与国际化

- 键盘：所有功能可无鼠标完成；焦点可见（2px `Theme.ac` 外轮廓，offset 2px）。
- 屏幕阅读器（Slint AccessKit）：
  - 浮层根 `accessible-role: dialog`（如 Slint 版本支持；否则用 `accessible-role: pane` + `accessible-label`）；
  - 输入框 `accessible-role: text-input`，`accessible-label: "全局搜索"`；
  - 结果列表 `accessible-role: list`，行 `accessible-role: list-item`，`accessible-label: title + subtitle`；
  - 结果数量变化时更新 `accessible-description`，例如「找到 12 个结果」。
- 对比度：主文字 ≥7:1、副文字 ≥4.5:1、选中态 ≥4.5:1；不依赖颜色单独传达状态（"已打开"同时有文字徽标）。
- `prefers-reduced-motion` 等价：读取系统动画设置（若 Slint 暴露），关闭面板缩放/位移动画，仅保留 80ms 淡入。
- i18n：所有文案走 `@tr()`；中文/英文 `.po` 同步；搜索关键词表同时包含中英文别名与拼音（全拼/首字母），如 "主题 theme zhuti zt"。

---

## 11. 验收清单

### 11.1 功能

- [ ] 任意主窗口位置双击 Shift（80–360ms 内）打开浮层；单次 Shift、Shift+字母、Shift+Click 不误触。
- [ ] 五类数据均可搜到：会话（含折叠组内）、命令（快捷+历史）、设置、动作。
- [ ] 已打开会话在"全部"中带「已打开」徽标并排在未打开之前。
- [ ] 输入 `web` / `docker` / `主题` / `日志` 均能命中合理结果；无结果时显示无结果态。
- [ ] ↑/↓、Ctrl+P/N、Home/End、PageUp/PageDown、Tab/Shift+Tab、Alt+1..5、Enter、Ctrl+Enter、Esc 全部符合 §6。
- [ ] 预览面板随选中项更新；主/次按钮可点击且语义与键盘一致。
- [ ] 会话 Enter 连接/激活、命令 Enter 发送、设置 Enter 打开、动作 Enter 执行；Ctrl+Enter 次操作正确。
- [ ] 空 query 显示最近搜索 + 最近使用；执行后写入最近搜索。
- [ ] 6000 条压力数据下，`web` 查询 <16ms，UI 不卡顿、不丢帧。
- [ ] 深/亮主题切换，浮层与主窗口一致；无硬编码颜色。

### 11.2 性能

- [ ] 打开首帧 P95 ≤16ms（本机 60Hz，记录 30 次）。
- [ ] 键入到列表更新 P95 ≤8ms（5,000 条，记录 100 次）。
- [ ] 索引构建 ≤30ms（5,000 条）。
- [ ] 滚动 60fps；无布局抖动（面板尺寸固定，预览切换不改变宽高）。
- [ ] 无每次按键的堆分配爆炸：复用 `Vec`、避免 `format!` 在热路径反复分配（可用 `String::with_capacity` / `Arc<str>`）。

### 11.3 视觉

- [ ] 与高保真稿逐项对齐：920×568、行高 48、头部 52、分类条 38、脚注 34。
- [ ] 选中态左侧 2px accent 竖条 + 16% accent 底；hover 仅背景变化；无位移/缩放。
- [ ] 分组头、徽标、快捷键键帽、滚动条、空态、无结果态全部实现。
- [ ] 1440×900 与 1280×720 下不溢出；小屏可滚动。

### 11.4 无障碍

- [ ] 全键盘可完成打开→搜索→切换分类→执行→关闭。
- [ ] 焦点环可见；Esc 后焦点回到原控件。
- [ ] 对比度达标；不只靠颜色传达"已打开/不可用"。
- [ ] 减少动效模式下无缩放/位移动画。

### 11.5 i18n

- [ ] 中文/英文文案均无截断；长英文动作名有 ellipsis。
- [ ] 搜索关键词支持中英文别名与拼音（全拼/首字母）。

---

## 12. 风险与待验证

| # | 风险 | 影响 | 缓解 |
|---|------|------|------|
| R1 | Slint 在 `TextInput`（侧栏搜索框/命令栏）内可能消费裸 Shift，窗口级收不到 | 双击 Shift 在部分焦点位置失效 | POC 先验证；失效则在对应 TextInput 加 `shift-tap()` 回调（v1 覆盖两处） |
| R2 | 百度拼音 C0 标记与 Backspace 的时序在真机上可能不止 120ms | IME 用户误触/延迟 | 用 `[KEY_DIAG]` 日志采集 20 组真实序列，按 P95 调整 `IME_SETTLE`；保留 `Ctrl+Shift+Space` 兜底 |
| R3 | 固定 360ms 窗口对部分手速偏慢的用户可能偏短 | 双击未触发，误以为功能失效 | 保留 `Ctrl+Shift+Space` 等价入口；设置中提供"触发灵敏度"（v2） |
| R4 | 设置项 anchor 需要改动 `InterfacePanel` 的滚动逻辑 | 落地成本 + 回归风险 | v1 只做"打开到页面"，anchor 定位作为增强；不改现有设置逻辑 |
| R5 | 动作执行涉及大量 `AppWindow` 回调，宿主桥可能膨胀 | `src/app/search_ui.rs` 变大 | 动作表只存 `action_id`，执行分派到既有函数；search_ui 只做 match，不复制业务逻辑 |
| R6 | 高保真稿的 6000 条压力测试与真实 Rust 索引性能有差异 | 验收数据不能直接当实现性能 | 实现后用 `cargo test --release` 基准单独验收；高保真稿数据只作交互演示 |

---

## 13. 附录：Token 映射

| 语义 | 暗色 | 亮色 |
|------|------|------|
| 页面底 | `#0B0C0E` | `#F2F0EC` |
| 应用舞台 | `#0E1013` | `#E9E6DF` |
| 面板 | `#181A1D` | `#FAF8F4` |
| 浮层/卡片 | `#1E2024` | `#FFFEFA` |
| 悬停 | `#22262D` | `#E8E5E1` |
| 选中底 | `rgba(31,200,180,.16)` | `rgba(31,118,107,.12)` |
| 边框 | `#3F444F` | `#D0CECB` |
| 主文字 | `#BCBEC4` | `#33302C` |
| 副文字 | `#7A7E85` | `#7B766F` |
| 弱文字 | `#6F737A` | `#968F85` |
| 强调 | `#1FC8B4` | `#1F766B` |
| 成功 | `#73B00A` | `#427213` |
| 警告 | `#E8A33E` | `#8F631C` |
| 危险 | `#F75464` | `#B0463F` |

> 亮色值取自 `src/theme/palettes.rs:108-119` 的 MTerm · 亮；暗色值取自 mterm-dark 派生规则，与 `ui/theme.slint` 对齐。

---

## 14. 交付声明

本文件是**设计规格**，不是实现记录。所有 `src/*`、`ui/*.slint` 均未因本设计修改。高保真稿 `docs/design/double-shift-hifi-2026-11-06-v1.html` 只验证视觉与交互原型，不代表应用代码已实现。
