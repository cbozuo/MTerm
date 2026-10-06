// ═════════════════════════════════════════════════════════════════════
// 设计稿主题层重构手术（2026-10-07）：删 8 套 + 增 11 套 → 36 预设
//  ① [data-theme] CSS 块层整体重写（36 块，四族顺序）
//  ② JS：THEME_IDS / GROUPS / PAL 重建
//  ③ §2b 校验表删行 + 重编号
//  ④ 文案同步（§2 标题/概览/授权表、§4 表、§6 已定与待定②④收口）
// 幂等性：CSS/JS/表按内容重建，重复运行结果一致；文字替换为等值幂等。
// 用法：node scripts/retheme.js
// ═════════════════════════════════════════════════════════════════════
const fs = require('fs');
const path = require('path');

const TOKENS = JSON.parse(fs.readFileSync(path.join(__dirname, 'theme-tokens.json'), 'utf8'));
const RADIX_T = JSON.parse(fs.readFileSync(path.join(__dirname, '_radix-themes.json'), 'utf8'));
const EXTRA = JSON.parse(fs.readFileSync(path.join(__dirname, '_extra-themes.json'), 'utf8'));
const ALL = { ...TOKENS, ...RADIX_T, ...EXTRA };

const P = path.join(__dirname, '..', 'docs', 'design', 'theme-wallpaper-split-2026-10-05.html');
let html = fs.readFileSync(P, 'utf8');

// ── 36 预设（与 gen_ansi_group.js 的 GROUPS_DEF/VARIANTS 一致，不含 meat 迁移档）──
const GROUPS = [
  ['radix', '现代 UI 色板',
   'Radix Colors 体系 · 为界面设计，明度等距 12 档。5 个中性色系 × 明暗 = 10 个变体，面板层级清晰、柔和分层。',
   ['graphite-dark', 'graphite-light', 'slate-dark', 'slate-light', 'mauve-dark', 'mauve-light',
    'sage-dark', 'sage-light', 'sand-dark', 'sand-light']],
  ['brand', '品牌官方',
   'GitHub（Primer 体系）与 JetBrains（IDEA New UI）的官方 UI 主题 —— 程序员最熟悉的界面语言。',
   ['github-dark', 'github-light', 'idea-dark', 'idea-light']],
  ['terminal', '经典终端配色',
   '终端社区的成熟色系 · 为长时间读输出设计，硬对比、护眼。按官方真有的档位取色。',
   ['dracula-dark', 'catppuccin-dark', 'catppuccin-light', 'solarized-dark', 'solarized-light',
    'kanagawa-wave', 'kanagawa-lotus', 'ayu-dark', 'ayu-mirage', 'ayu-light',
    'gruvbox-material-dark', 'gruvbox-material-light',
    'tokyonight-dark', 'tokyonight-light', 'nord-dark', 'nord-light', 'everforest-dark',
    'rosepine-dark', 'rosepine-light', 'onedark-dark', 'flexoki-dark', 'flexoki-light']],
];
// 校验 36
{
  const n = GROUPS.reduce((a, [, , , ids]) => a + ids.length, 0);
  if (n !== 36) { console.error('族内套数 ≠ 36:', n); process.exit(1); }
  for (const [, , , ids] of GROUPS) for (const id of ids)
    if (!ALL[id]) { console.error('缺 tokens:', id); process.exit(1); }
}
const THEME_IDS = GROUPS.flatMap(g => g[3]);

// ── ① CSS 块层整体重写 ──
function cssBlock(id) {
  const t = ALL[id];
  const star = t.star ? ` —— ${t.star}★` : ' —— 官方出品';
  return `  /* ${t.f} · ${t.m}${star} */
  [data-theme="${id}"]{
    --bg-root:        ${t.root};
    --bg-panel:       ${t.panel};
    --bg-panel-alt:   ${t.palt};
    --bg-elev:        ${t.elev};
    --bg-hover:       ${t.hov};
    --bg-active:      ${t.act};
    --bg-tab:         ${t.tab};
    --line:           ${t.line};
    --line-strong:    ${t.lstr};
    --t1:             ${t.t1};
    --t2:             ${t.t2};
    --t3:             ${t.t3};
    --term-bg:        ${t.tbg};
    --term-fg:        ${t.tfg};
    --accent:         ${t.ac};
    --accent-2:       ${t.ac2};
    --ok:             ${t.ok};
    --warn:           ${t.wr};
    --danger:         ${t.dg};
    --danger-strong:  ${t.dgS};
    /* 频道 4 槽：固定色相，跨全部主题共用 */
    --ch-a:               ${t.ch[0]};
    --ch-b:               ${t.ch[1]};
    --ch-c:               ${t.ch[2]};
    --ch-d:               ${t.ch[3]};
  }`;
}
const FAMILY_TAG = { radix: '现代 UI 色板', brand: '品牌官方', terminal: '经典终端配色' };
let cssOut = [];
for (const [key, name] of Object.entries(FAMILY_TAG)) {
  const grp = GROUPS.find(g => g[0] === key);
  cssOut.push(`  /* ═══ ${name} ═══ */`);
  cssOut.push(grp[3].map(cssBlock).join('\n\n'));
}
const newCss = cssOut.join('\n\n');

