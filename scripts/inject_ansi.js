// 把 gen_ansi_group.js 产出的 _ansi-group.json 注入设计稿：
//   ① §2b 与 §3 之间插入「2c · 终端 ANSI 16 色 + 分组色」数据节
//   ② demo 终端小样的着色类从 UI 语义色改为 ANSI 槽位
//   ③ sel() 换肤时下发 --ansi-N 变量
// 幂等：以注入标记定位，重复运行先移除旧块。
// 用法：node scripts/inject_ansi.js
const fs = require('fs');
const path = require('path');

const D = JSON.parse(fs.readFileSync(path.join(__dirname, '_ansi-group.json'), 'utf8'));
const P = path.join(__dirname, '..', 'docs', 'design', 'theme-wallpaper-split-2026-10-05.html');
let html = fs.readFileSync(P, 'utf8');

const MARK_OPEN = '<!-- ═══ 2c ANSI+分组色（scripts/gen_ansi_group.js + inject_ansi.js 注入）═══ -->';
const MARK_CLOSE = '<!-- ═══ /2c ═══ -->';

// ── 幂等清理 ──
{
  const i = html.indexOf(MARK_OPEN), j = html.indexOf(MARK_CLOSE);
  if (i >= 0 && j > i) html = html.slice(0, i) + html.slice(j + MARK_CLOSE.length);
}

// ── 小工具 ──
const ANAMES = ['黑', '红', '绿', '黄', '蓝', '紫', '青', '白', '亮黑', '亮红', '亮绿', '亮黄', '亮蓝', '亮紫', '亮青', '亮白'];

// 逐套明细行
function row(id, rec) {
  const meatTag = rec.kind === 'meat' ? ' <span class="tag tag-b">迁移保留档</span>' : '';
  const ansi = rec.ansi16.map((c, i) =>
    `<span title="ansi${i} ${ANAMES[i]} ${c}" class="sw" style="background:${c};width:15px;height:15px;border:1px solid rgba(128,128,128,.35);border-radius:3px"></span>`
  ).join('');
  const grp = rec.group24.map(c =>
    `<span title="${c}" class="sw" style="background:${c};width:9px;height:15px;border-radius:2px;opacity:.92"></span>`
  ).join('');
  const failN = rec.fails.length;
  const badge = failN === 0
    ? '<span class="tag tag-g">12 彩槽全过 4.5</span>'
    : `<span class="tag tag-o" title="${rec.fails.map(f => f.replace(/</g, '&lt;')).join('&#10;')}">${failN} 槽 &lt; 4.5 · ${rec.kind === 'meat' ? '现状原样' : '官方色原样'}</span>`;
  const notes = (rec.notes || []).map(n => `<div style="font-size:9.5px;color:var(--doc-t3);margin-top:2px;max-width:200px;white-space:normal;line-height:1.5">· ${n}</div>`).join('');
  return `<tr>
    <td class="mono" style="white-space:nowrap">${rec.f}·${rec.m}${meatTag}<div style="font-size:9px;color:var(--doc-t3)">${id}</div></td>
    <td><div style="display:flex;gap:2px;flex-wrap:wrap">${ansi}</div></td>
    <td><div style="display:flex;gap:1.5px;flex-wrap:wrap;max-width:210px">${grp}</div></td>
    <td style="white-space:nowrap">${badge}${notes}</td>
  </tr>`;
}

const ids = Object.keys(D).filter(k => k !== '_meta');
const officialTrue = ['dracula-dark', 'catppuccin-dark', 'catppuccin-light', 'solarized-dark', 'solarized-light',
  'tokyonight-dark', 'tokyonight-light', 'nord-dark', 'everforest-dark', 'rosepine-dark', 'rosepine-light', 'onedark-dark',
  'kanagawa-wave', 'github-dark', 'github-light', 'ayu-dark', 'ayu-mirage', 'ayu-light',
  'flexoki-dark', 'flexoki-light', 'gruvbox-material-dark', 'gruvbox-material-light'];
