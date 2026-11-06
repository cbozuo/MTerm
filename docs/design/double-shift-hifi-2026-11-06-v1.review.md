# 验证记录 · Double Shift 全局搜索高保真 v1

**被验件**：`docs/design/double-shift-hifi-2026-11-06-v1.html`
**类型**：单文件可交互高保真（内联 CSS / JS / SVG，无外部资源）
**验证时间**：2026-11-06 16:47（Asia/Shanghai）
**验证方式**：Node VM 静态检查 + Edge 156 headless CDP 实交互 + 截图巡检；2026-11-06 16:37 追加分类焦点与中英文搜索回归；2026-11-06 16:47 追加拼音搜索（全拼/首字母）回归

## 冻结基线

| 项 | 值 |
|---|---|
| SHA-256 | `F60E93A4C5525D63943B0D56DF0ED3AA19BB2CB3855CF03060AB14E60706B4A6` |
| 尺寸 / 行数 | 183,440 字节 / 3,101 行 |
| 应用代码改动 | 0（本次任务仅新增 `docs/design/` 下的 HTML、规格、研究与预览图） |

## 结果摘要

**判定：通过。**

### 静态检查

- 1 个内联 `<script>`；`SYNTAX OK`
- `55` 个 id，无重复
- `42` 个直接 `$('#id')` / `getElementById` 引用，无悬空
- `RUNTIME OK`（页面加载与初始化未抛异常）
- 无外部 CDN、字体、图片或网络请求；图标全部来自内联 SVG
- `.sr-only` 已定义，aria-live 文本不参与视觉布局；`prefers-reduced-motion` 分支存在

### CDP 交互检查

| 场景 | 实测 |
|---|---|
| 初始态 | 文档标题正确，overlay 隐藏，模拟器应用窗口 1240×720 |
| 双击 Shift | 150ms 双击打开浮层，焦点落在 `#palSearch`，面板 920×568 |
| 初始列表 | 5 条最近使用 + 4 个最近搜索 chips；首条 `web-prod-02` 标记「已打开」 |
| 搜索与高亮 | `web` 返回 3 条，首条 `web-prod-02`；预览显示主机/分组/认证/标签；命中 3 个 `<mark>` |
| 键盘导航 | ArrowDown / End / Home 选中正确；Enter 执行后关闭 |
| 分类快捷键 | Alt+1..5 依次为 全部 / 会话 / 命令 / 设置 / 动作；Alt+4 不再错位到动作 |
| 命令执行 | 命令分类 `docker` 4 条；Enter 写入终端（13→15 行）；Ctrl+Enter 只填入命令栏 |
| 会话执行 | Enter 切换终端到 `root@web-prod-01` 并关闭浮层 |
| 已打开会话次操作 | Ctrl+Enter 复制 `ssh user@host -p port`，toast「已复制 SSH 命令」 |
| 设置执行 | 设置「主题」Enter 切为浅色，文档页头标签同步为「浅色」 |
| 动作执行 | Enter 执行并 toast；Ctrl+Enter 显示「快捷键：Ctrl+Shift+N · 新建窗口」后关闭 |
| 空态 | `zzzz-not-found` 回显，推荐 chips 与建议可见 |
| 压力模式 | 索引 6,842 项；`web` 查询 4.3–12.3 ms（两次运行），均 <16 ms；结果 8 行 |
| 遮罩 / 面板点击 | `mousedown` 遮罩关闭；点击面板内部不关闭 |
| 无障碍属性 | `dialog` / `aria-modal`、`combobox` / `aria-activedescendant`、`listbox` / `option` / `aria-selected` 均存在 |
| 资源检查 | `scriptBlocks=1`、`externalRefs=0` |

**总计：44/44 通过，runtime errors = []。**

### 本轮追加回归（分类焦点 + 中英文搜索）

