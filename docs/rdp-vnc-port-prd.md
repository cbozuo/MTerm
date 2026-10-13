# PRD：RDP / VNC 内置远程桌面移植（rustterm → MTerm）

| 项 | 内容 |
|---|---|
| 项目名 | `rdp_vnc_embedded` |
| 语言 | Rust 2021 + Slint 1.18.1 + tokio |
| 需求来源 | 用户需求 + `luckbloke/rustterm` 的 `ironrdp` / `vnc-rs` 能力移植 |
| 性质 | **移植任务**——把 rustterm 已有能力搬进 MTerm 并适配其架构，不从零发明协议 |
| 文档版本 | v1.0（初稿） |

---

## 1. 原始需求复述

把 rustterm 的 **RDP** 与 **VNC** 功能移入 MTerm，且必须：

1. 两者都跑在 **Tab 里**（不是外部窗口）；
2. RDP 支持**双连接方式**：现有 `mstsc` / FreeRDP 外部客户端方式**保留**，新增内置 RDP 客户端方式（Tab 内），用户可在两种方式间切换；
3. 尊重 MTerm 既有的目录结构（`struct/` + `impls/`）、Slint UI、i18n 与会话模型。

---

## 2. 产品目标

1. **统一承载**：RDP 与 VNC 会话和 SSH 会话一样活在 Tab 里，可拖拽、拆分、跨窗口移动、并排查看，不打断终端工作流。
2. **双模式自由切换**：同一个 RDP 会话定义，可在「内置客户端」与「系统客户端」之间切换；存量会话行为**一字不变**。
3. **零回归**：`mstsc` 现有路径、已有 `sessions.json`、已有 RDP 弹窗布局与文案，全部保持原样可用。
4. **从 rustterm 复用成熟实现**：协议栈、剪贴板、输入、证书校验等能力以移植方式落地，而非自研。
5. **可预期的连接生命周期**：连接中 / 已连接 / 重连中 / 断线 / 失败 五种状态在 Tab 上明确可见，首次连接的自签证书有明确的信任决策入口。

---

## 3. 用户故事

> 格式：As a [角色], I want [功能] so that [收益]

### 3.1 新建 RDP 会话

1. **US-R1** — As a *系统管理员*, I want 新建 RDP 会话时选择「内置客户端」方式 so that 桌面直接开在 MTerm 的标签页里，不必跳出应用切到另一个窗口。
2. **US-R2** — As a *系统管理员*, I want 新建 RDP 会话时选择「系统客户端（mstsc / FreeRDP）」方式 so that 沿用我已验证过的外部客户端路径，尤其是需要它本机凭据策略和多显示器能力时。
3. **US-R3** — As a *多环境用户*, I want 在同一个会话上随时切换连接方式（改设置后重连）so that 一个会话定义覆盖"临时排查用内置"和"长期运维用 mstsc"两种场景。

### 3.2 新建 VNC 会话

4. **US-V1** — As a *运维*, I want 在连接协议下拉里能选 VNC 并填写主机/端口/密码 so that 就能连上没有 Windows 域体系的老设备（ESXi、Mac VNC、树莓派等）。
5. **US-V2** — As a *运维*, I want VNC 会话连接后能锁定为只读 so that 我在别人机器上看日志时不会误操作。

### 3.3 在 Tab 内操作远程桌面

6. **US-T1** — As a *重度用户*, I want 在远程桌面 Tab 里直接敲键盘、点鼠标、拖窗口、滚轮滚动 so that 日常操作和在 mstsc 里一样自然。
7. **US-T2** — As a *重度用户*, I want 调整 Tab 大小时远程桌面分辨率自动适配 so that 分屏或并排查看时画面不被拉伸变形。
8. **US-T3** — As a *重度用户*, I want 远程桌面 Tab 和终端 Tab 能并排放在同一窗口 so that 边看桌面边敲命令。

### 3.4 断线重连

9. **US-C1** — As a *运维*, I want 网络抖动导致连接断开时，Tab 上明确显示「已断线 / 重连中 / 已重连」 so that 我知道是掉线了还是卡住了，而不是对着黑屏猜。
10. **US-C2** — As a *运维*, I want 能手动点「重新连接」 so that 服务端重启后我能立刻恢复，不用删了重建会话。

> **实现定稿（#remote-reconnect 2026-10-13）**：重连**不是发给驱动的消息**。按钮只在 `state >= 3`（重连中 / 已断线 / 失败）时出现，而那一刻驱动早已结束——它丢掉 client future、关掉 socket 并把 `closed` 置真，句柄上的任何发送都是空操作（此前那条 `reconnect` 通道的接收端在创建处就被丢弃，按钮点了等于没点）。因此重连 = **在同一个 Tab 里重新起一次会话**（`app::respawn_remote_tab`）：复用 tab-id 与它的 pane 行，保留标题/分组/只读标记，清掉上一段的失败原因与残留画面，凭据仍走已保存密码与 Windows 凭据管理器，证书信任按指纹复用、不会重问。「已断线」同时补上原因（驱动侧的 `GracefulDisconnectReason` 翻成人话；归类不出来的不硬凑），见 P0-10。

### 3.5 切 Tab 时会话保持

11. **US-K1** — As a *多任务用户*, I want 在远程桌面 Tab、终端 Tab、侧栏之间来回切换时远程会话保持在线 so that 我不会因为看了一眼日志就丢掉桌面会话。
12. **US-K2** — As a *多任务用户*, I want 把远程桌面 Tab 拖到另一个窗口后连接不断 so that 大屏 + 小屏可以分开放。

### 3.6 多显示器 / 分辨率

13. **US-D1** — As a *多屏用户*, I want 选择让远程桌面跟随标签页尺寸，或固定一个预设/自定义分辨率 so that 我既能精确控制远端分辨率，也能贴合本地布局。
14. **US-D2** — As a *多屏用户*, I want 远端多显示器（含第二块屏向左延伸）能整体映射进一个 Tab so that 不需要为每块屏各开一个会话。