const derived = ['nord-light', 'kanagawa-lotus'];
const ideaSet = ['idea-dark', 'idea-light'];
const meatSet = ['meat-dark', 'meat-light'];
const radixSet = ids.filter(i => !officialTrue.includes(i) && !derived.includes(i) && !ideaSet.includes(i) && !meatSet.includes(i));
const allPass = ids.filter(i => D[i].fails.length === 0);
const hasFail = ids.filter(i => D[i].fails.length > 0);

const sec = `
${MARK_OPEN}
<!-- ══════════ §2c ANSI + 分组色（缺口补全） ══════════ -->
<div class="sec" id="sec-2c">
  <div class="sec-t">2c · 终端 ANSI 16 色 + 分组色 24 色（缺口补全 · 2026-10-07）</div>
  <div class="sec-d">
    §3 调整⑤要求"ANSI 16 色 2 套 → 按主题查表、分组色按主题重新取档"，但原稿只有 UI 槽位数据。
    本节把数据缺口补上：<b>36 套预设 × 16 ANSI</b>、<b>36 套 × 24 分组色</b>、<b>38 套 × 4 频道色</b>，
    外加 <b>2 个迁移保留档</b>（meat-dark / meat-light，§5 迁移表引用的现配色 id ——
    套数据总数 36 + 2 = 38，老用户迁移后观感零变化）。
    真源与生成器见 <code>scripts/gen_ansi_group.js</code>（可重跑），数据文件
    <code>scripts/_ansi-group.json</code>。逐套校验口径与 §2b 一致：12 个彩色槽（1-6, 9-14）按文字 4.5:1 判；
    0/7/8/15（黑/白系）在终端惯例里兼作背景与弱化用途（如 tokyonight day 的 black 槽），
    只记录不判失败。
  </div>
  <div class="card">
    <div class="cols" style="margin-bottom:14px">
      <div>
        <div class="shead" style="margin-bottom:7px">数据来源四类（38 套）</div>
        <table>
          <tr><th style="width:120px">类</th><th style="width:46px">数</th><th>说明</th></tr>
          <tr><td class="c ok-c">官方真档</td><td class="c mono">${officialTrue.length}</td>
            <td>取官方发布文件原样：Dracula（官方 spec）、Catppuccin（官方 kitty conf）、
              Solarized（官方映射表 + iTerm2 收录核对）、Tokyo Night（官方 kitty extras）、
              Nord 暗（官方 Xresources）、Everforest（官方 palette.md medium + 官方移植核对）、
              Rosé Pine（官方 alacritty dist）、One Dark 暗（atom 移植谱系）、
              神奈川 wave（官方 palette + 官方收录移植）、GitHub 暗/亮（官方 VS Code 主题 terminal 色）、
              Ayu 三档（官方收录移植）、墨纸 Flexoki 暗/亮（官方移植，README 同源）、岩焙 Gruvbox Material（官方 medium palette）</td></tr>
          <tr><td class="c new-c">Radix 自设计</td><td class="c mono">${radixSet.length}</td>
            <td>Radix 是<b>界面色板</b>，官方无 ANSI。按色阶确定性映射：暗档彩槽 normal=各色系 idx10 / bright=idx11，
              亮档 normal=idx10 / bright=idx11（更深，同 Gruvbox-light 官方做法）；黑/白系取各套中性色系；
              校验不足 4.5 自动换档并记录。彩色 6 系全族共用（Radix 色相体系一致），中性随套（5 套 × 2 = 10）</td></tr>
          <tr><td class="c new-c">项目派生档</td><td class="c mono">${derived.length}</td>
            <td>Nord 亮（官方 bright ambiance 指引派生）/ 神奈川 lotus（官方 palette 键映射）：
              官方<b>没有发布对应的终端档</b>。ANSI 与 UI 同规则自造——
              彩色槽保 H/S 调 L 至 4.5:1，来源已在数据 <code>src</code> 字段标注</td></tr>
          <tr><td class="c new-c">IDEA 共用</td><td class="c mono">${ideaSet.length}</td>
            <td>JetBrains Darcula 终端色（iTerm2 官方收录版）。<b>IDEA 的终端 ANSI 不随 UI 明暗切换</b>——
              两套共用同一彩色槽（Darcula 已删），仅底/前景随各套 tokens。亮底上部分彩槽对比度低是 IDEA 本身观感，如实记录</td></tr>
          <tr><td class="c ok-c">迁移保留档</td><td class="c mono">${meatSet.length}</td>
            <td>meat-dark / meat-light = <b>现配色原样</b>（ANSI16_DARK/LIGHT + 亮色 TUI 背景双表、
              分组色两套 24 色、tab 稿四槽频道色）——§5 迁移的落点，落地即"查表里的两行"</td></tr>
        </table>
      </div>
      <div>
        <div class="shead" style="margin-bottom:7px">校验汇总（12 彩槽 on tbg ≥ 4.5）</div>
        <div class="box" style="margin-bottom:9px">
          <b class="ok-c">${allPass.length} 套全过</b>（Radix 12 + dracula/catppuccin/tokyonight/nord 之外的全部暗档……逐套见下表）。
        </div>
        <div class="redbox">
          <b>${hasFail.length} 套有低于 4.5 的彩槽 —— 全部是官方色原样，未做任何改动</b><br>
          名主题的官方 ANSI 在其官方底上本来就普遍低于 4.5：
          Solarized 的设计哲学是低饱和（red 3.3）、Gruvbox red 3.0、Nord 红 3.1、
          Catppuccin Latte green 2.96、Everforest 亮档 yellow 2.12 ——
          这些是各家<b>自己的取舍</b>（多数彩色在真实终端里承载的是非正文输出）。<br><br>
          <b>建议：a) 原样保留（推荐）</b> —— 保真优先，用户选这套主题就是要它的原色；
          2.0 以下（仅 idea-light 的 bright 槽 1.07~1.9）可在落地时按"亮底 ANSI 加深"开关处理，
          作为 §6 待定项的附属决策。<b>b) 全部亮档走调深适配</b> —— 达标但失去官方感，与
          "不为色系硬凑档位"的既有纪律冲突。
        </div>
      </div>
    </div>

    <div class="shead" style="margin:0 0 8px">38 套逐套 · ANSI 16 色 + 分组色 24 色（悬停色块看槽位与色值）</div>
    <table class="chk">
      <tr><th style="width:96px">变体</th><th>ANSI 16（上排 0-7 · 下排 8-15）</th><th style="width:216px">分组色 24（on panel ≥ 3.0）</th><th style="width:150px">校验</th></tr>
      ${ids.map(id => row(id, D[id])).join('\n      ')}
    </table>
    <div style="font-size:10.5px;color:var(--doc-t3);margin-top:9px;line-height:1.75">
      分组色规则：基准取 <code>theme.slint:244-255</code> 现有 24 色（暗套用亮组 / 亮套用深组），
      保 H/S 调 L 至 on(panel) ≥ 3.0，<b>原色已达标则原样不动</b>（最小干预）——
      38 套里绝大多数套位的 24 色与现有代码完全一致，只有少数亮套的尾部灰阶微调。
      落地时 = 每套主题一份 <code>group-color</code> 数组进查表，与 ANSI 同走 §3 调整⑤的
      <code>palette_id</code> 重渲染路径。meat-light 的亮色 <b>TUI 背景双表</b>
      （<code>presentation.rs ANSI16_LIGHT_BG</code>，btop/htop 底色用）已在数据中保留
      （<code>ansi16bg</code> 字段，仅迁移档有）。
    </div>

    <div class="shead" style="margin:16px 0 8px">频道色 4 槽 × 38 套（tab-row-idia 第四章"深浅两套"根治方案的数据形态）</div>
    <div class="cols">
      <div style="flex:1.2">
        <table>
          <tr><th style="width:104px">套</th><th>频道色 A / B / C / D（on tab 底 与 on 选中底 双 ≥ 3.0）</th></tr>
          ${['meat-dark','meat-light'].map(id => {
            const r = D[id];
            const cells = r.ch.map((c, i) => {
              const v = r.channel[i];
              const changed = c.toLowerCase() !== v.toLowerCase();
              return `<span title="槽 ${'ABCD'[i]}：${c} → ${v}${changed ? '（微调达标）' : '（原样达标）'}" class="sw" style="background:${v};width:26px;height:18px;border-radius:4px;border:1px solid rgba(128,128,128,.4)"></span>` +
                     (changed ? `<span class="sw" style="background:${c};width:12px;height:18px;border-radius:4px;opacity:.55;margin-left:-14px;margin-right:14px" title="原值 ${c}"></span>` : '<span style="display:inline-block;width:14px"></span>');
            }).join('');
            return `<tr><td class="mono">${r.f}·${r.m}</td><td><div style="display:flex;align-items:center;gap:10px">${cells}</div></td></tr>`;
          }).join('\n          ')}
        </table>
        <div style="font-size:10px;color:var(--doc-t3);margin-top:5px;line-height:1.6">
          大色块 = 落地值；斜叠小色块 = tab 稿原四槽（仅不达标槽位显示对照）。
        </div>
      </div>
      <div>
        <div class="box">
          tab 稿第四章实测的短板在本数据里被精确复现并修掉：
          <b>D 槽 <code>#993556</code> 在暗色非选中 tab 底（#25282f）只有 2.11</b> →
          meat-dark 落地 <code>#c24e75</code>（双底 ≥ 3.0）；
          亮档 A/B（#378add / #1d9e75，实测 2.79 / 2.63）→ 微调加深为
          <code>#3387dc</code> / <code>#1b956f</code>。
          主题化后每套变体自带 4 槽频道色（<code>_ansi-group.json</code> 的 <code>channel</code> 字段），
          tab 稿"8 个常量"的方案被"每套 4 值"自然覆盖 —— 换主题时频道色随主题走。
        </div>
      </div>
    </div>
  </div>
</div>
${MARK_CLOSE}
`;