// 定位旧层：第一个套注释（graphite 之前）到 darcula 块结束
// ⚠️ 替换区间里原本夹着文档区/设置面板 mock 的样式（.sec/.card/.ifd/.ni/.srow/.slider/
// .seg/.accents/.tok 等，scripts/_mock-css.txt，提取自 git HEAD）——2026-10-07 曾因
// 重写时丢掉它导致 §3 控件全消失。每次重写都必须把它拼回层尾。
const cssStart = html.indexOf('  /* ═══ 现代 UI 色板 ═══ */');
const cssEndAnchor = '  .demo{border-radius:9px;overflow:hidden;border:1px solid var(--doc-line)}';
if (cssStart < 0 || !html.includes(cssEndAnchor)) { console.error('找不到 CSS 层边界'); process.exit(1); }
const cssEnd = html.indexOf(cssEndAnchor);
const MOCK_CSS = fs.readFileSync(path.join(__dirname, '_mock-css.txt'), 'utf8');
html = html.slice(0, cssStart) + newCss + '\n\n' + MOCK_CSS + '\n\n' + html.slice(cssEnd);

// ── ② JS 数据重建 ──
function palEntry(id) {
  const t = ALL[id];
  return `   "${id}": ${JSON.stringify({
    f: t.f, en: t.en, m: t.m, kind: t.kind, group: t.group,
    star: t.star, lic: t.lic, repo: t.repo, desc: t.desc,
    root: t.root, panel: t.panel, palt: t.palt, elev: t.elev, tab: t.tab,
    line: t.line, t1: t.t1, t2: t.t2, t3: t.t3, tbg: t.tbg, tfg: t.tfg,
    ac: t.ac, ac2: t.ac2, ok: t.ok, wr: t.wr, dg: t.dg, ch: t.ch,
  }, null, 1).replace(/\n/g, '\n   ')}`;
}
const newJsIds = '  var THEME_IDS = ' + JSON.stringify(THEME_IDS) + ';';
const newGroups = '  var GROUPS = ' + JSON.stringify(GROUPS.map(([k, n, d, ids]) => [k, n, d, ids]), null, 1)
  .replace(/\n/g, '\n  ') + ';';
const newPal = '  var PAL = {\n' + THEME_IDS.map(palEntry).join(',\n') + '\n  };';

{
  const a = html.indexOf('var THEME_IDS');
  const aEnd = html.indexOf(';', a) + 1;
  const b = html.indexOf('var GROUPS', aEnd);
  const bEnd = html.indexOf('];', b) + 2;
  const c = html.indexOf('var PAL = {', bEnd);
  const cEnd = html.indexOf('\n  };', c) + 5;
  if (a < 0 || b < 0 || c < 0) { console.error('找不到 JS 数据块'); process.exit(1); }
  html = html.slice(0, a) + newJsIds + '\n' + newGroups + '\n' + newPal + html.slice(cEnd);
}

