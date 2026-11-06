# 独立评审报告 · 多行粘贴高保真稿 v1

**被审件**：`docs/design/multiline-paste-hifi-2026-11-06-v1.html`
**权威依据**：`docs/design/multiline-paste-spec-2026-11-06.md`（含 §2 代码事实、§4.1 色板、§4.2 派生 token、§4.3 几何、§4.4 对比度、§5.1–§5.4、§6 七条验收）
**评审人**：verifier（独立复核，未参与设计）
**评审时间**：2026-11-06

**冻结基线（本次实测的字节级快照）**

| 项 | 值 |
|---|---|
| 被审件 SHA-256 | `C3697CBC61354D4C2C9677589940EA9747FDCCF677E138559295AD859A3B96E0` |
| 被审件尺寸 / 行数 / mtime | 163895 字节 / 1597 行 / 2026-11-06 09:43:14 |
| spec SHA-256 | `A5DE66F94CB886CE6D0A346C94C380E35B156AB34B477055AAEE80200E331B47` |
| 运行时 | Node v26.13.0；Microsoft Edge 156.0.4314.58（headless 实渲染） |

**行号约定**：`L###` = 被审件行号；其余文件均为绝对语义路径 + `文件:行`。评审过程**未修改**被审件与任何 `ui/`、`src/` 文件；全部脚手架写在 `output/paste-hifi-verify/`，smoke JSON 写在 `.tmp-lead/verifier-smoke.json`（spec §7 允许范围）。

---

## 一、结论摘要

**判定：不通过（需修订后复审）。28 项 pass / 10 项 fail（共 38 项）。**

稿子在「工程可用性」这一层是干净的——单文件零外链、内联脚本语法与初始化零报错、44 个 id 零悬空、§4.1 色板 34/34 逐值一致、spec §6 七条全部有据通过、7 个状态帧覆盖到位、D1/D2 的按键语义与按钮文案完全落地。但它在**自身数字自相矛盾**和**与权威源码对账**这两件事上失守：同一页面里档位阈值给两套互斥定义（L570 vs L770），风险行标记数与页面自己印的「3 处」对不上（L814/L800 vs L827），对照区的「现状」文案与 `ui/app.slint` 的实参不符。这三类问题都会**直接误导下游实现者**，因此不能按现状交付。

**最严重的 3 条**

1. **严重** — st-b 帧风险口径三处互斥：统计条 L814 与框顶汇总 L800 都说「3 处（A 级 2 · B 级 1）」，实际渲染了 **4** 个风险行（dg 2 + wr 2，L822/L826/L823/L827），其中 L827 `tail -n 20 /var/log/nginx/access.log` 既**不在** D4 的 B 级清单里，也**没有**行尾类别徽标。
2. **严重** — 档位阈值自相矛盾：L570 定义「评审档 = >12 行 **或** >600 字符」，L770 定义「标准档 = 4–12 行 **或** >600 字符」；权威 `src/terminal/impls/input.rs:93-104`（`COMPACT_CHAR_LIMIT = 600` / `COMPACT_LINE_LIMIT = 12`，任一超出即 large）判定 **L570 对、L770 错**。
3. **高** — 「打开后焦点落主按钮、关闭后焦点回终端、Tab 序 = 视觉序」被 L631 与 spec §5.4 声明，但稿内**没有任何 `autofocus` 或 `focus()` 调用**，只有 CSS 焦点环；Enter 键可用是靠脚本的视口兜底（`activeFrame()`），不是真实焦点。

---

## 二、A 组 · 语法 / 运行时 / 资源 / 引用（5 项，5 pass）

| # | 检查项 | 结论 | 证据 |
|---|---|---|---|
| A1 | 单文件 HTML、无外链资源、离线可开 | **pass** | `output/paste-hifi-verify/smoke.js`：`EXTERNAL_FOUND none`（检测 link/script/img/iframe/audio/video/source 的 src\|href、`@import`、css `url(http)`、fetch/XHR/importScripts 全 0）；另用 `msedge --headless=new` 以 `file:///` 直接实渲染成功并落盘 `output/paste-hifi-verify/shot-top.png`（207724 字节）、`shot-tall.png`（702003 字节） |
| A2 | 抽出的内联 `<script>` 通过语法检查 | **pass** | `smoke.js`：`script_tags=1`、`#0 attrs="" chars=9776`、`SYNTAX #0 OK`（`new vm.Script(src,{filename})` 编译，等价 `node --check` 且带行号） |
| A3 | DOM stub 下初始化与关键路径无异常 | **pass** | `smoke.js`：`RUNTIME exec: OK`；`ERRORS (0)`、`CONSOLE (0)`；对页面全部 44 个可点元素逐个 `.click()`：`CLICKS tried=44 threw=0`；verdict 四项全 0（`syntax_fail=0 / runtime_exec_fail=0 / runtime_errors=0 / external_resources=0`） |
| A4 | `$()`/`getElementById`/`querySelector` 引用的 id 全部存在（悬空引用 = 缺陷） | **pass** | `idcheck.js`：`DEFINED IDS (44)`（31 个 SVG symbol + 5 个 section + 7 个帧 + 1）、`REFERENCES (180)` 全部形如 `OK anchor href="#id"`、**`dangling = 0`**。注：脚本本身只用 `$$()` 类选择器（`var $$ = function (sel, root) { return Array.prototype.slice.call((root \|\| document).querySelectorAll(sel)) }`），不按 id 取元素，所以悬空风险只可能来自锚点——已全部命中 |
| A5 | CSS 变量无未定义引用（交叉口径） | **pass** | `.tmp-lead/smoke.mjs` 交叉验证 `css:{definedCount:83, usedCount:62, undefinedVars:[]}`、`ids:{markupIdCount:44, refCount:0, dangling:[]}`、`verdict=PASS`（与 A4 一致；`refCount=0` 是因为该工具不识别 `$$` 选择器，非缺陷） |

> **参考基线对照**：同一批脚本跑参考稿 `docs/design/left-panel-session-hifi-2026-10-28-v3.html` 时，`smoke.js` 会报 1 条 stub 缺口噪声（`listener(panelMoreBtn.click): Cannot read properties of undefined (reading 'getBoundingClientRect')`），`idcheck.js` 有 1 处 aria 悬空。本次被审件**两项噪声均为 0**，比参考基线更干净。

---

## 三、B 组 · Token 保真与视觉几何（9 项，4 pass / 5 fail）

### B1–B2 §4.1 色板逐值对账 = 34/34 全等

`output/paste-hifi-verify/audit-tokens.js`（先剥 CSS 注释再按括号深度扫描，共解析 292 条规则；`tokencheck.js` 首版因「尾随 `/* */` 并入下一个声明 key」导致暗块 53 个变量只解析出 18 个，已废弃，其旧结论全部作废）：

- **dark pass=17 / fail=0 / absent=0**
- **light pass=17 / fail=0 / absent=0**
- **TOTAL 34/34 PASS**

变量名口径 `--slot-<slot>`，`:root` 30 个、暗块 53 个、亮块 52 个自定义属性。**§4.1 保真这一条是干净的**（| # | 检查项 | 结论 |）。

### B3–B9 明细

| # | 检查项 | 结论 | 证据 |
|---|---|---|---|
| B3 | §4.2 派生 token 逐值对账 | **fail** | 见下方「派生 token 对账表」：`--field`、`--line-2` 两主题 EXACT；`--field-2`（两主题）、`--danger-strong`（亮）、`--popup-bg-inset`（两主题）取值不符。四者**引用次数均为 0**（死声明），无视觉影响，属保真性/一致性问题 |
| B4 | 除 §4.1 原值外的新色相是否只有 §4.4 第 8 条允许的 `--dg-ink` | **pass** | `audit-tokens.js` 全量颜色字面量分类：distinct=101，`SPEC-EXACT=30 / SLOT+ALPHA=28 / BLACK-WHITE-ALPHA=7 / BLACK-WHITE=1 / UNKNOWN=35`。逐项人工裁定后，唯一「非 §4.1 原值的新色相」是 `--dg-ink` 亮色 `#8E2F28`（L103），符合 §4.4 第 8 条：①4 处落点全部压在 11% danger 淡染上（L304 `.dlg-head .ic.dg`、L338 `.notice.dg`、L379 `.pv-line.dg .pv-g`、L459 `.result.dg`）；②危险填充/描边仍取 `--slot-dg`（L420-423 `.btn.dg-out` 的 border/color 均为 `var(--slot-dg)`）；③L1286-1288 设计说明写明派生规则、实测 6.55:1 与「严格保真替代方案」。`--on-solid`（暗 `#161619` / 亮 `#FAF8F4` = slot-root / slot-panel 原值）、`--wr-ink`（=slot-wr）、`--dg-soft/--wr-soft/--ac-soft/--ok-soft/--*-line`（既有色 × alpha）均属合规派生 |
| B5 | `--ac-hov` 是否等于 `Theme.accent-hover` | **fail** | 权威 `ui/theme.slint:312`：`accent-hover: root.accent-solid.brighter(0.12)`。`brighter` 在仓库内无自定义实现（`ui/` 下无 `fn brighter`），属 Slint 内建。两种常见语义推算：`L×(1+0.12)` → 暗 `#25DDC8` / 亮 `#238478`；`L+0.12` → 暗 `#42E2CF` / 亮 `#2CA697`。稿内 L86 `--ac-hov:#25D8C4`、L103 亮 `#1B6A5F`——**两组推算都不等**；且亮色 `#1B6A5F`（L 26.1%）比 `--slot-ac` 亮色 `#1F766B`（L 29.2%）**更暗**，与 L85 注释「仅提亮，对比度只升不降」自相矛盾。引用面 1 处（`.btn.primary:hover`） |
| B6 | 页面壳 token 是否忠于所声称的取样源 | **fail（低）** | L44 注释称沿用参考稿取样口径。与 `docs/design/left-panel-session-hifi-2026-10-28-v3.html` 逐值比对：暗壳 9 项中 8 项一致（L27 `--page:#0B0C0E; --stage:#0E1013;`、L30 `--doc-panel:#121417; --doc-elev:#17191D; --doc-line:#26292F; --doc-line2:#33373F;`、L31 `--doc-t1:#E8EAEE; --doc-t2:#9BA1AC;`），亮壳 9 项中 8 项一致（L46 `--page:#EDEBE7; --stage:#F6F4F0;`、L49 `--doc-panel:#FAF8F4; --doc-elev:#FFFEFA; --doc-line:#DEDAD3; --doc-line2:#CCC7BE;`、L50 `--doc-t1:#302E2A; --doc-t2:#75716B;`）。**唯一偏离：`--doc-t3` 暗 `#7C828D`（源 `#6E747E`）、亮 `#6F6B64`（源 `#969188`）**，两处都往更高对比度调，且未标注 |
| B7 | 字号是否落在 §4.2 刻度（10/11/13/13/15/18px） | **fail（低）** | 刻度见 `ui/theme.slint:453-458`。稿内脱刻度值：`.stat .k{10.5px}`、`.pv-toggle{10.5px}`、`.pv-line{font-size:11.5px}`、`.pv-badge …{9.5px}`、`.pv-g{10.5px}`（`.pv-bar{11px}` 合规） |
| B8 | 按钮/卡片几何是否与现状一致（§4.3、§5.3 D1） | **pass（带 1 处例外）** | L408-429：`.btn{height:var(--btn-h)=30px; min-width:var(--btn-min-w)=80px; border-radius:var(--r-sm)=4px; font-size:var(--fs-md)=13px; font-weight:700}` —— 与 `ui/confirm_dialog.slint:10-12`（30px / min-width 80 / radius-sm）及 `:18-19`（fs-md / fw-strong）一致；`.btn::after{top:-1px;bottom:-1px}` 命中区 32px；卡片 `.dlg` = `popup-bg` + `radius-lg` + 可拖动 + `.scrim`，评审档 `min(宽-40px,1160px)` ✓。例外见缺陷 L4（取消按钮圆角） |
| B9 | 遮罩是否用 `Theme.modal-scrim` 语义 | **fail（中）** | 权威：`ui/theme.slint:369` `scrim-base: dark ? #0c0d11 : #f4f5f8`，`:343` `scrim-strength: 0.32`（0..1 可调），`:368` `scrim-alpha = scrim-strength`，`:372-379` `modal-scrim = scrim-base.with-alpha(scrim-base-alpha)`；渲染层 `ui/widgets.slint:804-815`（`Scrim` 铺满纯色底 + 可叠 5 种装饰）。稿内改用自造 `--scrim`：暗 `rgba(0,0,0,.52)`、亮 `rgba(28,26,22,.42)`。**方向性偏离**：现状亮色遮罩是**浅色薄纱**（`#F4F5F8` @0.32，压在亮终端 `#FAF8F4` 上几乎等于不提暗），稿子是**深色纱**（压上去 ≈ `#9D9B97`，明显压暗）；且稿内没有体现 `scrim-style` 9 档与浓度滑杆的可配置语义。按 Lead 给的「40–60%」口径稿内取值合格（52% / 42%），故列为中而非严重 |

### 派生 token 对账表（B3 证据）