### 3.7 剪贴板

15. **US-B1** — As a *运维*, I want 本机和远端之间双向同步文本剪贴板 so that 从远端文档复制 URL 到本地浏览器粘贴，反之亦然。

### 3.8 证书信任

16. **US-E1** — As a *安全意识强的运维*, I want 首次连接自签证书时看到指纹、颁发主体和「信任 / 不信任 / 仅本次」三个选择 so that 我能明知情地决定是否建立信任，而不是被静默接受或直接失败。

---

## 4. 需求池

### 4.1 P0 —— 必须有（缺了功能不成立）

| # | ID | 需求 | 验收要点 |
|---|---|---|---|
| P0-1 | REQ-SESSION-TYPE | **会话类型扩展**：`SessionKind` 增加 `Vnc` 变体，并新增 RDP 连接方式子字段（详见 §6.2）。 | `as_str()`/`from_str()` 双向映射齐全；旧配置反序列化零变化 |
| P0-2 | REQ-VNC-IN-DIALOG | **VNC 回归协议下拉**：`ui/session_dialog.slint` 里 VNC 从「预览态」转为「已实装」，加入连接协议下拉；清除 `start-preview("vnc")` 特殊路径 | `is-real` 白名单加入 `vnc`；`default-port-of("vnc") == 5900`；`proto-label` / `proto-sub` 补齐；预览横幅不再出现 |
| P0-3 | REQ-RDP-MODE-SWITCH | **RDP 双模式切换**：新建/编辑会话时可选「内置客户端」或「系统客户端」；见 §5 UI 说明 | 切换控件复用 `seg.slint` 的 `SegOption`；默认值为「系统客户端」 |
| P0-4 | REQ-ZERO-REGRESSION-MSTSC | **mstsc 路径零回归**：`rdp_mode` 缺省或为系统模式时，行为与本次改动前**完全一致**（含字段布局、文案、`rdp::launch` 签名） | 存量 `sessions.json` 无需迁移；老 RDP 会话仍走 mstsc；`crate::rdp::launch` 签名不变 |
| P0-5 | REQ-TAB-HOST | **Tab 承载**：`TabInfo.kind` 增加第三种取值承载远程桌面；Tab 参与既有生命周期（新建/关闭/拖拽跨窗口/拆分/合并/重命名） | 复用 `app.slint` 中 `for term[i] in root.terminals : TerminalView` 的可见性调度范式，加一组桌面视图；`tabs.slint` 主题分支同步 |
| P0-6 | REQ-RENDER | **画面渲染**：把远端解码后的帧渲染到 Tab 内容区，保持正确宽高比，不拉伸、不留黑边、不闪烁 | 窗口缩放与 Tab 切换不触发画面丢失或错位 |
| P0-7 | REQ-INPUT-KBD | **键盘输入**：Tab 获得焦点时，远端完整收到按键（含修饰键、组合键、非 ASCII 字符、死键） | 与 `TerminalView` 的按键分发互斥，不双投递 |
| P0-8 | REQ-INPUT-MOUSE | **鼠标输入**：移动、按下/释放、拖拽、滚轮、双击、右键菜单 | 坐标需按画面缩放比例换算，不能用 Tab 本地像素 |
| P0-9 | REQ-RESOLUTION | **分辨率自适应**：`跟随标签页` 模式下，Tab 尺寸变化时通知服务端重设桌面尺寸；服务端不支持时按比例 letterbox 缩放 | 缩放有上限/下限保护，避免 0 尺寸请求；切换模式不崩。**「跟随标签页」必须真正落库（#rdp-follow-lost 2026-10-13 定稿）**：该选项在对话框里选了之后从未生效过——分辨率下拉只写 `draft.rdp_resolution`，而保存路径读的是界面上没有控件能写的 `draft.rdp_follow_tab_size`（恒为 `false`），选择被静默丢弃；`follows_tab_size` 判据统一后，保存路径、构造路径、下拉三处同步（下拉同步写 bool 也保证了「改回固定分辨率」能生效，不再单向卡死）。**新建内置 RDP 会话默认即「跟随标签页」**（#rdp-follow-default 2026-10-13）：切到内置模式且分辨率仍是出厂值时自动预置，编辑存量会话不动已存值，切回系统客户端自动还原。**另（#vnc-refused-is-final）**：VNC 连接被 `ECONNREFUSED` 拒（端口上没有服务在听）时立刻判失败，不再按传输错误空转 3 次退避（1+2+4s）——那 7 秒里界面停在「重连中…」而看不到原因 |
| P0-10 | REQ-CONN-STATE | **连接状态机**：连接中 / 已连接 / 重连中 / 已断线 / 连接失败 五态在 Tab 标题、状态栏、侧栏三处一致呈现；失败需给出人可读原因 | 状态与既有 `TabStatus` 体系衔接；断线时 Tab 不被销毁。**标签栏上的连接状态点已删除（2026-10-13）**：原先 `tabs.slint` 在 rdp/vnc 标签左侧画一个三色小圆点（绿/黄/红），与内容区整页状态是同一件事的第二种说法，且颜色口径与内容区并不总是一致；远程桌面的连接状态改由 Tab 内容区独有地表达（整页转圈 / 「重连中…」/ 「已断线 — <原因>」/ 「连接失败」+ 可展开详情），`TabInfo.connected`/`state` 字段保留（侧栏在线状态点的汇总口径未动）。**失败展示（#failure-wrap 2026-10-13 定稿）**：主标题是**归类后的一句话**（`remote::failure`，按 NTSTATUS 码 / socket 错误码 / 证书与协商关键词归类，如「用户名或密码不正确」「目标主机拒绝了连接」）；驱动返回的原文一字不改地收进「显示错误详情」折叠里（排查与反馈仍需要它），不再充当标题 —— 此前同一段原文还会在标题与详情两处各显示一遍。**断线也带原因（#remote-reconnect 2026-10-13）**：`已断线` 一直是真话（服务端挂断 / 会话被接管 / 传输中断都落到这一态），但驱动手里的 `GracefulDisconnectReason` 曾被丢掉；现在翻成一句话贴在状态后（「已断线 — 服务器断开了连接」），归类不出来的**不硬凑**（不能变成「已断线 — 连接失败」），原文照旧只在日志与 `Failed` 的详情折叠里。**状态覆盖层版式（#status-center 2026-10-13）**：转圈与「重新连接」按钮都必须水平居中——`VerticalLayout{alignment:center}` 只保证垂直居中，固定尺寸的子元素当年因此贴着 Tab 左边缘 |
| P0-11 | REQ-CRED-STORE | **凭据存储**：复用现有 `Secret`（ChaCha20-Poly1305）机制存储 RDP/VNC 密码；密码可留空（内置模式下留空=连上后由远端认证流程处理） | **留空 = 写入空密码**（2026-10-13 定稿）。原「编辑时留空=保留原密码」已废止：它是 `#10`「密码不回填」的补偿机制，用户定稿改为「以密码框当前值为准」，于是清空密码框即回到无密码。**追加（`#credential-store` 2026-10-13）**：内置 RDP 在密码框留空时会先查 Windows 凭据管理器的 `TERMSRV/<主机>`（与 `mstsc` 取的是同一条）——读到即走 NLA 直达桌面，读不到/不可读/为空才落到远端登录界面；只读 `普通`（generic）类型，密码不回显、不回写 |
| P0-12 | REQ-CERT-TRUST | **证书信任流程**：首次连接展示颁发主体/有效期/SHA-256 指纹，提供「信任并保存 / 拒绝 / 仅本次信任」；指纹变化时再次告警 | 决策持久化；已有信任记录重复连接不再打扰；指纹不匹配必须阻断而非静默降级 |