// ── ① 插入 2c 节（§2b 与 §3 之间） ──
const anchor = '<!-- ══════════ §3 设置页重排 ══════════ -->';
if (!html.includes(anchor)) { console.error('找不到 §3 锚点'); process.exit(1); }
html = html.replace(anchor, sec + '\n' + anchor);

// ── ② CSS：demo 终端着色类 → ANSI 槽位 ──
const oldCss = `  .dm-term .p{color:var(--t-accent)}
  .dm-term .g{color:var(--t-ok)} .dm-term .y{color:var(--t-warn)}
  .dm-term .r{color:var(--t-danger)} .dm-term .c{color:var(--t-accent2)}
  .dm-term .m{color:var(--t-mut)} .dm-term .dim{color:var(--t-mut);opacity:.75}`;
const newCss = `  /* 2c 补全后：终端小样按 ANSI 槽位着色（不再是 UI 语义色）——
     .p 提示符=蓝 .c 路径=青 .g 成功=绿 .y 警告=黄 .r 错误=红 .m 数字=紫，
     .dim 无 ANSI 槽，保持前景色半透明（终端里 dim 本就不占 16 色）。 */
  .dm-term .p{color:var(--ansi-4)}
  .dm-term .g{color:var(--ansi-2)} .dm-term .y{color:var(--ansi-3)}
  .dm-term .r{color:var(--ansi-1)} .dm-term .c{color:var(--ansi-6)}
  .dm-term .m{color:var(--ansi-5)} .dm-term .dim{color:var(--term-fg);opacity:.62}`;