| token | 权威来源 | 权威值 | 稿内值 | 判定 |
|---|---|---|---|---|
| `--field` | `ui/theme.slint:279` `dark ? #191b21 : #f4f5f8` | 暗 `#191B21` / 亮 `#F4F5F8` | 暗 `rgba(25,27,33,1)` / 亮 `rgba(244,245,248,1)` | **EXACT** |
| `--line-2` | `ui/theme.slint:281` `dark ? #3f444f : #c7c7cc` | 暗 `#3F444F` / 亮 `#C7C7CC` | `#3F444F` / `#C7C7CC` | **EXACT** |
| `--field-2` | `ui/theme.slint:280` `dark ? #1d2027 : #eef0f4` | 暗 `#1D2027` / 亮 `#EEF0F4` | 暗 `rgba(34,38,45,1)`=`#22262D` / 亮 `rgba(232,229,225,1)`=`#E8E5E1`（= `slot-hov`） | **FAIL 两主题** |
| `--danger-strong` | `ui/theme.slint:330` `#dc4b44`（主题无关常量） | 两主题 `#dc4b44` | 暗 `#dc4b44` ✓ / 亮 `#8E2F28` ✗ | **FAIL（亮）** |
| `--popup-bg-inset` | `ui/theme.slint:246` `base-bg-root.with-alpha(popup-alpha)`，`:149 base-bg-root = slot-root` | 基色 `slot-root`（暗 `#161619` / 亮 `#F2F0EC`），alpha = `1.0 - popup-transparency` = `1.0`（`:122` 默认 0.0） | 暗 `rgba(27,29,33,.92)`=`slot-palt` / 亮 `rgba(233,230,223,.92)`=`slot-palt` | **FAIL 两主题（基色 + alpha）** |
| `--popup-bg` | `ui/theme.slint:235` `base-bg-panel.with-alpha(popup-alpha)` | 基色 `slot-panel`，alpha 1.0 | `rgba(24,26,29,.98)` | 基色 ✓ / alpha 近似（低） |
| `--popup-bg-elev` | `ui/theme.slint:236` | 基色 `slot-elev` | `rgba(30,32,36,.99)` | 基色 ✓ / alpha 近似（低） |
| `--scrim` | `ui/theme.slint:369-379` | 见 B9 | 自造 rgba | **FAIL（语义）** |

> 死声明统计（`var(--x)` 引用次数）：`--danger-strong` 0、`--field-2` 0、`--popup-bg-inset` 0、`--field` 0、`--line-2` 0、`--mark` 0；`--popup-bg` 10、`--ac-soft` 8、`--ac-hov` 1。即 B3 的四处不符**都不会渲染**。

---

## 四、C 组 · 事实一致性（9 项，5 pass / 4 fail）

| # | 检查项 | 结论 | 证据 |
|---|---|---|---|
| C1 | 弹框触发条件 | **pass** | L598 写「中键 / `Ctrl+Shift+V` → 文本含 `\r`/`\n` 时弹通用 ConfirmDialog（`src/app.rs:12759`）」，与权威 `src/app.rs:12760` `if text.contains(['\r','\n']) && (confirm_multiline \|\| force_review)` 一致（`confirm_multiline` 即设置项 `paste_confirm_enabled`，默认 true，`ui/app.slint:1701-1704`） |
| C2 | large 档阈值口径 | **fail** | L570 正确；L770 错；两者互斥。详见缺陷 S2 |
| C3 | 按键语义（spec §5.3 D2 / §5.4） | **pass** | 脚本 `keydown`：`Escape` → `[data-do="cancel"].click()`；`Enter` → `frame.querySelector('.btn.primary').click()`（**绝不触发执行**）；`ev.altKey && (key==='p'\|\|'P')` → `[data-do="paste-run"].click()`（唯一执行路径）。帧内提示亦标「`Enter = 主按钮`」「`Alt+P`」。与 D2 完全一致 |
| C4 | 按钮文案与三动作结构（D1） | **pass** | 逐帧抽取（`frames.js`）：st-a 因「末尾无回车」**只渲染一个主按钮**「粘贴」（L703 说明一致）；含 A 级的 st-b/st-g 主按钮文案为「粘贴（已去掉末尾回车）」；危险动作为描边式 `.btn.dg-out`；取消为 `.btn.ghost`。与 D1 逐字一致 |
| C5 | D3 复选框边界文案 | **pass** | L658 常驻小字写「含危险命令或超过 100 KB 时仍会提示」，脚本勾选后 trace 亦写同一口径，与 D3「`force_review` 与 A 级危险不受开关静默」一致 |
| C6 | D5 目标上下文与 bracketed 文案 | **pass** | st-b 标题栏 `root@db-02:2222` + 标签 `web-01`/`db-02`（当前 tab `on`）；st-d 为「bracketed paste 未启用」并给「整段多行将逐行执行」，与 D5 的两分支文案一致 |
| C7 | 危险行标记双向核对（D4） | **fail** | A 级 8 处全部真实（L822 `rm -rf /var/tmp/cache/*`、L826 `:> /var/log/nginx/error.log`、L910、L917、L1064、L1144 `mkfs.ext4 -F /dev/nvme0n1p2`、L1145 `dd if=/dev/zero of=/dev/nvme0n1b`、L1224）。B 级 6 处中 **L827 `tail -n 20 …` 误标且无徽标**；其余 5 处（L823 `sudo`、L907 `sudo -u deploy`、L912 `curl … \| bash`、L1066 `sudo`、L1146 `iptables -F && iptables -X`）符合 D4。反向核对无漏标：L824/L914/L919 的本地 `curl healthz` 非 `curl\|sh`，L1147 `partprobe /dev/…`、L1148 `mount /dev/…` 不在 D4 的 16 条清单内，不标正确 |
| C8 | 「现状」对照区是否忠于 `ui/app.slint` | **fail** | L1319 现状 message 写「剪贴板内容包含多行，确认后立即粘贴。」，权威 `ui/app.slint:8212-8214` 实为「剪贴板内容包含换行，粘贴后可能连续执行多个命令，请确认内容后再继续。」；L1327 现状按钮写「确认 / Confirm」，权威 `ui/app.slint:8217` 实为 `confirm-label: root.lang-en ? "Paste" : "粘贴"`。详见缺陷 H3 |
| C9 | 行号列与折叠计数是否内部自洽 | **fail** | st-b 折叠账不平（见 H2）；st-e（47 = 34 + 4 + 7 + 2 ✓）、st-f（8 412 = 8 400 + 12 ✓，与 L1143「以下只渲染 12 行样本」逐字相符）自洽 |

---

## 五、D 组 · spec §6 七条验收（7 项，7 pass）

| §6 | 要求 | 结论 | 证据 |
|---|---|---|---|
| ① | 单文件 HTML、无外链资源、离线可开；顶部有暗/亮切换且整页同步切换 | **pass**（带保留说明） | 同 A1；`<html lang="zh-CN" data-theme="dark" data-lang="zh">`（L2）；顶栏有 `[data-set-theme]`/`[data-set-lang]` 按钮，脚本 `setTheme/setLang` 只改 `document.documentElement.dataset` 并刷新 `aria-pressed`。`data-theme=` 全稿仅 3 处（L2 / L43 / L94 两条选择器），页壳与所有非帧元素随主题切换。**保留说明**：7 个状态帧用固定 `data-skin`（L777/860/949/1022/1100/1184/1301）不随页面主题走——这是刻意设计（页面已注明帧肤色，便于同屏对比暗/亮），但"整页同步切换"的字面口径需按此分层理解：壳同步、样例帧不同步 |
| ② | ≥6 个状态（暗紧凑 / 暗标准含风险 / 暗评审>12 行 / 亮标准 / 亮评审 / 折叠或控制字符之一） | **pass** | 实渲 **7 帧**：`st-a` L700（暗·紧凑 380px）、`st-b` L767（暗·标准 640px·含危险命令）、`st-c` L850（暗·评审·>12 行 + 内部滚动）、`st-d` L940（亮·标准·bracketed 未启用）、`st-e` L1012（亮·评审·折叠/展开 + 控制字符剥离提示）、`st-f` L1090（暗·评审·超长摘要 >200 行/>100 KB）、`st-g` L1175（亮·紧凑·单行 + 末尾回车 + A 级）→ 覆盖要求全部状态 |
| ③ | 页面壳 + 模拟器两级结构、与既有高保真页同构 | **pass** | 壳：顶栏 + `.hero` L561 + `#sec-notes` L589（现状/新设计两卡）+ `#sec-states` L676 + `#sec-sim` L691 + `#sec-contrast` L1243 + `#sec-continuity` L1292；模拟器：`.sim > .win`（标题栏 / 标签栏 / 终端区）+ `.scrim`；图标为内联 SVG sprite（L504 起），与参考稿同构 |
| ④ | 所有颜色值可追溯到 §4 槽位名 | **pass**（分层） | `slot-*` 字样 273 处；预览区注释直接标 `popup-bg`/`slot-t1`/`slot-t3` 与实测比值。分层口径：应用面颜色 100% 走 §4.1/§4.2（B1–B4 已逐值对账）；页面壳（`--doc-*`/`--page`/`--stage`）沿用参考稿的独立取样口径（B6），不属 §4.1 槽位体系 |
| ⑤ | 与现状弹框的连续性可见 | **pass**（带保留说明） | `#sec-continuity` L1292-1362 为左右对照：左「现状·通用 ConfirmDialog（380px）」实渲（含 `.scrim`/`.dlg.compact`），右侧逐项列「保留 / 新增 / 改动」；L1338 保留「按钮 30px 高 / min-width 80px / radius-sm」、卡片底 `popup-bg` + 1px `border-strong` + `radius-lg` + `pop-shadow`。**保留说明**：左侧"现状"块的 message 与按钮文案与权威实参不符（缺陷 H3），故"连续性"这一条按结构判定 pass，按文本保真判定 fail（已计入 C8） |
| ⑥ | 无 JS 报错；抽出的内联脚本可通过语法检查 | **pass** | 同 A2/A3（`vm.Script` 编译 OK + stub 初始化 0 异常 + 44 次点击 0 抛错 + `ERRORS/CONSOLE` 均 0）；另有 headless Edge 实渲染无白屏/无未样式化闪烁 |
| ⑦ | 文案 zh/en 双份并存 | **pass** | `span.zh = 118`、`span.en = 118` 逐处配对；`html[data-lang="en"] .X .zh{display:none}` / `.X .en{display:inline}` 成对规则覆盖 `.notice`(L340-342)、`.pv-toggle`(L368-370)、`.pv-badge`(L392-394)、`.pv-fold`(L405-407)、`.btn`(L431-433)、`.enter-note`(L438-440)、`.opt`(L449 起) 等；帧标题、脚注、复选框、结果条全部双语 |

---

## 六、E 组 · Lead 追加的 8 条硬性 UX（8 项，7 pass / 1 fail）

| # | 要求 | 结论 | 证据 |
|---|---|---|---|
| E1 | 遮罩 40–60% 且用 `Theme.modal-scrim` 语义 | **pass（半条）** | 稿内 `--scrim` 暗 `rgba(0,0,0,.52)`（52%）、亮 `rgba(28,26,22,.42)`（42%），均在 40–60% 内，且用 `--scrim` 变量统一引用 → 满足 Lead 口径；但未取 `Theme.modal-scrim` 的真实语义 → 计入 B9（中） |
| E2 | 焦点管理（打开落位、关闭回终端、`focus-visible`、Tab 序 = 视觉序） | **fail** | CSS 侧齐备（L41 `--focus-w:1px`、L115 `:focus{outline:none}`、L116 `:focus-visible{outline:var(--focus-w) solid var(--slot-ac)}`、L435 `.btn.is-default{outline:…}`）；但**全稿无 `autofocus`、无任何 `focus()` 调用**（`focus\|autofocus` 仅命中上述 CSS 与 `focusable="false"`）。L631 自称「打开后焦点落卡片内首个可操作元素（预览区）；关闭后焦点回终端（对应 `terminal-focus-sequence += 1`）」在静态稿中只被声明、未实现。Enter 可用是脚本 `activeFrame()` 的视口兜底（优先 `document.activeElement`，否则取视口垂直中点最近的未关闭帧），不依赖真实焦点 |
| E3 | 统计条/徽标预留空间、不推动布局（`tabular-nums`） | **pass** | `.stat .v{font-variant-numeric:tabular-nums}` + 固定宽 `.n-lines{min-width:34px}`/`.n-chars{56px}`/`.n-risks{30px}`；`.pv-g` 固定 `--gutter-w:44px` + `text-align:right`；`.pv-badge` 固定 `--badge-w:96px` → 1 行↔47 行、0 处↔4 处风险均不跳动 |
| E4 | >12 行折叠态必须有展开/收起入口并显示被折叠行数 | **pass（但数字有误）** | 存在成对入口与计数：st-b 顶部 L819「以上另有 2 行（已折叠）」+ 底部 L828「以下另有 3 行（已折叠）」+ L832 按钮「展开全部（另有 5 行）」；st-e 有「另有 38 行」；st-f 有「以下另有 8 400 行」+ 摘要行「前 8 400 行未渲染（虚拟化）」。计数与实际 DOM 不符的部分计入 H2 |
| E5 | 颜色非唯一信息载体（危险行需文字/图标标记） | **pass** | 危险行 = 左 2px 竖标（`.pv-line.dg/.wr` 的 border-left）+ 11% 淡染 + 行尾「图标 + 中文类别名」徽标（如「递归强制删除」/「权限提升」）；帧内还有 `<span class="legend"><i class="a"></i>A 级<i class="b"></i>B 级</span>` 文字图例；A/B 级同时用颜色与文字区分 |
| E6 | 深浅色各自达标（正文 ≥4.5:1、次要 ≥3:1）；危险行不得白字实心红 | **pass** | 预览正文与统计条一律 `slot-t1` 压 `popup-bg`（暗 9.38 / 亮 12.37）；行号列用 `slot-t3`（暗 3.66 / 亮 3.02，§4.4 允许作装饰）；危险行正文仍 `t1`（压 11% danger 淡染 8.29/10.59），徽标文字用 `slot-dg`（5.32/5.22）；`--dg-ink` 仅用于压在淡染上的文字（4.70/6.55）；**全稿不存在白字实心 red**——唯一的实心红是 L1329 明确标注的反面样例（并在 L418-419 注明「不得沿用白字」）。稿内 38 格对账表逐格复算全部命中（见附录 §A.2） |
| E7 | 图标一律内联 SVG、无 emoji | **pass** | 内联 sprite `<symbol id="i-*">`（L504 起，31 个），全部以 `<use href="#i-…">` 引用；全文无 emoji 字符 |
| E8 | 4/8px 间距节奏 | **pass** | L29 `--s-1..--s-8` = 4/8/12/16/20/24/32/40；按钮 padding `0 14px` 与卡片 `padding:20px; spacing:12px` 均落在节奏上 |