### 4.2 P1 —— 应该有

| # | ID | 需求 |
|---|---|---|
| P1-1 | REQ-CLIP-TEXT | 文本剪贴板**双向**同步（远端→本地、本地→远端），带开关与大小上限 |
| P1-2 | REQ-FULLSCREEN | Tab 内全屏切换（隐藏 MTerm 界面装饰，占满目标显示器），并有明确退出路径 |
| P1-3 | REQ-SPECIAL-KEYS | 特殊键：Ctrl+Alt+End、Ctrl+Alt+Del、Alt+Tab、PrintScreen、PrtScn、Win 键、Alt+F4；提供"发送组合键"工具条 |
| P1-4 | REQ-MULTI-MON | 远端多显示器：枚举全部屏幕、整体映射进 Tab、允许切换主屏 |
| P1-5 | REQ-AUDIO | 音频播放（RDP 音频重定向 / VNC 音频），带音量与静音控制 |
| P1-6 | REQ-CLIP-FILE | 剪贴板文件传输（复制文件即在两机间传输），带进度与大小上限 |
| P1-7 | REQ-VNC-VIEWONLY | VNC 只读模式开关（会话级+连接时） |
| P1-8 | REQ-REMOTE-TOOLBAR | 远程桌面 Tab 工具条：全屏、重连、发送 Ctrl+Alt+End、缩放（适应窗口/原始比例/100%）、画质/编码选择 |
| P1-9 | REQ-TEST-CONN | 内置模式与 VNC 的「测试连接」按钮（拨号+握手探测，不进入桌面）；RDP 系统模式维持现状不显示 |
| P1-10 | REQ-RECONNECT-POLICY | 可配置自动重连（开关、退避次数与间隔）；重连成功后恢复画面 |

### 4.3 P2 —— 可以有

| # | ID | 需求 |
|---|---|---|
| P2-1 | REQ-PRINT | 远程打印（VNC 打印机映射 / RDP 打印机重定向） |
| P2-2 | REQ-USB | USB / 智能卡重定向 |
| P2-3 | REQ-CAMERA | 摄像头/麦克风重定向 |
| P2-4 | REQ-PERF-TUNE | 性能参数：RDP 侧（位图缓存、编码顺序、QoS），VNC 侧（像素格式、压缩级别），仅配置文件暴露，不做 UI |
| P2-5 | REQ-TUNNEL | 复用既有 SSH 跳板/端口转发能力做 RDP/VNC 隧道；RDP Gateway 支持 |
| P2-6 | REQ-SCREENSHOT | 远程画面截图导出 |
| P2-7 | REQ-SESSION-ROAM | 多实例/多标签连接同一主机时的会话漫游与互斥策略 |

---

## 5. UI 设计稿说明（新建/编辑会话对话框）

> 现状：`ui/session_dialog.slint` 顶部为「连接协议 / Connection protocol」下拉（SelectField）；选中 RDP 时，下方依次渲染 **分隔线 → 用户名+密码（双列）→ 域+分辨率（双列，自定义时展开宽×高）→ 提示文案**。VNC 曾在下拉里但走「预览态」，现已被移除。

### 5.1 RDP：新增「连接方式」切换控件

> **实现范围要求**：该控件在**新建与编辑**两种模式下都必须渲染，位置一致（这是 §7.3 Q14 定论 R1 的落地形式）。编辑存量 mstsc 会话时，切换此控件**不得清空任何其他字段**（R2），切换过程无二次确认、无提示弹窗（R3）。

**位置**：紧跟在「连接协议」下拉**正下方**、主机/端口字段**上方**，作为独立的带标签一行。

**理由**：
- 「协议 → 方式」是同层级的两级选择器（先选什么协议，再选用哪种客户端），相邻符合阅读顺序；
- 它决定了下方表单的语义与密码填写策略（系统模式密码可留空交 mstsc 处理，内置模式需要更明确），因此应在地址与凭据**之前**让用户先定；
- 紧贴下拉可避免用户滚到表单底部才发现还有一半表单的字段会变。