| 场景 | 实测 |
|---|---|
| 分类 tab 焦点方框 | 真实鼠标点击「全部」后 `activeElement=palSearch`、`:focus-visible=false`、`outline:none`；按 ArrowDown 后焦点仍在 `#palSearch`，方框不再出现 |
| 中文查询 | `主题` 2 条（首条「主题」）、`字体` 4 条（首条「字体」）、`新建窗口` 1 条、`设置` 9 条、`搜索` 1 条（全局搜索） |
| 英文查询 | `theme` 2 条、`font` 4 条、`new window` 1 条（新建窗口）、`settings` 9 条、`search` 2 条（全局搜索 + 终端查找）、`docker` 4 条 |
| 别名策略 | 设置/动作共 29 个 id 增加英文别名；拼音别名覆盖 48 个 id（设置/动作/快捷命令/会话分组），支持全拼、首字母与多音节连写 |

### 本轮追加回归（拼音搜索）

| 场景 | 实测 |
|---|---|
| 全拼 | `zhuti` → 「主题」「切换主题」2 条；`ziti` → 「字体」「字体放大」「字体缩小」「重置字体」4 条 |
| 连写与首字母 | `xinjianchuangkou` / `xjck` → 「新建窗口」1 条；`chakanneicun` → 「查看内存」1 条；`shengchan` → 生产环境 5 个会话 |
| 分类拼音 | `shezhi` / `sz` → 8 个设置项 + 「打开设置」动作；`docker` 仍为 4 条 |
| 中英一致性 | `theme` 与 `zhuti` 结果一致；`new window` 与「新建窗口」结果一致 |
| 断言汇总 | 9/9 通过，runtime errors = []（`.tmp-lead/ds-pinyin-test.json`） |

### 视觉巡检

已检查 1440×1200 视口下暗色/浅色首屏、设计判断、体验地图、LIVE PROTOTYPE、性能预算、项目结构、视觉规格与验收章节；模拟器暗色态下浮层层级、列表选中态、预览分区与底部操作区正常。`pal-coach` 已移至面板左下空白区，3 秒后自动隐藏，不遮挡「Enter 切换」按钮；`sr-only` 修复后底部不再出现多余的「最近使用 5 项」文本。

## 性能说明

- 常规索引 1,284 项：输入到结果约 0.4–4.4 ms（见 footer 实时值）。
- 压力索引 6,842 项：开启压力模式时预热查询路径，实际输入 `web` 实测 4.3–12.3 ms，低于设计预算 16 ms。
- 查询为纯本地同步计算，无网络、无异步索引、无骨架屏；列表只渲染可见 40 行，单行不重复创建 DOM。

## 与现有实现的关系

- 领域层分层对齐 `src/command_hint/`：`struct/model.rs` 纯数据、`struct/sources.rs` 数据源、`impls/index.rs` 索引、`impls/query.rs` 匹配、`impls/rank.rs` 排序、`impls/trigger.rs` 双击状态机；UI 桥为 `src/app/search_ui.rs`，组件为 `ui/quick_search.slint`，测试归档 `tests/app/search/`。
- `ui/app.slint` 只新增一个 `QuickSearch` 实例与一个 `search-requested(string)` 回调；逻辑不进入 58 万字节主文件。
- 触发点设计建议接在 `src/app.rs:12028-12031` 现有 lone-Shift 记录处；IME 左/右 Shift 的 C0 标记（U+0015 / U+0010）与尾随 U+0008 按「污染则取消」处理，`Ctrl+Shift+Space` 作为等价入口。
- 既有快捷键无冲突：Ctrl+Tab、Ctrl+B、Ctrl+Alt+B、Ctrl+F、Ctrl+Shift+R、Ctrl+Shift+N、Ctrl+Alt+Z、Ctrl+=/-/0 均保持原语义。
- 本记录只描述原型验证，不代表应用代码已实现。

## 产物

- `docs/design/double-shift-hifi-2026-11-06-v1.html` — 可交互高保真
- `docs/design/double-shift-hifi-2026-11-06-v1-preview.png` — 默认暗色态窗口预览（1240×720，92,624 字节）
- `docs/design/double-shift-spec-2026-11-06.md` — 交互规格（34,116 字节）
- `docs/design/double-shift-research-2026-11-06.md` — 研究记录（313 行 / 48,541 字节）