---

## 七、缺陷清单（13 条，按严重度）

### 严重

**S1 · st-b 帧风险口径三处互斥（计数错 + 误标 + 缺徽标）**
- **位置**：`L814`（统计条风险格 `3`）、`L800`（框顶「本段含 **3 处危险命令**（A 级 2 · B 级 1）」）、`L822`/`L826`（dg 2 行，均带徽标）、`L823`（wr，带「权限提升」徽标）、`L827`（wr，`tail -n 20 /var/log/nginx/access.log`，**无徽标**）
- **证据**：`frames.js` 逐帧抽取输出「dg=2, wr=2, 徽标=3；!! 风险行缺徽标: g10 "tail -n 20 /var/log/nginx/access.log"」。同帧的 `L810/L812/L814` 三格统计（行 13 / 字符 428 / 风险 3）与 `L800` 一致，说明「3」是设计意图 → 错的是 L827 的 `wr` class。对照 st-c（L884 框顶「18 行 · 1,146 字符，含 4 处危险命令（A 级 2 · B 级 2）」，`frames.js` 实测 dg2/wr2/徽标 4）完全自洽，说明 st-b 是孤例。
- **为什么是缺陷**：①同一帧内三个数字互相矛盾（页面自己印 3、渲染 4）；②`tail` 是只读命令，**不在** D4 的 B 级清单（sudo / chmod 777 / curl|sh / useradd / iptables / kill -9 / history -c / git reset --hard / apt remove）；③唯一承载危险语义的行尾徽标在 L827 缺失，违反 spec §5.2 第 ③ 项「危险行 = 竖标 + 淡染 + 行尾类别徽标」；④统计条与框顶汇总正是本稿的核心卖点（G1/G2），此处失真会直接误导实现者按「B 级 = 任何可疑命令」或「计数可随意」来编码。
- **最小修复**：删除 L827 的 `class="wr"`（保留 `class="pv-line"`），使渲染风险行 = 3，与 L800/L814 一致；若确需把 `tail` 纳入 B 级，则须同时 ①在 L827 补 `pv-badge wr` 徽标、②把 L800/L814 改为「4 处（A 级 2 · B 级 2）」、③在研究稿的 16 条清单中补 `tail`/只读日志读取这一类目。

**S2 · 档位阈值两处互斥定义（会直接误导 `paste_requires_large_review` 的实现）**
- **位置**：`L570`「三档自适应：紧凑 380px（≤3 行）、标准 640px（4–12 行）、评审 min(宽-40px,1160px)（>12 行 **或** >600 字符）」 vs `L770`「（st-b）**4–12 行或 >600 字符**的新增档」 vs `L769`（st-b 帧标题「暗·标准档 640px·含危险命令」）
- **证据**：权威 `src/terminal/impls/input.rs:93-104`：
  ```rust
  pub(crate) fn paste_requires_large_review(text: &str) -> bool {
      const COMPACT_CHAR_LIMIT: usize = 600;
      const COMPACT_LINE_LIMIT: usize = 12;
  ```
  即「>600 字符 **或** >12 行 → large」。故 **L570 正确、L770 错误**：按 L770 的写法，「13 行文本」会被判为标准档，而权威判定它是评审档；本帧 `L769` 自称标准档、统计却是「行 13」（L810），恰好落在错误定义一侧。
- **为什么是缺陷**：违反 spec §5.1（「评审 = 对齐 `paste_requires_large_review`」）与 §2 代码事实；两套定义共存会让实现者二选一，选错就导致 >600 字符但 ≤12 行的内容误入标准档（丢掉行号/危险标记/统计条）。
- **最小修复**：把 L770 改为「**4–12 行且 ≤600 字符**的新增档」，并在 L570 旁补一句「>600 字符与 >12 行任一命中即评审档（`src/terminal/impls/input.rs:93-104`）」；同时核对 L601「5–12 行的中等内容无档位」的措辞与之一致。

### 高

**H1 · 焦点管理只声明未实现（E2）**
- **位置**：L631（自述）、spec §5.4（Tab 序 = 视觉序）、`ui/app.slint:8221`（`root.terminal-focus-sequence += 1`）
- **证据**：`grep focus|autofocus` 在被审件内只命中 L41/L115/L116/L435 的 CSS 与 `focusable="false"`；脚本无 `focus()`、无焦点陷阱、无 `tabindex` 管理（全稿 `tabindex=` 7 处，均为静态值）。
- **为什么是缺陷**：spec §5.4 把「`Tab`/`Shift+Tab` 在 取消 / 粘贴（不含末尾回车）/ 粘贴并执行 / 不再提示 间循环、焦点环用 `accent`」列为键盘契约；现状弹框的 `terminal-focus-sequence += 1`（关闭后焦点回终端）也是真实行为。只画焦点环而不给落位，会让"高保真"在交互层失真——尤其 D2 把 `Enter` 交给主按钮，若不落位则 Enter 语义悬空。
- **最小修复**：给主按钮加 `autofocus`（或在脚本初始化里 `document.querySelector('[data-skin] .btn.primary').focus()`），并在关闭分支（`closeDlg`）里把焦点移回终端区容器（加 `tabindex="-1"` 并 `focus()`），同时把 `tabindex` 顺序写成与视觉顺序一致（取消 → 粘贴 → 粘贴并执行 → 不再提示）。

**H2 · st-b 折叠计数与实际 DOM 不符（行号缺 2 与 14）**
- **位置**：`L819`（顶部提示「… 以上另有 **2** 行（已折叠）」）、`L828`（底部提示「… 以下另有 **3** 行（已折叠）」）、`L832`（按钮「展开全部（另有 **5** 行）」）、实际 `is-hidden` 行 `L820`(g3)、`L821`(g4)、`L829`(g12)、`L830`(g13)
- **证据**：`count-rows.js` 输出 gutter 序列 = `1,3,4,5,6,7,8,9,10,11,12,13`（**缺 2 与 14**）、`is-hidden` = 4 行、折叠提示 = 2 行。总账校验：2（上折）+ 3（下折）+ 6（可见正文 g5..g10）+ 2（提示行）= 13 ✓ 与 L810 统计「13 行」自洽 → **少的正是尾部第 3 个折叠行（g14）**；按钮承诺展开 5 行，DOM 只会多出 4 行。
- **为什么是缺陷**：违反 UX 第 ④ 条（「显示被折叠行数」必须可信）与 spec §5.2 第 ⑥ 项；本帧是唯一演示"标准档首尾各留 1 行折叠"的样例，计数错了就等于折叠规范没被演示。
- **最小修复**：在 L830 之后补一行 `is-hidden`（gutter 14，如 `echo "rollout done"`），使尾部隐藏 3 行、展开得 5 行；并核对该帧行号连续（1..14 与统计 13 的口径二选一，建议把统计改为 14 或把顶部提示改为「以上另有 1 行」）。

**H3 · 「现状」对照区文案与权威源码不符（杜撰 message + 按钮文案错）**
- **位置**：`L1319`（message）、`L1327`（按钮「确认 / Confirm」）、`L1339`（"保留 标题文案"清单）
- **证据**：权威 `ui/app.slint:8212-8214` `message: root.lang-en ? "The clipboard contains line breaks and may execute multiple commands. Review it before pasting." : "剪贴板内容包含换行，粘贴后可能连续执行多个命令，请确认内容后再继续。"`；`ui/app.slint:8217` `confirm-label: root.lang-en ? "Paste" : "粘贴"`。稿内 L1319 写作「剪贴板内容包含多行，确认后立即粘贴。」（并丢掉了现状**本就有**的「粘贴后可能连续执行多个命令」这一安全警告），L1327 写作「确认」。
- **为什么是缺陷**：§6⑤ 要求"看得出是同一组件家族的加宽档"，其证据是左栏"现状"块；若该块的关键字符串是编的，则"连续性"论据失真，且被删掉的那句警告恰是本稿立论（G3"粘了就会执行"）的现状依据——把它删掉会让读者以为现状没有警告。标题（`ui/app.slint:8211`「确认多行粘贴」）与 `confirm-on-enter`（`:8210`）稿内写对了，说明目标是精确复刻，此二处属疏漏。
- **最小修复**：把 L1319 的中英文替换为 `ui/app.slint:8212-8214` 的逐字原文；把 L1327 的「确认 / Confirm」改为「粘贴 / Paste」；并在 L1339 的"保留"清单补一行「`message` 文案与 `confirm-label` 逐字保留（`ui/app.slint:8212-8217`）」。

### 中

**M1 · 遮罩语义偏离 + 亮色方向相反（B9）**
- **位置**：L91（暗 `--scrim:rgba(0,0,0,.52)`）、L105（亮 `--scrim:rgba(28,26,22,.42)`）、`.scrim` 用法（L292 起）
- **证据**：`ui/theme.slint:369` `scrim-base: dark ? #0c0d11 : #f4f5f8`；`:343` `scrim-strength: 0.32`；`:368` `scrim-alpha = scrim-strength`；`:377-379` `modal-scrim = scrim-base.with-alpha(scrim-base-alpha)`；渲染层 `ui/widgets.slint:804-815`。实算：暗色稿内 0.52 黑压 `#161619` → ≈ `#0B0B0D`，现状默认 0.32 × `#0c0d11` 压 `#161619` → ≈ `#131316`；亮色稿内 0.42 深色压 `#FAF8F4` → ≈ `#9D9B97`，现状 0.32 × `#f4f5f8` 压 `#FAF8F4` → ≈ `#F8F7F5`（几乎不压暗）。即：暗色稿子比现状重一档，亮色稿子与现状**方向相反**。
- **为什么是缺陷**：spec §4.2 把 `popup-bg` 等派生 token 都要求"可追溯"，遮罩虽未列 §4.1，但 §6⑤ 的连续性要求含遮罩（`ui/confirm_dialog.slint:60-61` 注释明确"遮罩统一到 Theme.modal-scrim"）；且现状的遮罩有 9 档样式 + 浓度滑杆，固定单一 rgba 无法表达这个可配置面。
- **最小修复**：在设计说明里标注「本稿遮罩取 40–60% 单值以体现"弹框与背板分离"，未实现 `Theme.modal-scrim` 的 `scrim-style`(9 档) × `scrim-strength`(默认 0.32) 可配置语义；亮色刻意改为深色纱以增强分离（现状亮色为浅纱）」，或改为引用 `Theme.modal-scrim` 的暗/亮基色 + 显式标注浓度。

**M2 · §4.2 派生 token 3 处取值不符（死声明，无视觉影响）**
- **位置**：L70 `--field-2`（暗）、L98 `--field-2`（亮）、L73 `--danger-strong`（亮）、L72 `--popup-bg-inset`（暗）、L99 `--popup-bg-inset`（亮）
- **证据**：见 §三「派生 token 对账表」（`ui/theme.slint:246,279-281,330`）；引用次数 0。
- **为什么是缺陷**：这 5 个 token 是照抄 `ui/theme.slint` 的派生表却抄错（`--field-2` 取成了 `slot-hov`、`--danger-strong` 亮色拿 `--dg-ink` 的值、`--popup-bg-inset` 基色取成了 `slot-palt` 且 alpha 用了 0.92），一旦后续有人启用它们就会拿到错色；同时它们是 §4.2 对账表里的"假通过"项。
- **最小修复**：按权威值改正（`--field-2` 暗 `#1D2027` / 亮 `#EEF0F4`；`--danger-strong` 两主题 `#dc4b44`；`--popup-bg-inset` = `slot-root` 基色 + `popup-alpha` = 1.0，即暗 `#161619` / 亮 `#F2F0EC`），或直接删除这些死声明。