**控件形态**：复用 `ui/seg.slint` 的 `SegOption` 分段控件（与"端口转发 -L/-R/-D"那行同款），不用新的下拉。

**文案（中/英双语）**：

| 元素 | 中文 | English |
|---|---|---|
| 字段标签 | 连接方式 | Connection mode |
| 选项 1 | 内置客户端（标签页） | Built-in client (in tab) |
| 选项 2 | 系统客户端（外部窗口） | System client (external window) |
| 提示（内置模式） | 远程桌面将直接显示在此标签页内。 | The remote desktop opens directly in this tab. |
| 提示（系统模式） | 交给系统远程桌面客户端，在独立窗口中打开（当前为 mstsc / FreeRDP）。 | Handed to the system remote desktop client, opened in its own window (mstsc / FreeRDP). |

**字段显隐矩阵**（关键：系统模式一列必须与现状像素级一致）：

| 字段 | 系统客户端（现状） | 内置客户端（新增） |
|---|---|---|
| 用户名 | 显示（占位符 `DOMAIN\user or user@domain`） | 显示（同） |
| 密码 | 显示（占位符「留空 — mstsc prompts if needed」） | 显示（占位符改为「留空 — 认证时提示 / blank — prompt on connect」） |
| 登录域 | 显示 | 显示 |
| 分辨率 | 显示（全屏 / 预设 / 自定义） | 显示，但**首选项语义扩展为 `跟随标签页 / Full screen / 预设 / 自定义`** |
| 画质 / 编码 | 不显示 | 显示（编码：自动 / 高压缩 / 低延迟） |
| 剪贴板同步 | 不显示 | 显示（复选框，默认开） |
| 测试连接 | 不显示（现状无端点可测） | 显示 |

**零回归约束**：系统客户端一列的字段顺序、文案、占位符、提示文字必须与当前 `session_dialog.slint` 中 `#rdp-seg` / `#rdp-no-adv` 段落逐字一致——这样该分支在视觉上"看不出被改过"。

### 5.2 VNC：选中协议后显示的字段

顺序建议（沿用 RDP 的双列节奏）：

| 顺序 | 字段 | 标签（中/英） | 说明 |
|---|---|---|---|
| 1 | 主机 | 主机 / Host | 占位符 `192.168.1.30` |
| 2 | 端口 | 端口 / Port | 默认 **5900** |
| 3 | 密码 | 密码（可留空） / Password (leave blank ok) | 留空 = 空密码（编辑态同语义，见 P0-11） |
| 4 | 分辨率 | 分辨率 / Resolution | `跟随标签页` / `全屏` / 预设 / 自定义（同 RDP 控件） |
| 5 | 只读 | 只读（不发送键鼠输入） / View only (do not send keyboard or mouse input) | P1，默认关 |
| 6 | 编码 | 画质 / Encoding | P1/P2，默认「自动」 |

**明确不显示**：「登录域 / Domain」——VNC 经典协议无 Windows 域概念，暴露该字段只会误导。

**端口与认证的 P0 校验**：主机为空 → 现有 `host-missing` 机制直接复用（`draft-kind != "serial"` 即要求 host，VNC 自动覆盖）。

### 5.3 其他 UI 改动点（非对话框）

- **Tab 卡片**：`tabs.slint` 已在 `info.kind` 上分支（`"welcome"`/`"terminal"`）控制底色；新增第三值需同步；标签页标题旁显示连接状态小圆点（连接中=琥珀、已连接=绿、断线=红、重连中=闪琥珀）。
- **协议图标**：`theme.slint:309` 已把 `"vnc"` 映射到 `remote-desktop` 图标，无需新增图标字形。
- **会话列表/侧栏**：RDP 会话可显示方式徽标（如 `内置` / `mstsc`），帮助用户区分同名会话。
- **文案统一走 i18n**：新增文案同步进 `lang/zh/LC_MESSAGES/mterm.po` 与 `lang/en/LC_MESSAGES/mterm.po`（当前各约 355 条 msgid）。

---

## 6. 与现有功能的关系

### 6.1 零回归清单（必须逐条验证）

| # | 现状 | 约束 |
|---|---|---|
| 1 | `crate::rdp::launch(&Session) -> Result<String,String>` | 签名**不变**；内置模式走**新入口**，不改这个函数 |
| 2 | `app.rs:7879` `if session.kind == SessionKind::Rdp { ...; return; }`（不建 Tab，早返回） | 改为**先判连接方式**：系统模式保持原早返回；内置模式才落 Tab |
| 3 | `session_runtime.rs:89` `SessionKind::Rdp => { warn!("cannot be hosted in a tab"); return; }` | 语义**收窄**为"RDP 系统客户端模式不能托管在 Tab 内"，内置模式走新增分支 |
| 4 | `rdp/mod.rs` 模块文档 "MTTerm never implements the protocol itself" | 文档需更新（描述现状而非否定），但代码结构与 `Account`/`RDP_DEFAULT_PORT`/`MIN_DESKTOP_SIDE` 常量保持 |
| 5 | `session_dialog.slint` `#rdp-seg` / `#rdp-no-adv` 段 | 系统模式下逐字不变（见 §5.1） |
| 6 | `rdp_domain` / `rdp_fullscreen` / `rdp_width` / `rdp_height` 字段名与语义 | 全部保留；内置模式复用其语义（`fullscreen` 语义在内置模式下扩展为"跟随 Tab"，建议新增字段而非改写原字段语义） |
| 7 | `SessionKind::Rdp` 在 `app.rs` 侧栏标签、`session_models.rs:887` 默认端口等处的分支 | 保持穷尽且不改变已有分支行为 |
| 8 | 已有 `sessions.json` | 无需迁移工具、无需改写文件 |

### 6.2 `SessionKind` 怎么扩展 —— 产品层建议

**建议：VNC 加新变体；RDP 的两种连接方式加子字段。两者不要混用同一机制。**

