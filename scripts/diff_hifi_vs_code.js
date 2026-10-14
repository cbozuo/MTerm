// 对比高保真目标数据（D.keep 28 套）与当前 src/theme/palettes.rs（38 套）—— 规范化版
// 用法: node scripts/diff_hifi_vs_code.js
const fs = require('fs');
const path = require('path');
const D = require('../output/theme-dedupe-target.json');
const src = fs.readFileSync(path.join(__dirname, '..', 'src', 'theme', 'palettes.rs'), 'utf8');

const UP = (s) => s.toUpperCase();
const blocks = src.split(/\n    Palette \{/).slice(1);
const hex = (n) => '#' + n.toString(16).toUpperCase().padStart(6, '0');
const code = [];
for (const b of blocks) {
  const id = /id:\s*"([^"]+)"/.exec(b)[1];
  const zh = /zh:\s*"([^"]+)"/.exec(b)[1];
  const dark = /dark:\s*(true|false)/.exec(b)[1] === 'true';
  const kind = /kind:\s*"([^"]+)"/.exec(b)[1];
  const one = (k) => hex(parseInt(new RegExp(k + ':\\s*0x([0-9A-Fa-f]+)').exec(b)[1], 16));
  const many = (k) => {
    const m = new RegExp(k + ':\\s*\\[([^\\]]+)\\]').exec(b);
    return m[1].split(',').map(s => hex(parseInt(s.trim().replace('0x', ''), 16)));
  };
  code.push({
    id, zh, dark, kind,
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

const RENAME = { 'mterm-dark': 'islands-dark', 'mterm-light': 'islands-light', 'darcula-dark': 'idea-dark', 'darcula-light': 'idea-light', 'onedark-light': 'github-light' };
const UNITS = { u: 'UI 槽', ansi: 'ANSI16', g24: 'group24', ch: 'channel' };
const optSkip = new Set(['g24', 'ch', 'ansi', 'fixedAnsi', 'locked', 't3k', 't3same']);

const lines = [];
let nChange = 0, nNew = 0;
for (const t of D.keep) {
  const curId = RENAME[t.id] || t.id;
  const c = byId[curId];
  if (!c) { nNew++; lines.push(`\n### ${t.id}  ★新增套（代码中无 ${curId}）`); continue; }
  const diffs = {};
  for (const sec of Object.keys(UNITS)) {
    const a = sec === 'u' ? t.u : t[sec];
    const b = sec === 'u' ? c.u : c[sec];
    for (const k of Object.keys(a)) {
      if (UP(a[k]) !== UP(b[k])) (diffs[sec] = diffs[sec] || []).push(`${sec === 'u' ? k : ''} ${b[k]} → ${a[k]}`.trim());
    }
  }
  const optUI = [];
  for (const k of Object.keys(t.opt)) if (!optSkip.has(k) && UP(t.opt[k]) !== UP(t.u[k])) optUI.push(`${k}: ${t.u[k]} → ${t.opt[k]}`);
  const ansiFixed = [];
  (t.opt.ansi || []).forEach((v, i) => { if (UP(v) !== UP(t.ansi[i])) ansiFixed.push(`a${i} ${t.ansi[i]}→${v}`); });
  let g24Fixed = [];
  (t.opt.g24 || []).forEach((v, i) => { if (UP(v) !== UP(t.g24[i])) g24Fixed.push(i); });
  const has = Object.keys(diffs).length || ansiFixed.length || g24Fixed.length || optUI.length;
  if (!has) { lines.push(`\n--- ${t.id} (${curId})  色值一致`); continue; }
  nChange++;
  lines.push(`\n### ${t.id} (${curId})   ${t.zh}`);
  for (const sec of Object.keys(UNITS)) if (diffs[sec]) lines.push(`  [${UNITS[sec]}] ${diffs[sec].join('; ')}`);
  if (optUI.length) lines.push(`  [优化算子·UI] ${optUI.join('; ')}`);
  if (ansiFixed.length) lines.push(`  [优化算子·ANSI] ${ansiFixed.join('; ')}`);
  if (g24Fixed.length) lines.push(`  [优化算子·group24] 修正槽 ${g24Fixed.join(',')}`);
}
lines.push(`\n===== 汇总: 目标 28 套 | 有差异 ${nChange} 套 | 新增 ${nNew} 套 | 下线 ${D.dropIds.length} 套 =====`);
lines.push('代码中将被下线的(不在目标 28): ' + code.filter(p => !new Set(D.keep.map(t => RENAME[t.id] || t.id)).has(p.id)).map(p => p.id).join(', '));

fs.writeFileSync(path.join(__dirname, '..', 'output', 'theme-dedupe-diff.txt'), lines.join('\n'), 'utf8');
console.log(lines.join('\n'));