if (html.includes(newCss)) {
  // 已注入过，跳过
} else if (!html.includes(oldCss)) { console.error('找不到 demo 终端 CSS 锚点'); process.exit(1); }
else html = html.replace(oldCss, newCss);

// ── ③ JS：THEME_ANSI 数据 + sel() 下发 --ansi-N ──
const ansiData = '/* 2c 注入：38 套 ANSI 16 色 + 分组色（真源 scripts/gen_ansi_group.js → _ansi-group.json） */\nvar THEME_ANSI=' +
  JSON.stringify(Object.fromEntries(ids.map(id => [id, { a: D[id].ansi16, g: D[id].group24 }]))) + ';\n';
const jsAnchor = 'var hoverId=null, pickerOpen=false;';
if (!html.includes(jsAnchor)) { console.error('找不到 JS 锚点'); process.exit(1); }
// 先清掉旧数据块（THEME_ANSI 在 script 区，不在 2c 标记内，须单独幂等）
html = html.replace(/\/\* 2c 注入：[\s\S]*?var THEME_ANSI=.*?;\n/g, '');
if (!html.includes('var THEME_ANSI=')) html = html.replace(jsAnchor, ansiData + jsAnchor);

const oldTm = `    if(tm){ tm.style.background=t.tbg; tm.style.color=t.tfg;
      tm.style.setProperty('--t-accent',t.ac);
      tm.style.setProperty('--t-accent2',t.ac2);
      tm.style.setProperty('--t-ok',t.ok);
      tm.style.setProperty('--t-warn',t.wr);
      tm.style.setProperty('--t-danger',t.dg);
      tm.style.setProperty('--t-mut',t.t3); }`;