```rust
// (a) 新增变体 —— VNC 是另一个协议
pub enum SessionKind { Ssh, Serial, Telnet, Local, Rdp, Vnc }

// (b) 新增子字段 —— RDP 的两种方式是同一协议的两种客户端实现
pub enum RdpClientMode { SystemClient, Embedded }   // 默认 SystemClient

// Session 上新增（均需 #[serde(default)] 以保证旧配置零变化）
pub rdp_mode: RdpClientMode,        // default = SystemClient
pub rdp_follow_tab_size: bool,     // 内置模式：分辨率跟随 Tab
pub rdp_embedded_fullscreen: bool,  // 内置模式：隐藏 MTerm 装饰占满显示器
pub vnc_view_only: bool,            // default = false
pub vnc_security: String,           // 空 = 交给客户端协商；取值待实现期确定（见 §7.2）
pub vnc_clipboard: bool,            // default = true
```

**唯一权威判据 —— `Session::uses_tab()`**：判断"这个会话要不要建 Tab"**必须**调它，不能用 kind 级方法：

```rust
impl Session {
    pub fn uses_tab(&self) -> bool {
        match self.kind {
            SessionKind::Vnc => true,
            SessionKind::Rdp => match self.rdp_mode {
                RdpClientMode::Embedded => true,
                RdpClientMode::SystemClient => false,
            },
            _ => false,
        }
    }
}
```

两个 `RdpClientMode` 臂都**显式写出**（而非 `!= SystemClient`），这样将来加第三种模式时编译器会强制在这里分类，不会静默落进 Tab 分支。误判的后果是"**连上了但不建 Tab**"——用户看到连接成功却什么也没有，属最难排查的一类，故接线时务必用这一个入口。

> ⚠️ **实现提示**：`SessionKind` 上另有一个 `is_tab_transport()`，它只能回答"这个协议**有没有** Tab 托管形态"（`Vnc` 为真），**读不到 `rdp_mode`**，无法区分 `Rdp + Embedded`。任何"要不要建 Tab / 选哪个渲染器"的判断都**不可**用它。

**理由 —— 为什么 VNC 用变体**：
1. VNC 是**不同协议**：无 Windows 登录域、认证模型不同（VNC 密码挑战 vs RDP NTLM/Kerberos）、字段集不同（画质/只读/编码 vs 域/NLA）。塞进 RDP 的字段组会造成"有一半字段永远无意义"的坏味道。
2. 现有代码已经把每个协议的所有权写进了穷尽 `match`（`session_runtime.rs:60-96`、`app.rs:7890-7900`、`session_models.rs:887`）。VNC 作为一等变体，能自然获得独立的 spawn 函数与状态标签，语义清晰。
3. 列表、图标、默认端口、欢迎页文案都能按协议维度正确区分。

**理由 —— 为什么 RDP 方式用子字段而不是变体**：
1. **零回归是硬约束**。若新增 `SessionKind::RdpEmbedded`，则所有 `match session.kind` 的地方都要加一支、`from_str` 要认识新字符串、侧栏/图标/导出/分享都要区分"两个都叫 RDP 的东西"——任何一处漏改就是行为回归。改成字段后，新增分支只出现在"**这个会话怎么被启动**"这一个决策点，受影响面小一个数量级。
2. **两者共享同一份配置语义**。主机/端口/用户名/密码/域/分辨率对两种方式完全一样，用户切方式时不该丢字段、不该变成两个条目。变体方案会让"切换方式"退化成"删了建一个"。
3. **用户的心智是"同一个 RDP 连接"的两种打开方式**，而不是"两种不同的协议"。UI 上就是一个二选一分段控件，不是两个下拉项。
4. `#[serde(default)]` 天然给出向后兼容：字段缺失 → `SystemClient` → 老会话全部保持今天的 mstsc 行为，不需要迁移脚本。

**代价与规避**：字段方案会让"按协议过滤"的代码（比如侧栏筛选）仍把两种 RDP 归为一类——这恰好是期望行为。若将来出现第三种 RDP 客户端，届时再评估是否值得升格为变体。

**命名建议**：`SessionKind::Vnc`（而非 `Vnc`/`Rfb` 混用）以匹配既有的 `Ssh/Serial/Telnet/Local/Rdp` 全小写风格；`from_str("vnc")` 需新增，且**旧代码中 `"vnc"` 未被识别时落到 `_ => Ssh`**，因此该新增不会让任何旧配置被误读。

---

## 7. 待确认问题（会影响架构）

### 7.1 阻塞级（不确认可能返工）

> 状态：**3 条待确认**（原 5 条：Q5 依赖兼容性已解除见 §7.1.1；Q1 键盘仲裁已解决见 §7.1.2）。

| # | 问题 | 为什么影响架构 |
|---|---|---|
| ~~Q1~~ | ~~Tab 内键盘/焦点仲裁规则：远程桌面 Tab 与终端 Tab 的按键分发如何互斥？`Alt+Tab`/`Alt+F4`/`Win` 键是否被 MTerm 窗口系统截走？~~ | **✅ 已解决（实测验证）** —— 见 §7.1.2 |
| Q2 | **`TabInfo.kind` 增加取值的影响面**：主题底色、侧栏状态、Tab 右键菜单、拆分/合并、跨窗口拖拽、Zen 模式、恢复覆盖层（`app.slint:3168`）逐处都要确认 | 这是 `kind` 目前只有两值的隐含约束扩散点，漏改会出现"桌面 Tab 底色错"或"恢复时丢 Tab" |
| Q3 | **内置 RDP 的分辨率协商能力边界**：目标 rustterm/ironrdp 组合是否支持 mid-session 动态 resize？不支持时是 letterbox 还是服务端保持固定尺寸？ | 决定 P0-9 是"真自适应"还是"缩放显示"，也决定要不要引入"缩放比例"这一 UI 概念 |
| Q4 | **证书信任的存储位置与粒度**：指纹存 `sessions.json` 还是独立信任库？是否按 (host,port) 还是全局按指纹？是否需要"仅本次"这种非持久选项？ | 决定信任模块的数据模型和 `.po`/配置文件结构，属于不可逆的格式决策 |
| ~~Q5~~ | ~~rustterm 的 `ironrdp` / `vnc-rs` 依赖版本与 Rust/Slint 兼容性~~ | **✅ 已解除** —— 实测版本对齐、无需打补丁，详见 §7.1.1 |