// ── ③ §2b 校验表：删行 + 重编号 ──
const REMOVE = new Set([['Gruvbox', '暗'], ['Gruvbox', '亮'], ['德古拉', '亮'], ['原子暗', '亮'],
  ['Darcula', '暗'], ['橄榄', '暗'], ['橄榄', '亮'], ['常青', '亮']].map(([a, b]) => a + '|' + b));
{
  // 2b 标题经历过多次改写——候选全部落空时 b2 会是 -1，slice(-1,…) 会把
  // 「§3 注释到文件尾」整段复制一份（2026-10-07 实际翻过车）。找不到就跳过。
  const B2_TITLES = ['2b · 36 个变体逐个校验', '2b · 33 个变体逐个校验', '2b · UI 色板逐套校验'];
  let b2 = -1;
  for (const t of B2_TITLES) { const i = html.indexOf(t); if (i >= 0) { b2 = i; break; } }
  const b2End = html.indexOf('<!-- ══════════ §3 设置页重排');
  if (b2 >= 0 && b2End > b2) {
  let seg = html.slice(b2, b2End);
  // 删行：tr 以 <td class="mono">N</td><td class="c"...>名</td><td class="c">档</td> 开头
  seg = seg.replace(/<tr><td class="mono">\d+<\/td><td class="c"[^>]*>([^<]+)<\/td><td class="c">([^<]+)<\/td>[\s\S]*?<\/tr>\n?/g,
    (m0, name, mode) => REMOVE.has(name + '|' + mode) ? '' : m0);
  // 三个 chk 表各自重编号
  seg = seg.replace(/<table class="chk"[^>]*>([\s\S]*?)<\/table>/g, (m0, body) => {
    let i = 0;
    const nb = body.replace(/<tr><td class="mono">(\d+)<\/td>/g, () => `<tr><td class="mono">${++i}</td>`);
    return m0.replace(body, nb);
  });
  html = html.slice(0, b2) + seg + html.slice(b2End);
  } else console.log('2b 段未命中已知标题，跳过（幂等）');
}

// ── ④ 文案同步 ──
const REPLACEMENTS = [
  ['<div class="sec-t">2 · 主题预设：9 套社区色系 + 3 套 IDEA 主题 = 21 个变体</div>',
   '<div class="sec-t">2 · 主题预设：四族 36 个变体</div>'],
  ['33 个变体 · 下方 1:1 里可以直接点选', '36 个变体 · 下方 1:1 里可以直接点选'],
  ['现代 UI 色板 · 12 个', '现代 UI 色板 · 10 个'],
  ['经典终端配色 · 18 个', '经典终端配色 · 22 个'],
  ['JetBrains 官方 · 3 个', '品牌官方 · 4 个'],
  ['2b · 33 个变体逐个校验（非抽样）', '2b · UI 色板逐套校验（非抽样 · 存量 25 套；新增 11 套与 ANSI/分组色的逐套校验见 2c）'],
  ['2b · 36 个变体逐个校验（非抽样）', '2b · UI 色板逐套校验（非抽样 · 存量 25 套；新增 11 套与 ANSI/分组色的逐套校验见 2c）'],
  ['<b>内置主题</b><br>21 个变体', '<b>内置主题</b><br>36 个变体'],
  ['9 套社区色系（Nord / Solarized / Tokyo Night / Gruvbox /\n          Dracula / Catppuccin / Everforest / One Dark / Rosé Pine）\n          + 3 套 IDEA 主题，按各自<b>真有的档位</b>生成。',
   '四族：<b>现代 UI 色板</b>（Radix 10）+ <b>品牌官方</b>（GitHub Primer 2 + IDEA 2）+\n          <b>经典终端配色</b>（22：Dracula / Catppuccin / Solarized / 神奈川 / Ayu / 岩焙 /\n          Tokyo Night / Nord / Everforest / Rosé Pine / One Dark / 墨纸），按各自<b>真有的档位</b>生成。'],
  ['<b>3 个族共 33 个变体</b>：现代 UI 色板（Radix，12）\n              + 经典终端配色（9 套，18）+ JetBrains 官方（3）。',
   '<b>3 个族共 36 个变体</b>：现代 UI 色板（Radix，10）\n              + 品牌官方（GitHub 2 + IDEA 2）+ 经典终端配色（22）。'],
  ['新增「现代 UI 色板」族（Radix Colors，MIT，1686★）\n      —— 6 个中性色系 × 明暗 = 12 个变体，打头排在选单最前。',
   '新增「现代 UI 色板」族（Radix Colors，MIT，1686★）\n      —— 5 个中性色系 × 明暗 = 10 个变体，打头排在选单最前。'],
];
for (const [a, b] of REPLACEMENTS) {
  if (html.includes(a)) html = html.split(a).join(b);
}

// Gruvbox 授权表行 → 已移除标注
html = html.replace(/<tr><td class="c">Gruvbox<\/td>[\s\S]*?<\/tr>/,
  `<tr><td class="c">Gruvbox<br><span style="font-size:9.5px;color:var(--doc-t3)">→ 已移除</span></td>
    <td class="c no-c">未声明</td>
    <td class="c">仓库没有 LICENSE 文件（README 写 "License MIT/X11" —— 声明但未落文件）。
      <b>2026-10-07 重构：整族移除</b>，暖色相空位由 <b>Gruvbox Material</b>（sainnhe，MIT，2649★）
      接替 —— 同风格、对比度重调、授权干净。</td></tr>`);

