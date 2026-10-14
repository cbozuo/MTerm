// 主题落地的验收对拍：palettes.rs(代码现状) vs 高保真目标数据(D.keep 28 套)
// 用法: node scripts/theme_parity_check.js
// 验收线：28 套 × (UI 20 槽 + ANSI16 + group24 + channel) 全部 0 差异。
const fs = require('fs');
const path = require('path');
const D = require('../output/theme-dedupe-target.json');
const src = fs.readFileSync(path.join(__dirname, '..', 'src', 'theme', 'palettes.rs'), 'utf8');

const UP = (s) => String(s).toUpperCase();
const blocks = src.split(/\n    Palette \{/).slice(1);
const hex = (n) => '#' + n.toString(16).toUpperCase().padStart(6, '0');
const code = [];
for (const b of blocks) {
  const id = /id:\s*"([^"]+)"/.exec(b)[1];
  const one = (k) => hex(parseInt(new RegExp(k + ':\\s*0x([0-9A-Fa-f]+)').exec(b)[1], 16));
  const many = (k) => new RegExp(k + ':\\s*\\[([^\\]]+)\\]').exec(b)[1]
    .split(',').map(s => hex(parseInt(s.trim().replace('0x', ''), 16)));
  code.push({
    id,
    u: {
      root: one('root'), panel: one('panel'), palt: one('palt'), elev: one('elev'),
      hov: one('hov'), act: one('act'), tab: one('tab'), line: one('line'), lstr: one('lstr'),
      t1: one('t1'), t2: one('t2'), t3: one('t3'), tbg: one('tbg'), tfg: one('tfg'),
      ac: one('ac'), ac2: one('ac2'), ok: one('ok'), wr: one('wr'), dg: one('dg'), dg_s: one('dg_s'),
    },
    ansi: many('ansi16'),
    g24: many('group24'),
    ch: many('channel'),
  });
}
const byId = Object.fromEntries(code.map(p => [p.id, p]));
const U_KEYS = Object.keys(code[0].u);

let pass = 0, fail = 0;
const failLines = [];
for (const t of D.keep) {
  const c = byId[t.id];
  if (!c) { fail++; failLines.push(`✗ ${t.id}  代码中缺失`); continue; }
  // 目标 = { ...u, ...opt 的 20 槽 }；ansi / g24 / ch 均 opt 优先（D 的两段式：
  // 门面/新增套直接填目标；算子套的修正值在 opt 覆盖层）。
  const optU = {};
  for (const k of U_KEYS) if (t.opt && t.opt[k] !== undefined) optU[k] = t.opt[k];
  const tU = { ...t.u, ...optU };
  const tAnsi = (t.opt && t.opt.ansi) ? t.opt.ansi : t.ansi;
  const tG24 = (t.opt && t.opt.g24) ? t.opt.g24 : t.g24;
  const tCh = (t.opt && t.opt.ch) ? t.opt.ch : t.ch;
  const bad = [];
  for (const k of U_KEYS) if (UP(tU[k]) !== UP(c.u[k])) bad.push(`u.${k}: ${c.u[k]}→目标 ${tU[k]}`);
  (tAnsi).forEach((v, i) => { if (UP(v) !== UP(c.ansi[i])) bad.push(`ansi${i}: ${c.ansi[i]}→${v}`); });
  (tG24).forEach((v, i) => { if (UP(v) !== UP(c.g24[i])) bad.push(`g24[${i}]: ${c.g24[i]}→${v}`); });
  (tCh).forEach((v, i) => { if (UP(v) !== UP(c.ch[i])) bad.push(`ch${i}: ${c.ch[i]}→${v}`); });
  if (bad.length === 0) pass++;
  else { fail++; failLines.push(`✗ ${t.id}  ${bad.length} 处差异`); failLines.push('    ' + bad.slice(0, 6).join('; ') + (bad.length > 6 ? ` …(${bad.length - 6} 处省略)` : '')); }
}
// 代码中不属于目标 28 套的（应为空；落地中段会列出待下线套）
const tgtIds = new Set(D.keep.map(t => t.id));
const extra = code.filter(p => !tgtIds.has(p.id)).map(p => p.id);

console.log(`═══ 对拍结果：${pass} / ${D.keep.length} 套全槽一致 ═══`);
if (failLines.length) { console.log(failLines.join('\n')); }
console.log(`代码共 ${code.length} 套；目标外（应下线）: ${extra.length ? extra.join(', ') : '无 ✓'}`);
if (pass === D.keep.length && extra.length === 0) console.log('🎯 100% 达成：28 套全部与高保真目标一致，且无多余套。');