### 7.1.2 ✅ Q1 已解决 —— 键盘/焦点仲裁（实测验证）

**原问题**：远程桌面 Tab 与终端 Tab 的按键分发如何互斥？`Alt+Tab`/`Alt+F4`/`Win` 键是否被 MTerm 窗口系统截走？是否需要原生全局热键钩子（Wayland 权限）？

**结论：不需要原生全局钩子。Slint 的 `FocusScope` + Tab 互斥显示已足以保证正确性**，实现已落地并通过验证。

**互斥机制（P0-7 验收点）**：

| 机制 | 作用 |
|---|---|
| Tab 互斥可见 | `visible: term.id == pane.active-id`，非活动 Tab 不渲染、不接收事件 |
| `active` 变化时抢/放焦点 | `changed active => { if (self.active) { root.grab-focus(); } else { root.drop-focus(); } }` |
| **离开 Tab 必须释放按键** | `drop-focus()` 里调 `release-all-keys()` —— 否则"按住 Shift 切 Tab"会让 Shift **卡在服务器上**。这是最容易漏的一环 |
| 信任弹窗打开时拒收 | `grab-focus()` 内 `if (root.active && !root.trust-open)`，避免弹窗期间按键漏进远端 |

**两个关键的 IME 结论**（与设计时的初判有出入，以实测为准）：

1. **必须同时用 `FocusScope` + 隐藏 `TextInput`**。架构师设计文档 §3.4 曾判断"RDP 需要真实 scancode，`ime-input` 的 TextInput 路径不适用"，但**实现证明两者都需要**：
   - **裸 `FocusScope`** 接 `key-pressed`/`key-released` 拿 **scancode**（快捷键、Ctrl+Alt+Del 等组合）
   - **隐藏 `TextInput`** 接 **IME 合成与粘贴文本**（`edited` 回调）—— 这是**非拉丁输入唯一能到达远端的通道**
   - 两者缺一：只留 FocusScope 则中文/日文/emoji 打不进远端；只留 TextInput 则拿不到 scancode
2. **`key-pressed` 必须 `accept` 每一个键**（`:230-236`）。若返回 `reject`，Slint 会把字符插入 `text` 而非转发给服务器 —— **非 ASCII 输入将全部丢失**。

**macOS 修饰键语义反转**：Slint 在 macOS 上会交换 ctrl/meta，因此代码里用 `Theme.is-mac ? event.modifiers.meta : event.modifiers.control` 取物理 Ctrl。这一条必须保留，否则 macOS 上 Ctrl 组合键全错。

**仍未覆盖、留待 P1 的部分**：`Alt+Tab`/`Alt+F4`/`Win` 等**系统级组合键**由 OS 截走，Slint 收不到。这属于 OS 行为而非本项目缺陷；P1-3 的"发送特殊键"应走**工具条按钮显式注入**（而非拦截系统快捷键），这条路径不依赖全局钩子，因此 Q1 原本担心的 Wayland 权限问题**不会出现**。

### 7.1.1 ✅ Q5 已解除 —— 依赖版本兼容性（实测结论）

**原问题**：rustterm vendored 的 `ironrdp` / `vnc-rs` 能否在本项目 Rust 工具链与依赖版本下编译？是否需要再打补丁？

**结论：解除阻塞。版本基本对齐，无需打补丁。** 主理人在本机构建机上以 `cargo new` + `cargo add` 实测：

| crate | 实测可拉取版本 | rustterm vendor 版本 | 结论 |
|---|---|---|---|
| `ironrdp` | 0.17.0 | **0.17.0** | **版本完全一致**（但注意：**代码仍需适配**，见下方风险条目） |
| `vnc-rs` | 0.6.0 | **0.5.3** | **采用 0.6.0** —— 架构师逐项比对确认 **0.6.0 是 0.5.3 的严格超集**（公开类型 15→20 个，0.5.3 的一个不少），且 `ClientKeyEvent`/`ClientMouseEvent` 逐字段相同、`VncConnector` 7 个 builder 签名全一致 → **升级零适配成本**，还能白拿 `DesktopLayout`/`DesktopUpdate`/`ScreenLayout`/`ResizeError`（即**动态分辨率**）。0.5.3 并不提供 0.6.0 做不到的任何能力 |

其他实测事实（对架构/工程有直接影响）：

- 项目走 **rsproxy-sparse 镜像**，crates.io 可通 —— **不需要 vendor 任何东西**（与 rustterm 的 vendor 策略不同）
- 工具链 `cargo 1.98.0` / `rustc 1.98.0`，满足项目 `rust-version = "1.92"`
- 目标平台 `x86_64-pc-windows-gnu`，`.cargo/config.toml` 已配好 dlltool / linker / windres
- `image` crate **MTerm 已有**（含 jpeg feature），rustterm 用的 `image::codecs::jpeg::JpegEncoder` 可直接复用
- tokio 现有 features 足够跑 `TcpStream` I/O 与 channel
- **不需要打补丁**

**已知风险（非阻塞，接受即可）**：新增 `ironrdp` 会拖入 rustls / ring 等传递依赖，**首次全量编译明显变慢、二进制体积增大**。本机 15.2GB 内存且 `jobs = 2` 防 OOM，编译耗时会偏长。这不改变任何产品需求或架构决策，但需在排期估算中体现。