**M3 · `--ac-hov` 与 `Theme.accent-hover` 不符，且亮色方向与注释矛盾（B5）**
- **位置**：L86（暗 `#25D8C4`）、L103（亮 `#1B6A5F`）、L427（唯一引用 `.btn.primary:hover`）、L85（注释「仅提亮」）
- **证据**：`ui/theme.slint:312` `accent-hover: root.accent-solid.brighter(0.12)`；两种 brighter 语义推算（`L×1.12` → `#25DDC8`/`#238478`；`L+0.12` → `#42E2CF`/`#2CA697`）均不等于稿内值；亮色 `#1B6A5F` 的 L 值（26.1%）低于 `--slot-ac` 亮色（29.2%），是"变暗"而非"提亮"。
- **为什么是缺陷**：主按钮 hover 是可见状态，稿内色值不在 §4.1 也不在 §4.2 的派生规则内（B4 的合规性判定里它是唯一"既有色调相内新增字面量"），且自述与实现相反。
- **最小修复**：改为按 `brighter(0.12)` 的实测结果取值（暗 `#25DDC8` 量级、亮 `#238478` 量级），或把注释改成中性描述（如「暗色近似提亮一档；亮色为保证深色字对比度改为压暗一档」）并注明该值非 §4.2 派生。

### 低

**L1 · 注释把 t2 的比值挂在 t1 名下，且写错 `popup-bg-inset` 的真值**
- 位置：L345-347、L1280。证据：注释写「`popup-bg-inset` 在暗色是 `#1B1D21`，t1 压上去只有 4.14 ❌」——`#1B1D21` 是 `slot-palt`（不是 `popup-bg-inset` 的真值 `slot-root #161619`），而 4.14 是**暗 t2/palt** 的比值（重算：暗 t1/palt = **9.08**、暗 t2/palt = **4.14**、暗 t2/elev ≈ 4.00）。同错见 L1280「`slot-elev`/`slot-inset` 上的 `slot-t1` 只有 4.00/4.14:1」。修法：把两处主语改为「`slot-t2` 压 `slot-palt`/`slot-elev`」，并把 `popup-bg-inset` 的真值改为 `slot-root`。

**L2 · `--doc-t3` 两主题偏离所声称的取样源且未标注**
- 位置：L45（暗 `#7C828D`）、L95（亮 `#6F6B64`）、L44（"沿用既有高保真页取样口径"）。证据：参考稿 L31 `--doc-t3:#6E747E`、L50 `--doc-t3:#969188`。修法：补一句标注（"本稿把 `--doc-t3` 抬高到 4.78/5.00:1 以过 3:1"）或还原参考值。

**L3 · 字号脱刻度**
- 位置：L354(`.stat .k` 10.5px)、L349(`.pv-bar` 11px 合规)、`.pv-toggle` 10.5px、`.pv-line` 11.5px、`.pv-g` 10.5px、`.pv-badge/.bdg` 9.5px。证据：§4.2 刻度 10/11/13/13/15/18px（`ui/theme.slint:453-458`）。修法：9.5→10、10.5→10 或 11、11.5→11 或 13；若为视觉密度刻意，需在说明中标为"派生字号"。

**L4 · 取消按钮圆角与 spec §5.3 D1 的 GhostButton 几何不一致**
- 位置：L411 `.btn{border-radius:var(--r-sm)=4px}` + L415 `.btn.ghost`；L1338（"保留 … radius-sm"）。证据：现状 `ui/confirm_dialog.slint:12` 的危险按钮用 `radius-sm`(4px)，但取消按钮在现状是 `ui/widgets.slint:167-202` 的 `GhostButton`，其 `btn-radius: 6px`（radius-md）；spec §5.3 D1 明写「取消 zh 取消 / en Cancel，GhostButton 现几何 30px / radius-md」。修法：给 `.btn.ghost` 单独 `border-radius:var(--r-md)`，或在 L1338 注明"取消按钮按通用按钮体系统一为 4px（偏离 GhostButton 的 6px）"。

**L5 · 注释里的 `ui/theme.slint` 行号错位**
- 位置：L66（注 `Theme.popup-bg-elevated（:236）`，实为 `popup-bg-alt`）、L67（注 `Theme.popup-bg-inset（:255）`，实为 `popup-bg-elevated`；inset 在 :246）。修法：按 `ui/theme.slint:235/236/246` 更正。

---

## 八、未能验证项（5 条，含原因）

1. **逐帧像素级实渲染核对（软折行时行号列对齐、遮罩下弹框的边界可分性、7 帧渲染与 DOM 的差异）**——部分未能验证。已做：用 `msedge --headless=new` 以 `file:///` 真实渲染并截图（`output/paste-hifi-verify/shot-top.png` 1400×1000、`shot-tall.png` 1400×3400），可证"离线可开、样式生效、无外链、无未样式化闪烁"；**未能做**：①交互态（点击主题/语言/折行/展开/拖动）的真实渲染——headless 只能出单帧快照，本会话未安装 CDP/puppeteer（不擅自改动本机依赖）；②截图经降采样后正文小字（10–11.5px）不可辨，故"折行是否破坏行号对齐"我依据的是 CSS 结构（`--gutter-w:44px` 固定宽 + `text-align:right` + `.pv-g` 独立列）而非像素证据；③`--doc-t3` 等页面壳颜色的渲染观感未做视觉比对。
2. **`Theme.accent-hover` 的精确值**——未能验证到底。`brighter()` 是 Slint 内建（`ui/` 下无实现，未在内建源码中定位其定义），我只能给出两种常见语义的推算区间并据此判定"稿内值与之不等"，无法给出唯一权威值。结论（"不等"）不受影响；"应该是多少"无法定论。
3. **D4 的 16 条危险命令清单本身没有外部权威**——来自研究稿（非 `src/` 事实）。因此 S1 的"`tail` 不该被标为风险"是按该清单判定的；若上游改口认为 `tail` 属 B 级，S1 降级为"缺徽标 + 计数不一致"（仍是缺陷，但不含"误标"）。
4. **designer 是否在 Lead 的复核之后又改过文件**——未能验证。我只持有一个时点（mtime 09:43:14、SHA-256 `C3697CBC…`），无更早的基线哈希可比对；本报告全部结论都锚定在上表的哈希上，若文件在此之后被改动，需按新哈希重跑。
5. **真机 Slint 应用内的弹框几何/遮罩观感**——按 spec §7 明确不在评审范围（不得 `cargo build/test`、不得改 `ui/`、`src/`），故未在 MTerm 内实测；相关结论全部来自源码逐值对账与稿内自述的交叉核对。

---

## 九、附录 · 脚本与原始输出摘要

### A.1 脚手架清单（全部位于 `output/paste-hifi-verify/`）

| 脚本 | 作用 |
|---|---|
| `smoke.js` | 外链资源检测 + 内联 `<script>` 抽取与 `vm.Script` 编译（带行号）+ Proxy DOM stub 运行时 + 全 id 点击 |
| `idcheck.js` | 定义 id ↔ `$('#id')`/`$$()`/`getElementById`/`querySelector(#id)`/`label[for]`/`aria-controls`/`aria-labelledby`/`href="#id"`/`location.hash`/`data-*` 交叉检查 |
| `audit-tokens.js` | 剥 CSS 注释后按括号深度解析 292 条规则 → §4.1 逐值对账 + 全量颜色字面量分类（输出 `audit-tokens-out.txt`） |
| `contrast.js` / `pairs.js` | WCAG 2.1 比值 + alpha 混色；对稿内实际用到的每一对前景/背景复算 |
| `table-audit.js` | 把稿内 §sec-contrast 对账表 19 行 × 2 主题 = 38 格逐格复算比对 |
| `frames.js` / `count-rows.js` | 逐帧抽取统计条/风险行/徽标/按钮/通告；精确统计 `.pv-line`/`is-hidden`/折叠提示/gutter 序列 |

### A.2 对账表 38/38 逐格复算 = 0 处不符（B4/E6 的关键证据）

| # | 落点 | 表暗 | 实算暗 | 表亮 | 实算亮 |
|---|---|---|---|---|---|
| 1 | t1/panel(popup-bg) | 9.38 | 9.38 | 12.37 | 12.37 |
| 2 | t1/root(.dlg-foot) | 9.72 | 9.72 | 11.53 | 11.53 |
| 3 | dg/panel(徽标文字/描边按钮) | 5.32 | 5.32 | 5.22 | 5.22 |
| 4 | dg-ink on 11% dg tint | 4.70 | 4.70 | 6.55 | 6.55 |
| 5 | t1 on 11% dg tint | 8.29 | 8.29 | 10.59 | 10.59 |
| 6 | wr/panel | 8.09 | 8.09 | 4.99 | 4.99 |
| 7 | ok/panel | 6.60 | 6.60 | 5.43 | 5.43 |
| 8 | onSolid/ac | 8.58 | 8.58 | 5.13 | 5.13 |
| 9 | dg/panel(透明底按钮) | 5.32 | 5.32 | 5.22 | 5.22 |
| 10 | onSolid/dg(实心) | 5.51 | 5.51 | 5.22 | 5.22 |
| 11 | t1/tbg(=panel) | 9.38 | 9.38 | 12.37 | 12.37 |
| 12 | ac/tbg(=panel) | 8.29 | 8.29 | 5.13 | 5.13 |
| 13 | t1×.72/panel | 5.46 | 5.46 | 5.22 | 5.22 |
| 14 | t1×.8/palt | 6.30 | 6.30 | 6.04 | 6.04 |
| 15 | t1×.85/palt | 6.93 | 6.93 | 6.96 | 6.96 |
| 16 | t3/panel(行号列) | 3.66 | 3.66 | 3.02 | 3.02 |
| 17 | doc-t1/doc-panel | 15.32 | 15.32 | 12.77 | 12.77 |
| 18 | doc-t2/doc-panel | 7.11 | 7.11 | 4.57 | 4.57 |
| 19 | doc-t3/doc-panel | 4.78 | 4.78 | 5.00 | 5.00 |

→ `table-audit.js`：**「共 19 行 / 38 格，不符 0 格」**（含 designer 自报修正过的 9.72 / 8.09 / 6.60 三个值，独立复算全部命中）。补充实算：暗 `--dg-ink` 压 16% 淡染（hover）= 4.38（仅行号列装饰位）；亮 `--wr-ink` 压 11%/16% `--wr-soft` = 4.34/4.04（仅行号列，L489 注释已限定「仅用于压在 `--wr-soft` 上的行号」）；暗 `popup-bg` 卡 vs 被遮罩背板 = 1.12:1、亮 = 2.62:1（靠 24px 阴影 + hairline 边框分离，与现状 app 同构）。

> **spec 自身勘误（不算稿子缺陷）**：spec:153 写「亮 t1 `#33302C` → panel = 9.72+」，实算 **12.37**（9.72 实为暗 t1 / 暗 root 的值）；`--doc-t1` 亮 `#302E2A` / `--doc-panel` 亮 = 12.77 ✓、`slot-t1` 亮 / panel 亮 = 12.37 ✓，两者都对。方向结论（✅）不变。

### A.3 其余口径的原始结论

- `smoke.js`：`EXTERNAL_FOUND none` / `script_tags=1` / `#0 attrs="" chars=9776` / `SYNTAX #0 OK` / `RUNTIME exec: OK` / `CLICKS tried=44 threw=0` / `ERRORS (0)` / `CONSOLE (0)`。
- `idcheck.js`：`DEFINED IDS (44)`、`REFERENCES (180)`、`dangling = 0`。
- `audit-tokens.js`：`:root` 30 vars、暗块 53 vars、亮块 52 vars、规则 292 条；§4.1 对账 dark 17/17 + light 17/17 = **34/34 PASS**；颜色字面量 distinct=101（`SPEC-EXACT=30 / SLOT+ALPHA=28 / BLACK-WHITE-ALPHA=7 / BLACK-WHITE=1 / UNKNOWN=35`）。
  - UNKNOWN 中已裁定为**误报**的：`#339922`/`#226622` 系正文 issue 号 `#392`/`#262` 被 3 位 hex 正则误抓；`#ec6a6a`/`#c44a4a` 出现在 L419 的 CSS 注释里。
  - 已裁定为**合规**的：`--dg-ink`（唯一例外，§4.4 第 8 条）、`--on-solid`、`--wr-ink`、`--scrim`、`--shadow`/`--pop-shadow`、九项页面壳 `--doc-*`/`--page`/`--stage`。
  - 详见正文 B3/B4 与缺陷 M2/M3。
- Lead 工具交叉验证：`node .tmp-lead/smoke.mjs docs/design/multiline-paste-hifi-2026-11-06-v1.html .tmp-lead/verifier-smoke.json` → `verdict=PASS`，与我的 `smoke.js`/`idcheck.js` 结论一致（`undefinedVars` 该项在本稿为空，比参考稿基线更干净）。**两套口径无冲突。**

---

# v1.1 复审（task-5 · 2026-11-06 10:34 起）

> 本节**只追加**，不改写上面 v1 的任何结论。被审对象仍是同一文件，但内容已被 designer 按 task-4 修订。

## 0. 复审基线（我独立实测）

| 项目 | v1 | v1.1 |
|---|---|---|
| 被审件 SHA-256 | `C3697CBC61354D4C2C9677589940EA9747FDCCF677E138559295AD859A3B96E0` | `4419564AA3A0B8C9EB59F1FE9089662D10E928851326D9194515430269BF6776` |
| 字节数 | 163895 | **173165** |
| 行数（`(Get-Content $f).Count`） | 1597 | **1575** |
| mtime | 2026-11-06 09:43:14 | 2026-11-06 10:31:04 |
| 内联脚本字符数 | 9776 | **10653** |