const newTm = `    if(tm){ tm.style.background=t.tbg; tm.style.color=t.tfg;
      var ta=THEME_ANSI[id].a;
      for(var ai=0;ai<16;ai++) tm.style.setProperty('--ansi-'+ai,ta[ai]);
      tm.style.setProperty('--t-mut',t.t3); }`;
if (html.includes(newTm)) {
  // 已注入过，跳过
} else if (!html.includes(oldTm)) { console.error('找不到 sel() demo 终端块锚点'); process.exit(1); }
else html = html.replace(oldTm, newTm);

fs.writeFileSync(P, html, 'utf8');
console.log('注入完成：2c 节 + demo ANSI 着色 + THEME_ANSI 数据 →', path.basename(P));

// ── ④ tab-row-idia 稿第四章：频道色数据补全注记（闭环） ──
const TP = path.join(__dirname, '..', 'docs', 'design', 'tab-row-idia-2026-10-05.html');
let thtml = fs.readFileSync(TP, 'utf8');
const T_OPEN = '<!-- ═══ 频道色数据补全注记（inject_ansi.js 注入）═══ -->';
const T_CLOSE = '<!-- ═══ /频道色注记 ═══ -->';
{
  const i = thtml.indexOf(T_OPEN), j = thtml.indexOf(T_CLOSE);
  if (i >= 0 && j > i) thtml = thtml.slice(0, i) + thtml.slice(j + T_CLOSE.length);
}
const meatD = D['meat-dark'], meatL = D['meat-light'];
const tBox = `
${T_OPEN}
  <div class="box" style="margin-top:11px">
    <b>注（2026-10-07）：本章"根治方案"的数据已随主题稿 2c 补全</b><br>
    <code>scripts/_ansi-group.json</code>（生成器 <code>scripts/gen_ansi_group.js</code>）现含
    <b>38 套 × 4 槽频道色</b>（36 预设 + meat-dark/light 迁移保留档），每槽在该套的
    <b>非选中 tab 底与选中底双 ≥ 3.0</b>。当前配色的落地值即本章实测短板的修正：
    meat-dark D 槽 <code>#993556</code>（暗/非选中 2.11）→ <b><code>#c24e75</code></b>；
    meat-light A/B 槽（亮/非选中 2.79 / 2.63）→ <b><code>#3387dc</code> / <code>#1b956f</code></b>；
    其余两槽原样达标。<br>
    主题系统（theme-wallpaper-split §3 调整⑤）落地后，"深浅两套共 8 个常量"被
    <b>"每套主题自带 4 槽"</b>自然覆盖 —— 换主题时频道色随主题走，
    本章 <code>channel-color-dark/light</code> 的命名只适用于迁移保留档内部。
  </div>
${T_CLOSE}
`;
const tAnchor = '报假警会让人不再信任它，此后所有真问题都会被忽略。\n    </div>';
if (!thtml.includes(tAnchor)) { console.error('找不到 tab 稿第四章锚点'); process.exit(1); }
thtml = thtml.replace(tAnchor, tAnchor + '\n' + tBox);
fs.writeFileSync(TP, thtml, 'utf8');
console.log('注入完成：频道色数据注记 →', path.basename(TP));