// §6 待定② Everforest 亮档 → 已收口
html = html.replace(/<b>②<\/b>\s*<div class="c">是否保留[\s\S]*?<\/div>/,
  `<b>②</b>
          <div class="c"><b>已收口（2026-10-07）</b>：Everforest 亮档按方案 a 处理 ——
            官方灰阶撑不起「主/次/弱」三档正文（次文本 2.90），<b>亮档已从预设移除，只推暗档</b>。</div>`);
// §6 待定④ Gruvbox → 已收口
html = html.replace(/<b>④<\/b>\s*<div class="c">Gruvbox 要不要留？[\s\S]*?<\/div>/,
  `<b>④</b>
          <div class="c"><b>已收口（2026-10-07）</b>：原版 Gruvbox 因授权未声明<b>整族移除</b>；
            暖色相空位由 <b>岩焙 Gruvbox Material</b>（同风格授权干净版，MIT）接替，明暗双档。</div>`);

// ── §2 开头插入重构说明 ──
const NOTE = `
    <div class="box" style="margin-bottom:15px">
      <b>2026-10-07 主题重构：删 8 增 11 → 36 预设（+2 迁移保留档，数据见 2c）</b><br>
      <b>移除</b>：Gruvbox ×2（授权未声明）、德古拉·亮/Alucard（官方明说不是亮色档，挂名困惑）、
      原子暗·亮（Atom 无 One Dark 亮档，纯自造违反「不硬凑档位」）、Darcula（与 IDEA 暗重叠）、
      橄榄 ×2（与青苔/暖砂色温重叠）、常青·亮（官方灰阶撑不起正文，待定②收口）。<br>
      <b>新增</b>：神奈川 Kanagawa（6420★，浮世绘蓝灰水墨，官方 wave/lotus 双档）、
      GitHub 暗/亮（Primer 官方体系）、Ayu 暗/霞/亮（官方三档，霞为官方独有中间档）、
      墨纸 Flexoki（3686★，Obsidian 作者的纸墨高对比）、岩焙 Gruvbox Material（2649★，
      Gruvbox 授权干净版）—— 全部 MIT（2026-10-07 gh api 实测），官方真档优先。
      <b>分组从三族变四族</b>：现代 UI 色板 10 / 品牌官方 4 / 经典终端配色 22 / 迁移保留档 2。
    </div>`;
{
  if (html.includes('2026-10-07 主题重构：删 8 增 11')) {
    console.log('重构说明已存在，跳过插入');
  } else {
  const anchor = '<div class="sec-d">\n    全部色值取自各自<b>官方发布的调色板文件</b>';
  const at = html.indexOf(anchor);
  if (at < 0) { console.error('找不到 §2 sec-d 锚点'); process.exit(1); }
  // 插在 sec-d 所在 card 内、redbox 之前 —— 即 sec-d 结束 </div> 之后
  const secEnd = html.indexOf('</div>', at + anchor.length) + 6;
  html = html.slice(0, secEnd) + NOTE + html.slice(secEnd);
  }
}

// ── 写盘前结构断言（防边界 bug 把文档搞出重复/提前闭合）──
function count(hay, needle) { return hay.split(needle).length - 1; }
const assertMsg = [];
if (count(html, '<script>') !== 1) assertMsg.push('script×' + count(html, '<script>'));
if (count(html, '</body>') !== 1) assertMsg.push('</body>×' + count(html, '</body>'));
if (count(html, 'var THEME_IDS') !== 1) assertMsg.push('THEME_IDS×' + count(html, 'var THEME_IDS'));
if (count(html, '2026-10-07 主题重构') > 1) assertMsg.push('NOTE×' + count(html, '2026-10-07 主题重构'));
if (count(html, '[data-theme]') < 1) assertMsg.push('公共派生块丢失');
if (assertMsg.length) { console.error('结构断言失败，拒绝写盘：', assertMsg.join(', ')); process.exit(1); }

fs.writeFileSync(P, html, 'utf8');
console.log('主题层重构完成：CSS 36 块 + JS 数据 + 2b 删行 + 文案同步 →', path.basename(P));