- **行数口径说明**：本节一律用「逐行读取」口径 = **1575**，与 designer 自报一致；Lead 的 `Measure-Object -Line` 得 1547，属该 cmdlet 对 CRLF/末尾空行的计数差异，**不是稿子问题**。Lead 工具 `node .tmp-lead\smoke.mjs` 自报 `lines: 1575`，与我的`(Get-Content).Count` 一致，可判定 1547 是 Measure-Object 口径。
- 本轮改动范围已核对：`ui/theme.slint`、`ui/widgets.slint`、`ui/app.slint`、`src/**` 未参与本次修订；本节引用的所有权威行号均已**重新读取当前文件**后引用（下述 M1/L5 的行号在 v1 报告中曾偏差约 5 行，本轮已按重读结果更正）。

## 1. 结论摘要

| 维度 | 结果 |
|---|---|
| v1 的 13 条缺陷 | **closed 11 / partial 1 / open 1** |
| 回归检查（A 组 + token + 对账表 + §6 七条 + 实渲染） | **13 项 13 pass / 0 fail** |
| 新增缺陷 | **3 条**：N1（中）、N2（低）、N3（低） |
| 判定 | **仍不通过** |

**一句话判定**：**仍不通过——但性质已从「设计缺陷」变成「收尾项」**。v1 的全部实质问题（S1 风险口径、S2 阈值口径、H1 焦点、H2 折叠账、H3 现状文案）本轮都在 DOM/脚本/权威源码层面**实测闭合**，token 与对比度对账表零回归；剩余 3 项为低severity 残留（L1 一句注释、L4 取消键圆角）与 1 条**由 M3 修法引入的中等新缺陷 N1（亮色主按钮 hover 文字 4.26:1）**。修掉 N1 + L4 + L1 残留即可交稿。

## 2. v1 缺陷逐条裁定

| 编号 | v1 结论 | v1.1 裁定 | 本轮新证据（位置 / 命令输出） |
|---|---|---|---|
| **S1** st-b 风险口径三处互斥 | 严重 | **closed** | L739 框顶「本段含 **3 处危险命令**（A 级 2 · B 级 1）」；L753 统计「风险 **3**」；`count-rows.js`：st-b `dg=2 wr=1 徽标=3`，风险行 = L762 `rm -rf /var/tmp/cache/*`(A) / L763 `sudo systemctl restart nginx`(B) / L766 `:> /var/log/nginx/error.log`(A)。v1 的 `tail -n 20 …access.log` 误标行已消失（反向检索 `.pv-line wr` 在 st-b 内只剩 `sudo` 一行）。三处口径一致，**且 `tail` 不再被标**。 |
| **S2** 档位阈值互斥 | 严重 | **closed** | 四处表述全部改为自洽写法并引 `src/terminal/impls/input.rs:93-104`：L507-508（hero「紧凑 1–3 行 **且** ≤600 / 标准 4–12 行 **且** ≤600 / 评审 >12 行 **或** >600」）、L548（G5 同口径）、L709（st-b `.fd`「4–12 行 **且** ≤600 字符…>12 行 **或** >600 字符才升级」）、L794（st-c `.fd`「>12 行 **或** >600 字符任一超出」）、L1302（连续性区「标准档 640px（4–12 行 **且** ≤600 字符）」）。窄模式重查 `4–12 行或 >600 字符` → **0 命中**。st-b 取 12 行/364 字符与「标准档」自洽（Lead 裁定 a 采纳）。 |
| **H1**（本轮亦称 S3）焦点只声明未实现 | 高 | **closed** | 脚本内已实现：L1426 `focusTerm(frame)`、L1437 `focusPrimary(frame)`、L1570-1571 `focusPrimary($$('.frame').filter(f=>!f.hasAttribute('data-closed'))[0])`（注释「初始化焦点：首个未关闭状态帧的主按钮（A 帧另有 autofocus 作静态兜底）」）；标记 L697 `<button class="btn primary is-default" autofocus tabindex="0" data-do="paste">`；关闭路径 L1485-1486 trace「已取消：未发送任何字节，终端焦点交还（terminal-focus-sequence + 1）」。`smoke.js` v1.1 `CLICKS tried=44 threw=0` 证明这些函数在缺元素时有守卫、不抛错。残留表示层问题见 **N3**。 |
| **H2** st-b 折叠账差 1 行 | 高 | **closed** | 独立复算三帧账目全部相等：st-b is-hidden=**4**（gutter 2,3,11,12）＝ 提示 2+2 ＝ 按钮数；gutter 序列 **1..12 连续无缺号**；st-e is-hidden=**20**（gutter 2..18 = 17 行 ＋ 22,23,24 = 3 行）＝ 提示「以上另有 17 行」+「以下另有 3 行」＝ refs `20`；st-f is-hidden=**4**（gutter 9,10,11,12）＝ 提示「以下另有 4 行」＝ refs `4`。稿内亦自述该等式（L1045 / L1131）。`data-fold-hidden` 标记计数：st-b 4 + st-e 20 + st-f 4 = 28，与 `is-hidden` 计数逐一对应；脚本 L1385 选择器 `[data-fold-hidden]:not([data-fold])` 能切到这些行。 |
| **H3** 现状区 message/按钮文案不符 | 高 | **closed** | L1279 message 逐字 = `ui/app.slint:8212`（zh「剪贴板内容包含换行，粘贴后可能连续执行多个命令，请确认内容后再继续。」/ en 同源）；L1287 `legacy-btn` 文案 zh「粘贴」/ en「Paste」= `ui/app.slint:8217` `confirm-label`；L1281 注释显式引用 `ui/app.slint:8212-8217`。v1 指的「确认后立即粘贴」/「确认」已不存在。 |
| **M1** `--scrim` 未取 `Theme.modal-scrim` | 中 | **closed** | L75 `--scrim:rgba(12,13,17,.32)`、L89 亮 `--scrim:rgba(244,245,248,.32)`；L74 注释「Scrim = Theme.modal-scrim：scrim-base（暗 #0c0d11 / 亮 #f4f5f8）× scrim-strength 0.32（`ui/theme.slint:374 / :348`；渲染 `ui/widgets.slint:804-815`）」。**我重读权威源核对：`ui/theme.slint:348 in-out property <float> scrim-strength: 0.32;`、`:374 property <color> scrim-base: root.dark ? #0c0d11 : #f4f5f8;` → 引用行号与取值全部正确**。v1 中「亮色方向与现状相反」的自造 42% 深纱已废止。副作用量化见 §4.N4。 |
| **M2** §4.2 派生 token 3 处取值不符 | 中 | **closed** | L51 暗 `--popup-bg-inset:#161619` / L81 亮 `#F2F0EC`；L54 暗 `--field-2:#1D2027` / L82 亮 `#EEF0F4`；L57 暗 `--danger-strong:#dc4b44` / L82 亮 `#dc4b44`。均与 `ui/theme.slint:251 / 285 / 335` 逐值一致（注释行号亦正确）。三者仍是 **0 引用**死声明（`var(--x)` 计数 = 0），稿子已在 L549 披露该状态，**无视觉影响**。 |
| **M3** `--ac-hov` 与 `Theme.accent-hover` 不符且亮色更暗 | 中 | **closed（含新副作用 N1）** | L70 暗 `--ac-hov:#25DDC8` / L87 亮 `#238478`，与 Lead 裁定值一致；亮度方向已修正：暗 L 0.45→**0.56**（更亮 ✓）、亮 L 0.14→**0.18**（更亮 ✓）。但引入新缺陷 → **N1**。 |
| **L1** 把 t2 的比值挂到 t1 名下 | 低 | **partial** | 已修：L281「注：4.14 是 slot-t2 压 slot-palt、4.00 是 slot-t2 压 slot-elev；slot-t1 压它们实为 9.08 / 8.78」、L1240「slot-t2 压 slot-palt / slot-elev 只有 4.14 / 4.00:1（slot-t1 压同两个面实为 9.08 / 8.78，可用）」。**残留 1 处**：L253 仍写「popup-bg-elevated 在暗色是 #1E2024，**t1** 压上去只有 **4.00** ❌」——同一注释块 L252 引用的 9.72/11.53 是 t1→root，L253 却把 t2 的 4.00 记到 t1 头上。 |
| **L2** `--doc-t3` 偏离取样源未标注 | 低 | **closed** | L44 注释已写明「唯一偏离取样源的是 --doc-t3：源值暗 #6E747E 压 doc-panel 3.99:1、亮 #969188 2.83:1 均 <3:1，故各上调一档（暗 → #7C828D / 亮 → #6F6B64），刻意偏离取样源而非笔误」。数字本身有误 → **N2**。 |
| **L3** 字号脱 §4.2 刻度 | 低 | **closed** | 弹窗内字号已全部落回刻度：`.pv-toggle` 11px、`.pv-line` 11px、`.pv-g` 11px、`.pv-badge` 10px、`.pv-fold` 11px（v1 的 9.5 / 10.5 / 11.5 已消失）。页面壳的 12/12.5/13.5px 属文档 chrome，与参考稿同构，不在 §4.2 约束面。 |
| **L4** 取消键圆角 4px（应为 GhostButton 的 radius-md 6px） | 低 | **open** | 全稿 `.btn{…}`（L348-350）仍是 `border-radius:var(--r-sm)`（=4px），`.btn.ghost`（L353）**无 radius 覆盖**；`border-radius:var(--r-md)` 在全文只出现于 `.brand .logo`(L103)、`.idx a`(L171)、`.dlg-head .ic`(L237)、`.pv`(L278)，**`.btn*` 一处都没有**。与 §5.3 D1「取消 = GhostButton 现几何 30px / radius-md」不符。**最小修复**：加 `.btn.ghost{border-radius:var(--r-md)}`（或把 `.btn` 统一改 r-md 并复核 dg-out/primary 的视觉）。 |
| **L5** 注释里 theme.slint 行号错位 | 低 | **closed** | 现引 `:251`（popup-bg-inset）、`:285`（field-2）、`:335`（danger-strong）、`:374`（scrim-base）、`:348`（scrim-strength）、`ui/widgets.slint:804-815` —— 逐条重读权威源，**全部命中**。 |

## 3. 回归检查表（13 项，全部 pass）

| # | 检查项 | 结果 | 证据（原始输出摘要） |
|---|---|---|---|
| R1 | 内联脚本语法 | **pass** | `SYNTAX #0 OK`；`vm.Script` 编译 1 段 / 10653 字符（v1 9776） |
| R2 | 运行时初始化 | **pass** | `RUNTIME exec: OK`；`ERRORS (0)`；`CONSOLE (0)`；全元素点击 `tried=44 threw=0` |
| R3 | 外链资源 | **pass** | `EXTERNAL_FOUND none`（link/img/script[src]/iframe/@import/url(http)/fetch/XHR 全 0） |
| R4 | id 悬空 | **pass** | `defined_ids=44 refs=180 dangling=0`（引用全为 `href="#symbol"` 锚点） |
| R5 | Lead 工具交叉验证 | **pass** | `node .tmp-lead\smoke.mjs … .tmp-lead\verifier-smoke.json` → `verdict=PASS`、`bytes=173165 lines=1575 ids=44 dangling=0 undefinedVars=0` |
| R6 | §4.1 token 逐值 | **pass** | 34/34（dark 17/17、light 17/17），`fail=0 absent=0` |
| R7 | §4.2 派生 token | **pass** | field/line-2 EXACT；field-2 / danger-strong / popup-bg-inset 本轮已对齐（见 M2） |
| R8 | 全量颜色字面量 | **pass** | distinct=104；新增字面量全部对上修复值（`#1d2027 #eef0f4 #dc4b44 #25ddc8 #238478 #0c0d11`），**未出现新的非 §4.1 色相**；唯一例外仍是 §4.4 第 8 条的 `--dg-ink` |
| R9 | 稿内对比度对账表 | **pass** | 20 个 `<tr>`（表头 + **19 行**）×2 主题 = 38 格，实算**逐格相符 0 处不符**（含 9.72 / 8.09 / 6.60 三个 designer 自修值） |
| R10 | §6① 单文件/离线/主题切换 | **pass** | 外链 0；`data-theme` 由 L2 根元素 + 两条 CSS 规则驱动；实渲染截图非白屏（见 §5） |
| R11 | §6② 状态覆盖 | **pass** | 7 帧：st-a 3/186/0、st-b 12/364/3、st-c 18/1146/4、st-d 6/204/0(亮)、st-e 24/881/2(亮)、st-f 8412/104300/3、st-g 1/71/1(亮) |
| R12 | §6③④⑤ 结构/追溯/连续性 | **pass** | 页壳 + 模拟器两级结构完整；`slot-*` 标注 273 处；连续性区 L1278-1287 复刻现状组件并与新档并列 |
| R13 | §6⑥⑦ 无报错 / zh-en 双份 | **pass** | 见 R1-R2；`span.zh`=118 / `span.en`=118 逐处配对 |

## 4. v1.1 新增缺陷