> **体积增大的具体来源（实测）**：`ironrdp` 的 `rustls` feature 链到的是 **`aws-lc-rs` 而非 ring**（`ironrdp/rustls → ironrdp-client/rustls → ironrdp-tls/rustls`，其中 `ironrdp-tls` 的 rustls = `["rustls-aws-lc-rs"]`）。因此 `Cargo.lock` 里 **`aws-lc-rs` 与 `ring` 两者都在**：ring 是 MTerm 原有依赖（russh/sha1），aws-lc-rs 是本次新增。即本项目**确实引入了第二套 TLS 密码学后端**，功能无影响（已实测可编译），但会多一个 `aws-lc-rs` 静态库。**注意区分**：`tokio-rustls 0.26` / `rustls 0.23` 这两个直接依赖与已有 `ureq` 共享同一份 crate（ring provider），这部分没有重复编译。

#### ⚠️ 已知风险：rustterm 的参考代码**不能直接编译**（版本对齐 ≠ 代码可照搬）

架构师实测发现：**两个协议都有这个问题**，但成因不同 —— `ironrdp` 是 **API 漂移**，`vnc-rs` 是 **fork 私有类型缺失**。

**RDP 侧（`ironrdp` 0.17，API 漂移）**

| 断点 | 说明 |
|---|---|
| `RdpOutputEvent::DesktopUpdate` | 在 0.17 中**已删除**，rustterm 依赖它做增量帧更新 |
| `ServerCertificateVerifier` trait | 全仓 grep 不到，rustterm 的证书校验实现需换挂点 |

**✅ 好消息**：实际工作量比预想小。`ironrdp` 0.17 直接给出 `Vec<u32>` 原始像素，因此 rustterm 那约 200 行**增量帧合成 + JPEG 编码逻辑可以整段丢弃**，不需要移植。这直接抵消了上面的适配成本。

**VNC 侧（`vnc-rs`，fork 私有类型缺失）**

rustterm 用的是 **fork 版** `vnc-rs`，`vnc.rs:16-19` import 的三个符号在 **crates.io 的 0.5.3 与 0.6.0 里都不存在**：

| rustterm import 的符号 | crates.io 两版 |
|---|---|
| `VncLimits` | ❌ 全 `src/` 搜不到 |
| `VncSecurityPolicy` | ❌ 全 `src/` 搜不到 |
| `VncServerKey` | ❌ 全 `src/` 搜不到 |

这三个是 **rustterm 自己加的类型**，不是上游 API。因此 VNC 侧不存在"照搬"路径，需按当前 crate API 重写认证方式选择与传输层配置。

#### ⚠️ 帧模型不对称 —— VNC 侧工作量显著高于 RDP（影响排期）

**不要按"RDP 与 VNC 对称"估算工时**，两者帧模型根本不同：

| | RDP（ironrdp 0.17） | VNC（vnc-rs 0.6.0） |
|---|---|---|
| 帧事件 | `Image{buffer: Vec<u32>, w, h}` —— **每帧完整画面** | `RawImage(Rect,…)` / `Copy(dst,src)` / `JpegImage(Rect,…)` —— **增量脏矩形** |
| 需自己合成全屏缓冲 | ❌ 不需要 | ✅ **必须，约 80–120 行** |

rustterm 的 `vnc.rs` 自己做了这个合成器，**这部分可移植**（不依赖 Tauri，把 `Channel::send` 换成写 `Arc<Frame>`）。验收标准见设计文档 §3.2（含"空闲时 CPU 接近 0""Copy 分支要处理重叠区域"）。

**给工程师的提醒**：不要按"照搬参考实现"去找 rustterm 的 RDP 帧合成代码，那段不存在且不需要；VNC 的合成器则**确实需要移植**。逐条适配方案见 `docs/rdp-vnc-port-design.md` §1.4。

### 7.2 影响体验（可在 P1 迭代中确认）

> 状态：**9 条待确认**（原 10 条，Q14 已给出产品层定论，见 §7.3）。

| # | 问题 |
|---|---|
| Q6 | 断线是否默认自动重连？退避策略（次数/间隔/上限）？重连成功后画面能否恢复，还是需要重登录？ |
| Q7 | Tab 内"全屏"的语义：隐藏 MTerm 装饰占满当前显示器（需处理多显示器），还是另开一个独立原生窗口？后者会削弱"一体化"的初衷 |
| Q8 | VNC 安全类型（None / VNC Auth / VeNCrypt / TLS）的支持范围？无密码 VNC 与"留空=不提示"的产品预期是否一致？ ⚠️ **取值范围待实现期确定** —— 见下方说明 |
| Q9 | 剪贴板文件传输走协议原生重定向（RDP drive redirection / VNC 文件传输扩展），还是走 MTerm 自己的上传下载对话框？两者 UX 差异很大 |
| Q10 | 音频默认行为：自动播放还是需用户手势？多音频设备如何选？远端无音频时是否静默？ |
| Q11 | 内置 RDP 是否需要 NLA / CredSSP / Gateway / 智能卡（P0 仅要求基础 RDP；决定 P2-5 的范围） |
| Q12 | 多显示器（Q-P1-4）在 Tab 内是"整体缩放映射到单 Tab"还是"每块屏一个 Tab 内画面"？后者需要 sub-view 概念，复杂度明显更高 |
| Q13 | 会话定义导出/分享（如 sessions.json 导出、或未来云同步）中 `SessionKind` 与 `rdp_mode` 的向后兼容策略 |
| ~~Q14~~ | ~~现有 mstsc 会话是否要提供"升级为内置模式"的一键迁移入口？~~ | **✅ 已定论** —— 不做专门的迁移按钮，但必须保证编辑时字段无损切换，见 §7.3 |
| Q15 | 性能与资源预算：Tab 内 60fps 的 CPU 上限、帧缓冲内存上限、多会话同时在线的策略（是否限制并发桌面 Tab 数量） |

**⚠️ Q8 补充说明 —— `vnc_security` 取值范围待实现期确定**：`Session.vnc_security` 字段的语义依据是 rustterm 的 `VncSecurityPolicy`，但该类型属于 **rustterm 的 fork 私有类型**，crates.io 的 `vnc-rs` 0.5.3/0.6.0 **均不存在**（见 §7.1.1）。因此该下拉框**没有现成类型可映射**，由 T03 新增 `src/rdp/vnc/struct/security.rs` 承接。