| 编号 | 严重度 | 位置 | 现象与证据 | 为什么算问题 | 最小修复 |
|---|---|---|---|---|---|
| **N1** | **中** | `.btn.primary:hover`（L382-383 附近）＋ L87 `--ac-hov:#238478` ＋ L98 附近 `--on-solid` 亮 = `#FAF8F4` | 实算 **`#FAF8F4` on `#238478` = 4.26:1 < 4.5**（暗色同规则 = 10.54:1 ✓）；静态态 5.13:1 ✓。即**亮色主题下主按钮 hover 时文字跌破正文阈值**。 | §4.4 门槛：正文 ≥4.5:1（13px 非大字，不适用 3:1 放宽）。该值是本轮 M3 修法（把亮色 ac-hov 从「更暗」改成「更亮」）**新引入**的，v1 不存在。 | 二选一：①hover 时保留 `--slot-ac` 做底、只用 `--ac-hov` 做描边/内阴影（文字对比度维持 5.13:1）；②hover 底继续用 `#238478`，但把文字换成 `slot-root` 亮 `#161619`/或压到 `slot-t1` 亮 `#33302C`（实算 ≈ 4.9:1，需复算确认）。推荐 ①，改动最小且不引入新色值。 |
| **N2** | 低 | L44 注释 | 注释内联写「上调一档（暗 → #7C828D **4.85** ✓ / 亮 → #6F6B64 **4.98** ✓）」，但① 按 WCAG 2.1 实算 `#7C828D`/`--doc-panel #121417` = **4.78**、`#6F6B64`/`--doc-panel #FAF8F4` = **5.00**；② 稿内自己的对账表第 19 行写的就是 **4.78 / 5.00**。同一份稿子里两个数不一致。 | 「可追溯到实测值」是 §6④/§4.4-⑥ 的硬要求，注释与表格自相矛盾会削弱可复核性（与 v1 的 L1 同类）。 | 把 L44 的 4.85/4.98 改成 4.78/5.00（与表一致）；若坚持 4.85/4.98，须给出其计算背景（哪一对前景/背景）。 |
| **N3** | 低 | L568 文案 vs 帧内 DOM | L568 声明 Tab 序为「上下文行 → **预览区** → **折行开关** → 取消 → 主按钮 → 粘贴并执行 → 不再提示」；帧内 DOM 实际为 L672 `.dlg-close`（标题栏 ✕）→ L684 `.pv-toggle`（折行开关）→ L685 `.pv-scroll`（预览区）→ L696 取消 → L697 主按钮 → dg-out → L699 复选框。 | ① 首个 Tab 落点是标题栏 ✕，稿子写的「上下文行」没有任何可聚焦元素；② 折行开关在 DOM 中**先于**预览区，与 L568 的列法相反。**注意**：视觉上折行开关确实位于预览区之上（`.pv-bar` 在 `.pv-scroll` 之前），所以「Tab 序 = 视觉序」这条要求本身是**满足**的，问题只在文档表述与首落点描述。 | 把 L568 改为「关闭 ✕ → 折行开关 → 预览区 → 取消 → 主按钮 → 粘贴并执行 → 不再提示」，去掉不存在的「上下文行」；或把上下文行做成 `tabindex="0"` 的可聚焦元素（不推荐，会多一个无操作停点）。 |

**观察项（不计缺陷）**
- **N4 遮罩下的弹框分离度**：按权威语义（暗 `rgba(12,13,17,.32)` / 亮 `rgba(244,245,248,.32)`）实算，卡片 vs 被纱背板 **暗 1.04:1 / 亮 1.01:1**（背板：暗 `#141619`、亮 `#F8F7F5`），卡片边框 vs 背板 暗 2.28:1 / 亮 1.47:1。即**弹框与身后的分离几乎完全依赖 24px 投影和 hairline 边框**，亮色尤其弱。这是 `Theme.modal-scrim` 的**权威取值**（`scrim-strength: 0.32`）导致，不是本稿引入 → 按 Lead 口径 d 记观察项，不计稿子缺陷；但请 Lead 知悉：若真机观感发闷，问题在 `ui/theme.slint` 的 `scrim-strength`，需另行决策。
- **设计者自报未跑脚本的补齐**：`contrast.js` / `pairs.js` 内置了 v1 的色值与行号（`pairs.js` 仍是 `--ac-hov #25D8C4`、亮 scrim 0.42），直接跑会输出 v1 结论，**本轮已改用新写的 `v11-new.js`（按 v1.1 实际取值）复算**，上表 N1/N4 与 R8 的数据均出自它；`smoke.js` / `idcheck.js` / `audit-tokens.js` / `table-audit.js` / `frames.js` / `count-rows.js` 均已对 v1.1 复跑。

## 5. 未能验证项（v1.1）

1. **逐帧像素级/交互态实渲染**：仅做了 3 张 headless 单帧截图（`shot-v11-top.png` 236743 B = 页壳；`shot-probe-ste.png` 230323 B = 亮色 st-e 帧，经同源 iframe + `scrollIntoView` 定位）。带 `#fragment` 的 3 张（`shot-v11-st-b/-e/-g.png`，均 6365 B）是 **headless 对 fragment 导航的竞态导致的空白图**（工具现象，非稿子缺陷，已弃用）。因此「浅纱遮罩下弹框的可读性/观感」只有量化分离度（N4），**没有人眼/像素级评估**；hover/焦点/拖动等交互态未做实渲染。
2. **`Theme.accent-hover` 的精确值**：`brighter()` 在仓库内无实现（Slint 内建），只能验证「稿值与两种 brighter 推算都不同、且方向正确（更亮）」，无法断言「应为多少」。N1 的修法①可绕开该不确定性。
3. **D4 的 16 条危险命令清单无外部权威**：st-f 的 `partprobe /dev/nvme0n1`、`mount -o noatime /dev/nvme0n1p2 /srv/data` 仍未标记（不在清单内），是否应升级为危险行属清单边界问题，无权威可依。
4. **F 帧「8 400 行虚拟化」是静态示意**（用 `.pv-line.note` 通告行表达，未做按需渲染）——按 Lead 裁定 b 接受，判 pass，但「真实虚拟化」未验证。
5. **真机 Slint 行为**（`terminal-focus-sequence` 实际焦点序列、bracketed paste 文案分支在真机的表现）不在 spec §7 评审范围。

## 6. 交稿结论

- **仍不通过**，剩余 **5 项**：N1（中，必修）、L4（open，低）、L1 残留 1 句（partial，低）、N2（低）、N3（低）。全部闭环后即可交稿；除 N1 需要一次取色决策外，其余均为文案/一行 CSS 级修改。
- 与 v1 相比的实质变化：**设计层面的对抗性复核已无未决问题**——风险口径自洽、档位口径与 `src/terminal/impls/input.rs:93-104` 对齐、折叠账三帧自洽、焦点管理真实实现、现状文案逐字对齐权威源码、token 与对比度对账 0 回归。剩余问题不涉及信息架构或安全语义。
- 建议交稿门槛：N1 修（或 Lead 明确裁定接受 4.26:1）+ L4 一行 CSS + L1/L2/N3 三处文案。

# v1.2 最终确认（task-5 · 2026-11-06 10:47 起）

## 0. 基线

- 被审件 `docs/design/multiline-paste-hifi-2026-11-06-v1.html`：**174305 字节 / 1581 行**（逐行口径）/ mtime 2026-11-06 10:46:54，**SHA-256 `61E3692EC60268F5E85D9893EE8C9B4D24D39B3BA389F56FB9BB747471017168`**（评审前后复核一致）。
- 与 v1.1 基线（173165 B / 1575 行、`4419564A…F6776`）差 **+1140 字节 / +6 行**，与 designer 自报的 5 处改动量级相符；帧内容零变动（逐帧复跑与 v1.1 完全一致：st-a 3/186/0、st-b 12/364/3、st-c 18/1146/4、st-d 6/204/0、st-e 24/881/2、st-f 8412/104300/3、st-g 1/71/1），说明改动确实只落在 CSS/文档层。

## 1. 结论摘要

**最终判定：passed —— v1.2 通过，可交稿。**

| 本轮待判 | 判定 | 关键证据 |
|---|---|---|
| N1 主按钮 hover 对比度 | **closed** | L368-370；`#161619`/`#1FC8B4`=8.58:1、`#FAF8F4`/`#1F766B`=5.13:1；`background:var(--ac-hov)`=0、`border-color:var(--ac-hov)`=1 |
| L4 取消按钮圆角 | **closed** | L356 `.btn.ghost{border-radius:var(--r-md)}`；`.btn.ghost` 仅 L353/356/357 三条，级联正确 |
| L1 残留句 | **closed** | L253 主语已改 `slot-t2`，与 L281/L1240 一致 |
| N2 `--doc-t3` 比值 | **closed** | L44 + L90 均为 4.78 / 5.00 并指「对账表第 19 行」，与实算及表内第 19 行三处一致 |
| N3 Tab 序表述 | **closed** | L573 改为真实 DOM 序，与 st-b 帧 DOM（L678→690→691→702→703→dg-out→705）逐项一致 |

- 累计：v1 的 13 条缺陷 **13/13 closed**；v1.1 新增 N1–N3 **3/3 closed**；**累计 16/16 closed**。
- 回归：A 组 5/5、§4.1 34/34、对账表 38/38 格 0 不符、§6 七条 7/7、悬空 id 0、外链 0、44 点击 0 抛错。
- 本轮新增 **N5（低·建议项，非阻断）**：hover 反馈由「底色变化」降为「1px 描边变化」。**spec §6 / §4.4 / D1–D5 均未规定 hover 反馈强度**，且 hover 描边对底色 暗 10.18:1 / 亮 4.26:1 均 ≥3:1（WCAG 1.4.11）→ **不计 spec 违约**，仅附一行 CSS 可选补法（见 §4）。
- 历史观察项 **N4** 不变（`Theme.modal-scrim` 0.32 下卡片 vs 背板 暗 1.04:1 / 亮 1.01:1，分离依赖 24px 投影 + hairline；根因在 `ui/theme.slint`，非本稿缺陷）。

## 2. 本轮 5 处改动逐条验证

### N1 — closed

- 证据：**L368-369** 注释「hover 只变描边、不变底色：`--ac-hov` 亮色是 `#238478`，若作底则 `#FAF8F4` 字只有 4.26:1 ❌，故 hover/active 底色一律保持 `slot-ac`（亮 5.13 ✓ / 暗 8.58 ✓），`--ac-hov` 仅用于 `border-color`。」**L370** `.btn.primary:hover{background:var(--slot-ac);border-color:var(--ac-hov);color:var(--on-solid)}`；**L371-372** `.btn.primary:active{background:var(--slot-ac);border-color:var(--slot-ac);box-shadow:inset 0 2px 5px rgba(0,0,0,.22)}`（按下态未动，仍有内阴影反馈）。**L591** 决策块补说明段。
- 独立复算（`output/paste-hifi-verify/hover-v12.js`，本轮新写）：`#161619` on `#1FC8B4` = **8.58:1** ✓；`#FAF8F4` on `#1F766B` = **5.13:1** ✓；旧写法 `#FAF8F4` on `#238478` = **4.26:1**（已不再作底色）。
- 全稿计数与 Lead 口径一致：`background:var(--ac-hov)` = **0**；`border-color:var(--ac-hov)` = **1**（仅 L370）；`--ac-hov` 其余出现仅 L70 / L87 两处 token 声明 + L368-369 注释 + L591 说明 + L1253 派生说明。

### L4 — closed

- 证据：**L349** `.btn{…border-radius:var(--r-sm);…}`（基类 4px）；**L353** `.btn.ghost{background:transparent;border-color:var(--slot-line);color:var(--slot-t1)}`（**不含 radius**）；**L356** `.btn.ghost{border-radius:var(--r-md)}`；**L357** `.btn.ghost:hover{…}`（不含 radius）。
- 级联核验：`.btn.ghost` 全稿**仅** L353/L356/L357 三处（无第四条覆盖）；L356 与 L353 同特异度（0,2,0）且源序更晚 → radius 判为 **6px**，L353 的 background/border-color/color 继续生效，**两条规则属性不重叠、无互相覆盖**。`.btn.dg-out`（L361）与 `.btn.primary`（L367）均未声明 radius → 继承基类 **4px**（符合 L355 注释「危险按钮保持 `r-sm`（DangerButton 语义，不改）」）。
- 落点核验：`.btn.ghost` = 7 个帧内的「取消」按钮（L702 / 786 / 876 / 948 / 1044 / 1130 / …）+ 连续性区现状 mock 的「取消」。前者对应 §5.3 D1「GhostButton 现几何 30px / radius-md」，后者对应真机 `ui/widgets.slint:167-202` 的 `btn-radius: 6px` → **两处均正确**。

### L1 — closed（v1.1 残留句已除）

- **L253** 现为「`popup-bg-elevated` 在暗色是 `#1E2024`，**`slot-t2`** 压上去只有 4.00 ❌（spec §4.4 表格已注明）」。与 **L281**（「4.14 是 `slot-t2` 压 `slot-palt`、4.00 是 `slot-t2` 压 `slot-elev`；`slot-t1` 压它们实为 9.08 / 8.78」）与 **L1240** 口径一致，全稿不再有把 t2 比值挂在 t1 名下的表述。

### N2 — closed

- **L44**：「…（暗 → `#7C828D` **4.78** ✓ / 亮 → `#6F6B64` **5.00** ✓，见对账表第 19 行）…」；**L90**：「…亮色 `--doc-t3` 同暗色上调，取样源 `#969188` 仅 2.83:1 → 取 `#6F6B64` = **5.00:1** ✓（见对账表第 19 行）…」。
- 实测复核（`table-audit.js` 第 19 行）：`#7C828D` / `--doc-panel #121417` = **4.78**；`#6F6B64` / `#FAF8F4` = **5.00** → 注释、实算、表内第 19 行**三处一致**。

### N3 — closed

- **L573** 现为「**Tab / Shift+Tab** = 视觉顺序 = DOM 顺序：关闭 ✕ → 折行开关 → 预览区（`tabindex="0"`，可聚焦滚动）→ 取消 → 主按钮 → 粘贴并执行 → 不再提示多行粘贴。」「上下文行」已删、也未给它加 `tabindex`。
- DOM 序核验（st-b 帧）：**L678** `.dlg-close`（✕，`tabindex="0"`）→ **L690** `.pv-toggle`（折行开关）→ **L691** `.pv-scroll`（预览区，`tabindex="0"`）→ **L702** `.btn.ghost`（取消）→ **L703** `.btn.primary is-default`（主按钮，`autofocus`）→ `.btn.dg-out`（粘贴并执行，原生 `<button>` 无需 `tabindex` 即在 Tab 序内）→ **L705** `.opt`（不再提示，`role="checkbox" tabindex="0"`）→ **与 L573 表述逐项一致**；竖向视觉顺序亦相同（✕ 在卡头、折行开关在预览区上方、页脚在预览区下方、复选行在页脚下方）→ **「视觉序 = DOM 序 = Tab 序」成立**。

### 追加核实：designer 自报的「表内第 8 行 / HTML 第 1226 行」指针

- 对账表 `tbody` 自 **L1219** 起：第 1 行 L1219（`slot-t1`/panel）、第 2 行 L1220（`slot-t1`/root）、第 3 行 L1221（徽标 `slot-dg`）、第 4 行 L1222（`--dg-ink`）、第 5 行 L1223（危险行行内正文）、第 6 行 L1224（`slot-wr`）、第 7 行 L1225（`slot-ok`）、**第 8 行 L1226 = 「主按钮 `.btn.primary`（accent 实底）」`8.58:1` / `5.13:1`** ✓ —— 指针**正确**，且正是 N1 说明所引的落点（`tbody` 共 19 个 `<tr>`）。

## 3. 回归复跑（v1.2）

| # | 项 | 命令/工具 | 结果 |
|---|---|---|---|
| R1 | 内联脚本语法 | `smoke.js` | `SYNTAX #0 OK`（1 段 / `chars=10653`） |
| R2 | DOM stub 运行时 | `smoke.js` | `RUNTIME exec: OK`、`CLICKS tried=44 threw=0`、`ERRORS (0)`、`CONSOLE (0)` |
| R3 | 外链资源 | `smoke.js` | `EXTERNAL_FOUND none` |
| R4 | id 悬空 | `idcheck.js` | `defined_ids=44 refs=180 dangling=0` |
| R5 | Lead 工具交叉 | `.tmp-lead\smoke.mjs` | `verdict=PASS`（bytes=174305 / lines=1581 / ids=44 / dangling=0 / undefinedVars=0） |
| R6 | §4.1 色板逐值 | `audit-tokens.js` | **`pass=34 fail=0 absent=0`**（PASS 行 35 含总计行 / FAIL 0 / ABSENT 0） |
| R7 | 颜色字面量分类 | `audit-tokens.js` | UNKNOWN 41 / SPEC-EXACT 30 / SLOT+ALPHA 26 / BLACK-WHITE-ALPHA 6 / BLACK-WHITE 1；**无新色相**（唯一例外仍为 §4.4 第 8 条 `--dg-ink`） |
| R8 | 稿内对账表 | `table-audit.js` | 19 行 × 2 主题 = **38 格，不符 0 格**（含 9.72 / 8.09 / 6.60） |
| R9 | 七帧数据 + 折叠账 | `count-rows.js` | 与 v1.1 逐项一致；st-b is-hidden 4 = 提示 2+2 = 按钮「另有 4 行」（L767 / L777 / L779）；st-e 20 = 17+3；st-f 4；风险 dg/wr/徽标 = 2/1/3、2/2/4、1/1/2 … |
| R10 | §6 七条验收 | 结构 grep + 实渲染 | 7/7 仍过（单文件离线、6+ 状态、两级结构、`slot-*` 标注、连续性、无 JS 报错、zh/en 各 118 处） |
| R11 | 白字实心 danger 用法 | grep | 全稿 **1** 处 = **L157 `.legacy-btn`**，位于**现状对照卡**（忠实复刻 `ui/confirm_dialog.slint:7-31` DangerButton = 稿内 L601 自陈的 3.28 ❌ 现状）；新设计三档全为描边式 / `--on-solid` 深色字 → **不违反 §4.4 第 5 条** |

- **工具口径备注**：`frames.js` 本轮报「折叠隐藏行 = 0 / 折叠提示 = []」，系该脚本**未解析 `.is-hidden` / `.fold`** 的已知局限（v1.1 已记录），折叠账以 `count-rows.js` + 行号 grep 为准，两者一致。

## 4. 新问题扫描

### N5（低 · 建议项，非阻断）— hover 反馈强度弱化

- **位置**：L367（静态）vs L370（hover）；唯一差异 = `border-color`。
- **量化**：描边 暗 `#1FC8B4`(L=45.3%) → `#25DDC8`(L=50.6%)，**ΔL = +5.3pp**；亮 `#1F766B`(L=29.2%) → `#238478`(L=32.7%)，**ΔL = +3.5pp**。有 `transition:background,border-color,color .12s`（L350）故有过渡；`:active` 另有 inset 阴影（L372）。
- **判定**：**可感知但偏弱**；且亮色下描边与卡片底的可辨度由 5.13 → 4.26（方向与暗色的 8.29 → 10.18 相反）。
- **为何不计缺陷**：spec §6 七条、§4.4 八条、§5.3 D1–D5 **均未规定 hover 的反馈强度**；hover 描边属非文本 UI 边界，暗 10.18:1 / 亮 4.26:1 均 ≥3:1（WCAG 1.4.11 非文本门槛）。这是「反馈强度」而非「合规性」问题。
- **最小补法（不改任何色值、零布局偏移，任选其一）**：① `.btn.primary:hover{transform:translateY(-1px)}`（纯几何提升，`:active` 回 0）；② `box-shadow:0 0 0 1px var(--ac-hov)`（等效 2px 描边；**需同步**把 L591「`--ac-hov` 只用于 `border-color`」改为「border-color / 描边环」）；③ 用既有 `--ac-soft` 做外环 `box-shadow:0 0 0 2px var(--ac-soft)`（不动 L591 措辞）。建议 ① 或 ③。**是否补由 Lead 决定，不影响本轮判定。**

### 其余扫描项

- **`.btn.ghost` 分两条规则**：级联正确、无副作用（见 §2 L4）；唯一「顺带」影响是连续性区现状 mock 的取消键也变 6px —— 这正是真机 GhostButton 取值，属**正确复刻**。
- **Tab 序**：闭合（见 §2 N3）；新增 `focus()` 未引起运行时回归（`CLICKS 44 threw=0`、`ERRORS 0 / CONSOLE 0`、`undefinedVars=[]`）。
- **浅纱遮罩下亮色帧**：N4 的量化分离度本轮无变化；除已记录的截图外未做新的像素级/人眼评估（见 §7）。

## 5. 累计计数（最终）

| 批次 | 条数 | 判定 |
|---|---|---|
| v1（S1 S2 H1 H2 H3 M1 M2 M3 L1 L2 L3 L4 L5） | 13 | **closed 13 / partial 0 / open 0** |
| v1.1 新增（N1 N2 N3） | 3 | **closed 3 / open 0** |
| **累计** | **16** | **16/16 closed** |
| v1.2 新增（N5） | 1 | 建议项（非 spec 违约），不计缺陷 |
| 历史观察项（N4） | 1 | 观察项（根因在 `ui/theme.slint`），不计缺陷 |

回归合计：**A 组 5/5、§4.1 34/34、对账表 38/38 格、§6 七条 7/7、§4.2 派生 token 3/3 对齐、无新色相、悬空 id 0、外链 0、44 点击 0 抛错。**

## 6. 可直接引用的结论段（交付说明用）

> **v1.2 通过，可交稿。** 独立复核（verifier，task-5 最终确认轮）针对被审件 `docs/design/multiline-paste-hifi-2026-11-06-v1.html`（174305 字节 / 1581 行，SHA-256 `61E3692EC60268F5E85D9893EE8C9B4D24D39B3BA389F56FB9BB747471017168`，评审前后哈希一致）逐条验证了本轮 5 处改动并完成全量回归：**N1**（主按钮 hover 底色改回 `slot-ac`、`--ac-hov` 仅作描边；`#161619`/`#1FC8B4`=8.58:1、`#FAF8F4`/`#1F766B`=5.13:1，`background:var(--ac-hov)`=0）、**L4**（L356 新增 `.btn.ghost{border-radius:var(--r-md)}`，级联正确、危险按钮仍 4px）、**L1**（L253 主语改为 `slot-t2`）、**N2**（L44/L90 改为 4.78/5.00 并指对账表第 19 行）、**N3**（L573 改为真实 DOM 序，与帧内 DOM 逐项一致）**全部 closed**；v1 的 13 条缺陷与 v1.1 的 N1–N3 **累计 16/16 closed**。回归**零失败**：内联脚本语法/运行时 OK、44 个元素点击 0 抛错、`ERRORS 0 / CONSOLE 0`、外链 0、悬空 id 0、§4.1 色板 **34/34 逐值相等**、稿内 19 行 × 2 主题 = **38 格对账 0 处不符**（含 9.72 / 8.09 / 6.60）、§6 七条验收 7/7、七帧折叠账与风险计数自洽（st-b 4=2+2、st-e 20=17+3、st-f 4）、主按钮文字对比度 暗 8.58:1 / 亮 5.13:1。唯一遗留为**非阻断建议项 N5**：hover 反馈由「底色变化」降为「1px 描边变化」（ΔL 暗 +5.3pp / 亮 +3.5pp，可感知但偏弱），**spec 无对应条款**、非文本描边暗 10.18:1 / 亮 4.26:1 ≥ 3:1 合规，可选补一行 `transform:translateY(-1px)` 或 `box-shadow:0 0 0 2px var(--ac-soft)`；另历史观察项 **N4**（`Theme.modal-scrim` 0.32 下卡片与背板仅 暗 1.04:1 / 亮 1.01:1，弹框分离依赖 24px 投影）根因在 `ui/theme.slint`，非本稿问题。**结论：无未决设计/合规问题，可交稿。**

## 7. 未能验证项（v1.2 更新）

1. **逐帧像素级 / 交互态人眼评估**：headless 单帧截图 + 量化比值不能替代观感判断（含 N5 的 hover 观感、浅纱遮罩下的实际观感）。
2. **`Theme.accent-hover` 的精确值**：`brighter()` 无仓库实现（Slint 内建），只能验证「方向正确（更亮）」。
3. **D4 的 16 条危险命令清单无外部权威**：st-f 的 `partprobe /dev/nvme0n1`、`mount -o noatime /dev/nvme0n1p2 /srv/data` 未标记，属清单边界问题。
4. **F 帧「8 400 行虚拟化」为静态示意**（按 Lead 裁定 b 接受）。
5. **真机 Slint 行为**（`terminal-focus-sequence` 实际焦点序列、bracketed paste 文案分支）不在 spec §7 范围。

---

# v1.3 N5 收尾微核（task-5 · 2026-11-06 10:53 起）

> **结论：v1.3 微核通过。** 改动无害、回归全绿、零布局位移、无新色相；**N5 记 closed**（附一条强度备注，见 §4）。累计 **16/16 + N5 closed**，可交稿收口。

## §0 基线

| 项 | v1.2 | v1.3 |
|---|---|---|
| SHA-256 | `61E3692E…7168` | **`B8F59405924FA622BDF7D4CFABFA527CC6CD669C89457B3BD90903C7BAF02205`** |
| 字节 / 行数 | 174305 / 1581 | **174566 / 1584**（+261 B / +3 行） |
| mtime | 2026-11-06 10:46:54 | **2026-11-06 10:53:16** |

- 改动 = Lead 直接编辑 `.btn.primary:hover` 一处（`L368-373`）；评审前后哈希一致（本人全程只读）。
- 行数口径：`(Get-Content $f).Count` = 1584，与 Lead 工具 JSON 的 `lines: 1584` 一致。

## §1 检查① 改动范围

| 取证 | 结果 | 判 |
|---|---|---|
| `box-shadow:0 0 0 2px` 全稿出现次数 | **1**（仅 L373） | pass |
| `--ac-soft` 定义 | L59 暗 `rgba(31,200,180,.16)` / L82 亮 `rgba(31,118,107,.12)`，与 v1.2 逐值相同，**未被重新定义** | pass |
| `.btn.primary` 静态（L367） | 未变 | pass |
| `.btn.primary:hover`（L372-373） | 仅新增 `box-shadow:0 0 0 2px var(--ac-soft)`；底色仍 `--slot-ac`（全稿 `background:var(--ac-hov)` = **0** 处） | pass |
| `.btn.primary:active`（L374-375） | 未变（`slot-ac` 底 + `inset` 阴影） | pass |
| 后续规则（L376-378 等） | 仅整体下移 3 行，内容未变 | pass |