**产品侧约定（不变）**：无论最终枚举如何裁剪，字段保持**字符串型、允许空值、空 = 交给客户端自行协商**，以免 crate 版本变化导致老配置无法反序列化。**具体可选值待 T03 实测 `vnc-rs` 实际协商能力后定稿**，届时如与下表不符，以实现为准并回头修订本节。

### 7.3 ✅ Q14 产品层定论 —— 不做专门的迁移按钮

**结论：不做"一键升级为内置模式"的迁移入口。** 用户切换连接方式的唯一途径就是**编辑该会话**（打开编辑对话框 → 改「连接方式」→ 保存）。

**理由**：

1. **迁移不是一次性的真实用户场景**。RDP 会话是要长期反复使用的配置，不是需要"升级"的历史包袱。真正需要一次性转换的老用户数量很小，且他们必然会知道怎么编辑会话——为此专门做一个按钮/菜单项/弹窗，是为一个低频场景增加永久 UI 面，得不偿失。
2. **字段本来就无损，迁移按钮没有实际价值**。切换方式只改一个枚举值，主机/端口/用户名/密码/域/分辨率完全共用（这正是 §6.2 采用子字段而非新变体的直接收益）。既然切换是零成本的，"迁移"这个动作在产品上根本不存在。
3. **会引入状态与视觉噪音**。一个只对存量 RDP 会话显示的迁移入口，需要额外定义"哪些会话算待迁移""迁移后按钮何时消失""迁移失败怎么办"，还要在列表/右键菜单/欢迎页各占位置。
4. **默认值已经给出了正确的渐进路径**。`rdp_mode` 默认为 `SystemClient`，存量会话不编辑就是 mstsc 行为。用户想要内置模式时，编辑对话框里那个二选一分段控件（§5.1）就在最显眼的位置——**发现成本已经足够低**，不需要额外的引导。

**但必须保证（这部分是硬要求，不是可选项）**：

| 要求 | 说明 |
|---|---|
| R1 编辑对话框里能看到并切换「连接方式」 | §5.1 的分段控件在**新建与编辑**两种模式下都要渲染，且位置一致 |
| R2 切换后主机/端口/用户名/密码/域/分辨率**全部原样保留** | 切换只改 `rdp_mode`，不得触碰其他任何字段。注意（2026-10-13 变更）：编辑对话框的密码框现已**不回填**已存密码，「留空即写入空密码」，因此**编辑任何字段（含只切连接方式）并保存都会清掉已存密码**——这是 `#blank-password-overwrites` 规则的结果，不是本项要处理的回归 |
| R3 不弹任何提示 | 切换是纯粹的枚举字段变更，不做二次确认、不做"迁移成功"通知 |
| R4 切换并保存后立即生效 | 下次连接该会话时按新方式启动；连接行为符合 P0-3 的双模式定义 |
| R5 会话列表可辨识当前方式 | 侧栏/列表对 RDP 会话显示方式标识（如 `内置` / `mstsc`），否则用户无法确认自己切了没有 |

**范围声明**：VNC 不涉及此问题（VNC 是新增协议，无存量会话需要转换）。

---

## 8. 附录：现状要点（供架构师参考）

- `src/rdp/` 现有职责仅为"把账号信息交给外部客户端"：`mod.rs`（入口 `launch`）+ `struct/account.rs`（`Account`/`RDP_DEFAULT_PORT=3389`/`MIN_DESKTOP_SIDE=200`）+ `impls/{mstsc,freerdp,freerdp_start}.rs`。
- `ui/session_dialog.slint` 中 VNC 走 `start-preview("vnc")` 分支（`:2431`、`:2602`），且在「协议下拉」里已被注释移除（`:536` 注释：*"VNC 实装时…"*）；`theme.slint:309` 已为 `vnc` 备好图标。
- `TabInfo.kind` 当前取值仅 `"welcome" | "terminal"`（`ui/tabs.slint:9`、`src/app.rs:7954`）。
- `app.slint` 中桌面视图可参照 `:2964` 的 `for term[i] in root.terminals : TerminalView { visible: term.id == pane.active-id; }` 范式。
- `start_session_in_tab`（`src/app/session_runtime.rs:54`）是所有 Tab 内会话的统一起点，RDP/VNC 应在此获得新分支。
- 密码加密：`Secret` + ChaCha20-Poly1305（`src/config`）。
- i18n：Rust 侧 `t("中文","English")`；Slint 侧 `@tr("English")` + `.po` 语言包。
- MTerm 已 vendored `vendor/i-slint-backend-winit`（`[patch.crates-io]`）。rustterm 侧 vendored `ironrdp` 0.17.0 / `rdp-rs-2-patched` / `vnc-rs` 0.5.3，但**本项目走 rsproxy-sparse 镜像、crates.io 可通，无需 vendor**（见 §7.1.1）。
- **可移植性边界（重要）**：**协议状态机与输入编解码可移植**；**帧输出消费、证书信任、认证方式选择、传输层配置需按当前 crate API 重写** —— 两个协议都有不能照搬之处（RDP 是 API 漂移，VNC 是 fork 私有类型缺失），详见 §7.1.1 风险条目。
- ⚠️ **rustterm 的参考实现不能直接编译**（`RdpOutputEvent::DesktopUpdate` 已删、`ServerCertificateVerifier` 不存在），但 0.17 直出原始像素使其帧合成/JPEG 编码可整段丢弃 —— 详见 §7.1.1 风险条目。
- ⚠️ **Slint 不支持位运算符**（`|`、`&`、`~`），按钮掩码需用算术（`set-bit`/`clear-bit` + `has-bit` 幂等守卫）实现，否则 `remote_desktop_view.slint` 会导致全仓 `cargo check` 失败。