→ 改动确实只落在 `.btn.primary:hover` 一处，未波及任何其他规则。

## §2 检查② token 与回归（零回归）

| 项 | v1.2 | v1.3 | 判 |
|---|---|---|---|
| §4.1 token 逐值对账 | 34/34 | **34/34**（fail 0 / absent 0） | pass |
| 颜色字面量分类 | UNKNOWN 41 / SPEC-EXACT 30 / SLOT+ALPHA 26 / BW-ALPHA 6 / BW 1 | **完全一致** | pass（无新色相、无新字面量） |
| 稿内 38 格对账表 | 0 不符 | **0 不符** | pass |
| 外链 / 语法 / 运行时 / id 悬空 | none / 0 / 0 / 0 | 同 | pass |
| 七帧折叠与风险账 | 见 v1.2 | 逐项一致 | pass |
| Lead `smoke.mjs` | PASS | **PASS**（174566 / 1584 / ids 44 / dangling 0 / undefinedVars 0） | pass |

原始输出摘要：`smoke.js` → `EXTERNAL_FOUND none`、`script_tags=1 #0 attrs="" chars=10653`、`SYNTAX #0 OK`、`RUNTIME exec: OK`、`CLICKS tried=44 threw=0`、`ERRORS (0)`、`CONSOLE (0)`；`idcheck.js` → `defined_ids=44 refs=180 dangling=0`；`audit-tokens.js` → `pass=34 fail=0 absent=0`；`table-audit.js` → `19 行 / 38 格，不符 0 格`；`count-rows.js` → st-b `12/364/3`、`is-hidden=4`（gutter 2,3,11,12）、提示「以上另有 2 行」「以下另有 2 行」。

## §3 检查③ 布局位移与遮挡

- `box-shadow` 不参与布局 → **零位移**（按钮盒尺寸与 `.btn::after` 命中区 32px 均不变）。
- 间距余量：`.btns{gap:var(--s-2)}` = **8px**（L347）、`.dlg-foot{padding:12px 16px}`（L251）→ 2px 外环距相邻按钮最坏 `8−2−2 = 4px`（实际仅主按钮带环 → 6px）；距卡片边缘 12px/16px。
- 裁切：`.dlg{…overflow:hidden}`（L230），环完全落在 padding 内 → **不被裁切**；`.dlg-foot` 自身未声明 overflow。
- 叠影：`.btn.primary:active`（L374-375）与 hover 同特异度且源序更晚，其 `box-shadow:inset…` 按下时**覆盖** hover 外环 → 按下态无叠影。
- 与焦点环关系：`.btn.is-default{outline:…;outline-offset:2px}`（L381）→ 外环恰好填满 border 与 outline 之间的 2px 缝隙，视觉上是连续的 border→外环→outline 光晕，不冲突。

→ 无重叠、无位移、无裁切。

## §4 检查④ N5 判定（含一条强度披露）

- **可记 closed**：spec §6 七条 / §4.4 八条 / §5.3 D1–D5 均未规定 hover 反馈强度；本次改动无回归、无位移、无新色相；hover 三件套成立（底色恒 `slot-ac` → 字色 8.58 / 5.13 恒定；1px 描边提亮；2px 外环）。
- **披露（强度改善属边际）**：外环用 `--ac-soft` 时，按 alpha 合成后与相邻底的对比度 = 暗 **1.32:1**（呈色 `#173232` on `.dlg-foot` `#161619`）/ 亮 **1.17:1**（呈色 `#D9E1DD` on `#F2F0EC`）→ **外环本身接近不可见**，L371 所述「使 hover 在亮色下也清晰可感知」**未由外环达成**；真正承载可感知的仍是 1px 描边提亮（暗 `#1FC8B4`→`#25DDC8`、亮 `#1F766B`→`#238478`，ΔL +5.3pp / +3.5pp；作非文本边界对底 ≥3:1）。
- 可选一行增强（**不阻断收口**）：把外环色换成 `--ac-hov` → `box-shadow:0 0 0 2px var(--ac-hov)`，对底 暗 **10.54:1** / 亮 **3.97:1**；代价是需把 L591 的「`--ac-hov` 只用于 `border-color`」改为「border-color / 外环」，仍属零新增色值。

## §5 累计计数与最终判定

| 轮次 | 缺陷 | 状态 |
|---|---|---|
| v1 | S1 S2 H1 H2 H3 M1 M2 M3 L1 L2 L3 L4 L5（13 条） | 13/13 closed（v1.1 轮） |
| v1.1 | N1 N2 N3（3 条） | 3/3 closed（v1.2 轮） |
| v1.2 | N5（建议项） | **closed（本轮）** |
| — | N4（观察项；根因在 `ui/theme.slint` 的 `scrim-strength: 0.32`，非稿子问题） | 不计缺陷 |

**最终判定：passed —— v1.3 微核通过，可交稿收口。**

> 可引用结论段：**v1.3 微核通过。** Lead 在 `.btn.primary:hover` 追加的 `box-shadow:0 0 0 2px var(--ac-soft)` 外环落在预期规则内且仅此一处；`--ac-soft` 未被重新定义，§4.1 token 仍 34/34、颜色字面量分类与 v1.2 完全一致（无新色相）、稿内 38 格对账表 0 不符、语法/运行时/外链/id 悬空/七帧账目全部零回归；`box-shadow` 不参与布局，按钮间距 8px 下最坏余量 4px、卡片 `overflow:hidden` 不裁切、`:active` 的 `inset` 阴影按源序覆盖 hover 外环 → 无位移、无重叠、无叠影；N5 记 closed。唯一备注：外环自身与相邻底仅 1.32:1（暗）/ 1.17:1（亮），接近不可见，hover 的可感知性实际由 1px 描边提亮承载（暗 ΔL +5.3pp / 亮 +3.5pp）；如需更强反馈，把外环色换成 `--ac-hov`（对底暗 10.54:1 / 亮 3.97:1，一行、零新增色值，需同步改 L591 措辞）。

## §6 未能验证项（沿用 v1–v1.2）

1–5 同 v1.2 章 §7。本轮新增同一性质的一条：**hover 观感的人眼评估**——只给量化比值（外环 1.32:1 / 1.17:1、描边 ΔL +5.3pp / +3.5pp），未做主观观感判级。

---

# v1.3b 收口轮：外环改用 `--ac-hov`（task-5 · 2026-11-06 10:57 起）

> **结论：v1.3b 收口轮通过 —— N5 最终 closed，累计 17/17 closed，可交付。**

## §0 基线

| 项 | v1.3 | v1.3b |
|---|---|---|
| SHA-256 | `B8F59405…2205` | **`1FA1B1235E87B0DDE1A8BD112574B73D98047E0F074BFA33AEF13E2D4A939233`** |
| 字节 / 行数 | 174566 / 1584 | **174621 / 1584**（+55 B / ±0 行） |
| mtime | 2026-11-06 10:53:16 | 2026-11-06 10:57:17 |

行数不变而字节 +55：注释 4 行（L368-371）**按原行数改写**（未增删行），外环色由 `--ac-soft` 换成 `--ac-hov`。评审前后哈希一致（本人全程只读）。

## §1 检查① 改动仍只在该规则块内

| 取证 | 结果 | 判 |
|---|---|---|
| `box-shadow:0 0 0 2px` 次数 / 取值 | **1** 处（L373）= `var(--ac-hov)` | pass |
| `--ac-soft` 定义 | L59 暗 `rgba(31,200,180,.16)`（亮侧 L82 `rgba(31,118,107,.12)`）——与 v1.2/v1.3 逐值相同，**未重新定义** | pass |
| `.btn.primary`（L367）/ `.btn.primary:active`（L374-375） | 未动（`active` 仍 `slot-ac` 底 + `inset` 阴影） | pass |
| 注释（L368-371） | 已改写为 `border-color` 与 `box-shadow` 外环同取 `--ac-hov`，并明写 `--ac-soft` 作外环对底仅 暗 1.32:1 / 亮 1.17:1「肉眼几乎不可见，故不采用」 | pass（与本报告 v1.3 实测一致） |
| 白字 + 实心 danger | 仍仅 **1** 处 = L157 `.legacy-btn`（现状对照卡，忠实复刻 `ui/confirm_dialog.slint:7-31`） | pass |

## §2 检查② 外环对比度（SC 1.4.11 ≥3:1）与遮挡

| 相邻对 | 暗 | 亮 | 判 |
|---|---|---|---|
| 外环 vs `.dlg-foot` 底（页脚 `#161619` / `#F2F0EC`） | `#25DDC8` → **10.54:1** | `#238478` → **3.97:1** | pass（均 ≥3） |
| 外环 vs 卡片底（`#181A1D` / `#FAF8F4`） | **10.18:1** | **4.26:1** | pass（均 ≥3） |
| 外环 vs 按钮自身边/底（同相 `#1FC8B4` / `#1F766B`） | 1.23:1 | 1.20:1 | **不适用**：双色光晕的内部过渡，非「指示器 ↔ 背景」对；外边缘（对背景）即上两行 |
| 对照：旧 `--ac-soft` 外环（alpha 合成后） | `#173232` 1.32:1 | `#D9E1DD` 1.17:1 | 已被证伪并替换 ✓ |

- **复算确认 Lead 给出的 10.54 / 3.97 正确**；门槛判定取相邻的**背景面**（页脚 / 卡片底）→ 全部 ≥3，最小 **3.97:1**（亮·页脚）✓。
- 遮挡 / 位移：`box-shadow` 不参与布局 → **零位移**；`.btns{gap:var(--s-2)}` = **8px**（L347）→ 2px 外环距相邻按钮最坏余量 **4px**（实际仅主按钮带环 → 6px）；`.dlg{overflow:hidden}`（L230）不裁切；`.btn.primary:active`（L374-375，同特异度、源序更晚）按下时以 `inset` 覆盖外环 → **无叠影**。

## §3 检查③ N5 最终判定 + 回归 + 累计计数

- hover 终态 = **底色恒 `slot-ac`**（字色 暗 8.58:1 / 亮 5.13:1，不变）**+ 1px 描边提亮**（`#1FC8B4`→`#25DDC8` / `#1F766B`→`#238478`，ΔL +5.3pp / +3.5pp）**+ 2px `--ac-hov` 外环**（对底 暗 10.54:1 / 亮 3.97:1）→ 暗/亮两主题均清晰可感知 ✓ → **N5 最终 closed**。
- 本轮回归（全绿 / 0 fail）：`smoke.js` → `EXTERNAL_FOUND none`、`script_tags=1`、`SYNTAX #0 OK`、`RUNTIME exec: OK`、`CLICKS tried=44 threw=0`、`syntax_fail=0 / runtime_exec_fail=0 / runtime_errors=0`；`idcheck.js` → `defined_ids=44 refs=180 dangling=0`；`audit-tokens.js` → **pass=34 fail=0 absent=0**（distinct 104；UNKNOWN 41 / SPEC-EXACT 30 / SLOT+ALPHA 26 / BW-ALPHA 6 / BW 1，与 v1.2/v1.3 完全一致 → **无新色相、无新字面量**）；`table-audit.js` → **19 行 / 38 格，不符 0 格**；`count-rows.js` → 七帧与 v1.3 逐项一致（st-b `12/364/3`、is-hidden 4 = 2+2；st-e is-hidden 20 = 17+3；st-f is-hidden 4；st-c/d/g 同前）；Lead `smoke.mjs` → **PASS**（174621 / 1584 / ids 44 / dangling 0 / undefinedVars 0，已写入 `.tmp-lead\verifier-smoke.json`）。
- 累计计数：**17/17 closed** ＝ v1 的 13 条 + v1.1 的 N1–N3（3 条）+ N5（1 条）；N4 为观察项（根因在 `ui/theme.slint` 的 `scrim-strength: 0.32`，非稿子问题），不计入。

**最终判定：passed —— 收口轮通过，可交付。**

> 可引用结论段：**v1.3b 收口轮通过，累计 17/17 closed。** 外环由 `--ac-soft` 改为 `--ac-hov` 后，`box-shadow:0 0 0 2px` 全稿仍仅 L373 一处、`--ac-soft` 定义未变、`:active` 未动；外环对相邻背景面的对比度为暗 10.54:1（页脚）/ 10.18:1（卡片底）、亮 3.97:1（页脚）/ 4.26:1（卡片底），全部 ≥ WCAG 2.1 SC 1.4.11 的 3:1；hover 三件套（底色不变 + 描边提亮 + 2px 外环）在暗/亮两主题下均可清晰感知；`box-shadow` 不参与布局，8px 按钮间距下最坏余量 4px，卡片 `overflow:hidden` 不裁切，`:active` 的 `inset` 阴影按源序覆盖外环 → 零位移、无遮挡、无叠影；回归零失败（§4.1 仍 34/34、38 格对账表 0 不符、语法/运行时/外链/id 悬空/七帧账目全绿）。

## §4 未能验证项

沿用 v1–v1.2 的 5 条 + v1.3 新增 1 条（hover 观感无主观判级），**本轮无新增**。补充说明：外环三组对比度均为脚本实算，但 `--ac-hov` 与 `Theme.accent-hover` 的**精确**关系仍无法验证（`brighter()` 无仓库实现）——方向已正确（亮色更亮）且满足非文本门槛。

